use crate::error::AppError;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

pub fn clear_cancel() {
    CANCEL_FLAG.store(false, Ordering::SeqCst);
}

pub fn request_cancel() {
    CANCEL_FLAG.store(true, Ordering::SeqCst);
}

pub fn is_cancelled() -> bool {
    CANCEL_FLAG.load(Ordering::SeqCst)
}

pub fn check_cancelled() -> Result<(), AppError> {
    if is_cancelled() {
        Err(AppError::Cancelled)
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub current: u32,
    pub total: u32,
    pub label: String,
}

/// Emits `job-progress` events and cooperates with the global cancel flag.
#[derive(Clone)]
pub struct Progress {
    app: Option<AppHandle>,
}

impl Progress {
    pub fn new(app: Option<AppHandle>) -> Self {
        Self { app }
    }

    pub fn none() -> Self {
        Self::new(None)
    }

    pub fn emit(&self, current: u32, total: u32, label: impl Into<String>) {
        let Some(app) = &self.app else {
            return;
        };
        let _ = app.emit(
            "job-progress",
            ProgressEvent {
                current,
                total,
                label: label.into(),
            },
        );
    }

    /// Check cancel, then emit progress. Call from page/file loops.
    pub fn tick(&self, current: u32, total: u32, label: impl Into<String>) -> Result<(), AppError> {
        check_cancelled()?;
        self.emit(current, total, label);
        Ok(())
    }
}
