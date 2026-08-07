use app_lib::pdf_engine::{
    self, delete_signature, get_form_fields, list_signatures, save_signature, sign_pdf,
    FieldFill, NewSignatureAsset, SignPlacement, SignatureKind, SignatureMethod,
};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
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

fn tiny_png() -> Vec<u8> {
    // 2x2 RGBA PNG
    let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([20, 20, 20, 255]));
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::Png,
        )
        .unwrap();
    buf
}

fn png_data_url() -> String {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    format!("data:image/png;base64,{}", B64.encode(tiny_png()))
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

fn make_form_pdf(path: &PathBuf) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    // Text field widget
    let mut field = Dictionary::new();
    field.set("Type", "Annot");
    field.set("Subtype", "Widget");
    field.set("FT", Object::Name(b"Tx".to_vec()));
    field.set("T", Object::string_literal("FullName"));
    field.set("V", Object::string_literal(""));
    field.set(
        "Rect",
        vec![
            Object::Real(72.0),
            Object::Real(700.0),
            Object::Real(300.0),
            Object::Real(724.0),
        ],
    );
    field.set("F", Object::Integer(4));
    let field_id = doc.add_object(field);

    // Signature field
    let mut sig = Dictionary::new();
    sig.set("Type", "Annot");
    sig.set("Subtype", "Widget");
    sig.set("FT", Object::Name(b"Sig".to_vec()));
    sig.set("T", Object::string_literal("Signature1"));
    sig.set(
        "Rect",
        vec![
            Object::Real(72.0),
            Object::Real(100.0),
            Object::Real(250.0),
            Object::Real(150.0),
        ],
    );
    sig.set("F", Object::Integer(4));
    let sig_id = doc.add_object(sig);

    let content_id = doc.add_object(Object::Stream(Stream::new(
        Dictionary::new(),
        b"BT /F1 12 Tf 72 750 Td (Form) Tj ET".to_vec(),
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
    page.set(
        "Annots",
        vec![Object::Reference(field_id), Object::Reference(sig_id)],
    );
    let page_id = doc.add_object(page);

    // Link widgets to page
    for id in [field_id, sig_id] {
        if let Ok(Object::Dictionary(d)) = doc.get_object_mut(id) {
            d.set("P", Object::Reference(page_id));
        }
    }

    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Count", 1_i64);
    pages.set("Kids", vec![Object::Reference(page_id)]);
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut acro = Dictionary::new();
    acro.set(
        "Fields",
        vec![Object::Reference(field_id), Object::Reference(sig_id)],
    );
    let acro_id = doc.add_object(acro);

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    catalog.set("AcroForm", Object::Reference(acro_id));
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);
    doc.max_id = doc.objects.len() as u32;
    doc.save(path).unwrap();
    let _ = page_id as ObjectId;
}

#[test]
fn signature_crud_roundtrip() {
    let dir = temp_dir("fase5_sig");
    let meta = save_signature(
        &dir,
        NewSignatureAsset {
            id: None,
            kind: SignatureKind::Signature,
            name: Some("Test User".into()),
            method: SignatureMethod::Type,
            font: Some("Great Vibes".into()),
            color: Some("#1a1a1a".into()),
            png_data_url: png_data_url(),
            source: serde_json::json!({}),
        },
    )
    .expect("save");
    assert!(!meta.id.is_empty());
    let list = list_signatures(&dir).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name.as_deref(), Some("Test User"));
    delete_signature(&dir, &meta.id).unwrap();
    assert!(list_signatures(&dir).unwrap().is_empty());
}

#[test]
fn form_fields_detect() {
    let dir = temp_dir("fase5_form");
    let pdf = dir.join("form.pdf");
    make_form_pdf(&pdf);
    let fields = get_form_fields(&pdf.to_string_lossy()).expect("fields");
    assert!(fields.iter().any(|f| f.name == "FullName"));
    assert!(fields.iter().any(|f| f.kind == pdf_engine::FormFieldKind::Signature));
}

#[test]
fn sign_pdf_bake() {
    let dir = temp_dir("fase5_bake");
    let input = dir.join("in.pdf");
    let output = dir.join("out.pdf");
    make_simple_pdf(&input, "Hello");
    let meta = save_signature(
        &dir,
        NewSignatureAsset {
            id: Some("bake1".into()),
            kind: SignatureKind::Signature,
            name: Some("A".into()),
            method: SignatureMethod::Upload,
            font: None,
            color: None,
            png_data_url: png_data_url(),
            source: serde_json::json!({}),
        },
    )
    .unwrap();

    let r = sign_pdf(
        &dir,
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        vec![SignPlacement {
            asset_id: Some(meta.id),
            png_bytes: None,
            png_data_url: None,
            page: 1,
            x: 100.0,
            y: 100.0,
            w: 120.0,
            h: 40.0,
        }],
        vec![FieldFill {
            name: "noop".into(),
            value: "x".into(),
        }],
    )
    .expect("sign");
    assert!(output.exists());
    assert_eq!(r.page_count, 1);
}
