mod compress;
mod images_to_pdf;
mod merge;
mod pdf_to_images;
mod preview;
mod rotate;
mod split;

pub use compress::compress_pdf;
pub use images_to_pdf::images_to_pdf;
pub use merge::merge_pdfs;
pub use pdf_to_images::pdf_to_jpg;
pub use preview::{preview_image, preview_pdf};
pub use rotate::rotate_pdf;
pub use split::split_pdf;

use crate::error::AppError;
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

static PDFIUM_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn init_pdfium(app: &AppHandle) -> Result<(), AppError> {
    if PDFIUM_PATH.get().is_some() {
        return Ok(());
    }

    let candidates = pdfium_candidates(app);
    let mut last_err = String::from("no candidate paths");

    for path in candidates {
        match Pdfium::bind_to_library(&path) {
            Ok(_bindings) => {
                let _ = PDFIUM_PATH.set(path.clone());
                log::info!("PDFium found at {}", path.display());
                return Ok(());
            }
            Err(err) => {
                last_err = format!("{} ({})", path.display(), err);
            }
        }
    }

    Err(AppError::Pdfium(format!(
        "Failed to locate pdfium.dll. Last attempt: {last_err}"
    )))
}

/// Create a Pdfium instance for the current operation.
/// Pdfium bindings are not Sync, so we bind per-call rather than caching globally.
pub fn create_pdfium() -> Result<Pdfium, AppError> {
    let path = if let Some(p) = PDFIUM_PATH.get() {
        p.clone()
    } else {
        // Fallback search without AppHandle
        let candidates = [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join(Pdfium::pdfium_platform_library_name()),
            PathBuf::from(".").join(Pdfium::pdfium_platform_library_name()),
            PathBuf::from("./resources").join(Pdfium::pdfium_platform_library_name()),
        ];
        let mut found = None;
        let mut last_err = String::new();
        for c in candidates {
            match Pdfium::bind_to_library(&c) {
                Ok(_) => {
                    found = Some(c);
                    break;
                }
                Err(e) => last_err = e.to_string(),
            }
        }
        found.ok_or_else(|| {
            AppError::Pdfium(format!("pdfium.dll not found ({last_err})"))
        })?
    };

    let bindings = Pdfium::bind_to_library(&path)
        .map_err(|e| AppError::Pdfium(format!("{}: {e}", path.display())))?;
    Ok(Pdfium::new(bindings))
}

fn pdfium_candidates(app: &AppHandle) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let lib_name = Pdfium::pdfium_platform_library_name();

    if let Ok(resource_dir) = app.path().resource_dir() {
        paths.push(resource_dir.join("resources").join(&lib_name));
        paths.push(resource_dir.join(&lib_name));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join(&lib_name));
            paths.push(dir.join("resources").join(&lib_name));
        }
    }

    paths.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join(&lib_name));
    paths.push(PathBuf::from(".").join(&lib_name));
    paths.push(PathBuf::from("./resources").join(&lib_name));

    paths
}

pub fn ensure_pdf_path(path: &str) -> Result<PathBuf, AppError> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(AppError::InvalidInput(format!("File not found: {path}")));
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "pdf" {
        return Err(AppError::InvalidInput(format!(
            "Expected a PDF file, got: {path}"
        )));
    }
    Ok(p)
}

pub fn ensure_image_path(path: &str) -> Result<PathBuf, AppError> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(AppError::InvalidInput(format!("File not found: {path}")));
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
        return Err(AppError::InvalidInput(format!(
            "Expected an image file (jpg/png/webp), got: {path}"
        )));
    }
    Ok(p)
}

pub fn ensure_parent_dir(path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

pub fn ensure_dir(path: &str) -> Result<PathBuf, AppError> {
    let p = PathBuf::from(path);
    if !p.exists() {
        std::fs::create_dir_all(&p)?;
    }
    if !p.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "Expected a directory: {path}"
        )));
    }
    Ok(p)
}
