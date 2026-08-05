use crate::error::{AppError, OpResult};
use crate::pdf_engine::{ensure_dir, ensure_pdf_path};
use lopdf::Document;
use std::path::PathBuf;
use std::time::Instant;

/// Ranges are 1-based inclusive (start, end).
pub fn split_pdf(
    path: String,
    ranges: Vec<(u32, u32)>,
    output_dir: String,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let out_dir = ensure_dir(&output_dir)?;

    if ranges.is_empty() {
        return Err(AppError::InvalidInput(
            "At least one page range is required".into(),
        ));
    }

    let doc = Document::load(&input)?;
    let pages = doc.get_pages();
    let total = pages.len() as u32;

    if total == 0 {
        return Err(AppError::InvalidInput("PDF has no pages".into()));
    }

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("split");

    let mut outputs = Vec::new();
    let mut page_count = 0u32;

    for (idx, (start, end)) in ranges.iter().enumerate() {
        if *start == 0 || *end == 0 {
            return Err(AppError::InvalidInput(
                "Page numbers are 1-based and must be >= 1".into(),
            ));
        }
        if start > end {
            return Err(AppError::InvalidInput(format!(
                "Invalid range {start}-{end}: start must be <= end"
            )));
        }
        if *end > total {
            return Err(AppError::InvalidInput(format!(
                "Range {start}-{end} exceeds total pages ({total})"
            )));
        }

        let page_numbers: Vec<u32> = (*start..=*end).collect();
        let mut new_doc = extract_pages(&doc, &page_numbers)?;

        let filename = if ranges.len() == 1 {
            format!("{stem}_p{start}-{end}.pdf")
        } else {
            format!("{stem}_part{}_{start}-{end}.pdf", idx + 1)
        };
        let out_path: PathBuf = out_dir.join(filename);
        new_doc.save(&out_path)?;
        outputs.push(out_path.to_string_lossy().to_string());
        page_count += page_numbers.len() as u32;
    }

    Ok(OpResult::new(
        outputs,
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn extract_pages(source: &Document, page_numbers: &[u32]) -> Result<Document, AppError> {
    let mut doc = source.clone();
    let pages = doc.get_pages();
    let to_delete: Vec<u32> = pages
        .keys()
        .copied()
        .filter(|n| !page_numbers.contains(n))
        .collect();

    for page_num in to_delete.into_iter().rev() {
        doc.delete_pages(&[page_num]);
    }

    doc.compress();
    Ok(doc)
}
