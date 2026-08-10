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
    assert!(!d.xref_broken);

    let r = repair_pdf(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        None,
    )
    .expect("repair");
    assert!(output.exists());
    assert_eq!(r.page_count, 1);
}

/// When structure is gone but JPEG/text still live in the bytes, partial salvage must produce a PDF.
#[test]
fn repair_partial_carves_jpeg() {
    use image::ImageEncoder;

    let dir = temp_dir("fase6_partial");
    let input = dir.join("garbled.pdf");
    let output = dir.join("rescued.pdf");

    let mut blob = b"%PDF-1.4\nNOT_A_REAL_PDF\n".to_vec();
    blob.extend_from_slice(b"This is salvageable plaintext about contract PPCC-11 year 2024.\n");
    {
        let img =
            image::RgbImage::from_fn(64, 64, |x, y| image::Rgb([(x * 3) as u8, (y * 3) as u8, 90]));
        let mut buf = std::io::Cursor::new(Vec::new());
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85);
        enc.write_image(img.as_raw(), 64, 64, image::ExtendedColorType::Rgb8)
            .unwrap();
        blob.extend_from_slice(&buf.into_inner());
    }
    std::fs::write(&input, &blob).unwrap();

    let r = repair_pdf(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        None,
    )
    .expect("partial salvage should succeed");
    assert!(output.exists());
    assert!(r.page_count >= 1);
    assert!(r.partial, "expected partial=true");
    Document::load(&output).expect("rescued pdf must open");
}

/// Corrupt startxref so lopdf fails; repair must recover via PDFium/qpdf/salvage.
#[test]
fn repair_broken_xref() {
    let dir = temp_dir("fase6_repair_xref");
    let input = dir.join("broken.pdf");
    let output = dir.join("fixed.pdf");
    make_simple_pdf(&input, "BrokenXref");

    let mut bytes = std::fs::read(&input).unwrap();
    // Point startxref at a nonsense offset so the xref table cannot be parsed.
    let needle = b"startxref";
    if let Some(pos) = bytes.windows(needle.len()).position(|w| w == needle) {
        let mut i = pos + needle.len();
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\r' || bytes[i] == b'\n') {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if start < i {
            bytes[start..i].fill(b'9');
        }
    }
    std::fs::write(&input, &bytes).unwrap();

    let d = diagnose_pdf(input.to_string_lossy().to_string()).expect("diagnose");
    assert!(d.xref_broken, "expected xref_broken diagnosis");

    let r = repair_pdf(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        None,
    )
    .expect("repair broken xref");
    assert!(output.exists());
    assert!(r.page_count >= 1);
    // Recovered file must open with lopdf.
    Document::load(&output).expect("repaired pdf should load with lopdf");
}

/// Destroy the xref table entirely — PDFium often still fails; native salvage must recover.
#[test]
fn repair_destroyed_xref_table() {
    let dir = temp_dir("fase6_repair_nuke_xref");
    let input = dir.join("nuked.pdf");
    let output = dir.join("fixed.pdf");
    make_simple_pdf(&input, "NukeXref");

    let bytes = std::fs::read(&input).unwrap();
    let xref_at = bytes
        .windows(5)
        .rposition(|w| w == b"xref\n" || w == b"xref\r")
        .map(|i| i)
        .or_else(|| bytes.windows(4).rposition(|w| w == b"xref"))
        .expect("xref marker");
    let mut nuked = bytes[..xref_at].to_vec();
    nuked.extend_from_slice(b"\n%%EOF\n");
    std::fs::write(&input, &nuked).unwrap();

    let r = repair_pdf(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        None,
    )
    .expect("repair nuked xref");
    assert!(output.exists());
    assert!(r.page_count >= 1);
    Document::load(&output).expect("repaired pdf should load with lopdf");
}

/// Pages live only inside an ObjStm — salvage must expand the stream.
#[test]
fn repair_objstm_pages() {
    let dir = temp_dir("fase6_repair_objstm");
    let input = dir.join("objstm.pdf");
    let output = dir.join("fixed.pdf");

    // Hand-built PDF: page dictionary only inside an uncompressed ObjStm.
    // Objects: 1=Catalog, 2=Pages, 3=ObjStm(contains page id 4), 5=Contents
    let content_stream = b"BT /F1 12 Tf 100 700 Td (ObjStmPage) Tj ET";
    let objstm_payload = b"4 0\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 5 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>";
    // First = offset of first object = after "4 0\n" = 4
    let first = 4usize;
    assert_eq!(&objstm_payload[..first], b"4 0\n");

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.5\n%\xE2\xE3\xCF\xD3\n");

    let mut offsets = std::collections::BTreeMap::new();

    let write_obj = |pdf: &mut Vec<u8>, offsets: &mut std::collections::BTreeMap<u32, usize>, id: u32, body: &[u8]| {
        offsets.insert(id, pdf.len());
        pdf.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        pdf.extend_from_slice(body);
        if !body.ends_with(b"\n") {
            pdf.push(b'\n');
        }
        pdf.extend_from_slice(b"endobj\n");
    };

    write_obj(
        &mut pdf,
        &mut offsets,
        1,
        b"<< /Type /Catalog /Pages 2 0 R >>",
    );
    write_obj(
        &mut pdf,
        &mut offsets,
        2,
        b"<< /Type /Pages /Count 1 /Kids [4 0 R] >>",
    );
    write_obj(
        &mut pdf,
        &mut offsets,
        5,
        &{
            let mut s = format!("<< /Length {} >>\nstream\n", content_stream.len()).into_bytes();
            s.extend_from_slice(content_stream);
            s.extend_from_slice(b"\nendstream");
            s
        },
    );
    write_obj(
        &mut pdf,
        &mut offsets,
        3,
        &{
            let mut s = format!(
                "<< /Type /ObjStm /N 1 /First {first} /Length {} >>\nstream\n",
                objstm_payload.len()
            )
            .into_bytes();
            s.extend_from_slice(objstm_payload);
            s.extend_from_slice(b"\nendstream");
            s
        },
    );

    // Deliberately broken xref (wrong startxref) — page 4 only exists inside ObjStm.
    let xref_pos = pdf.len();
    pdf.extend_from_slice(b"xref\n0 1\n0000000000 65535 f \n");
    pdf.extend_from_slice(b"trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n");
    pdf.extend_from_slice(b"99999999\n%%EOF\n");
    let _ = xref_pos;
    let _ = offsets;
    std::fs::write(&input, &pdf).unwrap();

    let r = repair_pdf(
        input.to_string_lossy().to_string(),
        output.to_string_lossy().to_string(),
        None,
    )
    .expect("repair objstm pdf");
    assert!(output.exists());
    assert!(r.page_count >= 1);
    Document::load(&output).expect("repaired objstm pdf should load");
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
