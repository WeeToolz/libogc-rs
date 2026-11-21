# Dolphin evidence report

Probe `wii_std_thread_probe` reported `pass`.
Passed checks 61. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-std-runtime::__wii_std_alloc | std ABI allocation honors alignment and releases the block | PASS | [log L97](wii_std_thread_probe/probe.log#L97), [page-003](wii_std_thread_probe/pages/page-003.png) |
| ogc-std-runtime::__wii_std_errno | std ABI errno returns the current errno value | PASS | [log L98](wii_std_thread_probe/probe.log#L98), [page-003](wii_std_thread_probe/pages/page-003.png) |
| ogc-std-runtime::__wii_std_tls_create | std ABI TLS key creates | PASS | [log L99](wii_std_thread_probe/probe.log#L99), [page-003](wii_std_thread_probe/pages/page-003.png) |
| ogc-std-runtime::__wii_std_tls_set | std ABI TLS set and get preserve the stored pointer | PASS | [log L100](wii_std_thread_probe/probe.log#L100), [page-003](wii_std_thread_probe/pages/page-003.png) |
| ogc-std-runtime::__wii_std_tls_destroy | std ABI TLS key destroys after use | PASS | [log L101](wii_std_thread_probe/probe.log#L101), [page-003](wii_std_thread_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
