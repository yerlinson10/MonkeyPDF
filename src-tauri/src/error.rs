use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("PDF error: {0}")]
    Pdf(String),

    #[error("PDFium error: {0}")]
    Pdfium(String),

    #[error("Image error: {0}")]
    Image(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

impl From<lopdf::Error> for AppError {
    fn from(value: lopdf::Error) -> Self {
        AppError::Pdf(value.to_string())
    }
}

impl From<image::ImageError> for AppError {
    fn from(value: image::ImageError) -> Self {
        AppError::Image(value.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub output_paths: Vec<String>,
    pub page_count: u32,
    pub elapsed_ms: u64,
    /// True when output is a best-effort salvage, not a full structural repair.
    #[serde(default)]
    pub partial: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl OpResult {
    pub fn new(output_paths: Vec<String>, page_count: u32, elapsed_ms: u64) -> Self {
        Self {
            output_paths,
            page_count,
            elapsed_ms,
            partial: false,
            warnings: Vec::new(),
        }
    }

    pub fn partial(
        output_paths: Vec<String>,
        page_count: u32,
        elapsed_ms: u64,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            output_paths,
            page_count,
            elapsed_ms,
            partial: true,
            warnings,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub data_url: String,
    pub page_count: u32,
    pub page: u32,
    pub kind: String,
    /// Positioned text spans (normalized 0–1, top-left) for selection/copy in the UI.
    #[serde(default)]
    pub text_spans: Vec<PreviewTextSpan>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTextSpan {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
