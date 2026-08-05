//! Manual check: cargo test --test unlock_roundtrip -- --nocapture
use app_lib::pdf_engine;
use image::GenericImageView;

#[test]
fn protect_unlock_image_pdf_renders() {
    let dir = std::env::temp_dir().join(format!("mp_unlock_rt_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let jpg = dir.join("p.jpg");
    image::RgbImage::from_fn(120, 160, |x, y| image::Rgb([(x % 255) as u8, (y % 255) as u8, 40]))
        .save(&jpg)
        .unwrap();

    let src = dir.join("src.pdf");
    pdf_engine::images_to_pdf(
        vec![jpg.to_string_lossy().to_string()],
        src.to_string_lossy().to_string(),
    )
    .unwrap();

    let prot = dir.join("prot.pdf");
    pdf_engine::protect_pdf(
        src.to_string_lossy().to_string(),
        "clave-secreta".into(),
        None,
        prot.to_string_lossy().to_string(),
    )
    .expect("protect");

    assert!(
        lopdf::Document::load(&prot).unwrap().is_encrypted(),
        "protected file should report encrypted"
    );

    let unlocked = dir.join("unlocked.pdf");
    pdf_engine::unlock_pdf(
        prot.to_string_lossy().to_string(),
        "clave-secreta".into(),
        unlocked.to_string_lossy().to_string(),
    )
    .expect("unlock");

    let doc = lopdf::Document::load(&unlocked).expect("reload unlocked");
    assert!(!doc.is_encrypted(), "unlocked must not be encrypted");
    assert!(doc.trailer.get(b"Encrypt").is_err());
    assert_eq!(doc.get_pages().len(), 1);

    let out = dir.join("jpg_out");
    std::fs::create_dir_all(&out).unwrap();
    let rendered = pdf_engine::pdf_to_jpg(
        unlocked.to_string_lossy().to_string(),
        72,
        out.to_string_lossy().to_string(),
    )
    .expect("pdfium must render unlocked PDF");

    let jpg_path = &rendered.output_paths[0];
    let img = image::open(jpg_path).expect("open rendered jpg");
    let non_white = img.pixels().filter(|(_, _, p)| {
        let rgba = p.0;
        rgba[0] < 250 || rgba[1] < 250 || rgba[2] < 250
    }).count();
    assert!(
        non_white > 100,
        "unlocked page looks blank (only {non_white} non-white pixels) — content was lost"
    );
}

#[test]
fn unlock_preserves_page_content_stream() {
    use lopdf::{dictionary, Document, Object, Stream};

    let dir = std::env::temp_dir().join(format!("mp_unlock_content_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Minimal PDF with visible content operators (not just form widgets).
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let content = b"BT /F1 24 Tf 100 700 Td (MonkeyPDF unlock content) Tj ET".to_vec();
    let content_id = doc.add_object(Stream::new(dictionary! {}, content));
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
        "Resources" => resources_id,
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let src = dir.join("text.pdf");
    doc.save(&src).unwrap();

    let prot = dir.join("prot.pdf");
    pdf_engine::protect_pdf(
        src.to_string_lossy().to_string(),
        "form-clave".into(),
        None,
        prot.to_string_lossy().to_string(),
    )
    .expect("protect text pdf");

    let unlocked = dir.join("unlocked.pdf");
    pdf_engine::unlock_pdf(
        prot.to_string_lossy().to_string(),
        "form-clave".into(),
        unlocked.to_string_lossy().to_string(),
    )
    .expect("unlock text pdf");

    let out = dir.join("jpg_out");
    std::fs::create_dir_all(&out).unwrap();
    let rendered = pdf_engine::pdf_to_jpg(
        unlocked.to_string_lossy().to_string(),
        96,
        out.to_string_lossy().to_string(),
    )
    .expect("render unlocked text pdf");

    let img = image::open(&rendered.output_paths[0]).unwrap();
    let non_white = img.pixels().filter(|(_, _, p)| {
        let rgba = p.0;
        rgba[0] < 250 || rgba[1] < 250 || rgba[2] < 250
    }).count();
    assert!(
        non_white > 50,
        "text page blank after unlock ({non_white} non-white px)"
    );
}
