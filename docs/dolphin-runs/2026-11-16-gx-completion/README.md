# GX completion callbacks in Dolphin

Dolphin 2609a on macOS with Metal passed 281 checks with zero failures.
All 24 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 21](video_probe/pages/page-021.png). The page shows probe text.

The probe installs finish and token callbacks, queues token `0x5a39`, and
requires one token callback with that value and one finish callback after
`finish_frame_and_wait`. It queues `set_draw_done`, waits for a second callback
with a bounded retrace loop, and calls `wait_draw_done` after notification.
It then calls `draw_done` and requires a third finish callback. Finally it
restores the old callbacks and checks that the setters return the handlers
that were installed. This checks the completed-event path of `wait_draw_done`;
it does not show that a thread sleeps while a finish is pending. The local C
reference hash is in the identities file.

The runner now requests `Graphics.Hacks.EFBToTextureEnable=False` so that the
EFB texture-copy probe can compare the copy in RAM. Its regression test failed
before the runner change and passed after it. This run uses the usual Metal
wrapper and passes the earlier texture-copy byte and pixel checks. The
[previous default-mode failure](../2026-11-16-gx-texture-copy/suite.json)
remains recorded.

This does not cover all callback replacement races, a pending-event sleep,
physical Wii behavior, or HBC. No production GX code changed. The runner stops
Dolphin after the summary, so `exit_code` is null. The stderr log includes
repeated partial-send and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-completion --timeout-seconds 180
```
