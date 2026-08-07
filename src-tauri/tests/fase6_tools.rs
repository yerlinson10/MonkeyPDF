use app_lib::pdf_engine::{
    self, diagnose_pdf, organize_pdf, repair_pdf, watermark_pdf, PageRef, WatermarkSpec,
};
use lopdf::{Dictionary, Document, Object, Stream};
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "monkeypdf_{tag}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

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

fn make_two_page_pdf(path: &PathBuf, a: &str, b: &str) {
    let dir = path.parent().unwrap();
    let p1 = dir.join(format!("_tmp_{a}.pdf"));
    let p2 = dir.join(format!("_tmp_{b}.pdf"));
    make_simple_pdf(&p1, a);
    make_simple_pdf(&p2, b);
    let r = pdf_engine::merge_pdfs(
        vec![
            p1.to_string_lossy().to_string(),
            p2.to_string_lossy().to_string(),
        ],
        path.to_string_lossy().to_string(),
    )
    .unwrap();
    assert_eq!(r.page_count, 2);
}

#[test]
fn diagnose_and_repair_smoke() {
    let dir = temp_dir("fase6_repair");
    let input = dir.join("in.pdf");
    let output = dir.join("out.pdf");
    make_simple_pdf(&input, "Hello");

    let d = diagnose_pdf(input.to_string_lossy().to_string()).expect("diagnose");
    assert_eq!(d.page_count, 1);
    assert!(!d.encrypted);

    let r = repair_pdf(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        None,
    )
    .expect("repair");
    assert!(output.exists());
    assert_eq!(r.page_count, 1);
}

#[test]
fn watermark_text_smoke() {
    let dir = temp_dir("fase6_wm");
    let input = dir.join("in.pdf");
    let output = dir.join("wm.pdf");
    make_simple_pdf(&input, "Doc");

    let r = watermark_pdf(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        WatermarkSpec {
            mode: "text".into(),
            text: Some("PREVIEW".into()),
            font: None,
            size: Some(42.0),
            bold: true,
            italic: false,
            underline: false,
            color: Some("#c0392b".into()),
            image_path: None,
            position: 4,
            mosaic: true,
            transparency: 50,
            rotation: 45.0,
            page_from: Some(1),
            page_to: Some(1),
            layer: "above".into(),
        },
    )
    .expect("watermark");
    assert!(output.exists());
    assert_eq!(r.page_count, 1);
}

#[test]
fn organize_multi_smoke() {
    let dir = temp_dir("fase6_org");
    let a = dir.join("a.pdf");
    let b = dir.join("b.pdf");
    let out = dir.join("ordered.pdf");
    make_two_page_pdf(&a, "A1", "A2");
    make_two_page_pdf(&b, "B1", "B2");

    let r = organize_pdf(
        vec![
            PageRef {
                source_path: a.to_string_lossy().to_string(),
                page: 2,
                rotate: 90,
            },
            PageRef {
                source_path: b.to_string_lossy().to_string(),
                page: 1,
                rotate: 0,
            },
            PageRef {
                source_path: a.to_string_lossy().to_string(),
                page: 1,
                rotate: 0,
            },
            PageRef {
                source_path: b.to_string_lossy().to_string(),
                page: 2,
                rotate: 180,
            },
        ],
        out.to_string_lossy().to_string(),
    )
    .expect("organize");
    assert!(out.exists());
    assert_eq!(r.page_count, 4);
}
