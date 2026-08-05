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
            commands::compress_pdf,
            commands::pdf_to_jpg,
            commands::jpg_to_pdf,
            commands::protect_pdf,
            commands::unlock_pdf,
            commands::add_page_numbers,
            commands::convert_office,
            commands::check_libreoffice,
            commands::pdf_to_markdown,
            commands::ai_process_pdf,
            commands::write_text_file,
            commands::get_pdf_page_count,
            commands::preview_pdf,
            commands::preview_image,
            commands::reveal_in_explorer,
            commands::notify_done,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MonkeyPDF");
}
