use super::draw;
use super::util::{
    append_page_content, compress_flate, ensure_ext_gstate, ensure_type1_font, escape_pdf_string,
    parse_color, text_width,
};
use crate::error::AppError;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

pub fn add_highlight(
    doc: &mut Document,
    page_id: ObjectId,
    quads: &[(f32, f32, f32, f32)],
    color: &str,
    opacity: f32,
    flatten: bool,
) -> Result<(), AppError> {
    if flatten {
        return bake_highlight(doc, page_id, quads, color, opacity);
    }
    let (r, g, b) = parse_color(color);
    let rect = union_quads(quads);
    let mut annot = Dictionary::new();
    annot.set("Type", "Annot");
    annot.set("Subtype", "Highlight");
    annot.set(
        "Rect",
        vec![
            Object::Real(rect.0),
            Object::Real(rect.1),
            Object::Real(rect.2),
            Object::Real(rect.3),
        ],
    );
    annot.set(
        "C",
        vec![Object::Real(r), Object::Real(g), Object::Real(b)],
    );
    annot.set("CA", Object::Real(opacity.clamp(0.1, 1.0)));
    annot.set("F", Object::Integer(4)); // print
    let mut qp = Vec::new();
    for &(x, y, w, h) in quads {
        // QuadPoints: 8 numbers per quad — UL UR LR LL
        qp.extend([
            Object::Real(x),
            Object::Real(y + h),
            Object::Real(x + w),
            Object::Real(y + h),
            Object::Real(x + w),
            Object::Real(y),
            Object::Real(x),
            Object::Real(y),
        ]);
    }
    annot.set("QuadPoints", qp);
    let ap = highlight_ap(doc, quads, r, g, b, opacity)?;
    let mut ap_dict = Dictionary::new();
    ap_dict.set("N", Object::Reference(ap));
    annot.set("AP", ap_dict);
    attach_annot(doc, page_id, annot)
}

pub fn add_underline(
    doc: &mut Document,
    page_id: ObjectId,
    quads: &[(f32, f32, f32, f32)],
    color: &str,
    flatten: bool,
) -> Result<(), AppError> {
    if flatten {
        return bake_line_markup(doc, page_id, quads, color, false);
    }
    add_text_markup(doc, page_id, quads, color, "Underline")
}

pub fn add_strikeout(
    doc: &mut Document,
    page_id: ObjectId,
    quads: &[(f32, f32, f32, f32)],
    color: &str,
    flatten: bool,
) -> Result<(), AppError> {
    if flatten {
        return bake_line_markup(doc, page_id, quads, color, true);
    }
    add_text_markup(doc, page_id, quads, color, "StrikeOut")
}

fn add_text_markup(
    doc: &mut Document,
    page_id: ObjectId,
    quads: &[(f32, f32, f32, f32)],
    color: &str,
    subtype: &str,
) -> Result<(), AppError> {
    let (r, g, b) = parse_color(color);
    let rect = union_quads(quads);
    let mut annot = Dictionary::new();
    annot.set("Type", "Annot");
    annot.set("Subtype", Object::Name(subtype.as_bytes().to_vec()));
    annot.set(
        "Rect",
        vec![
            Object::Real(rect.0),
            Object::Real(rect.1),
            Object::Real(rect.2),
            Object::Real(rect.3),
        ],
    );
    annot.set(
        "C",
        vec![Object::Real(r), Object::Real(g), Object::Real(b)],
    );
    annot.set("F", Object::Integer(4));
    let mut qp = Vec::new();
    for &(x, y, w, h) in quads {
        qp.extend([
            Object::Real(x),
            Object::Real(y + h),
            Object::Real(x + w),
            Object::Real(y + h),
            Object::Real(x + w),
            Object::Real(y),
            Object::Real(x),
            Object::Real(y),
        ]);
    }
    annot.set("QuadPoints", qp);
    let ap = line_markup_ap(doc, quads, r, g, b, subtype == "StrikeOut")?;
    let mut ap_dict = Dictionary::new();
    ap_dict.set("N", Object::Reference(ap));
    annot.set("AP", ap_dict);
    attach_annot(doc, page_id, annot)
}

pub fn add_note(
    doc: &mut Document,
    page_id: ObjectId,
    x: f32,
    y: f32,
    text: &str,
    color: &str,
    flatten: bool,
) -> Result<(), AppError> {
    if flatten {
        // Bake as small FreeText-like box
        let w = 120.0;
        let h = 40.0;
        draw::bake_rect(
            doc,
            page_id,
            x,
            y,
            w,
            h,
            color,
            Some("#fff8c5"),
            1.0,
            1.0,
        )?;
        return draw::bake_text(
            doc,
            page_id,
            x + 4.0,
            y + 4.0,
            w - 8.0,
            h - 8.0,
            text,
            "Helvetica",
            9.0,
            false,
            false,
            "#1a1a1a",
            "left",
            1.0,
        );
    }

    let (r, g, b) = parse_color(color);
    let size = 18.0;
    let mut annot = Dictionary::new();
    annot.set("Type", "Annot");
    annot.set("Subtype", "Text");
    annot.set(
        "Rect",
        vec![
            Object::Real(x),
            Object::Real(y),
            Object::Real(x + size),
            Object::Real(y + size),
        ],
    );
    annot.set(
        "C",
        vec![Object::Real(r), Object::Real(g), Object::Real(b)],
    );
    annot.set("Contents", Object::string_literal(text));
    annot.set("Name", Object::Name(b"Comment".to_vec()));
    annot.set("F", Object::Integer(4));
    let ap = note_ap(doc, size, r, g, b)?;
    let mut ap_dict = Dictionary::new();
    ap_dict.set("N", Object::Reference(ap));
    annot.set("AP", ap_dict);
    attach_annot(doc, page_id, annot)
}

pub fn add_square_annot(
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
    flatten: bool,
) -> Result<(), AppError> {
    if flatten {
        return draw::bake_rect(doc, page_id, x, y, w, h, stroke, fill, stroke_width, opacity);
    }
    shape_annot(doc, page_id, "Square", x, y, w, h, stroke, fill, stroke_width, opacity)
}

pub fn add_ellipse_annot(
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
    flatten: bool,
) -> Result<(), AppError> {
    if flatten {
        return draw::bake_ellipse(doc, page_id, x, y, w, h, stroke, fill, stroke_width, opacity);
    }
    shape_annot(doc, page_id, "Circle", x, y, w, h, stroke, fill, stroke_width, opacity)
}

pub fn add_line_annot(
    doc: &mut Document,
    page_id: ObjectId,
    from: (f32, f32),
    to: (f32, f32),
    color: &str,
    width: f32,
    arrow: &str,
    flatten: bool,
) -> Result<(), AppError> {
    if flatten {
        return draw::bake_line(doc, page_id, from, to, color, width, arrow);
    }
    let (r, g, b) = parse_color(color);
    let min_x = from.0.min(to.0) - 4.0;
    let min_y = from.1.min(to.1) - 4.0;
    let max_x = from.0.max(to.0) + 4.0;
    let max_y = from.1.max(to.1) + 4.0;
    let mut annot = Dictionary::new();
    annot.set("Type", "Annot");
    annot.set("Subtype", "Line");
    annot.set(
        "Rect",
        vec![
            Object::Real(min_x),
            Object::Real(min_y),
            Object::Real(max_x),
            Object::Real(max_y),
        ],
    );
    annot.set(
        "L",
        vec![
            Object::Real(from.0),
            Object::Real(from.1),
            Object::Real(to.0),
            Object::Real(to.1),
        ],
    );
    annot.set(
        "C",
        vec![Object::Real(r), Object::Real(g), Object::Real(b)],
    );
    annot.set("BS", {
        let mut bs = Dictionary::new();
        bs.set("W", Object::Real(width));
        Object::Dictionary(bs)
    });
    let endings = match arrow {
        "end" | "to" => ("None", "ClosedArrow"),
        "start" | "from" => ("ClosedArrow", "None"),
        "both" => ("ClosedArrow", "ClosedArrow"),
        _ => ("None", "None"),
    };
    annot.set(
        "LE",
        vec![
            Object::Name(endings.0.as_bytes().to_vec()),
            Object::Name(endings.1.as_bytes().to_vec()),
        ],
    );
    annot.set("F", Object::Integer(4));
    // Simple AP: bake as form XObject covering bbox
    let ap = line_ap(doc, from, to, r, g, b, width, min_x, min_y, max_x, max_y)?;
    let mut ap_dict = Dictionary::new();
    ap_dict.set("N", Object::Reference(ap));
    annot.set("AP", ap_dict);
    attach_annot(doc, page_id, annot)
}

pub fn add_stamp_annot(
    doc: &mut Document,
    page_id: ObjectId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    stamp: &str,
    custom_text: Option<&str>,
    color: &str,
    flatten: bool,
) -> Result<(), AppError> {
    if flatten {
        return draw::bake_stamp(doc, page_id, x, y, w, h, stamp, custom_text, color);
    }
    let (r, g, b) = parse_color(color);
    let label = custom_text
        .map(|s| s.to_string())
        .unwrap_or_else(|| draw::stamp_label(stamp));
    let mut annot = Dictionary::new();
    annot.set("Type", "Annot");
    annot.set("Subtype", "Stamp");
    annot.set(
        "Rect",
        vec![
            Object::Real(x),
            Object::Real(y),
            Object::Real(x + w),
            Object::Real(y + h),
        ],
    );
    annot.set(
        "C",
        vec![Object::Real(r), Object::Real(g), Object::Real(b)],
    );
    annot.set("Contents", Object::string_literal(label.as_str()));
    annot.set("Name", Object::Name(b"Approved".to_vec()));
    annot.set("F", Object::Integer(4));
    let ap = stamp_ap(doc, w, h, &label, r, g, b)?;
    let mut ap_dict = Dictionary::new();
    ap_dict.set("N", Object::Reference(ap));
    annot.set("AP", ap_dict);
    attach_annot(doc, page_id, annot)
}

/// Flatten all annotations on all pages into content streams and remove them.
/// Flatten helpers kept for future “aplanar documento abierto” without re-baking ops.
#[allow(dead_code)]
pub fn flatten_all_annots(doc: &mut Document) -> Result<(), AppError> {
    let pages: Vec<(u32, ObjectId)> = doc.get_pages().into_iter().collect();
    for (_n, page_id) in pages {
        let annot_ids = collect_annot_ids(doc, page_id);
        for ann_id in &annot_ids {
            if let Ok(Object::Dictionary(ann)) = doc.get_object(*ann_id).map(|o| o.clone()) {
                bake_annot_from_dict(doc, page_id, &ann)?;
            }
        }
        // Clear Annots
        if let Ok(Object::Dictionary(page)) = doc.get_object_mut(page_id) {
            page.remove(b"Annots");
        }
        for id in annot_ids {
            doc.objects.remove(&id);
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn bake_annot_from_dict(
    doc: &mut Document,
    page_id: ObjectId,
    ann: &Dictionary,
) -> Result<(), AppError> {
    // Prefer drawing the appearance stream if present
    if let Ok(ap) = ann.get(b"AP") {
            if let Ok(ap_dict) = resolve_dict(doc, ap) {
            if let Ok(n_obj) = ap_dict.get(b"N") {
                if let Object::Reference(id) = n_obj {
                    if let Ok(Object::Stream(stream)) = doc.get_object(*id).map(|o| o.clone()) {
                        return paint_form_xobject(doc, page_id, ann, &stream, *id);
                    }
                }
            }
        }
    }
    // Fallback by subtype
    let subtype = ann
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .unwrap_or_default();
    let rect = read_rect(ann).unwrap_or((0.0, 0.0, 50.0, 50.0));
    let (x0, y0, x1, y1) = rect;
    match subtype.as_str() {
        "Highlight" => {
            bake_highlight(
                doc,
                page_id,
                &[(x0, y0, x1 - x0, y1 - y0)],
                "#ffe066",
                0.4,
            )?;
        }
        "Square" => {
            draw::bake_rect(doc, page_id, x0, y0, x1 - x0, y1 - y0, "#e11d48", None, 1.5, 1.0)?;
        }
        "Circle" => {
            draw::bake_ellipse(doc, page_id, x0, y0, x1 - x0, y1 - y0, "#e11d48", None, 1.5, 1.0)?;
        }
        "Stamp" | "Text" | "FreeText" => {
            let text = ann
                .get(b"Contents")
                .ok()
                .and_then(|o| o.as_str().ok())
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            if subtype == "Stamp" {
                draw::bake_stamp(doc, page_id, x0, y0, x1 - x0, y1 - y0, "approved", Some(&text), "#e11d48")?;
            } else if !text.is_empty() {
                draw::bake_text(
                    doc,
                    page_id,
                    x0,
                    y0,
                    x1 - x0,
                    y1 - y0,
                    &text,
                    "Helvetica",
                    10.0,
                    false,
                    false,
                    "#1a1a1a",
                    "left",
                    1.0,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn paint_form_xobject(
    doc: &mut Document,
    page_id: ObjectId,
    ann: &Dictionary,
    _stream: &Stream,
    form_id: ObjectId,
) -> Result<(), AppError> {
    let (x0, y0, x1, y1) = read_rect(ann).unwrap_or((0.0, 0.0, 50.0, 50.0));
    let name = format!("EdFlat{}", form_id.0);
    let ops = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(1.0),
                Object::Real(x0),
                Object::Real(y0),
            ],
        ),
        Operation::new("Do", vec![Object::Name(name.as_bytes().to_vec())]),
        Operation::new("Q", vec![]),
    ];
    // Ensure XObject registered — Form already exists; just reference it
    let _ = (x1, y1);
    append_page_content(
        doc,
        page_id,
        ops,
        None,
        None,
        None,
        None,
        Some(&name),
        Some(form_id),
    )
}

fn shape_annot(
    doc: &mut Document,
    page_id: ObjectId,
    subtype: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    stroke: &str,
    fill: Option<&str>,
    stroke_width: f32,
    opacity: f32,
) -> Result<(), AppError> {
    let (r, g, b) = parse_color(stroke);
    let mut annot = Dictionary::new();
    annot.set("Type", "Annot");
    annot.set("Subtype", Object::Name(subtype.as_bytes().to_vec()));
    annot.set(
        "Rect",
        vec![
            Object::Real(x),
            Object::Real(y),
            Object::Real(x + w),
            Object::Real(y + h),
        ],
    );
    annot.set(
        "C",
        vec![Object::Real(r), Object::Real(g), Object::Real(b)],
    );
    annot.set("CA", Object::Real(opacity.clamp(0.1, 1.0)));
    annot.set("F", Object::Integer(4));
    annot.set("BS", {
        let mut bs = Dictionary::new();
        bs.set("W", Object::Real(stroke_width));
        Object::Dictionary(bs)
    });
    if let Some(fill_hex) = fill {
        let (fr, fg, fb) = parse_color(fill_hex);
        annot.set(
            "IC",
            vec![Object::Real(fr), Object::Real(fg), Object::Real(fb)],
        );
    }
    let ap = shape_ap(doc, subtype, w, h, r, g, b, fill, stroke_width, opacity)?;
    let mut ap_dict = Dictionary::new();
    ap_dict.set("N", Object::Reference(ap));
    annot.set("AP", ap_dict);
    attach_annot(doc, page_id, annot)
}

fn bake_highlight(
    doc: &mut Document,
    page_id: ObjectId,
    quads: &[(f32, f32, f32, f32)],
    color: &str,
    opacity: f32,
) -> Result<(), AppError> {
    let (r, g, b) = parse_color(color);
    let gs_id = ensure_ext_gstate(doc, opacity.clamp(0.1, 1.0));
    // Multiply blend for highlight feel
    if let Ok(Object::Dictionary(gs)) = doc.get_object_mut(gs_id) {
        gs.set("BM", Object::Name(b"Multiply".to_vec()));
    }
    let mut ops = vec![
        Operation::new("q", vec![]),
        Operation::new("gs", vec![Object::Name(b"EdGS".to_vec())]),
        Operation::new(
            "rg",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
    ];
    for &(x, y, w, h) in quads {
        ops.push(Operation::new(
            "re",
            vec![
                Object::Real(x),
                Object::Real(y),
                Object::Real(w),
                Object::Real(h),
            ],
        ));
        ops.push(Operation::new("f", vec![]));
    }
    ops.push(Operation::new("Q", vec![]));
    append_page_content(
        doc,
        page_id,
        ops,
        None,
        None,
        Some("EdGS"),
        Some(gs_id),
        None,
        None,
    )
}

fn bake_line_markup(
    doc: &mut Document,
    page_id: ObjectId,
    quads: &[(f32, f32, f32, f32)],
    color: &str,
    strike: bool,
) -> Result<(), AppError> {
    let (r, g, b) = parse_color(color);
    let mut ops = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "RG",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new("w", vec![Object::Real(1.2)]),
    ];
    for &(x, y, w, h) in quads {
        let ly = if strike { y + h * 0.45 } else { y + 1.0 };
        ops.push(Operation::new(
            "m",
            vec![Object::Real(x), Object::Real(ly)],
        ));
        ops.push(Operation::new(
            "l",
            vec![Object::Real(x + w), Object::Real(ly)],
        ));
        ops.push(Operation::new("S", vec![]));
    }
    ops.push(Operation::new("Q", vec![]));
    append_page_content(doc, page_id, ops, None, None, None, None, None, None)
}

fn highlight_ap(
    doc: &mut Document,
    quads: &[(f32, f32, f32, f32)],
    r: f32,
    g: f32,
    b: f32,
    opacity: f32,
) -> Result<ObjectId, AppError> {
    let (x0, y0, x1, y1) = union_quads(quads);
    let w = (x1 - x0).max(1.0);
    let h = (y1 - y0).max(1.0);
    let mut ops = vec![
        Operation::new(
            "rg",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new("gs", vec![Object::Name(b"GS0".to_vec())]),
    ];
    for &(x, y, qw, qh) in quads {
        ops.push(Operation::new(
            "re",
            vec![
                Object::Real(x - x0),
                Object::Real(y - y0),
                Object::Real(qw),
                Object::Real(qh),
            ],
        ));
        ops.push(Operation::new("f", vec![]));
    }
    make_form_xobject(doc, w, h, ops, Some(opacity))
}

fn line_markup_ap(
    doc: &mut Document,
    quads: &[(f32, f32, f32, f32)],
    r: f32,
    g: f32,
    b: f32,
    strike: bool,
) -> Result<ObjectId, AppError> {
    let (x0, y0, x1, y1) = union_quads(quads);
    let w = (x1 - x0).max(1.0);
    let h = (y1 - y0).max(1.0);
    let mut ops = vec![
        Operation::new(
            "RG",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new("w", vec![Object::Real(1.2)]),
    ];
    for &(x, y, qw, qh) in quads {
        let ly = if strike {
            y - y0 + qh * 0.45
        } else {
            y - y0 + 1.0
        };
        ops.push(Operation::new(
            "m",
            vec![Object::Real(x - x0), Object::Real(ly)],
        ));
        ops.push(Operation::new(
            "l",
            vec![Object::Real(x - x0 + qw), Object::Real(ly)],
        ));
        ops.push(Operation::new("S", vec![]));
    }
    make_form_xobject(doc, w, h, ops, None)
}

fn note_ap(doc: &mut Document, size: f32, r: f32, g: f32, b: f32) -> Result<ObjectId, AppError> {
    let ops = vec![
        Operation::new(
            "rg",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new(
            "re",
            vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(size),
                Object::Real(size),
            ],
        ),
        Operation::new("f", vec![]),
    ];
    make_form_xobject(doc, size, size, ops, None)
}

fn shape_ap(
    doc: &mut Document,
    subtype: &str,
    w: f32,
    h: f32,
    r: f32,
    g: f32,
    b: f32,
    fill: Option<&str>,
    stroke_width: f32,
    opacity: f32,
) -> Result<ObjectId, AppError> {
    let mut ops = vec![
        Operation::new("gs", vec![Object::Name(b"GS0".to_vec())]),
        Operation::new(
            "RG",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new("w", vec![Object::Real(stroke_width)]),
    ];
    if subtype == "Circle" {
        let cx = w / 2.0;
        let cy = h / 2.0;
        let rx = w / 2.0;
        let ry = h / 2.0;
        let k = 0.5522847498;
        let ox = rx * k;
        let oy = ry * k;
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
    } else {
        ops.push(Operation::new(
            "re",
            vec![
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(w),
                Object::Real(h),
            ],
        ));
    }
    if let Some(fill_hex) = fill {
        let (fr, fg, fb) = parse_color(fill_hex);
        ops.push(Operation::new(
            "rg",
            vec![Object::Real(fr), Object::Real(fg), Object::Real(fb)],
        ));
        ops.push(Operation::new("B", vec![]));
    } else {
        ops.push(Operation::new("S", vec![]));
    }
    make_form_xobject(doc, w, h, ops, Some(opacity))
}

fn line_ap(
    doc: &mut Document,
    from: (f32, f32),
    to: (f32, f32),
    r: f32,
    g: f32,
    b: f32,
    width: f32,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
) -> Result<ObjectId, AppError> {
    let w = (max_x - min_x).max(1.0);
    let h = (max_y - min_y).max(1.0);
    let ops = vec![
        Operation::new(
            "RG",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new("w", vec![Object::Real(width)]),
        Operation::new(
            "m",
            vec![
                Object::Real(from.0 - min_x),
                Object::Real(from.1 - min_y),
            ],
        ),
        Operation::new(
            "l",
            vec![Object::Real(to.0 - min_x), Object::Real(to.1 - min_y)],
        ),
        Operation::new("S", vec![]),
    ];
    make_form_xobject(doc, w, h, ops, None)
}

fn stamp_ap(
    doc: &mut Document,
    w: f32,
    h: f32,
    label: &str,
    r: f32,
    g: f32,
    b: f32,
) -> Result<ObjectId, AppError> {
    let font_id = ensure_type1_font(doc, "Helvetica-Bold");
    let mut size = (h * 0.35).clamp(10.0, 36.0);
    while text_width(label, size) > w * 0.85 && size > 8.0 {
        size -= 1.0;
    }
    let tw = text_width(label, size);
    let escaped = escape_pdf_string(label);
    let ops = vec![
        Operation::new(
            "RG",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new(
            "rg",
            vec![Object::Real(r), Object::Real(g), Object::Real(b)],
        ),
        Operation::new("w", vec![Object::Real(2.0)]),
        Operation::new(
            "re",
            vec![
                Object::Real(2.0),
                Object::Real(2.0),
                Object::Real(w - 4.0),
                Object::Real(h - 4.0),
            ],
        ),
        Operation::new("S", vec![]),
        Operation::new("BT", vec![]),
        Operation::new(
            "Tf",
            vec![Object::Name(b"F1".to_vec()), Object::Real(size)],
        ),
        Operation::new(
            "Td",
            vec![
                Object::Real((w - tw) / 2.0),
                Object::Real(h / 2.0 - size * 0.35),
            ],
        ),
        Operation::new("Tj", vec![Object::string_literal(escaped.as_str())]),
        Operation::new("ET", vec![]),
    ];

    // Form with font resource
    let content = Content { operations: ops };
    let data = content
        .encode()
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let mut resources = Dictionary::new();
    let mut fonts = Dictionary::new();
    fonts.set("F1", font_id);
    resources.set("Font", fonts);

    let mut dict = Dictionary::new();
    dict.set("Type", "XObject");
    dict.set("Subtype", "Form");
    dict.set(
        "BBox",
        vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(w),
            Object::Real(h),
        ],
    );
    dict.set("Resources", resources);
    dict.set("Filter", "FlateDecode");
    Ok(doc.add_object(Stream::new(dict, compress_flate(&data)?)))
}

fn make_form_xobject(
    doc: &mut Document,
    w: f32,
    h: f32,
    ops: Vec<Operation>,
    opacity: Option<f32>,
) -> Result<ObjectId, AppError> {
    let content = Content { operations: ops };
    let data = content
        .encode()
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    let mut resources = Dictionary::new();
    if let Some(a) = opacity {
        let mut gs = Dictionary::new();
        gs.set("Type", "ExtGState");
        gs.set("ca", Object::Real(a.clamp(0.05, 1.0)));
        gs.set("CA", Object::Real(a.clamp(0.05, 1.0)));
        gs.set("BM", Object::Name(b"Multiply".to_vec()));
        let gs_id = doc.add_object(gs);
        let mut eg = Dictionary::new();
        eg.set("GS0", gs_id);
        resources.set("ExtGState", eg);
    }

    let mut dict = Dictionary::new();
    dict.set("Type", "XObject");
    dict.set("Subtype", "Form");
    dict.set(
        "BBox",
        vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(w),
            Object::Real(h),
        ],
    );
    dict.set("Resources", resources);
    dict.set("Filter", "FlateDecode");
    Ok(doc.add_object(Stream::new(dict, compress_flate(&data)?)))
}

fn attach_annot(doc: &mut Document, page_id: ObjectId, annot: Dictionary) -> Result<(), AppError> {
    let ann_id = doc.add_object(annot);
    let mut page = doc
        .get_object(page_id)
        .and_then(|o| o.as_dict())
        .map(|d| d.clone())
        .map_err(|e| AppError::Pdf(e.to_string()))?;

    match page.get(b"Annots").ok().cloned() {
        Some(Object::Array(mut arr)) => {
            arr.push(Object::Reference(ann_id));
            page.set("Annots", Object::Array(arr));
        }
        Some(Object::Reference(id)) => {
            if let Ok(Object::Array(mut arr)) = doc.get_object(id).map(|o| o.clone()) {
                arr.push(Object::Reference(ann_id));
                doc.objects.insert(id, Object::Array(arr));
            } else {
                page.set(
                    "Annots",
                    Object::Array(vec![Object::Reference(id), Object::Reference(ann_id)]),
                );
            }
        }
        _ => {
            page.set("Annots", Object::Array(vec![Object::Reference(ann_id)]));
        }
    }
    doc.objects.insert(page_id, Object::Dictionary(page));
    Ok(())
}

fn collect_annot_ids(doc: &Document, page_id: ObjectId) -> Vec<ObjectId> {
    let Ok(page) = doc.get_object(page_id).and_then(|o| o.as_dict()) else {
        return vec![];
    };
    match page.get(b"Annots") {
        Ok(Object::Array(arr)) => arr
            .iter()
            .filter_map(|o| o.as_reference().ok())
            .collect(),
        Ok(Object::Reference(id)) => match doc.get_object(*id) {
            Ok(Object::Array(arr)) => arr
                .iter()
                .filter_map(|o| o.as_reference().ok())
                .collect(),
            _ => vec![*id],
        },
        _ => vec![],
    }
}

fn union_quads(quads: &[(f32, f32, f32, f32)]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for &(x, y, w, h) in quads {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + w);
        max_y = max_y.max(y + h);
    }
    if !min_x.is_finite() {
        return (0.0, 0.0, 10.0, 10.0);
    }
    (min_x, min_y, max_x, max_y)
}

fn read_rect(ann: &Dictionary) -> Option<(f32, f32, f32, f32)> {
    let arr = ann.get(b"Rect").ok()?.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    Some((
        super::util::to_f32(&arr[0])?,
        super::util::to_f32(&arr[1])?,
        super::util::to_f32(&arr[2])?,
        super::util::to_f32(&arr[3])?,
    ))
}

fn resolve_dict<'a>(doc: &'a Document, obj: &'a Object) -> Result<&'a Dictionary, AppError> {
    match obj {
        Object::Dictionary(d) => Ok(d),
        Object::Reference(id) => doc
            .get_object(*id)
            .and_then(|o| o.as_dict())
            .map_err(|e| AppError::Pdf(e.to_string())),
        _ => Err(AppError::Pdf("Expected dictionary".into())),
    }
}
