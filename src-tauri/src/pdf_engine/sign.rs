use crate::error::{AppError, OpResult};
use crate::pdf_engine::forms::{fill_form, FieldFill};
use crate::pdf_engine::signatures;
use crate::pdf_engine::{ensure_parent_dir, ensure_pdf_path};
use image::ImageReader;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::Deserialize;
use std::io::Cursor;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignPlacement {
    pub asset_id: Option<String>,
    /// Raw PNG bytes (base64-decoded by command) or empty — prefer asset_id.
    #[serde(default)]
    pub png_bytes: Option<Vec<u8>>,
    /// Alternative: PNG data URL from the frontend.
    #[serde(default)]
    pub png_data_url: Option<String>,
    pub page: u32,
    /// PDF points, origin bottom-left.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Bake visual signature placements + optional form fills into a new PDF.
pub fn sign_pdf(
    signatures_base: &Path,
    path: String,
    output: String,
    placements: Vec<SignPlacement>,
    form_fills: Vec<FieldFill>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    if placements.is_empty() && form_fills.is_empty() {
        return Err(AppError::InvalidInput(
            "Añade al menos una firma o rellena un campo".into(),
        ));
    }

    let mut doc = Document::load(&input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Desbloquea el PDF antes de firmar".into(),
        ));
    }

    let pages = doc.get_pages();
    let total = pages.len() as u32;
    if total == 0 {
        return Err(AppError::InvalidInput("PDF sin páginas".into()));
    }

    for (i, p) in placements.iter().enumerate() {
        if p.page == 0 || p.page > total {
            return Err(AppError::InvalidInput(format!(
                "Página {} fuera de rango (1-{total})",
                p.page
            )));
        }
        if p.w <= 1.0 || p.h <= 1.0 {
            return Err(AppError::InvalidInput(format!(
                "Colocación {i}: tamaño inválido"
            )));
        }

        let png = resolve_png(signatures_base, p)?;
        let page_id = *pages
            .get(&p.page)
            .ok_or_else(|| AppError::Pdf(format!("Página {} no encontrada", p.page)))?;

        place_png_on_page(&mut doc, page_id, &png, p.x, p.y, p.w, p.h, i)?;
    }

    fill_form(&mut doc, &form_fills)?;

    doc.compress();
    doc.save(output_path)?;

    Ok(OpResult::new(
        vec![output],
        total,
        started.elapsed().as_millis() as u64,
    ))
}

fn resolve_png(base: &Path, p: &SignPlacement) -> Result<Vec<u8>, AppError> {
    if let Some(id) = &p.asset_id {
        if !id.is_empty() {
            return signatures::load_png(base, id);
        }
    }
    if let Some(bytes) = &p.png_bytes {
        if !bytes.is_empty() {
            return Ok(bytes.clone());
        }
    }
    if let Some(url) = &p.png_data_url {
        return decode_data_url(url);
    }
    Err(AppError::InvalidInput(
        "Colocación sin imagen de firma".into(),
    ))
}

fn decode_data_url(data_url: &str) -> Result<Vec<u8>, AppError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let raw = if let Some(rest) = data_url.strip_prefix("data:") {
        let comma = rest
            .find(',')
            .ok_or_else(|| AppError::InvalidInput("data URL inválida".into()))?;
        &rest[comma + 1..]
    } else {
        data_url
    };
    B64.decode(raw.trim())
        .map_err(|e| AppError::InvalidInput(format!("Base64 inválido: {e}")))
}

fn place_png_on_page(
    doc: &mut Document,
    page_id: ObjectId,
    png: &[u8],
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    index: usize,
) -> Result<(), AppError> {
    let img = ImageReader::new(Cursor::new(png))
        .with_guessed_format()
        .map_err(|e| AppError::Image(e.to_string()))?
        .decode()
        .map_err(|e| AppError::Image(e.to_string()))?;

    let rgba = img.to_rgba8();
    let (iw, ih) = (rgba.width(), rgba.height());
    if iw == 0 || ih == 0 {
        return Err(AppError::InvalidInput("Imagen de firma vacía".into()));
    }

    let mut rgb = Vec::with_capacity((iw * ih * 3) as usize);
    let mut alpha = Vec::with_capacity((iw * ih) as usize);
    for px in rgba.pixels() {
        rgb.push(px[0]);
        rgb.push(px[1]);
        rgb.push(px[2]);
        alpha.push(px[3]);
    }

    let has_alpha = alpha.iter().any(|&a| a < 255);

    let mut img_dict = Dictionary::new();
    img_dict.set("Type", "XObject");
    img_dict.set("Subtype", "Image");
    img_dict.set("Width", iw as i64);
    img_dict.set("Height", ih as i64);
    img_dict.set("ColorSpace", "DeviceRGB");
    img_dict.set("BitsPerComponent", 8);
    img_dict.set("Filter", "FlateDecode");
    let rgb_stream = Stream::new(img_dict, compress_flate(&rgb)?);

    if has_alpha {
        let mut smask_dict = Dictionary::new();
        smask_dict.set("Type", "XObject");
        smask_dict.set("Subtype", "Image");
        smask_dict.set("Width", iw as i64);
        smask_dict.set("Height", ih as i64);
        smask_dict.set("ColorSpace", "DeviceGray");
        smask_dict.set("BitsPerComponent", 8);
        smask_dict.set("Filter", "FlateDecode");
        let smask_id = doc.add_object(Object::Stream(Stream::new(
            smask_dict,
            compress_flate(&alpha)?,
        )));
        // Re-build rgb stream with SMask — need mutable dict before add.
        let mut img_dict2 = Dictionary::new();
        img_dict2.set("Type", "XObject");
        img_dict2.set("Subtype", "Image");
        img_dict2.set("Width", iw as i64);
        img_dict2.set("Height", ih as i64);
        img_dict2.set("ColorSpace", "DeviceRGB");
        img_dict2.set("BitsPerComponent", 8);
        img_dict2.set("Filter", "FlateDecode");
        img_dict2.set("SMask", Object::Reference(smask_id));
        let image_id = doc.add_object(Object::Stream(Stream::new(
            img_dict2,
            compress_flate(&rgb)?,
        )));
        attach_image(doc, page_id, image_id, x, y, w, h, index)
    } else {
        let image_id = doc.add_object(Object::Stream(rgb_stream));
        attach_image(doc, page_id, image_id, x, y, w, h, index)
    }
}

fn compress_flate(data: &[u8]) -> Result<Vec<u8>, AppError> {
    use std::io::Write;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    enc.write_all(data)
        .map_err(|e| AppError::Pdf(format!("flate: {e}")))?;
    enc.finish()
        .map_err(|e| AppError::Pdf(format!("flate finish: {e}")))
}

fn attach_image(
    doc: &mut Document,
    page_id: ObjectId,
    image_id: ObjectId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    index: usize,
) -> Result<(), AppError> {
    let xname = format!("SigIm{index}");
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    Object::Real(w),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(h),
                    Object::Real(x),
                    Object::Real(y),
                ],
            ),
            Operation::new("Do", vec![xname.as_str().into()]),
            Operation::new("Q", vec![]),
        ],
    };
    let content_data = content
        .encode()
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let content_id = doc.add_object(Stream::new(Dictionary::new(), content_data));

    let mut page = doc
        .get_object(page_id)
        .and_then(|o| o.as_dict())
        .map(|d| d.clone())
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let mut resources = match page.get(b"Resources") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc
            .get_object(*id)
            .and_then(|o| o.as_dict())
            .map(|d| d.clone())
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };

    let mut xobjects = match resources.get(b"XObject") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc
            .get_object(*id)
            .and_then(|o| o.as_dict())
            .map(|d| d.clone())
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };
    xobjects.set(xname.as_str(), image_id);
    resources.set("XObject", xobjects);
    page.set("Resources", resources);

    match page.get(b"Contents").ok().cloned() {
        Some(Object::Reference(existing)) => {
            page.set(
                "Contents",
                Object::Array(vec![
                    Object::Reference(existing),
                    Object::Reference(content_id),
                ]),
            );
        }
        Some(Object::Array(mut arr)) => {
            arr.push(Object::Reference(content_id));
            page.set("Contents", Object::Array(arr));
        }
        _ => {
            page.set("Contents", content_id);
        }
    }

    doc.objects.insert(page_id, Object::Dictionary(page));
    Ok(())
}
