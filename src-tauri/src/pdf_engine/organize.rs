use crate::error::{AppError, OpResult};
use crate::pdf_engine::merge::merge_documents;
use crate::pdf_engine::{ensure_parent_dir, ensure_pdf_path};
use lopdf::{Document, Object};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageRef {
    pub source_path: String,
    /// 1-based page index in the source PDF
    pub page: u32,
    /// Additional rotation to apply (0/90/180/270)
    #[serde(default)]
    pub rotate: u32,
}

/// Build a new PDF from an ordered list of page references (possibly from multiple files).
pub fn organize_pdf(pages: Vec<PageRef>, output: String) -> Result<OpResult, AppError> {
    let started = Instant::now();
    if pages.is_empty() {
        return Err(AppError::InvalidInput(
            "Añade al menos una página".into(),
        ));
    }

    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    // Cache loaded source documents
    let mut sources: HashMap<String, Document> = HashMap::new();
    for pref in &pages {
        if !sources.contains_key(&pref.source_path) {
            let p = ensure_pdf_path(&pref.source_path)?;
            let doc = Document::load(p)?;
            if doc.is_encrypted() {
                return Err(AppError::InvalidInput(format!(
                    "Desbloquea primero: {}",
                    pref.source_path
                )));
            }
            sources.insert(pref.source_path.clone(), doc);
        }
        let doc = sources.get(&pref.source_path).unwrap();
        let total = doc.get_pages().len() as u32;
        if pref.page == 0 || pref.page > total {
            return Err(AppError::InvalidInput(format!(
                "Página {} fuera de rango en {} (1-{total})",
                pref.page, pref.source_path
            )));
        }
        if !matches!(pref.rotate, 0 | 90 | 180 | 270) {
            return Err(AppError::InvalidInput(
                "Rotación debe ser 0, 90, 180 o 270".into(),
            ));
        }
    }

    // Extract each requested page the same way as split (clone + delete others).
    // Avoids deep-copy bugs with Parent/Kids and missing object refs in real PDFs.
    let mut parts: Vec<Document> = Vec::with_capacity(pages.len());
    for pref in &pages {
        let src = sources.get(&pref.source_path).unwrap();
        let mut part = extract_single_page(src, pref.page)?;
        if pref.rotate != 0 {
            apply_rotate(&mut part, pref.rotate)?;
        }
        parts.push(part);
    }

    let page_count = merge_documents(parts, output_path)?;

    Ok(OpResult::new(
        vec![output],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn extract_single_page(source: &Document, page: u32) -> Result<Document, AppError> {
    let mut doc = source.clone();
    let pages = doc.get_pages();
    let to_delete: Vec<u32> = pages.keys().copied().filter(|n| *n != page).collect();
    for page_num in to_delete.into_iter().rev() {
        doc.delete_pages(&[page_num]);
    }
    Ok(doc)
}

fn apply_rotate(doc: &mut Document, angle: u32) -> Result<(), AppError> {
    let pages = doc.get_pages();
    let Some((_, &page_id)) = pages.iter().next() else {
        return Ok(());
    };
    let page_dict = doc
        .get_object_mut(page_id)
        .and_then(|obj| obj.as_dict_mut())
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let current = page_dict
        .get(b"Rotate")
        .and_then(Object::as_i64)
        .unwrap_or(0);
    page_dict.set("Rotate", (current + angle as i64).rem_euclid(360));
    Ok(())
}
