use app_lib::pdf_engine::{self, CropBox, RedactRegion};
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

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "monkeypdf_{tag}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn redact_and_crop_smoke() {
    let dir = temp_dir("fase3_redact_crop");
    let input = dir.join("in.pdf");
    let redacted = dir.join("redacted.pdf");
    let cropped = dir.join("cropped.pdf");
    make_simple_pdf(&input, "Hello");

    let r = pdf_engine::redact_pdf(
        input.to_string_lossy().to_string(),
        redacted.to_string_lossy().to_string(),
        vec![RedactRegion {
            page: 1,
            x: 80.0,
            y: 680.0,
            w: 200.0,
            h: 40.0,
        }],
    )
    .expect("redact failed");
    assert!(redacted.exists());
    assert_eq!(r.page_count, 1);
    // Secure flatten: text under black must not extract
    let extracted = Document::load(&redacted)
        .ok()
        .and_then(|doc| doc.extract_text(&[1]).ok())
        .unwrap_or_default();
    assert!(
        !extracted.to_lowercase().contains("hello"),
        "redacted PDF still exposes source text: {extracted:?}"
    );

    let c = pdf_engine::crop_pdf(
        input.to_string_lossy().to_string(),
        cropped.to_string_lossy().to_string(),
        CropBox {
            x: 50.0,
            y: 100.0,
            w: 400.0,
            h: 500.0,
        },
        None,
    )
    .expect("crop failed");
    assert!(cropped.exists());
    assert_eq!(c.page_count, 1);

    let (x, y, w, h) = pdf_engine::page_mediabox(&cropped.to_string_lossy(), 1).unwrap();
    assert!((x - 50.0).abs() < 0.1);
    assert!((y - 100.0).abs() < 0.1);
    assert!((w - 400.0).abs() < 0.1);
    assert!((h - 500.0).abs() < 0.1);
}

#[test]
fn compare_text_smoke() {
    let dir = temp_dir("fase3_compare");
    let a = dir.join("a.pdf");
    let b = dir.join("b.pdf");
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    make_simple_pdf(&a, "Alpha");
    make_simple_pdf(&b, "Beta");

    let result = pdf_engine::compare_pdfs(
        a.to_string_lossy().to_string(),
        b.to_string_lossy().to_string(),
        out.to_string_lossy().to_string(),
        Some("text".into()),
    )
    .expect("compare failed");
    assert!(out.join("compare.md").exists());
    assert!(!result.output_paths.is_empty());
}

#[test]
fn ocr_skips_without_tesseract() {
    if pdf_engine::tesseract_available() {
        // Smoke: just ensure availability probe works when installed
        assert!(pdf_engine::tesseract_available());
        return;
    }
    let dir = temp_dir("fase3_ocr_skip");
    let input = dir.join("in.pdf");
    let output = dir.join("out.md");
    make_simple_pdf(&input, "Scan");
    let err = pdf_engine::ocr_pdf(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        Some("eng".into()),
        Some("markdown".into()),
    );
    assert!(err.is_err(), "OCR should fail clearly without Tesseract");
}
