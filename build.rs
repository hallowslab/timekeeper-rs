fn main() {
    // When the 'bundled' feature is enabled on Windows, verify that ExifTool files
    // are present at compile time.
    #[cfg(all(windows, feature = "bundled"))]
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let bin_dir = std::path::Path::new(&manifest_dir)
            .join("bin")
            .join("windows");

        if !bin_dir.exists() {
            panic!(
                "\n\nERROR: ExifTool bin directory not found at: {}\n",
                bin_dir.display()
            );
        }

        // Search recursively for the executable and the files directory
        let mut found_exe = false;
        let mut found_files = false;

        for entry in walkdir::WalkDir::new(&bin_dir) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let file_name = entry.file_name().to_string_lossy();

            // Look for any exe starting with 'exiftool' (handles both exiftool.exe and exiftool(-k).exe)
            if entry.path().extension().map_or(false, |ext| ext == "exe")
                && file_name.to_lowercase().starts_with("exiftool")
            {
                found_exe = true;
            }

            // Look for the exiftool_files directory
            if entry.file_type().is_dir() && file_name == "exiftool_files" {
                found_files = true;
            }
        }

        if !found_exe || !found_files {
            panic!(
                "\n\n\
                 ══════════════════════════════════════════════════════════════\n\
                 ERROR: Bundled ExifTool files not found in structure!\n\n\
                 Inside bin/windows/, we could not find both an 'exiftool*.exe'\n\
                 and the 'exiftool_files/' directory.\n\n\
                 Checked path: {}\n\n\
                 Please ensure you have extracted ExifTool into this folder.\n\
                 ══════════════════════════════════════════════════════════════\n",
                bin_dir.display()
            );
        }

        // Tell Cargo to re-run this script if the bin directory changes
        println!("cargo:rerun-if-changed=bin/windows/");
    }
}
