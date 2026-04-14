use std::path::{Path, PathBuf};
use std::process::Command;

// Conditional imports for bundled ExifTool on Windows
#[cfg(all(windows, feature = "bundled"))]
use include_dir::{Dir, include_dir};

#[cfg(all(windows, feature = "bundled"))]
static EXIFTOOL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/bin/windows");

// Error Type
/// Typed error for ExifTool resolution failures.
/// This replaces all `Box<dyn Error>` usage in the resolution layer.
/// Callers (CLI, Tauri) decide how to surface these errors to users.
#[derive(Debug)]
pub enum ExifToolError {
    /// No valid ExifTool binary found in any resolution source.
    /// Contains platform-specific installation instructions.
    NotFound { instructions: &'static str },
    /// Bundled ExifTool extraction failed (I/O error or missing embedded asset).
    ExtractionFailed(String),
    /// A candidate binary exists but failed the `exiftool -ver` validation check.
    ValidationFailed(String),
    /// The user-supplied path was explicitly provided but is invalid.
    /// This is a hard failure — no fallthrough to other sources.
    UserPathInvalid(String),
}

impl std::fmt::Display for ExifToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExifToolError::NotFound { instructions } => {
                write!(f, "ExifTool not found. Install it with:\n{}", instructions)
            }
            ExifToolError::ExtractionFailed(msg) => {
                write!(f, "Failed to extract bundled ExifTool: {}", msg)
            }
            ExifToolError::ValidationFailed(msg) => {
                write!(f, "ExifTool validation failed: {}", msg)
            }
            ExifToolError::UserPathInvalid(msg) => {
                write!(f, "User-specified ExifTool path is invalid: {}", msg)
            }
        }
    }
}

impl std::error::Error for ExifToolError {}

// Public API

/// Resolve and validate a path to a working ExifTool binary.
/// Resolution follows a fixed priority order:
/// 1. **User path** (`Some(path)`) — validated, hard failure if invalid
/// 2. **Bundled binary** (Windows + `bundled` feature) — extracted next to exe, hard failure if broken
/// 3. **System PATH** — `which::which("exiftool")`, final fallback
/// Returns the first validated `PathBuf` or a typed error.
/// This function is deterministic for identical inputs.
pub fn get_exiftool_path(user_path: Option<PathBuf>) -> Result<PathBuf, ExifToolError> {
    // Source 1: User-specified path (highest priority)
    if let Some(path) = user_path {
        validate_exiftool(&path).map_err(|_| {
            ExifToolError::UserPathInvalid(format!(
                "Path '{}' does not point to a working ExifTool binary",
                path.display()
            ))
        })?;
        return Ok(path);
    }

    // Source 2: Bundled binary (Windows + bundled feature only)
    #[cfg(all(windows, feature = "bundled"))]
    {
        let bundled_path = extract_bundled_exiftool()?;
        validate_exiftool(&bundled_path)?;
        return Ok(bundled_path);
    }

    // Source 3: System PATH (final fallback)
    #[cfg(not(all(windows, feature = "bundled")))]
    {
        resolve_system_exiftool()
    }
}

// Validation
/// Validate that a path points to a working ExifTool binary.
/// Runs `exiftool -ver` and checks:
/// - Process exits successfully (exit code 0)
/// - stdout is non-empty (contains version string)
/// Does NOT mutate state. Does NOT log.
fn validate_exiftool(path: &Path) -> Result<(), ExifToolError> {
    let output = Command::new(path).arg("-ver").output().map_err(|e| {
        ExifToolError::ValidationFailed(format!("Failed to execute '{}': {}", path.display(), e))
    })?;

    if !output.status.success() {
        return Err(ExifToolError::ValidationFailed(format!(
            "'{}' exited with status: {}",
            path.display(),
            output.status
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Err(ExifToolError::ValidationFailed(format!(
            "'{}' produced no version output",
            path.display()
        )));
    }

    Ok(())
}

// System PATH Resolution
/// Locate ExifTool on the system PATH and validate it.
#[cfg(not(all(windows, feature = "bundled")))]
fn resolve_system_exiftool() -> Result<PathBuf, ExifToolError> {
    match which::which("exiftool") {
        Ok(path) => {
            validate_exiftool(&path)?;
            Ok(path)
        }
        Err(_) => Err(ExifToolError::NotFound {
            instructions: get_install_instructions(),
        }),
    }
}

/// Platform-specific install instructions, stored in the error — not printed.
#[cfg(not(all(windows, feature = "bundled")))]
fn get_install_instructions() -> &'static str {
    if cfg!(windows) {
        "winget install ExifTool"
    } else if cfg!(target_os = "linux") {
        "sudo apt install libimage-exiftool-perl"
    } else if cfg!(target_os = "macos") {
        "brew install exiftool"
    } else {
        "https://exiftool.org/"
    }
}

// Bundled Extraction (Windows + bundled feature only)
/// Extract the bundled ExifTool binary to a directory next to the running executable.
/// Extraction target: `<exe_dir>/exiftool/exiftool.exe`
/// Idempotent: if the binary already exists and passes validation, extraction is skipped.
/// On failure, returns `ExifToolError::ExtractionFailed` — does NOT fall through to system PATH.
#[cfg(all(windows, feature = "bundled"))]
fn extract_bundled_exiftool() -> Result<PathBuf, ExifToolError> {
    use std::fs;
    use std::io::Write;

    let exe_dir = std::env::current_exe()
        .map_err(|e| {
            ExifToolError::ExtractionFailed(format!("Cannot determine executable path: {}", e))
        })?
        .parent()
        .ok_or_else(|| {
            ExifToolError::ExtractionFailed("Executable has no parent directory".into())
        })?
        .to_path_buf();

    let extract_dir = exe_dir.join("exiftool");
    let exe_path = extract_dir.join("exiftool.exe");

    // Idempotent: if already extracted and valid, return immediately
    if exe_path.exists() && validate_exiftool(&exe_path).is_ok() {
        return Ok(exe_path);
    }

    // Create extraction directory
    fs::create_dir_all(&extract_dir).map_err(|e| {
        ExifToolError::ExtractionFailed(format!(
            "Cannot create directory '{}': {}",
            extract_dir.display(),
            e
        ))
    })?;

    // Find the files in the embedded directory (recursive search)
    let mut embedded_exe = None;
    let mut embedded_files_dir = None;

    // Helper to search recursively in include_dir::Dir
    fn find_in_dir<'a>(
        dir: &'a include_dir::Dir<'a>,
        exe_out: &mut Option<include_dir::File<'a>>,
        dir_out: &mut Option<&'a include_dir::Dir<'a>>,
    ) {
        for file in dir.files() {
            let name = file
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if name.to_lowercase().starts_with("exiftool") && name.to_lowercase().ends_with(".exe")
            {
                *exe_out = Some(file.clone());
            }
        }
        for sub in dir.dirs() {
            let name = sub
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if name == "exiftool_files" {
                *dir_out = Some(sub);
            }
            find_in_dir(sub, exe_out, dir_out);
        }
    }

    find_in_dir(&EXIFTOOL_DIR, &mut embedded_exe, &mut embedded_files_dir);

    // Extract main executable
    if let Some(exe_file) = embedded_exe {
        let mut file = fs::File::create(&exe_path).map_err(|e| {
            ExifToolError::ExtractionFailed(format!("Cannot create exiftool.exe: {}", e))
        })?;
        file.write_all(exe_file.contents()).map_err(|e| {
            ExifToolError::ExtractionFailed(format!("Cannot write exiftool.exe: {}", e))
        })?;
    } else {
        return Err(ExifToolError::ExtractionFailed(
            "ExifTool executable not found in embedded assets".into(),
        ));
    }

    // Extract exiftool_files directory
    if let Some(files_dir) = embedded_files_dir {
        extract_dir_recursive(files_dir, &extract_dir.join("exiftool_files"))?;
    } else {
        return Err(ExifToolError::ExtractionFailed(
            "exiftool_files directory not found in embedded assets".into(),
        ));
    }

    Ok(exe_path)
}

/// Recursively extract an embedded directory to disk.
/// This helper is correct and preserved as-is from the original implementation.
#[cfg(all(windows, feature = "bundled"))]
fn extract_dir_recursive(dir: &include_dir::Dir, dest_path: &PathBuf) -> Result<(), ExifToolError> {
    use std::fs;
    use std::io::Write;

    fs::create_dir_all(dest_path).map_err(|e| {
        ExifToolError::ExtractionFailed(format!(
            "Cannot create dir '{}': {}",
            dest_path.display(),
            e
        ))
    })?;

    for file in dir.files() {
        let file_path = dest_path.join(file.path().file_name().unwrap_or_default());
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                ExifToolError::ExtractionFailed(format!("Cannot create parent dir: {}", e))
            })?;
        }
        let mut output_file = fs::File::create(&file_path).map_err(|e| {
            ExifToolError::ExtractionFailed(format!(
                "Cannot create file '{}': {}",
                file_path.display(),
                e
            ))
        })?;
        output_file.write_all(file.contents()).map_err(|e| {
            ExifToolError::ExtractionFailed(format!(
                "Cannot write file '{}': {}",
                file_path.display(),
                e
            ))
        })?;
    }

    for sub_dir in dir.dirs() {
        let sub_name = sub_dir.path().file_name().unwrap_or_default();
        let sub_dest = dest_path.join(sub_name);
        extract_dir_recursive(sub_dir, &sub_dest)?;
    }

    Ok(())
}

// Metadata Extraction
pub fn extract_datetime(
    exiftool_path: &PathBuf,
    file_path: &PathBuf,
) -> Result<chrono::DateTime<chrono::Local>, Box<dyn std::error::Error>> {
    let date_fields = [
        "DateTimeOriginal",
        "CreateDate",
        "DateTime",
        "FileModifyDate",
    ];

    for field in &date_fields {
        let output = Command::new(exiftool_path)
            .args(["-s", "-s", "-s", &format!("-{}", field)])
            .arg(file_path)
            .output();

        match output {
            Ok(output) => {
                let date_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !date_str.is_empty() {
                    if let Ok(datetime) = parse_exif_date(&date_str) {
                        return Ok(datetime);
                    }
                }
            }
            Err(e) => {
                eprintln!("ExifTool command failed: {}", e);
                continue;
            }
        }
    }

    Err("No valid date found in EXIF data".into())
}

fn parse_exif_date(
    date_str: &str,
) -> Result<chrono::DateTime<chrono::Local>, Box<dyn std::error::Error>> {
    use chrono::{DateTime, Local, NaiveDateTime, TimeZone};

    let formats = [
        "%Y:%m:%d %H:%M:%S",
        "%Y:%m:%d %H:%M:%S%z",
        "%Y:%m:%d %H:%M:%SZ",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y-%m-%dT%H:%M:%S%z",
    ];

    for format in &formats {
        if let Ok(dt) = DateTime::parse_from_str(date_str, format) {
            return Ok(dt.with_timezone(&Local));
        }
        if let Ok(naive_dt) = NaiveDateTime::parse_from_str(date_str, format) {
            return Ok(Local
                .from_local_datetime(&naive_dt)
                .single()
                .unwrap_or_else(|| Local.from_utc_datetime(&naive_dt)));
        }
    }

    Err("Invalid date format".into())
}
