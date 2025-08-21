use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicBool};
use clap::Parser;
use clap::crate_version;
use rayon::prelude::*;

mod exiftool;
mod metadata;
mod stats;

use crate::metadata::{is_media_file, process_file_with_fallback, process_with_exiftool};
use crate::stats::Stats;

#[derive(Parser)]
#[command(version = crate_version!(), about = "A media file organizer that sorts files by date using EXIF metadata", name = "timekeeper")]
struct Args {
    /// Source file or directory
    #[arg(short = 's', long = "source")]
    source: PathBuf,

    /// Destination directory
    #[arg(short = 'd', long = "destination")]
    destination: PathBuf,

    /// Show what would be done without actually moving files
    #[arg(long = "dry-run")]
    dry_run: bool,

}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let stats = Arc::new(Stats::new());
    let terminate_flag = Arc::new(AtomicBool::new(false));

    // Register Ctrl+C handler
    {
        let terminate_flag = Arc::clone(&terminate_flag);
        ctrlc::set_handler(move || {
            println!("\n[INFO] Ctrl+C detected! Stopping gracefully...");
            terminate_flag.store(true, Ordering::SeqCst);
        })?;
    }

    process_path(
        &args.source,
        &args.destination,
        args.dry_run,
        &Arc::clone(&stats),
        &terminate_flag,
    )?;

    println!("\n[INFO] Finished processing or stopped by user.");
    stats.print();

    Ok(())
}


fn process_path(
    source_path: &PathBuf,
    dest_base: &PathBuf,
    dry_run: bool,
    stats: &Arc<Stats>,
    terminate_flag: &Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    if source_path.is_dir() {
        count_media_files(source_path, Arc::clone(&stats))?;
        process_directory(source_path, dest_base, dry_run, stats, terminate_flag)
    } else {
        stats.total.store(1, Ordering::SeqCst);
        process_single_file(source_path, dest_base, dry_run, stats, terminate_flag)
    }
}

fn count_media_files(
    source_dir: &PathBuf,
    stats: Arc<Stats>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in walkdir::WalkDir::new(source_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(path_str) = entry.path().to_str() {
                if is_media_file(path_str) {
                    stats.total.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
    }
    Ok(())
}

fn process_directory(
    source_dir: &PathBuf,
    dest_base: &PathBuf,
    dry_run: bool,
    stats: &Arc<Stats>,
    terminate_flag: &Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let entries: Vec<_> = walkdir::WalkDir::new(source_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .collect();

    stats.total.store(entries.len(), Ordering::SeqCst);

    entries.par_iter().for_each(|entry| {
        if terminate_flag.load(Ordering::SeqCst) {
            return; // stop processing this thread
        }

        let stats = Arc::clone(&stats);
        let terminate_flag = Arc::clone(terminate_flag);

        if let Some(path_str) = entry.path().to_str() {
            if is_media_file(path_str) {
                if let Err(e) =
                    process_single_file(&entry.path().to_path_buf(), dest_base, dry_run, &stats, &terminate_flag)
                {
                    eprintln!("Error processing {}: {}", entry.path().display(), e);
                    stats.errors.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
    });

    Ok(())
}

fn process_single_file(
    source_path: &PathBuf,
    dest_base: &PathBuf,
    dry_run: bool,
    stats: &Arc<Stats>,
    terminate_flag: &Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    if terminate_flag.load(Ordering::SeqCst) {
        return Ok(()); // skip if termination requested
    }

    stats.processed.fetch_add(1, Ordering::SeqCst);

    let filename = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    match exiftool::get_exiftool_path() {
        Ok(exiftool_path) => {
            match process_with_exiftool(&exiftool_path, source_path, dest_base, dry_run, &stats) {
                Ok(()) => {
                    stats.exif_count.fetch_add(1, Ordering::SeqCst);
                    println!(
                        "[{}/{}] Processed: {} (EXIF)",
                        stats.processed.load(Ordering::SeqCst),
                        stats.total.load(Ordering::SeqCst),
                        filename
                    );
                }
                Err(_) => {
                    process_file_with_fallback(source_path, dest_base, dry_run)?;
                    stats.fallback_count.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
        Err(_) => {
            process_file_with_fallback(source_path, dest_base, dry_run)?;
            stats.fallback_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    Ok(())
}
