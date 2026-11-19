# Dolphin evidence report

Probe `video_probe` reported `pass`.
Passed checks 366. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-gx::get_current_gx_thread | GX current thread setter returns previous owner | PASS | [log L630](video_probe/probe.log#L630), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::set_current_gx_thread | GX current thread setter returns previous owner | PASS | [log L630](video_probe/probe.log#L630), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::get_overflow_count | GX overflow counter reset returns prior idle count | PASS | [log L631](video_probe/probe.log#L631), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::reset_overflow_count | GX overflow counter reset returns prior idle count | PASS | [log L631](video_probe/probe.log#L631), [page-028](video_probe/pages/page-028.png) |
| ogc-gx::get_gp_status | GX status matches stable command processor register | PASS | [log L632](video_probe/probe.log#L632), [page-028](video_probe/pages/page-028.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- One Dolphin thread and an idle FIFO. Nonzero overflow, thread handoff under contention, active status transitions, and physical Wii behavior remain untested.
