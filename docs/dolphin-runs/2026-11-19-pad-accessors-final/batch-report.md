# Dolphin evidence report

Probe `input_probe` reported `pass`.
Passed checks 22. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input::buttons_down | PAD accessors match button transitions from raw samples | PASS | [log L131](input_probe/probe.log#L131), [page-002](input_probe/pages/page-002.png) |
| ogc-input::buttons_held | PAD accessors match button transitions from raw samples | PASS | [log L131](input_probe/probe.log#L131), [page-002](input_probe/pages/page-002.png) |
| ogc-input::buttons_up | PAD accessors match button transitions from raw samples | PASS | [log L131](input_probe/probe.log#L131), [page-002](input_probe/pages/page-002.png) |
| ogc-input::stick_x | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |
| ogc-input::stick_y | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |
| ogc-input::substick_x | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |
| ogc-input::substick_y | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |
| ogc-input::trigger_l | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |
| ogc-input::trigger_r | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin GameCube port 0 with scripted button and main-stick input. Substick and trigger checks cover observed neutral samples only; physical controllers are not covered.
