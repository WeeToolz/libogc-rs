# Wii controller configuration records in Dolphin

Dolphin 2609a on macOS with OpenGL passed 137 checks with zero failures.
All 13 captured pages were valid. See the [probe log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 7](runtime_probe/pages/page-007.png). The page shows probe text.

The probe reads the registered and guest controller records from a private
NAND copy. It sets synthetic records in memory and checks the complete
structures and selected raw byte offsets for the count, address, name, and
guest link key. It checks the maximum accepted counts and verifies that
counts above the limit return `CONF_EBADVALUE` without changing the records.
It checks that a null guest input clears the guest structure. It restores and
reads back both original structures. It does not call `save_changes`.

This checks the four registered and guest record getters and setters on
selected values. The getter and setter share the configuration buffer, so the
result does not prove persisted writes, controller pairing, physical Wii
behavior, or every record layout. HBC was not under test. The local C
reference hash and run identities are recorded in the identities file.
The runner stops Dolphin after the summary, so `exit_code` is null. The stderr
log contains repeated partial-send warnings. All checks and pages completed.
The runner uses a private NAND copy and removes it after the run.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-16-conf-pads --timeout-seconds 180
```
