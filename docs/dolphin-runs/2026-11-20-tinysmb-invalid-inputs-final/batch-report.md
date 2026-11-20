# Dolphin evidence report

Probe `tinysmb_probe` reported `pass`.
Passed checks 19. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| tinysmb-sys::check_connection | TinySMB connection query rejects null name | PASS | [log L75](tinysmb_probe/probe.log#L75), [page-002](tinysmb_probe/pages/page-002.png) |
| tinysmb-sys::connect | TinySMB connect rejects missing credentials | PASS | [log L77](tinysmb_probe/probe.log#L77), [page-002](tinysmb_probe/pages/page-002.png) |
| tinysmb-sys::create_directory | TinySMB directory create rejects null path | PASS | [log L85](tinysmb_probe/probe.log#L85), [page-003](tinysmb_probe/pages/page-003.png) |
| tinysmb-sys::delete_directory | TinySMB directory delete rejects null path | PASS | [log L86](tinysmb_probe/probe.log#L86), [page-003](tinysmb_probe/pages/page-003.png) |
| tinysmb-sys::delete_file | TinySMB file delete rejects null path | PASS | [log L87](tinysmb_probe/probe.log#L87), [page-003](tinysmb_probe/pages/page-003.png) |
| tinysmb-sys::disk_information | TinySMB disk query rejects null output | PASS | [log L89](tinysmb_probe/probe.log#L89), [page-003](tinysmb_probe/pages/page-003.png) |
| tinysmb-sys::find_close | TinySMB search close rejects unknown handle | PASS | [log L83](tinysmb_probe/probe.log#L83), [page-003](tinysmb_probe/pages/page-003.png) |
| tinysmb-sys::find_first | TinySMB first search rejects null path | PASS | [log L80](tinysmb_probe/probe.log#L80), [page-002](tinysmb_probe/pages/page-002.png) |
| tinysmb-sys::find_next | TinySMB next search rejects unknown handle | PASS | [log L81](tinysmb_probe/probe.log#L81), [page-002](tinysmb_probe/pages/page-002.png) |
| tinysmb-sys::init | TinySMB init rejects missing credentials | PASS | [log L76](tinysmb_probe/probe.log#L76), [page-002](tinysmb_probe/pages/page-002.png) |
| tinysmb-sys::init_device | TinySMB device init rejects null name | PASS | [log L74](tinysmb_probe/probe.log#L74), [page-002](tinysmb_probe/pages/page-002.png) |
| tinysmb-sys::open_file | TinySMB file open rejects null path | PASS | [log L84](tinysmb_probe/probe.log#L84), [page-003](tinysmb_probe/pages/page-003.png) |
| tinysmb-sys::path_info | TinySMB path query rejects null path | PASS | [log L79](tinysmb_probe/probe.log#L79), [page-002](tinysmb_probe/pages/page-002.png) |
| tinysmb-sys::reconnect | TinySMB reconnect rejects an unknown handle | PASS | [log L78](tinysmb_probe/probe.log#L78), [page-002](tinysmb_probe/pages/page-002.png) |
| tinysmb-sys::rename | TinySMB rename rejects null path | PASS | [log L88](tinysmb_probe/probe.log#L88), [page-003](tinysmb_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- The selected calls use null paths, missing credentials, a zeroed unknown handle, or an unknown connection. Each returns before opening an SMB session. This run does not test file I/O because the unsafe API requires a live file handle. Successful SMB logins, directory operations, and server interoperability remain unverified.
