# Raw video APIs in Dolphin

The 11 new checks passed in Dolphin 2609a on macOS with OpenGL. They check
framebuffer size and guarded memory clearing, active framebuffer and scan
state, fixed timing values, and retrace callback delivery and restoration.
See the [probe log](video_probe/probe.log), [check screen](video_probe/pages/page-005.png),
[result](video_probe/result.json), [suite](suite.json), and
[identities](identities.json).

The full run failed: it reported 346 passes and three failures in existing GX
EFB depth and copy checks. The [unchanged-probe control](../2026-11-17-raw-video-control/video_probe/probe.log)
reported the same three failures and 335 passes under the same runner and
Dolphin configuration. Their cause is not established. The results support the
named raw-video checks, not a full video suite pass. The snapshot check only
covers its framebuffer fields; the snapshot API remains unverified. No
production code changed. Physical Wii behavior remains open. HBC was not
under test. The runner stops Dolphin after the summary, so its process exit
code is null. The NAND source identity was measured after the run, not from
the initial private copy.

Set the MP3 fixture variables described in `docs/dependencies.md`, then run:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe video_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-raw-video --timeout-seconds 240
```
