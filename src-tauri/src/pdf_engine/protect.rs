use crate::error::{AppError, OpResult};
use crate::pdf_engine::{create_pdfium, ensure_parent_dir, ensure_pdf_path};
use lopdf::encryption::crypt_filters::{Aes128CryptFilter, CryptFilter};
use lopdf::xref::XrefType;
use lopdf::{
    Document, EncryptionState, EncryptionVersion, Object, ObjectId, Permissions,
};
use pdfium_render::prelude::FPDF_FILEWRITE;

/// Matches `FPDF_REMOVE_SECURITY` from pdfium `fpdf_save.h` (not re-exported by pdfium-render).
const FPDF_REMOVE_SECURITY: u32 = 3;
use rand::RngCore;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::os::raw::{c_int, c_ulong, c_void};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

fn open_permissions() -> Permissions {
    Permissions::PRINTABLE
        | Permissions::COPYABLE
        | Permissions::COPYABLE_FOR_ACCESSIBILITY
        | Permissions::PRINTABLE_IN_HIGH_QUALITY
        | Permissions::MODIFIABLE
        | Permissions::ANNOTABLE
        | Permissions::FILLABLE
        | Permissions::ASSEMBLABLE
}

/// Password-protect a PDF (AES-128 / V4). Prefers `qpdf` when installed.
pub fn protect_pdf(
    path: String,
    user_password: String,
    owner_password: Option<String>,
    output: String,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    if user_password.is_empty() {
        return Err(AppError::InvalidInput(
            "La contraseña de usuario no puede estar vacía".into(),
        ));
    }

    let input = ensure_pdf_path(&path)?;
    let output_path = PathBuf::from(&output);
    ensure_parent_dir(&output_path)?;

    let owner = owner_password.unwrap_or_else(|| user_password.clone());

    if let Some(qpdf) = find_qpdf() {
        return protect_with_qpdf(&qpdf, &input, &user_password, &owner, &output_path, started);
    }

    let mut doc = Document::load(&input)?;
    if doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "El PDF ya está protegido. Desbloquéalo primero.".into(),
        ));
    }

    ensure_file_id(&mut doc);
    doc.reference_table.cross_reference_type = XrefType::CrossReferenceTable;
    strip_xref_streams(&mut doc);

    let crypt_filter: Arc<dyn CryptFilter> = Arc::new(Aes128CryptFilter);
    let version = EncryptionVersion::V4 {
        document: &doc,
        encrypt_metadata: true,
        crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), crypt_filter)]),
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: &owner,
        user_password: &user_password,
        permissions: open_permissions(),
    };
    let state = EncryptionState::try_from(version)
        .map_err(|e| AppError::Pdf(format!("No se pudo crear cifrado: {e}")))?;
    doc.encrypt(&state)
        .map_err(|e| AppError::Pdf(format!("Error al proteger PDF: {e}")))?;

    let page_count = doc.get_pages().len() as u32;
    doc.encryption_state = None;
    doc.save(&output_path)
        .map_err(|e| AppError::Pdf(format!("Error al guardar PDF protegido: {e}")))?;

    Ok(OpResult::new(
        vec![output],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

/// Remove password protection from a PDF.
///
/// Order: qpdf (if present) → PDFium `FPDF_REMOVE_SECURITY` → lopdf fallback.
/// PDFium is required for form PDFs; lopdf often leaves blank page content.
pub fn unlock_pdf(path: String, password: String, output: String) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = ensure_pdf_path(&path)?;
    let output_path = PathBuf::from(&output);
    ensure_parent_dir(&output_path)?;

    if let Some(qpdf) = find_qpdf() {
        match unlock_with_qpdf(&qpdf, &input, &password, &output_path, started) {
            Ok(r) => return Ok(r),
            Err(e) => {
                // Wrong password: stop. Other qpdf failures: try PDFium.
                if matches!(e, AppError::InvalidInput(_)) {
                    return Err(e);
                }
                log::warn!("qpdf unlock failed, trying PDFium: {e}");
            }
        }
    }

    match unlock_with_pdfium(&input, &password, &output_path, started) {
        Ok(r) => return Ok(r),
        Err(e) => {
            if matches!(e, AppError::InvalidInput(_)) {
                return Err(e);
            }
            log::warn!("PDFium unlock failed, trying lopdf: {e}");
        }
    }

    unlock_with_lopdf(&input, &password, &output_path, started)
}

fn unlock_with_pdfium(
    input: &Path,
    password: &str,
    output: &Path,
    started: Instant,
) -> Result<OpResult, AppError> {
    let pdfium = create_pdfium()?;
    let bindings = pdfium.bindings();
    let bytes = std::fs::read(input).map_err(AppError::Io)?;

    let pwd = if password.is_empty() {
        None
    } else {
        Some(password)
    };
    let handle = bindings.FPDF_LoadMemDocument(&bytes, pwd);
    if handle.is_null() {
        let err = bindings.FPDF_GetLastError();
        // FPDF_ERR_PASSWORD = 4
        if err == 4 {
            return Err(AppError::InvalidInput("Contraseña incorrecta".into()));
        }
        return Err(AppError::Pdf(format!(
            "PDFium no pudo abrir el PDF cifrado (código {err})"
        )));
    }

    let page_count = bindings.FPDF_GetPageCount(handle).max(0) as u32;

    let mut file = File::create(output).map_err(AppError::Io)?;

    // Extended FPDF_FILEWRITE carrying the Rust File pointer (same pattern as pdfium-render).
    #[repr(C)]
    struct FileWriteExt {
        version: c_int,
        write_block: Option<
            unsafe extern "C" fn(
                this: *mut FileWriteExt,
                data: *const c_void,
                size: c_ulong,
            ) -> c_int,
        >,
        file: *mut File,
    }

    unsafe extern "C" fn write_block(
        this: *mut FileWriteExt,
        data: *const c_void,
        size: c_ulong,
    ) -> c_int {
        let file = unsafe { &mut *(*this).file };
        let slice = unsafe { std::slice::from_raw_parts(data as *const u8, size as usize) };
        if file.write_all(slice).is_ok() {
            1
        } else {
            0
        }
    }

    let mut writer = FileWriteExt {
        version: 1,
        write_block: Some(write_block),
        file: &mut file as *mut File,
    };

    let ok = bindings.FPDF_SaveAsCopy(
        handle,
        &mut writer as *mut FileWriteExt as *mut FPDF_FILEWRITE,
        FPDF_REMOVE_SECURITY,
    );
    bindings.FPDF_CloseDocument(handle);

    if let Err(e) = file.flush() {
        return Err(AppError::Io(e));
    }
    drop(file);

    if !bindings.is_true(ok) {
        let _ = std::fs::remove_file(output);
        return Err(AppError::Pdf(
            "PDFium no pudo guardar el PDF sin cifrado".into(),
        ));
    }

    // Sanity: must reopen unencrypted.
    let check = Document::load(output)
        .map_err(|e| AppError::Pdf(format!("El PDF desbloqueado no se pudo reabrir: {e}")))?;
    if check.is_encrypted() {
        let _ = std::fs::remove_file(output);
        return Err(AppError::Pdf(
            "El PDF desbloqueado sigue marcándose como cifrado".into(),
        ));
    }

    Ok(OpResult::new(
        vec![output.to_string_lossy().to_string()],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn unlock_with_lopdf(
    input: &Path,
    password: &str,
    output: &Path,
    started: Instant,
) -> Result<OpResult, AppError> {
    let mut doc = Document::load(input)?;
    if !doc.is_encrypted() {
        return Err(AppError::InvalidInput(
            "Este PDF no está protegido con contraseña".into(),
        ));
    }

    doc.decrypt(password).map_err(|e| {
        AppError::InvalidInput(format!("Contraseña incorrecta o cifrado no soportado: {e}"))
    })?;

    // Minimal cleanup only — do NOT strip ObjStm / renumber (that blanks form PDFs).
    doc.encryption_state = None;
    if let Ok(Object::Reference(id)) = doc.trailer.get(b"Encrypt").map(|o| o.clone()) {
        let _ = doc.trailer.remove(b"Encrypt");
        doc.objects.remove(&id);
    } else {
        let _ = doc.trailer.remove(b"Encrypt");
    }

    let page_count = doc.get_pages().len() as u32;
    doc.save(output)
        .map_err(|e| AppError::Pdf(format!("Error al guardar PDF desbloqueado: {e}")))?;

    Ok(OpResult::new(
        vec![output.to_string_lossy().to_string()],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn strip_xref_streams(doc: &mut Document) {
    let ids: Vec<ObjectId> = doc
        .objects
        .iter()
        .filter_map(|(id, obj)| {
            let is_xref = obj
                .as_stream()
                .map(|s| s.dict.has_type(b"XRef"))
                .unwrap_or(false)
                || obj.type_name().ok() == Some(b"XRef");
            if is_xref {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    for id in ids {
        doc.objects.remove(&id);
    }
}

fn ensure_file_id(doc: &mut Document) {
    if doc.trailer.get(b"ID").is_ok() {
        return;
    }
    let mut a = [0u8; 16];
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut a);
    rand::thread_rng().fill_bytes(&mut b);
    doc.trailer.set(
        "ID",
        Object::Array(vec![
            Object::String(a.to_vec(), lopdf::StringFormat::Hexadecimal),
            Object::String(b.to_vec(), lopdf::StringFormat::Hexadecimal),
        ]),
    );
}

fn find_qpdf() -> Option<PathBuf> {
    which("qpdf").or_else(|| {
        #[cfg(target_os = "windows")]
        {
            for c in [
                r"C:\Program Files\qpdf\bin\qpdf.exe",
                r"C:\Program Files (x86)\qpdf\bin\qpdf.exe",
            ] {
                let p = PathBuf::from(c);
                if p.exists() {
                    return Some(p);
                }
            }
        }
        None
    })
}

fn which(cmd: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = Command::new("where")
            .arg(cmd)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let line = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()?
            .trim()
            .to_string();
        if line.is_empty() {
            return None;
        }
        Some(PathBuf::from(line))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("which").arg(cmd).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let line = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()?
            .trim()
            .to_string();
        if line.is_empty() {
            return None;
        }
        Some(PathBuf::from(line))
    }
}

fn run_cmd(mut cmd: Command) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd
        .output()
        .map_err(|e| AppError::Pdf(format!("No se pudo ejecutar qpdf: {e}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(AppError::Pdf(format!("qpdf falló: {stderr} {stdout}")))
}

fn protect_with_qpdf(
    qpdf: &Path,
    input: &Path,
    user: &str,
    owner: &str,
    output: &Path,
    started: Instant,
) -> Result<OpResult, AppError> {
    let mut cmd = Command::new(qpdf);
    cmd.args([
        "--encrypt",
        user,
        owner,
        "256",
        "--",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
    ]);
    run_cmd(cmd)?;
    let page_count = Document::load(input)
        .map(|d| d.get_pages().len() as u32)
        .unwrap_or(1);
    Ok(OpResult::new(
        vec![output.to_string_lossy().to_string()],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}

fn unlock_with_qpdf(
    qpdf: &Path,
    input: &Path,
    password: &str,
    output: &Path,
    started: Instant,
) -> Result<OpResult, AppError> {
    let mut cmd = Command::new(qpdf);
    cmd.args([
        &format!("--password={password}"),
        "--decrypt",
        &input.to_string_lossy(),
        &output.to_string_lossy(),
    ]);
    run_cmd(cmd).map_err(|e| {
        if e.to_string().to_ascii_lowercase().contains("password") {
            AppError::InvalidInput("Contraseña incorrecta".into())
        } else {
            e
        }
    })?;

    let page_count = Document::load(output)
        .map(|d| d.get_pages().len() as u32)
        .unwrap_or(1);
    Ok(OpResult::new(
        vec![output.to_string_lossy().to_string()],
        page_count,
        started.elapsed().as_millis() as u64,
    ))
}
