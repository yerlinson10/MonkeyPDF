use super::util::{
    append_page_content, base14_font, compress_flate, ensure_ext_gstate, ensure_type1_font,
    escape_pdf_string, parse_color, text_width,
};
use crate::error::AppError;
use crate::pdf_engine::ensure_image_path;
use image::ImageReader;
use lopdf::content::Operation;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::io::Cursor;

pub fn bake_text(
    doc: &mut Document,
    page_id: ObjectId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    text: &str,
    font_family: &str,
    size: f32,
    bold: bool,
    italic: bool,
    color: &str,
    align: &str,
    opacity: f32,
) -> Result<(), AppError> {
    let base = base14_font(font_family, bold, italic);
    let font_id = ensure_type1_font(doc, base);
    let font_key = "EdF";
    let size = size.clamp(4.0, 200.0);
    let (r, g, b) = parse_color(color);
    let alpha = opacity.clamp(0.05, 1.0);

    let mut gs_name = None;
    let mut gs_id = None;
    if alpha < 0.999 {
        gs_id = Some(ensure_ext_gstate(doc, alpha));
        gs_name = Some("EdGS");
    }

    let lines = wrap_text(text, w.max(8.0), size);
    let line_height = size * 1.25;
    let total_h = lines.len() as f32 * line_height;
    // y is baseline of first line near bottom of box if h provided
    let start_y = if h > 0.0 {
        y + h - size
    } else {
        y
    };
    // Clamp so we don't go below box bottom
    let start_y = if h > 0.0 && total_h > h {
        y + h - size
    } else {
        start_y
    };

    let mut ops = vec![Operation::new("q", vec![])];
    if gs_name.is_some() {
        ops.push(Operation::new("gs", vec![Object::Name(b"EdGS".to_vec())]));
    }
    ops.push(Operation::new(
        "rg",
        vec![Object::Real(r), Object::Real(g), Object::Real(b)],
    ));
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new(
        "Tf",
        vec![Object::Name(font_key.as_bytes().to_vec()), Object::Real(size)],
    ));

    for (i, line) in lines.iter().enumerate() {
        let lw = text_width(line, size);
        let lx = match align {
            "center" => x + (w - lw) * 0.5,
            "right" => x + w - lw,
            _ => x,
        };
        let ly = start_y - i as f32 * line_height;
        if ly < y - size {
            break;
        }
        let escaped = escape_pdf_string(line);
        ops.push(Operation::new(
            "Tm",
            vec![
                1.0.into(),
                0.0.into(),
                0.0.into(),
                1.0.into(),
                Object::Real(lx),
                Object::Real(ly),
            ],
        ));
        ops.push(Operation::new(
            "Tj",
            vec![Object::string_literal(escaped.as_str())],
        ));
    }
    ops.push(Operation::new("ET", vec![]));

    // Underline for bold+italic combo is not needed; optional underline via separate flag
    // kept simple here.

    ops.push(Operation::new("Q", vec![]));

    append_page_content(
        doc,
        page_id,
        ops,
        Some(font_key),
        Some(font_id),
        gs_name,
        gs_id,
        None,
        None,
    )
}

pub fn bake_rect(
    doc: &mut Document,
    page_id: ObjectId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    stroke: &str,
    fill: Option<&str>,
    stroke_width: f32,
    opacity: f32,
) -> Result<(), AppError> {
    let (sr, sg, sb) = parse_color(stroke);
    let alpha = opacity.clamp(0.05, 1.0);
    let mut gs_id = None;
    let mut gs_name = None;
    if alpha < 0.999 {
        gs_id = Some(ensure_ext_gstate(doc, alpha));
        gs_name = Some("EdGS");
    }

    let mut ops = vec![Operation::new("q", vec![])];
    if gs_name.is_some() {
        ops.push(Operation::new("gs", vec![Object::Name(b"EdGS".to_vec())]));
    }
    ops.push(Operation::new(
        "w",
        vec![Object::Real(stroke_width.clamp(0.25, 40.0))],
    ));
    ops.push(Operation::new(
        "re",
        vec![
            Object::Real(x),
            Object::Real(y),
            Object::Real(w),
            Object::Real(h),
        ],
    ));
    if let Some(fill_hex) = fill {
        let (fr, fg, fb) = parse_color(fill_hex);
        ops.push(Operation::new(
            "rg",
            vec![Object::Real(fr), Object::Real(fg), Object::Real(fb)],
        ));
        ops.push(Operation::new(
            "RG",
            vec![Object::Real(sr), Object::Real(sg), Object::Real(sb)],
        ));
        ops.push(Operation::new("B", vec![])); // fill and stroke
    } else {
        ops.push(Operation::new(
            "RG",
            vec![Object::Real(sr), Object::Real(sg), Object::Real(sb)],
        ));
        ops.push(Operation::new("S", vec![]));
    }
    ops.push(Operation::new("Q", vec![]));

    append_page_content(doc, page_id, ops, None, None, gs_name, gs_id, None, None)
}

pub fn bake_ellipse(
    doc: &mut Document,
    page_id: ObjectId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    stroke: &str,
    fill: Option<&str>,
    stroke_width: f32,
    opacity: f32,
) -> Result<(), AppError> {
    // Bezier circle approximation in bounding box
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let rx = w / 2.0;
    let ry = h / 2.0;
    let k = 0.5522847498;
    let ox = rx * k;
    let oy = ry * k;

    let (sr, sg, sb) = parse_color(stroke);
    let alpha = opacity.clamp(0.05, 1.0);
    let mut gs_id = None;
    let mut gs_name = None;
    if alpha < 0.999 {
        gs_id = Some(ensure_ext_gstate(doc, alpha));
        gs_name = Some("EdGS");
    }

    let mut ops = vec![Operation::new("q", vec![])];
    if gs_name.is_some() {
        ops.push(Operation::new("gs", vec![Object::Name(b"EdGS".to_vec())]));
    }
    ops.push(Operation::new(
        "w",
        vec![Object::Real(stroke_width.clamp(0.25, 40.0))],
    ));
    // path
    ops.push(Operation::new(
        "m",
        vec![Object::Real(cx + rx), Object::Real(cy)],
    ));
    ops.push(Operation::new(
        "c",
        vec![
            Object::Real(cx + rx),
            Object::Real(cy + oy),
            Object::Real(cx + ox),
            Object::Real(cy + ry),
            Object::Real(cx),
            Object::Real(cy + ry),
        ],
    ));
    ops.push(Operation::new(
        "c",
        vec![
            Object::Real(cx - ox),
            Object::Real(cy + ry),
            Object::Real(cx - rx),
            Object::Real(cy + oy),
            Object::Real(cx - rx),
            Object::Real(cy),
        ],
    ));
    ops.push(Operation::new(
        "c",
        vec![
            Object::Real(cx - rx),
            Object::Real(cy - oy),
            Object::Real(cx - ox),
            Object::Real(cy - ry),
            Object::Real(cx),
            Object::Real(cy - ry),
        ],
    ));
    ops.push(Operation::new(
        "c",
        vec![
            Object::Real(cx + ox),
            Object::Real(cy - ry),
            Object::Real(cx + rx),
            Object::Real(cy - oy),
            Object::Real(cx + rx),
            Object::Real(cy),
        ],
    ));
    ops.push(Operation::new("h", vec![]));

    if let Some(fill_hex) = fill {
        let (fr, fg, fb) = parse_color(fill_hex);
        ops.push(Operation::new(
            "rg",
            vec![Object::Real(fr), Object::Real(fg), Object::Real(fb)],
        ));
        ops.push(Operation::new(
            "RG",
            vec![Object::Real(sr), Object::Real(sg), Object::Real(sb)],
        ));
        ops.push(Operation::new("B", vec![]));
    } else {
        ops.push(Operation::new(
            "RG",
            vec![Object::Real(sr), Object::Real(sg), Object::Real(sb)],
        ));
        ops.push(Operation::new("S", vec![]));
    }
    ops.push(Operation::new("Q", vec![]));

    append_page_content(doc, page_id, ops, None, None, gs_name, gs_id, None, None)
}

pub fn bake_line(
    doc: &mut Document,
    page_id: ObjectId,
    from: (f32, f32),
    to: (f32, f32),
    color: &str,
    width: f32,
    arrow: &str,
) -> Result<(), AppError> {
    let (r, g, b) = parse_color(color);
    let mut ops = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "RG",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new("rg", vec![Object::Real(r), Object::Real(g), Object::Real(b)]),
        Operation::new("w", vec![Object::Real(width.clamp(0.25, 40.0))]),
        Operation::new(
            "m",
            vec![Object::Real(from.0), Object::Real(from.1)],
        ),
        Operation::new("l", vec![Object::Real(to.0), Object::Real(to.1)]),
        Operation::new("S", vec![]),
    ];

    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let ux = dx / len;
    let uy = dy / len;
    let arrow_len = (width * 4.0).clamp(6.0, 18.0);
    let px = -uy;
    let py = ux;

    let draw_arrow_at = |ops: &mut Vec<Operation>, tip: (f32, f32), dir: (f32, f32)| {
        let base = (tip.0 - dir.0 * arrow_len, tip.1 - dir.1 * arrow_len);
        let left = (base.0 + px * arrow_len * 0.45, base.1 + py * arrow_len * 0.45);
        let right = (base.0 - px * arrow_len * 0.45, base.1 - py * arrow_len * 0.45);
        ops.push(Operation::new(
            "m",
            vec![Object::Real(tip.0), Object::Real(tip.1)],
        ));
        ops.push(Operation::new(
            "l",
            vec![Object::Real(left.0), Object::Real(left.1)],
        ));
        ops.push(Operation::new(
            "l",
            vec![Object::Real(right.0), Object::Real(right.1)],
        ));
        ops.push(Operation::new("h", vec![]));
        ops.push(Operation::new("f", vec![]));
    };

    match arrow {
        "end" | "to" => draw_arrow_at(&mut ops, to, (ux, uy)),
        "start" | "from" => draw_arrow_at(&mut ops, from, (-ux, -uy)),
        "both" => {
            draw_arrow_at(&mut ops, to, (ux, uy));
            draw_arrow_at(&mut ops, from, (-ux, -uy));
        }
        _ => {}
    }

    ops.push(Operation::new("Q", vec![]));
    append_page_content(doc, page_id, ops, None, None, None, None, None, None)
}

pub fn bake_free_draw(
    doc: &mut Document,
    page_id: ObjectId,
    paths: &[Vec<(f32, f32)>],
    color: &str,
    width: f32,
    opacity: f32,
) -> Result<(), AppError> {
    let (r, g, b) = parse_color(color);
    let alpha = opacity.clamp(0.05, 1.0);
    let mut gs_id = None;
    let mut gs_name = None;
    if alpha < 0.999 {
        gs_id = Some(ensure_ext_gstate(doc, alpha));
        gs_name = Some("EdGS");
    }

    let mut ops = vec![Operation::new("q", vec![])];
    if gs_name.is_some() {
        ops.push(Operation::new("gs", vec![Object::Name(b"EdGS".to_vec())]));
    }
    ops.push(Operation::new(
        "RG",
        vec![Object::Real(r), Object::Real(g), Object::Real(b)],
    ));
    ops.push(Operation::new(
        "w",
        vec![Object::Real(width.clamp(0.25, 40.0))],
    ));
    ops.push(Operation::new("J", vec![Object::Integer(1)])); // round caps
    ops.push(Operation::new("j", vec![Object::Integer(1)]));

    for path in paths {
        if path.is_empty() {
            continue;
        }
        ops.push(Operation::new(
            "m",
            vec![Object::Real(path[0].0), Object::Real(path[0].1)],
        ));
        for p in path.iter().skip(1) {
            ops.push(Operation::new(
                "l",
                vec![Object::Real(p.0), Object::Real(p.1)],
            ));
        }
        ops.push(Operation::new("S", vec![]));
    }
    ops.push(Operation::new("Q", vec![]));

    append_page_content(doc, page_id, ops, None, None, gs_name, gs_id, None, None)
}

pub fn bake_whiteout(
    doc: &mut Document,
    page_id: ObjectId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: Option<&str>,
) -> Result<(), AppError> {
    let (r, g, b) = parse_color(color.unwrap_or("#ffffff"));
    let ops = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "rg",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new(
            "re",
            vec![
                Object::Real(x),
                Object::Real(y),
                Object::Real(w),
                Object::Real(h),
            ],
        ),
        Operation::new("f", vec![]),
        Operation::new("Q", vec![]),
    ];
    append_page_content(doc, page_id, ops, None, None, None, None, None, None)
}

pub fn bake_image(
    doc: &mut Document,
    page_id: ObjectId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    image_path: &str,
    rotation_deg: f32,
    opacity: f32,
) -> Result<(), AppError> {
    let path = ensure_image_path(image_path)?;
    let (image_id, _iw, _ih) = embed_image(doc, &path)?;
    let name = format!("EdIm{}", image_id.0);

    let alpha = opacity.clamp(0.05, 1.0);
    let mut gs_id = None;
    let mut gs_name = None;
    if alpha < 0.999 {
        gs_id = Some(ensure_ext_gstate(doc, alpha));
        gs_name = Some("EdGS");
    }

    let rad = rotation_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    // Rotate around center of box
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;

    let mut ops = vec![Operation::new("q", vec![])];
    if gs_name.is_some() {
        ops.push(Operation::new("gs", vec![Object::Name(b"EdGS".to_vec())]));
    }
    // T = T(cx,cy) * R * T(-w/2,-h/2) * Scale(w,h)
    // Final cm for image unit square → page
    let a = cos * w;
    let b = sin * w;
    let c = -sin * h;
    let d = cos * h;
    let e = cx - cos * w / 2.0 + sin * h / 2.0;
    let f = cy - sin * w / 2.0 - cos * h / 2.0;
    ops.push(Operation::new(
        "cm",
        vec![
            Object::Real(a),
            Object::Real(b),
            Object::Real(c),
            Object::Real(d),
            Object::Real(e),
            Object::Real(f),
        ],
    ));
    ops.push(Operation::new(
        "Do",
        vec![Object::Name(name.as_bytes().to_vec())],
    ));
    ops.push(Operation::new("Q", vec![]));

    append_page_content(
        doc,
        page_id,
        ops,
        None,
        None,
        gs_name,
        gs_id,
        Some(&name),
        Some(image_id),
    )
}

pub fn bake_stamp(
    doc: &mut Document,
    page_id: ObjectId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    stamp: &str,
    custom_text: Option<&str>,
    color: &str,
) -> Result<(), AppError> {
    let label = custom_text
        .map(|s| s.to_string())
        .unwrap_or_else(|| stamp_label(stamp));
    let (r, g, b) = parse_color(color);
    let font_id = ensure_type1_font(doc, "Helvetica-Bold");
    let font_key = "EdStF";

    // Fit font size to box width
    let mut size = (h * 0.35).clamp(10.0, 48.0);
    while text_width(&label, size) > w * 0.85 && size > 8.0 {
        size -= 1.0;
    }

    let rad = (-12.0f32).to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;

    let mut ops = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "RG",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new(
            "rg",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new("w", vec![Object::Real(2.5)]),
    ];

    // Double rounded-ish rect (as rects) rotated via cm
    ops.push(Operation::new(
        "cm",
        vec![
            Object::Real(cos),
            Object::Real(sin),
            Object::Real(-sin),
            Object::Real(cos),
            Object::Real(cx),
            Object::Real(cy),
        ],
    ));
    let rw = w * 0.92;
    let rh = h * 0.7;
    ops.push(Operation::new(
        "re",
        vec![
            Object::Real(-rw / 2.0),
            Object::Real(-rh / 2.0),
            Object::Real(rw),
            Object::Real(rh),
        ],
    ));
    ops.push(Operation::new("S", vec![]));
    ops.push(Operation::new("w", vec![Object::Real(1.0)]));
    ops.push(Operation::new(
        "re",
        vec![
            Object::Real(-rw / 2.0 + 3.0),
            Object::Real(-rh / 2.0 + 3.0),
            Object::Real(rw - 6.0),
            Object::Real(rh - 6.0),
        ],
    ));
    ops.push(Operation::new("S", vec![]));

    let tw = text_width(&label, size);
    let escaped = escape_pdf_string(&label);
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new(
        "Tf",
        vec![
            Object::Name(font_key.as_bytes().to_vec()),
            Object::Real(size),
        ],
    ));
    ops.push(Operation::new(
        "Td",
        vec![Object::Real(-tw / 2.0), Object::Real(-size * 0.35)],
    ));
    ops.push(Operation::new(
        "Tj",
        vec![Object::string_literal(escaped.as_str())],
    ));
    ops.push(Operation::new("ET", vec![]));
    ops.push(Operation::new("Q", vec![]));

    append_page_content(
        doc,
        page_id,
        ops,
        Some(font_key),
        Some(font_id),
        None,
        None,
        None,
        None,
    )
}

pub fn stamp_label(stamp: &str) -> String {
    match stamp.to_ascii_lowercase().as_str() {
        "approved" | "aprobado" => "APROBADO".into(),
        "rejected" | "rechazado" => "RECHAZADO".into(),
        "confidential" | "confidencial" => "CONFIDENCIAL".into(),
        "draft" | "borrador" => "BORRADOR".into(),
        "signed" | "firmado" => "FIRMADO".into(),
        "urgent" | "urgente" => "URGENTE".into(),
        "copy" | "copia" => "COPIA".into(),
        "original" => "ORIGINAL".into(),
        other => other.to_ascii_uppercase(),
    }
}

fn wrap_text(text: &str, max_w: f32, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if text_width(&candidate, size) <= max_w || current.is_empty() {
                current = candidate;
            } else {
                lines.push(current);
                current = word.to_string();
            }
        }
        if !current.is_empty() || paragraph.ends_with(' ') {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn embed_image(doc: &mut Document, path: &std::path::Path) -> Result<(ObjectId, u32, u32), AppError> {
    let data = std::fs::read(path).map_err(|e| AppError::Io(e))?;
    let img = ImageReader::new(Cursor::new(data))
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
    Ok((image_id, iw, ih))
}
