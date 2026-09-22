# User requirements and decisions

Recorded September 14, 2026. These are acceptance requirements, not completion claims.

1. Finish existing advertised behavior before unrelated new features. Preserve an
   explicit feature roadmap and evidence; do not silently narrow the release scope.
2. Make the entire app, engine, CLI and website free/open source in one coherent
   project. Consolidate repositories thoughtfully and use open-source best practices.
3. Cancel all paid aspects: Polar, checkout, trials, activation, account requirements,
   device limits, paid upgrades, entitlement servers and purchase recovery.
4. Provide usable signed downloads and easy, secure app updates/latest versions.
5. Support macOS and Windows. Research Electron and alternatives, including T3 Code's
   actual implementation. A significant migration is acceptable; Windows must at
   least have a real build even when local manual Windows testing is unavailable.
6. Keep heavy transformation and non-UI logic fast, efficient and memory-conscious
   in suitable native code/tools, outside the web UI runtime. Do not promise that
   C/C++ dependencies become memory-safe merely because a wrapper uses Rust/Swift.
7. Use a custom, polished UI and Vicinae-inspired minimalism. The website should be
   similarly restrained, with original Fileform copy/assets and optionally teal.
8. Cut repetitive captions/subheadings/filler across every app and website surface.
   Preserve consequential instructions, errors, privacy facts and format limitations.
9. Align the icon with the app/site. Light/dark/system modes must be readable and usable.
10. Launch as a normal display-filling window, not a separate macOS full-screen Space.
11. Keep local AGENTS.md concise, below 300 lines, and all implementation decisions
    current. Preserve information across turns. No unnecessary permission pauses.
12. Verify end-to-end throughout, record concrete limits, and commit completed
    increments directly to main. The active goal includes the cross-platform migration.

## Evidence retained outside this public source import

The original workspace app/core/site repositories and all private history remain
at ../app, ../core and ../app/Website. Notarized macOS baseline candidates and real
native test evidence remain under ../app/Artifacts. They must not be overwritten.
The latest batch-preview source was built/notarized and native crop/export tested.
A Windows Electron preview installer and native table smoke tests now pass CI.
Full portable-engine parity and manual Windows GUI acceptance remain unfinished.


## Final framework selection

After reviewing the alternatives, the user explicitly chose Electron, not Tauri,
because of its ecosystem. Use Electron + React with native worker/CLI processing.
Do not treat the earlier Tauri recommendation as the implementation decision.

## Website design refinement

Keep the calm Vicinae-inspired typography, spacing and teal identity, but restore
rich product components, workflow widgets and real editor previews from the older
site. Minimal must not mean empty. Use restrained transitions, keyboard-accessible
interactions and reduced-motion support. Inspect both the live reference and its
website source; keep copy concise and platform availability explicit.

## Desktop identity and development cleanup

The final macOS and Windows product is one Electron application with shared Rust
processing. The Swift app is a migration reference. Multiple Fileform Preview
apps left open during QA were not intended product behavior. Keep one development
build running, enforce single-instance launch behavior, and preserve the existing
window/workspace on repeated launch. The Preview name and separate app identity
remain temporary migration safeguards, not a second paid or permanent product.

### Desktop design refinement — September 22, 2026

Follow the generated HTML app designs as a reference, with freedom to improve
execution. Use a modern, minimal Linear-like interface: deliberate typography,
quiet navigation, simple workflows, and snappy, restrained transitions. Preserve
teal identity, light/dark/system appearance, keyboard access and reduced motion.
Only expose implemented tools. This applies to the shared Electron UI on both
macOS and Windows and remains part of release acceptance.
