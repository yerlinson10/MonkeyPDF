use crate::error::{AppError, OpResult};
use crate::pdf_engine::{ensure_parent_dir, ensure_pdf_path};
use lopdf::{Document, Object, ObjectId};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    pub encrypted: bool,
    pub pdf_version: String,
    pub page_count: u32,
    pub has_xref_stream: bool,
    pub has_eof: bool,
    pub broken_objects: u32,
    pub orphan_objects: u32,
    pub missing_pages: u32,
    pub linearized: bool,
    pub warnings: Vec<String>,
}

/// Diagnose structural issues in a PDF (best-effort, does not modify the file).
pub fn diagnose_pdf(path: String) -> Result<Diagnosis, AppError> {
    let input = ensure_pdf_path(&path)?;
    let bytes = std::fs::read(&input)?;
    let has_eof = bytes.windows(5).any(|w| w == b"%%EOF");
    let has_xref_stream = String::from_utf8_lossy(&bytes).contains("/Type /XRef")
        || String::from_utf8_lossy(&bytes).contains("/Type/XRef");
    let linearized = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).contains("/Linearized");

    let version = if bytes.starts_with(b"%PDF-") {
        String::from_utf8_lossy(&bytes[5..bytes.len().min(8)])
            .trim()
            .to_string()
    } else {
        "?".into()
    };

    let mut warnings = Vec::new();
    if !has_eof {
        warnings.push("Falta marcador %%EOF".into());
    }
    if linearized {
        warnings.push("PDF linearizado (se normalizará al reparar)".into());
    }

    let doc = match Document::load(&input) {
        Ok(d) => d,
        Err(e) => {
            warnings.push(format!("No se pudo cargar con lopdf: {e}"));
            return Ok(Diagnosis {
                encrypted: false,
                pdf_version: version,
                page_count: 0,
                has_xref_stream,
                has_eof,
                broken_objects: 0,
                orphan_objects: 0,
                missing_pages: 0,
                linearized,
                warnings,
            });
        }
    };

    let encrypted = doc.is_encrypted();
    if encrypted {
        warnings.push("PDF cifrado — se necesita contraseña para reparar".into());
    }

    let pages = doc.get_pages();
    let page_count = pages.len() as u32;
    let mut missing_pages = 0u32;
    for (n, id) in &pages {
        match doc.get_object(*id).and_then(|o| o.as_dict()) {
            Ok(dict) => {
                if dict.get(b"MediaBox").is_err() && dict.get(b"CropBox").is_err() {
                    missing_pages += 1;
                    warnings.push(format!("Página {n} sin MediaBox/CropBox"));
                }
            }
            Err(_) => {
                missing_pages += 1;
                warnings.push(format!("Página {n} ilegible"));
            }
        }
    }

    let mut broken_objects = 0u32;
    for (id, obj) in &doc.objects {
        if let Object::Stream(stream) = obj {
            let declared = stream
                .dict
                .get(b"Length")
                .ok()
                .and_then(|o| match o {
                    Object::Integer(i) => Some(*i as usize),
                    Object::Reference(r) => doc
                        .get_object(*r)
                        .ok()
                        .and_then(|o| o.as_i64().ok())
                        .map(|i| i as usize),
                    _ => None,
                });
            if let Some(len) = declared {
                if stream.content.len() != len && stream.content.len() + 1 != len {
                    // Allow small mismatch; flag larger ones
                    if (stream.content.len() as i64 - len as i64).abs() > 2 {
                        broken_objects += 1;
                        warnings.push(format!(
                            "Stream {} {} Length inconsistente ({} vs {})",
                            id.0,
                            id.1,
                            stream.content.len(),
                            len
                        ));
                    }
                }
            }
        }
    }

    let reachable = collect_reachable(&doc);
    let orphan_objects = doc
        .objects
        .keys()
        .filter(|id| !reachable.contains(id))
        .count() as u32;
    if orphan_objects > 0 {
        warnings.push(format!("{orphan_objects} objeto(s) huérfano(s)"));
    }

    if warnings.is_empty() {
        warnings.push("Sin problemas evidentes — el re-guardado limpiará la estructura".into());
    }

    Ok(Diagnosis {
        encrypted,
        pdf_version: version,
        page_count,
        has_xref_stream,
        has_eof,
        broken_objects,
        orphan_objects,
        missing_pages,
        linearized,
        warnings,
    })
}

/// Best-effort repair: decrypt if needed, fix stream lengths, drop orphans, renumber, re-save.
pub fn repair_pdf(
    path: String,
    output: String,
    password: Option<String>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    let mut doc = Document::load(&input)?;
    if doc.is_encrypted() {
        let pwd = password.unwrap_or_default();
        if pwd.is_empty() {
            return Err(AppError::InvalidInput(
                "PDF cifrado: indica la contraseña para reparar".into(),
            ));
        }
        // Try decrypt via authenticate if available; otherwise fail clearly.
        #[allow(deprecated)]
        {
            if let Err(e) = doc.decrypt(&pwd) {
                return Err(AppError::InvalidInput(format!(
                    "No se pudo descifrar: {e}"
                )));
            }
        }
    }

    // Fix stream Length values to match content.
    let mut fixed_streams = 0u32;
    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in ids {
        let Ok(obj) = doc.get_object(id) else {
            continue;
        };
        if let Object::Stream(stream) = obj {
            let content_len = stream.content.len() as i64;
            let needs_fix = match stream.dict.get(b"Length") {
                Ok(Object::Integer(i)) => *i != content_len,
                Ok(Object::Reference(_)) => true,
                _ => true,
            };
            if needs_fix {
                let mut stream = stream.clone();
                stream.dict.set("Length", content_len);
                doc.objects.insert(id, Object::Stream(stream));
                fixed_streams += 1;
            }
        }
    }
    let _ = fixed_streams;

    // Drop orphan objects not reachable from catalog.
    let reachable = collect_reachable(&doc);
    let orphans: Vec<ObjectId> = doc
        .objects
        .keys()
        .copied()
        .filter(|id| !reachable.contains(id))
        .collect();
    for id in orphans {
        doc.objects.remove(&id);
    }

    // Ensure pages have MediaBox.
    let pages = doc.get_pages();
    for (_n, page_id) in pages {
        if let Ok(Object::Dictionary(dict)) = doc.get_object(page_id).cloned() {
            if dict.get(b"MediaBox").is_err() && dict.get(b"CropBox").is_err() {
                let mut d = dict;
                d.set(
                    "MediaBox",
                    vec![
                        Object::Integer(0),
                        Object::Integer(0),
                        Object::Integer(595),
                        Object::Integer(842),
                    ],
                );
                doc.objects.insert(page_id, Object::Dictionary(d));
            }
        }
    }

    // Strip encryption leftovers (lives on trailer).
    doc.trailer.remove(b"Encrypt");

    let page_count = doc.get_pages().len() as u32;
    doc.renumber_objects();
    doc.compress();
    doc.save(output_path)?;

    Ok(OpResult::new(
        vec![output],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn collect_reachable(doc: &Document) -> HashSet<ObjectId> {
    let mut seen = HashSet::new();
    let mut stack = Vec::new();

    if let Ok(root) = doc.trailer.get(b"Root").and_then(|o| o.as_reference()) {
        stack.push(root);
    }
    if let Ok(info) = doc.trailer.get(b"Info").and_then(|o| o.as_reference()) {
        stack.push(info);
    }
    if let Ok(Object::Array(ids)) = doc.trailer.get(b"ID") {
        for o in ids {
            if let Ok(r) = o.as_reference() {
                stack.push(r);
            }
        }
    }

    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Ok(obj) = doc.get_object(id) else {
            continue;
        };
        push_refs(obj, &mut stack);
    }
    seen
}

fn push_refs(obj: &Object, stack: &mut Vec<ObjectId>) {
    match obj {
        Object::Reference(id) => stack.push(*id),
        Object::Array(arr) => {
            for o in arr {
                push_refs(o, stack);
            }
        }
        Object::Dictionary(dict) => {
            for (_, v) in dict.iter() {
                push_refs(v, stack);
            }
        }
        Object::Stream(stream) => {
            for (_, v) in stream.dict.iter() {
                push_refs(v, stack);
            }
        }
        _ => {}
    }
}

