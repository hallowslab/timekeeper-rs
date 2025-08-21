use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use chrono::{DateTime, Datelike, Local};

use crate::exiftool;
use crate::stats::Stats;

lazy_static::lazy_static! {
    static ref SUPPORTED_EXTENSIONS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        // Image formats
        set.insert("jpg");
        set.insert("jpeg");
        set.insert("png");
        set.insert("tiff");
        set.insert("tif");
        set.insert("raw");
        set.insert("cr2");
        set.insert("nef");
        set.insert("arw");
        set.insert("dng");

        // Video formats
        set.insert("mp4");
        set.insert("mov");
        set.insert("avi");
        set.insert("mkv");
        set.insert("wmv");
        set.insert("m4v");
        set.insert("3gp");
        set.insert("webm");

        // Other formats
        set.insert("webp");
        set.insert("gif");

        set
    };
}

pub fn is_media_file(filename: &str) -> bool {
    if let Some(extension) = std::path::Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
    {
        SUPPORTED_EXTENSIONS.contains(&extension.to_lowercase().as_str())
    } else {
        false
    }
}

pub fn process_with_exiftool(
    exiftool_path: &PathBuf,
    source_path: &PathBuf,
    dest_base: &PathBuf,
    dry_run: bool,
    stats: &Arc<Stats>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Extract datetime using ExifTool
    let datetime = exiftool::extract_datetime(exiftool_path, source_path)?;
    let month_name = datetime.format("%B").to_string();

    // Determine destination directory
    let dest_dir = dest_base
        .join(datetime.year().to_string())
        .join(&month_name);

    // Check if the file is already in the correct directory
    if let Some(current_dir) = source_path.parent() {
        if current_dir == dest_dir {
            stats.skipped.fetch_add(1, Ordering::SeqCst);
            println!("[SKIP] Already in correct folder: {}", source_path.display());
            return Ok(());
        }
    }

    let filename = source_path
        .file_name()
        .ok_or("Invalid filename")?;

    let dest_path = dest_dir.join(filename);
    let unique_dest_path = get_unique_file_path(&dest_path);

    let prefix = if dry_run { "[DRY RUN] " } else { "" };
    println!(
        "{}Moving: {} -> {}",
        prefix,
        source_path.display(),
        unique_dest_path.display()
    );

    if !dry_run {
        // Create destination directory
        fs::create_dir_all(&dest_dir)?;

        // Move the file
        if let Err(e) = fs::rename(source_path, &unique_dest_path) {
            match e.kind() {
                #[cfg(unix)]
                std::io::ErrorKind::CrossDeviceLink => {
                    // TODO: handle cross-device move (rename across filesystems)
                }
                _ => return Err(e.into()),
            }
        }
    }

    Ok(())
}


pub fn process_file_with_fallback(
    source_path: &PathBuf,
    dest_base: &PathBuf,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use file modification time as fallback
    let metadata = fs::metadata(source_path)?;
    let mod_time = metadata.modified()?;
    let datetime: DateTime<Local> = mod_time.into();
    let month_name = datetime.format("%B").to_string();
    
    // Create destination directory structure
    let dest_dir = dest_base
        .join(datetime.year().to_string())
        .join(format!("{}", month_name));
    
    let filename = source_path
        .file_name()
        .ok_or("Invalid filename")?;
    
    let dest_path = dest_dir.join(filename);
    let unique_dest_path = get_unique_file_path(&dest_path);
    
    let prefix = if dry_run { "[DRY RUN] " } else { "" };
    println!(
        "{}[FALLBACK] Moving: {} -> {}",
        prefix,
        source_path.display(),
        unique_dest_path.display()
    );
    
    if !dry_run {
        // Create destination directory
        fs::create_dir_all(&dest_dir)?;
        
        // Move the file
        fs::rename(source_path, &unique_dest_path)?;
    }
    
    Ok(())
}

fn get_unique_file_path(original_path: &PathBuf) -> PathBuf {
    if !original_path.exists() {
        return original_path.clone();
    }
    
    let parent = original_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let file_stem = original_path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let extension = original_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    
    let mut counter = 1;
    loop {
        let new_filename = if extension.is_empty() {
            format!("{}_{}", file_stem, counter)
        } else {
            format!("{}_{}.{}", file_stem, counter, extension)
        };
        
        let new_path = parent.join(new_filename);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}