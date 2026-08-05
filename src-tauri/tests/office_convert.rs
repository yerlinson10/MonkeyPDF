#[test]
fn office_docx_to_pdf() {
    let user = std::env::var("USERPROFILE").unwrap();
    let input = format!("{user}\\Downloads\\contrato_plantilla.docx");
    if !std::path::Path::new(&input).exists() {
        eprintln!("skip: no sample docx");
        return;
    }
    let out = std::env::temp_dir().join(format!("mp_lo_out_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let r = app_lib::pdf_engine::convert_with_libreoffice(
        input,
        "pdf".into(),
        out.to_string_lossy().to_string(),
    );
    assert!(r.is_ok(), "{r:?}");
    let r = r.unwrap();
    assert!(!r.output_paths.is_empty());
    assert!(std::path::Path::new(&r.output_paths[0]).exists());
    println!("ok: {}", r.output_paths[0]);
}

#[test]
fn office_pdf_to_docx() {
    let user = std::env::var("USERPROFILE").unwrap();
    let downloads = std::path::PathBuf::from(format!("{user}\\Downloads"));
    let pdf = std::fs::read_dir(&downloads)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
        });
    let Some(input) = pdf else {
        eprintln!("skip: no sample pdf in Downloads");
        return;
    };

    // Simulate OneDrive-ish destination with spaces — conversion must still land here.
    let out = std::env::temp_dir().join(format!("mp lo out spaces {}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let r = app_lib::pdf_engine::convert_with_libreoffice(
        input.to_string_lossy().to_string(),
        "docx".into(),
        out.to_string_lossy().to_string(),
    );
    assert!(r.is_ok(), "pdf→docx failed: {r:?}");
    let r = r.unwrap();
    let dest = std::path::Path::new(&r.output_paths[0]);
    assert!(dest.exists(), "missing {}", dest.display());
    assert!(
        dest.metadata().unwrap().len() > 1000,
        "docx too small / empty"
    );
    println!("ok: {}", dest.display());
}
