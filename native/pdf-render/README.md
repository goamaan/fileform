# PDF renderer helper — evaluation

This native adapter evaluates the pinned non-V8 PDFium distribution documented in
`Documentation/PDFIUM_RESEARCH.md`. It is not yet a production tool pack. The native engine exposes an explicit
render command for evaluation packs; the desktop has not integrated it. JavaScript/XFA build flags are checked at configure time;
the caller must independently verify archive hashes and provenance first.

Build with CMake, setting `PDFIUM_ROOT` to the extracted verified archive. On
Windows use the Visual Studio x64 generator; on macOS use the matching native
architecture. The build copies the library alongside the helper and adjusts the
Mac executable's library reference. Distribution signing is not performed here.
Retain the archive's LICENSE and complete licenses directory when redistributing.

`fileform-pdf-render SNAPSHOT PAGE_INDEX MAX_EDGE [crop|media]` writes one binary P6 PPM image
to stdout. Page indexes are zero-based; maximum edge is 1–2048 pixels, with a
second cap of 2 pixels per PDF default coordinate unit. PDFium ignores UserUnit
for this sizing; the output is a bounded preview, not a physical-DPI export. Input is bounded to 512 MiB and 1000 pages.
Output is white-backed RGB, at most 2048²×3 bytes plus a short PPM header.
The default uses PDFium's effective crop and intrinsic rotation. `media` expands
the in-memory crop to MediaBox before rendering, without saving the document.
Graph and geometry evidence must remain independent of bitmap comparisons. Noninteractive annotations are included;
widgets/popups, forms and XFA do not have appearance parity. No document actions
or form callbacks are invoked. Per-page text extraction is described below.

The helper reads a private snapshot into memory. The engine now supervises a
direct child with a 60-second timeout, cancellation and bounded output; it checks
both executable and library hashes, validates PPM framing, and independently
redecodes the staged PNG before a no-clobber save. Global concurrency limits and
full process containment remain required before desktop exposure.
Input-size and bitmap caps do not bound PDFium's internal allocations. Full OS
containment, process-tree cleanup, immutable snapshot/pack integration, source
provenance, physical-size/font/color coverage and signed packaging are pending. The
standalone helper is not a safe general-purpose document viewer or a sandbox.

Run `crates/fileform-engine/tests/smoke-pdf-render.py HELPER QPDF` for generated
text/vector fixtures, graph-neutral rewrites, deliberate changes, crop/rotation,
Unicode paths, bounds, rejected malformed/protected input and source safety.
Tests compare one renderer against itself, not pixels across different platforms.

`stage-evaluation.py BUILD_DIRECTORY VERIFIED_ARCHIVE_DIRECTORY NEW_PACK_DIRECTORY`
copies the helper/library and upstream notices, then hashes both into a local
manifest. It does not authenticate inputs or produce a signed release. After
staging, `fileform-native render-pdf-page INPUT OUTPUT.png PACK PAGE_INDEX` renders
a 512-pixel preview; the `render_pdf_page` worker request accepts `max_dimension`
from 1 to 2048 and `page_box` as `crop` (default) or `media`. Use the CLI
`--media-box` option for the full page, including content outside CropBox. The PNG is an explicit caller-owned output. No automatic app cache
or preview lifetime policy is implemented yet.

`fileform-pdf-render SNAPSHOT PAGE_INDEX text` extracts a bounded text stream.
Its FT1 header declares Unicode scalar and UTF-8 byte counts, followed by the
exact payload. At most one million PDFium character entries and four million
UTF-8 bytes are accepted. The adapter joins valid UTF-16 surrogate pairs exposed
by PDFium, rejects unmapped/invalid scalars and invalid mapping reports, and emits
no partial result on those failures. It preserves generated whitespace and does
not normalize Unicode or promise semantic reading order. This is not OCR.

`fileform-native extract-pdf-text INPUT OUTPUT.txt PACK PAGE_INDEX` and worker
`extract_pdf_text` publish one page's UTF-8 text without a BOM. They validate the
framing, UTF-8, scalar and byte counts before a synced no-clobber save, with the
same pack verification, source snapshot and process supervision as rendering.
A page without text produces an empty file; image-only pages are not OCRed.
Synthetic ToUnicode fixtures verify Greek/CJK/supplementary/combining mappings,
not font appearance or full language/layout coverage. Whole-document extraction
and complex reading-order/font acceptance remain incomplete.
