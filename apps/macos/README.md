# Native macOS reference app

This is the working SwiftUI/AppKit application, preserved during the macOS and
Windows desktop migration. It is free/open source under the root Apache-2.0
license. There is no commerce or activation service.

Run `Tools/build-development.sh` from this directory after building the root
engine packs. The Xcode project consumes the root Swift package. XcodeGen is
required. Signed/notarized builds use the existing release tools and credentials
stored outside the repository. Do not distribute intermediate migration builds.

The shared cross-platform desktop will live in apps/desktop. Keep this reference
app until advertised workflow parity and native end-to-end checks pass.
