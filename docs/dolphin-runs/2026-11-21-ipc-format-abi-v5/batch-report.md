# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 790. Failed checks 2.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge null-call guard | PASS | [log L448](runtime_probe/probe.log#L448), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge copies ES title count to output vector | PASS | [log L453](runtime_probe/probe.log#L453), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge transfers title input and directory output | PASS | [log L459](runtime_probe/probe.log#L459), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format public ABI sync :i returns the ES title count | PASS | [log L455](runtime_probe/probe.log#L455), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format public ABI async :i preserves output and callback | PASS | [log L456](runtime_probe/probe.log#L456), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format public ABI sync q:d formats title directory | PASS | [log L461](runtime_probe/probe.log#L461), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format public ABI sync qi:d decodes the overflow length | FAIL | [log L468](runtime_probe/probe.log#L468), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format public ABI async q:d spills and copies back before callback | FAIL | [log L476](runtime_probe/probe.log#L476), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC fixture closes ES descriptor | PASS | [log L477](runtime_probe/probe.log#L477), [page-018](runtime_probe/pages/page-018.png) |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-ipc::__ogc_ipc_ioctlv_format_bridge — IPC format public ABI sync qi:d decodes the overflow length (FAIL)
- ogc-ipc::__ogc_ipc_ioctlv_format_bridge — IPC format public ABI async q:d spills and copies back before callback (FAIL)

## Coverage limits

- Dolphin evidence only; no physical console cache-coherency claim.
- Covers the public synchronous and asynchronous symbols, selected q:d and qi:d forms, and one callback path. The full formatter grammar, rejection paths, allocation failures, concurrency, and DMA boundary matrix remain open.
- IPC descriptor close is fixture cleanup and does not count as formatter behavior.
