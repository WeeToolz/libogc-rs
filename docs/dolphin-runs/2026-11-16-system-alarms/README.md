# System alarms in Dolphin

Dolphin 2609a on macOS with OpenGL passed 154 checks with zero failures.
All 14 captured pages were valid. See the [probe log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 4](runtime_probe/pages/page-004.png). The page shows probe text.

The probe creates an alarm and schedules a one-shot callback with a 30 ms
delay. It checks the callback count, alarm handle, and opaque argument. It
then schedules a 30 ms periodic alarm and waits for at least three additional
callbacks. After cancellation it checks that the count remains unchanged for
eight retraces, then removes the alarm. The production create, set, periodic
set, cancel, and remove APIs are called directly. The test uses bounded
retraces, rather than asserting exact scheduling latency.

This checks one timing configuration and cancellation after callbacks. It does
not prove every timing or concurrency case, physical Wii behavior, or HBC.
The runner stops Dolphin after the summary, so `exit_code` is null. The stderr
log contains repeated partial-send warnings. All checks and pages completed.
The runner uses a private NAND copy and removes it after the run.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-16-system-alarms --timeout-seconds 180
```
