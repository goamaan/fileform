# Portable image pipeline

Status: native PNG inspection and orientation are implemented. Portable image
conversion/export and Electron image controls are not yet available.

Build with `cargo build --release --workspace`, then run:

```sh
target/release/fileform-native inspect-image photo.png
```

Windows uses `target/release/fileform-native.exe`. The internal worker accepts
`{"operation":"inspect_image","input":"/path/to/photo.png"}` and returns the
same inspection in its versioned response envelope.

The receipt distinguishes raw width/height and decoded RGBA checksum from
orientation-adjusted display dimensions and oriented RGBA checksum. No file is
saved or original changed. Pixel hashes are before color normalization.

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
release requirements. `conversion_available` therefore remains false.

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
