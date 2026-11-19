# Dolphin evidence report

Probe `pad_state_probe` reported `pass`.
Passed checks 61. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input::recalibrate | mask-channel-0 | PASS | [log L114](pad_state_probe/probe.log#L114), [page-005](pad_state_probe/pages/page-005.png) |
| ogc-input::recalibrate | mask-channel-1 | PASS | [log L132](pad_state_probe/probe.log#L132), [page-006](pad_state_probe/pages/page-006.png) |
| ogc-input::recalibrate | mask-channel-2 | PASS | [log L151](pad_state_probe/probe.log#L151), [page-008](pad_state_probe/pages/page-008.png) |
| ogc-input::recalibrate | mask-channel-3 | PASS | [log L169](pad_state_probe/probe.log#L169), [page-009](pad_state_probe/pages/page-009.png) |
| ogc-input::recalibrate | mask-all | PASS | [log L197](pad_state_probe/probe.log#L197), [page-011](pad_state_probe/pages/page-011.png) |
| ogc-input::recalibrate | mask-mixed-0-2 | PASS | [log L218](pad_state_probe/probe.log#L218), [page-012](pad_state_probe/pages/page-012.png) |
| ogc-input::recalibrate | invalid-mask-filter | PASS | [log L240](pad_state_probe/probe.log#L240), [page-014](pad_state_probe/pages/page-014.png) |
| ogc-input::recalibrate | return-busy-read-lifecycle | PASS | [log L259](pad_state_probe/probe.log#L259), [page-015](pad_state_probe/pages/page-015.png) |
| ogc-input::reset | transfer-submission-failure-recovery | PASS | [log L321](pad_state_probe/probe.log#L321), [page-020](pad_state_probe/pages/page-020.png) |
| ogc-input::sync | return-busy-read-lifecycle | PASS | [log L259](pad_state_probe/probe.log#L259), [page-015](pad_state_probe/pages/page-015.png) |
| ogc-input::read | return-busy-read-lifecycle | PASS | [log L259](pad_state_probe/probe.log#L259), [page-015](pad_state_probe/pages/page-015.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Deterministic SI fault fixtures exercise reset and recalibration scheduling and a busy read; physical controllers and normal input samples are not covered.
