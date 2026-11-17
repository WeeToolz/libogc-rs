# TEV order automatic texture scale: investigation in progress

Three Dolphin 2609a Metal runs each reported 317 passes and one failure.
The probe loads an 8-by-4 texture at map 0, assigns it to coordinate 3 with
`set_tev_order`, and captures the commands before a zero-vertex primitive.
The C reference marks SU state dirty and emits BP words `28000058`, `36000007`,
and `37000003` before the primitive. The probe checks those words, padding,
and buffer guards.

The [trace before the change](../2026-11-16-gx-tev-order-trace/video_probe/probe.log)
contains `61 00 00 00 58 90 00 00`: register 00 and no scale words. The
[trace after the shadow change](video_probe/probe.log) contains
`61 28 00 00 58 90 00 00`: register 28, but still no scale words. See
[page 23](video_probe/pages/page-023.png), [result](video_probe/result.json),
and [identities](identities.json). The earlier [first run](../2026-11-16-gx-tev-order-before/video_probe/result.json)
reported the same failing check without byte tracing.

At this intermediate run, the local change preserved the BP register address
but did not implement automatic scale updates. See the [final report](../2026-11-16-gx-tev-order-final/README.md)
for the completed fix and passing checks. HBC and physical Wii were not under
test.
