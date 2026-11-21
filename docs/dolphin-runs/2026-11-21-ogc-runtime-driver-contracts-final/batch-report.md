# Dolphin evidence report

Probe `lwp_probe` reported `pass`.
Passed checks 270. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-runtime::block_count | runtime BlockDevice reports independent size and count | PASS | [log L84](lwp_probe/probe.log#L84), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::block_size | runtime BlockDevice reports independent size and count | PASS | [log L84](lwp_probe/probe.log#L84), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::cancel | runtime AsyncOperation cancel changes the mock completion | PASS | [log L89](lwp_probe/probe.log#L89), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::flush | runtime BlockDevice flush reports completion | PASS | [log L87](lwp_probe/probe.log#L87), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::initialize | runtime Lifecycle state and initialize return expected states | PASS | [log L82](lwp_probe/probe.log#L82), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::open | runtime DebugTransport opens and transfers independent bytes | PASS | [log L90](lwp_probe/probe.log#L90), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::read | runtime DebugTransport opens and transfers independent bytes | PASS | [log L90](lwp_probe/probe.log#L90), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::read_blocks | runtime BlockDevice reads the selected block and rejects invalid lengths | PASS | [log L86](lwp_probe/probe.log#L86), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::receive_report | runtime HidTransport receives the expected report | PASS | [log L93](lwp_probe/probe.log#L93), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::send_report | runtime HidTransport sends the requested report | PASS | [log L92](lwp_probe/probe.log#L92), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::shutdown | runtime Lifecycle shutdown returns to uninitialized state | PASS | [log L83](lwp_probe/probe.log#L83), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::state | runtime Lifecycle state and initialize return expected states | PASS | [log L82](lwp_probe/probe.log#L82), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::write | runtime DebugTransport opens and transfers independent bytes | PASS | [log L90](lwp_probe/probe.log#L90), [page-003](lwp_probe/pages/page-003.png) |
| ogc-runtime::write_blocks | runtime BlockDevice writes a block and rejects out-of-range writes | PASS | [log L85](lwp_probe/probe.log#L85), [page-003](lwp_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin exercises each public trait method through deterministic mock implementations. It does not verify a concrete hardware driver implementation.
