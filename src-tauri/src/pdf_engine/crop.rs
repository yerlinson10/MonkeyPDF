use crate::error::{AppError, OpResult};
use crate::pdf_engine::{ensure_parent_dir, ensure_pdf_path};
use lopdf::{Document, Object, ObjectId};
use serde::Deserialize;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
pub struct CropBox {
    /// PDF points, origin bottom-left
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Set CropBox (+ MediaBox) on selected pages (or all).
pub fn crop_pdf(
    path: String,
    output: String,
    crop: CropBox,
    pages: Option<Vec<u32>>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    if crop.w <= 1.0 || crop.h <= 1.0 {
        return Err(AppError::InvalidInput(
            "El recorte debe tener ancho y alto mayores que 1 pt".into(),
        ));
    }

    let mut doc = Document::load(&input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Desbloquea el PDF antes de recortar".into(),
        ));
    }

    let all_pages = doc.get_pages();
    let total = all_pages.len() as u32;
    if total == 0 {
        return Err(AppError::InvalidInput("PDF sin páginas".into()));
    }

    let targets: Vec<u32> = match pages {
        Some(list) if !list.is_empty() => {
            for n in &list {
                if *n == 0 || *n > total {
                    return Err(AppError::InvalidInput(format!(
                        "Página {n} fuera de rango (1-{total})"
                    )));
                }
            }
            list
        }
        _ => all_pages.keys().copied().collect(),
    };

    let x1 = crop.x + crop.w;
    let y1 = crop.y + crop.h;
    let box_arr = vec![
        Object::Real(crop.x),
        Object::Real(crop.y),
        Object::Real(x1),
        Object::Real(y1),
    ];

    let mut cropped = 0u32;
    for page_num in targets {
        let Some(&page_id) = all_pages.get(&page_num) else {
            continue;
        };
        set_page_boxes(&mut doc, page_id, &box_arr)?;
        cropped += 1;
    }

    doc.compress();
    doc.save(output_path)?;

    Ok(OpResult::new(
        vec![output],
        cropped,
        started.elapsed().as_millis() as u64,
    ))
}

fn set_page_boxes(doc: &mut Document, page_id: ObjectId, box_arr: &[Object]) -> Result<(), AppError> {
    let page = doc
        .get_object_mut(page_id)
        .and_then(|o| o.as_dict_mut())
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    page.set("CropBox", Object::Array(box_arr.to_vec()));
    page.set("MediaBox", Object::Array(box_arr.to_vec()));
    Ok(())
}

/// Return MediaBox as (x0, y0, width, height) for a 1-based page.
pub fn page_mediabox(path: &str, page: u32) -> Result<(f32, f32, f32, f32), AppError> {
    let input = ensure_pdf_path(path)?;
    let doc = Document::load(input)?;
    let pages = doc.get_pages();
    let page_id = *pages.get(&page).ok_or_else(|| {
        AppError::InvalidInput(format!("Página {page} fuera de rango"))
    })?;
    let dict = doc
        .get_object(page_id)
        .and_then(|o| o.as_dict())
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let box_obj = dict
        .get(b"MediaBox")
        .or_else(|_| dict.get(b"CropBox"))
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let arr = box_obj
        .as_array()
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    if arr.len() < 4 {
        return Err(AppError::Pdf("MediaBox inválido".into()));
    }
    let x0 = to_f32(&arr[0]).ok_or_else(|| AppError::Pdf("MediaBox x0".into()))?;
    let y0 = to_f32(&arr[1]).ok_or_else(|| AppError::Pdf("MediaBox y0".into()))?;
    let x1 = to_f32(&arr[2]).ok_or_else(|| AppError::Pdf("MediaBox x1".into()))?;
    let y1 = to_f32(&arr[3]).ok_or_else(|| AppError::Pdf("MediaBox y1".into()))?;
    Ok((x0, y0, x1 - x0, y1 - y0))
}

fn to_f32(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r as f32),
        _ => None,
    }
}
