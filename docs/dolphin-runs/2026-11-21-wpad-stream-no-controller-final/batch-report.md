# Dolphin evidence report

Probe `wpad_probe` reported `pass`.
Passed checks 77. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::send_stream_data | WPAD stream probe confirms empty channel | PASS | [log L118](wpad_probe/probe.log#L118), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::send_stream_data | WPAD empty channel speaker stream succeeds | PASS | [log L119](wpad_probe/probe.log#L119), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::send_stream_data | WPAD empty channel rejects malformed speaker stream arguments | PASS | [log L120](wpad_probe/probe.log#L120), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin has no connected Wii Remote. This probe checks a valid empty-channel no-op and Rust safety guards for malformed arguments.
