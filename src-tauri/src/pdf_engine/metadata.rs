use crate::error::{AppError, OpResult};
use crate::pdf_engine::{ensure_parent_dir, ensure_pdf_path};
use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfMetadata {
    pub title: String,
    pub author: String,
    pub subject: String,
    pub keywords: String,
    pub creator: String,
    pub producer: String,
    pub creation_date: String,
    pub mod_date: String,
    pub page_count: u32,
}

/// Read the document Info dictionary (best-effort string fields).
pub fn get_pdf_metadata(path: String) -> Result<PdfMetadata, AppError> {
    let input = ensure_pdf_path(&path)?;
    let doc = Document::load(&input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "PDF cifrado: desbloquéalo antes de editar metadatos".into(),
        ));
    }
    Ok(metadata_from_doc(&doc))
}

/// Write Info dictionary fields. Empty strings clear the key.
pub fn set_pdf_metadata(path: String, output: String, meta: PdfMetadata) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    let mut doc = Document::load(&input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "PDF cifrado: desbloquéalo antes de editar metadatos".into(),
        ));
    }

    let page_count = doc.get_pages().len() as u32;
    let info_id = ensure_info_dict(&mut doc)?;
    let Object::Dictionary(dict) = doc
        .objects
        .get_mut(&info_id)
        .ok_or_else(|| AppError::Pdf("Info dictionary missing".into()))?
    else {
        return Err(AppError::Pdf("Info is not a dictionary".into()));
    };

    set_info_string(dict, "Title", &meta.title);
    set_info_string(dict, "Author", &meta.author);
    set_info_string(dict, "Subject", &meta.subject);
    set_info_string(dict, "Keywords", &meta.keywords);
    set_info_string(dict, "Creator", &meta.creator);
    set_info_string(dict, "Producer", &meta.producer);
    set_info_string(dict, "CreationDate", &meta.creation_date);
    set_info_string(dict, "ModDate", &meta.mod_date);

    doc.save(output_path)?;

    Ok(OpResult::new(
        vec![output],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn metadata_from_doc(doc: &Document) -> PdfMetadata {
    let page_count = doc.get_pages().len() as u32;
    let mut meta = PdfMetadata {
        page_count,
        ..Default::default()
    };

    let Ok(id) = doc.trailer.get(b"Info").and_then(|o| o.as_reference()) else {
        return meta;
    };
    let Ok(Object::Dictionary(dict)) = doc.get_object(id) else {
        return meta;
    };

    meta.title = get_info_string(dict, "Title");
    meta.author = get_info_string(dict, "Author");
    meta.subject = get_info_string(dict, "Subject");
    meta.keywords = get_info_string(dict, "Keywords");
    meta.creator = get_info_string(dict, "Creator");
    meta.producer = get_info_string(dict, "Producer");
    meta.creation_date = get_info_string(dict, "CreationDate");
    meta.mod_date = get_info_string(dict, "ModDate");
    meta
}

fn ensure_info_dict(doc: &mut Document) -> Result<ObjectId, AppError> {
    if let Ok(id) = doc.trailer.get(b"Info").and_then(|o| o.as_reference()) {
        if matches!(doc.objects.get(&id), Some(Object::Dictionary(_))) {
            return Ok(id);
        }
    }
    let id = doc.add_object(Object::Dictionary(Dictionary::new()));
    doc.trailer.set("Info", Object::Reference(id));
    Ok(id)
}

fn get_info_string(dict: &Dictionary, key: &str) -> String {
    match dict.get(key.as_bytes()) {
        Ok(Object::String(bytes, _)) => decode_pdf_string(bytes),
        Ok(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
        _ => String::new(),
    }
}

fn set_info_string(dict: &mut Dictionary, key: &str, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        dict.remove(key.as_bytes());
        return;
    }
    dict.set(
        key,
        Object::String(trimmed.as_bytes().to_vec(), StringFormat::Literal),
    );
}

fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let utf16: Vec<u16> = bytes[2..]
            .chunks(2)
            .filter_map(|c| {
                if c.len() == 2 {
                    Some(u16::from_be_bytes([c[0], c[1]]))
                } else {
                    None
                }
            })
            .collect();
        return String::from_utf16_lossy(&utf16);
    }
    String::from_utf8_lossy(bytes).into_owned()
}
