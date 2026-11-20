# Dolphin evidence report

Probe `input_probe` reported `pass`.
Passed checks 28. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-input-sys::pad_buttons_down | PAD accessors match button transitions from raw samples | PASS | [log L131](input_probe/probe.log#L131), [page-002](input_probe/pages/page-002.png) |
| ogc-input-sys::pad_buttons_held | PAD accessors match button transitions from raw samples | PASS | [log L131](input_probe/probe.log#L131), [page-002](input_probe/pages/page-002.png) |
| ogc-input-sys::pad_buttons_up | PAD accessors match button transitions from raw samples | PASS | [log L131](input_probe/probe.log#L131), [page-002](input_probe/pages/page-002.png) |
| ogc-input-sys::pad_stick_x | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |
| ogc-input-sys::pad_stick_y | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |
| ogc-input-sys::pad_sub_stick_x | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |
| ogc-input-sys::pad_sub_stick_y | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |
| ogc-input-sys::pad_trigger_l | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |
| ogc-input-sys::pad_trigger_r | PAD accessors match axes from raw samples | PASS | [log L132](input_probe/probe.log#L132), [page-002](input_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Scripted Dolphin channel zero input compared with raw scanned samples. Other channels, nonzero trigger and substick coverage, physical controllers, and transition races remain open.
