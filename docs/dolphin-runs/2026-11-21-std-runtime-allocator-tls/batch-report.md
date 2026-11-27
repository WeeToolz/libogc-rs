# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 258. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-std-runtime::__wii_std_alloc | std ABI allocation honors alignment and releases the block | Missing |  |
| ogc-std-runtime::__wii_std_errno | std ABI errno returns the current errno value | Missing |  |
| ogc-std-runtime::__wii_std_tls_create | std ABI TLS key creates | Missing |  |
| ogc-std-runtime::__wii_std_tls_set | std ABI TLS set and get preserve the stored pointer | Missing |  |
| ogc-std-runtime::__wii_std_tls_destroy | std ABI TLS key destroys after use | Missing |  |

## Evidence gaps

- ogc-std-runtime::__wii_std_alloc — std ABI allocation honors alignment and releases the block (Missing)
- ogc-std-runtime::__wii_std_errno — std ABI errno returns the current errno value (Missing)
- ogc-std-runtime::__wii_std_tls_create — std ABI TLS key creates (Missing)
- ogc-std-runtime::__wii_std_tls_set — std ABI TLS set and get preserve the stored pointer (Missing)
- ogc-std-runtime::__wii_std_tls_destroy — std ABI TLS key destroys after use (Missing)
