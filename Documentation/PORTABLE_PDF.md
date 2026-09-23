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
