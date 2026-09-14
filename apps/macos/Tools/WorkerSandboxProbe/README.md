# Native worker sandbox probe

This development-only native host proves the public worker can consume a file
selected through NSOpenPanel using inherited file descriptors. It is not a Fileform
product screen and is never included in the customer app.

1. Run `Tools/WorkerSandboxProbe/build.sh`. This builds a real worker, embeds it
   with sandbox-inherit entitlements, and ad-hoc signs the host with user-selected
   file access. No entitlement disables the sandbox.
2. Generate a synthetic chart using the core fixture generator. In a dedicated
   fixture directory, create two copies named `source.png` and `unselected.png`.
   Independently confirm both exist before starting the test.
3. Launch `Artifacts/WorkerSandboxProbe.app` through Computer Use. Click **Choose
   source fixture…**, navigate in the native dialog, select only `source.png`, and
   click **Inspect fixture**. Do not select its parent directory or sibling.
4. Verify PASS, actual family/preview dimensions/bytes, source retention and denied
   sibling access. Capture the native dialog and complete result. Open the picker
   again and cancel; no worker should start.

The source is inspected and previewed by separate short-lived workers. The host
reads the selected source for a before/after hash and attempts one read-only open
of `unselected.png`; an EPERM denial is required for PASS. This check establishes
selected-file sandbox transfer, not protection against every compromised parser
or full-product conversion isolation. Worker cancellation/crash/resource/descriptor
cases are covered separately by public automated tests.

September 9 evidence is recorded in `Documentation/Implementation/PROGRESS.md`.
Artifacts stay under ignored `Artifacts/Verification/w1-worker/`. Development
signatures are not Developer ID or notarization acceptance.
