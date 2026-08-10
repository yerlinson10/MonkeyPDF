use app_lib::pdf_engine;
use lopdf::{Dictionary, Document, Object, Stream};
use std::path::PathBuf;

fn make_simple_pdf(path: &PathBuf, label: &str) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let content = format!("BT /F1 24 Tf 100 700 Td ({label}) Tj ET");
    let content_id = doc.add_object(Object::Stream(Stream::new(
        Dictionary::new(),
        content.into_bytes(),
    )));

    let mut font = Dictionary::new();
    font.set("Type", "Font");
    font.set("Subtype", "Type1");
    font.set("BaseFont", "Helvetica");
    let font_id = doc.add_object(font);

    let mut fonts = Dictionary::new();
    fonts.set("F1", font_id);
    let mut resources = Dictionary::new();
    resources.set("Font", fonts);

    let mut page = Dictionary::new();
    page.set("Type", "Page");
    page.set("Parent", pages_id);
    page.set(
        "MediaBox",
        vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(595),
            Object::Integer(842),
        ],
    );
    page.set("Contents", content_id);
    page.set("Resources", resources);
    let page_id = doc.add_object(page);

    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Count", 1_i64);
    pages.set("Kids", vec![Object::Reference(page_id)]);
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);
    doc.max_id = doc.objects.len() as u32;
    doc.save(path).unwrap();
}

#[test]
fn merge_split_rotate_smoke() {
    let dir = std::env::temp_dir().join(format!(
        "monkeypdf_test_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let a = dir.join("a.pdf");
    let b = dir.join("b.pdf");
    let merged = dir.join("merged.pdf");
    let rotated = dir.join("rotated.pdf");
    make_simple_pdf(&a, "A");
    make_simple_pdf(&b, "B");

    let merge_result = pdf_engine::merge_pdfs(
        vec![
            a.to_string_lossy().to_string(),
            b.to_string_lossy().to_string(),
        ],
        merged.to_string_lossy().to_string(),
    )
    .expect("merge failed");
    assert_eq!(merge_result.page_count, 2);
    assert!(merged.exists());

    let split_dir = dir.join("split");
    std::fs::create_dir_all(&split_dir).unwrap();
    let split_result = pdf_engine::split_pdf(
        merged.to_string_lossy().to_string(),
        vec![(1, 1), (2, 2)],
        split_dir.to_string_lossy().to_string(),
    )
    .expect("split failed");
    assert_eq!(split_result.output_paths.len(), 2);

    let rotate_result = pdf_engine::rotate_pdf(
        merged.to_string_lossy().to_string(),
        90,
        None,
        rotated.to_string_lossy().to_string(),
    )
    .expect("rotate failed");
    assert_eq!(rotate_result.page_count, 2);
    assert!(rotated.exists());

    let compressed = dir.join("compressed.pdf");
    let compress_result = pdf_engine::compress_pdf(
        merged.to_string_lossy().to_string(),
        60,
        compressed.to_string_lossy().to_string(),
        None,
    )
    .expect("compress failed");
    assert!(compressed.exists());
    assert_eq!(compress_result.page_count, 2);

    // JPG → PDF → JPG roundtrip (requires pdfium.dll in resources/)
    let jpg = dir.join("page.jpg");
    {
        // Create a tiny JPEG with the image crate
        let img = image::RgbImage::from_fn(64, 64, |x, y| {
            image::Rgb([x as u8, y as u8, 128])
        });
        img.save(&jpg).unwrap();
    }
    let from_images = dir.join("from_images.pdf");
    let img_result = pdf_engine::images_to_pdf(
        vec![jpg.to_string_lossy().to_string()],
        from_images.to_string_lossy().to_string(),
    )
    .expect("images_to_pdf failed");
    assert_eq!(img_result.page_count, 1);
    assert!(from_images.exists());

    let jpg_out = dir.join("jpg_out");
    std::fs::create_dir_all(&jpg_out).unwrap();
    let pdfium_result = pdf_engine::pdf_to_jpg(
        from_images.to_string_lossy().to_string(),
        72,
        jpg_out.to_string_lossy().to_string(),
        None,
    );
    match pdfium_result {
        Ok(r) => {
            assert_eq!(r.page_count, 1);
            assert!(!r.output_paths.is_empty());
        }
        Err(e) => {
            // Allow failure only if pdfium.dll is missing in CI environments
            let msg = e.to_string();
            assert!(
                msg.contains("pdfium") || msg.contains("PDFium"),
                "unexpected pdf_to_jpg error: {msg}"
            );
        }
    }

    // Protect → unlock roundtrip
    let protected = dir.join("protected.pdf");
    let unlocked = dir.join("unlocked.pdf");
    pdf_engine::protect_pdf(
        merged.to_string_lossy().to_string(),
        "secret123".into(),
        None,
        protected.to_string_lossy().to_string(),
    )
    .expect("protect failed");
    assert!(protected.exists());
    pdf_engine::unlock_pdf(
        protected.to_string_lossy().to_string(),
        "secret123".into(),
        unlocked.to_string_lossy().to_string(),
    )
    .expect("unlock failed");
    assert!(unlocked.exists());

    let numbered = dir.join("numbered.pdf");
    pdf_engine::add_page_numbers(
        unlocked.to_string_lossy().to_string(),
        numbered.to_string_lossy().to_string(),
        "bottom-center".into(),
        Some("{n}/{total}".into()),
        Some(1),
        Some(10.0),
    )
    .expect("page numbers failed");
    assert!(numbered.exists());

    let md = dir.join("out.md");
    pdf_engine::pdf_to_markdown(
        unlocked.to_string_lossy().to_string(),
        md.to_string_lossy().to_string(),
    )
    .expect("markdown failed");
    assert!(md.exists());
}
