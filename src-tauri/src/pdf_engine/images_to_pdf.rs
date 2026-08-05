use crate::error::{AppError, OpResult};
use crate::pdf_engine::{ensure_image_path, ensure_parent_dir};
use image::codecs::jpeg::JpegEncoder;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageEncoder, ImageReader};
use lopdf::{Dictionary, Document, Object, Stream};
use std::io::Cursor;
use std::path::Path;
use std::time::Instant;

/// A4 portrait in points (72 dpi). Used as the max page box; actual page
/// matches each image's aspect ratio so the photo fills the page edge-to-edge.
const MAX_PAGE_W: f32 = 595.0;
const MAX_PAGE_H: f32 = 842.0;

pub fn images_to_pdf(paths: Vec<String>, output: String) -> Result<OpResult, AppError> {
    let started = Instant::now();

    if paths.is_empty() {
        return Err(AppError::InvalidInput(
            "At least one image is required".into(),
        ));
    }

    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = Vec::with_capacity(paths.len());

    for (index, path) in paths.iter().enumerate() {
        let img_path = ensure_image_path(path)?;
        let img = load_oriented_image(&img_path)?;
        let (img_w, img_h) = (img.width(), img.height());
        if img_w == 0 || img_h == 0 {
            return Err(AppError::InvalidInput(format!(
                "Image has invalid dimensions: {path}"
            )));
        }

        let (page_w, page_h) = page_size_for_image(img_w, img_h);

        let rgb = img.to_rgb8();
        let mut jpeg_buf = Cursor::new(Vec::new());
        let encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, 92);
        encoder
            .write_image(
                rgb.as_raw(),
                img_w,
                img_h,
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| AppError::Image(e.to_string()))?;
        let jpeg_bytes = jpeg_buf.into_inner();

        let mut img_dict = Dictionary::new();
        img_dict.set("Type", "XObject");
        img_dict.set("Subtype", "Image");
        img_dict.set("Width", img_w as i64);
        img_dict.set("Height", img_h as i64);
        img_dict.set("ColorSpace", "DeviceRGB");
        img_dict.set("BitsPerComponent", 8);
        img_dict.set("Filter", "DCTDecode");
        img_dict.set("Length", jpeg_bytes.len() as i64);
        let image_id = doc.add_object(Object::Stream(Stream::new(img_dict, jpeg_bytes)));

        // Unique XObject name per page (avoids viewer quirks with reused names).
        let xname = format!("Im{index}");
        // Image fills the page completely — no letterboxing / offset.
        let content = format!("q\n{page_w:.4} 0 0 {page_h:.4} 0 0 cm\n/{xname} Do\nQ\n");
        let content_id = doc.add_object(Object::Stream(Stream::new(
            Dictionary::new(),
            content.into_bytes(),
        )));

        let mut xobject = Dictionary::new();
        xobject.set(xname.as_str(), image_id);
        let mut resources = Dictionary::new();
        resources.set("XObject", xobject);

        let mut page = Dictionary::new();
        page.set("Type", "Page");
        page.set("Parent", pages_id);
        page.set(
            "MediaBox",
            vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Real(page_w),
                Object::Real(page_h),
            ],
        );
        page.set("Contents", content_id);
        page.set("Resources", resources);

        page_ids.push(doc.add_object(page));
    }

    let page_count = page_ids.len() as u32;
    let kids: Vec<Object> = page_ids.iter().map(|&id| Object::Reference(id)).collect();

    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Count", page_count as i64);
    pages.set("Kids", kids);
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);
    doc.renumber_objects();
    doc.compress();
    doc.save(output_path)?;

    Ok(OpResult::new(
        vec![output],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

/// Load image and apply EXIF orientation so phone photos aren't rotated/misplaced.
fn load_oriented_image(path: &Path) -> Result<DynamicImage, AppError> {
    let reader = ImageReader::open(path)
        .map_err(|e| AppError::Image(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| AppError::Image(e.to_string()))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| AppError::Image(e.to_string()))?;
    let orientation = decoder
        .orientation()
        .unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| AppError::Image(e.to_string()))?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// Page box matches image aspect ratio and fits inside an A4-sized envelope
/// (portrait or landscape depending on the photo).
fn page_size_for_image(img_w: u32, img_h: u32) -> (f32, f32) {
    let w = img_w as f32;
    let h = img_h as f32;
    let (max_w, max_h) = if w >= h {
        // Landscape page: A4 rotated
        (MAX_PAGE_H, MAX_PAGE_W)
    } else {
        (MAX_PAGE_W, MAX_PAGE_H)
    };
    let scale = (max_w / w).min(max_h / h);
    ((w * scale).max(1.0), (h * scale).max(1.0))
}
