use crate::error::{AppError, OpResult};
use crate::pdf_engine::{create_pdfium, ensure_dir, ensure_pdf_path};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use image::codecs::jpeg::JpegEncoder;
use image::ImageEncoder;
use lopdf::Document;
use pdfium_render::prelude::*;
use serde::Serialize;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextChange {
    pub page: u32,
    /// only_a | only_b | changed
    pub kind: String,
    pub text_a: String,
    pub text_b: String,
}

/// Underline / highlight band in normalized page coords (0–1, top-left origin).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffUnderline {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    /// Band height for translucent highlight (not just a hairline).
    pub h: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualPageDiff {
    pub page: u32,
    pub changed_px: u32,
    /// JPEG data URL of the visual diff heatmap (mapa). Empty if skipped.
    pub diff_data_url: String,
    /// Subrayados de zonas distintas (mismo layout en A y B).
    pub underlines: Vec<DiffUnderline>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareReport {
    pub pages_a: u32,
    pub pages_b: u32,
    pub text_changes: Vec<TextChange>,
    pub visual_pages: Vec<VisualPageDiff>,
    pub elapsed_ms: u64,
    /// Paths written when export_dir was provided
    pub output_paths: Vec<String>,
}

/// Interactive compare: structured report (+ optional export folder).
pub fn compare_report(
    path_a: String,
    path_b: String,
    mode: Option<String>,
    export_dir: Option<String>,
) -> Result<CompareReport, AppError> {
    let started = Instant::now();
    let a = ensure_pdf_path(&path_a)?;
    let b = ensure_pdf_path(&path_b)?;
    let mode = mode.unwrap_or_else(|| "both".into()).to_ascii_lowercase();
    if !matches!(mode.as_str(), "text" | "visual" | "both") {
        return Err(AppError::InvalidInput(
            "Modo inválido (text|visual|both)".into(),
        ));
    }

    let do_text = mode == "text" || mode == "both";
    let do_visual = mode == "visual" || mode == "both";
    let want_export = export_dir.is_some();

    let mut text_changes = Vec::new();
    let (mut pages_a, mut pages_b) = if do_text {
        let report = text_diff_structured(&a, &b)?;
        text_changes = report.changes;
        (report.pages_a, report.pages_b)
    } else {
        let doc_a = Document::load(&a)?;
        let doc_b = Document::load(&b)?;
        (
            doc_a.get_pages().len() as u32,
            doc_b.get_pages().len() as u32,
        )
    };

    let mut visual_pages = Vec::new();
    let mut exported_images = Vec::new();
    if do_visual {
        let out = match &export_dir {
            Some(dir) => Some(ensure_dir(dir)?),
            None => None,
        };
        let visual = visual_diff_structured(&a, &b, out.as_ref(), want_export)?;
        pages_a = pages_a.max(visual.pages_a);
        pages_b = pages_b.max(visual.pages_b);
        visual_pages = visual.pages;
        exported_images = visual.saved_paths;
    }

    let mut output_paths = Vec::new();
    if let Some(dir) = export_dir {
        let out = ensure_dir(&dir)?;
        let mut md = String::from("# Comparación de PDFs\n\n");
        md.push_str(&format!("- **A:** `{}`\n", a.display()));
        md.push_str(&format!("- **B:** `{}`\n\n", b.display()));
        md.push_str(&format!(
            "Páginas A: {pages_a} · Páginas B: {pages_b}\n\n"
        ));
        if do_text {
            md.push_str("## Diff de texto\n\n");
            if text_changes.is_empty() {
                md.push_str("Sin diferencias de texto detectadas.\n\n");
            } else {
                md.push_str(&format!(
                    "**{}** página(s) con diferencias.\n\n",
                    text_changes.len()
                ));
                for c in &text_changes {
                    md.push_str(&format!("### Página {}\n\n", c.page));
                    match c.kind.as_str() {
                        "only_a" => {
                            md.push_str("- Solo en **A**\n\n```\n");
                            md.push_str(&truncate(&c.text_a, 800));
                            md.push_str("\n```\n\n");
                        }
                        "only_b" => {
                            md.push_str("- Solo en **B**\n\n```\n");
                            md.push_str(&truncate(&c.text_b, 800));
                            md.push_str("\n```\n\n");
                        }
                        _ => {
                            md.push_str("- Contenido distinto\n\n**A**\n\n```\n");
                            md.push_str(&truncate(&c.text_a, 500));
                            md.push_str("\n```\n\n**B**\n\n```\n");
                            md.push_str(&truncate(&c.text_b, 500));
                            md.push_str("\n```\n\n");
                        }
                    }
                }
            }
        }
        if do_visual {
            md.push_str("## Diff visual\n\n");
            if visual_pages.is_empty() {
                md.push_str("Sin diferencias visuales significativas.\n");
            } else {
                md.push_str(&format!(
                    "**{}** página(s) con diff visual.\n\n",
                    visual_pages.len()
                ));
                for v in &visual_pages {
                    md.push_str(&format!(
                        "- Página {}: ~{} px distintos · {} subrayado(s)\n",
                        v.page,
                        v.changed_px,
                        v.underlines.len()
                    ));
                }
            }
        }
        let md_path = out.join("compare.md");
        std::fs::write(&md_path, md.as_bytes())?;
        output_paths.push(md_path.to_string_lossy().to_string());
        output_paths.extend(exported_images);
    }

    Ok(CompareReport {
        pages_a,
        pages_b,
        text_changes,
        visual_pages,
        elapsed_ms: started.elapsed().as_millis() as u64,
        output_paths,
    })
}

/// Legacy export-only entry used by older IPC / tests.
pub fn compare_pdfs(
    path_a: String,
    path_b: String,
    output_dir: String,
    mode: Option<String>,
) -> Result<OpResult, AppError> {
    let report = compare_report(path_a, path_b, mode, Some(output_dir))?;
    let pages = report.pages_a.max(report.pages_b);
    Ok(OpResult::new(
        report.output_paths,
        pages,
        report.elapsed_ms,
    ))
}

struct TextStructured {
    pages_a: u32,
    pages_b: u32,
    changes: Vec<TextChange>,
}

fn text_diff_structured(a: &PathBuf, b: &PathBuf) -> Result<TextStructured, AppError> {
    let doc_a = Document::load(a)?;
    let doc_b = Document::load(b)?;
    let pages_a: Vec<u32> = doc_a.get_pages().keys().copied().collect();
    let pages_b: Vec<u32> = doc_b.get_pages().keys().copied().collect();
    let max_pages = pages_a.len().max(pages_b.len()) as u32;

    let mut changes = Vec::new();
    for i in 1..=max_pages {
        let ta = if pages_a.contains(&i) {
            doc_a
                .extract_text(&[i])
                .unwrap_or_default()
                .trim()
                .to_string()
        } else {
            String::new()
        };
        let tb = if pages_b.contains(&i) {
            doc_b
                .extract_text(&[i])
                .unwrap_or_default()
                .trim()
                .to_string()
        } else {
            String::new()
        };

        if ta == tb {
            continue;
        }
        let kind = if ta.is_empty() && !tb.is_empty() {
            "only_b"
        } else if !ta.is_empty() && tb.is_empty() {
            "only_a"
        } else {
            "changed"
        };
        changes.push(TextChange {
            page: i,
            kind: kind.into(),
            text_a: truncate(&ta, 1200),
            text_b: truncate(&tb, 1200),
        });
    }

    Ok(TextStructured {
        pages_a: pages_a.len() as u32,
        pages_b: pages_b.len() as u32,
        changes,
    })
}

struct VisualStructured {
    pages_a: u32,
    pages_b: u32,
    pages: Vec<VisualPageDiff>,
    saved_paths: Vec<String>,
}

/// Fast visual compare: low-res render, single pass, underlines instead of tinted images.
fn visual_diff_structured(
    a: &PathBuf,
    b: &PathBuf,
    out: Option<&PathBuf>,
    build_heatmap: bool,
) -> Result<VisualStructured, AppError> {
    let pdfium = create_pdfium()?;
    let doc_a = pdfium
        .load_pdf_from_file(a, None)
        .map_err(|e| AppError::Pdfium(e.to_string()))?;
    let doc_b = pdfium
        .load_pdf_from_file(b, None)
        .map_err(|e| AppError::Pdfium(e.to_string()))?;

    let n_a = doc_a.pages().len() as u32;
    let n_b = doc_b.pages().len() as u32;
    let max_pages = n_a.max(n_b);
    // Low res: enough for underlines, much faster than 720.
    let target_width = 400u32;
    let render_config_for = |page_width: f32| {
        let scale = target_width as f32 / page_width.max(1.0);
        PdfRenderConfig::new().scale_page_by_factor(scale)
    };

    let mut pages = Vec::new();
    let mut saved_paths = Vec::new();

    for i in 0..max_pages {
        let page_num = i + 1;
        let img_a = if i < n_a {
            let page = doc_a
                .pages()
                .get(i as u16)
                .map_err(|e| AppError::Pdfium(e.to_string()))?;
            let cfg = render_config_for(page.width().value);
            let bitmap = page
                .render_with_config(&cfg)
                .map_err(|e| AppError::Pdfium(e.to_string()))?;
            Some(bitmap.as_image().to_rgb8())
        } else {
            None
        };
        let img_b = if i < n_b {
            let page = doc_b
                .pages()
                .get(i as u16)
                .map_err(|e| AppError::Pdfium(e.to_string()))?;
            let cfg = render_config_for(page.width().value);
            let bitmap = page
                .render_with_config(&cfg)
                .map_err(|e| AppError::Pdfium(e.to_string()))?;
            Some(bitmap.as_image().to_rgb8())
        } else {
            None
        };

        let analysis = match (img_a.as_ref(), img_b.as_ref()) {
            (Some(ia), Some(ib)) => analyze_pair(ia, ib),
            (Some(ia), None) => Analysis {
                changed_px: ia.width() * ia.height(),
                underlines: vec![DiffUnderline {
                    x: 0.04,
                    y: 0.96,
                    w: 0.92,
                    h: 0.004,
                }],
            },
            (None, Some(ib)) => Analysis {
                changed_px: ib.width() * ib.height(),
                underlines: vec![DiffUnderline {
                    x: 0.04,
                    y: 0.96,
                    w: 0.92,
                    h: 0.004,
                }],
            },
            (None, None) => continue,
        };

        if analysis.changed_px < 28 {
            continue;
        }

        let mut diff_data_url = String::new();
        if build_heatmap {
            let heat = match (img_a.as_ref(), img_b.as_ref()) {
                (Some(ia), Some(ib)) => quick_heatmap(ia, ib),
                (Some(ia), None) => tint_rgb(ia, [180, 40, 40]),
                (None, Some(ib)) => tint_rgb(ib, [34, 160, 72]),
                _ => continue,
            };
            diff_data_url = rgb_to_jpeg_data_url(&heat, 62)?;
            if let Some(dir) = out {
                let path = dir.join(format!("diff_page{page_num:03}.jpg"));
                heat.save(&path)
                    .map_err(|e| AppError::Image(e.to_string()))?;
                saved_paths.push(path.to_string_lossy().to_string());
            }
        }

        pages.push(VisualPageDiff {
            page: page_num,
            changed_px: analysis.changed_px,
            diff_data_url,
            underlines: analysis.underlines,
        });
    }

    Ok(VisualStructured {
        pages_a: n_a,
        pages_b: n_b,
        pages,
        saved_paths,
    })
}

struct Analysis {
    changed_px: u32,
    underlines: Vec<DiffUnderline>,
}

const DIFF_THRESHOLD: u8 = 22;
/// Sample every Nth pixel for speed (still dense enough for underlines).
const SAMPLE: u32 = 2;

fn analyze_pair(a: &image::RgbImage, b: &image::RgbImage) -> Analysis {
    let w = a.width().min(b.width());
    let h = a.height().min(b.height());
    if w == 0 || h == 0 {
        return Analysis {
            changed_px: 0,
            underlines: vec![],
        };
    }

    let aw = a.width();
    let bw = b.width();
    let a_raw = a.as_raw();
    let b_raw = b.as_raw();

    // Per-row: min/max x of changed samples (in full-res coords).
    let mut row_min = vec![w; h as usize];
    let mut row_max = vec![0u32; h as usize];
    let mut row_hits = vec![0u32; h as usize];
    let mut changed_px = 0u32;

    let mut y = 0u32;
    while y < h {
        let mut x = 0u32;
        while x < w {
            let ai = ((y * aw + x) * 3) as usize;
            let bi = ((y * bw + x) * 3) as usize;
            let dr = a_raw[ai].abs_diff(b_raw[bi]);
            let dg = a_raw[ai + 1].abs_diff(b_raw[bi + 1]);
            let db = a_raw[ai + 2].abs_diff(b_raw[bi + 2]);
            if dr.max(dg).max(db) > DIFF_THRESHOLD {
                changed_px += SAMPLE * SAMPLE;
                let yi = y as usize;
                row_hits[yi] += 1;
                if x < row_min[yi] {
                    row_min[yi] = x;
                }
                if x > row_max[yi] {
                    row_max[yi] = x;
                }
            }
            x += SAMPLE;
        }
        y += SAMPLE;
    }

    let underlines = rows_to_underlines(w, h, &row_hits, &row_min, &row_max);
    Analysis {
        changed_px,
        underlines,
    }
}

fn rows_to_underlines(
    w: u32,
    h: u32,
    hits: &[u32],
    mins: &[u32],
    maxs: &[u32],
) -> Vec<DiffUnderline> {
    let min_hits = 3u32;
    let gap_tol = 4usize; // merge nearby hot rows into one line
    let mut out = Vec::new();
    let mut y = 0usize;
    while y < hits.len() {
        if hits[y] < min_hits {
            y += 1;
            continue;
        }
        let start = y;
        let mut min_x = mins[y];
        let mut max_x = maxs[y];
        y += 1;
        let mut gap = 0usize;
        while y < hits.len() {
            if hits[y] >= min_hits {
                min_x = min_x.min(mins[y]);
                max_x = max_x.max(maxs[y]);
                gap = 0;
                y += 1;
            } else if gap < gap_tol {
                gap += 1;
                y += 1;
            } else {
                break;
            }
        }
        // rewind trailing gap
        y -= gap;
        let end = y.max(start + 1);
        let band_px = end.saturating_sub(start);
        if band_px < 3 && max_x.saturating_sub(min_x) < w / 16 {
            continue;
        }
        let pad = (w as f32 * 0.006).max(1.0);
        let x0 = ((min_x as f32 - pad) / w as f32).clamp(0.0, 1.0);
        let x1 = ((max_x as f32 + pad * 2.0) / w as f32).clamp(0.0, 1.0);
        // Thin underline at the bottom of the changed band — not a full-height fill.
        let y_line = ((end as f32) / h as f32).clamp(0.002, 0.998);
        let stroke_h = (3.0 / h as f32).clamp(0.0025, 0.005);
        out.push(DiffUnderline {
            x: x0,
            y: (y_line - stroke_h).max(0.0),
            w: (x1 - x0).max(0.01),
            h: stroke_h,
        });
    }
    if out.len() > 60 {
        out.truncate(60);
    }
    out
}

fn quick_heatmap(a: &image::RgbImage, b: &image::RgbImage) -> image::RgbImage {
    let w = a.width().min(b.width());
    let h = a.height().min(b.height());
    let mut out = image::RgbImage::new(w, h);
    let aw = a.width();
    let bw = b.width();
    let a_raw = a.as_raw();
    let b_raw = b.as_raw();
    for y in 0..h {
        for x in 0..w {
            let ai = ((y * aw + x) * 3) as usize;
            let bi = ((y * bw + x) * 3) as usize;
            let dr = a_raw[ai].abs_diff(b_raw[bi]);
            let dg = a_raw[ai + 1].abs_diff(b_raw[bi + 1]);
            let db = a_raw[ai + 2].abs_diff(b_raw[bi + 2]);
            if dr.max(dg).max(db) > DIFF_THRESHOLD {
                out.put_pixel(x, y, image::Rgb([255, 220, 40]));
            } else {
                out.put_pixel(
                    x,
                    y,
                    image::Rgb([
                        ((a_raw[ai] as u16 + b_raw[bi] as u16) / 4) as u8,
                        ((a_raw[ai + 1] as u16 + b_raw[bi + 1] as u16) / 4) as u8,
                        ((a_raw[ai + 2] as u16 + b_raw[bi + 2] as u16) / 4) as u8,
                    ]),
                );
            }
        }
    }
    out
}

fn tint_rgb(base: &image::RgbImage, mark: [u8; 3]) -> image::RgbImage {
    let mut out = base.clone();
    for p in out.pixels_mut() {
        p.0[0] = ((p.0[0] as u16 + mark[0] as u16) / 2) as u8;
        p.0[1] = ((p.0[1] as u16 + mark[1] as u16) / 2) as u8;
        p.0[2] = ((p.0[2] as u16 + mark[2] as u16) / 2) as u8;
    }
    out
}

fn rgb_to_jpeg_data_url(img: &image::RgbImage, quality: u8) -> Result<String, AppError> {
    let mut buf = Cursor::new(Vec::new());
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| AppError::Image(e.to_string()))?;
    let b64 = B64.encode(buf.into_inner());
    Ok(format!("data:image/jpeg;base64,{b64}"))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect::<String>() + "…"
}
