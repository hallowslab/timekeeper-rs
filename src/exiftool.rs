use std::path::PathBuf;
use std::process::Command;

// Conditional compilation for bundled ExifTool on Windows
#[cfg(all(windows, feature = "bundled"))]
use include_dir::{include_dir, Dir};

#[cfg(all(windows, feature = "bundled"))]
static EXIFTOOL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/bin/windows");

pub fn get_exiftool_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    #[cfg(all(windows, feature = "bundled"))]
    {
        extract_bundled_exiftool()
    }
    
    #[cfg(not(all(windows, feature = "bundled")))]
    {
        get_system_exiftool()
    }
}

#[cfg(all(windows, feature = "bundled"))]
fn extract_bundled_exiftool() -> Result<PathBuf, Box<dyn std::error::Error>> {
    use std::fs;
    use std::io::Write;

    let temp_dir = std::env::temp_dir().join("timekeeper-exiftool");
    let exe_path = temp_dir.join("exiftool.exe");

    // Check if already extracted
    if exe_path.exists() {
        return Ok(exe_path);
    }

    // Create temp directory
    fs::create_dir_all(&temp_dir)?;

    // Extract main executable
    if let Some(exe_file) = EXIFTOOL_DIR.get_file("exiftool.exe") {
        let mut file = fs::File::create(&exe_path)?;
        file.write_all(exe_file.contents())?;
    } else {
        return Err("ExifTool executable not found in embedded files".into());
    }

    // Extract exiftool_files directory
    if let Some(files_dir) = EXIFTOOL_DIR.get_dir("exiftool_files") {
        extract_dir_recursive(files_dir, &temp_dir.join("exiftool_files"))?;
    }

    Ok(exe_path)
}

#[cfg(all(windows, feature = "bundled"))]
fn extract_dir_recursive(dir: &include_dir::Dir, dest_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::io::Write;

    fs::create_dir_all(dest_path)?;

    for file in dir.files() {
        let file_path = dest_path.join(file.path());
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output_file = fs::File::create(&file_path)?;
        output_file.write_all(file.contents())?;
    }

    for sub_dir in dir.dirs() {
        let sub_dest = dest_path.join(sub_dir.path());
        extract_dir_recursive(sub_dir, &sub_dest)?;
    }

    Ok(())
}

#[cfg(not(all(windows, feature = "bundled")))]
fn get_system_exiftool() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Try to find exiftool in PATH
    match which::which("exiftool") {
        Ok(path) => Ok(path),
        Err(_) => {
            let instructions = get_install_instructions();
            Err(format!("exiftool not found. Install it with:\n{}", instructions).into())
        }
    }
}

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

pub fn extract_datetime(exiftool_path: &PathBuf, file_path: &PathBuf) -> Result<chrono::DateTime<chrono::Local>, Box<dyn std::error::Error>> {
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

fn parse_exif_date(date_str: &str) -> Result<chrono::DateTime<chrono::Local>, Box<dyn std::error::Error>> {
    use chrono::{DateTime, Local, TimeZone, NaiveDateTime};

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
            return Ok(Local.from_local_datetime(&naive_dt)
                .single()
                .unwrap_or_else(|| Local.from_utc_datetime(&naive_dt)));
        }
    }

    Err("Invalid date format".into())
}