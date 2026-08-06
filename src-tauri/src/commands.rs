use crate::error::{FilePreview, OpResult};
use crate::pdf_engine::{self, AiResult};
use tauri::command;

#[command]
pub async fn merge_pdfs(paths: Vec<String>, output: String) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::merge_pdfs(paths, output))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn split_pdf(
    path: String,
    ranges: Vec<(u32, u32)>,
    output_dir: String,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::split_pdf(path, ranges, output_dir))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn rotate_pdf(
    path: String,
    angle: u32,
    pages: Option<Vec<u32>>,
    output: String,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::rotate_pdf(path, angle, pages, output))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn compress_pdf(
    path: String,
    quality: u8,
    output: String,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::compress_pdf(path, quality, output))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn pdf_to_jpg(
    path: String,
    dpi: u32,
    output_dir: String,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::pdf_to_jpg(path, dpi, output_dir))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn jpg_to_pdf(paths: Vec<String>, output: String) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::images_to_pdf(paths, output))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn protect_pdf(
    path: String,
    user_password: String,
    owner_password: Option<String>,
    output: String,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        pdf_engine::protect_pdf(path, user_password, owner_password, output)
    })
    .await
    .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn unlock_pdf(
    path: String,
    password: String,
    output: String,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::unlock_pdf(path, password, output))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn add_page_numbers(
    path: String,
    output: String,
    position: String,
    format: Option<String>,
    start_from: Option<u32>,
    font_size: Option<f32>,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        pdf_engine::add_page_numbers(path, output, position, format, start_from, font_size)
    })
    .await
    .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn convert_office(
    path: String,
    target: String,
    output_dir: String,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        pdf_engine::convert_with_libreoffice(path, target, output_dir)
    })
    .await
    .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn check_libreoffice() -> Result<bool, crate::error::AppError> {
    Ok(pdf_engine::soffice_available())
}

#[command]
pub async fn check_tesseract() -> Result<bool, crate::error::AppError> {
    Ok(pdf_engine::tesseract_available())
}

#[command]
pub async fn ocr_pdf(
    path: String,
    output: String,
    lang: Option<String>,
    mode: Option<String>,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::ocr_pdf(path, output, lang, mode))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn redact_pdf(
    path: String,
    output: String,
    regions: Vec<pdf_engine::RedactRegion>,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::redact_pdf(path, output, regions))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn crop_pdf(
    path: String,
    output: String,
    crop: pdf_engine::CropBox,
    pages: Option<Vec<u32>>,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::crop_pdf(path, output, crop, pages))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn compare_pdfs(
    path_a: String,
    path_b: String,
    output_dir: String,
    mode: Option<String>,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        pdf_engine::compare_pdfs(path_a, path_b, output_dir, mode)
    })
    .await
    .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn compare_report(
    path_a: String,
    path_b: String,
    mode: Option<String>,
    export_dir: Option<String>,
) -> Result<pdf_engine::CompareReport, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        pdf_engine::compare_report(path_a, path_b, mode, export_dir)
    })
    .await
    .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMediaBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[command]
pub async fn get_page_mediabox(
    path: String,
    page: u32,
) -> Result<PageMediaBox, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let (x, y, width, height) = pdf_engine::page_mediabox(&path, page)?;
        Ok(PageMediaBox {
            x,
            y,
            width,
            height,
        })
    })
    .await
    .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn pdf_to_markdown(
    path: String,
    output: String,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::pdf_to_markdown(path, output))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn ai_process_pdf(
    path: String,
    action: String,
    provider: String,
    api_key: String,
    model: Option<String>,
    target_lang: Option<String>,
    base_url: Option<String>,
) -> Result<AiResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        pdf_engine::run_ai_on_pdf(path, action, provider, api_key, model, target_lang, base_url)
    })
    .await
    .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn write_text_file(
    path: String,
    content: String,
) -> Result<OpResult, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || pdf_engine::write_text_file(path, content))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn get_pdf_page_count(path: String) -> Result<u32, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = pdf_engine::ensure_pdf_path(&path)?;
        let doc = lopdf::Document::load(p)?;
        Ok(doc.get_pages().len() as u32)
    })
    .await
    .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn preview_pdf(
    path: String,
    page: Option<u32>,
    max_width: Option<u32>,
) -> Result<FilePreview, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        pdf_engine::preview_pdf(path, page.unwrap_or(1), max_width.unwrap_or(480))
    })
    .await
    .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn preview_image(
    path: String,
    max_width: Option<u32>,
) -> Result<FilePreview, crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        pdf_engine::preview_image(path, max_width.unwrap_or(480))
    })
    .await
    .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

#[command]
pub async fn reveal_in_explorer(path: String) -> Result<(), crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || reveal_path(&path))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

/// Open http(s) URLs in the system default browser (Tauri webview blocks <a target=_blank>).
#[command]
pub async fn open_url(url: String) -> Result<(), crate::error::AppError> {
    tauri::async_runtime::spawn_blocking(move || open_external_url(&url))
        .await
        .map_err(|e| crate::error::AppError::Pdf(format!("Task join error: {e}")))?
}

fn open_external_url(url: &str) -> Result<(), crate::error::AppError> {
    let trimmed = url.trim();
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err(crate::error::AppError::InvalidInput(
            "Solo se permiten URLs http(s)".into(),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // `start` is a cmd builtin; empty title arg avoids swallowing the URL.
        std::process::Command::new("cmd")
            .args(["/C", "start", "", trimmed])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(crate::error::AppError::Io)?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(trimmed)
            .spawn()
            .map_err(crate::error::AppError::Io)?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(trimmed)
            .spawn()
            .map_err(crate::error::AppError::Io)?;
    }

    Ok(())
}

#[command]
pub async fn notify_done(
    title: String,
    body: String,
    path: Option<String>,
) -> Result<(), crate::error::AppError> {
    std::thread::Builder::new()
        .name("monkeypdf-notify".into())
        .spawn(move || {
            if let Err(err) = show_clickable_notification(&title, &body, path.as_deref()) {
                log::warn!("notify_done failed: {err}");
            }
        })
        .map_err(|e| crate::error::AppError::Pdf(format!("Failed to spawn notify thread: {e}")))?;
    Ok(())
}

fn show_clickable_notification(
    title: &str,
    body: &str,
    path: Option<&str>,
) -> Result<(), crate::error::AppError> {
    let mut notification = notify_rust::Notification::new();
    notification
        .summary(title)
        .body(body)
        .timeout(notify_rust::Timeout::Milliseconds(10_000));

    #[cfg(target_os = "windows")]
    apply_windows_app_id(&mut notification);

    notification.action("open", "Abrir en explorador");

    let handle = notification
        .show()
        .map_err(|e| crate::error::AppError::Pdf(format!("Notification error: {e}")))?;

    let path = path.map(str::to_owned);
    let _ = handle.wait_for_response(|response: &notify_rust::NotificationResponse| {
        use notify_rust::NotificationResponse;
        let should_open = matches!(
            response,
            NotificationResponse::Default | NotificationResponse::Action(_)
        );
        if should_open {
            if let Some(ref p) = path {
                if let Err(err) = reveal_path(p) {
                    log::warn!("reveal after notification click failed: {err}");
                }
            }
        }
    });

    Ok(())
}

#[cfg(target_os = "windows")]
fn apply_windows_app_id(notification: &mut notify_rust::Notification) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let s = dir.to_string_lossy();
    let sep = std::path::MAIN_SEPARATOR;
    let debug = format!("{sep}target{sep}debug");
    let release = format!("{sep}target{sep}release");
    let test = format!("{sep}target-test{sep}");
    if !(s.ends_with(&debug) || s.ends_with(&release) || s.contains(&test)) {
        notification.app_id("com.monkeypdf.desktop");
    }
}

fn reveal_path(path: &str) -> Result<(), crate::error::AppError> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(crate::error::AppError::InvalidInput(format!(
            "Path not found: {path}"
        )));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut cmd = std::process::Command::new("explorer");
        if p.is_dir() {
            cmd.arg(p.as_os_str());
        } else {
            cmd.arg(format!("/select,{}", p.display()));
        }
        cmd.creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(crate::error::AppError::Io)?;
    }

    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        if p.is_dir() {
            cmd.arg(p.as_os_str());
        } else {
            cmd.args(["-R", path]);
        }
        cmd.spawn().map_err(crate::error::AppError::Io)?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = if p.is_dir() {
            p.to_path_buf()
        } else {
            p.parent()
                .map(|d| d.to_path_buf())
                .unwrap_or_else(|| p.to_path_buf())
        };
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(crate::error::AppError::Io)?;
    }

    Ok(())
}
