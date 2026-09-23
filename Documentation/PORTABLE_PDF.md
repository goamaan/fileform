# Portable PDF migration

Status: qpdf pack verification and bounded structural inspection are available in
Rust/CLI/worker. PDF composition, page operations, compression, image extraction,
rendering/text/OCR, previews and final UI integration remain parity work. Preserve
the Swift/PDFKit/qpdf implementations as references; do not mark their historical
completion as portable acceptance.

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
