# Portable offline OCR research

Research date: 2026-09-22. Primary-source research plus the limited local build
evaluation recorded below; full OCR parity is not established.

## Recommended integration

Use native Tesseract + Leptonica in the existing bounded child-process tool-pack
architecture. Rust normalizes supported images and PDFium renders scanned pages;
pass only an owned PGM/PPM file to OCR, never arbitrary user paths or configuration.
Keep heavy OCR outside Electron and unsafe C/C++ outside the Rust process. This is
an integration recommendation, not a claim that native libraries are memory-safe.

## Exact candidate versions and provenance

| Component | Candidate pin | Source evidence |
| --- | --- | --- |
| Tesseract | 5.5.3, commit `db0ec62f81b0737fbbe184d8fea40af5738f8eef` | [Release](https://github.com/tesseract-ocr/tesseract/releases/tag/5.5.3), [release API](https://api.github.com/repos/tesseract-ocr/tesseract/releases/latest), [tag commit](https://api.github.com/repos/tesseract-ocr/tesseract/commits/5.5.3) |
| Leptonica | 1.87.0, commit `13275a278eb55b5746e33f95fbf5a2c8f604b3ab` | [Release](https://github.com/DanBloomberg/leptonica/releases/tag/1.87.0), [tag commit](https://api.github.com/repos/DanBloomberg/leptonica/commits/1.87.0) |
| tessdata_fast | `87416418657359cb625c412a48b6e1d6d41c29bd` | [Pinned tree](https://github.com/tesseract-ocr/tessdata_fast/tree/87416418657359cb625c412a48b6e1d6d41c29bd) |
| tessdata_best | `e12c65a915945e4c28e237a9b52bc4a8f39a0cec` | [Pinned tree](https://github.com/tesseract-ocr/tessdata_best/tree/e12c65a915945e4c28e237a9b52bc4a8f39a0cec) |

Tesseract's latest release was published July 24, 2026. Its API lists a Windows
installer asset, not a separately uploaded source tarball with a digest. Use the
exact commit archive and independently compute/pin its SHA-256 before building;
do not use the installer digest as a source digest. During independent local
acquisition, the implementation agent measured the exact Tesseract commit archive
SHA-256 as `21dbaff9867d937c156d0b898e90b726aa339129cfebd7e1365cd5fee5046d67`
(not an upstream signed checksum). Leptonica's uploaded
[leptonica-1.87.0.tar.gz](https://github.com/DanBloomberg/leptonica/releases/download/1.87.0/leptonica-1.87.0.tar.gz)
has release-API SHA-256
`c73363397f96eb1295602bf44d708a994ad42046c791bf03ea0505d829bdb6a7`.
These are HTTPS/API observations; detached signature verification has not been
established. Retain source archives, exact commits, binary/model hashes and notices.
[Release metadata](https://api.github.com/repos/DanBloomberg/leptonica/releases/latest).

## Build configuration confirmed in tagged source

For **Leptonica 1.87.0**, use a fresh CMake build directory:

```text
-DBUILD_SHARED_LIBS=OFF -DBUILD_PROG=OFF -DSW_BUILD=OFF
-DENABLE_ZLIB=OFF -DENABLE_PNG=OFF -DENABLE_GIF=OFF
-DENABLE_JPEG=OFF -DENABLE_TIFF=OFF -DENABLE_WEBP=OFF
-DENABLE_OPENJPEG=OFF
```

These options exist in the [tagged CMake file](https://github.com/DanBloomberg/leptonica/blob/1.87.0/CMakeLists.txt).
`SW_BUILD` otherwise defaults ON on Windows and selects externally managed
dependencies. PNM reading is compiled by `USE_PNMIO=1`; it needs no optional image
codec. **This is not literally a PNM-only binary**: BMP and other built-in paths
remain enabled in [environ.h](https://github.com/DanBloomberg/leptonica/blob/1.87.0/src/environ.h).
Enforce PNM-only *input* in Fileform's adapter, or review an explicit source patch
if a smaller compiled format surface is required. Confirm a real OCR invocation
works without zlib; unsupported output formats must never be requested.

For **Tesseract 5.5.3**:

```text
-DBUILD_SHARED_LIBS=OFF -DBUILD_TRAINING_TOOLS=OFF -DBUILD_TESTS=OFF
-DGRAPHICS_DISABLED=ON -DDISABLE_CURL=ON -DDISABLE_ARCHIVE=ON
-DDISABLE_TIFF=ON -DOPENMP_BUILD=OFF -DENABLE_NATIVE=OFF
-DLeptonica_DIR=<isolated installation's CMake package directory>
```

These flags are in [5.5.3 CMakeLists.txt](https://github.com/tesseract-ocr/tesseract/blob/5.5.3/CMakeLists.txt).
Unlike older releases, 5.5.3 has removed `SW_BUILD`; do not mistake an unused
variable for an effective setting. The CLI target `tesseract` is unconditional;
training tools can be disabled. Disable ScrollView graphics and CURL, audit
actual linked dependencies, and retain process/network containment as a separate
gate. Windows still links system `Ws2_32`; its presence alone does not prove CURL
was enabled. Avoid reusing caches containing previously discovered libraries.

Build macOS arm64 and x86_64 separately with `CMAKE_OSX_ARCHITECTURES` and a deliberate
deployment target. Tagged source explicitly rejects a multi-architecture fallback
when deriving `CMAKE_SYSTEM_PROCESSOR`. Windows x64 supports MSVC; the source uses
`WIN32_MT_BUILD` for a static CRT. Align Leptonica's runtime with Tesseract's and
audit DLLs. Keep host-specific optimization disabled and test CPU dispatch on
representative hardware. These are source-supported build routes, **not yet
verified Fileform builds**. [Platform/compiler configuration](https://github.com/tesseract-ocr/tesseract/blob/5.5.3/CMakeLists.txt).

## Language and orientation parity

The original Vision behavior cannot be replaced honestly by English-only OCR.
Tesseract accepts explicit `-l eng+deu` and script models, with recognition/time
sensitive to language order; its default is English. Automatic page segmentation
is not automatic language identification. [Official command usage](https://tesseract-ocr.github.io/tessdoc/Command-Line-Usage.html).

Tesseract's `DetectOrientationScript` returns orientation and an alphabet/script,
not a language. Its implementation is under `#ifndef DISABLED_LEGACY_ENGINE`.
Therefore **leave `DISABLED_LEGACY_ENGINE=OFF` if using OSD** and bundle pinned
`osd.traineddata`; recognition can still explicitly use LSTM `--oem 1`.
[API implementation](https://github.com/tesseract-ocr/tesseract/blob/5.5.3/src/api/baseapi.cpp).
An OSD result cannot distinguish all languages sharing Latin, Cyrillic, etc.
An automatic-language policy needs additional design and measured multilingual
coverage; a script-first model selection is a candidate, not proven Vision parity.
Low-confidence or short text must allow language selection and correction.

`tessdata_fast` offers smaller, integer models optimized for speed; `best` offers
higher-accuracy floating models at greater cost. Both support LSTM; benchmark
representative documents before choosing defaults. Script models cover multiple
languages (Latin excludes Vietnamese; many scripts include English, Cyrillic is an
exception). [Model comparison](https://tesseract-ocr.github.io/tessdoc/Data-Files.html),
[pinned fast README](https://github.com/tesseract-ocr/tessdata_fast/blob/87416418657359cb625c412a48b6e1d6d41c29bd/README.md).

API tree metadata at these pins gives English fast 4,113,088 bytes (Git blob
`bbef4675053b5b468cdb477053e28b1c698ba08e`), English best 15,400,601 bytes
(`176dc3220de7db34d3b3aecbfa42043a6038348b`), and OSD 10,562,727 bytes
(`527457ca8f8fe1fda7c2f88bce3c0e4be12be9d0`) in both trees. Git blob IDs are **not
file SHA-256 hashes**. Compute binary model SHA-256 during independent acquisition.
[Fast tree API](https://api.github.com/repos/tesseract-ocr/tessdata_fast/git/trees/87416418657359cb625c412a48b6e1d6d41c29bd),
[best tree API](https://api.github.com/repos/tesseract-ocr/tessdata_best/git/trees/e12c65a915945e4c28e237a9b52bc4a8f39a0cec).

Tesseract and both model repositories use Apache-2.0; Leptonica uses its BSD-style
two-condition license. Ship exact license/copyright notices with binaries and
models. [Tesseract license](https://github.com/tesseract-ocr/tesseract/blob/5.5.3/LICENSE),
[fast license](https://github.com/tesseract-ocr/tessdata_fast/blob/87416418657359cb625c412a48b6e1d6d41c29bd/LICENSE),
[best README/license declaration](https://github.com/tesseract-ocr/tessdata_best/blob/e12c65a915945e4c28e237a9b52bc4a8f39a0cec/README.md),
[Leptonica license](https://github.com/DanBloomberg/leptonica/blob/1.87.0/leptonica-license.txt).

## Remaining release gates

- Build and inspect each target's real dependency graph; source/model hash checks
  and notices must fail closed. No runtime model retrieval by the OCR process.
- Bound page pixels, input/output bytes, model count, total time and memory; test
  cancellation and cleanup. Single-page subprocesses bound lifetime but do not
  themselves impose an OS memory limit.
- Test image normalization and PDF scan rendering at useful OCR resolution,
  including orientation, small print, columns, blank pages and mixed embedded/
  scanned text. OCR output is fallible; never label it lossless extraction.
- Test multilingual language/script selection against original behavior. Bundle
  the intended offline coverage or clearly record missing model availability;
  an English-only prototype must not close this parity requirement.
- Verify complete no-clobber document output, per-page provenance, macOS runtime,
  Windows CI runtime, and signed tool/model delivery. Research closes none of
  these implementation or release gates by itself.

## Local evaluation and reproducible recipe

`Tools/build-ocr-evaluation.py --work NEW_DIRECTORY` builds the exact sources
above with their recorded archive hashes, disables optional Leptonica codecs and
Tesseract network/archive/graphics/training features, and stages an explicitly
`evaluationOnly` pack. It retains source archives, all three license notices, model
hashes and local build logs/commands. No OCR execution adapter consumes this pack yet.
The recipe supports a Windows x64 build route, whose new CI job must still prove it.

A fresh Mac arm64 source build completed and the binary linked only system
libSystem/libc++. A generated raster of “Fileform page 1” was recognized exactly;
blank input and malformed input checks also passed. This is a tiny English baseline,
not a scan-quality or multilingual benchmark. The source-owned compressed raster
and provenance are retained under the engine's test fixtures. No PDF text layer
is passed to Tesseract.

Disabling TIFF initially produced bitmap-font diagnostics. The exact upstream
`src/ccstruct/debugpixa.h` supports `TESSERACT_DISABLE_DEBUG_FONTS`; defining it
removed that diagnostic path without patching source or adding TIFF. The final
Mac recipe was rebuilt and retested with that define. Legacy recognition remains
enabled for the separate OSD evaluation.

Downloaded model SHA-256 values at the pinned fast-model commit:

- English: `7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2`.
- OSD: `9cf5d576fcc47564f11265841e5ca839001e7e6f38ff7f7aacf46d15a96b00ff`.

English is the only recognition language currently in this evaluation pack. OSD
does not close automatic-language parity. OCR execution in Rust,
image/PDF normalization, bounded OCR supervision, cancellation, accuracy coverage,
multilingual selection, Windows runtime and signing remain implementation gates.

## Native pack verification and Windows build correction

`verify-ocr-pack PACK` and worker `verify_ocr_pack` now verify the executable plus
English/OSD model hashes, required offline declaration, architecture and pack
identity. Models are bounded to 64 MiB each and confined to the pack root. The
receipt explicitly reports English recognition only and no automatic language
detection. This is manifest integrity, not publisher authentication or OCR
execution. Unit and real CLI/worker checks reject missing/modified models, outside-
pack symlinks, cancellation and an enabled-network declaration.

Windows run 35823036206 built Leptonica but failed linking Tesseract because the
C runtimes differed. Leptonica's CMake 3.10 policy baseline left CMP0091 unset,
so the modern static-runtime property was ignored. The Windows recipe now supplies
`CMAKE_POLICY_DEFAULT_CMP0091=NEW` along with the static runtime setting. A fresh
Windows build is required to verify that correction; no Windows OCR success is
claimed from the failed run. The workflow also exercises native CLI/worker model
verification after its controlled recognition baseline.
