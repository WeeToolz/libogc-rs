# Wii typed configuration values in Dolphin

Dolphin 2609a on macOS with OpenGL passed 129 checks with zero failures.
All 13 captured pages were valid. See the [probe log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 6](runtime_probe/pages/page-006.png). The page shows probe text.

The probe reads and temporarily replaces private in-memory configuration values.
It checks the two bytes of shutdown and LED mode, big-endian counter bias,
IR sensitivity and WiiConnect24 values, a signed display offset, EULA, and
ten big-endian nickname characters. It also checks that an empty nickname
returns `CONF_EBADVALUE` without changing the output. Each check restores and
reads back its original setting. The probe does not call `save_changes`.

For a 74-byte parental data entry, the local C reference checks that the read
length is one and returns `CONF_EBADVALUE` otherwise. The Rust probe checks
that same error and unchanged output buffers for both parental getters. This
is evidence for that error path only; it does not show that a parental password
or answer can be retrieved successfully. The C source hash is in identities.

This is Dolphin evidence for selected in-memory values, not persisted writes
or physical Wii behavior. HBC was not under test. The runner stops Dolphin
after the summary, so `exit_code` is null. The stderr log contains repeated
partial-send warnings. All checks and pages completed. The runner uses a
private NAND copy and removes it after the run.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-16-conf-typed --timeout-seconds 180
```
