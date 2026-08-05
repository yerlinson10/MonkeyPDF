use crate::error::{AppError, OpResult};
use crate::pdf_engine::{create_pdfium, ensure_parent_dir, ensure_pdf_path};
use lopdf::Document;
use std::path::Path;
use std::time::Instant;

/// Extract PDF text to Markdown with light heading/paragraph heuristics.
pub fn pdf_to_markdown(path: String, output: String) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    // Prefer lopdf extract_text (structured enough); fall back to PDFium page text.
    let markdown = match extract_with_lopdf(&input) {
        Ok(md) if !md.trim().is_empty() => md,
        _ => extract_with_pdfium(&input)?,
    };

    std::fs::write(output_path, markdown.as_bytes())?;

    let page_count = Document::load(&input)
        .map(|d| d.get_pages().len() as u32)
        .unwrap_or(1);

    Ok(OpResult::new(
        vec![output],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn extract_with_lopdf(path: &Path) -> Result<String, AppError> {
    let doc = Document::load(path)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Desbloquea el PDF antes de exportar a Markdown".into(),
        ));
    }
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    let raw = doc
        .extract_text(&pages)
        .map_err(|e| AppError::Pdf(e.to_string()))?;
    Ok(to_markdown(&raw))
}

fn extract_with_pdfium(path: &Path) -> Result<String, AppError> {
    let pdfium = create_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| AppError::Pdfium(e.to_string()))?;

    let mut chunks = Vec::new();
    for (i, page) in document.pages().iter().enumerate() {
        let text = page
            .text()
            .map_err(|e| AppError::Pdfium(e.to_string()))?
            .all();
        if text.trim().is_empty() {
            continue;
        }
        chunks.push(format!("<!-- página {} -->\n{}", i + 1, text.trim()));
    }

    if chunks.is_empty() {
        return Err(AppError::Pdf(
            "No se pudo extraer texto (¿PDF escaneado? Prueba OCR en Fase 3)".into(),
        ));
    }

    Ok(to_markdown(&chunks.join("\n\n")))
}

fn to_markdown(raw: &str) -> String {
    let mut out = String::new();
    out.push_str("# Documento\n\n");

    for block in raw.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        // Heuristic: short line, mostly letters, no trailing period → heading
        let lines: Vec<&str> = block.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            continue;
        }

        if lines.len() == 1 {
            let line = lines[0];
            let looks_heading = line.chars().count() <= 80
                && !line.ends_with('.')
                && !line.ends_with(',')
                && line.chars().any(|c| c.is_alphabetic());
            if looks_heading && line == line.to_uppercase() && line.chars().count() > 3 {
                out.push_str(&format!("## {}\n\n", title_case(line)));
                continue;
            }
            if looks_heading && line.chars().count() <= 60 {
                out.push_str(&format!("### {}\n\n", line));
                continue;
            }
            out.push_str(line);
            out.push_str("\n\n");
            continue;
        }

        // Table-ish: lines with multiple spaces/tabs columns
        if lines.iter().filter(|l| l.contains('\t') || l.matches("  ").count() >= 2).count()
            >= lines.len().saturating_sub(1).max(1)
            && lines.len() >= 2
        {
            out.push_str(&format_table(&lines));
            out.push('\n');
            continue;
        }

        for line in &lines {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }

    out
}

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_table(lines: &[&str]) -> String {
    let rows: Vec<Vec<String>> = lines
        .iter()
        .map(|l| {
            if l.contains('\t') {
                l.split('\t').map(|c| c.trim().to_string()).collect()
            } else {
                l.split("  ")
                    .map(|c| c.trim().to_string())
                    .filter(|c| !c.is_empty())
                    .collect()
            }
        })
        .collect();

    if rows.is_empty() {
        return String::new();
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
    let mut md = String::new();
    // header = first row
    let header = pad_row(&rows[0], cols);
    md.push('|');
    md.push_str(&header.join("|"));
    md.push_str("|\n|");
    md.push_str(&vec!["---"; cols].join("|"));
    md.push_str("|\n");
    for row in rows.iter().skip(1) {
        let cells = pad_row(row, cols);
        md.push('|');
        md.push_str(&cells.join("|"));
        md.push_str("|\n");
    }
    md
}

fn pad_row(row: &[String], cols: usize) -> Vec<String> {
    let mut out = row.to_vec();
    while out.len() < cols {
        out.push(String::new());
    }
    out.truncate(cols);
    out
}
