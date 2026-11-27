# Dolphin evidence report

Probe `video_probe` reported `probe_fail`.
Passed checks 385. Failed checks 6.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-gx::abort_frame | GX abort frame empties pending GP FIFO work | PASS | [log L662](video_probe/probe.log#L662), [page-030](video_probe/pages/page-030.png) |
| ogc-gx::clear_gp_metric | GX GP metric clear resets both counters | FAIL | [log L659](video_probe/probe.log#L659), [page-030](video_probe/pages/page-030.png) |
| ogc-gx::disable_break_pt | GX breakpoint disable clears status | PASS | [log L650](video_probe/probe.log#L650), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::enable_break_pt | GX null breakpoint enable leaves status unchanged | PASS | [log L649](video_probe/probe.log#L649), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::init | GX init rejects null and misaligned FIFO storage | PASS | [log L648](video_probe/probe.log#L648), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::read_clks_per_vtx | GX clocks per vertex query returns its documented byte range | PASS | [log L660](video_probe/probe.log#L660), [page-030](video_probe/pages/page-030.png) |
| ogc-gx::redirect_write_gather_pipe | GX write gather redirect selects its aperture | PASS | [log L655](video_probe/probe.log#L655), [page-030](video_probe/pages/page-030.png) |
| ogc-gx::restore_write_gather_pipe | GX write gather restore keeps the CPU FIFO live | PASS | [log L656](video_probe/probe.log#L656), [page-030](video_probe/pages/page-030.png) |
| ogc-gx::set_cpu_fifo | GX CPU FIFO setter preserves the active FIFO | PASS | [log L652](video_probe/probe.log#L652), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::set_gp_fifo | GX GP FIFO setter preserves the active FIFO | PASS | [log L653](video_probe/probe.log#L653), [page-029](video_probe/pages/page-029.png) |
| ogc-gx::set_misc | GX misc DL save mode restores vertex descriptors | PASS | [log L657](video_probe/probe.log#L657), [page-030](video_probe/pages/page-030.png) |
| ogc-gx::set_misc | GX misc DL no-save mode keeps vertex descriptor changes | PASS | [log L658](video_probe/probe.log#L658), [page-030](video_probe/pages/page-030.png) |
| ogc-gx::set_v_cache_metric | GX vertex cache metric setter preserves the current counters | PASS | [log L661](video_probe/probe.log#L661), [page-030](video_probe/pages/page-030.png) |
| ogc-gx::tpl_open_tpl_from_file | GX TPL file open handles null inputs | PASS | [log L651](video_probe/probe.log#L651), [page-029](video_probe/pages/page-029.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-gx::clear_gp_metric — GX GP metric clear resets both counters (FAIL)
- Other failed check — GX EFB Z24 depths preserve X and Y coordinates
- Other failed check — GX EFB replacement preserves adjacent color and depth
- Other failed check — GX EFB copy writes expected tiled RGBA8 bytes
- Other failed check — GX frame lifecycle presents white EFB pixels
- Other failed check — GX frame progress and clearing copy
