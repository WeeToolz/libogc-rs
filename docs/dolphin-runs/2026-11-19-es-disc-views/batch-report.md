# Dolphin evidence report

Probe `general_probe` reported `pass`.
Passed checks 69. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-es-sys::di_get_tmd_view_size | ES disc TMD view size matches system menu | PASS | [log L89](general_probe/probe.log#L89), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::di_get_tmd_view_size | ES disc view size reports no active title | PASS | [log L91](general_probe/probe.log#L91), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::di_get_tmd_view | ES disc TMD view matches menu and rejects null | PASS | [log L90](general_probe/probe.log#L90), [page-003](general_probe/pages/page-003.png) |
| ogc-es-sys::get_ticket_view | ES disc ticket view reports no active title | PASS | [log L92](general_probe/probe.log#L92), [page-003](general_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Uses a system-menu TMD from the imported Dolphin NAND. Ticket view covers the no-active-title error path. Disc media and physical hardware remain untested.
