# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 629. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| lwbt-sys::disconnect | Bluetooth disconnect rejects null PCB | PASS | [log L229](runtime_probe/probe.log#L229), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::listenasync | Bluetooth listen rejects null PCB | PASS | [log L230](runtime_probe/probe.log#L230), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::connectasync | Bluetooth connect rejects null PCB | PASS | [log L231](runtime_probe/probe.log#L231), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::listenasync_step2 | Bluetooth listen step rejects null PCB | PASS | [log L232](runtime_probe/probe.log#L232), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::connectasync_step2 | Bluetooth connect step rejects null PCB | PASS | [log L233](runtime_probe/probe.log#L233), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::senddata | Bluetooth send data rejects null PCB | PASS | [log L234](runtime_probe/probe.log#L234), [page-009](runtime_probe/pages/page-009.png) |
| lwbt-sys::sendmessage | Bluetooth send message rejects null PCB | PASS | [log L235](runtime_probe/probe.log#L235), [page-009](runtime_probe/pages/page-009.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
