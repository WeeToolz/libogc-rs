# Wii text configuration values in Dolphin

Dolphin 2609a on macOS with OpenGL passed 141 checks with zero failures.
All 13 captured pages were valid. See the [probe log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 7](runtime_probe/pages/page-007.png). The page shows probe text.

The probe opens and reads the private NAND `setting.txt` through IOS. It
decodes the 256 bytes independently of the configuration cache, extracts the
`GAME`, `AREA`, and `VIDEO` fields, and maps the recognized strings to fixed
reference values. It checks the production `get_region`, `get_area`, and
`get_video` results against those values. The probe does not print the file
contents. The local C reference and source hashes are in identities.

This checks the field values present in this Dolphin profile. Other regions,
malformed or missing settings, physical Wii, and HBC remain open. The runner
stops Dolphin after the summary, so `exit_code` is null. The stderr log contains
repeated partial-send warnings. All checks and pages completed. The runner
uses a private NAND copy and removes it after the run.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-16-conf-text --timeout-seconds 180
```
