# Building Timekeeper

`timekeeper` is a media file organizer that sorts files using EXIF metadata.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (Edition 2024 supported)
- [ExifTool](https://exiftool.org/) (Required at runtime if not bundled)

## Build Options

### Standard Build (External ExifTool)

By default, `timekeeper` expects `exiftool` to be available in the system PATH or at a specified location at runtime.

```bash
cargo build --release
```

### Build with Bundled ExifTool

If you want to bundle ExifTool binaries directly into the `timekeeper` executable (ideal for portable distributions), use the `bundled` feature.

```bash
cargo build --release --features bundled
```

**Note:** This requires the ExifTool binaries to be present in the expected internal directory (`bin/exiftool/`) during the build process. The build script handles the validation of these binaries.

## Usage after Build

The compiled binary will be located at `target/release/timekeeper`.
