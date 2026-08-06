use crate::error::{AppError, FilePreview, PreviewTextSpan};
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
    let max_width = max_width.clamp(120, 2400);

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
    let height_pts = pdf_page.height().value;
    let scale = max_width as f32 / width_pts.max(1.0);
    let render_config = PdfRenderConfig::new().scale_page_by_factor(scale);

    let jpeg_q = if max_width >= 1400 { 88 } else if max_width >= 900 { 85 } else { 82 };
    let data_url = {
        let bitmap = pdf_page
            .render_with_config(&render_config)
            .map_err(|e| AppError::Pdfium(e.to_string()))?;
        image_to_jpeg_data_url(&bitmap.as_image(), jpeg_q)?
    };

    // Skip text layer for tiny thumbs (keeps rail snappy).
    let text_spans = if max_width >= 360 {
        extract_text_spans(&pdf_page, width_pts, height_pts).unwrap_or_default()
    } else {
        Vec::new()
    };

    Ok(FilePreview {
        data_url,
        page_count,
        page,
        kind: "pdf".into(),
        text_spans,
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
        text_spans: Vec::new(),
    })
}

fn extract_text_spans(
    page: &PdfPage<'_>,
    page_w: f32,
    page_h: f32,
) -> Result<Vec<PreviewTextSpan>, AppError> {
    let page_w = page_w.max(1.0);
    let page_h = page_h.max(1.0);
    let text = page
        .text()
        .map_err(|e| AppError::Pdfium(e.to_string()))?;

    // Collect char boxes, then merge into word/line runs for fewer DOM nodes.
    let mut chars: Vec<(String, f32, f32, f32, f32)> = Vec::new();
    for ch in text.chars().iter() {
        let Some(s) = ch.unicode_string() else { continue };
        if s.is_empty() || s == "\u{0}" {
            continue;
        }
        let Ok(b) = ch.loose_bounds() else { continue };
        let left = b.left().value;
        let right = b.right().value;
        let bottom = b.bottom().value;
        let top = b.top().value;
        if right <= left || top <= bottom {
            continue;
        }
        let x = (left / page_w).clamp(0.0, 1.0);
        let y = (1.0 - top / page_h).clamp(0.0, 1.0);
        let w = ((right - left) / page_w).clamp(0.0, 1.0);
        let h = ((top - bottom) / page_h).clamp(0.0, 1.0);
        chars.push((s, x, y, w, h));
    }

    Ok(merge_spans(chars))
}

fn merge_spans(chars: Vec<(String, f32, f32, f32, f32)>) -> Vec<PreviewTextSpan> {
    let mut out: Vec<PreviewTextSpan> = Vec::new();
    for (s, x, y, w, h) in chars {
        if let Some(last) = out.last_mut() {
            let same_line = (y - last.y).abs() < (h.max(last.h) * 0.45).max(0.004);
            let gap = x - (last.x + last.w);
            let close = gap < (h.max(last.h) * 0.55).max(0.012);
            if same_line && close && last.text.len() < 120 {
                if gap > h.max(last.h) * 0.12 {
                    last.text.push(' ');
                }
                last.text.push_str(&s);
                let right = (x + w).max(last.x + last.w);
                last.w = right - last.x;
                last.y = last.y.min(y);
                last.h = (last.y + last.h).max(y + h) - last.y;
                continue;
            }
        }
        out.push(PreviewTextSpan {
            text: s,
            x,
            y,
            w: w.max(0.004),
            h: h.max(0.006),
        });
        if out.len() >= 900 {
            break;
        }
    }
    out
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
