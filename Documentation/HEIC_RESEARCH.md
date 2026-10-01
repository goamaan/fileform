# Portable HEIC input research

Checked 2026-10-01 against upstream source and release APIs. This note establishes
an implementation route; it does not claim successful builds or restored parity.

## Required scope from the original app

[ImageBackend.swift](../Sources/FileformCore/ImageBackend.swift) reads ImageIO
formats, including HEIC, rather than advertising an encoder for every input type.
Its output list is JPEG, PNG and TIFF. The original conversion contract is:

- Input at most 512 MiB and 80 million pixels, exactly one image.
- Eight-bit or lower decoded SDR; reject higher depth, Apple HDR gain maps and
  (macOS 15+) ISO gain maps. These are existing preservation limits, not a new
  justification to exclude ordinary HEIC images.
- Honor orientation, fully decode the input, render through an sRGB color space,
  preserve alpha for PNG/TIFF and require an explicit white/black matte for JPEG.
- Support conversion, compression and fit-size with the existing resize/crop flows;
  validate output completeness, dimensions, type and alpha.

The portable implementation must restore ordinary single-image HEIC including
color-managed wide-gamut SDR and alpha. Restricting it to untagged sRGB, ignoring
ICC, or rejecting all auxiliary images would leave parity incomplete. Depth maps
and thumbnails are not necessarily extra user-visible frames; compare actual
ImageIO behavior using fixtures instead of equating item count with frame count.

## Candidate dependencies and provenance

| Component | Release / exact commit | Uploaded source SHA-256 |
| --- | --- | --- |
| libheif | v1.23.5; `413e2a87e6a70b3eccc3a3adc5801179dd2d9e00` | `fd9036064c4432f0550d15072ddf34956a248279ee9aeaff0fba3fa0f77d8f1a` |
| libde265 | v1.1.3; `ba62bf4cfb3242f3bf0a45617ff09e35236e4d82` | `554228bd17788c99a7e63b37ab5634722190e6e2bf60c1dcb01cef328e133905` |

The releases were published September 21 and September 14, 2026 respectively.
Hashes above are GitHub release-asset metadata, not independently measured files
or verified detached signatures. Verify downloaded bytes before building.
[libheif release/API](https://api.github.com/repos/strukturag/libheif/releases/latest),
[libheif source archive](https://github.com/strukturag/libheif/releases/download/v1.23.5/libheif-1.23.5.tar.gz),
[libde265 release/API](https://api.github.com/repos/strukturag/libde265/releases/latest),
[libde265 source archive](https://github.com/strukturag/libde265/releases/download/v1.1.3/libde265-1.1.3.tar.gz).

Use these current security releases as the initial pins, not older commonly
packaged versions. Recheck upstream security status when shipping. The libheif
1.23.5 release specifically fixes container/bitstream dimension mismatch leading
to decoder allocation before validation. [Release notes](https://github.com/strukturag/libheif/releases/tag/v1.23.5).

## Minimal build route

Build an isolated native helper linked with libheif and libde265. Rust supervises
it and owns source snapshots/staging; no native FFI inside the Rust engine or
Electron renderer. Decode-only here means Fileform exposes only decoding: it does
not prove every unused encoding function was removed from library source.

Confirmed flags from [libde265 v1.1.3 CMake](https://github.com/strukturag/libde265/blob/v1.1.3/CMakeLists.txt):

```text
-DENABLE_DECODER=OFF -DENABLE_ENCODER=OFF -DENABLE_SDL=OFF
-DENABLE_SHERLOCK265=OFF -DENABLE_INTERNAL_DEVELOPMENT_TOOLS=OFF
-DWITH_FUZZERS=OFF
```

`ENABLE_DECODER=OFF` disables the `dec265` example executable, **not** the decoder
library. SIMD is ON by default and contains x86/ARM paths; keep portable CPU
settings and test dispatch. Set `BUILD_SHARED_LIBS` deliberately on both libraries.

Confirmed libheif settings, including case-sensitive option names:

```text
-DENABLE_PLUGIN_LOADING=OFF -DWITH_LIBDE265=ON -DWITH_LIBDE265_PLUGIN=OFF
-DWITH_X265=OFF -DWITH_KVAZAAR=OFF -DWITH_UVG266=OFF -DWITH_VVDEC=OFF
-DWITH_VVENC=OFF -DWITH_X264=OFF -DWITH_OpenH264_DECODER=OFF
-DWITH_DAV1D=OFF -DWITH_AOM_DECODER=OFF -DWITH_AOM_ENCODER=OFF
-DWITH_SvtEnc=OFF -DWITH_RAV1E=OFF -DWITH_JPEG_DECODER=OFF
-DWITH_JPEG_ENCODER=OFF -DWITH_OpenJPEG_DECODER=OFF
-DWITH_OpenJPEG_ENCODER=OFF -DWITH_FFMPEG_DECODER=OFF
-DWITH_OPENJPH_ENCODER=OFF -DWITH_UNCOMPRESSED_CODEC=OFF
-DWITH_WEBCODECS=OFF -DWITH_LIBSHARPYUV=OFF
-DWITH_EXAMPLES=OFF -DWITH_GDK_PIXBUF=OFF -DBUILD_TESTING=OFF
-DBUILD_DEVELOPMENT_TOOLS=OFF -DBUILD_DOCUMENTATION=OFF -DWITH_FUZZERS=OFF
-DWITH_HEADER_COMPRESSION=OFF -DENABLE_PARALLEL_TILE_DECODING=OFF
```

[Tagged libheif CMake](https://github.com/strukturag/libheif/blob/v1.23.5/CMakeLists.txt).
Fresh build directories and isolated dependency paths are required to confirm
that optional host packages were not linked. Disabling plugin loading removes
runtime codec discovery. Use separate macOS arm64/x86_64 builds and Windows x64
builds; inspect actual binary dependencies and preserve source/toolchain manifests.
No platform build was performed for this research note.

## Decode and inspection contract

1. Allocate context; set security limits **before reading**. Bound source bytes,
   image pixels, tiles, items, ICC size, allocation blocks, total memory and sequence
   counts. Never set a bound to zero unintentionally (zero disables it). Leave
   internal `parent` untouched. Library accounting is not proof of a whole-process
   limit; apply OS containment and a timeout/cancellation supervisor as well.
   [Security API](https://github.com/strukturag/libheif/blob/v1.23.5/libheif/api/libheif/heif_security.h).
2. Require one top-level image and no sequence. Top-level count excludes grid tiles
   and thumbnails; support grids within resource bounds. Inspect the primary image
   and enumerate auxiliary image types, preserving alpha and detecting gain maps.
   Do not reject ordinary thumbnails/depth merely because they are items.
   [Context](https://github.com/strukturag/libheif/blob/v1.23.5/libheif/api/libheif/heif_context.h),
   [sequences](https://github.com/strukturag/libheif/blob/v1.23.5/libheif/api/libheif/heif_sequences.h),
   [auxiliary API](https://github.com/strukturag/libheif/blob/v1.23.5/libheif/api/libheif/heif_aux_images.h).
3. Inspect luma/chroma depth, alpha/premultiplication, ICC **and** NCLX. The summary
   color-profile type hides NCLX when ICC exists. The handle NCLX API notes that
   bitstream-only profiles may not be returned: validate decoded metadata too.
   Reject existing unsupported HDR/high-depth preservation cases explicitly rather
   than silently converting them to eight bits. Exact Apple/ISO gain-map detection
   across auxiliary types/derived items remains a fixture-backed implementation gate.
   [Handle API](https://github.com/strukturag/libheif/blob/v1.23.5/libheif/api/libheif/heif_image_handle.h),
   [color API](https://github.com/strukturag/libheif/blob/v1.23.5/libheif/api/libheif/heif_color.h).
4. Allocate decoding options with the API; enable `strict_decoding`, leave
   `ignore_transformations=false`, avoid implicit high-depth truncation. libheif
   applies cropping/rotation/mirroring; do not also blindly apply EXIF orientation
   to already transformed pixels. Test irot/imir/clap order plus EXIF-only and
   conflicting cases against ImageIO before declaring orientation parity.
   [Decode API](https://github.com/strukturag/libheif/blob/v1.23.5/libheif/api/libheif/heif_decoding.h),
   [ordered transforms](https://github.com/strukturag/libheif/blob/v1.23.5/libheif/api/libheif/heif_properties.h).
5. Define color ownership explicitly. Version 1.23.5 documents default null output
   NCLX as conversion to sRGB; its new passthrough option preserves source NCLX.
   That does not establish correct ICC conversion. A robust candidate is native
   decode with source color information retained, then the existing bounded ICC/
   NCLX-to-sRGB path exactly once. Verify actual decoded profile/pixel semantics
   with ICC-only, NCLX-only and both-profile fixtures; avoid applying an ICC transform
   to pixels already converted by another path. Alpha must have an explicit
   straight/premultiplied convention, including zero-alpha colors.
   [Decode options](https://github.com/strukturag/libheif/blob/v1.23.5/libheif/api/libheif/heif_decoding.h).
6. Transfer bounded pixels plus a small validated metadata receipt to Rust through
   owned files; validate dimensions/stride/length before allocation. Reuse normal
   image export, no-clobber publication and source-unchanged checks. Decode warnings,
   unsupported color descriptions and inconsistent dimensions must not become
   falsely successful conversions.

## Licensing and delivery gates

Both libraries identify LGPL version 3 or later in source headers; example tools
are separately licensed. Retain LGPL/GPL texts, copyright notices, exact source,
patches and build instructions. For static linking, plan complete corresponding
source/application relinking materials; shared-library packaging also needs a
compliant replacement/relinking path. Signing and hash verification must not make
user-rebuilt LGPL components impossible to use in a rebuild. Apache-2.0 for original
Fileform code does not replace third-party licenses. This is a delivery checklist,
not a legal determination of a finished distribution.
[libheif license](https://github.com/strukturag/libheif/blob/v1.23.5/COPYING),
[libde265 license](https://github.com/strukturag/libde265/blob/v1.1.3/COPYING).
HEVC patent questions are separate from source copyright licensing; no patent
clearance is claimed by selecting these libraries.

Required acceptance remains: real HEIC conversions on both platforms; grids and
all orientation combinations; Display-P3/ICC and NCLX SDR; alpha/matte; original
HDR/gain-map/high-depth/multiple-image rejections; damaged/truncated inputs and
allocation limits; preview/crop/resize/compress/fit and image-to-PDF/OCR routes;
source preservation, cancellation, cleanup, and no-clobber output. Reuse original
ImageIO as a behavior/color oracle on Mac, with tolerances appropriate to decoder
rounding rather than unsupported byte-exact equality. Keep missing cases visible
until verified; a restricted prototype does not close the HEIC parity requirement.


## Dependency source build — October 1, 2026

`Tools/build-heic-evaluation.py --work NEW_DIRECTORY --jobs 4` now verifies the
actual downloaded archive bytes against both pins above, safely extracts bounded
regular files/directories, builds shared libraries with explicit codec paths and
optional codecs/examples/plugins disabled, and retains commands, caches, source
archives and licenses. A fresh Mac arm64 build passed. `otool` reports libheif and
libde265 plus system C++/System dependencies, with no host codec library linked.
The Windows workflow builds the same pinned source and retains its evidence;
Windows source-build acceptance is pending. This is dependency evaluation only,
not HEIC support in the app, engine adapter, helper acceptance or distribution.

Next implementation gates remain the isolated bounded decode helper and protocol,
original single-image SDR/ICC/NCLX/alpha/orientation parity fixtures against ImageIO,
HDR/gain-map rejection (including tone-map/compact-container representations),
Rust image/OCR/PDF integration, native runtime tests on both systems, library
replacement/rebuild packaging and Electron end-to-end integration. Do not replace
these gates with a basic dependency build or advertise restored HEIC input yet.
