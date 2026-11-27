# Dolphin evidence report

Probe `wpad_probe` reported `probe_fail`.
Passed checks 76. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| wpad-sys::send_stream_data | WPAD stream probe confirms empty channel | PASS | [log L118](wpad_probe/probe.log#L118), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::send_stream_data | WPAD empty channel speaker stream succeeds | FAIL | [log L119](wpad_probe/probe.log#L119), [page-004](wpad_probe/pages/page-004.png) |
| wpad-sys::send_stream_data | WPAD empty channel rejects malformed speaker stream arguments | PASS | [log L120](wpad_probe/probe.log#L120), [page-004](wpad_probe/pages/page-004.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- wpad-sys::send_stream_data — WPAD empty channel speaker stream succeeds (FAIL)

## Coverage limits

- Dolphin has no connected Wii Remote. This probe checks a valid empty-channel no-op and Rust safety guards for malformed arguments.
