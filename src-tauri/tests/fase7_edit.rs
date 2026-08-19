use app_lib::pdf_engine::{
    self, edit_pdf, list_text_runs, EditOp,
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
    font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
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
fn list_text_runs_finds_label() {
    let dir = temp_dir("edit_list");
    let pdf = dir.join("in.pdf");
    make_simple_pdf(&pdf, "HelloEdit");
    let runs = list_text_runs(pdf.to_string_lossy().into(), 1).unwrap();
    assert!(!runs.is_empty());
    assert!(runs.iter().any(|r| r.text.contains("HelloEdit")));
    assert!(runs[0].editable);
    assert!(runs[0].font_size > 0.0);
}

#[test]
fn replace_text_surgical() {
    let dir = temp_dir("edit_replace");
    let pdf = dir.join("in.pdf");
    let out = dir.join("out.pdf");
    make_simple_pdf(&pdf, "OldLabel");
    let runs = list_text_runs(pdf.to_string_lossy().into(), 1).unwrap();
    let run = runs.iter().find(|r| r.text.contains("OldLabel")).unwrap();
    let r = edit_pdf(
        pdf.to_string_lossy().into(),
        out.to_string_lossy().into(),
        vec![EditOp::ReplaceText {
            page: 1,
            run_id: run.run_id,
            new_text: "NewLabel".into(),
            fit_width: None,
        }],
        false,
        None,
    )
    .unwrap();
    assert_eq!(r.page_count, 1);
    assert!(out.exists());
    let extracted = {
        let doc = Document::load(&out).unwrap();
        doc.extract_text(&[1]).unwrap()
    };
    assert!(
        extracted.contains("NewLabel"),
        "expected NewLabel in {extracted:?}"
    );
}

#[test]
fn add_text_and_whiteout() {
    let dir = temp_dir("edit_add");
    let pdf = dir.join("in.pdf");
    let out = dir.join("out.pdf");
    make_simple_pdf(&pdf, "Base");
    let r = edit_pdf(
        pdf.to_string_lossy().into(),
        out.to_string_lossy().into(),
        vec![
            EditOp::Whiteout {
                page: 1,
                x: 90.0,
                y: 690.0,
                w: 120.0,
                h: 30.0,
                color: Some("#ffffff".into()),
            },
            EditOp::AddText {
                page: 1,
                x: 100.0,
                y: 700.0,
                w: 200.0,
                h: 40.0,
                text: "Added line".into(),
                font: "Helvetica".into(),
                size: 14.0,
                bold: true,
                italic: false,
                color: "#e11d48".into(),
                align: "left".into(),
                opacity: 1.0,
            },
        ],
        true,
        None,
    )
    .unwrap();
    assert!(r.output_paths[0].ends_with("out.pdf"));
    let doc = Document::load(&out).unwrap();
    let content = doc.get_page_content(*doc.get_pages().get(&1).unwrap()).unwrap();
    let s = String::from_utf8_lossy(&content);
    assert!(s.contains("Added line") || s.contains("Added"));
}

#[test]
fn shapes_and_free_draw() {
    let dir = temp_dir("edit_shapes");
    let pdf = dir.join("in.pdf");
    let out = dir.join("out.pdf");
    make_simple_pdf(&pdf, "Shapes");
    edit_pdf(
        pdf.to_string_lossy().into(),
        out.to_string_lossy().into(),
        vec![
            EditOp::Rect {
                page: 1,
                x: 50.0,
                y: 50.0,
                w: 100.0,
                h: 60.0,
                stroke: "#2563eb".into(),
                fill: Some("#dbeafe".into()),
                stroke_width: 2.0,
                opacity: 1.0,
            },
            EditOp::Ellipse {
                page: 1,
                x: 200.0,
                y: 100.0,
                w: 80.0,
                h: 50.0,
                stroke: "#16a34a".into(),
                fill: None,
                stroke_width: 1.5,
                opacity: 0.9,
            },
            EditOp::Line {
                page: 1,
                from: [40.0, 40.0],
                to: [200.0, 200.0],
                color: "#e11d48".into(),
                width: 2.0,
                arrow: "end".into(),
            },
            EditOp::FreeDraw {
                page: 1,
                paths: vec![vec![[10.0, 10.0], [30.0, 40.0], [60.0, 20.0]]],
                color: "#7c3aed".into(),
                width: 2.0,
                opacity: 1.0,
            },
        ],
        true,
        None,
    )
    .unwrap();
    assert!(out.exists());
    let doc = Document::load(&out).unwrap();
    let content = doc.get_page_content(*doc.get_pages().get(&1).unwrap()).unwrap();
    let s = String::from_utf8_lossy(&content);
    assert!(s.contains(" re") || s.contains("\nre") || s.contains(" re\n") || s.contains("re"));
    assert!(s.contains(" c") || s.contains("\nc") || s.contains("c\n") || s.contains(" m"));
}

#[test]
fn highlight_as_annot_and_flattened() {
    let dir = temp_dir("edit_hl");
    let pdf = dir.join("in.pdf");
    let out_ann = dir.join("ann.pdf");
    let out_flat = dir.join("flat.pdf");
    make_simple_pdf(&pdf, "MarkMe");

    edit_pdf(
        pdf.to_string_lossy().into(),
        out_ann.to_string_lossy().into(),
        vec![EditOp::Highlight {
            page: 1,
            quads: vec![[100.0, 700.0, 80.0, 20.0]],
            color: "#ffe066".into(),
            opacity: 0.4,
        }],
        false,
        None,
    )
    .unwrap();
    let doc = Document::load(&out_ann).unwrap();
    let page_id = *doc.get_pages().get(&1).unwrap();
    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
    let annots = page.get(b"Annots").unwrap();
    match annots {
        Object::Array(arr) => assert!(!arr.is_empty()),
        Object::Reference(_) => {}
        _ => panic!("expected Annots"),
    }

    edit_pdf(
        pdf.to_string_lossy().into(),
        out_flat.to_string_lossy().into(),
        vec![EditOp::Highlight {
            page: 1,
            quads: vec![[100.0, 700.0, 80.0, 20.0]],
            color: "#ffe066".into(),
            opacity: 0.4,
        }],
        true,
        None,
    )
    .unwrap();
    let doc = Document::load(&out_flat).unwrap();
    let page_id = *doc.get_pages().get(&1).unwrap();
    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
    assert!(page.get(b"Annots").is_err());
}

#[test]
fn stamp_and_note() {
    let dir = temp_dir("edit_stamp");
    let pdf = dir.join("in.pdf");
    let out = dir.join("out.pdf");
    make_simple_pdf(&pdf, "StampMe");
    edit_pdf(
        pdf.to_string_lossy().into(),
        out.to_string_lossy().into(),
        vec![
            EditOp::Stamp {
                page: 1,
                x: 200.0,
                y: 400.0,
                w: 160.0,
                h: 50.0,
                stamp: "aprobado".into(),
                custom_text: None,
                color: "#e11d48".into(),
            },
            EditOp::Note {
                page: 1,
                x: 50.0,
                y: 500.0,
                text: "Revisar".into(),
                color: "#f5c542".into(),
            },
        ],
        false,
        None,
    )
    .unwrap();
    let doc = Document::load(&out).unwrap();
    let page_id = *doc.get_pages().get(&1).unwrap();
    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
    assert!(page.get(b"Annots").is_ok());
}

#[test]
fn form_fill_roundtrip() {
    let dir = temp_dir("edit_form");
    let pdf = dir.join("form.pdf");
    let out = dir.join("filled.pdf");
    // Build minimal AcroForm via existing test helper pattern from fase5 if available;
    // otherwise create a simple widget.
    make_form_pdf(&pdf);
    let fields = pdf_engine::get_form_fields(&pdf.to_string_lossy()).unwrap();
    assert!(!fields.is_empty());
    let name = fields[0].name.clone();
    edit_pdf(
        pdf.to_string_lossy().into(),
        out.to_string_lossy().into(),
        vec![EditOp::FormFill {
            field: name.clone(),
            value: "FilledValue".into(),
        }],
        false,
        None,
    )
    .unwrap();
    let after = pdf_engine::get_form_fields(&out.to_string_lossy()).unwrap();
    let f = after.iter().find(|f| f.name == name).unwrap();
    assert_eq!(f.value, "FilledValue");
}

fn make_form_pdf(path: &PathBuf) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
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

    let mut field = Dictionary::new();
    field.set("Type", "Annot");
    field.set("Subtype", "Widget");
    field.set("FT", Object::Name(b"Tx".to_vec()));
    field.set("T", Object::string_literal("Name"));
    field.set("V", Object::string_literal(""));
    field.set(
        "Rect",
        vec![
            Object::Real(72.0),
            Object::Real(700.0),
            Object::Real(300.0),
            Object::Real(720.0),
        ],
    );
    field.set("F", Object::Integer(4));
    let field_id = doc.add_object(field);

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
    page.set("Annots", vec![Object::Reference(field_id)]);
    let page_id = doc.add_object(page);

    // Parent on field
    if let Ok(Object::Dictionary(f)) = doc.get_object_mut(field_id) {
        f.set("P", Object::Reference(page_id));
    }

    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Count", 1_i64);
    pages.set("Kids", vec![Object::Reference(page_id)]);
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut acro = Dictionary::new();
    acro.set("Fields", vec![Object::Reference(field_id)]);
    acro.set("NeedAppearances", Object::Boolean(true));
    let acro_id = doc.add_object(acro);

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    catalog.set("AcroForm", acro_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);
    doc.max_id = doc.objects.len() as u32;
    doc.save(path).unwrap();
}

#[test]
fn image_insert() {
    let dir = temp_dir("edit_img");
    let pdf = dir.join("in.pdf");
    let png = dir.join("dot.png");
    let out = dir.join("out.pdf");
    make_simple_pdf(&pdf, "Img");
    // 1x1 red PNG
    let png_bytes: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8,
        0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE, 0x02, 0xFE, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(&png, png_bytes).unwrap();
    edit_pdf(
        pdf.to_string_lossy().into(),
        out.to_string_lossy().into(),
        vec![EditOp::Image {
            page: 1,
            x: 100.0,
            y: 100.0,
            w: 80.0,
            h: 80.0,
            image_path: png.to_string_lossy().into(),
            rotation: 0.0,
            opacity: 1.0,
        }],
        true,
        None,
    )
    .unwrap();
    assert!(out.exists());
}
