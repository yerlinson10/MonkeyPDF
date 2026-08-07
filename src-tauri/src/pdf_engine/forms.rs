use crate::error::AppError;
use crate::pdf_engine::ensure_pdf_path;
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FormFieldKind {
    Text,
    Checkbox,
    Radio,
    Choice,
    Signature,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    pub page: u32,
    pub name: String,
    pub kind: FormFieldKind,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub value: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldFill {
    pub name: String,
    pub value: String,
}

/// List interactive AcroForm fields by walking page /Annots (most reliable).
pub fn get_form_fields(path: &str) -> Result<Vec<FormField>, AppError> {
    let input = ensure_pdf_path(path)?;
    let doc = Document::load(input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Desbloquea el PDF antes de leer el formulario".into(),
        ));
    }

    let pages = doc.get_pages();
    let mut page_by_id: HashMap<ObjectId, u32> = HashMap::new();
    for (n, id) in &pages {
        page_by_id.insert(*id, *n);
    }

    let mut seen: HashSet<ObjectId> = HashSet::new();
    let mut out = Vec::new();

    for (page_num, page_id) in &pages {
        let annots = collect_annots(&doc, *page_id);
        for ann_id in annots {
            if !seen.insert(ann_id) {
                continue;
            }
            if let Some(field) = parse_widget(&doc, ann_id, *page_num, &page_by_id) {
                out.push(field);
            }
        }
    }

    if out.is_empty() {
        let field_ids = collect_field_ids_from_acroform(&doc);
        for id in field_ids {
            if !seen.insert(id) {
                continue;
            }
            if let Some(field) = parse_widget(&doc, id, 0, &page_by_id) {
                out.push(field);
            }
        }
    }

    out.sort_by(|a, b| a.page.cmp(&b.page).then(a.name.cmp(&b.name)));
    Ok(out)
}
/// Set field values and mark NeedAppearances so viewers regenerate appearances.
pub fn fill_form(doc: &mut Document, fills: &[FieldFill]) -> Result<(), AppError> {
    if fills.is_empty() {
        return Ok(());
    }
    let by_name: HashMap<&str, &str> = fills
        .iter()
        .map(|f| (f.name.as_str(), f.value.as_str()))
        .collect();

    let pages = doc.get_pages();
    let mut visited: HashSet<ObjectId> = HashSet::new();
    for (_n, page_id) in &pages {
        let annots = collect_annots(doc, *page_id);
        for ann_id in annots {
            if !visited.insert(ann_id) {
                continue;
            }
            let owner = find_owner_field(doc, ann_id);
            if let Some(owner_id) = owner {
                let name = field_full_name(doc, owner_id).unwrap_or_default();
                if let Some(value) = by_name.get(name.as_str()) {
                    set_field_value(doc, owner_id, value);
                }
            }
        }
    }

    let catalog_id = doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|o| o.as_reference().ok())
        .ok_or_else(|| AppError::Pdf("Catalog missing".into()))?;
    let catalog = doc
        .get_object(catalog_id)
        .and_then(|o| o.as_dict())
        .map(|d| d.clone())
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    if let Ok(Object::Reference(af_id)) = catalog.get(b"AcroForm") {
        let af_id = *af_id;
        if let Ok(Object::Dictionary(af)) = doc.get_object(af_id).cloned() {
            let mut af = af;
            af.set("NeedAppearances", Object::Boolean(true));
            doc.objects.insert(af_id, Object::Dictionary(af));
        }
    } else if let Ok(Object::Dictionary(af)) = catalog.get(b"AcroForm") {
        let mut af = af.clone();
        af.set("NeedAppearances", Object::Boolean(true));
        let mut cat = catalog;
        cat.set("AcroForm", Object::Dictionary(af));
        doc.objects.insert(catalog_id, Object::Dictionary(cat));
    }

    Ok(())
}

fn collect_annots(doc: &Document, page_id: ObjectId) -> Vec<ObjectId> {
    let Ok(page) = doc.get_object(page_id).and_then(|o| o.as_dict()) else {
        return Vec::new();
    };
    let annots_obj = match page.get(b"Annots") {
        Ok(Object::Array(a)) => a.clone(),
        Ok(Object::Reference(r)) => match doc.get_object(*r).ok() {
            Some(Object::Array(a)) => a.clone(),
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    annots_obj
        .iter()
        .filter_map(|o| o.as_reference().ok())
        .collect()
}

fn collect_field_ids_from_acroform(doc: &Document) -> Vec<ObjectId> {
    let catalog = match catalog_dict(doc) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let acro = match catalog.get(b"AcroForm") {
        Ok(Object::Reference(id)) => doc.get_object(*id).and_then(|o| o.as_dict()).ok(),
        Ok(Object::Dictionary(d)) => Some(d),
        _ => None,
    };
    let Some(acro) = acro else { return Vec::new() };
    let fields = match acro.get(b"Fields") {
        Ok(Object::Array(arr)) => arr.clone(),
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut stack: Vec<ObjectId> = fields.iter().filter_map(|o| o.as_reference().ok()).collect();
    while let Some(id) = stack.pop() {
        let Ok(dict) = doc.get_object(id).and_then(|o| o.as_dict()) else { continue };
        out.push(id);
        if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
            for k in kids {
                if let Ok(kid) = k.as_reference() {
                    stack.push(kid);
                }
            }
        }
    }
    out
}

fn catalog_dict(doc: &Document) -> Result<Dictionary, AppError> {
    let root = doc.trailer.get(b"Root").map_err(|e| AppError::Pdf(e.to_string()))?;
    let id = root.as_reference().map_err(|e| AppError::Pdf(e.to_string()))?;
    doc.get_object(id)
        .and_then(|o| o.as_dict())
        .map(|d| d.clone())
        .map_err(|e| AppError::Pdf(e.to_string()))
}

fn parse_widget(
    doc: &Document,
    ann_id: ObjectId,
    page: u32,
    page_by_id: &HashMap<ObjectId, u32>,
) -> Option<FormField> {
    let dict = doc.get_object(ann_id).and_then(|o| o.as_dict()).ok()?;

    let subtype = dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok());
    if subtype != Some(b"Widget") {
        return None;
    }

    let owner = find_owner_field(doc, ann_id).unwrap_or(ann_id);
    let name = field_full_name(doc, owner).unwrap_or_default();
    if name.is_empty() {
        return None;
    }

    let ft = inherited_ft(doc, owner).unwrap_or_default();
    let kind = match ft.as_str() {
        "Tx" => FormFieldKind::Text,
        "Btn" => {
            let flags = inherited_flags(doc, owner).unwrap_or(0);
            if flags & (1 << 15) != 0 {
                FormFieldKind::Radio
            } else if flags & (1 << 16) != 0 {
                FormFieldKind::Unknown
            } else {
                FormFieldKind::Checkbox
            }
        }
        "Ch" => FormFieldKind::Choice,
        "Sig" => FormFieldKind::Signature,
        _ => FormFieldKind::Unknown,
    };

    let rect = match dict.get(b"Rect") {
        Ok(Object::Array(arr)) if arr.len() >= 4 => {
            let x0 = to_f32(&arr[0]).unwrap_or(0.0);
            let y0 = to_f32(&arr[1]).unwrap_or(0.0);
            let x1 = to_f32(&arr[2]).unwrap_or(0.0);
            let y1 = to_f32(&arr[3]).unwrap_or(0.0);
            (x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs())
        }
        _ => return None,
    };

    let resolved_page = dict
        .get(b"P")
        .ok()
        .and_then(|o| o.as_reference().ok())
        .and_then(|id| page_by_id.get(&id).copied())
        .unwrap_or(page);

    let value = extract_value(doc, owner);
    let options = extract_options(doc, owner);

    Some(FormField {
        page: resolved_page.max(1),
        name,
        kind,
        x: rect.0,
        y: rect.1,
        w: rect.2,
        h: rect.3,
        value,
        options,
    })
}

fn find_owner_field(doc: &Document, start: ObjectId) -> Option<ObjectId> {
    let mut current = start;
    for _ in 0..32 {
        let dict = doc.get_object(current).ok()?.as_dict().ok()?;
        if dict.get(b"T").is_ok() {
            return Some(current);
        }
        match dict.get(b"Parent") {
            Ok(Object::Reference(p)) => current = *p,
            _ => return None,
        }
    }
    None
}

fn field_full_name(doc: &Document, id: ObjectId) -> Option<String> {
    let mut parts = Vec::new();
    let mut current = id;
    for _ in 0..32 {
        let dict = doc.get_object(current).ok()?.as_dict().ok()?;
        if let Ok(Object::String(bytes, _)) = dict.get(b"T") {
            let s = String::from_utf8_lossy(bytes).to_string();
            if !s.is_empty() {
                parts.push(s);
            }
        }
        match dict.get(b"Parent") {
            Ok(Object::Reference(p)) => current = *p,
            _ => break,
        }
    }
    parts.reverse();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

fn inherited_ft(doc: &Document, id: ObjectId) -> Option<String> {
    let mut current = id;
    for _ in 0..32 {
        let dict = doc.get_object(current).ok()?.as_dict().ok()?;
        if let Ok(Object::Name(n)) = dict.get(b"FT") {
            return Some(String::from_utf8_lossy(n).to_string());
        }
        match dict.get(b"Parent") {
            Ok(Object::Reference(p)) => current = *p,
            _ => break,
        }
    }
    None
}

fn inherited_flags(doc: &Document, id: ObjectId) -> Option<i64> {
    let mut current = id;
    for _ in 0..32 {
        let dict = doc.get_object(current).ok()?.as_dict().ok()?;
        if let Ok(Object::Integer(i)) = dict.get(b"Ff") {
            return Some(*i);
        }
        match dict.get(b"Parent") {
            Ok(Object::Reference(p)) => current = *p,
            _ => break,
        }
    }
    None
}

fn extract_value(doc: &Document, owner: ObjectId) -> String {
    let dict = match doc.get_object(owner).and_then(|o| o.as_dict()) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };
    match dict.get(b"V") {
        Ok(Object::String(bytes, _)) => String::from_utf8_lossy(bytes).to_string(),
        Ok(Object::Name(n)) => String::from_utf8_lossy(n).to_string(),
        Ok(Object::Boolean(b)) => if *b { "Yes" } else { "Off" }.into(),
        _ => String::new(),
    }
}

fn extract_options(doc: &Document, owner: ObjectId) -> Vec<String> {
    let dict = match doc.get_object(owner).and_then(|o| o.as_dict()) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let Ok(Object::Array(arr)) = dict.get(b"Opt") else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|o| match o {
            Object::String(b, _) => Some(String::from_utf8_lossy(b).to_string()),
            Object::Array(inner) if !inner.is_empty() => match &inner[0] {
                Object::String(b, _) => Some(String::from_utf8_lossy(b).to_string()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn set_field_value(doc: &mut Document, owner_id: ObjectId, value: &str) {
    let Ok(dict) = doc.get_object(owner_id).and_then(|o| o.as_dict()) else {
        return;
    };
    let ft = dict.get(b"FT").ok().and_then(|o| o.as_name().ok()).map(|n| String::from_utf8_lossy(n).to_string());
    let is_btn = ft.as_deref() == Some("Btn");
    let has_name_v = matches!(dict.get(b"V"), Ok(Object::Name(_)));

    let mut cloned = dict.clone();
    if has_name_v || is_btn {
        let on = if value.is_empty() || value.eq_ignore_ascii_case("off") || value == "0" {
            "Off"
        } else if value.eq_ignore_ascii_case("yes") || value == "1" || value.eq_ignore_ascii_case("true") {
            "Yes"
        } else {
            value
        };
        cloned.set("V", Object::Name(on.as_bytes().to_vec()));
        cloned.set("AS", Object::Name(on.as_bytes().to_vec()));
    } else {
        cloned.set("V", Object::string_literal(value));
    }
    doc.objects.insert(owner_id, Object::Dictionary(cloned));
}

fn to_f32(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r as f32),
        _ => None,
    }
}
