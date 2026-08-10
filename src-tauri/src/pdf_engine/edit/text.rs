use super::draw;
use super::util::{
    append_page_content, approx_char_width, escape_pdf_string, name_to_str, text_width, to_f32,
};
use crate::error::AppError;
use crate::pdf_engine::ensure_pdf_path;
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Encoding, Object, ObjectId, StringFormat};
use serde::Serialize;

/// Row-major 2D affine transform `[a, b, c, d, e, f]` as used by PDF `cm` / `Tm`.
type Mat = [f32; 6];

const IDENTITY: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn mat_mul(m: Mat, n: Mat) -> Mat {
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

fn translation(tx: f32, ty: f32) -> Mat {
    [1.0, 0.0, 0.0, 1.0, tx, ty]
}

/// Length of the transformed unit basis vectors, i.e. how much the matrix
/// stretches horizontally and vertically.
fn mat_scale(m: Mat) -> (f32, f32) {
    (
        (m[0] * m[0] + m[1] * m[1]).sqrt().max(0.0001),
        (m[2] * m[2] + m[3] * m[3]).sqrt().max(0.0001),
    )
}

fn mat_from_operands(operands: &[Object]) -> Option<Mat> {
    let mut m = IDENTITY;
    for (i, slot) in m.iter_mut().enumerate() {
        *slot = to_f32(operands.get(i)?)?;
    }
    Some(m)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRun {
    pub run_id: u32,
    pub page: u32,
    pub text: String,
    /// Baseline origin in PDF points (bottom-left page origin), after CTM.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    /// Effective font size in points; the glyph box spans roughly
    /// `y - 0.24*h` to `y + 0.80*h`.
    pub h: f32,
    pub font_name: String,
    pub font_size: f32,
    pub color: String,
    /// False when font is Type0/CID or encoding cannot map WinAnsi.
    pub editable: bool,
}

#[derive(Debug, Clone)]
pub enum ReplaceOutcome {
    Surgical,
    Overlay { warning: String },
}

struct FontInfo {
    base_name: String,
    #[allow(dead_code)]
    subtype: String,
    editable: bool,
    avg_width: f32,
}

pub fn list_text_runs(path: String, page: u32) -> Result<Vec<TextRun>, AppError> {
    let input = ensure_pdf_path(&path)?;
    let doc = Document::load(&input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Desbloquea el PDF antes de editar texto".into(),
        ));
    }
    let pages = doc.get_pages();
    let page_id = *pages
        .get(&page)
        .ok_or_else(|| AppError::InvalidInput(format!("Página {page} no existe")))?;
    list_text_runs_doc(&doc, page, page_id)
}

pub fn list_text_runs_doc(
    doc: &Document,
    page: u32,
    page_id: ObjectId,
) -> Result<Vec<TextRun>, AppError> {
    let content_data = doc
        .get_page_content(page_id)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let content = Content::decode(&content_data).map_err(|e| AppError::Pdf(e.to_string()))?;

    let fonts = doc.get_page_fonts(page_id).unwrap_or_default();
    let mut font_cache: std::collections::HashMap<Vec<u8>, FontInfo> =
        std::collections::HashMap::new();
    for (name, font_dict) in &fonts {
        font_cache.insert(name.clone(), analyze_font(doc, font_dict));
    }

    let encodings: std::collections::BTreeMap<Vec<u8>, Encoding> = fonts
        .iter()
        .filter_map(|(name, font)| {
            font.get_font_encoding(doc)
                .ok()
                .map(|enc| (name.clone(), enc))
        })
        .collect();

    let mut runs = Vec::new();
    // Stable id across list/replace: index of every text-showing operator,
    // including the ones that decode to an empty string.
    let mut run_id: u32 = 0;

    // Text state
    let mut font_key: Option<Vec<u8>> = None;
    let mut font_size: f32 = 12.0;
    let mut leading: f32 = 14.0;
    let mut h_scale: f32 = 1.0;
    let mut rise: f32 = 0.0;
    let mut fill_rgb: (f32, f32, f32) = (0.0, 0.0, 0.0);
    let mut in_text = false;

    // Graphics + text matrices, so runs land in real device space.
    let mut ctm: Mat = IDENTITY;
    let mut ctm_stack: Vec<Mat> = Vec::new();
    let mut tm: Mat = IDENTITY;
    let mut tlm: Mat = IDENTITY;

    for op in &content.operations {
        match op.operator.as_str() {
            "q" => ctm_stack.push(ctm),
            "Q" => {
                if let Some(prev) = ctm_stack.pop() {
                    ctm = prev;
                }
            }
            "cm" if op.operands.len() >= 6 => {
                if let Some(m) = mat_from_operands(&op.operands) {
                    ctm = mat_mul(m, ctm);
                }
            }
            "BT" => {
                in_text = true;
                tm = IDENTITY;
                tlm = IDENTITY;
            }
            "ET" => {
                in_text = false;
            }
            "rg" if op.operands.len() >= 3 => {
                fill_rgb = (
                    to_f32(&op.operands[0]).unwrap_or(0.0),
                    to_f32(&op.operands[1]).unwrap_or(0.0),
                    to_f32(&op.operands[2]).unwrap_or(0.0),
                );
            }
            "g" if !op.operands.is_empty() => {
                let g = to_f32(&op.operands[0]).unwrap_or(0.0);
                fill_rgb = (g, g, g);
            }
            "Tf" if op.operands.len() >= 2 => {
                if let Ok(name) = op.operands[0].as_name() {
                    font_key = Some(name.to_vec());
                }
                font_size = to_f32(&op.operands[1]).unwrap_or(12.0);
                leading = font_size * 1.2;
            }
            "Tz" if !op.operands.is_empty() => {
                h_scale = to_f32(&op.operands[0]).unwrap_or(100.0) / 100.0;
            }
            "TL" if !op.operands.is_empty() => {
                leading = to_f32(&op.operands[0]).unwrap_or(leading);
            }
            "Ts" if !op.operands.is_empty() => {
                rise = to_f32(&op.operands[0]).unwrap_or(0.0);
            }
            "Td" | "TD" if in_text && op.operands.len() >= 2 => {
                let dx = to_f32(&op.operands[0]).unwrap_or(0.0);
                let dy = to_f32(&op.operands[1]).unwrap_or(0.0);
                if op.operator == "TD" {
                    leading = -dy;
                }
                tlm = mat_mul(translation(dx, dy), tlm);
                tm = tlm;
            }
            "Tm" if in_text && op.operands.len() >= 6 => {
                if let Some(m) = mat_from_operands(&op.operands) {
                    tlm = m;
                    tm = m;
                }
            }
            "T*" if in_text => {
                tlm = mat_mul(translation(0.0, -leading), tlm);
                tm = tlm;
            }
            "Tj" | "'" | "\"" | "TJ" if in_text => {
                // ' and " move to the next line *before* showing the text.
                if op.operator == "'" || op.operator == "\"" {
                    tlm = mat_mul(translation(0.0, -leading), tlm);
                    tm = tlm;
                }

                let encoding = font_key.as_ref().and_then(|k| encodings.get(k));
                let text = extract_operand_text(&op.operands, encoding);
                let info = font_key.as_ref().and_then(|k| font_cache.get(k));
                let (editable, font_name, avg_w) = match info {
                    Some(fi) => (fi.editable, fi.base_name.clone(), fi.avg_width),
                    None => (false, "Unknown".into(), 500.0),
                };

                // Advance in unscaled text space.
                let advance = if op.operator == "TJ" {
                    measure_tj_width(&op.operands, font_size, h_scale, avg_w, encoding)
                } else {
                    measure_text(&text, font_size, h_scale, avg_w)
                };

                if !text.is_empty() {
                    // Trm = [Tfs*Th, 0, 0, Tfs, 0, Ts] x Tm x CTM
                    let scale = [font_size * h_scale, 0.0, 0.0, font_size, 0.0, rise];
                    let trm = mat_mul(mat_mul(scale, tm), ctm);
                    let base = mat_mul(tm, ctm);
                    let (sx, sy) = mat_scale(base);
                    let eff_size = (font_size * sy).max(0.1);

                    runs.push(TextRun {
                        run_id,
                        page,
                        text,
                        x: trm[4],
                        y: trm[5],
                        w: (advance * sx).max(0.1),
                        h: eff_size,
                        font_name,
                        font_size: eff_size,
                        color: rgb_to_hex(fill_rgb),
                        editable,
                    });
                }

                tm = mat_mul(translation(advance, 0.0), tm);
                run_id += 1;
            }
            _ => {}
        }
    }

    Ok(runs)
}

pub fn replace_text_run(
    doc: &mut Document,
    page: u32,
    run_id: u32,
    new_text: &str,
) -> Result<ReplaceOutcome, AppError> {
    let pages = doc.get_pages();
    let page_id = *pages
        .get(&page)
        .ok_or_else(|| AppError::InvalidInput(format!("Página {page} no existe")))?;

    let runs = list_text_runs_doc(doc, page, page_id)?;
    let run = runs
        .iter()
        .find(|r| r.run_id == run_id)
        .cloned()
        .ok_or_else(|| AppError::InvalidInput(format!("Run {run_id} no encontrado")))?;

    if !run.editable {
        return overlay_replace(doc, page_id, &run, new_text);
    }

    match surgical_replace(doc, page_id, run_id, &run, new_text) {
        Ok(()) => Ok(ReplaceOutcome::Surgical),
        Err(e) => {
            let warning = format!(
                "Reemplazo quirúrgico falló ({e}); se usó tapar+reescribir en run {run_id}"
            );
            overlay_replace(doc, page_id, &run, new_text)?;
            Ok(ReplaceOutcome::Overlay { warning })
        }
    }
}

fn surgical_replace(
    doc: &mut Document,
    page_id: ObjectId,
    run_id: u32,
    run: &TextRun,
    new_text: &str,
) -> Result<(), AppError> {
    let fonts = doc
        .get_page_fonts(page_id)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let encodings: std::collections::BTreeMap<Vec<u8>, Encoding> = fonts
        .iter()
        .filter_map(|(name, font)| {
            font.get_font_encoding(doc)
                .ok()
                .map(|enc| (name.clone(), enc))
        })
        .collect();

    let content_data = doc
        .get_page_content(page_id)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    let mut content = Content::decode(&content_data).map_err(|e| AppError::Pdf(e.to_string()))?;

    let mut current_font: Option<Vec<u8>> = None;
    let mut seen: u32 = 0;
    let mut found = false;
    let mut insert_tz_at: Option<usize> = None;
    let mut tz_value: f32 = 100.0;

    for (idx, op) in content.operations.iter_mut().enumerate() {
        match op.operator.as_str() {
            "Tf" => {
                if let Some(Object::Name(n)) = op.operands.first() {
                    current_font = Some(n.clone());
                }
            }
            "Tj" | "TJ" | "'" | "\"" => {
                if seen == run_id {
                    let encoding = current_font
                        .as_ref()
                        .and_then(|k| encodings.get(k))
                        .ok_or_else(|| AppError::Pdf("Sin encoding para la fuente".into()))?;

                    let old_w = run.w.max(1.0);
                    let new_w = text_width(new_text, run.font_size).max(0.5);
                    if new_w > old_w * 1.02 {
                        // Scale horizontally to fit
                        tz_value = (old_w / new_w * 100.0).clamp(40.0, 100.0);
                        insert_tz_at = Some(idx);
                    }

                    replace_text_in_op(op, encoding, new_text)?;
                    found = true;
                    break;
                }
                seen += 1;
            }
            _ => {}
        }
    }

    if !found {
        return Err(AppError::Pdf(format!(
            "No se encontró el operador del run {run_id}"
        )));
    }

    if let Some(at) = insert_tz_at {
        content.operations.insert(
            at,
            Operation::new("Tz", vec![Object::Real(tz_value)]),
        );
        // Restore scale after the text op (now at at+1)
        content.operations.insert(
            at + 2,
            Operation::new("Tz", vec![Object::Real(100.0)]),
        );
    }

    let encoded = content
        .encode()
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    doc.change_page_content(page_id, encoded)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    Ok(())
}

fn overlay_replace(
    doc: &mut Document,
    page_id: ObjectId,
    run: &TextRun,
    new_text: &str,
) -> Result<ReplaceOutcome, AppError> {
    // Cover the original glyph box: descenders sit below the baseline,
    // ascenders above it.
    let x = run.x - 1.0;
    let y = run.y - run.h * 0.26;
    let w = run.w + 2.0;
    let h = run.h * 1.12;

    let mut ops = vec![
        Operation::new("q", vec![]),
        Operation::new("rg", vec![1.0.into(), 1.0.into(), 1.0.into()]),
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
    append_page_content(doc, page_id, ops.drain(..).collect(), None, None, None, None, None, None)?;

    // bake_text puts the first baseline at `y + h - size`, so pass the run's
    // own height to land exactly on the original baseline.
    draw::bake_text(
        doc,
        page_id,
        run.x,
        run.y,
        w,
        run.h,
        new_text,
        "Helvetica",
        run.font_size,
        false,
        false,
        &run.color,
        "left",
        1.0,
    )?;

    Ok(ReplaceOutcome::Overlay {
        warning: format!(
            "Run {} usó tapar+reescribir (fuente no editable o encoding)",
            run.run_id
        ),
    })
}

fn replace_text_in_op(
    op: &mut Operation,
    encoding: &Encoding,
    new_text: &str,
) -> Result<(), AppError> {
    let encoded = Document::encode_text(encoding, new_text);
    match op.operator.as_str() {
        "TJ" => {
            // Replace first string in the TJ array; drop remaining strings
            if let Some(Object::Array(arr)) = op.operands.first_mut() {
                let mut new_arr = vec![Object::String(encoded, StringFormat::Literal)];
                // Keep numeric kerning? Simpler: just one string
                *arr = new_arr.drain(..).collect();
            } else {
                op.operands = vec![Object::String(encoded, StringFormat::Literal)];
                op.operator = "Tj".into();
            }
        }
        _ => {
            if let Some(bytes) = op.operands.iter_mut().find_map(|o| match o {
                Object::String(b, _) => Some(b),
                _ => None,
            }) {
                *bytes = encoded;
            } else {
                op.operands = vec![Object::String(encoded, StringFormat::Literal)];
                op.operator = "Tj".into();
            }
        }
    }
    Ok(())
}

fn analyze_font(doc: &Document, font: &Dictionary) -> FontInfo {
    let subtype = font
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .map(name_to_str)
        .unwrap_or_else(|| "Unknown".into());

    let base_name = font
        .get(b"BaseFont")
        .ok()
        .and_then(|o| o.as_name().ok())
        .map(name_to_str)
        .or_else(|| {
            // Type0 → DescendantFonts → BaseFont
            font.get(b"DescendantFonts")
                .ok()
                .and_then(|o| match o {
                    Object::Array(arr) => arr.first(),
                    Object::Reference(id) => doc.get_object(*id).ok(),
                    _ => None,
                })
                .and_then(|o| match o {
                    Object::Reference(id) => doc.get_object(*id).ok().and_then(|x| x.as_dict().ok()),
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                })
                .and_then(|d| d.get(b"BaseFont").ok())
                .and_then(|o| o.as_name().ok())
                .map(name_to_str)
        })
        .unwrap_or_else(|| "Unknown".into());

    let editable = matches!(
        subtype.as_str(),
        "Type1" | "TrueType" | "MMType1"
    ) && !base_name.contains('+'); // subset fonts often have + prefix → overlay

    // Prefer Widths average if present
    let avg_width = font
        .get(b"Widths")
        .ok()
        .and_then(|o| o.as_array().ok())
        .map(|arr| {
            let mut sum = 0.0f32;
            let mut n = 0u32;
            for v in arr {
                if let Some(w) = to_f32(v) {
                    sum += w;
                    n += 1;
                }
            }
            if n > 0 {
                sum / n as f32
            } else {
                500.0
            }
        })
        .unwrap_or(500.0);

    FontInfo {
        base_name,
        subtype,
        editable,
        avg_width,
    }
}

fn extract_operand_text(operands: &[Object], encoding: Option<&Encoding>) -> String {
    let mut out = String::new();
    collect_text(&mut out, encoding, operands);
    out
}

fn collect_text(text: &mut String, encoding: Option<&Encoding>, operands: &[Object]) {
    for operand in operands {
        match operand {
            Object::String(bytes, _) => {
                if let Some(enc) = encoding {
                    if let Ok(s) = Document::decode_text(enc, bytes) {
                        text.push_str(&s);
                        continue;
                    }
                }
                // Latin-1 fallback
                for &b in bytes {
                    if b >= 32 {
                        text.push(b as char);
                    }
                }
            }
            Object::Array(arr) => collect_text(text, encoding, arr),
            Object::Integer(i) if *i < -100 => {
                text.push(' ');
            }
            _ => {}
        }
    }
}

fn measure_text(text: &str, font_size: f32, h_scale: f32, avg_w: f32) -> f32 {
    let w: f32 = text
        .chars()
        .map(|c| {
            let unit = if avg_w > 50.0 {
                // blend approx with font avg
                (approx_char_width(c) + avg_w) * 0.5
            } else {
                approx_char_width(c)
            };
            unit * font_size / 1000.0
        })
        .sum();
    w * h_scale
}

fn measure_tj_width(
    operands: &[Object],
    font_size: f32,
    h_scale: f32,
    avg_w: f32,
    encoding: Option<&Encoding>,
) -> f32 {
    let mut w = 0.0f32;
    let arr = match operands.first() {
        Some(Object::Array(a)) => a.as_slice(),
        _ => operands,
    };
    for item in arr {
        match item {
            Object::String(bytes, _) => {
                let s = if let Some(enc) = encoding {
                    Document::decode_text(enc, bytes).unwrap_or_else(|_| {
                        bytes.iter().map(|&b| b as char).collect()
                    })
                } else {
                    bytes.iter().map(|&b| b as char).collect()
                };
                w += measure_text(&s, font_size, 1.0, avg_w);
            }
            Object::Integer(i) => {
                w -= (*i as f32) / 1000.0 * font_size;
            }
            Object::Real(r) => {
                w -= (*r) / 1000.0 * font_size;
            }
            _ => {}
        }
    }
    w * h_scale
}

fn rgb_to_hex((r, g, b): (f32, f32, f32)) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8
    )
}

#[allow(dead_code)]
fn _escape_unused(s: &str) -> String {
    escape_pdf_string(s)
}
