# Measured performance

## Native table CLI comparison — September 14, 2026

MacBookPro18,3, 16 GiB RAM, macOS 26.2; optimized Swift and Rust executables.
Each case has one warmup and five measured runs, alternating CLI order. Wall time
includes process launch, parsing, encoding, verification, publication and receipt.
Every output was independently decoded with Python and compared cell by cell;
the original bytes were checked after every run. Filesystem caches were warm.

| CSV fixture | Input | Swift median | Rust median | Swift peak RSS | Rust peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: |
| 100 rows × 4 columns | 5,337 B | 0.021 s | 0.012 s | 14.5 MiB | 1.8 MiB |
| 50,000 rows × 6 columns | 3,450,055 B | 1.492 s | 0.154 s | 326.3 MiB | 1.8 MiB |
| 2,000 rows × 100 columns | 1,822,991 B | 0.985 s | 0.104 s | 101.6 MiB | 2.0 MiB |

RSS figures are medians of each run's maximum resident set size, as reported by
macOS /usr/bin/time -l (getrusage documents this field in bytes). They are not
whole-system memory, unique memory, GUI memory or measurements of all file types.
The implementations do different work and emit differently formatted JSON;
semantic cell equality is the comparison criterion. Other machine activity was
uncontrolled. These are local observations, not universal speed claims.

The first benchmark attempt exposed the Swift reference verifier reusing its
8 MiB input cap for expanded JSON output. The verified measurements above include
the fix: separate 128 MiB output verification, bounded row-wise encoding, and
regression coverage. This correction preserves the 8 MiB source-input limit.

Reproduce with release builds and `python3 Tools/benchmark-tables.py`. The script
stores binary checksums, toolchain versions, all samples and fixture hashes.
[Recorded samples](Benchmarks/table-cli-macos-2026-09-14.json) were collected with
the reference fix uncommitted; the retained binary hashes identify the measured
builds. Do not compare these numbers to a different unrecorded build.

## Still required

- Electron versus SwiftUI launch, idle and active total-memory measurements.
- Windows runtime measurements on the same fixtures with an OS-specific harness.
- Image, PDF and media throughput and memory on real parity fixtures.
- Repeat representative measurements after each engine family is ported.
- Set regression thresholds from stable repeated runs, not this single machine.
