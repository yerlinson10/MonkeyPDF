use crate::error::AppError;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SignatureKind {
    Signature,
    Initials,
    Logo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SignatureMethod {
    Type,
    Draw,
    Upload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureAssetMeta {
    pub id: String,
    pub kind: SignatureKind,
    pub name: Option<String>,
    pub method: SignatureMethod,
    pub font: Option<String>,
    pub color: Option<String>,
    /// PNG as data URL for the UI preview.
    pub png_data_url: String,
    #[serde(default)]
    pub source: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSignatureAsset {
    pub id: Option<String>,
    pub kind: SignatureKind,
    pub name: Option<String>,
    pub method: SignatureMethod,
    pub font: Option<String>,
    pub color: Option<String>,
    pub png_data_url: String,
    #[serde(default)]
    pub source: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignatureDiskMeta {
    id: String,
    kind: SignatureKind,
    name: Option<String>,
    method: SignatureMethod,
    font: Option<String>,
    color: Option<String>,
    #[serde(default)]
    source: Value,
}

pub fn signatures_dir(base: &Path) -> PathBuf {
    base.join("signatures")
}

pub fn ensure_signatures_dir(base: &Path) -> Result<PathBuf, AppError> {
    let dir = signatures_dir(base);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn list_signatures(base: &Path) -> Result<Vec<SignatureAssetMeta>, AppError> {
    let dir = ensure_signatures_dir(base)?;
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let meta: SignatureDiskMeta = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let png_path = dir.join(format!("{}.png", meta.id));
        let png_bytes = match std::fs::read(&png_path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        out.push(SignatureAssetMeta {
            id: meta.id,
            kind: meta.kind,
            name: meta.name,
            method: meta.method,
            font: meta.font,
            color: meta.color,
            png_data_url: format!("data:image/png;base64,{}", B64.encode(png_bytes)),
            source: meta.source,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub fn save_signature(base: &Path, input: NewSignatureAsset) -> Result<SignatureAssetMeta, AppError> {
    let dir = ensure_signatures_dir(base)?;
    let id = input.id.unwrap_or_else(new_id);
    let png_bytes = decode_data_url_png(&input.png_data_url)?;
    if png_bytes.is_empty() {
        return Err(AppError::InvalidInput("PNG de firma vacío".into()));
    }

    let disk = SignatureDiskMeta {
        id: id.clone(),
        kind: input.kind.clone(),
        name: input.name.clone(),
        method: input.method.clone(),
        font: input.font.clone(),
        color: input.color.clone(),
        source: input.source.clone(),
    };
    let json_path = dir.join(format!("{id}.json"));
    let png_path = dir.join(format!("{id}.png"));
    let json = serde_json::to_string_pretty(&disk)
        .map_err(|e| AppError::Pdf(format!("JSON error: {e}")))?;
    std::fs::write(&json_path, json)?;
    std::fs::write(&png_path, &png_bytes)?;

    Ok(SignatureAssetMeta {
        id,
        kind: input.kind,
        name: input.name,
        method: input.method,
        font: input.font,
        color: input.color,
        png_data_url: format!("data:image/png;base64,{}", B64.encode(png_bytes)),
        source: input.source,
    })
}

pub fn delete_signature(base: &Path, id: &str) -> Result<(), AppError> {
    if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(AppError::InvalidInput("ID de firma inválido".into()));
    }
    let dir = ensure_signatures_dir(base)?;
    let _ = std::fs::remove_file(dir.join(format!("{id}.json")));
    let _ = std::fs::remove_file(dir.join(format!("{id}.png")));
    Ok(())
}

pub fn load_png(base: &Path, id: &str) -> Result<Vec<u8>, AppError> {
    if id.is_empty() || id.contains(['/', '\\']) {
        return Err(AppError::InvalidInput("ID de firma inválido".into()));
    }
    let path = signatures_dir(base).join(format!("{id}.png"));
    if !path.exists() {
        return Err(AppError::InvalidInput(format!(
            "Firma no encontrada: {id}"
        )));
    }
    Ok(std::fs::read(path)?)
}

fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("sig_{nanos}")
}

fn decode_data_url_png(data_url: &str) -> Result<Vec<u8>, AppError> {
    let raw = if let Some(rest) = data_url.strip_prefix("data:") {
        let comma = rest
            .find(',')
            .ok_or_else(|| AppError::InvalidInput("data URL inválida".into()))?;
        &rest[comma + 1..]
    } else {
        data_url
    };
    B64.decode(raw.trim())
        .map_err(|e| AppError::InvalidInput(format!("Base64 inválido: {e}")))
}
