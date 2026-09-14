# Portable image backend research

Reviewed September 14, 2026. This is an implementation route, not a parity claim.

## Decision

Use `image = "=0.25.10"` with `default-features = false` and explicit `png`,
`jpeg`, `tiff` features for the first portable image pipeline. Keep decoding,
color conversion, resizing and encoding in the Rust worker. These codecs support
both reading and writing; the package already depends on `moxcms 0.8`, `png 0.18`
and `tiff 0.11.2`. Add direct dependencies on those compatible versions only where
Fileform uses their metadata or color APIs; pin the reviewed resolution in
Cargo.lock. This avoids enabling unrelated codecs or Rayon by default.
[Manifest](https://github.com/image-rs/image/blob/v0.25.10/Cargo.toml),
[codec overview](https://docs.rs/image/0.25.10/image/codecs/index.html).

The existing contract in [ImageBackend.swift](../Sources/FileformCore/ImageBackend.swift)
is **512 MiB input, 80 million pixels, single-frame, at most 8-bit SDR**. It rejects
HDR gain maps, applies orientation, draws into sRGB, and requires an explicit
white/black JPEG background for alpha input. PNG/JPEG/TIFF are its output formats;
ImageIO accepts additional input types. Three portable codecs do not establish
input-format parity with ImageIO.

## Required handling

| Concern | Evidence and implementation consequence |
| --- | --- |
| ICC color | `icc_profile()` returns bytes; encoder `set_icc_profile()` stores bytes. Neither is a pixel transform. Decode the source profile with `moxcms::ColorProfile::new_from_slice`, construct `new_srgb`, and apply an appropriate `create_transform_8bit` executor before writing an sRGB profile. Reject malformed/unsupported profiles instead of silently dropping them. Preserve alpha separately/with a compatible layout. [Decoder trait](https://github.com/image-rs/image/blob/v0.25.10/src/io/decoder.rs), [PNG writer](https://github.com/image-rs/image/blob/v0.25.10/src/codecs/png.rs), [moxcms APIs](https://docs.rs/moxcms/0.8.0/moxcms/struct.ColorProfile.html). |
| Color labels | `DynamicImage` has CICP color-space operations, but `set_color_space` changes interpretation, not values. The decoder trait provides no automatic ICC-to-sRGB pipeline. Do not treat `into_rgba8` or an sRGB label as normalization. [DynamicImage source](https://github.com/image-rs/image/blob/v0.25.10/src/images/dynimage.rs). |
| PNG metadata | Use direct `png::Info` for `bit_depth`, ICC, sRGB, gamma/chromaticities, CICP, mastering-display and content-light metadata. Resolve color metadata deliberately; reject unsupported HDR transfer characteristics. The `image` wrapper does not expose all these fields. Low sample depth does not prove SDR. [Info](https://docs.rs/png/0.18.0/png/struct.Info.html), [wrapper](https://github.com/image-rs/image/blob/v0.25.10/src/codecs/png.rs). |
| Orientation | Read orientation before consuming the decoder, apply it once with `DynamicImage::apply_orientation`, then omit stale EXIF or write orientation 1. Existing helpers default invalid/unrecognized orientation to no transform; strict parity needs raw-tag validation (1–8), distinguishing absent tags from malformed metadata. TIFF has a separate orientation tag. [Decoder trait](https://github.com/image-rs/image/blob/v0.25.10/src/io/decoder.rs), [TIFF source](https://github.com/image-rs/image/blob/v0.25.10/src/codecs/tiff.rs), [orientation application](https://docs.rs/image/0.25.10/image/enum.DynamicImage.html#method.apply_orientation). |
| Animation/pages | Reject `PngDecoder::is_apng()` input. For TIFF use the underlying `tiff::decoder::Decoder::more_images()` after the first IFD; the image wrapper does not expose a page count. Validate the IFD chain/container rather than decoding only its first page. [APNG check](https://github.com/image-rs/image/blob/v0.25.10/src/codecs/png.rs), [TIFF traversal](https://github.com/image-rs/image-tiff/blob/v0.11.2/src/decoder/mod.rs). |
| Bit depth | Inspect native container sample depth before converting to 8-bit. `color_type()` describes decoder output; `original_color_type()` defaults to that output unless overridden (TIFF overrides it). PNG palette expansion and JPEG colorspace conversion make output channel shape insufficient evidence of source properties. [Trait](https://github.com/image-rs/image/blob/v0.25.10/src/io/decoder.rs), [JPEG source](https://github.com/image-rs/image/blob/v0.25.10/src/codecs/jpeg/decoder.rs). |
| JPEG completeness/color | The image wrapper reads the entire compressed input in its constructor and sets Zune strict mode false. It converts non-RGB/Luma source colorspaces toward RGB. Add bounded container validation and truncated/corrupt fixtures; do not claim strict decoding from a successful image load. CMYK/YCCK plus ICC requires source-channel-aware CMS work or an explicit unsupported result until implemented. [JPEG source](https://github.com/image-rs/image/blob/v0.25.10/src/codecs/jpeg/decoder.rs). |
| HDR gain maps | The reviewed common decoder API has ICC/EXIF/XMP but no gain-map detection API. PNG CICP checks cover only part of this requirement. JPEG auxiliary-image/MPF, gain-map XMP and ISO metadata need a reviewed bounded parser and real fixtures. Merely checking 8-bit pixels or searching raw bytes for one keyword is not a parity solution. This remains a release gate. [Available decoder API](https://github.com/image-rs/image/blob/v0.25.10/src/io/decoder.rs). |

## Resource and file safety

`image::Limits` guarantees width/height limits, but **max_alloc is best effort**;
some decoders may ignore it. Its default 512 MiB concerns decoder allocations,
not the input-file limit or the entire process. Check `width * height` using
checked arithmetic against 80,000,000 independently. Set limits before header
allocations where possible: PNG has `with_limits`; changing its limits later
currently does not update the inner PNG reader. JPEG's constructor copies input
before `set_limits`. [Limits](https://docs.rs/image/0.25.10/image/struct.Limits.html),
[PNG implementation](https://github.com/image-rs/image/blob/v0.25.10/src/codecs/png.rs),
[JPEG implementation](https://github.com/image-rs/image/blob/v0.25.10/src/codecs/jpeg/decoder.rs).

Implementation recommendation: bound the source reader as well as checking file
metadata; independently cap ICC/EXIF/XMP and TIFF IFD values, decoder scratch,
output bytes, and dimensions. Budget simultaneous decode, orientation, CMS,
resize and verification buffers: one 80 MP RGBA8 buffer is 320,000,000 bytes.
Use checked sizes, fallible allocations and buffer reuse; retain worker timeout,
cancellation and process isolation. A strict process-memory ceiling needs a
separately verified OS mechanism; do not market best-effort crate limits as one.

Reuse Fileform's snapshot/hash recheck, sibling temporary output, no-clobber
publication and cancellation cleanup. Fully reopen output, check actual format,
dimensions, frame count, depth and alpha, and verify lossless pixel content where
applicable. JPEG needs dimension/completeness checks plus a documented lossy
comparison, not byte equality. No original may be replaced during failures.

## Acceptance before parity

- PNG/JPEG/TIFF cross-conversion, transparent input with both explicit JPEG
  backgrounds, grayscale and palette input, all eight EXIF orientations.
- Known ICC sRGB/P3/Adobe RGB images compared with the native reference using
  tolerances; invalid profiles, CMYK/YCCK and metadata conflicts handled explicitly.
- Rejection of APNG, multi-IFD TIFF, 16-bit/float input and supported HDR/gain-map
  detection fixtures. Record any still unsupported input codecs separately.
- Truncation, malformed chunks/IFDs, oversized dimensions/metadata, growing input,
  low-memory allocation failure, cancellation and destination collision tests.
- Resize/crop, quality/compression and target-size search must match the existing
  advertised workflows; basic format conversion alone does not complete the port.
- Native Mac GUI acceptance plus real Windows worker/CLI tests and packaging;
  Windows GUI remains unverified until exercised on Windows.
