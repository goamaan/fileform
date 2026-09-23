# PDFium integration research

Checked September 22, 2026. Dependency evaluation only: the Mac arm64 archive
was downloaded and inspected after research. No PDFium code was executed and no
implementation/runtime acceptance is established.

## Candidate and provenance — verified metadata

The release API currently identifies **chromium/8066**, published September 21,
2026, as latest. Pin this release for evaluation, never a `latest` download URL.
The distribution repository revision is
`f2e9a1c45bb17b85b540abf1af30146ef65416ac`.
[Release](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium/8066),
[release API](https://api.github.com/repos/bblanchon/pdfium-binaries/releases/latest).

| Non-V8 asset | Archive bytes | SHA-256 from release metadata |
| --- | ---: | --- |
| `pdfium-mac-arm64.tgz` | 3,483,717 | `336219e80580b93c6523f44db7dc1de59cc497b13a7390ddac84223f68ca162b` |
| `pdfium-mac-x64.tgz` | 3,676,986 | `841ecac278cdd46288dd065873522cf72f3996560d8978f473d336f01d59942c` |
| `pdfium-win-x64.tgz` | 3,823,498 | `739a57d597d864297909cc40a2411eba728490c76a0fa25e3ea299c7f6b07020` |

Download base:
`https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8066/`.
The downloaded JSON attestation lists matching subject digests, the distribution
revision above and build invocation
[35584475700](https://github.com/bblanchon/pdfium-binaries/actions/runs/35584475700/attempts/1).
Its signature was **not cryptographically verified** in this research. Its resolved
dependency identifies the distribution repository, not the PDFium source revision.
[Attestation](https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8066/pdfium-attestation.json).

The upstream `chromium/8066` branch resolves today to
`fc46361ce75055cd549cb938fae5d6a3fe3a1a05`, also the release notes' newest commit.
This is a candidate source pin, **not proof of the binary's exact source tree**:
the distribution checkout script resolves an upstream branch at build time and
applies packaging patches. Preserve its build logs, patches and resolved dependency
revisions before claiming reproducibility.
[Upstream commit](https://pdfium.googlesource.com/pdfium/+/fc46361ce75055cd549cb938fae5d6a3fe3a1a05),
[checkout recipe](https://github.com/bblanchon/pdfium-binaries/blob/f2e9a1c45bb17b85b540abf1af30146ef65416ac/steps/02-checkout.sh).

## Packaging and disabled capabilities — verified source

The distribution supplies shared libraries. Its staging recipe includes matching
public headers, `args.gn`, version metadata and notices; macOS uses
`lib/libpdfium.dylib`, Windows uses `bin/pdfium.dll` and
`lib/pdfium.dll.lib`. Native build jobs cover all three target architectures.
This establishes artifact availability, not Fileform's minimum OS compatibility.
[Staging recipe](https://github.com/bblanchon/pdfium-binaries/blob/f2e9a1c45bb17b85b540abf1af30146ef65416ac/steps/07-stage.sh),
[build matrix](https://github.com/bblanchon/pdfium-binaries/blob/f2e9a1c45bb17b85b540abf1af30146ef65416ac/.github/workflows/build-all.yml).

For non-V8 builds the recipe sets `pdf_enable_v8=false` and
`pdf_enable_xfa=false`, along with `is_debug=false`, `is_component_build=false`
and `pdf_use_partition_alloc=false`. Upstream documents the first two switches
as removing JavaScript and XFA support. Check the actual archived `args.gn`
before accepting a pack; do not select `pdfium-v8-*` artifacts. Disabling these
features does not make the remaining native parser safe or provide OS isolation.
[Distribution configuration](https://github.com/bblanchon/pdfium-binaries/blob/f2e9a1c45bb17b85b540abf1af30146ef65416ac/steps/05-configure.sh),
[upstream build instructions](https://pdfium.googlesource.com/pdfium/+/fc46361ce75055cd549cb938fae5d6a3fe3a1a05/README.md).

Preserve the complete upstream LICENSE (this revision contains BSD terms and
Apache-2.0 text), the distribution's MIT license, and every applicable dependency
notice. The packaging script collects dependency notices from build inputs but
warns on unknown libraries; audit completeness rather than treating that script
as legal clearance. Fileform's Apache-2.0 license does not replace these notices.
[PDFium LICENSE](https://pdfium.googlesource.com/pdfium/+/fc46361ce75055cd549cb938fae5d6a3fe3a1a05/LICENSE),
[distribution LICENSE](https://github.com/bblanchon/pdfium-binaries/blob/f2e9a1c45bb17b85b540abf1af30146ef65416ac/LICENSE),
[notice collection](https://github.com/bblanchon/pdfium-binaries/blob/f2e9a1c45bb17b85b540abf1af30146ef65416ac/steps/08-licenses.sh).

## C API facts

Use matching headers and `FPDF_InitLibraryWithConfig`, load the document/page,
then close page/document before `FPDF_DestroyLibrary`. All PDFium calls must be
serialized; upstream explicitly says its APIs are not thread-safe.
`FPDF_LoadDocument` accepts UTF-8 paths; `FPDF_LoadMemDocument64` requires its input
buffer to remain valid until document close. A private immutable input snapshot
fits either route.
[Public API](https://pdfium.googlesource.com/pdfium/+/fc46361ce75055cd549cb938fae5d6a3fe3a1a05/public/fpdfview.h).

`FPDFBitmap_CreateEx(width,height,FPDFBitmap_BGRA,buffer,stride)` accepts a caller
buffer. BGRA is four bytes per pixel with independent alpha; the caller owns the
buffer. Check positive dimensions, integer conversions and multiplication before
allocation; explicitly set stride and retain the buffer through bitmap destruction.
`FPDF_RenderPageBitmap` renders into it and returns void, so returning from that
call is not a complete visual-validity proof. `FPDF_REVERSE_BYTE_ORDER` exists,
but an explicit bounded BGRA-to-RGBA channel swap offers a simple protocol with
no ambiguity about initialization/fill byte order. `FPDF_ANNOT` excludes widget
and popup annotations; base rendering alone cannot prove form appearance parity.
[Bitmap/render contracts](https://pdfium.googlesource.com/pdfium/+/fc46361ce75055cd549cb938fae5d6a3fe3a1a05/public/fpdfview.h).

Text uses `FPDFText_LoadPage`, `FPDFText_CountChars`, `FPDFText_GetText`, then
`FPDFText_ClosePage`. `GetText` needs `count + 1` 16-bit units and includes its
terminator in the returned length. Its documented contract is UCS-2 and explicitly
omits characters without that representation; it also includes text outside the
crop box. Do not promise lossless Unicode, visible-text-only extraction, semantic
reading order or OCR. `FPDFText_GetUnicode` provides per-character Unicode values
and merits separate fixtures for supplementary characters and mapping failures.
[Text API](https://pdfium.googlesource.com/pdfium/+/fc46361ce75055cd549cb938fae5d6a3fe3a1a05/public/fpdf_text.h).

## Fileform recommendation — engineering judgment

Keep safe Rust responsible for snapshots, pack verification, checked limits,
process supervision, cancellation, output validation and transactions. Launch a
small `fileform-pdf-render` helper linked to the pinned PDFium library; never load
PDFium into Electron or the general Rust worker. A minimal C/C++ C-API adapter
is a practical explicitly audited native boundary and avoids adding unsafe Rust
to the engine. It is not memory-safe merely because it lives in another process.
An independently reviewed safe Rust binding is an alternative, but none was
assessed here and it would still require subprocess isolation.

Start with one request/document per helper process and one PDFium thread. Pass
only a fixed operation, private snapshot and validated numeric options. Return a
small versioned header plus an exact-length raster or bounded text payload; never
allow the helper to select final user output paths. Validate results again in
Rust. Suggested evaluation caps: existing 512 MiB input/1000 pages; one requested
page at a time; 8192 pixels per side, 16 million pixels total (64 MB BGRA);
1 million extracted units/page; 60-second process deadline. These are proposed
budgets, not measured performance guarantees, and do not bound internal PDFium
allocation. Apply tested OS resource controls and cap concurrent helpers.

Use absolute trusted library paths/loader configuration, a private working
directory, no shell, restricted inherited handles and no network. Add Windows
Job Object tree termination and tested macOS process containment; a child process
alone is not a sandbox. Bundle outside ASAR and preserve sign-library → sign-helper
→ calculate pack hashes → sign outer-app ordering. Keep qpdf's structural/graph
proof independent of raster comparisons. Restrict initial verification to eligible
noninteractive PDFs until annotation/form/rotation/crop/font cases pass.

## Remaining acceptance gates

- Download archives with the pinned digests; cryptographically verify provenance;
  inspect actual headers, flags, notices, architecture, deployment targets,
  imports, loader paths and nested signatures. Retain source/dependency pins and
  patches. No binary-content or complete-source-provenance claim exists yet.
- Build the helper natively for macOS arm64/x64 and Windows x64. Run real fixtures
  for crop/rotation/UserUnit, transparency, fonts, CJK/RTL/supplementary Unicode,
  malformed/encrypted input, oversize pages, rendering failures and cancellation.
- Prove bounded IPC, memory handling, crash recovery, process-tree kill and
  unchanged source bytes. Compare appearances to the Swift reference and verify
  packaging/signing separately. Record unavailable manual Windows GUI testing.
- Establish dependency security-update cadence and evaluate peak memory/latency
  before capability claims. This note neither implements nor approves PDF parity.

## Local archive verification — September 22, 2026

The Mac arm64 archive was downloaded to ignored `Artifacts/pdfium-evaluation/`.
Its SHA-256 matches the table above. `gh attestation verify` against
`bblanchon/pdfium-binaries` succeeded; its JSON verification result identifies the
expected distribution revision and GitHub-hosted build workflow, with a verified
transparency-log timestamp. This supersedes the research-only signature status
above for this archive. It does not resolve the exact upstream source/dependency
revision gap or establish trust in every build input.

Archived `args.gn` confirms `pdf_enable_v8=false`, `pdf_enable_xfa=false`,
`target_cpu="arm64"`, and `target_os="mac"`. Version metadata is 156.0.8066.0.
Mach-O load commands declare macOS 13.0 minimum and SDK 26.0. Imports are system
AppKit, CoreGraphics, CoreFoundation, Foundation and libSystem. The library ID is
`./libpdfium.dylib`; release integration must deliberately configure its loader
path before signing. The archive includes headers and dependency notices; their
presence is not a completed license-completeness audit. Windows/x64 archive
inspection, helper execution, source provenance and production packaging remain
pending. Local verification JSON is retained with the ignored evaluation files.
