# Portable image pipeline

Status: native PNG inspection, orientation, color normalization and verified PNG
and PNG/JPEG export are implemented in the native CLI/worker and Electron Images
workspace, with native-generated previews, crop and resize controls. TIFF output
and target-size fitting are pending.

Build with `cargo build --release --workspace`, then run:

```sh
target/release/fileform-native inspect-image photo.png
target/release/fileform-native convert-image photo.png normalized.png
```

Windows uses `target/release/fileform-native.exe`. The internal worker accepts
`{"operation":"inspect_image","input":"/path/to/photo.png"}` and returns the
same inspection in its versioned response envelope.

The receipt distinguishes raw width/height and decoded RGBA checksum from
orientation-adjusted display dimensions and oriented RGBA checksum. Inspection does not save a file. Conversion creates a new PNG and never replaces
the original. Raw/oriented hashes precede color normalization; the sRGB hash
identifies normalized pixels.

PNG decoding checks the complete IEND record, frame completion, an 80-million-pixel
limit and a 512 MiB source limit. Animation and 16-bit input are rejected. Metadata
flags indicate presence, not profile validity or proof that an image is SDR.

EXIF orientation supports values 1–8 in little- or big-endian TIFF IFD0 metadata.
Absent orientation in a valid directory means 1. Bad offsets, truncation, invalid
field types/counts, duplicate orientation fields and invalid values are rejected.
Metadata is bounded at 1 MiB and the root directory at 4096 entries. This is an
orientation reader, not a general EXIF validator. The mapping matches the
[image crate's EXIF orientation API](https://docs.rs/image/0.25.10/image/metadata/enum.Orientation.html).

Orientation preserves all RGBA bytes, including alpha, uses checked/fallible
output allocation, and checks cancellation while traversing pixels. Rotated images
can temporarily require both input and output pixel buffers; this is not a claim
of a hard process-memory limit.

Run `python3 Tools/smoke-images.py` for independent generated PNG fixtures and
real CLI/worker checks. Eight asymmetric pixel-layout fixtures verify every
orientation and dimension swap. Unit tests also cover malformed metadata and
cancellation. Both desktop CI platforms run the process checks.

Next required stages remain those in PORTABLE_IMAGE_RESEARCH.md: ICC-to-sRGB
normalization, gamma/chromaticity/HDR metadata handling, JPEG/TIFF and other
reference input formats, verified output encoding, resize/crop/quality/fit,
transparent JPEG backgrounds, and the Electron workspaces.

## ICC transform stage

The native pipeline now parses embedded ICC profiles (up to 1 MiB), validates
that RGB/gray profile space matches PNG samples, and transforms color values to
sRGB with moxcms 0.8.1. It processes at most 4096 pixels per block with fallible
scratch allocations and cancellation checks. Alpha is kept outside the CMS and
copied unchanged. The inspection's optional `icc_srgb_rgba_sha256` hashes these
orientation-adjusted, ICC-transformed pixels; absence means no embedded ICC
transform was performed. It is not a claim of HDR-safe rendering or export.

Tests cover sRGB identity, a P3-to-sRGB sample checked against an independent
D65 matrix/transfer calculation, gray gamma conversion, profile/sample mismatch,
malformed profiles and cancellation. Real CLI tests embed generated standard ICC
profiles in PNG files and check their receipts and unchanged source bytes.
Gamma/chromaticity-only PNG color, metadata precedence/conflicts, HDR/gain maps,
comparison against the native Apple renderer and output profile embedding remain
release requirements. `conversion_available` is true for the implemented PNG rendering path and false
for extended-color inputs still awaiting preservation support.

## PNG gamma, chromaticity and metadata validation

PNG inspection now builds native matrix/TRC profiles for gAMA/cHRM metadata and
returns `srgb_rgba_sha256` with a `color_interpretation` label. ICC takes priority
over sRGB, then gamma/chromaticities. A cICP or HDR-metadata image is marked
`extended_color_pending` and receives no normalized hash until that path is
implemented. This follows the [PNG color priority rules](https://www.w3.org/TR/png-3/#colorSpace).
Untagged inputs are explicitly `assumed_srgb`; missing primaries use sRGB primaries,
and chromaticities without gamma currently use the sRGB transfer curve. These
assumptions require native-reference comparison before export is enabled.

A bounded preflight verifies recognized color/EXIF chunk lengths, duplicates,
CRC checksums and basic value constraints, and rejects metadata the decoder
silently ignored. It bounds metadata chunks at 1 MiB and the container at 100,000
chunks. Raw gama_chunk/chrm_chunk fields are used rather than relying on derived
fields that were not populated in the tested png 0.18.1 decode path.

Independent process fixtures verify linear-gamma normalization, explicit-sRGB
precedence, extended-color deferral, zero-gamma rejection, metadata CRC rejection
and duplicate rejection. Unit tests check Display P3 chromaticity conversion,
invalid gamma and degenerate primaries. These extend the shared Windows/macOS
worker checks; they do not complete image export or all color-space parity.

## Verified PNG export and native comparison

`convert-image INPUT.png OUTPUT.png` uses the same render stages as inspection,
writes an eight-bit RGBA PNG marked sRGB with stale EXIF removed, then reopens it
and compares every pixel, dimensions and normalized orientation before saving.
Output is capped at 512 MiB. Source rechecks, cancellation, directory-identity
checks and no-clobber publication apply. The worker also accepts convert_image
with optional expected_source_sha256 to reject changed inspected inputs.

Tools/smoke-images.py verifies actual CLI/worker exports, all eight orientations,
exact alpha/pixels, stale-source rejection, collisions and invalid-input denial.
Tools/compare-image-colors.py additionally compares exported PNGs with the Swift
reference on macOS, using Pillow for independent output decoding. Four opaque
fixtures (Apple sRGB, Apple Display P3, linear gamma and orientation 6) matched
exactly in this run; the declared comparison tolerance is two code values.
[Evidence](Benchmarks/native-color-macos-2026-09-14.json) records binary/profile
hashes. Apple profile files are read from the OS and are not copied into Git.

An initial synthetic moxcms-generated P3 profile was ignored by the Apple path,
while the Rust transform matched the independent P3 matrix calculation. That
fixture compatibility discrepancy is retained as a limitation; standard Apple
profiles establish the native comparison above, not universal ICC equivalence.
Additional real profiles, transparency rounding, gamut edges, CICP/HDR and other
input/output formats remain parity work.

## Electron PNG workspace

The shared Electron app now has Tables and Images workspaces. Images uses native
open/save dialogs, opaque source/result IDs, validated image receipts and the
same Rust normalization/export code as the CLI. Unavailable extended-color images
cannot be exported. Workspace switching retains the selected image and saved
result during the session; it does not yet persist workspace state across launch.

Packaged macOS acceptance selected an EXIF orientation-6 RGBA PNG, displayed its
12 × 16 oriented dimensions, saved a new PNG, and retained state when switching
to Tables and back. Pillow independently confirmed every pixel/alpha value,
orientation normalization and sRGB tagging; the source checksum was unchanged.
Light and dark appearance were inspected; the previous Dark setting was restored.
Evidence is in Artifacts/Verification/electron-image/verified.json.
The first workspace provides metadata and saving; image previews and the remaining
editing controls still need to be ported. Windows GUI acceptance remains pending.

## Bounded desktop previews

Electron requests an optional preview during native inspection. Rust generates
an orientation/color-adjusted thumbnail no larger than 128 × 128 pixels, using
alpha-weighted area averaging so hidden transparent colors do not contaminate
the result. The pixel payload is at most 64 KiB before JSON encoding. The main
process validates dimensions, length and byte ranges before exposing it through
the bridge; the renderer only draws the buffer to a canvas. Preview pixels never
become export input, and unhandled extended-color images receive no preview.

Unit tests cover payload bounds, transparent-color averaging and cancellation.
The real worker smoke checks preview values and response size. The packaged Mac
app displayed the correctly oriented transparent fixture over a checkerboard.
The preview uses area averages in the encoded display space; higher-quality
linear-light resampling and larger zoom previews remain refinement work.

## JPEG output

PNG input can now be saved as .jpg/.jpeg through the same native render and
publication path. Transparent input requires an explicit white or black background
in Electron, the worker request, or the CLI:

```sh
target/release/fileform-native convert-image input.png output.jpg --background white
```

JPEG defaults to quality 85 and supports an explicit 1–100 setting. Target-size
fitting is still required; JPEG input decoding is also a separate pending port. The encoder embeds
sRGB ICC data, then the verifier checks dimensions, RGB decode, ICC bytes, normal
orientation and complete ending before publication. JPEG verification is lossy
and does not promise pixel equality. PNG output retains its exact pixel check.

Actual Mac app acceptance verified disabled saving before a background choice,
white-background selection and native-dialog JPEG export. Pillow independently
confirmed dimensions/profile and solid-color output within one code value; the
black-background CLI fixture was exact. Source bytes were unchanged. Evidence:
Artifacts/Verification/electron-image/jpeg-verified.json. Process tests on both CI
platforms cover background enforcement, container/profile checks and collisions.

## JPEG quality controls

Electron exposes a keyboard-operable 1–100 quality slider; the default is 85.
The CLI accepts `--quality 40` alongside `--background white`, in either order.
The worker and main process validate the value independently. Missing, repeated,
unknown and out-of-range options are rejected, and PNG does not silently ignore
JPEG-only quality/background options.

A detailed native process fixture produces a smaller JPEG at quality 20 than 95.
Packaged Mac acceptance set the slider to 40 using the keyboard, saved via the
native dialog, and compared against a CLI quality-40 conversion. Quantization,
compressed image data and decoded pixels matched; the embedded ICC profile's
creation timestamp differed, so full-file byte equality is not claimed. The
quality-85 fixture has different quantization. Evidence: electron-image/
quality-verified.json under Artifacts/Verification. The preview shows normalized
source pixels and the chosen matte, not a recompressed JPEG preview.

## Exact pixel cropping

The native worker and CLI now accept an optional crop after orientation/color
normalization, before PNG/JPEG encoding:

```sh
target/release/fileform-native convert-image input.png cropped.png --crop 10,20,300,200
```

Coordinates are x,y,width,height in oriented-image pixels, with a top-left origin.
The worker uses a crop object with those four integer fields. Empty, overflowing,
out-of-bounds and malformed rectangles fail without publication. Row copies use
fallible allocation and cancellation checks; selected RGBA pixels are retained
exactly before any lossy JPEG encoding. Electron crop controls are now connected to this native operation.

Thirty-six Rust tests and the real image process checks pass. An independent
Pillow comparison applied EXIF orientation, cropped the selected region and
matched every output pixel/alpha value; the original bytes stayed unchanged.
Evidence: Artifacts/Verification/electron-image/crop-verified.json.

## Electron crop controls

Images now provides a visible crop selection, four draggable corner handles,
keyboard corner adjustment (Shift for larger steps), a centered 1:1 preset,
exact pixel fields, remove-crop, and bounded undo/redo history. Main-process and
native validation both enforce a non-empty rectangle within oriented dimensions.
Export receipts are checked against the selected crop dimensions.

Packaged Mac acceptance applied the square preset, dragged width from 12 to 10,
undid to 12, redid to 10, adjusted a corner with the keyboard, and saved through a
native dialog. Independent Pillow decoding matched every selected pixel and alpha
value; the original checksum was unchanged. An observed drag-history grouping
failure was fixed before this successful retest. Geometry boundary tests now run
in both desktop CI jobs. Evidence: electron-image/crop-ui-verified.json under
Artifacts/Verification. Windows interactive crop acceptance remains pending.

## Native downscaling

The worker and CLI accept `max_dimension` / `--max-dimension` after cropping:

```sh
target/release/fileform-native convert-image input.png small.png --max-dimension 1200
```

The longest edge is capped without enlargement. The other dimension is rounded
to the nearest integer, with a minimum of one pixel. Resampling uses exact pixel
area overlap in linear sRGB with alpha weighting; transparent hidden colors do
not tint visible edges. It allocates only the byte output buffer plus small
working data, not a full-size floating-point image, and checks cancellation.
This is an area filter, not a claim of pixel-identical ImageIO resampling.

Thirty-nine Rust tests and real process checks pass, including fractional areas,
no enlargement, alpha, crop-then-resize and invalid limits. Independent Pillow
decoding matched an analytic linear-light black/white average. Evidence:
Artifacts/Verification/resize-verified.json. Electron size controls, broader
resampling/performance comparisons and target-byte fitting are still pending.

## Electron resize controls and export options

The Images workspace now has an optional longest-edge limit and predicted output
dimensions. Prediction uses the same integer rounding policy as the native engine,
after cropping, and never enlarges smaller images. Invalid/empty enabled limits
show an error and disable saving. The image bridge now takes named export options;
main validates allowed fields, formats, alpha background, quality, crop and size,
and checks the receipt against predicted dimensions.

Tests cover invalid export objects, crop copying, large integer rounding and
no-enlargement behavior; they run on both desktop CI platforms. Packaged Mac
acceptance combined a square crop and eight-pixel limit, rejected zero, displayed
8 × 8, and saved via a native dialog. The resulting PNG matched the CLI bytes
exactly and retained alpha/sRGB metadata, with the source unchanged. Evidence:
Artifacts/Verification/electron-image/resize-ui-verified.json. The preview remains
a selection preview; output dimensions describe the saved resampling result.
