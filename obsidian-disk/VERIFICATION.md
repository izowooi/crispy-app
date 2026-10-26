# Windows verification — 2026-10-26

Verified on the development Windows x64 machine with Rust 1.99.1 / MSVC. These observations cover this machine; they are not a hardware comparison with CrystalDiskMark or a macOS validation.

## Automated checks

- `cargo fmt -- --check`: passed.
- `cargo clippy --all-targets --locked -- -D warnings`: passed.
- `cargo test --locked`: 2 passed, 0 failed.
  - Real unbuffered I/O for all four workloads, both read and write; finite positive scores and byte/IOPS conversion checks.
  - Running Q32 write cancellation within 2 seconds; invalid duration rejection; normal completion and cancelled preparation remove the temporary file; an existing neighbouring file remains unchanged.
- `powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/package-windows.ps1`: built and packaged the explicit `x86_64-pc-windows-msvc` target.
- PE dependency inspection (`dumpbin /dependents`): Windows system DLLs only; no VCRUNTIME DLL dependency after static CRT linkage.

## Native app checks

Using the computer-use skill, verified the compact 510 × 395 client layout and selectors. A 64 MiB / 1 run / 1 s whole-suite run completed, Stop cancelled a second run, and closing during a run left no process or benchmark temporary file.

Launched the **final packaged executable** and ran the full default configuration: 1,024 MiB, 3 measured runs plus one discarded warm-up per workload, 5 seconds per run. All eight cells populated, status became `Complete · test file removed`, and filesystem inspection found no benchmark temporary file. The app remained responsive. Copy results returned the matching settings and all eight MB/s, IOPS and latency values.

| Workload | Read MB/s | Write MB/s |
| --- | ---: | ---: |
| SEQ1M Q8T1 | 3081.00 | 2566.58 |
| SEQ1M Q1T1 | 2609.18 | 1794.70 |
| RND4K Q32T1 | 412.29 | 336.70 |
| RND4K Q1T1 | 45.23 | 103.65 |

Values are one local smoke run, not reference performance targets. µs is application-observed completion latency as described in the README.

## Local artifacts

- `dist/obsidian-disk-windows-x64/obsidian-disk.exe`: 5,040,128 bytes.
- Executable SHA-256: `B9A37CB9C502AA68A3E359FF373CD5BAABEBFEEC39F8B0B404D3C6B88C40AAF6`.
- ZIP: `dist/obsidian-disk-windows-x64.zip`.
- ZIP SHA-256: `FB5DB16333A84548987A7FA2B00B48687A98F2D910545A6BC9541A8BA0DBC81C`.
- Packaged executable hash matched `target/x86_64-pc-windows-msvc/release/obsidian-disk.exe`.

Generated artifacts remain local and are ignored by Git. No release or tag was created. macOS implementation and validation remain a follow-up as requested.
