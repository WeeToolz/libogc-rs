# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 722. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwbt-sys::try_shutdown | lwBT shutdown releases a live PCB | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::init_core | lwBT core init completes with closed adapter error | PASS | [log L252](runtime_probe/probe.log#L252), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::apply_patch | lwBT patch completes with closed adapter error | PASS | [log L253](runtime_probe/probe.log#L253), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::init_sub | lwBT sub init completes with closed adapter error | PASS | [log L254](runtime_probe/probe.log#L254), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::read_bd_addr | lwBT address read preserves output on closed adapter | PASS | [log L255](runtime_probe/probe.log#L255), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::read_remote_name | lwBT name read preserves output on closed adapter | PASS | [log L256](runtime_probe/probe.log#L256), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::inquiry | lwBT inquiry completes with closed adapter error | PASS | [log L257](runtime_probe/probe.log#L257), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::periodic_inquiry | lwBT periodic inquiry rejects closed adapter | PASS | [log L258](runtime_probe/probe.log#L258), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::get_pair_mode | lwBT failed inquiry preserves normal pair mode | PASS | [log L259](runtime_probe/probe.log#L259), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::exit_periodic_inquiry | lwBT inquiry exit rejects closed adapter | PASS | [log L260](runtime_probe/probe.log#L260), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::set_evt_filter | lwBT event filter validates input before transport | PASS | [log L261](runtime_probe/probe.log#L261), [page-010](runtime_probe/pages/page-010.png) |
| lwbt-sys::sendmessageasync | lwBT asynchronous message rejects null PCB | PASS | [log L262](runtime_probe/probe.log#L262), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- This run checks shutdown with a live PCB, error completion with a closed adapter, and argument handling. It does not exercise Bluetooth radio traffic, pairing, vendor patch upload, or adapter startup.
