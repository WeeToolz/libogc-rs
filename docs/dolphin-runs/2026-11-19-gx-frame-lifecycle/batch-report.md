# Dolphin evidence report

Probe `video_probe` reported `probe_fail`.
Passed checks 358. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-gx::initialize_display | GX frame lifecycle initializes display | FAIL | [log L622](video_probe/probe.log#L622), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::present_frame_and_wait | GX frame lifecycle presents white EFB pixels | FAIL | [log L624](video_probe/probe.log#L624), [page-028](video_probe/pages/page-028.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-gx::initialize_display — GX frame lifecycle initializes display (FAIL)
- ogc-gx::present_frame_and_wait — GX frame lifecycle presents white EFB pixels (FAIL)

## Coverage limits

- Black initialization and a white EFB sample in one Dolphin display mode. Other colors, frame dimensions, asynchronous presentation, and physical Wii behavior remain open.
