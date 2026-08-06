use crate::error::{AppError, OpResult};
use crate::pdf_engine::{create_pdfium, ensure_parent_dir, ensure_pdf_path};
use image::codecs::jpeg::JpegEncoder;
use image::{ImageEncoder, Rgb, RgbImage};
use lopdf::{Dictionary, Document, Object, Stream};
use pdfium_render::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
pub struct RedactRegion {
    /// 1-based page number
    pub page: u32,
    /// PDF points, origin bottom-left
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Secure redaction: rasterize pages, burn opaque black into pixels, rebuild as
/// image-only PDF (no selectable text, no fillable form fields underneath).
pub fn redact_pdf(
    path: String,
    output: String,
    regions: Vec<RedactRegion>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    if regions.is_empty() {
        return Err(AppError::InvalidInput(
            "Añade al menos una zona de censura".into(),
        ));
    }

    // Probe page count / encryption with lopdf first (clear Spanish errors).
    {
        let doc = Document::load(&input)?;
        if doc.is_encrypted() {
            return Err(AppError::InvalidInput(
                "Desbloquea el PDF antes de censurar".into(),
            ));
        }
        let total = doc.get_pages().len() as u32;
        if total == 0 {
            return Err(AppError::InvalidInput("PDF sin páginas".into()));
        }
        for r in &regions {
            if r.page == 0 || r.page > total {
                return Err(AppError::InvalidInput(format!(
                    "Página {} fuera de rango (1-{total})",
                    r.page
                )));
            }
            if r.w <= 0.0 || r.h <= 0.0 {
                return Err(AppError::InvalidInput(
                    "Cada zona debe tener ancho y alto > 0".into(),
                ));
            }
        }
    }

    let mut by_page: HashMap<u32, Vec<RedactRegion>> = HashMap::new();
    for r in regions {
        by_page.entry(r.page).or_default().push(r);
    }

    let pdfium = create_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(&input, None)
        .map_err(|e| AppError::Pdfium(e.to_string()))?;

    // ~200 DPI — enough for print/read, burns pixels so OCR/copy can't recover.
    let dpi = 200u32;
    let scale = dpi as f32 / 72.0;
    let render_config = PdfRenderConfig::new().scale_page_by_factor(scale);

    let mut raster_pages = Vec::new();
    for (index, page) in document.pages().iter().enumerate() {
        let page_num = (index + 1) as u32;
        let width_pts = page.width().value.max(1.0);
        let height_pts = page.height().value.max(1.0);

        let rendered = page
            .render_with_config(&render_config)
            .map_err(|e| AppError::Pdfium(e.to_string()))?
            .as_image();
        let mut rgb = rendered.to_rgb8();

        if let Some(regs) = by_page.get(&page_num) {
            for r in regs {
                burn_black(&mut rgb, width_pts, height_pts, r);
            }
        }

        let jpeg = encode_jpeg_rgb(&rgb, 92)?;
        raster_pages.push(FlatPage {
            width_pts,
            height_pts,
            img_w: rgb.width(),
            img_h: rgb.height(),
            jpeg,
        });
    }

    let page_count = raster_pages.len() as u32;
    build_flat_pdf(&raster_pages, output_path)?;

    Ok(OpResult::new(
        vec![output],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

struct FlatPage {
    width_pts: f32,
    height_pts: f32,
    img_w: u32,
    img_h: u32,
    jpeg: Vec<u8>,
}

fn burn_black(img: &mut RgbImage, page_w: f32, page_h: f32, r: &RedactRegion) {
    let sx = img.width() as f32 / page_w.max(1.0);
    let sy = img.height() as f32 / page_h.max(1.0);

    // PDF origin bottom-left → bitmap origin top-left. Pad 1px so edges don't leak.
    let x0 = ((r.x * sx).floor() as i32 - 1).max(0) as u32;
    let y0 = (((page_h - r.y - r.h) * sy).floor() as i32 - 1).max(0) as u32;
    let x1 = (((r.x + r.w) * sx).ceil() as i32 + 1).clamp(0, img.width() as i32) as u32;
    let y1 = (((page_h - r.y) * sy).ceil() as i32 + 1).clamp(0, img.height() as i32) as u32;

    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for y in y0..y1 {
        for x in x0..x1 {
            img.put_pixel(x, y, Rgb([0, 0, 0]));
        }
    }
}

fn encode_jpeg_rgb(rgb: &RgbImage, quality: u8) -> Result<Vec<u8>, AppError> {
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
    Ok(buf.into_inner())
}

fn build_flat_pdf(pages: &[FlatPage], output: &Path) -> Result<(), AppError> {
    if pages.is_empty() {
        return Err(AppError::InvalidInput("PDF sin páginas".into()));
    }

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::new();

    for (i, page) in pages.iter().enumerate() {
        let mut img_dict = Dictionary::new();
        img_dict.set("Type", "XObject");
        img_dict.set("Subtype", "Image");
        img_dict.set("Width", page.img_w as i64);
        img_dict.set("Height", page.img_h as i64);
        img_dict.set("ColorSpace", "DeviceRGB");
        img_dict.set("BitsPerComponent", 8);
        img_dict.set("Filter", "DCTDecode");
        img_dict.set("Length", page.jpeg.len() as i64);
        let image_id = doc.add_object(Object::Stream(Stream::new(img_dict, page.jpeg.clone())));

        let xname = format!("Im{i}");
        let content = format!(
            "q\n{:.4} 0 0 {:.4} 0 0 cm\n/{xname} Do\nQ\n",
            page.width_pts, page.height_pts
        );
        let content_id = doc.add_object(Object::Stream(Stream::new(
            Dictionary::new(),
            content.into_bytes(),
        )));

        let mut xobject = Dictionary::new();
        xobject.set(xname.as_str(), image_id);
        let mut resources = Dictionary::new();
        resources.set("XObject", xobject);

        let mut page_dict = Dictionary::new();
        page_dict.set("Type", "Page");
        page_dict.set("Parent", pages_id);
        page_dict.set(
            "MediaBox",
            vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Real(page.width_pts),
                Object::Real(page.height_pts),
            ],
        );
        page_dict.set("Contents", content_id);
        page_dict.set("Resources", resources);
        // No Annots / AcroForm — forms and widgets cannot survive flatten.

        page_ids.push(doc.add_object(page_dict));
    }

    let page_count = page_ids.len() as u32;
    let kids: Vec<Object> = page_ids.iter().map(|&id| Object::Reference(id)).collect();

    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", "Pages");
    pages_dict.set("Count", page_count as i64);
    pages_dict.set("Kids", kids);
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);
    doc.max_id = doc.objects.len() as u32;
    doc.compress();
    doc.save(output)?;
    Ok(())
}
