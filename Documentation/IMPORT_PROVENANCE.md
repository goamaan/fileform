# Consolidation provenance

Published September 14, 2026 at https://github.com/goamaan/fileform.

- Root history: public goamaan/fileform-core through
  5fc4656061f7c397d3f5ce7169213f160d63b8bf, cloned without hardlinks.
- apps/macos: reviewed source snapshot from the former private app. Source/tests,
  build tools, app assets and required font licenses were imported; old commerce
  configuration, private Git history, internal plans/design handoffs, local
  credentials, build artifacts and obsolete sibling-clone tooling were excluded.
- website: tracked source snapshot from goamaan/fileform-site at
  7ef8235f5c8e96a42fbc0f679d799b83efebffd3, without private Git history.
  Existing website copy is being replaced before public release.
- Original source is covered by the root Apache-2.0 license under the owner's
  explicit open-source instruction. Dependency licenses remain separate.

The original checkouts, parked password branches and notarized candidates remain
unchanged as local/private backups. The public engine repository was renamed to fileform, preserving its history and
issues. The former private app repository was renamed fileform-native-archive;
it and fileform-site remain private and archived. Their Git histories were not
made public. No history rewrite or force push was used. The reviewed source was
fast-forwarded onto the existing public engine history after the publication
review in PUBLICATION_REVIEW.md.


Local verification: the imported macOS app builds against the root package and
passes the five-executable bundle audit. Native tests initially exposed obsolete
sibling-core fixture paths; those were corrected to the monorepo root and the
63-test suite passed. Logs are local at /tmp/fileform-monorepo-build.log and
/tmp/fileform-monorepo-tests.log. The first Electron/Rust table workflow is now implemented. Both platform CI builds
and native worker checks passed; full operation/UI parity remains incomplete.
