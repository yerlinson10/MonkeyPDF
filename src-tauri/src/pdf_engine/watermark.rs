use crate::error::{AppError, OpResult};
use crate::pdf_engine::{ensure_image_path, ensure_parent_dir, ensure_pdf_path};
use image::ImageReader;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::Deserialize;
use std::io::Cursor;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkSpec {
    pub mode: String, // text | image
    pub text: Option<String>,
    pub font: Option<String>,
    pub size: Option<f32>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    pub color: Option<String>,
    pub image_path: Option<String>,
    /// 0–8 grid index (0=TL … 4=center … 8=BR)
    #[serde(default = "default_position")]
    pub position: u32,
    #[serde(default)]
    pub mosaic: bool,
    /// 0–100
    #[serde(default = "default_transparency")]
    pub transparency: u32,
    #[serde(default)]
    pub rotation: f32,
    pub page_from: Option<u32>,
    pub page_to: Option<u32>,
    /// above | below
    #[serde(default = "default_layer")]
    pub layer: String,
}

fn default_position() -> u32 {
    4
}
fn default_transparency() -> u32 {
    50
}
fn default_layer() -> String {
    "above".into()
}

pub fn watermark_pdf(
    path: String,
    output: String,
    spec: WatermarkSpec,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    let mode = spec.mode.to_ascii_lowercase();
    if !matches!(mode.as_str(), "text" | "image") {
        return Err(AppError::InvalidInput(
            "Modo inválido (text|image)".into(),
        ));
    }

    let mut doc = Document::load(&input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Desbloquea el PDF antes de aplicar marca de agua".into(),
        ));
    }

    let pages: Vec<(u32, ObjectId)> = doc.get_pages().into_iter().collect();
    let total = pages.len() as u32;
    if total == 0 {
        return Err(AppError::InvalidInput("PDF sin páginas".into()));
    }

    let from = spec.page_from.unwrap_or(1).max(1);
    let to = spec.page_to.unwrap_or(total).min(total);
    if from > to {
        return Err(AppError::InvalidInput(
            "Rango de páginas inválido".into(),
        ));
    }

    let alpha = (1.0 - (spec.transparency.min(100) as f32 / 100.0)).clamp(0.05, 1.0);
    let rotation = spec.rotation;
    let position = spec.position.min(8);
    let mosaic = spec.mosaic;
    let below = spec.layer.eq_ignore_ascii_case("below");

    // ExtGState for transparency
    let mut gs_dict = Dictionary::new();
    gs_dict.set("Type", "ExtGState");
    gs_dict.set("ca", Object::Real(alpha));
    gs_dict.set("CA", Object::Real(alpha));
    let gs_id = doc.add_object(gs_dict);

    let (r, g, b) = parse_color(spec.color.as_deref().unwrap_or("#1a1a1a"));
    let font_size = spec.size.unwrap_or(36.0).clamp(8.0, 120.0);
    let base_font = match (
        spec.bold,
        spec.italic,
        spec.font.as_deref().unwrap_or("Helvetica"),
    ) {
        (true, true, _) => "Helvetica-BoldOblique",
        (true, false, _) => "Helvetica-Bold",
        (false, true, _) => "Helvetica-Oblique",
        _ => "Helvetica",
    };

    let mut font_dict = Dictionary::new();
    font_dict.set("Type", "Font");
    font_dict.set("Subtype", "Type1");
    font_dict.set("BaseFont", base_font);
    let font_id = doc.add_object(font_dict);

    let image_xobj = if mode == "image" {
        let img_path = spec
            .image_path
            .as_deref()
            .ok_or_else(|| AppError::InvalidInput("Falta imagen de marca de agua".into()))?;
        let p = ensure_image_path(img_path)?;
        let bytes = std::fs::read(&p)?;
        Some(embed_image(&mut doc, &bytes)?)
    } else {
        None
    };

    let text = spec.text.clone().unwrap_or_else(|| "CONFIDENCIAL".into());
    let escaped = escape_pdf_string(&text);

    let mut applied = 0u32;
    for (page_num, page_id) in &pages {
        if *page_num < from || *page_num > to {
            continue;
        }
        let (pw, ph) = page_size(&doc, *page_id).unwrap_or((595.0, 842.0));
        let content_ops = if mode == "text" {
            build_text_ops(
                &escaped,
                font_size,
                r,
                g,
                b,
                pw,
                ph,
                position,
                mosaic,
                rotation,
                spec.underline,
            )
        } else {
            let (iw, ih, img_name) = image_xobj.as_ref().unwrap();
            build_image_ops(*iw, *ih, img_name, pw, ph, position, mosaic, rotation)
        };

        let mut ops = vec![
            Operation::new("q", vec![]),
            Operation::new("gs", vec!["WmGS".into()]),
        ];
        ops.extend(content_ops);
        ops.push(Operation::new("Q", vec![]));

        let content = Content { operations: ops };
        let content_data = content
            .encode()
            .map_err(|e| AppError::Pdf(e.to_string()))?;
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content_data));

        attach_watermark(
            &mut doc,
            *page_id,
            content_id,
            font_id,
            gs_id,
            image_xobj.as_ref().map(|(_, _, n)| n.as_str()),
            image_xobj.as_ref().map(|(id, _, _)| *id),
            below,
        )?;
        applied += 1;
    }

    doc.compress();
    doc.save(output_path)?;

    Ok(OpResult::new(
        vec![output],
        applied,
        started.elapsed().as_millis() as u64,
    ))
}

fn build_text_ops(
    text: &str,
    size: f32,
    r: f32,
    g: f32,
    b: f32,
    pw: f32,
    ph: f32,
    position: u32,
    mosaic: bool,
    rotation: f32,
    underline: bool,
) -> Vec<Operation> {
    let approx_w = text.len() as f32 * size * 0.5;
    let approx_h = size;
    let positions = if mosaic {
        mosaic_positions(pw, ph, approx_w.max(80.0), approx_h.max(24.0))
    } else {
        vec![anchor_point(position, pw, ph, approx_w, approx_h)]
    };

    let mut ops = Vec::new();
    for (x, y) in positions {
        ops.push(Operation::new("q", vec![]));
        let rad = rotation.to_radians();
        let (c, s) = (rad.cos(), rad.sin());
        // Rotate around center of text box
        let cx = x + approx_w / 2.0;
        let cy = y + approx_h / 2.0;
        ops.push(Operation::new(
            "cm",
            vec![
                Object::Real(c),
                Object::Real(s),
                Object::Real(-s),
                Object::Real(c),
                Object::Real(cx),
                Object::Real(cy),
            ],
        ));
        ops.push(Operation::new(
            "cm",
            vec![
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(1.0),
                Object::Real(-approx_w / 2.0),
                Object::Real(-approx_h / 2.0),
            ],
        ));
        ops.push(Operation::new("BT", vec![]));
        ops.push(Operation::new(
            "rg",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ));
        ops.push(Operation::new(
            "Tf",
            vec!["WmF".into(), Object::Real(size)],
        ));
        ops.push(Operation::new(
            "Td",
            vec![Object::Real(0.0), Object::Real(0.0)],
        ));
        ops.push(Operation::new(
            "Tj",
            vec![Object::string_literal(text)],
        ));
        ops.push(Operation::new("ET", vec![]));
        if underline {
            ops.push(Operation::new(
                "rg",
                vec![Object::Real(r), Object::Real(g), Object::Real(b)],
            ));
            ops.push(Operation::new(
                "re",
                vec![
                    Object::Real(0.0),
                    Object::Real(-2.0),
                    Object::Real(approx_w),
                    Object::Real(1.0),
                ],
            ));
            ops.push(Operation::new("f", vec![]));
        }
        ops.push(Operation::new("Q", vec![]));
    }
    ops
}

fn build_image_ops(
    image_id: ObjectId,
    _ih: u32,
    name: &str,
    pw: f32,
    ph: f32,
    position: u32,
    mosaic: bool,
    rotation: f32,
) -> Vec<Operation> {
    let _ = image_id;
    let draw_w = (pw * 0.28).clamp(60.0, 220.0);
    let draw_h = draw_w * 0.6;
    let positions = if mosaic {
        mosaic_positions(pw, ph, draw_w, draw_h)
    } else {
        vec![anchor_point(position, pw, ph, draw_w, draw_h)]
    };

    let mut ops = Vec::new();
    for (x, y) in positions {
        ops.push(Operation::new("q", vec![]));
        let rad = rotation.to_radians();
        let (c, s) = (rad.cos(), rad.sin());
        let cx = x + draw_w / 2.0;
        let cy = y + draw_h / 2.0;
        ops.push(Operation::new(
            "cm",
            vec![
                Object::Real(c),
                Object::Real(s),
                Object::Real(-s),
                Object::Real(c),
                Object::Real(cx),
                Object::Real(cy),
            ],
        ));
        ops.push(Operation::new(
            "cm",
            vec![
                Object::Real(draw_w),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(draw_h),
                Object::Real(-draw_w / 2.0),
                Object::Real(-draw_h / 2.0),
            ],
        ));
        ops.push(Operation::new("Do", vec![name.into()]));
        ops.push(Operation::new("Q", vec![]));
    }
    ops
}

fn mosaic_positions(pw: f32, ph: f32, cell_w: f32, cell_h: f32) -> Vec<(f32, f32)> {
    let pad_x = cell_w * 0.4;
    let pad_y = cell_h * 0.6;
    let step_x = cell_w + pad_x;
    let step_y = cell_h + pad_y;
    let mut out = Vec::new();
    let mut y = pad_y * 0.5;
    while y + cell_h < ph {
        let mut x = pad_x * 0.5;
        while x + cell_w < pw {
            out.push((x, y));
            x += step_x;
        }
        y += step_y;
    }
    if out.is_empty() {
        out.push(anchor_point(4, pw, ph, cell_w, cell_h));
    }
    out
}

fn anchor_point(position: u32, pw: f32, ph: f32, w: f32, h: f32) -> (f32, f32) {
    let margin = 36.0;
    let col = position % 3;
    let row = position / 3; // 0 top, 1 mid, 2 bottom in UI (top-left origin conceptually)
    // PDF y grows up: row 0 = top
    let x = match col {
        0 => margin,
        1 => (pw - w) / 2.0,
        _ => pw - w - margin,
    };
    let y = match row {
        0 => ph - h - margin,
        1 => (ph - h) / 2.0,
        _ => margin,
    };
    (x.max(0.0), y.max(0.0))
}

fn embed_image(doc: &mut Document, bytes: &[u8]) -> Result<(ObjectId, u32, String), AppError> {
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| AppError::Image(e.to_string()))?
        .decode()
        .map_err(|e| AppError::Image(e.to_string()))?;
    let rgba = img.to_rgba8();
    let (iw, ih) = (rgba.width(), rgba.height());
    let mut rgb = Vec::with_capacity((iw * ih * 3) as usize);
    let mut alpha = Vec::with_capacity((iw * ih) as usize);
    for px in rgba.pixels() {
        rgb.extend_from_slice(&[px[0], px[1], px[2]]);
        alpha.push(px[3]);
    }
    let has_alpha = alpha.iter().any(|&a| a < 255);

    let mut smask_id = None;
    if has_alpha {
        let mut smask_dict = Dictionary::new();
        smask_dict.set("Type", "XObject");
        smask_dict.set("Subtype", "Image");
        smask_dict.set("Width", iw as i64);
        smask_dict.set("Height", ih as i64);
        smask_dict.set("ColorSpace", "DeviceGray");
        smask_dict.set("BitsPerComponent", 8);
        smask_dict.set("Filter", "FlateDecode");
        smask_id = Some(doc.add_object(Object::Stream(Stream::new(
            smask_dict,
            compress_flate(&alpha)?,
        ))));
    }

    let mut img_dict = Dictionary::new();
    img_dict.set("Type", "XObject");
    img_dict.set("Subtype", "Image");
    img_dict.set("Width", iw as i64);
    img_dict.set("Height", ih as i64);
    img_dict.set("ColorSpace", "DeviceRGB");
    img_dict.set("BitsPerComponent", 8);
    img_dict.set("Filter", "FlateDecode");
    if let Some(sid) = smask_id {
        img_dict.set("SMask", Object::Reference(sid));
    }
    let image_id = doc.add_object(Object::Stream(Stream::new(
        img_dict,
        compress_flate(&rgb)?,
    )));
    Ok((image_id, ih, "WmIm".into()))
}

fn attach_watermark(
    doc: &mut Document,
    page_id: ObjectId,
    content_id: ObjectId,
    font_id: ObjectId,
    gs_id: ObjectId,
    image_name: Option<&str>,
    image_id: Option<ObjectId>,
    below: bool,
) -> Result<(), AppError> {
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

    let mut fonts = match resources.get(b"Font") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc
            .get_object(*id)
            .and_then(|o| o.as_dict())
            .map(|d| d.clone())
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };
    fonts.set("WmF", font_id);
    resources.set("Font", fonts);

    let mut ext_g = match resources.get(b"ExtGState") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc
            .get_object(*id)
            .and_then(|o| o.as_dict())
            .map(|d| d.clone())
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };
    ext_g.set("WmGS", gs_id);
    resources.set("ExtGState", ext_g);

    if let (Some(name), Some(iid)) = (image_name, image_id) {
        let mut xobjects = match resources.get(b"XObject") {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(id)) => doc
                .get_object(*id)
                .and_then(|o| o.as_dict())
                .map(|d| d.clone())
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        xobjects.set(name, iid);
        resources.set("XObject", xobjects);
    }

    page.set("Resources", resources);

    match page.get(b"Contents").ok().cloned() {
        Some(Object::Reference(existing)) => {
            if below {
                page.set(
                    "Contents",
                    Object::Array(vec![
                        Object::Reference(content_id),
                        Object::Reference(existing),
                    ]),
                );
            } else {
                page.set(
                    "Contents",
                    Object::Array(vec![
                        Object::Reference(existing),
                        Object::Reference(content_id),
                    ]),
                );
            }
        }
        Some(Object::Array(mut arr)) => {
            if below {
                arr.insert(0, Object::Reference(content_id));
            } else {
                arr.push(Object::Reference(content_id));
            }
            page.set("Contents", Object::Array(arr));
        }
        _ => {
            page.set("Contents", content_id);
        }
    }

    doc.objects.insert(page_id, Object::Dictionary(page));
    Ok(())
}

fn page_size(doc: &Document, page_id: ObjectId) -> Option<(f32, f32)> {
    let dict = doc.get_object(page_id).ok()?.as_dict().ok()?;
    let box_obj = dict
        .get(b"MediaBox")
        .or_else(|_| dict.get(b"CropBox"))
        .ok()?;
    let arr = box_obj.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    let x0 = to_f32(&arr[0])?;
    let y0 = to_f32(&arr[1])?;
    let x1 = to_f32(&arr[2])?;
    let y1 = to_f32(&arr[3])?;
    Some((x1 - x0, y1 - y0))
}

fn to_f32(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r as f32),
        _ => None,
    }
}

fn parse_color(hex: &str) -> (f32, f32, f32) {
    let h = hex.trim().trim_start_matches('#');
    if h.len() >= 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(26) as f32 / 255.0;
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(26) as f32 / 255.0;
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(26) as f32 / 255.0;
        (r, g, b)
    } else {
        (0.1, 0.1, 0.1)
    }
}

fn escape_pdf_string(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '(' | ')' | '\\' => format!("\\{c}"),
            c if c.is_ascii() => c.to_string(),
            _ => "?".into(),
        })
        .collect()
}

fn compress_flate(data: &[u8]) -> Result<Vec<u8>, AppError> {
    use std::io::Write;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    enc.write_all(data)
        .map_err(|e| AppError::Pdf(format!("flate: {e}")))?;
    enc.finish()
        .map_err(|e| AppError::Pdf(format!("flate finish: {e}")))
}
