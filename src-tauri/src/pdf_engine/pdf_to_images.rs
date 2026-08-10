use crate::error::{AppError, OpResult};
use crate::pdf_engine::{create_pdfium, ensure_dir, ensure_pdf_path, Progress};
use pdfium_render::prelude::*;
use std::path::PathBuf;
use std::time::Instant;

pub fn pdf_to_jpg(
    path: String,
    dpi: u32,
    output_dir: String,
    progress: Option<Progress>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let progress = progress.unwrap_or_else(Progress::none);
    let dpi = dpi.clamp(72, 600);
    let input = ensure_pdf_path(&path)?;
    let out_dir = ensure_dir(&output_dir)?;

    let pdfium = create_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(&input, None)
        .map_err(|e| AppError::Pdfium(e.to_string()))?;

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("page");

    let scale = dpi as f32 / 72.0;
    let render_config = PdfRenderConfig::new().scale_page_by_factor(scale);

    let mut outputs = Vec::new();
    let page_count = document.pages().len() as u32;

    for (index, page) in document.pages().iter().enumerate() {
        progress.tick(
            (index as u32) + 1,
            page_count.max(1),
            format!("Renderizando página {}/{}", index + 1, page_count),
        )?;
        let image = page
            .render_with_config(&render_config)
            .map_err(|e| AppError::Pdfium(e.to_string()))?
            .as_image();

        let filename = format!("{stem}_page{:03}.jpg", index + 1);
        let out_path: PathBuf = out_dir.join(&filename);
        image
            .save_with_format(&out_path, image::ImageFormat::Jpeg)
            .map_err(|e| AppError::Image(e.to_string()))?;
        outputs.push(out_path.to_string_lossy().to_string());
    }

    Ok(OpResult::new(
        outputs,
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}
