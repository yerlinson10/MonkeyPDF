use crate::error::{AppError, OpResult};
use crate::pdf_engine::ensure_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// LibreOffice is effectively single-instance; parallel soffice calls corrupt each other.
static LIBREOFFICE_LOCK: Mutex<()> = Mutex::new(());

/// Locate LibreOffice `soffice` on this machine.
/// On Windows prefer `soffice.com` (console) over `soffice.exe` (GUI stub).
pub fn find_soffice() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files\LibreOffice\program\soffice.com",
            r"C:\Program Files\LibreOffice\program\soffice.exe",
            r"C:\Program Files (x86)\LibreOffice\program\soffice.com",
            r"C:\Program Files (x86)\LibreOffice\program\soffice.exe",
        ];
        for c in candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
    }

    if let Some(p) = which("soffice.com").or_else(|| which("soffice")) {
        return Some(prefer_com_sibling(p));
    }
    if let Some(p) = which("libreoffice") {
        return Some(prefer_com_sibling(p));
    }

    #[cfg(target_os = "macos")]
    {
        let p = PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice");
        if p.exists() {
            return Some(p);
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for c in ["/usr/bin/soffice", "/usr/bin/libreoffice", "/snap/bin/libreoffice"] {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

fn prefer_com_sibling(path: PathBuf) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("exe"))
            .unwrap_or(false)
        {
            let com = path.with_extension("com");
            if com.exists() {
                return com;
            }
        }
    }
    path
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
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().next()?.trim();
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
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().next()?.trim();
        if line.is_empty() {
            return None;
        }
        Some(PathBuf::from(line))
    }
}

pub fn soffice_available() -> bool {
    find_soffice().is_some()
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn is_writer_target(target: &str) -> bool {
    matches!(target, "docx" | "odt" | "html" | "doc" | "rtf")
}

fn is_impress_target(target: &str) -> bool {
    matches!(target, "pptx" | "odp")
}

fn is_calc_target(target: &str) -> bool {
    matches!(target, "xlsx" | "ods" | "csv")
}

/// Export filter for `--convert-to`.
fn convert_to_arg(target: &str, src_ext: &str) -> String {
    match target {
        "pdf" => match src_ext {
            "doc" | "docx" | "odt" | "rtf" | "txt" | "html" | "htm" => {
                "pdf:writer_pdf_Export".into()
            }
            "xls" | "xlsx" | "ods" | "csv" => "pdf:calc_pdf_Export".into(),
            "ppt" | "pptx" | "odp" => "pdf:impress_pdf_Export".into(),
            // PDF opened via writer_pdf_import still exports with writer filter.
            "pdf" => "pdf:writer_pdf_Export".into(),
            _ => "pdf".into(),
        },
        // Keep export names simple — LO picks the right filter once the
        // document is in the correct module (Writer/Calc/Impress).
        "docx" | "xlsx" | "pptx" | "html" | "odt" | "ods" | "odp" => target.to_string(),
        other => other.to_string(),
    }
}

/// Import filter so PDF is opened in the right LO module (not Draw by default).
fn infilter_for(src_ext: &str, target: &str) -> Option<&'static str> {
    if src_ext != "pdf" {
        return None;
    }
    if is_writer_target(target) {
        Some("writer_pdf_import")
    } else if is_impress_target(target) {
        Some("draw_pdf_import")
    } else if is_calc_target(target) {
        // Calc has no solid PDF import; Writer import is the least-bad fallback.
        Some("writer_pdf_import")
    } else {
        Some("writer_pdf_import")
    }
}

fn file_uri(path: &Path) -> String {
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let s = abs.to_string_lossy().replace('\\', "/");
    let s = s.strip_prefix("//?/").unwrap_or(&s);
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

fn safe_stem(path: &Path) -> String {
    let raw = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted");
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.trim_matches('_').is_empty() {
        "converted".into()
    } else {
        cleaned
    }
}

struct WorkDir {
    root: PathBuf,
}

impl WorkDir {
    fn create() -> Result<Self, AppError> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("monkeypdf_lo_{stamp}_{}", std::process::id()));
        fs::create_dir_all(root.join("in"))?;
        fs::create_dir_all(root.join("out"))?;
        fs::create_dir_all(root.join("profile"))?;
        Ok(Self { root })
    }

    fn input_dir(&self) -> PathBuf {
        self.root.join("in")
    }
    fn output_dir(&self) -> PathBuf {
        self.root.join("out")
    }
    fn profile_dir(&self) -> PathBuf {
        self.root.join("profile")
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Convert Office/HTML ↔ PDF via LibreOffice headless.
///
/// Strategy to avoid LO/OneDrive/path issues:
/// 1. Copy input into `%TEMP%` with a simple ASCII name
/// 2. Isolated UserInstallation profile (no lock with GUI LO)
/// 3. Correct `--infilter` for PDF (Writer, not Draw)
/// 4. Convert into temp `out/`, then copy to the user folder
pub fn convert_with_libreoffice(
    path: String,
    target: String,
    output_dir: String,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let input = PathBuf::from(&path);
    if !input.exists() {
        return Err(AppError::InvalidInput(format!("File not found: {path}")));
    }

    let target = target.to_ascii_lowercase();
    let allowed = ["pdf", "docx", "xlsx", "pptx", "html", "odt", "ods", "odp"];
    if !allowed.contains(&target.as_str()) {
        return Err(AppError::InvalidInput(format!(
            "Formato no soportado: {target}"
        )));
    }

    let src_ext = ext_of(&input);
    if src_ext == target {
        return Err(AppError::InvalidInput(format!(
            "El archivo ya es .{target}"
        )));
    }

    let soffice = find_soffice().ok_or_else(|| {
        AppError::InvalidInput(
            "LibreOffice no encontrado. Instálalo desde libreoffice.org y reinicia MonkeyPDF."
                .into(),
        )
    })?;

    let final_out_dir = ensure_dir(&output_dir)?;

    // Serialize all soffice launches (GUI profile locks + parallel converts).
    let _guard = LIBREOFFICE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let work = WorkDir::create()?;

    // Sandbox input: avoid spaces, OneDrive locks, and weird Unicode during LO I/O.
    let sandbox_name = format!("input.{}", if src_ext.is_empty() { "bin" } else { &src_ext });
    let sandbox_input = work.input_dir().join(&sandbox_name);
    fs::copy(&input, &sandbox_input).map_err(|e| {
        AppError::Pdf(format!("No se pudo preparar el archivo para LibreOffice: {e}"))
    })?;

    let program_dir = soffice
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let profile_uri = file_uri(&work.profile_dir());
    let user_install = format!("-env:UserInstallation={profile_uri}");
    let convert_to = convert_to_arg(&target, &src_ext);
    let lo_out = work.output_dir();

    let mut cmd = Command::new(&soffice);
    cmd.current_dir(&program_dir);
    cmd.arg("--headless");
    cmd.arg("--nologo");
    cmd.arg("--nofirststartwizard");
    cmd.arg("--norestore");
    cmd.arg("--invisible");
    cmd.arg(&user_install);
    if let Some(infilter) = infilter_for(&src_ext, &target) {
        cmd.arg(format!("--infilter={infilter}"));
    }
    cmd.arg("--convert-to");
    cmd.arg(&convert_to);
    cmd.arg("--outdir");
    cmd.arg(&lo_out);
    cmd.arg(&sandbox_input);

    if let Ok(old_path) = std::env::var("PATH") {
        let sep = if cfg!(windows) { ";" } else { ":" };
        cmd.env(
            "PATH",
            format!("{}{sep}{old_path}", program_dir.display()),
        );
    } else {
        cmd.env("PATH", program_dir.as_os_str());
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .map_err(|e| AppError::Pdf(format!("No se pudo ejecutar LibreOffice: {e}")))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}\n{stdout}");

    // soffice sometimes returns before the file is fully flushed — poll briefly.
    let produced = wait_for_output(&lo_out, &sandbox_input, &target, 8_000).ok_or_else(|| {
        if combined.to_ascii_lowercase().contains("no export filter")
            || combined.contains("as a Draw document")
        {
            AppError::Pdf(
                "LibreOffice abrió el PDF en el módulo incorrecto. \
                 Prueba otra vez; si falla, convierte a ODT o reinstala el componente Writer de LibreOffice."
                    .into(),
            )
        } else if combined.contains("0xc10") || combined.to_ascii_lowercase().contains("impl_store")
        {
            AppError::Pdf(
                "LibreOffice no pudo guardar el archivo (bloqueo de escritura). \
                 Elige una carpeta local (no OneDrive) o cierra el archivo si está abierto."
                    .into(),
            )
        } else {
            AppError::Pdf(format!(
                "LibreOffice no generó el archivo. {}",
                summarize_lo_error(&combined)
            ))
        }
    })?;

    // Deliver to the user folder with the original (sanitized) stem.
    let dest_name = format!("{}.{}", safe_stem(&input), target);
    let dest = unique_path(&final_out_dir.join(dest_name));
    fs::copy(&produced, &dest).map_err(|e| {
        AppError::Pdf(format!(
            "La conversión OK, pero no se pudo copiar a la carpeta de salida: {e}. \
             Prueba una carpeta local fuera de OneDrive."
        ))
    })?;

    // `work` Drop cleans temp profile/in/out.
    Ok(OpResult::new(
        vec![dest.to_string_lossy().to_string()],
        1,
        started.elapsed().as_millis() as u64,
    ))
}

/// Convert a PDF to PDF/A-1b / A-2b / A-3b via LibreOffice Writer export.
/// `version`: 1 | 2 | 3 (SelectPdfVersion).
pub fn convert_to_pdfa(
    path: String,
    version: u8,
    output_dir: String,
) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let version = match version {
        1 | 2 | 3 => version,
        _ => {
            return Err(AppError::InvalidInput(
                "Versión PDF/A inválida (1, 2 o 3)".into(),
            ))
        }
    };

    let input = PathBuf::from(&path);
    if !input.exists() {
        return Err(AppError::InvalidInput(format!("File not found: {path}")));
    }
    if ext_of(&input) != "pdf" {
        return Err(AppError::InvalidInput(
            "PDF/A solo acepta archivos PDF".into(),
        ));
    }

    let soffice = find_soffice().ok_or_else(|| {
        AppError::InvalidInput(
            "LibreOffice no encontrado. Instálalo desde libreoffice.org y reinicia MonkeyPDF."
                .into(),
        )
    })?;

    let final_out_dir = ensure_dir(&output_dir)?;

    let _guard = LIBREOFFICE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let work = WorkDir::create()?;
    let sandbox_input = work.input_dir().join("input.pdf");
    fs::copy(&input, &sandbox_input).map_err(|e| {
        AppError::Pdf(format!("No se pudo preparar el archivo para LibreOffice: {e}"))
    })?;

    let program_dir = soffice
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let profile_uri = file_uri(&work.profile_dir());
    let user_install = format!("-env:UserInstallation={profile_uri}");
    // LibreOffice JSON filter: SelectPdfVersion 1=A-1b, 2=A-2b, 3=A-3b
    let convert_to = format!(
        r#"pdf:writer_pdf_Export:{{"SelectPdfVersion":{{"type":"long","value":"{version}"}}}}"#
    );
    let lo_out = work.output_dir();

    let mut cmd = Command::new(&soffice);
    cmd.current_dir(&program_dir);
    cmd.arg("--headless");
    cmd.arg("--nologo");
    cmd.arg("--nofirststartwizard");
    cmd.arg("--norestore");
    cmd.arg("--invisible");
    cmd.arg(&user_install);
    cmd.arg("--infilter=writer_pdf_import");
    cmd.arg("--convert-to");
    cmd.arg(&convert_to);
    cmd.arg("--outdir");
    cmd.arg(&lo_out);
    cmd.arg(&sandbox_input);

    if let Ok(old_path) = std::env::var("PATH") {
        let sep = if cfg!(windows) { ";" } else { ":" };
        cmd.env(
            "PATH",
            format!("{}{sep}{old_path}", program_dir.display()),
        );
    } else {
        cmd.env("PATH", program_dir.as_os_str());
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .map_err(|e| AppError::Pdf(format!("No se pudo ejecutar LibreOffice: {e}")))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}\n{stdout}");

    let produced = wait_for_output(&lo_out, &sandbox_input, "pdf", 12_000).ok_or_else(|| {
        AppError::Pdf(format!(
            "LibreOffice no generó PDF/A. {}",
            summarize_lo_error(&combined)
        ))
    })?;

    let dest_name = format!("{}_pdfa{}.pdf", safe_stem(&input), version);
    let dest = unique_path(&final_out_dir.join(dest_name));
    fs::copy(&produced, &dest).map_err(|e| {
        AppError::Pdf(format!(
            "Conversión OK, pero no se pudo copiar a la carpeta de salida: {e}"
        ))
    })?;

    Ok(OpResult::new(
        vec![dest.to_string_lossy().to_string()],
        1,
        started.elapsed().as_millis() as u64,
    ))
}

fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    for i in 1..1000 {
        let candidate = if ext.is_empty() {
            parent.join(format!("{stem}_{i}"))
        } else {
            parent.join(format!("{stem}_{i}.{ext}"))
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

fn wait_for_output(
    out_dir: &Path,
    input: &Path,
    target: &str,
    timeout_ms: u64,
) -> Option<PathBuf> {
    let start = Instant::now();
    loop {
        if let Some(p) = find_output(out_dir, input, target) {
            // Ensure size is stable (flush finished).
            let len1 = p.metadata().ok()?.len();
            std::thread::sleep(std::time::Duration::from_millis(150));
            let len2 = p.metadata().ok()?.len();
            if len1 > 0 && len1 == len2 {
                return Some(p);
            }
        }
        if start.elapsed().as_millis() as u64 >= timeout_ms {
            return find_output(out_dir, input, target);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

fn find_output(out_dir: &Path, input: &Path, target: &str) -> Option<PathBuf> {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("input");
    let expected = out_dir.join(format!("{stem}.{target}"));
    if expected.exists() && expected.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Some(expected);
    }

    std::fs::read_dir(out_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case(target))
                    .unwrap_or(false)
                && p.metadata().map(|m| m.len() > 0).unwrap_or(false)
        })
        .max_by_key(|p| {
            p.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        })
}

fn summarize_lo_error(raw: &str) -> String {
    let interesting: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !l.contains("platform independent libraries") && !l.contains("<prefix>"))
        .collect();
    if interesting.is_empty() {
        "Revisa que el archivo no esté abierto o dañado.".into()
    } else {
        interesting.into_iter().take(3).collect::<Vec<_>>().join(" ")
    }
}
