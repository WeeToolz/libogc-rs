# Raw SRAM getters and setters in Dolphin

Dolphin 2609a on macOS with OpenGL passed 18 checks with zero failures. All
three captured pages were valid. See the [probe log](sram_probe/probe.log),
[result](sram_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 3](sram_probe/pages/page-003.png).

The probe calls raw getters and setters for counter bias, display offset,
RGB60, language, progressive scan, sound, video, wireless ID, and GBS mode.
It compares selected changed values with values calculated from a saved SRAM
snapshot, restores each field, and checks the final standard and extended
snapshots. It also checks an invalid wireless channel. The runner uses a
private Dolphin profile. Source, binary, and fixture identities are recorded.

This is Dolphin evidence for selected inputs and a single thread. Other values,
concurrent access, physical SRAM writes, and physical Wii behavior remain open.
HBC was not under test. No production code changed. The runner stops Dolphin
after the summary, so `exit_code` is null. The stderr log can contain repeated
partial-send and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe sram_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-sram-raw --timeout-seconds 180
```
