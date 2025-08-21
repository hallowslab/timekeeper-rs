use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Stats {
    pub total: AtomicUsize,
    pub processed: AtomicUsize,
    pub exif_count: AtomicUsize,
    pub fallback_count: AtomicUsize,
    pub skipped: AtomicUsize,
    pub errors: AtomicUsize,
}

impl Stats {
    pub fn new() -> Self {
        Stats {
            total: AtomicUsize::new(0),
            processed: AtomicUsize::new(0),
            exif_count: AtomicUsize::new(0),
            fallback_count: AtomicUsize::new(0),
            skipped: AtomicUsize::new(0),
            errors: AtomicUsize::new(0),
        }
    }

    pub fn print(&self) {
        let total = self.total.load(Ordering::SeqCst);
        let processed = self.processed.load(Ordering::SeqCst);
        let exif_count = self.exif_count.load(Ordering::SeqCst);
        let fallback_count = self.fallback_count.load(Ordering::SeqCst);
        let skipped = self.skipped.load(Ordering::SeqCst);
        let errors = self.errors.load(Ordering::SeqCst);

        println!("\n=== SUMMARY ===");
        println!("Total files: {}", total);
        println!("Successfully processed: {}", processed);
        println!("Skipped: {}", skipped);

        if processed > 0 {
            let exif_percentage = (exif_count as f64 / processed as f64) * 100.0;
            let fallback_percentage = (fallback_count as f64 / processed as f64) * 100.0;

            println!(
                "  - Using EXIF data: {} ({:.1}%)",
                exif_count, exif_percentage
            );
            println!(
                "  - Using fallback (ModTime): {} ({:.1}%)",
                fallback_count, fallback_percentage
            );
        }

        println!("Errors: {}", errors);
    }
}