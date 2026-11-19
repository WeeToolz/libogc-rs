# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 540. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::get_buffer_lo | IPC Rust buffer setters and getters update bounds | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::get_buffer_lo | IPC C buffer setters and getters update bounds | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::get_buffer_lo | IPC buffer bounds restored | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::get_buffer_hi | IPC Rust buffer setters and getters update bounds | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::get_buffer_hi | IPC C buffer setters and getters update bounds | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::get_buffer_hi | IPC buffer bounds restored | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::set_buffer_lo | IPC Rust buffer setters and getters update bounds | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::set_buffer_lo | IPC C buffer setters and getters update bounds | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::set_buffer_lo | IPC buffer bounds restored | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::set_buffer_hi | IPC Rust buffer setters and getters update bounds | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::set_buffer_hi | IPC C buffer setters and getters update bounds | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::set_buffer_hi | IPC buffer bounds restored | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_GetBufferLo | IPC Rust buffer setters and getters update bounds | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_GetBufferLo | IPC C buffer setters and getters update bounds | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_GetBufferLo | IPC buffer bounds restored | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_GetBufferHi | IPC Rust buffer setters and getters update bounds | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_GetBufferHi | IPC C buffer setters and getters update bounds | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_GetBufferHi | IPC buffer bounds restored | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_SetBufferLo | IPC Rust buffer setters and getters update bounds | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_SetBufferLo | IPC C buffer setters and getters update bounds | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_SetBufferLo | IPC buffer bounds restored | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_SetBufferHi | IPC Rust buffer setters and getters update bounds | PASS | [log L249](runtime_probe/probe.log#L249), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_SetBufferHi | IPC C buffer setters and getters update bounds | PASS | [log L250](runtime_probe/probe.log#L250), [page-010](runtime_probe/pages/page-010.png) |
| ogc-ipc::IPC_SetBufferHi | IPC buffer bounds restored | PASS | [log L251](runtime_probe/probe.log#L251), [page-010](runtime_probe/pages/page-010.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin IPC buffer bounds within the initial free range only. Heap allocations while bounds move, invalid boundaries, concurrency, and physical Wii remain unverified.
