pub mod exiftool;
pub mod metadata;
pub mod stats;

pub use exiftool::ExifToolError;

use crate::metadata::{is_media_file, process_file_with_fallback, process_with_exiftool};
use crate::stats::Stats;
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Organizer {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub dry_run: bool,
    pub use_copy: bool,
    pub exiftool_path: Option<PathBuf>,
}

impl Organizer {
    pub fn new(source: PathBuf, destination: PathBuf, dry_run: bool) -> Self {
        Self {
            source,
            destination,
            dry_run,
            use_copy: true, // Default to copy
            exiftool_path: None,
        }
    }

    pub fn with_copy(mut self, use_copy: bool) -> Self {
        self.use_copy = use_copy;
        self
    }

    pub fn with_exiftool(mut self, path: PathBuf) -> Self {
        self.exiftool_path = Some(path);
        self
    }

    pub fn run(
        &self,
        stats: Arc<Stats>,
        terminate_flag: Arc<AtomicBool>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let exiftool_path = exiftool::get_exiftool_path(self.exiftool_path.clone())?;

        if self.source.is_dir() {
            self.count_media_files(&stats)?;
            self.process_directory(&exiftool_path, &stats, &terminate_flag)
        } else {
            stats.total.store(1, Ordering::SeqCst);
            self.process_single_file(&exiftool_path, &self.source, &stats, &terminate_flag)
        }
    }

    fn count_media_files(&self, stats: &Arc<Stats>) -> Result<(), Box<dyn std::error::Error>> {
        for entry in walkdir::WalkDir::new(&self.source) {
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
        &self,
        exiftool_path: &PathBuf,
        stats: &Arc<Stats>,
        terminate_flag: &Arc<AtomicBool>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let entries: Vec<_> = walkdir::WalkDir::new(&self.source)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .collect();

        entries.par_iter().for_each(|entry| {
            if terminate_flag.load(Ordering::SeqCst) {
                return;
            }

            if let Some(path_str) = entry.path().to_str() {
                if is_media_file(path_str) {
                    if let Err(e) = self.process_single_file(
                        exiftool_path,
                        &entry.path().to_path_buf(),
                        stats,
                        terminate_flag,
                    ) {
                        eprintln!("Error processing {}: {}", entry.path().display(), e);
                        stats.errors.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        });

        Ok(())
    }

    fn process_single_file(
        &self,
        exiftool_path: &PathBuf,
        source_path: &PathBuf,
        stats: &Arc<Stats>,
        terminate_flag: &Arc<AtomicBool>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if terminate_flag.load(Ordering::SeqCst) {
            return Ok(());
        }

        stats.processed.fetch_add(1, Ordering::SeqCst);

        let filename = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        match process_with_exiftool(
            exiftool_path,
            source_path,
            &self.destination,
            self.dry_run,
            self.use_copy,
            stats,
        ) {
            Ok(()) => {
                stats.exif_count.fetch_add(1, Ordering::SeqCst);
            }
            Err(_) => {
                let _ = process_file_with_fallback(
                    source_path,
                    &self.destination,
                    self.dry_run,
                    self.use_copy,
                );
                stats.fallback_count.fetch_add(1, Ordering::SeqCst);
            }
        }

        Ok(())
    }
}
