# HEIC helper evaluation

Build `Tools/build-heic-evaluation.py --work NEW_DIRECTORY`, then configure this
helper with `-DHEIC_ROOT=NEW_DIRECTORY/install`. macOS and Windows use the same
libheif/libde265 sources. Libraries remain shared and plugin loading is disabled.
The Rust engine/CLI now uses this helper; Electron bundling/integration remains open.

One private source snapshot is decoded per process. The Rust supervisor must own
input/staging and impose cancellation, deadlines and OS resource containment.
Context limits bound library-accounted allocations, not the entire process.

The binary protocol is `FH1\n`, then nine decimal fields and a newline:
width, height, alpha-present, premultiplied, ICC bytes, NCLX primaries, NCLX transfer,
EXIF bytes and HEIF rotation/mirror-properties-applied. Original ICC and EXIF bytes
follow, then exactly width × height × 4 tightly packed RGBA bytes. Pixels retain
the source color space: the Rust adapter manages color and alpha once and validates lengths. HEIF container
transforms own orientation; EXIF display orientation is ignored, matching ImageIO. Nonzero exit rejects the
whole output; partial stdout must never be accepted.

`smoke.py HELPER fixtures` checks synthetic, source-owned HEICs and independently
oriented ImageIO pixel oracles. Regenerate on Mac with
`swift Tools/generate-heic-fixtures.swift NEW_FIXTURE_DIRECTORY`; encoding may vary
with the OS, so retain a fresh hash inventory with reviewed regenerated fixtures.

Full adapter/parity and distribution gates remain in
`Documentation/HEIC_RESEARCH.md`. In particular, basic sRGB/P3/alpha/orientation
fixtures do not establish every gain-map, grid, depth, transfer or conflicting-EXIF
case. Production packaging must retain LGPL notices, exact source/build materials
and a usable component replacement/rebuild path. No signed app is produced here.
