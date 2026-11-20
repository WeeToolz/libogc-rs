# Dolphin evidence report

Probe `input_probe` reported `pass`.
Passed checks 35. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input-sys::pad_set_sampling_callback | PAD sampling callback replaces delivers and removes | PASS | [log L155](input_probe/probe.log#L155), [page-003](input_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin SI polling callback replacement and removal with one scripted controller. Timing under interrupt contention and physical Wii behavior remain open.
