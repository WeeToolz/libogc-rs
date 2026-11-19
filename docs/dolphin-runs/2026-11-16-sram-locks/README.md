# System SRAM locks in Dolphin

Dolphin 2609a on macOS with OpenGL passed 159 checks with zero failures.
All 14 captured pages were valid. See the [probe log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 5](runtime_probe/pages/page-005.png). The page shows probe text.

The probe acquires the standard SRAM lock and checks that another standard or
extended lock returns null. It unlocks without writeback, acquires the extended
lock, and checks the same exclusion in reverse. It unlocks the extended lock
and checks that the standard lock can be acquired and released again. The
four production lock and unlock APIs are called directly.

This checks lock exclusion in one Dolphin thread with no writeback. It does
not prove concurrent thread behavior, physical SRAM writes, physical Wii
behavior, or HBC. The runner stops Dolphin after the summary, so `exit_code` is
null. The stderr log contains repeated partial-send warnings. All checks and
pages completed. The runner uses a private NAND copy and removes it afterward.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-16-sram-locks --timeout-seconds 180
```
