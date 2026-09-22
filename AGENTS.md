# Fileform

Fileform is one completely free, open-source project for macOS and Windows.
The user's September 14, 2026 decisions supersede all private/paid-app plans.
No Polar, checkout, trial, activation, entitlement or device-limit work is allowed.

Read Documentation/USER_REQUIREMENTS.md and Documentation/CROSS_PLATFORM.md.
Electron + React is the user-selected desktop stack; Tauri was rejected.
Preserve the existing Swift engine/CLI and macOS app as tested references while
porting to the selected cross-platform architecture. Do not remove capabilities
or claim parity without real evidence. Keep shared processing independent of UI.

Use safe Rust for new portable control/engine code where appropriate. Heavy
processing belongs in bounded native workers/tools, not the web renderer. Preserve
originals, validate outputs, prevent clobbers, bound resources, and cancel process
trees correctly on both operating systems. Treat input files as untrusted.

Original source uses Apache-2.0; retain dependency notices. Never import private
history, secrets, local test artifacts, account data or design handoff archives.
Use reviewed source snapshots for repository consolidation.

The website follows Vicinae-inspired minimalism with Fileform's own teal identity,
copy and real screenshots. Keep downloads, docs and contribution paths obvious.

Test end-to-end on macOS. Produce Windows builds and run Windows CI/tests where
possible; distinguish build success, automated tests and manual GUI acceptance.
Keep signed downloads and secure updates in scope. Commit verified increments
on main. New feature breadth follows release stabilization, not the reverse.

During desktop QA, quit superseded test builds before launching a replacement.
Keep only one active Fileform Preview app; separate artifacts may remain on disk.
Repeated launches must focus/restore the existing window, preserving active work.

Final UX is file-first: one drop/choose surface, then contextual actions; also
allow action-first tasks such as PDF merging. Tables/Images workspaces are only a
temporary migration UI, explicitly rejected as the final structure. Restore
original-app parity before the complete UX pass; revisit the generated HTML's
richer composition. Do not keep polishing category-specific entry screens.
