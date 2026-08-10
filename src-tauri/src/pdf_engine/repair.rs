use crate::error::{AppError, OpResult};
use crate::pdf_engine::{create_pdfium, ensure_parent_dir, ensure_pdf_path};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use pdfium_render::prelude::FPDF_FILEWRITE;
use serde::Serialize;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::os::raw::{c_int, c_ulong, c_void};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Matches pdfium `fpdf_save.h` flags.
const FPDF_NO_INCREMENTAL: u32 = 2;
const FPDF_REMOVE_SECURITY: u32 = 3;

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
    /// True when lopdf cannot parse the xref (needs rewrite/salvage).
    pub xref_broken: bool,
    /// Objects found by brute-force / ObjStm expansion.
    pub scanned_objects: u32,
    /// Page-like objects found by salvage heuristics.
    pub recoverable_pages: u32,
    pub warnings: Vec<String>,
}

/// Diagnose structural issues in a PDF (best-effort, does not modify the file).
pub fn diagnose_pdf(path: String) -> Result<Diagnosis, AppError> {
    let input = ensure_pdf_path(&path)?;
    let raw = std::fs::read(&input)?;
    let bytes = normalize_pdf_bytes(&raw);
    let has_eof = bytes.windows(5).any(|w| w == b"%%EOF");
    let lossy = String::from_utf8_lossy(&bytes);
    let has_xref_stream =
        lossy.contains("/Type /XRef") || lossy.contains("/Type/XRef");
    let encrypt_marker = lossy.contains("/Encrypt");
    let linearized =
        String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).contains("/Linearized");

    let version = if bytes.starts_with(b"%PDF-") {
        String::from_utf8_lossy(&bytes[5..bytes.len().min(8)])
            .trim()
            .to_string()
    } else {
        "?".into()
    };

    let mut warnings = Vec::new();
    if !bytes.windows(5).any(|w| w == b"%PDF-") {
        warnings.push("No se encontró cabecera %PDF- (puede no ser un PDF)".into());
    }
    if !has_eof {
        warnings.push("Falta marcador %%EOF".into());
    }
    if linearized {
        warnings.push("PDF linearizado (se normalizará al reparar)".into());
    }
    if bytes.as_slice() != raw.as_slice() {
        warnings.push("Se detectó basura antes/después del PDF (se normalizará)".into());
    }
    if encrypt_marker {
        warnings.push(
            "Se detectó /Encrypt — si otro visor pide contraseña, indícala aquí".into(),
        );
    }

    let expanded = extract_objects_expanded(&bytes);
    let scanned_objects = expanded.len() as u32;
    let recoverable_pages = find_page_ids_in(&expanded).len() as u32;
    if scanned_objects > 0 {
        warnings.push(format!(
            "Escaneo: {scanned_objects} objeto(s), {recoverable_pages} página(s) candidatas"
        ));
    } else {
        warnings.push(
            "Escaneo: 0 objetos — el archivo puede estar truncado, vacío o no ser un PDF"
                .into(),
        );
    }

    let pdfium_pages = count_pages_pdfium(&bytes, None)
        .or_else(|| count_pages_pdfium(&bytes, Some("")));

    let doc = match Document::load_mem(&bytes) {
        Ok(d) => d,
        Err(e) => {
            let msg = e.to_string();
            let xref_broken = msg.to_ascii_lowercase().contains("cross reference")
                || msg.to_ascii_lowercase().contains("xref")
                || msg.to_ascii_lowercase().contains("invalid start");
            warnings.push(format!("lopdf no pudo parsear: {msg}"));
            if xref_broken {
                warnings.push(
                    "Tabla xref dañada — la reparación intentará reconstruir la estructura"
                        .into(),
                );
            }
            if recoverable_pages == 0 {
                warnings.push(
                    "Sin páginas recuperables: no hay garantía de poder reparar este archivo"
                        .into(),
                );
            }
            return Ok(Diagnosis {
                encrypted: encrypt_marker,
                pdf_version: version,
                page_count: pdfium_pages.unwrap_or(recoverable_pages),
                has_xref_stream,
                has_eof,
                broken_objects: if xref_broken { 1 } else { 0 },
                orphan_objects: 0,
                missing_pages: 0,
                linearized,
                xref_broken,
                scanned_objects,
                recoverable_pages,
                warnings,
            });
        }
    };

    let encrypted = doc.is_encrypted() || encrypt_marker;
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
            let declared = stream.dict.get(b"Length").ok().and_then(|o| match o {
                Object::Integer(i) => Some(*i as usize),
                Object::Reference(r) => doc
                    .get_object(*r)
                    .ok()
                    .and_then(|o| o.as_i64().ok())
                    .map(|i| i as usize),
                _ => None,
            });
            if let Some(len) = declared {
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
        xref_broken: false,
        scanned_objects: scanned_objects.max(doc.objects.len() as u32),
        recoverable_pages: recoverable_pages.max(page_count),
        warnings,
    })
}

/// Best-effort repair with multiple recovery strategies.
pub fn repair_pdf(
    path: String,
    output: String,
    password: Option<String>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = PathBuf::from(&output);
    ensure_parent_dir(&output_path)?;
    let pwd = password.unwrap_or_default();
    let raw = std::fs::read(&input).map_err(AppError::Io)?;
    let normalized = normalize_pdf_bytes(&raw);

    let mut errors: Vec<String> = Vec::new();

    // 1) lopdf on original / normalized
    for (label, bytes) in [("original", raw.as_slice()), ("normalizado", normalized.as_slice())] {
        match try_lopdf_cleanup(bytes, &pwd, &output_path) {
            Ok(pages) => {
                return Ok(OpResult::new(
                    vec![output],
                    pages,
                    started.elapsed().as_millis() as u64,
                ));
            }
            Err(e) => {
                log::warn!("lopdf ({label}) failed: {e}");
                errors.push(format!("lopdf/{label}: {e}"));
            }
        }
    }

    // 1b) Embedded libqpdf — strongest structural recovery (vendored).
    for (label, bytes) in [("original", raw.as_slice()), ("normalizado", normalized.as_slice())] {
        match repair_with_libqpdf(bytes, &output_path, &pwd) {
            Ok(pages) => {
                polish_output(&output_path);
                return Ok(OpResult::new(
                    vec![output],
                    pages,
                    started.elapsed().as_millis() as u64,
                ));
            }
            Err(e) => {
                log::warn!("libqpdf ({label}) failed: {e}");
                errors.push(format!("libqpdf/{label}: {e}"));
            }
        }
    }

    // 1c) Expand ObjStm → classical objects, then retry parsers.
    match classicalize_pdf(&normalized) {
        Ok(classical) => {
            match try_lopdf_cleanup(&classical, &pwd, &output_path) {
                Ok(pages) => {
                    return Ok(OpResult::new(
                        vec![output],
                        pages,
                        started.elapsed().as_millis() as u64,
                    ));
                }
                Err(e) => errors.push(format!("lopdf/classical: {e}")),
            }
            match repair_with_libqpdf(&classical, &output_path, &pwd) {
                Ok(pages) => {
                    polish_output(&output_path);
                    return Ok(OpResult::new(
                        vec![output],
                        pages,
                        started.elapsed().as_millis() as u64,
                    ));
                }
                Err(e) => errors.push(format!("libqpdf/classical: {e}")),
            }
            match repair_with_pdfium_bytes(&classical, &output_path, &pwd) {
                Ok(pages) => {
                    polish_output(&output_path);
                    return Ok(OpResult::new(
                        vec![output],
                        pages,
                        started.elapsed().as_millis() as u64,
                    ));
                }
                Err(e) => errors.push(format!("pdfium/classical: {e}")),
            }
            if std::fs::write(&output_path, &classical).is_ok() {
                if let Ok(pages) = verify_page_count(&output_path) {
                    if pages > 0 {
                        polish_output(&output_path);
                        return Ok(OpResult::new(
                            vec![output],
                            pages,
                            started.elapsed().as_millis() as u64,
                        ));
                    }
                }
            }
        }
        Err(e) => errors.push(format!("classicalize: {e}")),
    }

    if let Ok(fixed) = rebuild_xref_bytes(&normalized) {
        match try_lopdf_cleanup(&fixed, &pwd, &output_path) {
            Ok(pages) => {
                return Ok(OpResult::new(
                    vec![output],
                    pages,
                    started.elapsed().as_millis() as u64,
                ));
            }
            Err(e) => {
                log::warn!("lopdf after xref rebuild failed: {e}");
                errors.push(format!("lopdf/xref-rebuild: {e}"));
            }
        }
        match repair_with_libqpdf(&fixed, &output_path, &pwd) {
            Ok(pages) => {
                polish_output(&output_path);
                return Ok(OpResult::new(
                    vec![output],
                    pages,
                    started.elapsed().as_millis() as u64,
                ));
            }
            Err(e) => errors.push(format!("libqpdf/xref-rebuild: {e}")),
        }
        match repair_with_pdfium_bytes(&fixed, &output_path, &pwd) {
            Ok(pages) => {
                polish_output(&output_path);
                return Ok(OpResult::new(
                    vec![output],
                    pages,
                    started.elapsed().as_millis() as u64,
                ));
            }
            Err(e) => {
                log::warn!("pdfium after xref rebuild failed: {e}");
                errors.push(format!("pdfium/xref-rebuild: {e}"));
            }
        }
    }

    // 3) External CLIs (qpdf / mutool / ghostscript) — best structural healers
    for (name, attempt) in [
        ("qpdf", repair_cli_qpdf as fn(&Path, &Path, &str) -> Result<u32, AppError>),
        ("mutool", repair_cli_mutool),
        ("ghostscript", repair_cli_ghostscript),
    ] {
        match attempt(&input, &output_path, &pwd) {
            Ok(pages) => {
                polish_output(&output_path);
                return Ok(OpResult::new(
                    vec![output],
                    pages,
                    started.elapsed().as_millis() as u64,
                ));
            }
            Err(e) => {
                log::warn!("{name} repair failed: {e}");
                errors.push(format!("{name}: {e}"));
            }
        }
        // Also try against a temp normalized file
        let tmp = output_path.with_extension("norm.tmp.pdf");
        if std::fs::write(&tmp, &normalized).is_ok() {
            match attempt(&tmp, &output_path, &pwd) {
                Ok(pages) => {
                    let _ = std::fs::remove_file(&tmp);
                    polish_output(&output_path);
                    return Ok(OpResult::new(
                        vec![output],
                        pages,
                        started.elapsed().as_millis() as u64,
                    ));
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    errors.push(format!("{name}/norm: {e}"));
                }
            }
        }
    }

    // 4) PDFium rewrite on original / normalized
    for (label, bytes) in [("original", raw.as_slice()), ("normalizado", normalized.as_slice())] {
        match repair_with_pdfium_bytes(bytes, &output_path, &pwd) {
            Ok(pages) => {
                polish_output(&output_path);
                return Ok(OpResult::new(
                    vec![output],
                    pages,
                    started.elapsed().as_millis() as u64,
                ));
            }
            Err(e) => {
                log::warn!("pdfium ({label}) failed: {e}");
                errors.push(format!("pdfium/{label}: {e}"));
            }
        }
    }

    // 5) Nuclear: rewrite a brand-new PDF from scanned objects (+ page tree)
    match rebuild_fresh_pdf(&normalized) {
        Ok(fresh) => {
            match try_lopdf_cleanup(&fresh, &pwd, &output_path) {
                Ok(pages) => {
                    return Ok(OpResult::new(
                        vec![output],
                        pages,
                        started.elapsed().as_millis() as u64,
                    ));
                }
                Err(e) => errors.push(format!("lopdf/fresh: {e}")),
            }
            match repair_with_pdfium_bytes(&fresh, &output_path, &pwd) {
                Ok(pages) => {
                    polish_output(&output_path);
                    return Ok(OpResult::new(
                        vec![output],
                        pages,
                        started.elapsed().as_millis() as u64,
                    ));
                }
                Err(e) => errors.push(format!("pdfium/fresh: {e}")),
            }
            // Even if parsers are picky, write the rebuilt bytes — often open elsewhere.
            if let Err(e) = std::fs::write(&output_path, &fresh) {
                errors.push(format!("write fresh: {e}"));
            } else if let Ok(pages) = verify_page_count(&output_path) {
                if pages > 0 {
                    return Ok(OpResult::new(
                        vec![output],
                        pages,
                        started.elapsed().as_millis() as u64,
                    ));
                }
            }
        }
        Err(e) => {
            log::warn!("fresh rebuild failed: {e}");
            errors.push(format!("fresh-rebuild: {e}"));
        }
    }

    // 6) Last resort: carve whatever intact content remains (images, text, pages).
    match salvage_partial(&normalized, &output_path) {
        Ok((pages, notes)) if pages > 0 => {
            return Ok(OpResult::partial(
                vec![output],
                pages,
                started.elapsed().as_millis() as u64,
                notes,
            ));
        }
        Ok(_) => errors.push("partial-salvage: nada recuperable".into()),
        Err(e) => {
            log::warn!("partial salvage failed: {e}");
            errors.push(format!("partial-salvage: {e}"));
        }
    }

    // Prefer actionable errors; skip "tool not installed" noise at the end.
    let actionable: Vec<String> = errors
        .iter()
        .filter(|e| {
            let l = e.to_ascii_lowercase();
            !l.contains("no instalado") && !l.contains("not found")
        })
        .cloned()
        .collect();
    let detail_src = if actionable.is_empty() {
        &errors
    } else {
        &actionable
    };
    let detail = detail_src
        .iter()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ");
    Err(AppError::Pdf(format!(
        "No se pudo reparar ni rescatar fragmentos: el archivo no tiene \
         páginas, imágenes ni texto recuperables. {detail}"
    )))
}

fn polish_output(path: &Path) {
    if let Ok(mut doc) = Document::load(path) {
        let _ = cleanup_and_save(&mut doc, path);
    }
}

/// Best-effort carve: intact pages, embedded JPEG/PNG, and readable text strings.
fn salvage_partial(data: &[u8], output_path: &Path) -> Result<(u32, Vec<String>), AppError> {
    let mut notes = Vec::new();
    notes.push(
        "Recuperación parcial: el PDF original no se pudo reconstruir completo; \
         este archivo solo contiene fragmentos intactos."
            .into(),
    );

    let objs = extract_objects_expanded(data);
    let page_ids = find_page_ids_in(&objs);
    let jpegs = carve_jpegs(data);
    let pngs = carve_pngs(data);
    let texts = extract_readable_strings(data);

    notes.push(format!(
        "Encontrado: {} página(s) candidatas, {} JPEG, {} PNG, {} fragmentos de texto",
        page_ids.len(),
        jpegs.len(),
        pngs.len(),
        texts.len()
    ));

    if page_ids.is_empty() && jpegs.is_empty() && pngs.is_empty() && texts.is_empty() {
        return Err(AppError::Pdf("Sin fragmentos recuperables".into()));
    }

    // Prefer classicalize if we have real pages.
    if !page_ids.is_empty() {
        if let Ok(classical) = classicalize_pdf(data) {
            std::fs::write(output_path, &classical).map_err(AppError::Io)?;
            if let Ok(n) = verify_page_count(output_path) {
                if n > 0 {
                    notes.push(format!("Se reconstruyeron {n} página(s) desde objetos intactos"));
                    return Ok((n, notes));
                }
            }
        }
    }

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();

    // Cover note page
    {
        let mut lines = vec![
            "RECUPERACION PARCIAL — MonkeyPDF".into(),
            String::new(),
            "No se pudo reparar la estructura completa del PDF.".into(),
            "Este documento contiene solo lo que aún era legible.".into(),
            String::new(),
        ];
        lines.extend(notes.iter().cloned());
        if !texts.is_empty() {
            lines.push(String::new());
            lines.push(format!("Texto rescatado ({} fragmentos):", texts.len().min(40)));
        }
        kids.push(add_text_page(&mut doc, pages_id, &lines, 10.0)?);
    }

    // Image pages from carved JPEGs
    for (i, jpeg) in jpegs.iter().take(50).enumerate() {
        match add_jpeg_page(&mut doc, pages_id, jpeg, i) {
            Ok(id) => kids.push(id),
            Err(e) => log::warn!("skip carved jpeg {i}: {e}"),
        }
    }

    // PNG → re-encode as JPEG page
    for (i, png) in pngs.iter().take(30).enumerate() {
        match add_png_page(&mut doc, pages_id, png, i) {
            Ok(id) => kids.push(id),
            Err(e) => log::warn!("skip carved png {i}: {e}"),
        }
    }

    // Extra text dump pages (chunked)
    if !texts.is_empty() {
        let chunk: Vec<String> = texts.into_iter().take(200).collect();
        for part in chunk.chunks(35) {
            let mut lines = vec!["Texto rescatado del PDF dañado:".into(), String::new()];
            lines.extend(part.iter().cloned());
            if let Ok(id) = add_text_page(&mut doc, pages_id, &lines, 9.0) {
                kids.push(id);
            }
        }
    }

    if kids.is_empty() {
        return Err(AppError::Pdf("No se pudo armar ninguna página de rescate".into()));
    }

    let page_count = kids.len() as u32;
    let kid_refs: Vec<Object> = kids.iter().map(|&id| Object::Reference(id)).collect();
    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Count", page_count as i64);
    pages.set("Kids", kid_refs);
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);
    doc.renumber_objects();
    doc.compress();
    doc.save(output_path)?;

    notes.push(format!("PDF de rescate generado con {page_count} página(s)"));
    Ok((page_count, notes))
}

fn add_text_page(
    doc: &mut Document,
    pages_id: ObjectId,
    lines: &[String],
    font_size: f32,
) -> Result<ObjectId, AppError> {
    let mut font = Dictionary::new();
    font.set("Type", "Font");
    font.set("Subtype", "Type1");
    font.set("BaseFont", "Helvetica");
    let font_id = doc.add_object(font);
    let mut fonts = Dictionary::new();
    fonts.set("F1", font_id);
    let mut resources = Dictionary::new();
    resources.set("Font", fonts);

    let mut content = String::from("BT /F1 ");
    content.push_str(&format!("{font_size:.1} Tf 40 800 Td\n"));
    let leading = (font_size + 4.0).max(12.0);
    content.push_str(&format!("{leading:.1} TL\n"));
    for (i, line) in lines.iter().enumerate() {
        let safe = pdf_escape_text(line);
        if i == 0 {
            content.push_str(&format!("({safe}) Tj\n"));
        } else {
            content.push_str(&format!("T* ({safe}) Tj\n"));
        }
    }
    content.push_str("ET\n");

    let content_id = doc.add_object(Object::Stream(Stream::new(
        Dictionary::new(),
        content.into_bytes(),
    )));

    let mut page = Dictionary::new();
    page.set("Type", "Page");
    page.set("Parent", pages_id);
    page.set(
        "MediaBox",
        vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(595),
            Object::Integer(842),
        ],
    );
    page.set("Contents", content_id);
    page.set("Resources", resources);
    Ok(doc.add_object(page))
}

fn pdf_escape_text(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '(' => "\\(".into(),
            ')' => "\\)".into(),
            '\\' => "\\\\".into(),
            '\r' | '\n' | '\t' => ' '.into(),
            c if c.is_ascii() && !c.is_control() => c.to_string(),
            _ => '?'.to_string(),
        })
        .collect::<String>()
        .chars()
        .take(110)
        .collect()
}

fn add_jpeg_page(
    doc: &mut Document,
    pages_id: ObjectId,
    jpeg: &[u8],
    index: usize,
) -> Result<ObjectId, AppError> {
    let (w, h) = jpeg_size(jpeg).unwrap_or((595, 842));
    let mut img_dict = Dictionary::new();
    img_dict.set("Type", "XObject");
    img_dict.set("Subtype", "Image");
    img_dict.set("Width", w as i64);
    img_dict.set("Height", h as i64);
    img_dict.set("ColorSpace", "DeviceRGB");
    img_dict.set("BitsPerComponent", 8);
    img_dict.set("Filter", "DCTDecode");
    img_dict.set("Length", jpeg.len() as i64);
    let image_id = doc.add_object(Object::Stream(Stream::new(img_dict, jpeg.to_vec())));

    let xname = format!("Im{index}");
    let page_w = 595.0_f32;
    let page_h = if w > 0 {
        page_w * (h as f32) / (w as f32)
    } else {
        842.0
    };
    let content = format!("q\n{page_w:.2} 0 0 {page_h:.2} 0 0 cm\n/{xname} Do\nQ\n");
    let content_id = doc.add_object(Object::Stream(Stream::new(
        Dictionary::new(),
        content.into_bytes(),
    )));

    let mut xobject = Dictionary::new();
    xobject.set(xname.as_str(), image_id);
    let mut resources = Dictionary::new();
    resources.set("XObject", xobject);

    let mut page = Dictionary::new();
    page.set("Type", "Page");
    page.set("Parent", pages_id);
    page.set(
        "MediaBox",
        vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Real(page_w),
            Object::Real(page_h.min(2000.0)),
        ],
    );
    page.set("Contents", content_id);
    page.set("Resources", resources);
    Ok(doc.add_object(page))
}

fn add_png_page(
    doc: &mut Document,
    pages_id: ObjectId,
    png: &[u8],
    index: usize,
) -> Result<ObjectId, AppError> {
    let img = image::load_from_memory(png).map_err(|e| AppError::Image(e.to_string()))?;
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut jpeg_buf = std::io::Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buf, 90);
    use image::ImageEncoder;
    encoder
        .write_image(
            rgb.as_raw(),
            w,
            h,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| AppError::Image(e.to_string()))?;
    add_jpeg_page(doc, pages_id, &jpeg_buf.into_inner(), 1000 + index)
}

fn jpeg_size(data: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2usize;
    while i + 9 < data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = data[i + 1];
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if marker >= 0xC0 && marker <= 0xC3 {
            let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
            let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
            if w > 0 && h > 0 {
                return Some((w, h));
            }
        }
        if i + 3 >= data.len() {
            break;
        }
        let seglen = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        i += 2 + seglen;
    }
    None
}

fn carve_jpegs(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 3 < data.len() {
        if data[i] == 0xFF && data[i + 1] == 0xD8 && data[i + 2] == 0xFF {
            // Find EOI
            let mut j = i + 3;
            while j + 1 < data.len() {
                if data[j] == 0xFF && data[j + 1] == 0xD9 {
                    let end = j + 2;
                    let slice = &data[i..end];
                    // Skip tiny / false positives
                    if slice.len() > 2000 {
                        if jpeg_size(slice).is_some() {
                            out.push(slice.to_vec());
                        }
                    }
                    i = end;
                    break;
                }
                j += 1;
            }
            if j + 1 >= data.len() {
                break;
            }
        } else {
            i += 1;
        }
        if out.len() >= 80 {
            break;
        }
    }
    out
}

fn carve_pngs(data: &[u8]) -> Vec<Vec<u8>> {
    let sig = b"\x89PNG\r\n\x1a\n";
    let iend = b"IEND";
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 < data.len() {
        if data[i..].starts_with(sig) {
            // Find IEND chunk
            let mut j = i + 8;
            let mut found = false;
            while j + 12 < data.len() {
                if &data[j + 4..j + 8] == iend {
                    // chunk: length(4) + type(4) + data(0) + crc(4)
                    let end = j + 12;
                    if end <= data.len() && end - i > 100 {
                        out.push(data[i..end].to_vec());
                    }
                    i = end;
                    found = true;
                    break;
                }
                j += 1;
            }
            if !found {
                break;
            }
        } else {
            i += 1;
        }
        if out.len() >= 40 {
            break;
        }
    }
    out
}

fn extract_readable_strings(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    let push_cur = |cur: &mut Vec<u8>, out: &mut Vec<String>| {
        if cur.len() >= 6 {
            if let Ok(s) = std::str::from_utf8(cur) {
                let t = s.trim();
                if is_useful_text(t) {
                    out.push(t.to_string());
                }
            }
        }
        cur.clear();
    };

    for &b in data {
        if (0x20..=0x7E).contains(&b) {
            cur.push(b);
            if cur.len() > 200 {
                push_cur(&mut cur, &mut out);
            }
        } else {
            push_cur(&mut cur, &mut out);
        }
        if out.len() >= 300 {
            break;
        }
    }
    push_cur(&mut cur, &mut out);

    // Also PDF literal strings (...) 
    let mut i = 0usize;
    while i < data.len() && out.len() < 400 {
        if data[i] == b'(' {
            if let Some(s) = take_literal_string(data, i) {
                let inner = &s[1..s.len().saturating_sub(1)];
                if let Ok(txt) = std::str::from_utf8(inner) {
                    let t = txt.trim();
                    if is_useful_text(t) {
                        out.push(t.to_string());
                    }
                }
                i += s.len();
                continue;
            }
        }
        i += 1;
    }

    out.sort();
    out.dedup();
    out
}

fn is_useful_text(t: &str) -> bool {
    if t.len() < 6 || t.len() > 200 {
        return false;
    }
    // Skip PDF operators / boilerplate
    let lower = t.to_ascii_lowercase();
    for bad in [
        "endobj", "endstream", "startxref", "trailer", "flatedecode", "device", "filter",
        "length", "subtype", "type /", "obj", "xref", "mediabox", "resources", "contents",
        "helvetica", "basefont", "encoding",
    ] {
        if lower.contains(bad) {
            return false;
        }
    }
    let alpha = t.chars().filter(|c| c.is_ascii_alphabetic()).count();
    alpha >= 4
}

fn try_lopdf_cleanup(bytes: &[u8], pwd: &str, output_path: &Path) -> Result<u32, AppError> {
    let mut doc = Document::load_mem(bytes).map_err(|e| AppError::Pdf(e.to_string()))?;
    if doc.is_encrypted() {
        if pwd.is_empty() {
            return Err(AppError::InvalidInput(
                "PDF cifrado: indica la contraseña para reparar".into(),
            ));
        }
        #[allow(deprecated)]
        {
            doc.decrypt(pwd)
                .map_err(|e| AppError::InvalidInput(format!("No se pudo descifrar: {e}")))?;
        }
    }
    if doc.get_pages().is_empty() {
        return Err(AppError::Pdf(
            "El PDF se abrió pero no tiene páginas legibles".into(),
        ));
    }
    cleanup_and_save(&mut doc, output_path)?;
    let pages = doc.get_pages().len() as u32;
    if pages == 0 {
        let _ = std::fs::remove_file(output_path);
        return Err(AppError::Pdf(
            "La limpieza dejó el PDF sin páginas".into(),
        ));
    }
    Ok(pages)
}

fn cleanup_and_save(doc: &mut Document, output_path: &Path) -> Result<(), AppError> {
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
            }
        }
    }

    let pages_before = doc.get_pages().len();
    let reachable = collect_reachable(doc);
    if !reachable.is_empty() {
        let orphans: Vec<ObjectId> = doc
            .objects
            .keys()
            .copied()
            .filter(|id| !reachable.contains(id))
            .collect();
        // Never strip orphans if that would destroy the page tree.
        let mut trial_removed = 0usize;
        for id in &orphans {
            if doc.get_pages().values().any(|p| p == id) {
                trial_removed += 1;
            }
        }
        if trial_removed == 0 {
            for id in orphans {
                doc.objects.remove(&id);
            }
        }
    }

    let pages = doc.get_pages();
    if pages.is_empty() && pages_before > 0 {
        return Err(AppError::Pdf(
            "La reparación eliminó todas las páginas".into(),
        ));
    }
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

    doc.trailer.remove(b"Encrypt");
    doc.renumber_objects();
    doc.compress();
    doc.save(output_path)?;
    Ok(())
}

fn repair_with_libqpdf(bytes: &[u8], output: &Path, password: &str) -> Result<u32, AppError> {
    // qpdf enables recovery by default on read; rewriting heals xref/trailer.
    let pdf = if password.is_empty() {
        qpdf::QPdf::read_from_memory(bytes)
    } else {
        qpdf::QPdf::read_from_memory_encrypted(bytes, password)
    }
    .map_err(|e| AppError::Pdf(format!("qpdf no pudo leer: {e}")))?;

    let pages = pdf
        .get_num_pages()
        .map_err(|e| AppError::Pdf(format!("qpdf páginas: {e}")))?;
    if pages == 0 {
        return Err(AppError::Pdf("qpdf: PDF sin páginas".into()));
    }

    let mut writer = pdf.writer();
    writer.preserve_encryption(false);
    writer.object_stream_mode(qpdf::ObjectStreamMode::Disable);
    writer
        .write(output)
        .map_err(|e| AppError::Pdf(format!("qpdf no pudo escribir: {e}")))?;
    Ok(pages)
}

fn repair_with_pdfium_bytes(bytes: &[u8], output: &Path, password: &str) -> Result<u32, AppError> {
    let pdfium = create_pdfium()?;
    let bindings = pdfium.bindings();

    let mut handle = bindings.FPDF_LoadMemDocument(
        bytes,
        if password.is_empty() {
            None
        } else {
            Some(password)
        },
    );
    // Some broken encrypted files open with an empty password string.
    if handle.is_null() && password.is_empty() {
        handle = bindings.FPDF_LoadMemDocument(bytes, Some(""));
    }
    if handle.is_null() {
        let err = bindings.FPDF_GetLastError();
        if err == 4 {
            return Err(AppError::InvalidInput("Contraseña incorrecta".into()));
        }
        return Err(AppError::Pdf(format!(
            "PDFium no pudo abrir el PDF (código {err})"
        )));
    }

    let page_count = bindings.FPDF_GetPageCount(handle).max(0) as u32;
    let mut file = File::create(output).map_err(AppError::Io)?;

    #[repr(C)]
    struct FileWriteExt {
        version: c_int,
        write_block: Option<
            unsafe extern "C" fn(
                this: *mut FileWriteExt,
                data: *const c_void,
                size: c_ulong,
            ) -> c_int,
        >,
        file: *mut File,
    }

    unsafe extern "C" fn write_block(
        this: *mut FileWriteExt,
        data: *const c_void,
        size: c_ulong,
    ) -> c_int {
        let file = unsafe { &mut *(*this).file };
        let slice = unsafe { std::slice::from_raw_parts(data as *const u8, size as usize) };
        if file.write_all(slice).is_ok() {
            1
        } else {
            0
        }
    }

    let mut writer = FileWriteExt {
        version: 1,
        write_block: Some(write_block),
        file: &mut file as *mut File,
    };

    let flags = if password.is_empty() {
        FPDF_NO_INCREMENTAL
    } else {
        FPDF_REMOVE_SECURITY
    };

    let ok = bindings.FPDF_SaveAsCopy(
        handle,
        &mut writer as *mut FileWriteExt as *mut FPDF_FILEWRITE,
        flags,
    );
    bindings.FPDF_CloseDocument(handle);
    file.flush().map_err(AppError::Io)?;
    drop(file);

    if !bindings.is_true(ok) {
        let _ = std::fs::remove_file(output);
        return Err(AppError::Pdf("PDFium no pudo reescribir el PDF".into()));
    }
    if page_count == 0 {
        let _ = std::fs::remove_file(output);
        return Err(AppError::Pdf("PDF sin páginas tras reescritura".into()));
    }
    Ok(page_count)
}

fn verify_page_count(path: &Path) -> Result<u32, AppError> {
    if let Ok(doc) = Document::load(path) {
        return Ok(doc.get_pages().len() as u32);
    }
    let bytes = std::fs::read(path).map_err(AppError::Io)?;
    count_pages_pdfium(&bytes, None)
        .or_else(|| count_pages_pdfium(&bytes, Some("")))
        .ok_or_else(|| AppError::Pdf("No se pudo verificar el PDF reparado".into()))
}

fn count_pages_pdfium(bytes: &[u8], password: Option<&str>) -> Option<u32> {
    let pdfium = create_pdfium().ok()?;
    let bindings = pdfium.bindings();
    let handle = bindings.FPDF_LoadMemDocument(bytes, password);
    if handle.is_null() {
        return None;
    }
    let n = bindings.FPDF_GetPageCount(handle).max(0) as u32;
    bindings.FPDF_CloseDocument(handle);
    Some(n)
}

// ---------------------------------------------------------------------------
// Byte-level salvage
// ---------------------------------------------------------------------------

/// Strip junk before `%PDF-`, keep through last `%%EOF` (or append one).
fn normalize_pdf_bytes(raw: &[u8]) -> Vec<u8> {
    let start = raw
        .windows(5)
        .position(|w| w == b"%PDF-")
        .unwrap_or(0);
    let mut bytes = raw[start..].to_vec();
    if let Some(eof) = bytes.windows(5).rposition(|w| w == b"%%EOF") {
        let end = (eof + 5).min(bytes.len());
        // Keep a little trailing whitespace if present, else cut hard.
        let mut cut = end;
        while cut < bytes.len() && matches!(bytes[cut], b'\r' | b'\n' | b' ' | b'\t') {
            cut += 1;
        }
        bytes.truncate(cut);
    }
    if !bytes.windows(5).any(|w| w == b"%%EOF") {
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        bytes.extend_from_slice(b"%%EOF\n");
    }
    bytes
}

#[derive(Clone, Debug)]
struct ObjHit {
    id: u32,
    gen: u16,
    start: usize,
    end: usize,
}

fn scan_objects(data: &[u8]) -> Vec<ObjHit> {
    let mut hits: Vec<ObjHit> = Vec::new();
    let mut i = 0usize;
    while i < data.len() {
        let prev = if i == 0 { None } else { Some(data[i - 1]) };
        if !is_boundary(prev) {
            i += 1;
            continue;
        }
        let Some((id, gen, obj_kw_end)) = match_obj_header(data, i) else {
            i += 1;
            continue;
        };
        let Some(end) = find_object_end(data, obj_kw_end) else {
            i += 1;
            continue;
        };
        // Prefer later definitions of the same id (incremental updates).
        if let Some(existing) = hits.iter_mut().find(|h| h.id == id) {
            *existing = ObjHit {
                id,
                gen,
                start: i,
                end,
            };
        } else {
            hits.push(ObjHit {
                id,
                gen,
                start: i,
                end,
            });
        }
        i = end;
    }
    hits.sort_by_key(|h| h.id);
    hits
}

fn is_boundary(prev: Option<u8>) -> bool {
    match prev {
        None => true,
        Some(b) => matches!(b, b'\n' | b'\r' | b'\t' | b' ' | b'\0' | b'>'),
    }
}

fn match_obj_header(data: &[u8], i: usize) -> Option<(u32, u16, usize)> {
    let mut j = i;
    if j >= data.len() || !data[j].is_ascii_digit() {
        return None;
    }
    let id_start = j;
    while j < data.len() && data[j].is_ascii_digit() {
        j += 1;
    }
    if j == id_start || j >= data.len() || data[j] != b' ' {
        return None;
    }
    let id: u32 = std::str::from_utf8(&data[id_start..j]).ok()?.parse().ok()?;
    j += 1;
    let gen_start = j;
    if j >= data.len() || !data[j].is_ascii_digit() {
        return None;
    }
    while j < data.len() && data[j].is_ascii_digit() {
        j += 1;
    }
    if j == gen_start {
        return None;
    }
    let gen: u16 = std::str::from_utf8(&data[gen_start..j]).ok()?.parse().ok()?;
    while j < data.len() && matches!(data[j], b' ' | b'\t') {
        j += 1;
    }
    if j + 3 > data.len() || &data[j..j + 3] != b"obj" {
        return None;
    }
    j += 3;
    if j < data.len() && data[j].is_ascii_alphanumeric() {
        return None;
    }
    if id == 0 {
        return None;
    }
    Some((id, gen, j))
}

fn find_object_end(data: &[u8], after_obj: usize) -> Option<usize> {
    let mut i = after_obj;
    while i < data.len() && matches!(data[i], b' ' | b'\t' | b'\r' | b'\n') {
        i += 1;
    }
    // Heuristic: if we see a dict then `stream`, honour /Length when present.
    if let Some(stream_at) = find_stream_keyword(data, i) {
        let dict_bytes = &data[i..stream_at];
        let length = parse_length_int(dict_bytes);
        let mut p = stream_at + 6; // after "stream"
        if p < data.len() && data[p] == b'\r' {
            p += 1;
        }
        if p < data.len() && data[p] == b'\n' {
            p += 1;
        }
        if let Some(len) = length {
            p = (p + len).min(data.len());
        }
        if let Some(endstream) = find_keyword(data, p, b"endstream") {
            p = endstream + b"endstream".len();
            if let Some(endobj) = find_keyword(data, p, b"endobj") {
                return Some(endobj + b"endobj".len());
            }
        }
    }
    let endobj = find_keyword(data, after_obj, b"endobj")?;
    Some(endobj + b"endobj".len())
}

fn find_stream_keyword(data: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 6 <= data.len() {
        if &data[i..i + 6] == b"stream"
            && (i == 0 || is_boundary(Some(data[i - 1])))
            && (i + 6 >= data.len() || matches!(data[i + 6], b'\r' | b'\n' | b' ' | b'\t'))
        {
            // Must appear after a dict close roughly nearby
            if data[from..i].windows(2).any(|w| w == b">>") {
                return Some(i);
            }
        }
        i += 1;
        // Don't scan forever inside huge objects without structure
        if i - from > 2_000_000 {
            break;
        }
    }
    None
}

fn find_keyword(data: &[u8], from: usize, kw: &[u8]) -> Option<usize> {
    let mut i = from;
    while i + kw.len() <= data.len() {
        if &data[i..i + kw.len()] == kw
            && (i == 0 || is_boundary(Some(data[i - 1])))
            && (i + kw.len() >= data.len()
                || !data[i + kw.len()].is_ascii_alphanumeric())
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn parse_length_int(dict_bytes: &[u8]) -> Option<usize> {
    let key = b"/Length";
    let pos = dict_bytes.windows(key.len()).position(|w| w == key)?;
    let mut i = pos + key.len();
    while i < dict_bytes.len() && matches!(dict_bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
        i += 1;
    }
    let start = i;
    while i < dict_bytes.len() && dict_bytes[i].is_ascii_digit() {
        i += 1;
    }
    if start == i {
        return None;
    }
    // Reject indirect refs: "12 0 R"
    let mut j = i;
    while j < dict_bytes.len() && matches!(dict_bytes[j], b' ' | b'\t') {
        j += 1;
    }
    if j < dict_bytes.len() && dict_bytes[j].is_ascii_digit() {
        return None;
    }
    std::str::from_utf8(&dict_bytes[start..i])
        .ok()?
        .parse()
        .ok()
}

fn body_has_type(body: &[u8], type_name: &[u8]) -> bool {
    // Match `/Type /Name` or `/Type/Name`
    let mut i = 0;
    while i + 5 < body.len() {
        if body[i] == b'/' && body.get(i..i + 5) == Some(b"/Type") {
            let mut j = i + 5;
            while j < body.len() && matches!(body[j], b' ' | b'\t' | b'\r' | b'\n') {
                j += 1;
            }
            if j < body.len() && body[j] == b'/' {
                j += 1;
                if body.get(j..j + type_name.len()) == Some(type_name) {
                    let after = j + type_name.len();
                    if after >= body.len() || !body[after].is_ascii_alphanumeric() {
                        return true;
                    }
                }
            }
        }
        i += 1;
    }
    false
}

fn body_has_key(body: &[u8], key: &[u8]) -> bool {
    body.windows(key.len()).any(|w| w == key)
}

fn looks_like_page(body: &[u8]) -> bool {
    if body_has_type(body, b"Pages") {
        return false;
    }
    if body_has_type(body, b"Page") {
        return true;
    }
    // Heuristic for pages missing /Type (common in broken exports).
    let has_mb = body_has_key(body, b"/MediaBox") || body_has_key(body, b"/CropBox");
    let has_content = body_has_key(body, b"/Contents") || body_has_key(body, b"/Resources");
    has_mb && has_content && !body_has_type(body, b"Catalog") && !body_has_type(body, b"XObject")
}

#[derive(Clone, Debug)]
struct ExtractedObj {
    id: u32,
    gen: u16,
    body: Vec<u8>,
}

fn extract_objects_expanded(data: &[u8]) -> Vec<ExtractedObj> {
    let hits = scan_objects(data);
    let mut map: std::collections::BTreeMap<u32, ExtractedObj> = std::collections::BTreeMap::new();

    for h in &hits {
        let body = &data[h.start..h.end];
        if body_has_type(body, b"XRef") {
            continue;
        }
        // Expand object streams into classical objects.
        if is_objstm(body) {
            for nested in expand_objstm(body) {
                map.insert(
                    nested.id,
                    nested,
                );
            }
            continue;
        }
        // Also try expanding any stream that has /N + /First (ObjStm without readable /Type).
        if body_has_key(body, b"/First") && body_has_key(body, b"/N") && body_has_key(body, b"stream")
        {
            let nested = expand_objstm(body);
            if !nested.is_empty() {
                for n in nested {
                    map.insert(n.id, n);
                }
                continue;
            }
        }
        map.insert(
            h.id,
            ExtractedObj {
                id: h.id,
                gen: h.gen,
                body: {
                    let mut b = body.to_vec();
                    if !b.ends_with(b"\n") {
                        b.push(b'\n');
                    }
                    b
                },
            },
        );
    }

    // Second pass: decompress remaining Flate streams and hunt for buried page dicts
    // that weren't in a proper ObjStm (rare, but helps with odd exporters).
    let existing_ids: Vec<u32> = map.keys().copied().collect();
    let mut extras = Vec::new();
    for id in existing_ids {
        let Some(obj) = map.get(&id) else { continue };
        if !body_has_key(&obj.body, b"stream") {
            continue;
        }
        if is_objstm(&obj.body) {
            continue;
        }
        if let Some(payload) = decompress_stream_body(&obj.body) {
            if payload.windows(5).any(|w| w == b"/Type") && looks_like_page(&payload) {
                // Unlikely as a whole stream; skip
            }
            // If decompressed payload looks like an ObjStm index, try expand
            if let Some(first) = parse_dict_int(dict_portion(&obj.body).unwrap_or(&obj.body), b"/First")
            {
                if first > 0 && first < payload.len() {
                    for n in expand_objstm_payload(
                        &obj.body,
                        &payload,
                        first,
                        parse_dict_int(dict_portion(&obj.body).unwrap_or(&obj.body), b"/N")
                            .unwrap_or(0),
                    ) {
                        if !map.contains_key(&n.id) {
                            extras.push(n);
                        }
                    }
                }
            }
        }
    }
    for n in extras {
        map.entry(n.id).or_insert(n);
    }

    map.into_values().collect()
}

fn is_objstm(body: &[u8]) -> bool {
    body_has_type(body, b"ObjStm")
}

fn dict_portion(body: &[u8]) -> Option<&[u8]> {
    let stream_at = find_stream_keyword(body, 0)?;
    Some(&body[..stream_at])
}

fn parse_dict_int(dict: &[u8], key: &[u8]) -> Option<usize> {
    let pos = dict.windows(key.len()).position(|w| w == key)?;
    let mut i = pos + key.len();
    while i < dict.len() && matches!(dict[i], b' ' | b'\t' | b'\r' | b'\n') {
        i += 1;
    }
    let start = i;
    while i < dict.len() && dict[i].is_ascii_digit() {
        i += 1;
    }
    if start == i {
        return None;
    }
    std::str::from_utf8(&dict[start..i]).ok()?.parse().ok()
}

fn decompress_stream_body(obj_body: &[u8]) -> Option<Vec<u8>> {
    let dict = dict_portion(obj_body)?;
    let stream_at = find_stream_keyword(obj_body, 0)?;
    let mut p = stream_at + 6;
    if p < obj_body.len() && obj_body[p] == b'\r' {
        p += 1;
    }
    if p < obj_body.len() && obj_body[p] == b'\n' {
        p += 1;
    }
    let length = parse_length_int(dict);
    let end = if let Some(len) = length {
        (p + len).min(obj_body.len())
    } else {
        find_keyword(obj_body, p, b"endstream").unwrap_or(obj_body.len())
    };
    let raw = &obj_body[p..end];
    decompress_pdf_stream(dict, raw)
}

fn decompress_pdf_stream(dict: &[u8], raw: &[u8]) -> Option<Vec<u8>> {
    let has_flate = body_has_key(dict, b"/FlateDecode") || body_has_key(dict, b"/Fl");
    if !has_flate {
        return Some(raw.to_vec());
    }
    // Zlib wrapper
    {
        use std::io::Read;
        let mut dec = flate2::read::ZlibDecoder::new(raw);
        let mut out = Vec::new();
        if dec.read_to_end(&mut out).is_ok() && !out.is_empty() {
            return Some(out);
        }
    }
    // Raw deflate
    {
        use std::io::Read;
        let mut dec = flate2::read::DeflateDecoder::new(raw);
        let mut out = Vec::new();
        if dec.read_to_end(&mut out).is_ok() && !out.is_empty() {
            return Some(out);
        }
    }
    None
}

fn expand_objstm(obj_body: &[u8]) -> Vec<ExtractedObj> {
    let Some(dict) = dict_portion(obj_body) else {
        return Vec::new();
    };
    let Some(payload) = decompress_stream_body(obj_body) else {
        return Vec::new();
    };
    let first = parse_dict_int(dict, b"/First").unwrap_or(0);
    let n = parse_dict_int(dict, b"/N").unwrap_or(0);
    expand_objstm_payload(obj_body, &payload, first, n)
}

fn expand_objstm_payload(
    _obj_body: &[u8],
    payload: &[u8],
    first: usize,
    n: usize,
) -> Vec<ExtractedObj> {
    if first == 0 || first >= payload.len() || n == 0 {
        // Try to infer First: find first `<<` after number pairs
        return expand_objstm_infer(payload);
    }
    let header = std::str::from_utf8(&payload[..first]).unwrap_or("");
    let nums: Vec<u32> = header
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    let mut out = Vec::new();
    let pairs = nums.len() / 2;
    for i in 0..pairs.min(n) {
        let id = nums[i * 2];
        let offset = nums[i * 2 + 1] as usize;
        let abs = first + offset;
        if abs >= payload.len() || id == 0 {
            continue;
        }
        let Some(obj_bytes) = take_direct_object(payload, abs) else {
            continue;
        };
        let mut body = format!("{id} 0 obj\n").into_bytes();
        body.extend_from_slice(obj_bytes);
        if !body.ends_with(b"\n") {
            body.push(b'\n');
        }
        body.extend_from_slice(b"endobj\n");
        out.push(ExtractedObj { id, gen: 0, body });
    }
    if out.is_empty() {
        return expand_objstm_infer(payload);
    }
    out
}

fn expand_objstm_infer(payload: &[u8]) -> Vec<ExtractedObj> {
    // Fallback: scan decompressed payload for `/Type /Page` dicts and invent ids.
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut synth_id = 100_000u32;
    while i + 8 < payload.len() {
        if payload[i] == b'<' && payload.get(i + 1) == Some(&b'<') {
            if let Some(dict) = take_dict(payload, i) {
                if looks_like_page(dict) || body_has_type(dict, b"Catalog") || body_has_type(dict, b"Pages")
                {
                    let id = synth_id;
                    synth_id += 1;
                    let mut body = format!("{id} 0 obj\n").into_bytes();
                    body.extend_from_slice(dict);
                    body.extend_from_slice(b"\nendobj\n");
                    out.push(ExtractedObj { id, gen: 0, body });
                }
                i += dict.len().max(1);
                continue;
            }
        }
        i += 1;
    }
    out
}

fn take_direct_object(data: &[u8], start: usize) -> Option<&[u8]> {
    let mut i = start;
    while i < data.len() && matches!(data[i], b' ' | b'\t' | b'\r' | b'\n') {
        i += 1;
    }
    if i >= data.len() {
        return None;
    }
    let begin = i;
    match data[i] {
        b'<' if data.get(i + 1) == Some(&b'<') => take_dict(data, i),
        b'[' => take_array(data, i),
        b'(' => take_literal_string(data, i),
        b'<' => take_hex_string(data, i),
        b'/' => {
            i += 1;
            while i < data.len() && !matches!(data[i], b' ' | b'\t' | b'\r' | b'\n' | b'/' | b'[' | b']' | b'<' | b'>' | b'(' | b')') {
                i += 1;
            }
            Some(&data[begin..i])
        }
        b't' if data[i..].starts_with(b"true") => Some(&data[begin..begin + 4]),
        b'f' if data[i..].starts_with(b"false") => Some(&data[begin..begin + 5]),
        b'n' if data[i..].starts_with(b"null") => Some(&data[begin..begin + 4]),
        b'+' | b'-' | b'.' | b'0'..=b'9' => {
            // number or "n n R"
            while i < data.len()
                && (data[i].is_ascii_digit()
                    || matches!(data[i], b'+' | b'-' | b'.' | b' ' | b'\t'))
            {
                i += 1;
            }
            if data.get(i) == Some(&b'R') {
                i += 1;
            }
            Some(&data[begin..i])
        }
        _ => None,
    }
}

fn take_dict(data: &[u8], start: usize) -> Option<&[u8]> {
    if data.get(start..start + 2) != Some(b"<<") {
        return None;
    }
    let mut i = start + 2;
    let mut depth = 1i32;
    while i < data.len() {
        match data[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'(' => {
                let s = take_literal_string(data, i)?;
                i += s.len();
                continue;
            }
            b'<' if data.get(i + 1) == Some(&b'<') => {
                depth += 1;
                i += 2;
                continue;
            }
            b'>' if data.get(i + 1) == Some(&b'>') => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return Some(&data[start..i]);
                }
                continue;
            }
            b'<' => {
                if let Some(s) = take_hex_string(data, i) {
                    i += s.len();
                    continue;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

fn take_array(data: &[u8], start: usize) -> Option<&[u8]> {
    if data.get(start) != Some(&b'[') {
        return None;
    }
    let mut i = start + 1;
    let mut depth = 1i32;
    while i < data.len() {
        match data[i] {
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return Some(&data[start..i]);
                }
            }
            b'(' => {
                let s = take_literal_string(data, i)?;
                i += s.len();
            }
            b'<' if data.get(i + 1) == Some(&b'<') => {
                let s = take_dict(data, i)?;
                i += s.len();
            }
            b'<' => {
                if let Some(s) = take_hex_string(data, i) {
                    i += s.len();
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    None
}

fn take_literal_string(data: &[u8], start: usize) -> Option<&[u8]> {
    if data.get(start) != Some(&b'(') {
        return None;
    }
    let mut i = start + 1;
    let mut depth = 1i32;
    while i < data.len() {
        match data[i] {
            b'\\' => i += 2,
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return Some(&data[start..i]);
                }
            }
            _ => i += 1,
        }
    }
    None
}

fn take_hex_string(data: &[u8], start: usize) -> Option<&[u8]> {
    if data.get(start) != Some(&b'<') || data.get(start + 1) == Some(&b'<') {
        return None;
    }
    let mut i = start + 1;
    while i < data.len() {
        if data[i] == b'>' {
            return Some(&data[start..i + 1]);
        }
        i += 1;
    }
    None
}

fn find_catalog_in(objs: &[ExtractedObj]) -> Option<u32> {
    objs.iter().find_map(|o| {
        if body_has_type(&o.body, b"Catalog") {
            Some(o.id)
        } else {
            None
        }
    })
}

fn find_pages_root_in(objs: &[ExtractedObj]) -> Option<u32> {
    objs.iter()
        .find(|o| body_has_type(&o.body, b"Pages") && body_has_key(&o.body, b"/Kids"))
        .map(|o| o.id)
        .or_else(|| {
            objs.iter()
                .find(|o| body_has_type(&o.body, b"Pages"))
                .map(|o| o.id)
        })
}

fn find_page_ids_in(objs: &[ExtractedObj]) -> Vec<u32> {
    objs.iter()
        .filter(|o| looks_like_page(&o.body))
        .map(|o| o.id)
        .collect()
}

/// Expand ObjStm and rewrite as a classical PDF with a fresh xref.
fn classicalize_pdf(data: &[u8]) -> Result<Vec<u8>, AppError> {
    let mut objs = extract_objects_expanded(data);
    if objs.is_empty() {
        return Err(AppError::Pdf("Escaneo: 0 objetos (ni en ObjStm)".into()));
    }

    let mut catalog = find_catalog_in(&objs);
    let mut pages_id = find_pages_root_in(&objs);
    let page_ids = find_page_ids_in(&objs);

    let mut next_id = objs.iter().map(|o| o.id).max().unwrap_or(1) + 1;

    if pages_id.is_none() {
        if page_ids.is_empty() {
            return Err(AppError::Pdf(
                "No hay páginas (/Type /Page) recuperables — ¿PDF cifrado o truncado?".into(),
            ));
        }
        let new_pages = next_id;
        next_id += 1;
        let kids = page_ids
            .iter()
            .map(|id| format!("{id} 0 R"))
            .collect::<Vec<_>>()
            .join(" ");
        objs.push(ExtractedObj {
            id: new_pages,
            gen: 0,
            body: format!(
                "{new_pages} 0 obj\n<< /Type /Pages /Count {} /Kids [{kids}] >>\nendobj\n",
                page_ids.len()
            )
            .into_bytes(),
        });
        pages_id = Some(new_pages);
    }

    if catalog.is_none() {
        let pages = pages_id.unwrap();
        let new_catalog = next_id;
        objs.push(ExtractedObj {
            id: new_catalog,
            gen: 0,
            body: format!(
                "{new_catalog} 0 obj\n<< /Type /Catalog /Pages {pages} 0 R >>\nendobj\n"
            )
            .into_bytes(),
        });
        catalog = Some(new_catalog);
    }

    write_objects_pdf(&objs, catalog.unwrap())
}

fn write_objects_pdf(objs: &[ExtractedObj], catalog: u32) -> Result<Vec<u8>, AppError> {
    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    out.extend_from_slice(&[b'%', 0xE2, 0xE3, 0xCF, 0xD3, b'\n']);
    let mut offsets: Vec<(u32, u16, usize)> = Vec::new();
    // Stable order by id; last write wins for duplicates (already deduped).
    let mut sorted = objs.to_vec();
    sorted.sort_by_key(|o| o.id);
    for o in &sorted {
        let off = out.len();
        // Ensure header matches id
        let body = ensure_obj_header(&o.body, o.id, o.gen);
        out.extend_from_slice(&body);
        if !body.ends_with(b"\n") {
            out.push(b'\n');
        }
        offsets.push((o.id, o.gen, off));
    }
    write_classical_xref(&mut out, &offsets, catalog);
    Ok(out)
}

fn ensure_obj_header(body: &[u8], id: u32, gen: u16) -> Vec<u8> {
    // If body already starts with "id gen obj", keep it.
    let prefix = format!("{id} {gen} obj");
    if body.starts_with(prefix.as_bytes())
        || body
            .windows(3)
            .position(|w| w == b"obj")
            .map(|p| p < 32)
            .unwrap_or(false)
    {
        return body.to_vec();
    }
    let mut v = format!("{id} {gen} obj\n").into_bytes();
    v.extend_from_slice(body);
    if !body.windows(6).any(|w| w == b"endobj") {
        v.extend_from_slice(b"\nendobj\n");
    }
    v
}

fn find_catalog_id(hits: &[ObjHit], data: &[u8]) -> Option<u32> {
    hits.iter().find_map(|h| {
        let body = &data[h.start..h.end];
        if body_has_type(body, b"Catalog") {
            Some(h.id)
        } else {
            None
        }
    })
}

fn find_pages_root_id(hits: &[ObjHit], data: &[u8]) -> Option<u32> {
    hits.iter()
        .find_map(|h| {
            let body = &data[h.start..h.end];
            if body_has_type(body, b"Pages") && body_has_key(body, b"/Kids") {
                Some(h.id)
            } else {
                None
            }
        })
        .or_else(|| {
            hits.iter().find_map(|h| {
                let body = &data[h.start..h.end];
                if body_has_type(body, b"Pages") {
                    Some(h.id)
                } else {
                    None
                }
            })
        })
}

fn find_page_ids(hits: &[ObjHit], data: &[u8]) -> Vec<u32> {
    let mut pages = Vec::new();
    for h in hits {
        let body = &data[h.start..h.end];
        if looks_like_page(body) {
            pages.push(h.id);
        }
    }
    pages
}

/// Append a classical xref built from object scan (parsers read the last startxref).
fn rebuild_xref_bytes(data: &[u8]) -> Result<Vec<u8>, AppError> {
    // Prefer full classicalization when we can expand streams.
    if let Ok(c) = classicalize_pdf(data) {
        return Ok(c);
    }

    let hits = scan_objects(data);
    if hits.is_empty() {
        return Err(AppError::Pdf("No se encontraron objetos PDF".into()));
    }

    let mut catalog = find_catalog_id(&hits, data);
    let mut extra_objects = Vec::<Vec<u8>>::new();
    let mut next_id = hits.iter().map(|h| h.id).max().unwrap_or(1) + 1;

    if catalog.is_none() {
        let pages_id = match find_pages_root_id(&hits, data) {
            Some(id) => Some(id),
            None => {
                let page_ids = find_page_ids(&hits, data);
                if page_ids.is_empty() {
                    None
                } else {
                    let new_pages = next_id;
                    next_id += 1;
                    let kids = page_ids
                        .iter()
                        .map(|id| format!("{id} 0 R"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    extra_objects.push(
                        format!(
                            "{new_pages} 0 obj\n<< /Type /Pages /Count {} /Kids [{kids}] >>\nendobj\n",
                            page_ids.len()
                        )
                        .into_bytes(),
                    );
                    Some(new_pages)
                }
            }
        };

        if let Some(pages_id) = pages_id {
            let new_catalog = next_id;
            extra_objects.push(
                format!(
                    "{new_catalog} 0 obj\n<< /Type /Catalog /Pages {pages_id} 0 R >>\nendobj\n"
                )
                .into_bytes(),
            );
            catalog = Some(new_catalog);
        }
    }

    let catalog = catalog.ok_or_else(|| {
        AppError::Pdf("No se encontró catálogo ni páginas recuperables".into())
    })?;

    let mut out = data.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }

    let mut offsets: Vec<(u32, u16, usize)> =
        hits.iter().map(|h| (h.id, h.gen, h.start)).collect();

    for extra in &extra_objects {
        let id = std::str::from_utf8(extra)
            .ok()
            .and_then(|s| s.split_whitespace().next()?.parse().ok())
            .unwrap_or(0);
        let off = out.len();
        out.extend_from_slice(extra);
        if id > 0 {
            offsets.push((id, 0, off));
        }
    }

    write_classical_xref(&mut out, &offsets, catalog);
    Ok(out)
}

fn write_classical_xref(out: &mut Vec<u8>, offsets: &[(u32, u16, usize)], catalog: u32) {
    let size = offsets.iter().map(|(id, _, _)| *id).max().unwrap_or(1) + 1;
    let xref_pos = out.len();
    out.extend_from_slice(b"xref\n");
    out.extend_from_slice(format!("0 {size}\n").as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    let mut map = std::collections::BTreeMap::new();
    for (id, gen, off) in offsets {
        map.insert(*id, (*gen, *off));
    }
    for id in 1..size {
        if let Some((gen, off)) = map.get(&id) {
            out.extend_from_slice(format!("{off:010} {gen:05} n \n").as_bytes());
        } else {
            out.extend_from_slice(b"0000000000 65535 f \n");
        }
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {size} /Root {catalog} 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n")
            .as_bytes(),
    );
}

/// Build a clean PDF copying only scanned objects (drops broken xref streams).
fn rebuild_fresh_pdf(data: &[u8]) -> Result<Vec<u8>, AppError> {
    classicalize_pdf(data)
}

// ---------------------------------------------------------------------------
// External CLIs
// ---------------------------------------------------------------------------

fn repair_cli_qpdf(input: &Path, output: &Path, password: &str) -> Result<u32, AppError> {
    let qpdf = find_tool("qpdf").ok_or_else(|| AppError::Pdf("qpdf no instalado".into()))?;
    let mut cmd = Command::new(&qpdf);
    if !password.is_empty() {
        cmd.arg(format!("--password={password}"));
        cmd.arg("--decrypt");
    }
    // qpdf always attempts recovery when writing.
    cmd.arg("--object-streams=disable");
    cmd.arg("--normalize-content=n");
    cmd.arg(input.as_os_str());
    cmd.arg(output.as_os_str());
    run_hidden(&mut cmd)?;
    verify_page_count(output)
}

fn repair_cli_mutool(input: &Path, output: &Path, _password: &str) -> Result<u32, AppError> {
    let mutool = find_tool("mutool").ok_or_else(|| AppError::Pdf("mutool no instalado".into()))?;
    let mut cmd = Command::new(&mutool);
    cmd.args(["clean", "-gggg", "-f"]);
    cmd.arg(input.as_os_str());
    cmd.arg(output.as_os_str());
    run_hidden(&mut cmd)?;
    verify_page_count(output)
}

fn repair_cli_ghostscript(input: &Path, output: &Path, _password: &str) -> Result<u32, AppError> {
    let gs = find_tool("gswin64c")
        .or_else(|| find_tool("gswin32c"))
        .or_else(|| find_tool("gs"))
        .ok_or_else(|| AppError::Pdf("Ghostscript no instalado".into()))?;
    let mut cmd = Command::new(&gs);
    cmd.args([
        "-dSAFER",
        "-dBATCH",
        "-dNOPAUSE",
        "-sDEVICE=pdfwrite",
        "-dPDFSETTINGS=/default",
    ]);
    cmd.arg(format!("-sOutputFile={}", output.display()));
    cmd.arg(input.as_os_str());
    run_hidden(&mut cmd)?;
    verify_page_count(output)
}

fn run_hidden(cmd: &mut Command) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .map_err(|e| AppError::Pdf(format!("No se pudo ejecutar: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err2 = String::from_utf8_lossy(&out.stdout);
        return Err(AppError::Pdf(format!(
            "falló: {} {}",
            err.trim(),
            err2.trim()
        )));
    }
    Ok(())
}

fn find_tool(cmd: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = Command::new("where")
            .arg(cmd)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if output.status.success() {
            let line = String::from_utf8_lossy(&output.stdout);
            let p = line.lines().next()?.trim();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
        if cmd == "qpdf" {
            for c in [
                r"C:\Program Files\qpdf\bin\qpdf.exe",
                r"C:\Program Files (x86)\qpdf\bin\qpdf.exe",
            ] {
                let p = PathBuf::from(c);
                if p.exists() {
                    return Some(p);
                }
            }
        }
        if cmd.starts_with("gs") {
            for c in [
                r"C:\Program Files\gs",
                r"C:\Program Files (x86)\gs",
            ] {
                if let Ok(entries) = std::fs::read_dir(c) {
                    for ent in entries.flatten() {
                        let bin = ent.path().join("bin").join(format!("{cmd}.exe"));
                        if bin.exists() {
                            return Some(bin);
                        }
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("which").arg(cmd).output().ok()?;
        if output.status.success() {
            let line = String::from_utf8_lossy(&output.stdout);
            let p = line.lines().next()?.trim();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    None
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
