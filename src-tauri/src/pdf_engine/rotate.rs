use crate::error::{AppError, OpResult};
use crate::pdf_engine::{ensure_parent_dir, ensure_pdf_path};
use lopdf::Document;
use std::path::Path;
use std::time::Instant;

pub fn rotate_pdf(
    path: String,
    angle: u32,
    pages: Option<Vec<u32>>,
    output: String,
) -> Result<OpResult, AppError> {
    let started = Instant::now();

    if !matches!(angle, 90 | 180 | 270) {
        return Err(AppError::InvalidInput(
            "Angle must be 90, 180, or 270 degrees".into(),
        ));
    }

    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    let mut doc = Document::load(&input)?;
    let all_pages = doc.get_pages();
    let total = all_pages.len() as u32;

    let target_pages: Vec<u32> = match pages {
        Some(list) if !list.is_empty() => {
            for n in &list {
                if *n == 0 || *n > total {
                    return Err(AppError::InvalidInput(format!(
                        "Page {n} is out of range (1-{total})"
                    )));
                }
            }
            list
        }
        _ => all_pages.keys().copied().collect(),
    };

    let mut rotated = 0u32;
    for page_num in &target_pages {
        if let Some(&page_id) = all_pages.get(page_num) {
            let page_dict = doc
                .get_object_mut(page_id)
                .and_then(|obj| obj.as_dict_mut())
                .map_err(|e| AppError::Pdf(e.to_string()))?;

            let current_rotation = page_dict
                .get(b"Rotate")
                .and_then(|obj| obj.as_i64())
                .unwrap_or(0);

            let new_rotate = (current_rotation + angle as i64).rem_euclid(360);
            page_dict.set("Rotate", new_rotate);
            rotated += 1;
        }
    }

    doc.compress();
    doc.save(output_path)?;

    Ok(OpResult::new(
        vec![output],
        rotated,
        started.elapsed().as_millis() as u64,
    ))
}
