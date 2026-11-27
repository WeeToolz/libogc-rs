# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 804. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge null-call guard | PASS | [log L448](runtime_probe/probe.log#L448), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge copies ES title count to output vector | PASS | [log L466](runtime_probe/probe.log#L466), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge transfers title input and directory output | PASS | [log L472](runtime_probe/probe.log#L472), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync :i returns the ES title count | PASS | [log L468](runtime_probe/probe.log#L468), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync q:d formats title directory | PASS | [log L473](runtime_probe/probe.log#L473), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync qi:d decodes the overflow length | PASS | [log L480](runtime_probe/probe.log#L480), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync b/h/i/q decodes register and aligned overflow words | PASS | [log L449](runtime_probe/probe.log#L449), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync :b/:h/:i/:q constructs exact scalar widths | PASS | [log L451](runtime_probe/probe.log#L451), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync ds constructs input buffer and string vectors | PASS | [log L453](runtime_probe/probe.log#L453), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync :d constructs a direct output buffer | PASS | [log L454](runtime_probe/probe.log#L454), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync d:d separates input and output vectors | PASS | [log L457](runtime_probe/probe.log#L457), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync accepts empty normal and maximum length strings | PASS | [log L459](runtime_probe/probe.log#L459), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync rejects unknown malformed overlong and unterminated input | PASS | [log L461](runtime_probe/probe.log#L461), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync output copyback preserves DMA boundary canaries | PASS | [log L455](runtime_probe/probe.log#L455), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public ABI async :i preserves output and callback | PASS | [log L469](runtime_probe/probe.log#L469), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public ABI async q:d spills and copies back before callback | PASS | [log L497](runtime_probe/probe.log#L497), [page-019](runtime_probe/pages/page-019.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async b/h/i/q decodes register and aligned overflow words | PASS | [log L450](runtime_probe/probe.log#L450), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async :b/:h/:i/:q constructs exact scalar widths | PASS | [log L452](runtime_probe/probe.log#L452), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async d:d separates input and overflow output arguments | PASS | [log L458](runtime_probe/probe.log#L458), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async output copyback preserves DMA boundary canaries | PASS | [log L456](runtime_probe/probe.log#L456), [page-017](runtime_probe/pages/page-017.png) |

## Behavioral contracts

| Contract | Environment | Scope | Required checks | Result |
| --- | --- | --- | --- | --- |
| `ipc-format.internal.null-call-guard` | dolphin | partial | IPC format internal bridge null-call guard | PASS |
| `ipc-format.internal.scalar-output` | dolphin | partial | IPC format internal bridge copies ES title count to output vector | PASS |
| `ipc-format.internal.title-input-buffer-output` | dolphin | partial | IPC format internal bridge transfers title input and directory output | PASS |
| `ipc-format.sync.public-abi` | dolphin | partial | IPC format public ABI sync :i returns the ES title count, IPC format public ABI sync q:d formats title directory, IPC format public ABI sync qi:d decodes the overflow length | PASS |
| `ipc-format.sync.overflow-arguments` | dolphin | partial | IPC format public ABI sync qi:d decodes the overflow length | PASS |
| `ipc-format.sync.input-scalars` | dolphin | partial | IPC format public sync b/h/i/q decodes register and aligned overflow words | PASS |
| `ipc-format.sync.output-scalars` | dolphin | partial | IPC format public sync :b/:h/:i/:q constructs exact scalar widths | PASS |
| `ipc-format.sync.input-string-and-buffer` | dolphin | partial | IPC format public sync ds constructs input buffer and string vectors | PASS |
| `ipc-format.sync.output-buffer` | dolphin | partial | IPC format public sync :d constructs a direct output buffer | PASS |
| `ipc-format.sync.mixed-vectors` | dolphin | partial | IPC format public sync d:d separates input and output vectors | PASS |
| `ipc-format.sync.string-limits` | dolphin | partial | IPC format public sync accepts empty normal and maximum length strings | PASS |
| `ipc-format.sync.malformed-format` | dolphin | partial | IPC format public sync rejects unknown malformed overlong and unterminated input | PASS |
| `ipc-format.sync.dma-output-copyback` | dolphin | partial | IPC format public sync output copyback preserves DMA boundary canaries | PASS |
| `ipc-format.async.public-abi` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.overflow-arguments` | dolphin | partial | IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.userdata` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.exactly-once` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.copyback-before-callback` | dolphin | partial | IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.input-scalars` | dolphin | partial | IPC format public async b/h/i/q decodes register and aligned overflow words | PASS |
| `ipc-format.async.output-scalars` | dolphin | partial | IPC format public async :b/:h/:i/:q constructs exact scalar widths | PASS |
| `ipc-format.async.mixed-vectors` | dolphin | partial | IPC format public async d:d separates input and overflow output arguments | PASS |
| `ipc-format.async.dma-output-copyback` | dolphin | partial | IPC format public async output copyback preserves DMA boundary canaries | PASS |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin evidence only; no physical console cache-coherency claim.
- The ES descriptor close is fixture cleanup and is not formatter evidence.
- Output boundary checks simulate IOS writes to the formatted vector and check software copyback plus adjacent canaries. They do not prove Dolphin or physical hardware DMA/cache behavior.
