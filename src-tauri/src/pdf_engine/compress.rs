use crate::error::{AppError, OpResult};
use crate::pdf_engine::{create_pdfium, ensure_parent_dir, ensure_pdf_path, Progress};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageEncoder, RgbImage};
use lopdf::{Dictionary, Document, Object, Stream};
use pdfium_render::prelude::*;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::time::Instant;

/// Compress a PDF. Tries image re-encoding and (when helpful) page rasterization
/// at a DPI/JPEG quality derived from `quality` (1–100). Never writes an output
/// larger than the original — falls back to a copy if nothing shrinks.
pub fn compress_pdf(
    path: String,
    quality: u8,
    output: String,
    progress: Option<Progress>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let progress = progress.unwrap_or_else(Progress::none);
    let quality = quality.clamp(1, 100);

    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    progress.emit(0, 3, "Leyendo PDF");
    let original = fs::read(&input)?;
    let original_size = original.len();

    let mut doc = Document::load(&input)?;
    let page_count = doc.get_pages().len() as u32;

    let mut best = original;

    progress.tick(1, 3, "Recomprimiendo imágenes")?;
    if let Ok(bytes) = compress_by_reencoding_images(&mut doc, quality) {
        if bytes.len() < best.len() {
            best = bytes;
        }
    }

    // Rasterize when the file is still large enough that page JPEGs help,
    // or when image re-encoding barely moved the needle.
    let should_rasterize = best.len() >= original_size.saturating_mul(95) / 100
        || best.len() > 80_000;
    if should_rasterize {
        progress.tick(2, 3, "Rasterizando páginas")?;
        if let Ok(bytes) = compress_by_rasterizing(&input, quality, &progress) {
            if bytes.len() < best.len() {
                best = bytes;
            }
        }
    }

    progress.tick(3, 3, "Escribiendo salida")?;
    fs::write(output_path, &best)?;

    Ok(OpResult::new(
        vec![output],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn compress_by_reencoding_images(doc: &mut Document, quality: u8) -> Result<Vec<u8>, AppError> {
    let max_dim = max_image_dimension(quality);
    let object_ids: Vec<_> = doc.objects.keys().copied().collect();

    for id in object_ids {
        let Some(Object::Stream(stream)) = doc.objects.get(&id).cloned() else {
            continue;
        };
        if !is_image_xobject(&stream) {
            continue;
        }
        if let Some(new_stream) = try_recompress_image(&stream, quality, max_dim) {
            doc.objects.insert(id, Object::Stream(new_stream));
        }
    }

    save_doc_bytes(doc)
}

fn compress_by_rasterizing(
    input: &Path,
    quality: u8,
    progress: &Progress,
) -> Result<Vec<u8>, AppError> {
    let dpi = quality_to_dpi(quality);
    let jpeg_quality = quality_to_jpeg(quality);

    let pdfium = create_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(input, None)
        .map_err(|e| AppError::Pdfium(e.to_string()))?;

    let scale = dpi as f32 / 72.0;
    let render_config = PdfRenderConfig::new().scale_page_by_factor(scale);
    let page_count = document.pages().len() as u32;

    let mut pages: Vec<RasterPage> = Vec::new();
    for (index, page) in document.pages().iter().enumerate() {
        progress.tick(
            (index as u32) + 1,
            page_count.max(1),
            format!("Raster página {}/{}", index + 1, page_count),
        )?;
        let width_pts = page.width().value.max(1.0);
        let height_pts = page.height().value.max(1.0);
        let image = page
            .render_with_config(&render_config)
            .map_err(|e| AppError::Pdfium(e.to_string()))?
            .as_image();
        let jpeg = encode_jpeg(&image, jpeg_quality)?;
        pages.push(RasterPage {
            width_pts,
            height_pts,
            img_w: image.width(),
            img_h: image.height(),
            jpeg,
        });
    }

    build_pdf_from_raster_pages(&pages)
}

struct RasterPage {
    width_pts: f32,
    height_pts: f32,
    img_w: u32,
    img_h: u32,
    jpeg: Vec<u8>,
}

fn build_pdf_from_raster_pages(pages: &[RasterPage]) -> Result<Vec<u8>, AppError> {
    if pages.is_empty() {
        return Err(AppError::InvalidInput("PDF has no pages".into()));
    }

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::new();

    for page in pages {
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

        let content = format!(
            "q\n{:.2} 0 0 {:.2} 0 0 cm\n/Im0 Do\nQ\n",
            page.width_pts, page.height_pts
        );
        let content_id = doc.add_object(Object::Stream(Stream::new(
            Dictionary::new(),
            content.into_bytes(),
        )));

        let mut xobject = Dictionary::new();
        xobject.set("Im0", image_id);
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

    save_doc_bytes(&mut doc)
}

fn save_doc_bytes(doc: &mut Document) -> Result<Vec<u8>, AppError> {
    doc.compress();
    let mut buf = Vec::new();
    doc.save_to(&mut buf)?;
    Ok(buf)
}

fn is_image_xobject(stream: &Stream) -> bool {
    match stream.dict.get(b"Subtype") {
        Ok(Object::Name(name)) => name == b"Image",
        _ => false,
    }
}

fn try_recompress_image(stream: &Stream, quality: u8, max_dim: u32) -> Option<Stream> {
    let width = match stream.dict.get(b"Width").ok()? {
        Object::Integer(w) => *w as u32,
        _ => return None,
    };
    let height = match stream.dict.get(b"Height").ok()? {
        Object::Integer(h) => *h as u32,
        _ => return None,
    };
    if width == 0 || height == 0 {
        return None;
    }

    let color_space = match stream.dict.get(b"ColorSpace").ok() {
        Some(Object::Name(cs)) => cs.as_slice(),
        _ => return None,
    };

    let filter = match stream.dict.get(b"Filter").ok() {
        Some(Object::Name(f)) => Some(f.as_slice()),
        Some(Object::Array(arr)) if arr.len() == 1 => match &arr[0] {
            Object::Name(f) => Some(f.as_slice()),
            _ => None,
        },
        _ => None,
    };

    let mut working = stream.clone();
    let content = if filter == Some(b"DCTDecode") {
        working.content.clone()
    } else {
        // FlateDecode / raw — decompress if needed so we get pixel bytes.
        let _ = working.decompress();
        working.content.clone()
    };

    let mut rgb = if filter == Some(b"DCTDecode") {
        let img = image::load_from_memory(&content).ok()?;
        img.to_rgb8()
    } else if color_space == b"DeviceRGB" {
        if content.len() != (width * height * 3) as usize {
            return None;
        }
        RgbImage::from_raw(width, height, content)?
    } else if color_space == b"DeviceGray" {
        if content.len() != (width * height) as usize {
            return None;
        }
        let gray = image::GrayImage::from_raw(width, height, content)?;
        DynamicImage::ImageLuma8(gray).to_rgb8()
    } else {
        return None;
    };

    let (out_w, out_h) = downsample_dims(rgb.width(), rgb.height(), max_dim);
    if out_w != rgb.width() || out_h != rgb.height() {
        let resized = DynamicImage::ImageRgb8(rgb).resize_exact(out_w, out_h, FilterType::Triangle);
        rgb = resized.to_rgb8();
    }

    let bytes = encode_jpeg(&DynamicImage::ImageRgb8(rgb), quality).ok()?;
    // Only keep the new image if it is meaningfully smaller.
    if bytes.len() + 32 >= stream.content.len() && out_w >= width && out_h >= height {
        return None;
    }

    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Image".to_vec()));
    dict.set("Width", Object::Integer(out_w as i64));
    dict.set("Height", Object::Integer(out_h as i64));
    dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
    dict.set("BitsPerComponent", Object::Integer(8));
    dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
    dict.set("Length", Object::Integer(bytes.len() as i64));

    Some(Stream::new(dict, bytes))
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, AppError> {
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
    Ok(buf.into_inner())
}

fn downsample_dims(width: u32, height: u32, max_dim: u32) -> (u32, u32) {
    let long = width.max(height);
    if long <= max_dim {
        return (width, height);
    }
    let scale = max_dim as f32 / long as f32;
    (
        ((width as f32 * scale).round() as u32).max(1),
        ((height as f32 * scale).round() as u32).max(1),
    )
}

fn max_image_dimension(quality: u8) -> u32 {
    match quality {
        0..=35 => 960,
        36..=55 => 1280,
        56..=75 => 1600,
        _ => 2048,
    }
}

fn quality_to_dpi(quality: u8) -> u32 {
    // 10 → 72 dpi, 70 → ~110, 95 → ~150
    let q = quality as u32;
    72 + ((q.saturating_sub(10)) * 78 / 85).min(78)
}

fn quality_to_jpeg(quality: u8) -> u8 {
    // Slightly more aggressive than the raw slider so size actually drops.
    quality.saturating_sub(5).clamp(25, 90)
}
