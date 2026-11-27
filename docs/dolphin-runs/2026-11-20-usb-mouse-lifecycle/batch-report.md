# Dolphin evidence report

Probe `runtime_probe` reported `timeout`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-usb::init | USB mouse initializes its worker | PASS | [log L321](runtime_probe/probe.log#L321) |
| ogc-usb::deinit | USB mouse deinitialization joins its worker and releases state | Missing |  |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-usb::deinit — USB mouse deinitialization joins its worker and releases state (Missing)

## Coverage limits

- Dolphin checks the no-device USB mouse worker lifecycle and explicit USB stack cleanup. It does not test a physical mouse or input reports.
