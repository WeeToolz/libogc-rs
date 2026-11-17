# TEV order and automatic texture scale in Dolphin

Dolphin 2609a on macOS with Metal passed 322 checks with zero failures. All
27 captured pages were valid. See the [probe log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), [identities](identities.json),
and [pages 23](video_probe/pages/page-023.png) and [24](video_probe/pages/page-024.png).

The probe loads an 8-by-4 texture and checks `set_tev_order` command bytes
and automatic scale updates against the C reference. It also checks a second
map with wrap enabled, manual scaling, a null texture, and indirect order.
The capture checks padding and buffer guards. Display-list captures restore
the saved GX shadow state after each case. Existing rendered rectangle checks
also pass; they do not prove every texture configuration.

The [before trace](../2026-11-16-gx-tev-order-trace/video_probe/probe.log)
failed with register `00` and no automatic scale words. The
[intermediate trace](../2026-11-16-gx-tev-order-shadow/video_probe/probe.log)
uses the correct register `28` but still lacks the scale words. The fix retains
per-map size and wrap, marks automatic scale state dirty on texture load and
TEV order changes, flushes direct and indirect scale state, and saves it in
display-list shadows. Invalid direct texture maps are ignored for safety.
The [first passing run](../2026-11-16-gx-tev-order-auto-first/video_probe/result.json)
and [expanded run](../2026-11-16-gx-tev-order-expanded/video_probe/result.json)
are retained.

This is evidence for selected command inputs in Dolphin. Other combinations
and physical Wii behavior remain open. HBC was not under test. The runner
stops Dolphin after the summary, so `exit_code` is null. The stderr log has
repeated partial-send and alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
DOLPHIN_METAL_BIN=/Applications/Dolphin.app/Contents/MacOS/Dolphin cargo xtask dolphin-validate --no-build --probe video_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-16-gx-tev-order-final --timeout-seconds 180
```
