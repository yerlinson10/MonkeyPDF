use crate::error::{AppError, OpResult};
use crate::pdf_engine::{ensure_parent_dir, ensure_pdf_path};
use lopdf::{Document, Object, ObjectId};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageRef {
    pub source_path: String,
    /// 1-based page index in the source PDF
    pub page: u32,
    /// Additional rotation to apply (0/90/180/270)
    #[serde(default)]
    pub rotate: u32,
}

/// Build a new PDF from an ordered list of page references (possibly from multiple files).
pub fn organize_pdf(pages: Vec<PageRef>, output: String) -> Result<OpResult, AppError> {
    let started = Instant::now();
    if pages.is_empty() {
        return Err(AppError::InvalidInput(
            "Añade al menos una página".into(),
        ));
    }

    let output_path = Path::new(&output);
    ensure_parent_dir(output_path)?;

    // Cache loaded source documents
    let mut sources: HashMap<String, Document> = HashMap::new();
    for pref in &pages {
        if !sources.contains_key(&pref.source_path) {
            let p = ensure_pdf_path(&pref.source_path)?;
            let doc = Document::load(p)?;
            if doc.is_encrypted() {
                return Err(AppError::InvalidInput(format!(
                    "Desbloquea primero: {}",
                    pref.source_path
                )));
            }
            sources.insert(pref.source_path.clone(), doc);
        }
        let doc = sources.get(&pref.source_path).unwrap();
        let total = doc.get_pages().len() as u32;
        if pref.page == 0 || pref.page > total {
            return Err(AppError::InvalidInput(format!(
                "Página {} fuera de rango en {} (1-{total})",
                pref.page, pref.source_path
            )));
        }
        if !matches!(pref.rotate, 0 | 90 | 180 | 270) {
            return Err(AppError::InvalidInput(
                "Rotación debe ser 0, 90, 180 o 270".into(),
            ));
        }
    }

    // Extract each requested page into its own mini-doc then merge (reuses merge renumber logic).
    let mut max_id = 1u32;
    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();
    let mut document = Document::with_version("1.5");

    for pref in &pages {
        let src = sources.get(&pref.source_path).unwrap();
        let page_map = src.get_pages();
        let &page_id = page_map
            .get(&pref.page)
            .ok_or_else(|| AppError::Pdf(format!("Página {} no encontrada", pref.page)))?;

        // Clone only the needed page tree into a temporary document.
        let mut temp = Document::with_version("1.5");
        let pages_id = temp.new_object_id();

        // Deep-copy page object + dependencies into temp
        let new_page_id = copy_object_deep(src, &mut temp, page_id, &mut HashMap::new())?;

        // Apply extra rotation
        if pref.rotate != 0 {
            if let Ok(Object::Dictionary(dict)) = temp.get_object_mut(new_page_id) {
                let current = dict
                    .get(b"Rotate")
                    .and_then(|o| o.as_i64())
                    .unwrap_or(0);
                let new_rot = (current + pref.rotate as i64).rem_euclid(360);
                dict.set("Rotate", new_rot);
            }
        }

        if let Ok(Object::Dictionary(dict)) = temp.get_object_mut(new_page_id) {
            dict.set("Parent", pages_id);
            dict.set("Type", "Page");
        }

        let mut pages_dict = lopdf::Dictionary::new();
        pages_dict.set("Type", "Pages");
        pages_dict.set("Count", 1_i64);
        pages_dict.set("Kids", vec![Object::Reference(new_page_id)]);
        temp.objects
            .insert(pages_id, Object::Dictionary(pages_dict));

        let mut catalog = lopdf::Dictionary::new();
        catalog.set("Type", "Catalog");
        catalog.set("Pages", pages_id);
        let catalog_id = temp.add_object(catalog);
        temp.trailer.set("Root", catalog_id);
        temp.max_id = temp.objects.keys().map(|id| id.0).max().unwrap_or(1);

        // Merge into destination (same algorithm as merge.rs)
        temp.renumber_objects_with(max_id);
        max_id = temp.max_id + 1;

        let t_pages = temp.get_pages();
        for (_, object_id) in t_pages {
            let object = temp.get_object(object_id)?.to_owned();
            documents_pages.insert(object_id, object);
        }
        documents_objects.extend(temp.objects);
    }

    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for (object_id, object) in documents_objects {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                catalog_object = Some((
                    catalog_object.map(|(id, _)| id).unwrap_or(object_id),
                    object,
                ));
            }
            b"Pages" => {
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref existing)) = pages_object {
                        if let Ok(old_dictionary) = existing.as_dict() {
                            dictionary.extend(old_dictionary);
                        }
                    }
                    pages_object = Some((
                        pages_object.map(|(id, _)| id).unwrap_or(object_id),
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                document.objects.insert(object_id, object);
            }
        }
    }

    let pages_object =
        pages_object.ok_or_else(|| AppError::Pdf("Pages root not found".into()))?;
    let catalog_object =
        catalog_object.ok_or_else(|| AppError::Pdf("Catalog root not found".into()))?;

    for (object_id, object) in &documents_pages {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_object.0);
            document
                .objects
                .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    let (page_id, page_object) = pages_object;
    let page_count = documents_pages.len() as u32;

    if let Ok(dictionary) = page_object.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Count", page_count);
        dictionary.set(
            "Kids",
            documents_pages
                .into_iter()
                .map(|(object_id, _)| Object::Reference(object_id))
                .collect::<Vec<_>>(),
        );
        document
            .objects
            .insert(page_id, Object::Dictionary(dictionary));
    }

    let (catalog_id, catalog_obj) = catalog_object;
    if let Ok(dictionary) = catalog_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", page_id);
        dictionary.remove(b"Outlines");
        document
            .objects
            .insert(catalog_id, Object::Dictionary(dictionary));
    }

    document.trailer.set("Root", catalog_id);
    document.max_id = document.objects.len() as u32;
    document.renumber_objects();
    document.compress();
    document.save(output_path)?;

    Ok(OpResult::new(
        vec![output],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn copy_object_deep(
    src: &Document,
    dest: &mut Document,
    id: ObjectId,
    map: &mut HashMap<ObjectId, ObjectId>,
) -> Result<ObjectId, AppError> {
    if let Some(&existing) = map.get(&id) {
        return Ok(existing);
    }

    let obj = src
        .get_object(id)
        .map_err(|e| AppError::Pdf(e.to_string()))?
        .clone();

    // Reserve id early to break cycles
    let new_id = dest.new_object_id();
    map.insert(id, new_id);

    let rewritten = rewrite_refs(src, dest, obj, map)?;
    dest.objects.insert(new_id, rewritten);
    Ok(new_id)
}

fn rewrite_refs(
    src: &Document,
    dest: &mut Document,
    obj: Object,
    map: &mut HashMap<ObjectId, ObjectId>,
) -> Result<Object, AppError> {
    match obj {
        Object::Reference(id) => {
            let new_id = copy_object_deep(src, dest, id, map)?;
            Ok(Object::Reference(new_id))
        }
        Object::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(rewrite_refs(src, dest, item, map)?);
            }
            Ok(Object::Array(out))
        }
        Object::Dictionary(dict) => {
            let mut out = lopdf::Dictionary::new();
            for (k, v) in dict.into_iter() {
                out.set(k, rewrite_refs(src, dest, v, map)?);
            }
            Ok(Object::Dictionary(out))
        }
        Object::Stream(mut stream) => {
            let mut new_dict = lopdf::Dictionary::new();
            for (k, v) in stream.dict.into_iter() {
                new_dict.set(k, rewrite_refs(src, dest, v, map)?);
            }
            stream.dict = new_dict;
            Ok(Object::Stream(stream))
        }
        other => Ok(other),
    }
}
