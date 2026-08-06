use crate::error::{AppError, OpResult};
use crate::pdf_engine::{create_pdfium, ensure_parent_dir, ensure_pdf_path, merge_pdfs};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Locate Tesseract OCR on this machine.
pub fn find_tesseract() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files\Tesseract-OCR\tesseract.exe",
            r"C:\Program Files (x86)\Tesseract-OCR\tesseract.exe",
        ];
        for c in candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
    }

    if let Some(p) = which("tesseract") {
        return Some(p);
    }

    #[cfg(target_os = "macos")]
    {
        for c in [
            "/opt/homebrew/bin/tesseract",
            "/usr/local/bin/tesseract",
        ] {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for c in ["/usr/bin/tesseract", "/usr/local/bin/tesseract"] {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

fn which(cmd: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = Command::new("where")
            .arg(cmd)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().next()?.trim();
        if line.is_empty() {
            return None;
        }
        Some(PathBuf::from(line))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("which").arg(cmd).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().next()?.trim();
        if line.is_empty() {
            return None;
        }
        Some(PathBuf::from(line))
    }
}

pub fn tesseract_available() -> bool {
    find_tesseract().is_some()
}

/// OCR a PDF. `mode`: markdown | txt | searchable_pdf. `lang` e.g. spa+eng.
pub fn ocr_pdf(
    path: String,
    output: String,
    lang: Option<String>,
    mode: Option<String>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = PathBuf::from(&output);
    ensure_parent_dir(&output_path)?;

    let tess = find_tesseract().ok_or_else(|| {
        AppError::InvalidInput(
            "Tesseract no encontrado. Instálalo (tesseract-ocr) con packs spa/eng y reinicia MonkeyPDF."
                .into(),
        )
    })?;

    let lang = lang.unwrap_or_else(|| "spa+eng".into());
    let mode = mode.unwrap_or_else(|| "markdown".into()).to_ascii_lowercase();
    if !matches!(mode.as_str(), "markdown" | "txt" | "searchable_pdf") {
        return Err(AppError::InvalidInput(
            "Modo OCR inválido (markdown|txt|searchable_pdf)".into(),
        ));
    }

    let work = std::env::temp_dir().join(format!(
        "monkeypdf_ocr_{}_{}",
        std::process::id(),
        started.elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&work)?;

    let result = (|| -> Result<OpResult, AppError> {
        let page_images = render_pages_png(&input, &work, 200)?;
        let page_count = page_images.len() as u32;
        if page_count == 0 {
            return Err(AppError::InvalidInput("PDF sin páginas".into()));
        }

        match mode.as_str() {
            "searchable_pdf" => {
                let mut pdf_parts = Vec::new();
                for (i, img) in page_images.iter().enumerate() {
                    let stem = work.join(format!("ocr_page_{i:03}"));
                    run_tesseract(&tess, img, &stem, &lang, Some("pdf"))?;
                    let pdf = stem.with_extension("pdf");
                    if !pdf.exists() {
                        return Err(AppError::Pdf(format!(
                            "Tesseract no generó PDF para página {}",
                            i + 1
                        )));
                    }
                    pdf_parts.push(pdf.to_string_lossy().to_string());
                }
                if pdf_parts.len() == 1 {
                    std::fs::copy(&pdf_parts[0], &output_path)?;
                } else {
                    merge_pdfs(pdf_parts, output.clone())?;
                }
            }
            "txt" | "markdown" => {
                let mut chunks = Vec::new();
                for (i, img) in page_images.iter().enumerate() {
                    let stem = work.join(format!("ocr_page_{i:03}"));
                    run_tesseract(&tess, img, &stem, &lang, None)?;
                    let txt_path = stem.with_extension("txt");
                    let text = if txt_path.exists() {
                        std::fs::read_to_string(&txt_path).unwrap_or_default()
                    } else {
                        String::new()
                    };
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if mode == "markdown" {
                        chunks.push(format!("## Página {}\n\n{trimmed}", i + 1));
                    } else {
                        chunks.push(format!("--- Página {} ---\n{trimmed}", i + 1));
                    }
                }
                let body = if mode == "markdown" {
                    format!("# OCR\n\n{}\n", chunks.join("\n\n"))
                } else {
                    chunks.join("\n\n")
                };
                std::fs::write(&output_path, body.as_bytes())?;
            }
            _ => unreachable!(),
        }

        Ok(OpResult::new(
            vec![output],
            page_count,
            started.elapsed().as_millis() as u64,
        ))
    })();

    let _ = std::fs::remove_dir_all(&work);
    result
}

fn render_pages_png(pdf: &Path, work: &Path, dpi: u32) -> Result<Vec<PathBuf>, AppError> {
    let pdfium = create_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(pdf, None)
        .map_err(|e| AppError::Pdfium(e.to_string()))?;

    let scale = dpi as f32 / 72.0;
    let render_config = PdfRenderConfig::new().scale_page_by_factor(scale);
    let mut out = Vec::new();

    for (index, page) in document.pages().iter().enumerate() {
        let image = page
            .render_with_config(&render_config)
            .map_err(|e| AppError::Pdfium(e.to_string()))?
            .as_image();
        let path = work.join(format!("page_{index:03}.png"));
        image
            .save_with_format(&path, image::ImageFormat::Png)
            .map_err(|e| AppError::Image(e.to_string()))?;
        out.push(path);
    }
    Ok(out)
}

fn run_tesseract(
    tess: &Path,
    image: &Path,
    output_stem: &Path,
    lang: &str,
    pdf_config: Option<&str>,
) -> Result<(), AppError> {
    let mut cmd = Command::new(tess);
    cmd.arg(image).arg(output_stem).args(["-l", lang]);
    if let Some(cfg) = pdf_config {
        cmd.arg(cfg);
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .map_err(|e| AppError::Pdf(format!("No se pudo ejecutar Tesseract: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(AppError::Pdf(format!(
            "Tesseract falló: {stderr} {stdout}"
        )));
    }
    Ok(())
}
