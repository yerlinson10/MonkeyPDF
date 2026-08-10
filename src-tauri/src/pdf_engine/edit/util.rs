use crate::error::AppError;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

#[allow(dead_code)]
pub fn page_size(doc: &Document, page_id: ObjectId) -> (f32, f32) {
    page_mediabox(doc, page_id)
        .map(|(_x0, _y0, w, h)| (w, h))
        .unwrap_or((595.0, 842.0))
}

/// Returns (x0, y0, width, height) in PDF points.
#[allow(dead_code)]
pub fn page_mediabox(doc: &Document, page_id: ObjectId) -> Option<(f32, f32, f32, f32)> {
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
    Some((x0, y0, x1 - x0, y1 - y0))
}

pub fn to_f32(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r as f32),
        _ => None,
    }
}

pub fn parse_color(hex: &str) -> (f32, f32, f32) {
    let h = hex.trim().trim_start_matches('#');
    if h.len() >= 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(26) as f32 / 255.0;
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(26) as f32 / 255.0;
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(26) as f32 / 255.0;
        (r, g, b)
    } else {
        (0.1, 0.1, 0.1)
    }
}

pub fn escape_pdf_string(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '(' | ')' | '\\' => format!("\\{c}"),
            c if (c as u32) < 128 && !c.is_control() => c.to_string(),
            c => {
                // WinAnsi-ish: keep Latin-1 printable via octal escape
                let code = c as u32;
                if code <= 255 {
                    format!("\\{code:03o}")
                } else {
                    "?".into()
                }
            }
        })
        .collect()
}

pub fn base14_font(family: &str, bold: bool, italic: bool) -> &'static str {
    let fam = family.to_ascii_lowercase();
    match (fam.as_str(), bold, italic) {
        (f, true, true) if f.contains("times") => "Times-BoldItalic",
        (f, true, false) if f.contains("times") => "Times-Bold",
        (f, false, true) if f.contains("times") => "Times-Italic",
        (f, _, _) if f.contains("times") => "Times-Roman",
        (f, true, true) if f.contains("courier") => "Courier-BoldOblique",
        (f, true, false) if f.contains("courier") => "Courier-Bold",
        (f, false, true) if f.contains("courier") => "Courier-Oblique",
        (f, _, _) if f.contains("courier") => "Courier",
        (_, true, true) => "Helvetica-BoldOblique",
        (_, true, false) => "Helvetica-Bold",
        (_, false, true) => "Helvetica-Oblique",
        _ => "Helvetica",
    }
}

/// Approximate glyph width in font units (1000 = em). Helvetica-ish.
pub fn approx_char_width(ch: char) -> f32 {
    match ch {
        'i' | 'l' | 'I' | '!' | '.' | ',' | ':' | ';' | '\'' | '|' => 278.0,
        't' | 'f' | 'r' | 'j' => 333.0,
        ' ' => 278.0,
        'W' | 'M' | 'm' | 'w' => 833.0,
        '%' | '@' => 889.0,
        _ if ch.is_ascii_uppercase() => 667.0,
        _ if ch.is_ascii_digit() => 556.0,
        _ => 556.0,
    }
}

pub fn text_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|c| approx_char_width(c) * font_size / 1000.0)
        .sum()
}

pub fn ensure_type1_font(doc: &mut Document, base_font: &str) -> ObjectId {
    let mut font_dict = Dictionary::new();
    font_dict.set("Type", "Font");
    font_dict.set("Subtype", "Type1");
    font_dict.set("BaseFont", Object::Name(base_font.as_bytes().to_vec()));
    font_dict.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
    doc.add_object(font_dict)
}

pub fn ensure_ext_gstate(doc: &mut Document, alpha: f32) -> ObjectId {
    let a = alpha.clamp(0.05, 1.0);
    let mut gs = Dictionary::new();
    gs.set("Type", "ExtGState");
    gs.set("ca", Object::Real(a));
    gs.set("CA", Object::Real(a));
    doc.add_object(gs)
}

pub fn append_page_content(
    doc: &mut Document,
    page_id: ObjectId,
    ops: Vec<Operation>,
    font_name: Option<&str>,
    font_id: Option<ObjectId>,
    gs_name: Option<&str>,
    gs_id: Option<ObjectId>,
    xobject_name: Option<&str>,
    xobject_id: Option<ObjectId>,
) -> Result<(), AppError> {
    let content = Content { operations: ops };
    let content_data = content
        .encode()
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let content_id = doc.add_object(Stream::new(Dictionary::new(), content_data));

    let mut page = doc
        .get_object(page_id)
        .and_then(|o| o.as_dict())
        .map(|d| d.clone())
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let mut resources = match page.get(b"Resources") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc
            .get_object(*id)
            .and_then(|o| o.as_dict())
            .map(|d| d.clone())
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };

    if let (Some(name), Some(fid)) = (font_name, font_id) {
        let mut fonts = match resources.get(b"Font") {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(id)) => doc
                .get_object(*id)
                .and_then(|o| o.as_dict())
                .map(|d| d.clone())
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        fonts.set(name, fid);
        resources.set("Font", fonts);
    }

    if let (Some(name), Some(gid)) = (gs_name, gs_id) {
        let mut ext_g = match resources.get(b"ExtGState") {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(id)) => doc
                .get_object(*id)
                .and_then(|o| o.as_dict())
                .map(|d| d.clone())
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        ext_g.set(name, gid);
        resources.set("ExtGState", ext_g);
    }

    if let (Some(name), Some(xid)) = (xobject_name, xobject_id) {
        let mut xobjects = match resources.get(b"XObject") {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(id)) => doc
                .get_object(*id)
                .and_then(|o| o.as_dict())
                .map(|d| d.clone())
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        xobjects.set(name, xid);
        resources.set("XObject", xobjects);
    }

    page.set("Resources", resources);

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

pub fn compress_flate(data: &[u8]) -> Result<Vec<u8>, AppError> {
    use std::io::Write;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    enc.write_all(data)
        .map_err(|e| AppError::Pdf(format!("flate: {e}")))?;
    enc.finish()
        .map_err(|e| AppError::Pdf(format!("flate finish: {e}")))
}

pub fn name_to_str(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}
