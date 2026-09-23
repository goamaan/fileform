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
