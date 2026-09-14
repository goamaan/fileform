# Public source review

September 14, 2026. The owner explicitly authorized making original Fileform
app/engine/CLI/site source free and open source and consolidating the repositories.

## Scope and history

The retained history is the previously public engine history. Private app/site Git
history, internal design handoffs/plans, local artifacts and credentials were not
imported. The exact current source tree includes the working Mac reference,
Electron/Rust migration slice and website. Original source uses Apache-2.0;
third-party licenses remain separate.

Gitleaks 8.30.1 was downloaded from its official release and its archive checksum
verified. It scanned main's retained history with redacted reports. Its sole
finding was a historical 40-character FFmpeg OpenPGP verification fingerprint
used in a VALIDSIG check. The exact finding is documented in .gitleaksignore;
no blanket directory/rule exclusion was added. The reviewed scan found no
unresolved credentials. This is evidence, not a guarantee that every possible
secret pattern is detectable.

## Licenses and dependencies

- Added the upstream shadcn/ui MIT notice for derived website components.
- Retained font OFL notices and native-pack source/license policies.
- Added a build-time collector for locked native Rust dependency licenses, Rust
  toolchain notices, React/runtime notices and Chromium's license document from
  the checksum-verified Electron archive.
- The packaged Mac preview contains 36 component entries; all listed notice hashes
  were independently checked after packaging.
- Website advisories traced to sharp through Cloudflare's tooling. Updated
  @cloudflare/vite-plugin, wrangler and their compatible worker types without
  force/legacy-peer overrides. npm now reports zero known vulnerabilities.

## Project surfaces

Paid/proprietary/Polar/checkout messaging was removed from current website pages.
The site now distinguishes the broad native Mac reference from the partial
cross-platform preview. All nine content pages were rendered at 320px without
page-level horizontal overflow; screenshots loaded and mobile navigation worked.
The website build passed. Contributor, security, support, conduct, issue templates
and the full non-commerce roadmap are included. Private vulnerability reporting
was enabled on the public engine repository and will follow its rename.

## Still not release acceptance

This source review does not establish full Electron feature parity, a Windows
installer/runtime result, complete worker-containment hardening, signed desktop
updates, clean-machine installation or a final public application release.
Those requirements remain in REQUIREMENTS.json and PORTABLE_PROGRESS.md.

Local reports are in Artifacts/audit; they are intentionally not published.
