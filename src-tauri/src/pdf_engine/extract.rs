use crate::error::{AppError, OpResult};
use crate::pdf_engine::{create_pdfium, ensure_dir, ensure_parent_dir, ensure_pdf_path, Progress};
use image::{DynamicImage, ImageFormat, RgbImage};
use lopdf::{Document, Object, Stream};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Extract embedded Image XObjects (not page rasterization) to `output_dir`.
pub fn extract_images(
    path: String,
    output_dir: String,
    progress: Option<Progress>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let progress = progress.unwrap_or_else(Progress::none);
    let input = ensure_pdf_path(&path)?;
    let out_dir = ensure_dir(&output_dir)?;

    let doc = Document::load(&input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Desbloquea el PDF antes de extraer imágenes".into(),
        ));
    }

    let page_count = doc.get_pages().len() as u32;
    let object_ids: Vec<_> = doc.objects.keys().copied().collect();
    let total = object_ids.len().max(1) as u32;
    let mut outputs = Vec::new();
    let mut skipped = 0u32;
    let mut idx = 0u32;

    for (step, id) in object_ids.iter().enumerate() {
        progress.tick(
            (step as u32) + 1,
            total,
            format!("Revisando objeto {}/{}", step + 1, total),
        )?;
        let Some(Object::Stream(stream)) = doc.objects.get(id).cloned() else {
            continue;
        };
        if !is_image_xobject(&stream) {
            continue;
        }
        match decode_image_xobject(&stream) {
            Some((ext, bytes)) => {
                idx += 1;
                let name = format!("img_{:03}_{}_{}.{}", idx, id.0, id.1, ext);
                let out_path = unique_file(&out_dir.join(name));
                fs::write(&out_path, &bytes)?;
                outputs.push(out_path.to_string_lossy().to_string());
            }
            None => skipped += 1,
        }
    }

    if outputs.is_empty() {
        return Err(AppError::Pdf(
            if skipped > 0 {
                format!(
                    "Se encontraron {skipped} imagen(es) en formatos no soportados (JPX/CMYK/CCITT). Sin exportables RGB/JPEG."
                )
            } else {
                "No hay imágenes embebidas en este PDF (puede ser solo texto o páginas rasterizadas).".into()
            },
        ));
    }

    let mut result = OpResult::new(
        outputs,
        page_count,
        started.elapsed().as_millis() as u64,
    );
    if skipped > 0 {
        result.partial = true;
        result.warnings = vec![format!(
            "{skipped} imagen(es) omitidas (filtro/espacio de color no soportado)"
        )];
    }
    Ok(result)
}

/// Extract plain text to a `.txt` file (lopdf, then PDFium fallback).
pub fn extract_text(
    path: String,
    output: String,
    progress: Option<Progress>,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let progress = progress.unwrap_or_else(Progress::none);
    let input = ensure_pdf_path(&path)?;
    let output_path = PathBuf::from(&output);
    ensure_parent_dir(&output_path)?;

    progress.emit(0, 1, "Extrayendo texto");
    let text = match extract_text_lopdf(&input) {
        Ok(t) if !t.trim().is_empty() => t,
        Ok(_) | Err(_) => extract_text_pdfium(&input, &progress)?,
    };
    progress.tick(1, 1, "Escribiendo TXT")?;

    if text.trim().is_empty() {
        return Err(AppError::Pdf(
            "No se pudo extraer texto (¿PDF escaneado? Prueba OCR)".into(),
        ));
    }

    fs::write(&output_path, text.as_bytes())?;

    let page_count = Document::load(&input)
        .map(|d| d.get_pages().len() as u32)
        .unwrap_or(1);

    Ok(OpResult::new(
        vec![output],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn extract_text_lopdf(path: &Path) -> Result<String, AppError> {
    let doc = Document::load(path)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Desbloquea el PDF antes de extraer texto".into(),
        ));
    }
    let mut pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    pages.sort_unstable();
    let mut chunks = Vec::new();
    for n in pages {
        let raw = doc
            .extract_text(&[n])
            .map_err(|e| AppError::Pdf(e.to_string()))?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        chunks.push(format!("--- Página {n} ---\n{trimmed}"));
    }
    Ok(chunks.join("\n\n"))
}

fn extract_text_pdfium(path: &Path, progress: &Progress) -> Result<String, AppError> {
    let pdfium = create_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| AppError::Pdfium(e.to_string()))?;
    let page_count = document.pages().len() as u32;
    let mut chunks = Vec::new();
    for (i, page) in document.pages().iter().enumerate() {
        progress.tick(
            (i as u32) + 1,
            page_count.max(1),
            format!("Texto página {}/{}", i + 1, page_count),
        )?;
        let text = page
            .text()
            .map_err(|e| AppError::Pdfium(e.to_string()))?
            .all();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        chunks.push(format!("--- Página {} ---\n{trimmed}", i + 1));
    }
    if chunks.is_empty() {
        return Err(AppError::Pdf(
            "No se pudo extraer texto (¿PDF escaneado? Prueba OCR)".into(),
        ));
    }
    Ok(chunks.join("\n\n"))
}

fn is_image_xobject(stream: &Stream) -> bool {
    match stream.dict.get(b"Subtype") {
        Ok(Object::Name(name)) => name == b"Image",
        _ => false,
    }
}

fn decode_image_xobject(stream: &Stream) -> Option<(String, Vec<u8>)> {
    let width = match stream.dict.get(b"Width").ok()? {
        Object::Integer(w) => *w as u32,
        _ => return None,
    };
    let height = match stream.dict.get(b"Height").ok()? {
        Object::Integer(h) => *h as u32,
        _ => return None,
    };
    if width == 0 || height == 0 {
        return None;
    }

    let color_space = match stream.dict.get(b"ColorSpace").ok() {
        Some(Object::Name(cs)) => Some(cs.as_slice()),
        _ => None,
    };

    let filter = match stream.dict.get(b"Filter").ok() {
        Some(Object::Name(f)) => Some(f.as_slice()),
        Some(Object::Array(arr)) if arr.len() == 1 => match &arr[0] {
            Object::Name(f) => Some(f.as_slice()),
            _ => None,
        },
        _ => None,
    };

    // Keep JPEG bytes as-is when possible.
    if filter == Some(b"DCTDecode") {
        return Some(("jpg".into(), stream.content.clone()));
    }

    let mut working = stream.clone();
    let _ = working.decompress();
    let content = working.content.clone();

    let rgb = if color_space == Some(b"DeviceRGB") {
        if content.len() != (width * height * 3) as usize {
            return None;
        }
        RgbImage::from_raw(width, height, content)?
    } else if color_space == Some(b"DeviceGray") {
        if content.len() != (width * height) as usize {
            return None;
        }
        let gray = image::GrayImage::from_raw(width, height, content)?;
        DynamicImage::ImageLuma8(gray).to_rgb8()
    } else {
        return None;
    };

    let mut buf = Vec::new();
    DynamicImage::ImageRgb8(rgb)
        .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
        .ok()?;
    Some(("png".into(), buf))
}

fn unique_file(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("img");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("bin");
    for i in 1..10_000 {
        let candidate = parent.join(format!("{stem}_{i}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}
