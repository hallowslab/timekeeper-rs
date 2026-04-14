# Building Timekeeper

`timekeeper` is a media file organizer that sorts files using EXIF metadata.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (Edition 2024 supported)
- [ExifTool](https://sourceforge.net/projects/exiftool/) (Required at runtime if not bundled)

## Build Options

### Standard Build (External ExifTool)

By default, `timekeeper` expects `exiftool` to be available in the system PATH or at a specified location at runtime, the built releases will not have exiftool bundled.

```bash
cargo build --release
```

### Build with Bundled ExifTool

If you want to bundle ExifTool binaries directly into the `timekeeper` executable (ideal for portable distributions), you will have to build the project using the `bundled` feature.

```bash
cargo build --release --features bundled
```

**Note:** This requires the ExifTool binaries to be present in the expected internal directory (`bin/windows/exiftool/(exiftool.exe or exiftool(-k).exe) and exiftool_files/`) during the build process. The build script only verifies the names it does not check contents, make sure the files are valid

## Usage after Build

The compiled binary will be located at `target/release/timekeeper`.
