use crate::error::{AppError, OpResult};
use crate::pdf_engine::{create_pdfium, ensure_dir, ensure_pdf_path};
use image::{DynamicImage, Rgba, RgbaImage};
use lopdf::Document;
use pdfium_render::prelude::*;
use std::path::PathBuf;
use std::time::Instant;

/// Compare two PDFs. `mode`: text | visual | both.
pub fn compare_pdfs(
    path_a: String,
    path_b: String,
    output_dir: String,
    mode: Option<String>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let a = ensure_pdf_path(&path_a)?;
    let b = ensure_pdf_path(&path_b)?;
    let out = ensure_dir(&output_dir)?;
    let mode = mode.unwrap_or_else(|| "both".into()).to_ascii_lowercase();
    if !matches!(mode.as_str(), "text" | "visual" | "both") {
        return Err(AppError::InvalidInput(
            "Modo inválido (text|visual|both)".into(),
        ));
    }

    let do_text = mode == "text" || mode == "both";
    let do_visual = mode == "visual" || mode == "both";

    let mut md = String::from("# Comparación de PDFs\n\n");
    md.push_str(&format!("- **A:** `{}`\n", a.display()));
    md.push_str(&format!("- **B:** `{}`\n\n", b.display()));

    let mut outputs = Vec::new();
    let mut page_count = 0u32;

    if do_text {
        let report = text_diff(&a, &b)?;
        md.push_str("## Diff de texto\n\n");
        md.push_str(&report.body);
        md.push('\n');
        page_count = page_count.max(report.pages);
    }

    if do_visual {
        let visual = visual_diff(&a, &b, &out)?;
        md.push_str("## Diff visual\n\n");
        md.push_str(&visual.summary);
        md.push('\n');
        outputs.extend(visual.image_paths);
        page_count = page_count.max(visual.pages);
    }

    let md_path = out.join("compare.md");
    std::fs::write(&md_path, md.as_bytes())?;
    outputs.insert(0, md_path.to_string_lossy().to_string());

    Ok(OpResult::new(
        outputs,
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

struct TextReport {
    body: String,
    pages: u32,
}

fn text_diff(a: &PathBuf, b: &PathBuf) -> Result<TextReport, AppError> {
    let doc_a = Document::load(a)?;
    let doc_b = Document::load(b)?;
    let pages_a: Vec<u32> = doc_a.get_pages().keys().copied().collect();
    let pages_b: Vec<u32> = doc_b.get_pages().keys().copied().collect();
    let max_pages = pages_a.len().max(pages_b.len()) as u32;

    let mut body = String::new();
    body.push_str(&format!(
        "Páginas A: {} · Páginas B: {}\n\n",
        pages_a.len(),
        pages_b.len()
    ));

    let mut diffs = 0u32;
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
        diffs += 1;
        body.push_str(&format!("### Página {i}\n\n"));
        if ta.is_empty() && !tb.is_empty() {
            body.push_str("- Solo en **B**\n\n");
            body.push_str("```\n");
            body.push_str(&truncate(&tb, 800));
            body.push_str("\n```\n\n");
        } else if !ta.is_empty() && tb.is_empty() {
            body.push_str("- Solo en **A**\n\n");
            body.push_str("```\n");
            body.push_str(&truncate(&ta, 800));
            body.push_str("\n```\n\n");
        } else {
            body.push_str("- Contenido distinto\n\n");
            body.push_str("**A**\n\n```\n");
            body.push_str(&truncate(&ta, 500));
            body.push_str("\n```\n\n**B**\n\n```\n");
            body.push_str(&truncate(&tb, 500));
            body.push_str("\n```\n\n");
        }
    }

    if diffs == 0 {
        body.push_str("Sin diferencias de texto detectadas.\n");
    } else {
        body.insert_str(0, &format!("**{diffs}** página(s) con diferencias.\n\n"));
    }

    Ok(TextReport {
        body,
        pages: max_pages,
    })
}

struct VisualReport {
    summary: String,
    image_paths: Vec<String>,
    pages: u32,
}

fn visual_diff(a: &PathBuf, b: &PathBuf, out: &PathBuf) -> Result<VisualReport, AppError> {
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
    let target_width = 720u32;
    let render_config_for = |page_width: f32| {
        let scale = target_width as f32 / page_width.max(1.0);
        PdfRenderConfig::new().scale_page_by_factor(scale)
    };

    let mut image_paths = Vec::new();
    let mut changed = 0u32;
    let mut summary = String::new();

    for i in 0..max_pages {
        let page_num = i + 1;
        let img_a = if i < n_a {
            let page = doc_a
                .pages()
                .get(i as u16)
                .map_err(|e| AppError::Pdfium(e.to_string()))?;
            let cfg = render_config_for(page.width().value);
            let image = page
                .render_with_config(&cfg)
                .map_err(|e| AppError::Pdfium(e.to_string()))?
                .as_image();
            Some(image)
        } else {
            None
        };
        let img_b = if i < n_b {
            let page = doc_b
                .pages()
                .get(i as u16)
                .map_err(|e| AppError::Pdfium(e.to_string()))?;
            let cfg = render_config_for(page.width().value);
            let image = page
                .render_with_config(&cfg)
                .map_err(|e| AppError::Pdfium(e.to_string()))?
                .as_image();
            Some(image)
        } else {
            None
        };

        let diff_img = match (img_a, img_b) {
            (Some(a), Some(b)) => {
                let d = abs_diff_image(&a, &b);
                let changed_px = count_hot_pixels(&d);
                if changed_px < 40 {
                    continue;
                }
                changed += 1;
                summary.push_str(&format!(
                    "- Página {page_num}: ~{changed_px} px distintos\n"
                ));
                d
            }
            (Some(a), None) => {
                changed += 1;
                summary.push_str(&format!("- Página {page_num}: solo en A\n"));
                a.to_rgba8()
            }
            (None, Some(b)) => {
                changed += 1;
                summary.push_str(&format!("- Página {page_num}: solo en B\n"));
                b.to_rgba8()
            }
            (None, None) => continue,
        };

        let path = out.join(format!("diff_page{page_num:03}.jpg"));
        DynamicImage::ImageRgba8(diff_img)
            .to_rgb8()
            .save(&path)
            .map_err(|e| AppError::Image(e.to_string()))?;
        image_paths.push(path.to_string_lossy().to_string());
    }

    if changed == 0 {
        summary.push_str("Sin diferencias visuales significativas.\n");
    } else {
        summary.insert_str(0, &format!("**{changed}** página(s) con diff visual.\n\n"));
    }

    Ok(VisualReport {
        summary,
        image_paths,
        pages: max_pages,
    })
}

fn abs_diff_image(a: &DynamicImage, b: &DynamicImage) -> RgbaImage {
    let a = a.to_rgba8();
    let b = b.to_rgba8();
    let w = a.width().min(b.width());
    let h = a.height().min(b.height());
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let pa = a.get_pixel(x, y).0;
            let pb = b.get_pixel(x, y).0;
            let dr = pa[0].abs_diff(pb[0]);
            let dg = pa[1].abs_diff(pb[1]);
            let db = pa[2].abs_diff(pb[2]);
            let mag = dr.max(dg).max(db);
            if mag > 18 {
                // Highlight diffs in banana-ish yellow on dark
                out.put_pixel(x, y, Rgba([255, 220, 40, 255]));
            } else {
                // Dim average of originals
                out.put_pixel(
                    x,
                    y,
                    Rgba([
                        ((pa[0] as u16 + pb[0] as u16) / 2) as u8 / 2,
                        ((pa[1] as u16 + pb[1] as u16) / 2) as u8 / 2,
                        ((pa[2] as u16 + pb[2] as u16) / 2) as u8 / 2,
                        255,
                    ]),
                );
            }
        }
    }
    out
}

fn count_hot_pixels(img: &RgbaImage) -> u32 {
    img.pixels()
        .filter(|p| p.0[0] > 200 && p.0[1] > 180 && p.0[2] < 80)
        .count() as u32
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect::<String>() + "…"
}
