mod commands;
mod error;
pub mod pdf_engine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            if let Err(err) = pdf_engine::init_pdfium(app.handle()) {
                log::warn!("PDFium init deferred/failed at startup: {err}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::merge_pdfs,
            commands::split_pdf,
            commands::rotate_pdf,
            commands::cancel_job,
            commands::clear_job,
            commands::compress_pdf,
            commands::pdf_to_jpg,
            commands::jpg_to_pdf,
            commands::protect_pdf,
            commands::unlock_pdf,
            commands::add_page_numbers,
            commands::convert_office,
            commands::convert_to_pdfa,
            commands::extract_images,
            commands::extract_text,
            commands::check_libreoffice,
            commands::check_tesseract,
            commands::ocr_pdf,
            commands::redact_pdf,
            commands::crop_pdf,
            commands::compare_pdfs,
            commands::compare_report,
            commands::get_page_mediabox,
            commands::list_signatures,
            commands::save_signature,
            commands::delete_signature,
            commands::get_form_fields,
            commands::sign_pdf,
            commands::diagnose_pdf,
            commands::repair_pdf,
            commands::watermark_pdf,
            commands::organize_pdf,
            commands::get_pdf_metadata,
            commands::set_pdf_metadata,
            commands::pdf_to_markdown,
            commands::ai_process_pdf,
            commands::write_text_file,
            commands::get_pdf_page_count,
            commands::preview_pdf,
            commands::preview_image,
            commands::reveal_in_explorer,
            commands::open_url,
            commands::notify_done,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MonkeyPDF");
}
