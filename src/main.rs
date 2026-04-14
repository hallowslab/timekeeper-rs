use clap::Parser;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use timekeeper::Organizer;
use timekeeper::stats::Stats;

#[derive(Parser)]
#[command(
    version,
    about = "A media file organizer that sorts files by date using EXIF metadata",
    name = "timekeeper"
)]
struct Args {
    /// Source file or directory
    #[arg(short = 's', long = "source")]
    source: std::path::PathBuf,

    /// Destination directory
    #[arg(short = 'd', long = "destination")]
    destination: std::path::PathBuf,

    /// Show what would be done without actually moving files
    #[arg(long = "dry-run")]
    dry_run: bool,

    /// Path to ExifTool executable (optional, auto-detected if not specified)
    #[arg(long = "exiftool")]
    exiftool: Option<std::path::PathBuf>,
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
            terminate_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        })?;
    }

    let mut organizer = Organizer::new(args.source, args.destination, args.dry_run);
    if let Some(p) = args.exiftool {
        organizer = organizer.with_exiftool(p);
    }

    organizer.run(Arc::clone(&stats), Arc::clone(&terminate_flag))?;

    println!("\n[INFO] Finished processing or stopped by user.");
    stats.print();

    Ok(())
}
