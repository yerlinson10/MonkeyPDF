use app_lib::pdf_engine;

#[test]
fn images_to_pdf_includes_all_pages() {
    let dir = std::env::temp_dir().join(format!("monkeypdf_imgpdf_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut paths = Vec::new();
    // Portrait, landscape, square — each must become its own page.
    let specs = [(120u32, 200u32), (300, 150), (180, 180)];
    for (i, (w, h)) in specs.iter().enumerate() {
        let img = image::RgbImage::from_fn(*w, *h, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, (i as u8).wrapping_mul(70)])
        });
        let p = dir.join(format!("p{i}.jpg"));
        img.save(&p).unwrap();
        paths.push(p.to_string_lossy().to_string());
    }

    let out = dir.join("out.pdf");
    let result =
        pdf_engine::images_to_pdf(paths, out.to_string_lossy().to_string()).expect("images_to_pdf");
    assert_eq!(result.page_count, 3, "OpResult page_count");
    assert!(out.exists());

    let doc = lopdf::Document::load(&out).unwrap();
    let pages = doc.get_pages();
    assert_eq!(
        pages.len(),
        3,
        "PDF page tree must list 3 pages, got {:?}",
        pages.keys().collect::<Vec<_>>()
    );

    // Each page MediaBox should match image orientation (portrait / landscape / square).
    let mut boxes = Vec::new();
    for (_num, page_id) in pages {
        let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
        let mb = page.get(b"MediaBox").unwrap().as_array().unwrap();
        let w = match &mb[2] {
            lopdf::Object::Real(v) => *v,
            lopdf::Object::Integer(v) => *v as f32,
            _ => panic!("bad MediaBox width"),
        };
        let h = match &mb[3] {
            lopdf::Object::Real(v) => *v,
            lopdf::Object::Integer(v) => *v as f32,
            _ => panic!("bad MediaBox height"),
        };
        boxes.push((w, h));
    }
    boxes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    // Landscape wider than tall, portrait taller than wide, square equal.
    assert!(boxes.iter().any(|(w, h)| w > h), "expected a landscape page");
    assert!(boxes.iter().any(|(w, h)| h > w), "expected a portrait page");
    assert!(
        boxes.iter().any(|(w, h)| (w - h).abs() < 1.0),
        "expected a square page"
    );
}
