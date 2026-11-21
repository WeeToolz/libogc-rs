# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 739. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi-sys::attach | raw EXI attach rejects an invalid channel | PASS | [log L453](runtime_probe/probe.log#L453), [page-017](runtime_probe/pages/page-017.png) |
| ogc-exi-sys::detach | raw EXI detach rejects an invalid channel | PASS | [log L454](runtime_probe/probe.log#L454), [page-017](runtime_probe/pages/page-017.png) |
| ogc-exi-sys::get_id | raw EXI device ID query rejects an invalid channel without changing output | PASS | [log L455](runtime_probe/probe.log#L455), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
