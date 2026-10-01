# Portable PDF migration

Status: the portable engine has bounded PDF inspection, graph/geometry evidence,
page rendering/text extraction, lossless compression, and selected-page composition
with PDF/PNG/JPEG/TIFF inputs and split-folder publication. These are native CLI/worker capabilities; the desktop
has not yet integrated them. Lossy optimization, embedded
image extraction, OCR fallback for text export and broader format/resource
acceptance remain parity work. Preserve the Swift implementations as references;
historical completion is not portable acceptance.

## Current entry points

- `fileform-native verify-pdf-pack PACK_DIRECTORY`
- `fileform-native inspect-pdf FILE PACK_DIRECTORY`
- Worker requests `verify_pdf_pack` and `inspect_pdf` with the corresponding paths.

Media and PDF now share bounded native-pack manifest/hash verification. PDF requires
`app.fileform.pdf` and `bin/qpdf` (or `qpdf.exe` on Windows), correct declared
architecture, a nonempty version and a matching executable hash. Media's required
no-network declaration remains enforced separately. A matching manifest is not
publisher authentication; the outer distribution must be trusted and signed.

Inspection snapshots a regular input up to 512 MiB, obtains a bounded page count
(currently 1–1000), runs qpdf's structural check and rechecks source identity/hash.
Each subprocess has a 60-second deadline and bounded output. Warnings/errors are
not silently accepted. Password-protected documents needing a password are rejected;
no password/unlocking product feature or private deferred branch was added.

`qpdf_check_passed` means that specific check succeeded. It is not a visual-render,
PDF conformance, active-content safety, accessibility or preservation proof. Future
transformations require the reference graph/content checks and independent rendering
where appropriate. Process-tree containment, hard-kill cleanup and filesystem-race
hardening remain shared release requirements.

## Verification and next work

The real Mac qpdf 12.4.1 pack passed verification. A generated two-page fixture
passed CLI and worker inspection with the same receipt; malformed/protected inputs
failed and source bytes stayed unchanged. Run
`python3 crates/fileform-engine/tests/smoke-pdf.py PACK_DIRECTORY` after a release
build. All 79 active Rust tests, Clippy, release build, Windows target checking and
existing media smoke pass. Windows PDF-tool execution is not established yet.

Next: build/review the Windows qpdf pack with retained sources/notices; port PDF
object-graph inspection and preservation checks, then existing transformations and
render/text/OCR support. Revisit limits against the complete reference contracts
before declaring parity or publishing capability claims.

## Windows source-build workflow

`crates/fileform-engine/tools/build-pdf-windows.sh` builds the same pinned qpdf
12.4.1 and libjpeg-turbo 3.2.0 sources in MSYS2 UCRT64, selects static zlib/JPEG/qpdf
linkage and audits imported DLLs against Windows system libraries. It retains
source archives, license notices, CMake caches and installed-toolchain metadata.
The recipe follows qpdf's documented MinGW/MSYS2 build route:
https://qpdf.readthedocs.io/en/latest/installation.html

`.github/workflows/pdf-windows.yml` builds or restores an exact recipe/toolchain
cache, builds the current Rust CLI/worker, and runs the real PDF smoke suite. A
cache hit never skips current pack or PDF inspection checks. Source build and
runtime verification are pending the first execution; local shell/YAML checks
alone do not prove Windows PDF support. Toolchain packages are recorded, not yet
frozen into a bit-for-bit reproducible snapshot. Retained CI artifacts are unsigned
and expire after 14 days; final public distribution/signing remain separate gates.

## Reachable-object graph evidence

`fileform-native inspect-pdf-graph FILE PACK_DIRECTORY` and worker
`inspect_pdf_graph` hash qpdf's generalized-decoded reachable document graph.
Indirect objects are assigned traversal identities, so object renumbering and
cycles are handled without relying on physical object numbers. Stream payloads
must be present. Only stream Length and scoped trailer/xref serialization fields
are ignored; ordinary content fields with those names remain significant.

JSON numbers retain arbitrary precision and are normalized as exact decimal
values, not floating-point approximations. Traversal is bounded to 100,000 objects,
1,000,000 nodes and depth 128; tool output is capped at 128 MiB with a 120-second
limit. Source identity/hash is rechecked. This graph evidence is not an independent
rendering proof or authorization to rewrite signed/encrypted/interactive documents;
those need the reference eligibility guards and additional preservation checks.

Tests verify renumbering/cycles, stream changes, missing references/data and decimal
values beyond floating-point precision. A real qpdf object-stream/compression rewrite
keeps the digest, while a changed drawing stream changes it. CLI/worker receipts
match; table/media regressions remain passing with exact-number JSON enabled.

## Windows static-zlib correction

The first Windows build (35815982405) compiled successfully but failed the DLL audit
because it imported zlib1.dll. The pinned qpdf CMake source uses ZLIB_LIB_PATH and
ZLIB_H_PATH, not the standard FindZLIB variable names used in the initial recipe.
The recipe now supplies qpdf's actual variables for libz.a. The DLL guard remains
unchanged; corrected Windows build/runtime acceptance is still pending.

## Preservation-sensitive feature detection

Graph inspection now reports `special_preservation_keys` from reachable
dictionaries, matching the reference optimizer's conservative guard set: forms,
annotations, signature byte ranges/permissions, actions/JavaScript, embedded or
associated files, tagged/layered structures, portfolios, XFA, encryption and
outlines. Unreachable objects do not trigger the reachable-graph report. Key
presence is a conservative signal, not complete PDF semantic classification.

A matching graph digest alone must not authorize an optimization when these
features require special handling. Independent rendering, page geometry/content
checks and transformation-specific eligibility remain required. Real annotated
PDF and unit reachability cases pass; all 83 active Rust tests, Clippy, release
build, Windows target check and PDF smoke pass locally. The corrected Windows
pack build is still active; no PDF output operation is declared complete.

## Page geometry and order

`fileform-native inspect-pdf-pages FILE PACK_DIRECTORY` and worker
`inspect_pdf_pages` report ordered media/crop/bleed/trim/art boxes, effective crop,
normalized right-angle rotation and page UserUnit. MediaBox/CropBox/Rotate follow
bounded parent inheritance; bleed/trim/art boxes and UserUnit remain page-local.
Null entries use PDF defaults/inheritance. Indirect geometry values are resolved
with cycle/depth checks. Boxes are normalized for inspection; exact preservation
comparisons still use the graph proof, not rounded display coordinates.

The reader checks page object types/order, at most 1000 pages/100000 objects,
finite positive geometry, the reference million-unit size bound, valid UserUnit,
64 MiB tool output and a bounded reply. It snapshots/rechecks the source. This is
geometry evidence, not a render or completed composition operation.

Real generated PDFs verify mixed page sizes, inherited media/crop/rotation, local
bleed overrides and null trim defaults. Unit tests reject cyclic ancestry. All 84
active Rust tests, Clippy, release build, Windows target check and PDF smoke pass
locally. Windows execution of this new reader is pending the next revision.

## Windows PDF baseline

Run [35816815487](https://github.com/goamaan/fileform/actions/runs/35816815487) at
`77f461c8bfb428b298971236d56ea59e3aa72e87` passed the corrected static qpdf/JPEG/zlib
build, system-DLL audit, native Rust build and real PDF inspection/graph suite.
The changed drawing stream was detected and the structural rewrite preserved its
graph. That revision predates special-feature and page-geometry additions; it does
not prove those newer paths, rendering, output operations or final app packaging.

## Renderer evaluation

See [PDFium research](PDFIUM_RESEARCH.md) for pinned candidate artifacts and
verified Mac arm64 archive/attestation evidence, and
[rendering acceptance](PDF_RENDER_ACCEPTANCE.md) for reference-derived checks.
The generated qpdf smoke fixture now also includes standard-font text alongside
vector shapes. This is preparation for rendering tests, not rendering parity.

### Initial native renderer helper

`native/pdf-render` now builds a separate C++ adapter against the evaluated
non-V8 PDFium archive. On Mac arm64, real fixtures pass bounded RGB raster output,
standard-font text/vector rendering, identical pixels after a qpdf object-stream
rewrite, changed pixels after content/text removal, inherited crop/rotation,
Unicode paths, invalid arguments, malformed/protected input and unchanged source
bytes. The original structural/graph smoke also still passes after sharing the
fixture generator.

This is an evaluation executable, not yet a Rust/worker API or app preview. Its
effective crop rendering differs from the Swift MediaBox fingerprint; UserUnit,
fonts/color/forms and full preservation coverage remain open. See its README for
limits and the required supervisor/containment integration. Windows CI now builds
and exercises the same helper against a hash-pinned, attestation-verified archive;
the result must be checked before claiming Windows acceptance.

### Engine rendering integration

`render-pdf-page INPUT OUTPUT.png RENDER_PACK PAGE_INDEX` and worker operation
`render_pdf_page` now render through a separate `app.fileform.pdf-render` pack.
The shared verifier checks the helper and required platform library SHA-256.
Rendering uses the source snapshot, a private working directory, cleared child
environment (Windows SystemRoot retained), a 60-second deadline, direct-child
cancellation/reaping and bounded raster output. Rust strictly validates dimensions
and payload length, encodes PNG, fully redecodes/compares pixels, rechecks the
source/folder, then syncs and saves without replacing an existing file.

Mac real CLI/worker tests verify identical PNGs and receipt hashes, invalid-page
cleanup, source preservation, collision rejection and modified-library rejection.
Rust unit coverage rejects truncated, oversized and ambiguous helper replies.
This does not provide OS sandboxing, global resource limits, signed pack
authentication or final preview-cache ownership. PDFium internal allocations and
pack replacement races remain release-hardening work.

Windows run 35818479062 verified the archive and attestation but failed compiling
the helper because Windows headers define min/max macros. The target now defines
NOMINMAX; a new CI run must verify the fix and full engine path. No Windows runtime
success is claimed from the failed run.

### MediaBox and richer rendering fixtures

The helper, CLI and worker now select cropped preview or full MediaBox rendering.
`--media-box` (worker `page_box: "media"`) expands only the in-memory viewport;
original files remain unchanged. A generated negative-origin fixture proves why
this matters: changing a shape outside CropBox leaves preview pixels unchanged,
but changes the MediaBox render. This enables broader preservation checks without
claiming the checks are already integrated into PDF transformations.

Mac native tests also cover embedded RGB image pixels, half-opacity vector
blending, all four right-angle rotations and qpdf rewrite equality for both box
modes. Engine/CLI/worker PNG parity includes the full-page option. PDFium 8066
ignores UserUnit for raster dimensions: fixtures with values 1 and 2 have identical
pixels. Raster scale is therefore capped per default coordinate unit, not physical
points; independent qpdf geometry/UserUnit checks are mandatory for preservation.
Color profiles, masks, complex fonts, form appearances and exact text verification
remain incomplete. Windows CI runs the same expanded fixtures after this commit.

### Bounded page text extraction

The helper/engine/CLI/worker now extract a selected page into UTF-8 TXT through
`extract-pdf-text INPUT OUTPUT.txt RENDER_PACK PAGE_INDEX` / `extract_pdf_text`.
No OCR, language reordering or Unicode normalization is performed. One million
PDFium character entries and four million bytes are the per-page limits; process
timeout/cancellation, pack/library hashes, source checks and no-clobber saving
match the rendering adapter. Invalid Unicode mappings fail without partial files.

A real synthetic ToUnicode fixture revealed that PDFium's GetUnicode returns
surrogate entries for an emoji. The adapter joins validated adjacent pairs and
rejects orphan surrogates, zero/unmapped characters and mapping errors. Mac tests
verify exact Greek/CJK/emoji/combining UTF-8, scalar/byte/hash receipts, CLI/worker
parity, empty text, collisions, invalid-mapping cleanup, and identical text after
a qpdf rewrite. These fixtures do not establish CJK font rendering or semantic
reading-order parity. Whole-document export and complex font/layout tests remain.

Windows run [35818862988](https://github.com/goamaan/fileform/actions/runs/35818862988)
passed at 6f9e6e0, including the fixed helper build, verified archive/attestation,
real renderer fixture checks and Rust CLI/worker PNG tests. It predates MediaBox
and text extraction; newer runs must validate those independently.

Windows run [35819141366](https://github.com/goamaan/fileform/actions/runs/35819141366)
then passed at 8d47ad7, adding full MediaBox, embedded image/transparency, all
rotations, UserUnit limitation and full-page CLI/worker PNG checks. Text extraction
is newly added after that run and still requires its own Windows result.

### Lossless structural compression

`compress-pdf INPUT OUTPUT.pdf PDF_PACK RENDER_PACK [--max-bytes BYTES]` and worker
`optimize_pdf` now port the original lossless compression policy. qpdf recompresses
flate streams at level 9 and generates object streams; no image resizing or lossy
encoding occurs. Every candidate must pass strict qpdf validation, the exact
canonical reachable-graph digest, ordered geometry, full MediaBox pixel hashes
and exact extracted text on every page. Signed/encrypted/annotated/form/outline/
tagged/interactive documents remain explicitly ineligible under the reference
policy. PDF version may increase to 1.5.

The input is snapshotted and rechecked, candidates are private and size-monitored
at 512 MiB, and final saves sync without replacing existing files. Compression
returns `not_smaller` with no output when the candidate is not smaller. An explicit
byte target instead requires a fully verified candidate within that target, or
fails without saving; this matches the original fit policy. Each document's
render/text verification has a ten-minute subprocess budget and each helper call
is capped at 60 seconds; qpdf phases retain their separate 60/120-second bounds.
No per-page disk snapshots or preview artifacts are created in the proof loop;
each helper still loads the document into bounded input memory.

A real inherited-MediaBox fixture exposed that PDFium's direct getter does not
resolve parent entries. Preservation now supplies the independently resolved qpdf
MediaBox coordinates to the helper; it sets only an in-memory viewport. Public
standalone `--media-box` preview still needs a directly accessible MediaBox and
fails safely when unavailable. Unifying that preview with qpdf geometry remains
work for the final app pipeline.

Mac smoke coverage passes text, inherited geometry, images/transparency and
Unicode documents; CLI/worker operation; real size reduction; unchanged-size
retention; explicit byte targets; collision handling; and annotation/invalid-text
rejection. Windows CI now runs the same compression suite. Lossy PDF optimization,
composition, OCR and broader resource/security/performance release gates remain
open; this increment does not establish full PDF parity.

Windows run [35819512430](https://github.com/goamaan/fileform/actions/runs/35819512430)
passed at 9952b5c, including the Unicode helper and CLI/worker text fixtures.
Lossless compression and resolved inherited-box rendering are newer and await
their own Windows run.

### PDF composition: merge, select, reorder, duplicate and rotate

`merge-pdf OUTPUT.pdf PDF_PACK RENDER_PACK INPUT.pdf...` merges all source pages
in input order. Worker `compose_pdf` accepts `inputs`, `output`, `directory`,
`renderer_directory`, `allow_document_changes: true`, and optional `pages`. Each
selection has zero-based `source_index` and `page_index`, plus an optional
`clockwise_rotation` in multiples of 90 degrees. Omitting `pages` merges all pages.
The API requires acceptance of document-level changes; the explicit CLI merge
action selects that policy and returns the corresponding warning.

Reference limits are retained: 128 sources, combined 512 MiB input, 1000 output
pages, and 512 MiB output. Sources are immutable snapshots; output is size-bounded,
strictly checked, synced and committed without replacing an existing file. A
private qpdf job JSON avoids Windows command-line length limits and supports
interleaved selections. Expected geometry includes the requested rotation; the
helper renders the matching in-memory orientation before comparing every output
page's text and pixels. This does not claim document-level graph identity: the
result is a new document, and metadata/bookmarks/form behavior and signature
certification are explicitly not guaranteed.

Mac end-to-end fixtures pass mixed text/image/Unicode PDF merges, page ordering,
independent rotations of duplicate selections, negative-origin/inherited boxes,
source hashes, output hashes, collision and invalid-selection handling. Compression
and render/text regressions also pass with the orientation-aware verifier. The
Windows workflow now exercises the same composition suite. Still-image inputs,
atomic split-directory output, and complex forms/annotations need additional
implementation/acceptance before full original composition parity is claimed.

Windows run [35820043378](https://github.com/goamaan/fileform/actions/runs/35820043378)
passed at 668a4d4, establishing the earlier lossless compression and inherited-box
verification baseline. It predates this composition implementation.

### Still images in PDF assembly

Composition now accepts the existing verified PNG/JPEG/TIFF SDR pipeline as well
as PDFs, detecting source content rather than relying on its extension. Every
image becomes one page at one PDF point per oriented pixel. ICC/gamma conversion
and all EXIF orientations use the existing image preparation code. Extended/HDR
and unsupported image cases remain rejected rather than silently flattened.

A safe Rust writer emits only generated PDF objects, embedding straight RGB, a
grayscale alpha soft mask and an sRGB ICC profile. It streams samples in 4096-pixel
blocks, bounds combined prepared image PDFs to 512 MiB, and releases decoded pixels
before qpdf independently reads and hashes the RGB, alpha and profile streams.
The normal assembly path then verifies every final page's geometry, text and
rendered appearance. Descriptive image metadata is intentionally omitted and
disclosed; source files remain unchanged. Uncompressed intermediate PDFs are
private staging files, not final exports.

Mac fixtures pass all eight EXIF orientations, native-scale rendered pixel colors,
transparent and half-transparent samples, mixed PDF/PNG/JPEG/TIFF merging, page
sizes, output hashes and high-depth rejection. Enlarged PDFium previews may
interpolate samples, so source-color assertions use a one-to-one render scale;
embedded-stream checks remain exact. Windows CI runs the same image suite.

Windows run [35820493422](https://github.com/goamaan/fileform/actions/runs/35820493422)
passed at f140c0e, covering PDF-only merge, selection, duplication and independent
rotation. The new still-image assembly paths require their own Windows result.

### Complete split-folder publication

`split-pdf OUTPUT_FOLDER PDF_PACK RENDER_PACK INPUT...` creates one PDF per source
page. Worker `split_pdf` accepts the composition inputs/pack paths and explicit
`allow_document_changes`, plus optional `groups` of page selections for custom
parts. Names are `part-0001.pdf`, etc. Every part uses the same geometry, text and
render verification as composition. Sources and prepared image pages are shared
across groups instead of re-decoded for each part.

All parts stay inside an owned staging directory until the entire set succeeds.
Source checks are repeated immediately before one no-replace directory move.
Limits are 1000 total selected pages and 512 MiB combined output; each new part
receives the remaining monitored byte budget. The receipt stores the output root
once, compact part names/hashes and shared source hashes/warnings, and must fit
the worker response budget before publication. A failure or cancellation removes
the owned staging tree without publishing a partial folder.

Do not replace this with a preflight existence check followed by ordinary rename:
[standard rename can replace an empty directory](https://doc.rust-lang.org/std/fs/fn.rename.html).
The pinned `tempfile` 3.27.0 no-clobber implementation uses macOS RENAME_EXCL and
Windows MoveFileExW without REPLACE_EXISTING. The enclosing TempDir owns cleanup;
TempPath cleanup is disabled for this directory adapter. Unsupported filesystem
operations fail closed. Mac unit tests cover destinations created after staging
(file, empty folder, populated folder), success, cancellation and cleanup. Broader
hostile parent-path races and crash durability remain shared release-hardening
requirements, not claims established by these tests.

Mac real-worker tests pass default and custom-group splits, receipt hashes,
existing-folder protection, rollback after a successful first part followed by an
invalid second group, and cancellation after observing the first staged part.
Windows CI includes these tests; its result is still required.

Windows run [35821016403](https://github.com/goamaan/fileform/actions/runs/35821016403)
passed at 266496a, including mixed PNG/JPEG/TIFF/PDF assembly and orientation/alpha
fixtures. This predates the split implementation.

### Whole-document embedded-text export

`export-pdf-text INPUT OUTPUT.txt PDF_PACK RENDER_PACK` / worker `export_pdf_text`
exports every page, trimming page-edge whitespace and retaining the reference
app's multi-page labels and form-feed separators, with a final newline. The source
is snapshotted once. Text is validated one page at a time and streamed to bounded
staging; the final file is independently hashed against the expected written
bytes, synced, source-checked and saved without replacement. Limits remain
512 MiB input/output and 1000 pages, with the helper's per-page character limit,
60-second calls and a ten-minute extraction budget.

This is not full original text-export parity: the Swift app automatically runs
Vision OCR on pages without embedded text. No portable OCR engine is installed
or integrated yet. The default portable operation returns `ocr_required` and
saves nothing if a page has no embedded text. An explicit `--allow-missing-text`
(worker `allow_missing_text: true`) writes a visible marker on each such page and
reports its zero-based index. The receipt always says `ocr_performed: false`.
Do not advertise this as scanned-PDF text recognition or silently substitute it
for the original OCR-backed conversion.

Mac end-to-end fixtures pass exact multi-page labels/separators, Unicode, single-
page formatting, CLI/worker equality, source/output hashes, collisions, cleanup
after a later page requires OCR, explicit missing-page disclosure and invalid
Unicode rejection. The Windows workflow includes this suite.

Windows run [35821665428](https://github.com/goamaan/fileform/actions/runs/35821665428)
passed at a140e5f, including split-folder publication, existing-folder rejection,
later-group rollback and cancellation after the first staged part. Whole-document
embedded-text export is newer and awaits its own Windows result.

Windows run 35822052666 passed the native renderer, compression, composition,
images and split tests but failed the new document-text test because Python used
Windows CP1252 to read a UTF-8 file. The assertion now checks the exact UTF-8 bytes
instead of the host default text encoding; a new Windows result is required.

Windows run [35822398439](https://github.com/goamaan/fileform/actions/runs/35822398439)
passed at dc303eb after the exact-byte test fix, including whole-document embedded
text export. Portable OCR dependency evaluation is now recorded in
[OCR_RESEARCH.md](OCR_RESEARCH.md); its English raster baseline is not OCR parity
or an integrated app capability.

### Scanned-page fallback with explicit English OCR

`export-pdf-text ... --ocr OCR_PACK eng` now adds local recognition for pages with
no embedded text. The worker accepts `ocr: {directory, language: "eng"}`. It uses
the full resolved MediaBox at a 4096-pixel longest edge, verifies raster framing,
and reuses a private, rehashed model snapshot. Page labels/separators remain intact.
Receipts report zero-based `ocr_pages`, `pages_without_recognized_text`, and the
explicit `ocr_language`; text-only pages do not invoke OCR. Blank multi-page entries
are marked, while a blank single page saves nothing.

Mac fixtures pass mixed embedded/scanned/blank documents, exact final text and
hashes, CLI/worker parity, source preservation and collision/annotation rejection.
Standalone image OCR and embedded-only extraction remain regression requirements.
The Windows PDF workflow now builds the pinned OCR dependency and runs the same
integrated fixture. Automatic language detection, annotations/forms and broader
scan-quality coverage remain parity gates; see [OCR_RESEARCH.md](OCR_RESEARCH.md).

### Static annotation/widget appearances

Static AcroForm text widgets and normal annotations now render together, including
missing widget appearances generated from stored values. The helper has no action,
timer, navigation or JavaScript services. Native fixtures verify Hidden/NoView
exclusion, printable-but-hidden cases, nonzero crop origins, all right-angle
rotations, rewrite stability and unchanged sources. Explicit-English OCR can now
read these rendered appearances; an empty initial OCR result gets one adaptive
threshold retry within the same deadline. XFA/encryption remain unsupported for
OCR, and wider form/recognition coverage is still required.

### Explicit-resolution raster preparation

`plan-pdf-raster INPUT PDF_PACK DPI` / worker `plan_pdf_raster` now computes
read-only page targets for 36–600 DPI from effective crop boxes, native rotation
and UserUnit. Dimensions round upward; targets above 16384 pixels per side or
64 million pixels fail instead of reducing resolution. The helper's internal
`raster:WIDTHxHEIGHT` mode renders exact bounded targets and accepts resolved crop
coordinates/rotation. Preview and OCR sizing retain their separate policies.

Mac fixtures verify 600-DPI targets beyond the preview cap, inherited geometry,
UserUnit scaling, native/extra rotation arithmetic, CLI/worker plan equality,
exact helper output lengths, existing appearance pixels, XFA rejection and
oversized-target failures. This is preparation, not the complete page-image
export feature: bounded file-backed raster transport, PNG/JPEG encoding with DPI
metadata, selected/batch publication and final app integration still remain.

Windows [PDF run 35829743560](https://github.com/goamaan/fileform/actions/runs/35829743560)
passed at 61f2caa, including static forms, adaptive OCR and normalized line endings.
It predates this exact-resolution work, which has a new Windows fixture step.

### Streamed single-page PNG export

`export-pdf-png INPUT OUTPUT.png PDF_PACK RENDER_PACK PAGE DPI` and worker
`export_pdf_png` now save an explicit page at 36–600 DPI. Crop, rotation and UserUnit
determine dimensions; the established 16384-side/64-million-pixel limits reject
oversized requests without scaling down. Output is opaque RGB with sRGB and
physical-resolution metadata. Rasterization losses are reported in the receipt.

Large helper replies are captured into owned staging files with a hard byte
ceiling, 64 KiB transfer buffers and bounded diagnostics. They are not returned
through JSON or accumulated as an extra full-image Vec. PNG encoding streams from
that raster; verification independently decodes rows, checks all pixels against
the raw raster hash, checks DPI/color metadata and strict PNG checksums/end record,
then rechecks the source and atomically saves without replacing a file. The native
renderer still needs its bounded bitmap allocation; OS resource containment remains
a separate release requirement.

Mac fixtures independently decode PNG chunks/pixels and verify high-DPI output,
physical-resolution rounding, inherited geometry, UserUnit, form appearances,
CLI/worker equality, file hashes and collision handling. Shared process tests cover
binary preservation, strict file-output caps, child failure, timeout and cancellation.
The Windows workflow includes the real export fixture. JPEG, selected/batch output,
mixed image sources and final desktop wiring still remain for full page-image parity.

Windows run [35832062083](https://github.com/goamaan/fileform/actions/runs/35832062083)
passed at ea9e1f9, proving the earlier exact-target helper and DPI planner. The
streamed PNG exporter is newer and needs its own Windows acceptance.

### Single-page JPEG export — October 1, 2026

`export-pdf-jpeg INPUT OUTPUT.jpg PDF_PACK RENDER_PACK PAGE DPI [QUALITY]` and
worker `export_pdf_jpeg` share the exact-size rendering, snapshot and no-clobber
publication path with PNG. JPEG accepts 36–600 DPI and integer quality 5–100
(default 85), writes JFIF density in inches and an sRGB ICC profile, and reports
the lossy encoding policy. The complete JPEG is independently decoded to verify
dimensions, RGB samples, ICC/orientation and its end marker; the JFIF header must
match the requested DPI. Quality is explicit and never silently adjusted.

The helper raster stays file-backed. The current safe Rust JPEG encoder requires
a bounded RGB buffer (at most 192 MB under the 64-million-pixel limit); that buffer
is released before full output decode. PNG retains row streaming. This does not
establish final process-memory budgets or cancellation latency at maximum size.
The shared source/size checks and owned staging cleanup remain enforced.

Mac real fixtures pass PNG regressions plus JPEG at qualities 5/85/100, independent
zune-JPEG decode and stdlib PNG pixel checks, measured quality/size/error tradeoffs,
JFIF/ICC metadata, 600-DPI output beyond preview limits, forms/rotation/UserUnit,
CLI/worker decoded parity, collisions and unchanged sources. The ICC encoder uses
a creation timestamp: byte-identical repeated JPEG output is not promised. Each
receipt hash is checked against its own actual file; decoded pixels are compared
for repeated-operation parity. Windows CI runs the same suite. Batch/selected
publication and mixed image sources remain needed for full original raster parity.

Windows run [35833734048](https://github.com/goamaan/fileform/actions/runs/35833734048)
passed at afab174, covering streamed PNG output. The JPEG increment is newer and
requires its own Windows result.

### Ordered page-image folders — October 1, 2026

`export-pdf-images FOLDER PDF_PACK RENDER_PACK png|jpeg DPI INPUT...` exports all
source pages in input order. Worker `export_pdf_images` accepts optional explicit
`pages` with the same zero-based source/page selection and extra clockwise rotation
as PDF composition, plus DPI, format, JPEG quality and `allow_rasterization: true`.
Selections preserve order and duplicates. Naming is `001.png`/`001.jpg`, etc.; the
receipt records each image's source/page/rotation, dimensions, bytes and hash.
This is page rendering; embedded image object extraction remains a different task.

The operation shares verified source snapshots and prepared PNG/JPEG/TIFF image
pages with composition. Every selected geometry/target is checked before rendering;
all bound sources, including unused ones, are rechecked before final publication.
Each output retains the independently verified single-page encoding path. Limits
are 128 bound sources, 512 MiB combined input/prepared-image staging, 1000 selected
outputs, 512 MiB combined final image output and bounded result metadata. Each
encoder receives the remaining output byte budget. Images inherit the composition
policy of one PDF point per oriented pixel, sRGB normalization and white-backed
raster output. The final directory moves exclusively only after the full set
succeeds. Cancel/failure cleans the owned tree without publishing partial results.

Mac fixture evidence covers mixed PDF/image input, exact order, duplicates with
independent rotations, pixel equality against the planned renderer, DPI/receipt
provenance, default JPEG batch equality to standalone decoded JPEGs, invalid
selection/fidelity/quality/DPI rejection, existing-folder protection, cancellation
after the first staged image and a changed unused source. Shared single-page PNG/
JPEG, composition and split regression tests pass after the refactor. Windows CI
now includes the same folder fixture; its result is still required. CLI named
selection/rotation options, collision rename policy and final UI integration remain
open even though the worker can express ordered page selections.

### Embedded image extraction — October 1, 2026

`plan-pdf-images PDF_PACK INPUT...` and `extract-pdf-images FOLDER PDF_PACK INPUT...`
now inspect/export embedded image XObjects. Worker `extract_pdf_images` accepts
explicit source/page selections and a dry-run flag. Selection uses intrinsic image
pixels; rotation is rejected. The resource traversal follows inherited/indirect
resources and nested Forms, terminates cycles, and deduplicates by source/object/
generation. Every retained resource page/path is reported. Resource references
may include unused images; inline images, annotation appearances and patterns are
not enumerated, matching the original documented extraction policy.

Eligible standalone RGB/gray DCT JPEG bytes are copied exactly after strict
preallocation geometry/component/orientation checks and full decoding. Other
eligible 8-bit RGB/gray streams are decoded by qpdf from plain/Flate/ASCII85/
ASCIIHex/RunLength data and reconstructed into exact straight RGBA PNG, including
same-size gray soft-mask alpha and RGB under alpha zero. Candidate skip reasons
cover the original unsupported colors, bit depths, masks, predictor parameters,
filter chains and alternate semantics; runtime corruption fails the entire job.

Metadata JSON and extracted streams use bounded file-backed process capture.
Source/output bounds are 512 MiB, encoded JPEG streams 256 MiB, individual images
16384 pixels per edge/64 million pixels, total decoded RGBA-equivalent samples
512 MiB, metadata 32 MiB per source, objects 100000, resources 100000 visits,
Form/indirect depth 32 and ancestry 64. Provenance is bounded to 4096 bytes per
path/4 MiB total; the existing worker receipt cap can reject an otherwise larger
metadata result explicitly before folder publication. Nothing is silently dropped.
Physical source aliases are rejected. All outputs are staged and verified before
one exclusive folder publication; cancellation/failure removes the owned tree.

Mac real fixtures pass original JPEG bytes with nonzero object generation,
RGB/gray/soft-mask samples, hidden RGB, all supported sample filters, inherited/
indirect/nested/cyclic resources, reuse/dedup/provenance, selections/skips, aliases,
no-image plans, malformed sample rollback and cancellation after staging begins.
Windows CI now runs this same suite. The final UI, CLI selection/rename options,
portable setups and broader original fixture coverage still need acceptance.


## Targeted image optimization — October 1, 2026

`optimize-pdf-images INPUT OUTPUT.pdf PDF_PACK RENDER_PACK` now accepts
`--quality`, `--minimum-quality`, `--max-dimension`, `--max-bytes` and
`--dry-run`. The worker route is `optimize_pdf_images`; it requires explicit
`allow_lossy: true`. It recompresses eligible 8-bit DeviceRGB/DeviceGray images
as JPEG, optionally reducing their longest intrinsic edge with integer-floor
aspect dimensions. It retains masked images, images reused as masks, profiled
JPEGs and unsupported sample interpretations with explicit candidate reasons.
JPEG integer quality rounds upward so it never undercuts the requested floor.

Byte fitting makes at most six descending attempts, including the exact quality
floor, each decoded afresh from original samples. Every attempt must preserve the
entire expected graph: only planned image dictionaries/streams can differ. All
page geometry and extracted text must match, and every page must successfully
render. Rendering/text verification share a ten-minute budget. Candidate files
are privately staged, bounded, source-rechecked and published without clobbering;
non-smaller results keep the original without writing a redundant output. Target
misses include attempted qualities and candidate reasons in the structured error.

Mac fixture evidence: real three-page photographic/gray optimization, independent
complete expected-graph comparison (vectors/fonts/text/metadata/unsupported CMYK),
resize floor rounding, byte fit, noninteger floors, actual retained masks and ICC
JPEG streams, CLI/worker equivalence, collisions, non-smaller retention, cancellation
after a candidate exists, changed-source cleanup and unsupported document rejection.
Workspace tests (97 passed, two subprocess fixtures intentionally ignored then
invoked by their supervising tests), Clippy and Windows cross-check passed.
Windows runtime acceptance is pending the new workflow; the preceding extraction
run exposed Windows FlushFileBuffers on a read-only file, corrected to open the
owned staged artifact for reading and writing before hashing/flushing. None of
this establishes Electron integration or production release acceptance.

Windows run [36921065308](https://github.com/goamaan/fileform/actions/runs/36921065308)
at `36dd0b1` now passes the complete native PDF fixture set, including extraction,
lossy optimization/fit, actual mask/profile preservation and cancellation/source
change cleanup. This supersedes the pending Windows acceptance above.


## Page planning and collision policies — October 1, 2026

`split-pdf FOLDER PDF_PACK RENDER_PACK INPUT...` supports exactly one optional
`--every COUNT`, `--ranges '1-3;4,2;5'` or `--after '3,7'`, plus `--dry-run`.
Without a selector it still exports individual pages. Positions are one-based
in the concatenated inputs; intervals include a final shorter group, ranges
preserve order/duplicates, markers must be unique increasing positions before
the last page. Worker `split_pdf.selection` is a tagged `{mode: ranges|every|after}`
object. Explicit `groups` and selection are mutually exclusive. Dry-run returns
validated groups and source hashes without creating an output directory.

Merge, batch page export and embedded extraction now accept `--pages '3,1-2,3'`;
worker `page_ranges` and explicit `pages` are mutually exclusive. Parse/resolve
happens against the same private source snapshots used by execution. Selection
is bounded to 16384 bytes and 1000 output pages. Batch JPEG also accepts
`--quality 5-100`. Use `--` before literal input names starting with `--`.

These routes and image optimization support `--collision fail|rename` (worker
`collision`, default fail). Rename atomically publishes the **same verified staged
candidate** as `name (1).ext`, `name (2).ext`, or `folder (1)`, with 1000 bounded
name candidates. It does not rerun processing or take a new source snapshot on a
late conflict. Actual destination is returned in the receipt. Complete-directory
publication preserves every existing file/folder, including empty directories,
symlinks and late arrivals. Plans do not reserve an output name.

Mac CLI/worker fixtures cover mixed-source range order/duplicates, split interval
remainder, markers, invalid/conflicting selectors, no-output plans, actual rename
receipts for split/raster/extraction/optimization, source preservation and existing
cancel/change cleanup. Workspace tests pass 99 tests with two supervised subprocess
fixtures ignored in their standalone form; Clippy and Windows cross-check pass.
Current Windows runtime acceptance for this planning/policy increment is pending.
Desktop integration remains open.
