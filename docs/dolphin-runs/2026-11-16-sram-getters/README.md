# System SRAM getters in Dolphin

Dolphin 2609a on macOS with OpenGL passed 149 checks with zero failures.
All 14 captured pages were valid. See the [probe log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 4](runtime_probe/pages/page-004.png). The page shows probe text.

The probe locks the shared SRAM buffer, saves its full contents, and sets a
counter bias of `0x12345678`, display offset of -7, RGB60 bit, language 4,
and flags `0x86`. After unlocking without writeback, it checks the seven
system getters against those fixed values: counter bias, display offset,
RGB60, language, progressive scan, sound mode, and video mode. It reacquires
the lock, restores the full original buffer, and checks it after another
lock. The local C reference and source hashes are in identities.

This tests getter behavior on one in-memory SRAM pattern in Dolphin. It does
not prove physical SRAM reads, writeback, concurrent access, all bit patterns,
or physical Wii behavior. HBC was not under test. The runner stops Dolphin
after the summary, so `exit_code` is null. The stderr log contains repeated
partial-send warnings. All checks and pages completed. The runner uses a
private NAND copy and removes it after the run.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-16-sram-getters --timeout-seconds 180
```
