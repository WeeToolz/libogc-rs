# Wii configuration values in Dolphin

Dolphin 2609a on macOS with OpenGL passed 120 checks with zero failures.
All 12 captured pages were valid. See the [probe log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 6](runtime_probe/pages/page-006.png). The page shows probe text.

The probe initializes `ogc-conf` and reads nine one-byte settings from the
private NAND copy. It checks the generic length, type, and value APIs and
missing-key, small-buffer, and wrong-length errors. It then sets known values
in memory and checks language, sound, aspect ratio, progressive mode, RGB60,
sensor bar, speaker volume, motor mode, and screen saver getters. It restores
and reads back all nine original bytes. It does not call `save_changes`, so
the temporary settings are not written to NAND. The generic setter and getter
share the configuration buffer; these checks do not independently prove a
persisted settings write or every valid value.

The [first run](../2026-11-16-conf-fixture/suite.json) had 118 passes and
one failed RGB60 check. The source NAND contained 1, but the probe did not
read 1 after Dolphin boot. The private NAND copy was removed by the runner,
so the exact source of the difference was not established. The revised test
uses known in-memory values and restores the values it read, instead of
assuming a particular initial value. The first failure remains recorded and
is not counted as a pass.

This covers one-byte settings on this Dolphin profile. Persistent writes,
other configuration types, physical Wii, and HBC remain open. The runner
stops Dolphin after the summary, so `exit_code` is null. The stderr logs contain
repeated partial-send warnings. Both runs captured valid pages. The profile is
a private copy; the runner removes its NAND copy after the run.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-16-conf-values --timeout-seconds 180
```
