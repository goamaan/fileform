# Verification workflows

Both workflows remain manually dispatchable. A release candidate needs successful
full Core verification and Portable desktop preview runs for its exact source,
plus the documented signing, artifact and installation acceptance checks. Path
filtering during development is not a substitute for release verification.

- Core verification: Swift source/tests/package, native macOS reference, reference
  tools, workflow configuration and shipped LICENSE/NOTICE. Its three Rust-only
  smoke scripts are excluded so their edits do not cancel a long reference run.
- Portable desktop preview: Rust workspace/toolchain, Electron app, the three
  portable smoke scripts, routing check, workflow configuration and LICENSE/NOTICE.
- Website changes use the website build/publication workflow, not either native
  engine matrix merely because they share a repository.

The reference matrix retains macOS 14/26 compilation, pinned media/PDF packs, full
Swift tests, native app tests, CLI E2E and development archive packaging. The
portable matrix retains Windows/macOS Rust tests, Clippy, process/image/cancellation
smoke, desktop validation tests, notices and package creation. No stages were
removed when the path filters were separated.

`python3 Tools/check-ci-routing.py` checks push/PR filter agreement, representative
change routing, manual dispatch and directly invoked Tools scripts. Its matcher
intentionally supports only the literal/*/** filters currently used; it fails on
unrecognized syntax. It does not claim to interpret arbitrary GitHub YAML/globs
or discover all transitive script dependencies. Add helpers and new verification
scripts to the appropriate triggers and extend the routing cases when needed.

GitHub evaluates positive/negative path patterns in order; exclusions follow the
broad Tools include in the core workflow. See the [official workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#onpushpull_requestpull_request_targetpathspaths-ignore).

Evidence: full reference run 34894538829 at
2259e35317f7010a2dd5d5f9fe4625c55b13e7e6 passed on both macOS runners, including the
expanded-table verification correction. Later image/frontend increments require
their own portable evidence; a cancelled job is never recorded as a pass.

Desktop layout commit `d9089e2fd224f4240b84dbefd779a0c95e6f8e60` passed portable
workflow run `35773207660` on both macOS 26 and Windows 2025 (September 22, 2026).
This proves the configured build/tests and installer packaging, not manual Windows
GUI acceptance or signed public distribution.
