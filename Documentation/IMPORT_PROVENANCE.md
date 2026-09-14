# Consolidation provenance

Prepared September 14, 2026; not yet published as the canonical repository.

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
unchanged as local/private backups. No existing repository has been renamed,
made public, deleted or force-pushed by this import. A broader source/secret and
license review remains required before publication; a narrow credential-pattern
scan of the imported app/site found no matches.


Local verification: the imported macOS app builds against the root package and
passes the five-executable bundle audit. Native tests initially exposed obsolete
sibling-core fixture paths; those were corrected to the monorepo root and the
63-test suite passed. Logs are local at /tmp/fileform-monorepo-build.log and
/tmp/fileform-monorepo-tests.log. No portable desktop/Windows completion is claimed.
