# Raw system alarms in Dolphin

Dolphin 2609a on macOS with OpenGL passed 165 checks with zero failures. All
15 captured pages were valid. See the [probe log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json), [identities](identities.json),
and [pages 4](runtime_probe/pages/page-004.png) and [5](runtime_probe/pages/page-005.png).

The probe calls raw system alarm creation, one-shot scheduling, periodic
scheduling, cancellation, removal, and the state snapshot. It checks invalid
inputs, callback handle and argument, repeated callbacks, no callbacks after
cancellation during eight retraces, and selected snapshot fields after removal.
The probe removes the alarm after use. The C reference informs the operations;
the expected callback and cancellation results are checked at runtime.

This is Dolphin evidence for selected timing and error cases. Other timing,
concurrent alarms, physical Wii behavior, and callback guarantees beyond the
observed interval remain open. HBC was not under test. No production code
changed. The runner stops Dolphin after the summary, so `exit_code` is null.
The stderr log can contain repeated partial-send and alignment warnings. All
checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-alarms-raw --timeout-seconds 180
```
