# Dolphin evidence report

Probe `wii_std_fs_probe` reported `pass`.
Passed checks 101. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-std-runtime::__wii_std_output_stream | Wii std stream ABI writes and validates inputs | PASS | [log L73](wii_std_fs_probe/probe.log#L73), [page-002](wii_std_fs_probe/pages/page-002.png) |
| ogc-std-runtime::__wii_std_output_stream | Wii std routes stdout and stderr by stream | PASS | [log L72](wii_std_fs_probe/probe.log#L72), [page-002](wii_std_fs_probe/pages/page-002.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin UART and console routing. Physical UART and transport failure injection require separate evidence.
