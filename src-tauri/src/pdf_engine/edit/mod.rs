mod annots;
mod draw;
mod text;
mod util;

pub use text::{list_text_runs, TextRun};

use crate::error::{AppError, OpResult};
use crate::pdf_engine::forms::{fill_form, FieldFill};
use crate::pdf_engine::progress::Progress;
use crate::pdf_engine::{ensure_parent_dir, ensure_pdf_path};
use lopdf::Document;
use serde::Deserialize;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum EditOp {
    #[serde(rename_all = "camelCase")]
    ReplaceText {
        page: u32,
        run_id: u32,
        new_text: String,
        /// Horizontal space the text may occupy, in points. Defaults to the
        /// run's own width; a line edit passes the whole line so the text is
        /// not squeezed into the first fragment.
        #[serde(default)]
        fit_width: Option<f32>,
    },
    #[serde(rename_all = "camelCase")]
    AddText {
        page: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        text: String,
        #[serde(default = "default_font")]
        font: String,
        #[serde(default = "default_size")]
        size: f32,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default = "default_color")]
        color: String,
        #[serde(default = "default_align")]
        align: String,
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
    #[serde(rename_all = "camelCase")]
    Highlight {
        page: u32,
        /// Each quad: x,y,w,h in PDF points
        quads: Vec<[f32; 4]>,
        #[serde(default = "default_highlight")]
        color: String,
        #[serde(default = "default_highlight_opacity")]
        opacity: f32,
    },
    #[serde(rename_all = "camelCase")]
    Underline {
        page: u32,
        quads: Vec<[f32; 4]>,
        #[serde(default = "default_color")]
        color: String,
    },
    #[serde(rename_all = "camelCase")]
    Strikeout {
        page: u32,
        quads: Vec<[f32; 4]>,
        #[serde(default = "default_strike")]
        color: String,
    },
    #[serde(rename_all = "camelCase")]
    Note {
        page: u32,
        x: f32,
        y: f32,
        text: String,
        #[serde(default = "default_note_color")]
        color: String,
    },
    #[serde(rename_all = "camelCase")]
    Rect {
        page: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[serde(default = "default_stroke")]
        stroke: String,
        fill: Option<String>,
        #[serde(default = "default_stroke_width")]
        stroke_width: f32,
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
    #[serde(rename_all = "camelCase")]
    Ellipse {
        page: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[serde(default = "default_stroke")]
        stroke: String,
        fill: Option<String>,
        #[serde(default = "default_stroke_width")]
        stroke_width: f32,
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
    #[serde(rename_all = "camelCase")]
    Line {
        page: u32,
        from: [f32; 2],
        to: [f32; 2],
        #[serde(default = "default_stroke")]
        color: String,
        #[serde(default = "default_stroke_width")]
        width: f32,
        #[serde(default = "default_arrow")]
        arrow: String,
    },
    #[serde(rename_all = "camelCase")]
    FreeDraw {
        page: u32,
        paths: Vec<Vec<[f32; 2]>>,
        #[serde(default = "default_stroke")]
        color: String,
        #[serde(default = "default_stroke_width")]
        width: f32,
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
    #[serde(rename_all = "camelCase")]
    Whiteout {
        page: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Image {
        page: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        image_path: String,
        #[serde(default)]
        rotation: f32,
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
    #[serde(rename_all = "camelCase")]
    Stamp {
        page: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        stamp: String,
        custom_text: Option<String>,
        #[serde(default = "default_stroke")]
        color: String,
    },
    #[serde(rename_all = "camelCase")]
    FormFill {
        field: String,
        value: String,
    },
}

fn default_font() -> String {
    "Helvetica".into()
}
fn default_size() -> f32 {
    12.0
}
fn default_color() -> String {
    "#1a1a1a".into()
}
fn default_align() -> String {
    "left".into()
}
fn default_opacity() -> f32 {
    1.0
}
fn default_highlight() -> String {
    "#ffe066".into()
}
fn default_highlight_opacity() -> f32 {
    0.45
}
fn default_strike() -> String {
    "#e11d48".into()
}
fn default_note_color() -> String {
    "#f5c542".into()
}
fn default_stroke() -> String {
    "#e11d48".into()
}
fn default_stroke_width() -> f32 {
    1.5
}
fn default_arrow() -> String {
    "none".into()
}

pub fn edit_pdf(
    path: String,
    output: String,
    ops: Vec<EditOp>,
    flatten: bool,
    progress: Option<Progress>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    if ops.is_empty() {
        return Err(AppError::InvalidInput(
            "No hay operaciones de edición".into(),
        ));
    }

    let mut doc = Document::load(&input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Desbloquea el PDF antes de editar".into(),
        ));
    }

    let pages = doc.get_pages();
    let total_pages = pages.len() as u32;
    if total_pages == 0 {
        return Err(AppError::InvalidInput("PDF sin páginas".into()));
    }

    let progress = progress.unwrap_or_else(Progress::none);
    let mut warnings: Vec<String> = Vec::new();
    let total_ops = ops.len() as u32;
    progress.emit(0, total_ops, "Editando PDF…");

    // Phase 1: ReplaceText (mutates content streams)
    for (i, op) in ops.iter().enumerate() {
        progress.tick(i as u32, total_ops, "Reemplazando texto…")?;
        if let EditOp::ReplaceText {
            page,
            run_id,
            new_text,
            fit_width,
        } = op
        {
            match text::replace_text_run(&mut doc, *page, *run_id, new_text, *fit_width)? {
                text::ReplaceOutcome::Surgical => {}
                text::ReplaceOutcome::Overlay { warning } => warnings.push(warning),
            }
        }
    }

    // Phase 2: Form fills
    let fills: Vec<FieldFill> = ops
        .iter()
        .filter_map(|op| match op {
            EditOp::FormFill { field, value } => Some(FieldFill {
                name: field.clone(),
                value: value.clone(),
            }),
            _ => None,
        })
        .collect();
    if !fills.is_empty() {
        fill_form(&mut doc, &fills)?;
    }

    // Phase 3: Draw / annotate
    for (i, op) in ops.iter().enumerate() {
        progress.tick(i as u32, total_ops, "Aplicando ediciones…")?;
        let pages_map = doc.get_pages();
        match op {
            EditOp::ReplaceText { .. } | EditOp::FormFill { .. } => {}
            EditOp::AddText {
                page,
                x,
                y,
                w,
                h,
                text: t,
                font,
                size,
                bold,
                italic,
                color,
                align,
                opacity,
            } => {
                let page_id = page_id(&pages_map, *page)?;
                draw::bake_text(
                    &mut doc, page_id, *x, *y, *w, *h, t, font, *size, *bold, *italic, color,
                    align, *opacity,
                )?;
            }
            EditOp::Highlight {
                page,
                quads,
                color,
                opacity,
            } => {
                let page_id = page_id(&pages_map, *page)?;
                let q: Vec<(f32, f32, f32, f32)> =
                    quads.iter().map(|a| (a[0], a[1], a[2], a[3])).collect();
                annots::add_highlight(&mut doc, page_id, &q, color, *opacity, flatten)?;
            }
            EditOp::Underline { page, quads, color } => {
                let page_id = page_id(&pages_map, *page)?;
                let q: Vec<(f32, f32, f32, f32)> =
                    quads.iter().map(|a| (a[0], a[1], a[2], a[3])).collect();
                annots::add_underline(&mut doc, page_id, &q, color, flatten)?;
            }
            EditOp::Strikeout { page, quads, color } => {
                let page_id = page_id(&pages_map, *page)?;
                let q: Vec<(f32, f32, f32, f32)> =
                    quads.iter().map(|a| (a[0], a[1], a[2], a[3])).collect();
                annots::add_strikeout(&mut doc, page_id, &q, color, flatten)?;
            }
            EditOp::Note {
                page,
                x,
                y,
                text: t,
                color,
            } => {
                let page_id = page_id(&pages_map, *page)?;
                annots::add_note(&mut doc, page_id, *x, *y, t, color, flatten)?;
            }
            EditOp::Rect {
                page,
                x,
                y,
                w,
                h,
                stroke,
                fill,
                stroke_width,
                opacity,
            } => {
                let page_id = page_id(&pages_map, *page)?;
                annots::add_square_annot(
                    &mut doc,
                    page_id,
                    *x,
                    *y,
                    *w,
                    *h,
                    stroke,
                    fill.as_deref(),
                    *stroke_width,
                    *opacity,
                    flatten,
                )?;
            }
            EditOp::Ellipse {
                page,
                x,
                y,
                w,
                h,
                stroke,
                fill,
                stroke_width,
                opacity,
            } => {
                let page_id = page_id(&pages_map, *page)?;
                annots::add_ellipse_annot(
                    &mut doc,
                    page_id,
                    *x,
                    *y,
                    *w,
                    *h,
                    stroke,
                    fill.as_deref(),
                    *stroke_width,
                    *opacity,
                    flatten,
                )?;
            }
            EditOp::Line {
                page,
                from,
                to,
                color,
                width,
                arrow,
            } => {
                let page_id = page_id(&pages_map, *page)?;
                annots::add_line_annot(
                    &mut doc,
                    page_id,
                    (from[0], from[1]),
                    (to[0], to[1]),
                    color,
                    *width,
                    arrow,
                    flatten,
                )?;
            }
            EditOp::FreeDraw {
                page,
                paths,
                color,
                width,
                opacity,
            } => {
                let page_id = page_id(&pages_map, *page)?;
                let converted: Vec<Vec<(f32, f32)>> = paths
                    .iter()
                    .map(|p| p.iter().map(|pt| (pt[0], pt[1])).collect())
                    .collect();
                // Free-draw always baked (no standard Ink AP complexity for v1)
                draw::bake_free_draw(&mut doc, page_id, &converted, color, *width, *opacity)?;
            }
            EditOp::Whiteout {
                page,
                x,
                y,
                w,
                h,
                color,
            } => {
                let page_id = page_id(&pages_map, *page)?;
                draw::bake_whiteout(&mut doc, page_id, *x, *y, *w, *h, color.as_deref())?;
            }
            EditOp::Image {
                page,
                x,
                y,
                w,
                h,
                image_path,
                rotation,
                opacity,
            } => {
                let page_id = page_id(&pages_map, *page)?;
                draw::bake_image(
                    &mut doc,
                    page_id,
                    *x,
                    *y,
                    *w,
                    *h,
                    image_path,
                    *rotation,
                    *opacity,
                )?;
            }
            EditOp::Stamp {
                page,
                x,
                y,
                w,
                h,
                stamp,
                custom_text,
                color,
            } => {
                let page_id = page_id(&pages_map, *page)?;
                annots::add_stamp_annot(
                    &mut doc,
                    page_id,
                    *x,
                    *y,
                    *w,
                    *h,
                    stamp,
                    custom_text.as_deref(),
                    color,
                    flatten,
                )?;
            }
        }
    }

    // When flatten=true, ops already baked into content (no Annots created).
    // Do not call flatten_all_annots — it would strip AcroForm Widgets.

    progress.emit(total_ops, total_ops, "Guardando…");
    doc.compress();
    doc.save(output_path)?;

    let mut result = OpResult::new(
        vec![output],
        total_pages,
        started.elapsed().as_millis() as u64,
    );
    if !warnings.is_empty() {
        result.warnings = warnings;
    }
    Ok(result)
}

fn page_id(
    pages: &std::collections::BTreeMap<u32, lopdf::ObjectId>,
    page: u32,
) -> Result<lopdf::ObjectId, AppError> {
    pages
        .get(&page)
        .copied()
        .ok_or_else(|| AppError::InvalidInput(format!("Página {page} no existe")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Dictionary, Object, Stream};
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "monkeypdf_edit_{tag}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_simple_pdf(path: &PathBuf, label: &str) {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content = format!("BT /F1 24 Tf 100 700 Td ({label}) Tj ET");
        let content_id = doc.add_object(Object::Stream(Stream::new(
            Dictionary::new(),
            content.into_bytes(),
        )));
        let mut font = Dictionary::new();
        font.set("Type", "Font");
        font.set("Subtype", "Type1");
        font.set("BaseFont", "Helvetica");
        font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        let font_id = doc.add_object(font);
        let mut fonts = Dictionary::new();
        fonts.set("F1", font_id);
        let mut resources = Dictionary::new();
        resources.set("Font", fonts);
        let mut page = Dictionary::new();
        page.set("Type", "Page");
        page.set("Parent", pages_id);
        page.set(
            "MediaBox",
            vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(595),
                Object::Integer(842),
            ],
        );
        page.set("Contents", content_id);
        page.set("Resources", resources);
        let page_id = doc.add_object(page);
        let mut pages = Dictionary::new();
        pages.set("Type", "Pages");
        pages.set("Count", 1_i64);
        pages.set("Kids", vec![Object::Reference(page_id)]);
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let mut catalog = Dictionary::new();
        catalog.set("Type", "Catalog");
        catalog.set("Pages", pages_id);
        let catalog_id = doc.add_object(catalog);
        doc.trailer.set("Root", catalog_id);
        doc.max_id = doc.objects.len() as u32;
        doc.save(path).unwrap();
    }

    #[test]
    fn list_and_replace_text() {
        let dir = temp_dir("replace");
        let pdf = dir.join("in.pdf");
        let out = dir.join("out.pdf");
        make_simple_pdf(&pdf, "OldLabel");
        let runs = list_text_runs(pdf.to_string_lossy().into(), 1).unwrap();
        assert!(runs.iter().any(|r| r.text.contains("OldLabel")));
        let run = runs.iter().find(|r| r.text.contains("OldLabel")).unwrap();
        edit_pdf(
            pdf.to_string_lossy().into(),
            out.to_string_lossy().into(),
            vec![EditOp::ReplaceText {
                page: 1,
                run_id: run.run_id,
                new_text: "NewLabel".into(),
                fit_width: None,
            }],
            false,
            None,
        )
        .unwrap();
        let doc = Document::load(&out).unwrap();
        let text = doc.extract_text(&[1]).unwrap();
        assert!(text.contains("NewLabel"), "{text}");
    }

    #[test]
    fn add_text_shapes_stamp_flatten() {
        let dir = temp_dir("draw");
        let pdf = dir.join("in.pdf");
        let out = dir.join("out.pdf");
        make_simple_pdf(&pdf, "Base");
        edit_pdf(
            pdf.to_string_lossy().into(),
            out.to_string_lossy().into(),
            vec![
                EditOp::AddText {
                    page: 1,
                    x: 72.0,
                    y: 640.0,
                    w: 200.0,
                    h: 40.0,
                    text: "Hola editor".into(),
                    font: "Helvetica".into(),
                    size: 14.0,
                    bold: true,
                    italic: false,
                    color: "#e11d48".into(),
                    align: "left".into(),
                    opacity: 1.0,
                },
                EditOp::Rect {
                    page: 1,
                    x: 50.0,
                    y: 50.0,
                    w: 80.0,
                    h: 40.0,
                    stroke: "#2563eb".into(),
                    fill: None,
                    stroke_width: 2.0,
                    opacity: 1.0,
                },
                EditOp::Stamp {
                    page: 1,
                    x: 200.0,
                    y: 400.0,
                    w: 140.0,
                    h: 48.0,
                    stamp: "aprobado".into(),
                    custom_text: None,
                    color: "#e11d48".into(),
                },
                EditOp::Highlight {
                    page: 1,
                    quads: vec![[100.0, 700.0, 60.0, 18.0]],
                    color: "#ffe066".into(),
                    opacity: 0.4,
                },
            ],
            true,
            None,
        )
        .unwrap();
        assert!(out.exists());
        let doc = Document::load(&out).unwrap();
        let page_id = *doc.get_pages().get(&1).unwrap();
        let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
        assert!(page.get(b"Annots").is_err());
        let content = doc.get_page_content(page_id).unwrap();
        let s = String::from_utf8_lossy(&content);
        assert!(s.contains("Hola editor") || s.contains("Hola"));
    }

    #[test]
    fn highlight_keeps_annots_when_not_flattened() {
        let dir = temp_dir("ann");
        let pdf = dir.join("in.pdf");
        let out = dir.join("out.pdf");
        make_simple_pdf(&pdf, "Mark");
        edit_pdf(
            pdf.to_string_lossy().into(),
            out.to_string_lossy().into(),
            vec![EditOp::Highlight {
                page: 1,
                quads: vec![[100.0, 700.0, 60.0, 18.0]],
                color: "#ffe066".into(),
                opacity: 0.4,
            }],
            false,
            None,
        )
        .unwrap();
        let doc = Document::load(&out).unwrap();
        let page_id = *doc.get_pages().get(&1).unwrap();
        let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
        assert!(page.get(b"Annots").is_ok());
    }
}
