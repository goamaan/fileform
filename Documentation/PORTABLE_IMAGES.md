# Portable image pipeline

The shared Rust worker/CLI and Electron Images workspace support PNG, JPEG and
TIFF input/output, with the limits below. Heavy decoding, color conversion,
orientation, crop, resize, encoding and verification remain outside Electron.
Full application/release readiness is tracked in ROADMAP.md and CROSS_PLATFORM.md.

## Current formats

| Format | Input | Output |
| --- | --- | --- |
| PNG | Still images up to 8-bit samples, including palette/grayscale expansion | 8-bit RGBA, sRGB, exact rendered-pixel verification |
| JPEG | 8-bit baseline/progressive RGB and grayscale, strict decoding | Quality 1–100 (default 85), sRGB ICC, explicit matte for alpha input |
| TIFF | Single-image classic TIFF/BigTIFF, 8-bit RGB/RGBA/gray/gray-alpha, interleaved or planar | LZW-compressed 8-bit RGBA, unassociated alpha, orientation 1, sRGB ICC |

Unmodeled preservation cases are not silently flattened. These include animated
PNG, multi-image/sub-IFD TIFF, high-depth/float images, unsupported TIFF palette or
bilevel layouts, CMYK/YCCK, special JPEG coding modes, and unresolved HDR/gain-map
or application metadata. TIFF color tags without an ICC profile may require
preservation handling. Expanding this coverage remains part of the full goal.

TIFF associated-alpha samples are unpremultiplied with integer rounding; original
stored premultiplied values are not the same representation as the resulting RGBA.
Directory entry counts, metadata values, full-plane buffer layouts and offsets
are checked before pixel processing. The decoder does not read just one plane.

## CLI

Build with `cargo build --release --workspace`. On Windows, use the .exe suffix.

```sh
target/release/fileform-native inspect-image photo.jpg
target/release/fileform-native convert-image photo.jpg normalized.png
target/release/fileform-native convert-image input.png output.jpg --background white --quality 85
target/release/fileform-native convert-image photo.jpg output.tiff --crop 10,20,300,200 --max-dimension 1200
```

Crop coordinates are x,y,width,height in oriented-image pixels, top-left origin.
Crop follows orientation/color normalization and precedes resize. Empty,
overflowing or out-of-bounds regions fail without saving. Maximum dimension never
enlarges an image; the other edge rounds to the nearest integer, minimum one pixel.
Resize uses pixel-area overlap in linear sRGB with alpha weighting, without a
full-size float intermediate. It is not pixel-identical ImageIO resampling.

JPEG transparency requires white/black background selection. Quality/background
options on PNG/TIFF and unknown/repeated/malformed options are rejected. Target
byte-size fitting remains unfinished.

## Color and metadata

- Embedded ICC profiles use moxcms for actual pixel conversion to sRGB. Copying a
  profile or changing a color label alone is not normalization. Alpha stays out of
  the CMS. Color conversion is processed in bounded cancellable blocks.
- PNG follows [color-tag precedence](https://www.w3.org/TR/png-3/#colorSpace): cICP,
  ICC, sRGB, then gAMA/cHRM. Unhandled cICP/HDR inputs are deferred. Lower-priority
  tags do not override them. Independent chunk checks reject corrupt or silently
  ignored ancillary color/EXIF metadata.
- JPEG EXIF sRGB/R98 and uncalibrated+R03 Adobe RGB are recognized without ICC.
  ColorSpace 2 is supported as a non-standard compatibility value. Conflicting,
  unknown or gamma declarations defer without ICC; ICC remains authoritative.
  See [ExifTool's tag documentation](https://exiftool.org/TagNames/EXIF.html).
- Untagged inputs use an explicit sRGB assumption. Missing PNG primaries use sRGB
  primaries; cHRM without gamma currently uses the sRGB transfer curve. Wider
  native-reference/color-profile comparisons remain required.
- EXIF/TIFF orientation values 1–8 are applied once. Invalid orientation metadata
  fails; exports omit stale orientation metadata or write orientation 1.
- JPEG unknown application metadata (including XMP/MPF-style segments) and TIFF
  unresolved metadata are conservatively deferred. Detailed preservation support
  remains required before claiming universal JPEG/TIFF coverage.

Inspection distinguishes raw, oriented and normalized pixel checksums, inferred
color interpretation, embedded ICC presence and conversion availability. A preview
is optional. Inferred Adobe RGB is not mislabeled as an embedded ICC profile.

## Resource and file safety

Image inputs/outputs are bounded at 512 MiB and decoding at 80 million pixels.
Metadata values/chunks have 1 MiB limits; aggregate JPEG/TIFF metadata is bounded
at 8 MiB, TIFF directories at 4096 entries and PNG/JPEG marker work at 100,000.
Decoder scratch limits are not a hard whole-process memory cap. Simultaneous
pixel buffers and forced-stop recovery still require release acceptance.

Sources are snapshotted, hashed and rechecked. Export accepts an optional inspected
source hash. Temporary results are verified, synced and published without replacing
an existing path; source/output collisions and invalid requests preserve originals.
PNG/TIFF verification compares every rendered pixel and required metadata. JPEG
verification fully decodes and checks dimensions/profile/orientation/completeness;
it does not promise lossless pixel equality. Cooperative cancellation cleans owned
staging/snapshots; hard-kill cleanup remains a separate release gate.

## Desktop controls

Electron provides native dialogs, bounded native previews, PNG/JPEG/TIFF selection,
JPEG quality/matte, draggable and keyboard crop corners, exact pixel fields, 1:1,
remove-crop, undo/redo, and longest-edge resizing with predicted output dimensions.
Named export options and receipts are validated in main and again in Rust.

Previews are at most 128 × 128 RGBA (64 KiB before JSON), alpha-weighted area
averages in display encoding. They never become export input. The canvas shows
normalized source/selection and matte, not a recompressed or full-resolution
output preview. Workspace state survives tab changes, not app restarts yet.
Only one development preview is kept running; repeated launch reuses its window.

## Verification

`cargo test --workspace`, Clippy, Tools/smoke-images.py and the desktop tests run
in macOS/Windows CI. Process fixtures cover orientation, color, crop/resize,
PNG/JPEG/TIFF round-trips, invalid metadata, collision and stale-source rejection.
Tools/compare-image-colors.py uses the Swift CLI and Pillow on macOS. Standard
Apple-profile fixtures matched the reference; a generated P3 profile was ignored
by the Apple path, so universal ICC equivalence is not claimed.

Local independent evidence is under Artifacts/Verification, including
portable-images.json, tiff-input-independent.json and electron-image/*-verified.json.
Packaged Mac acceptance covers import, previews, editing and native-dialog saves.
Latest TIFF acceptance reopened Fileform's own TIFF and exported PNG with exact
independently decoded RGBA. Interactive Windows acceptance remains pending.
