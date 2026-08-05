use crate::error::{AppError, FilePreview};
use crate::pdf_engine::{create_pdfium, ensure_image_path, ensure_pdf_path};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageEncoder};
use pdfium_render::prelude::*;
use std::io::Cursor;

/// Render a PDF page (1-based) to a JPEG data URL for UI thumbnails / preview.
pub fn preview_pdf(path: String, page: u32, max_width: u32) -> Result<FilePreview, AppError> {
    let input = ensure_pdf_path(&path)?;
    let page = page.max(1);
    let max_width = max_width.clamp(120, 1200);

    let pdfium = create_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(&input, None)
        .map_err(|e| AppError::Pdfium(e.to_string()))?;

    let page_count = document.pages().len() as u32;
    if page > page_count {
        return Err(AppError::InvalidInput(format!(
            "Page {page} out of range (1-{page_count})"
        )));
    }

    let pdf_page = document
        .pages()
        .get(page as u16 - 1)
        .map_err(|e| AppError::Pdfium(e.to_string()))?;

    let width_pts = pdf_page.width().value;
    let scale = max_width as f32 / width_pts.max(1.0);
    let render_config = PdfRenderConfig::new().scale_page_by_factor(scale);

    let image = pdf_page
        .render_with_config(&render_config)
        .map_err(|e| AppError::Pdfium(e.to_string()))?
        .as_image();

    let data_url = image_to_jpeg_data_url(&image, 82)?;

    Ok(FilePreview {
        data_url,
        page_count,
        page,
        kind: "pdf".into(),
    })
}

/// Resize an image file to a JPEG data URL for UI preview.
pub fn preview_image(path: String, max_width: u32) -> Result<FilePreview, AppError> {
    let input = ensure_image_path(&path)?;
    let max_width = max_width.clamp(120, 1200);
    let img = image::open(&input)?;
    let resized = if img.width() > max_width {
        img.resize(max_width, max_width * 4, FilterType::Triangle)
    } else {
        img
    };
    let data_url = image_to_jpeg_data_url(&resized, 85)?;

    Ok(FilePreview {
        data_url,
        page_count: 1,
        page: 1,
        kind: "image".into(),
    })
}

fn image_to_jpeg_data_url(img: &DynamicImage, quality: u8) -> Result<String, AppError> {
    let rgb = img.to_rgb8();
    let mut buf = Cursor::new(Vec::new());
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| AppError::Image(e.to_string()))?;
    let b64 = STANDARD.encode(buf.into_inner());
    Ok(format!("data:image/jpeg;base64,{b64}"))
}
