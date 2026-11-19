# Dolphin evidence report

Probe `input_probe` reported `pass`.
Passed checks 28. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-controller::scan | controller scan sees scripted pad | PASS | [log L133](input_probe/probe.log#L133), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::scan_at | controller timed scan sees scripted pad | PASS | [log L134](input_probe/probe.log#L134), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::scan_with_pad_status | controller scan returns matching pad status | PASS | [log L135](input_probe/probe.log#L135), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::scan_with_raw | controller raw scan returns matching pad status | PASS | [log L136](input_probe/probe.log#L136), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::scan_with_raw_at | controller timed raw scan returns matching pad status | PASS | [log L137](input_probe/probe.log#L137), [page-002](input_probe/pages/page-002.png) |
| ogc-controller::set_input_scan_hook | controller scan hook observes PAD boundaries | PASS | [log L138](input_probe/probe.log#L138), [page-002](input_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Scripted GameCube port 0 connection and scan hook boundaries; timestamp contents, other sources and physical controllers are not covered.
