use crate::error::{AppError, OpResult};
use crate::pdf_engine::{ensure_parent_dir, ensure_pdf_path};
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::path::Path;
use std::time::Instant;

/// Overlay page numbers on every page.
/// `format` supports `{n}` and `{total}`. Position: bottom-center|bottom-right|bottom-left|top-center
pub fn add_page_numbers(
    path: String,
    output: String,
    position: String,
    format: Option<String>,
    start_from: Option<u32>,
    font_size: Option<f32>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    let fmt = format.unwrap_or_else(|| "{n}".into());
    let start = start_from.unwrap_or(1).max(1);
    let size = font_size.unwrap_or(10.0).clamp(6.0, 48.0);

    let mut doc = Document::load(&input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Desbloquea el PDF antes de numerar páginas".into(),
        ));
    }

    let pages: Vec<(u32, ObjectId)> = doc.get_pages().into_iter().collect();
    let total = pages.len() as u32;
    if total == 0 {
        return Err(AppError::InvalidInput("PDF sin páginas".into()));
    }

    let mut font_dict = Dictionary::new();
    font_dict.set("Type", "Font");
    font_dict.set("Subtype", "Type1");
    font_dict.set("BaseFont", "Helvetica");
    let font_id = doc.add_object(font_dict);

    for (page_num, page_id) in &pages {
        let display_n = start + page_num - 1;
        let label = fmt
            .replace("{n}", &display_n.to_string())
            .replace("{total}", &total.to_string());
        let (width, height) = page_size(&doc, *page_id).unwrap_or((595.0, 842.0));
        let (x, y) = match position.as_str() {
            "bottom-right" => (width - 48.0, 28.0),
            "bottom-left" => (36.0, 28.0),
            "top-center" | "top" => (width / 2.0 - 10.0, height - 28.0),
            _ => (width / 2.0 - 10.0, 28.0), // bottom-center
        };

        let escaped = escape_pdf_string(&label);
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["FNum".into(), Object::Real(size)]),
                Operation::new("Td", vec![Object::Real(x), Object::Real(y)]),
                Operation::new("Tj", vec![Object::string_literal(escaped.as_str())]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_data = content
            .encode()
            .map_err(|e| AppError::Pdf(e.to_string()))?;
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content_data));

        attach_font_and_content(&mut doc, *page_id, font_id, content_id)?;
    }

    doc.compress();
    doc.save(output_path)?;

    Ok(OpResult::new(
        vec![output],
        total,
        started.elapsed().as_millis() as u64,
    ))
}

fn attach_font_and_content(
    doc: &mut Document,
    page_id: ObjectId,
    font_id: ObjectId,
    content_id: ObjectId,
) -> Result<(), AppError> {
    // Clone page dict, mutate offline, write back — avoids nested borrow issues.
    let mut page = doc
        .get_object(page_id)
        .and_then(|o| o.as_dict())
        .map(|d| d.clone())
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    // Resources / Font
    let mut resources = match page.get(b"Resources") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc
            .get_object(*id)
            .and_then(|o| o.as_dict())
            .map(|d| d.clone())
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };

    let mut fonts = match resources.get(b"Font") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc
            .get_object(*id)
            .and_then(|o| o.as_dict())
            .map(|d| d.clone())
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };
    fonts.set("FNum", font_id);
    resources.set("Font", fonts);
    page.set("Resources", resources);

    // Contents append
    match page.get(b"Contents").ok().cloned() {
        Some(Object::Reference(existing)) => {
            page.set(
                "Contents",
                Object::Array(vec![
                    Object::Reference(existing),
                    Object::Reference(content_id),
                ]),
            );
        }
        Some(Object::Array(mut arr)) => {
            arr.push(Object::Reference(content_id));
            page.set("Contents", Object::Array(arr));
        }
        _ => {
            page.set("Contents", content_id);
        }
    }

    doc.objects.insert(page_id, Object::Dictionary(page));
    Ok(())
}

fn page_size(doc: &Document, page_id: ObjectId) -> Option<(f32, f32)> {
    let dict = doc.get_object(page_id).ok()?.as_dict().ok()?;
    let box_obj = dict
        .get(b"MediaBox")
        .or_else(|_| dict.get(b"CropBox"))
        .ok()?;
    let arr = box_obj.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    let x0 = to_f32(&arr[0])?;
    let y0 = to_f32(&arr[1])?;
    let x1 = to_f32(&arr[2])?;
    let y1 = to_f32(&arr[3])?;
    Some((x1 - x0, y1 - y0))
}

fn to_f32(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r as f32),
        _ => None,
    }
}

fn escape_pdf_string(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '(' | ')' | '\\' => format!("\\{c}"),
            c if c.is_ascii() => c.to_string(),
            _ => "?".into(),
        })
        .collect()
}
