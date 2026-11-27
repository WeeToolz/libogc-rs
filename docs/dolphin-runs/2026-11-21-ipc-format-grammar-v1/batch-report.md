# Dolphin evidence report

Probe `runtime_probe` reported `probe_fail`.
Passed checks 801. Failed checks 1.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge null-call guard | PASS | [log L448](runtime_probe/probe.log#L448), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge copies ES title count to output vector | PASS | [log L464](runtime_probe/probe.log#L464), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge transfers title input and directory output | PASS | [log L470](runtime_probe/probe.log#L470), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync :i returns the ES title count | PASS | [log L466](runtime_probe/probe.log#L466), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync q:d formats title directory | PASS | [log L471](runtime_probe/probe.log#L471), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync qi:d decodes the overflow length | PASS | [log L478](runtime_probe/probe.log#L478), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync b/h/i/q decodes register and aligned overflow words | PASS | [log L449](runtime_probe/probe.log#L449), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync :b/:h/:i/:q constructs exact scalar widths | PASS | [log L451](runtime_probe/probe.log#L451), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync d:s constructs input buffer and string vectors | FAIL | [log L453](runtime_probe/probe.log#L453), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync :d constructs a direct output buffer | PASS | [log L454](runtime_probe/probe.log#L454), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync d:d separates input and output vectors | PASS | [log L455](runtime_probe/probe.log#L455), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync accepts empty normal and maximum length strings | PASS | [log L457](runtime_probe/probe.log#L457), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync rejects unknown malformed overlong and unterminated input | PASS | [log L458](runtime_probe/probe.log#L458), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public ABI async :i preserves output and callback | PASS | [log L467](runtime_probe/probe.log#L467), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public ABI async q:d spills and copies back before callback | PASS | [log L495](runtime_probe/probe.log#L495), [page-019](runtime_probe/pages/page-019.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async b/h/i/q decodes register and aligned overflow words | PASS | [log L450](runtime_probe/probe.log#L450), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async :b/:h/:i/:q constructs exact scalar widths | PASS | [log L452](runtime_probe/probe.log#L452), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async d:d separates input and overflow output arguments | PASS | [log L456](runtime_probe/probe.log#L456), [page-017](runtime_probe/pages/page-017.png) |

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
| `ipc-format.sync.input-string-and-buffer` | dolphin | partial | IPC format public sync d:s constructs input buffer and string vectors | FAIL or missing |
| `ipc-format.sync.output-buffer` | dolphin | partial | IPC format public sync :d constructs a direct output buffer | PASS |
| `ipc-format.sync.mixed-vectors` | dolphin | partial | IPC format public sync d:d separates input and output vectors | PASS |
| `ipc-format.sync.string-limits` | dolphin | partial | IPC format public sync accepts empty normal and maximum length strings | PASS |
| `ipc-format.sync.malformed-format` | dolphin | partial | IPC format public sync rejects unknown malformed overlong and unterminated input | PASS |
| `ipc-format.async.public-abi` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.overflow-arguments` | dolphin | partial | IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.userdata` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.exactly-once` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.copyback-before-callback` | dolphin | partial | IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.input-scalars` | dolphin | partial | IPC format public async b/h/i/q decodes register and aligned overflow words | PASS |
| `ipc-format.async.output-scalars` | dolphin | partial | IPC format public async :b/:h/:i/:q constructs exact scalar widths | PASS |
| `ipc-format.async.mixed-vectors` | dolphin | partial | IPC format public async d:d separates input and overflow output arguments | PASS |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-ipc::IOS_IoctlvFormat — IPC format public sync d:s constructs input buffer and string vectors (FAIL)

## Coverage limits

- Dolphin evidence only; no physical console cache-coherency claim.
- Selected scalar, q:d, qi:d, one sync stack length, one async pointer and length stack shape, and one callback completion. The full format grammar, rejection paths, allocation failures, concurrent requests, and DMA boundary matrix remain open.
- The ES descriptor close is fixture cleanup and is not formatter evidence.
