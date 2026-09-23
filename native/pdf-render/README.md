# PDF renderer helper — evaluation

This native adapter evaluates the pinned non-V8 PDFium distribution documented in
`Documentation/PDFIUM_RESEARCH.md`. It is not yet a production tool pack or a
public engine command. JavaScript/XFA build flags are checked at configure time;
the caller must independently verify archive hashes and provenance first.

Build with CMake, setting `PDFIUM_ROOT` to the extracted verified archive. On
Windows use the Visual Studio x64 generator; on macOS use the matching native
architecture. The build copies the library alongside the helper and adjusts the
Mac executable's library reference. Distribution signing is not performed here.
Retain the archive's LICENSE and complete licenses directory when redistributing.

`fileform-pdf-render SNAPSHOT PAGE_INDEX MAX_EDGE` writes one binary P6 PPM image
to stdout. Page indexes are zero-based; maximum edge is 1–2048 pixels, with a
second cap of 2 pixels per point. Input is bounded to 512 MiB and 1000 pages.
Output is white-backed RGB, at most 2048²×3 bytes plus a short PPM header.
PDFium supplies effective crop and intrinsic rotation; this is not the Swift
reference's MediaBox fingerprint. Noninteractive annotations are included;
widgets/popups, forms and XFA do not have appearance parity. No document actions
or form callbacks are invoked. Text extraction is not implemented.

The helper reads a private snapshot into memory. It must run under the engine's
timeout, cancellation, output and concurrency controls before user exposure.
Input-size and bitmap caps do not bound PDFium's internal allocations. Full OS
containment, process-tree cleanup, immutable snapshot/pack integration, source
provenance, UserUnit/font/color coverage and signed packaging are pending. The
standalone helper is not a safe general-purpose document viewer or a sandbox.

Run `crates/fileform-engine/tests/smoke-pdf-render.py HELPER QPDF` for generated
text/vector fixtures, graph-neutral rewrites, deliberate changes, crop/rotation,
Unicode paths, bounds, rejected malformed/protected input and source safety.
Tests compare one renderer against itself, not pixels across different platforms.
