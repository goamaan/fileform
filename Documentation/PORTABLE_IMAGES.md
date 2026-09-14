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
