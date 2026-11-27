# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 792. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge null-call guard | PASS | [log L448](runtime_probe/probe.log#L448), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge copies ES title count to output vector | PASS | [log L453](runtime_probe/probe.log#L453), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge transfers title input and directory output | PASS | [log L459](runtime_probe/probe.log#L459), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync :i returns the ES title count | PASS | [log L455](runtime_probe/probe.log#L455), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync q:d formats title directory | PASS | [log L461](runtime_probe/probe.log#L461), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync qi:d decodes the overflow length | PASS | [log L468](runtime_probe/probe.log#L468), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public ABI async :i preserves output and callback | PASS | [log L456](runtime_probe/probe.log#L456), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public ABI async q:d spills and copies back before callback | PASS | [log L484](runtime_probe/probe.log#L484), [page-018](runtime_probe/pages/page-018.png) |

## Behavioral contracts

| Contract | Environment | Scope | Required checks | Result |
| --- | --- | --- | --- | --- |
| `ipc-format.internal.null-call-guard` | dolphin | partial | IPC format internal bridge null-call guard | PASS |
| `ipc-format.internal.scalar-output` | dolphin | partial | IPC format internal bridge copies ES title count to output vector | PASS |
| `ipc-format.internal.title-input-buffer-output` | dolphin | partial | IPC format internal bridge transfers title input and directory output | PASS |
| `ipc-format.sync.public-abi` | dolphin | partial | IPC format public ABI sync :i returns the ES title count, IPC format public ABI sync q:d formats title directory, IPC format public ABI sync qi:d decodes the overflow length | PASS |
| `ipc-format.sync.overflow-arguments` | dolphin | partial | IPC format public ABI sync qi:d decodes the overflow length | PASS |
| `ipc-format.async.public-abi` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.overflow-arguments` | dolphin | partial | IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.userdata` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.exactly-once` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.copyback-before-callback` | dolphin | partial | IPC format public ABI async q:d spills and copies back before callback | PASS |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin evidence only; no physical console cache-coherency claim.
- Selected scalar, q:d, qi:d, one sync stack length, one async pointer and length stack shape, and one callback completion. The full format grammar, rejection paths, allocation failures, concurrent requests, and DMA boundary matrix remain open.
- The ES descriptor close is fixture cleanup and is not formatter evidence.
