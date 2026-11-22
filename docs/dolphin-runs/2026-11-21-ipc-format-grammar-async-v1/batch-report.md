# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 814. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge null-call guard | PASS | [log L448](runtime_probe/probe.log#L448), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge copies ES title count to output vector | PASS | [log L471](runtime_probe/probe.log#L471), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge transfers title input and directory output | PASS | [log L477](runtime_probe/probe.log#L477), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync :i returns the ES title count | PASS | [log L473](runtime_probe/probe.log#L473), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync q:d formats title directory | PASS | [log L478](runtime_probe/probe.log#L478), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync qi:d decodes the overflow length | PASS | [log L485](runtime_probe/probe.log#L485), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync b/h/i/q decodes register and aligned overflow words | PASS | [log L449](runtime_probe/probe.log#L449), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync :b/:h/:i/:q constructs exact scalar widths | PASS | [log L451](runtime_probe/probe.log#L451), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync ds constructs input buffer and string vectors | PASS | [log L455](runtime_probe/probe.log#L455), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync :d constructs a direct output buffer | PASS | [log L457](runtime_probe/probe.log#L457), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync d:d separates input and output vectors | PASS | [log L461](runtime_probe/probe.log#L461), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync accepts empty normal and maximum length strings | PASS | [log L463](runtime_probe/probe.log#L463), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync rejects unknown malformed overlong and unterminated input | PASS | [log L464](runtime_probe/probe.log#L464), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync output copyback preserves DMA boundary canaries | PASS | [log L458](runtime_probe/probe.log#L458), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync allocation failures release partial formatter state | PASS | [log L505](runtime_probe/probe.log#L505), [page-019](runtime_probe/pages/page-019.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format formatter recovers after injected allocation failures | PASS | [log L507](runtime_probe/probe.log#L507), [page-019](runtime_probe/pages/page-019.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync scalar output copyback preserves width canaries | PASS | [log L453](runtime_probe/probe.log#L453), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync rejects a null scalar output pointer | PASS | [log L465](runtime_probe/probe.log#L465), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync rejects an unavailable heap | PASS | [log L466](runtime_probe/probe.log#L466), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public ABI async :i preserves output and callback | PASS | [log L474](runtime_probe/probe.log#L474), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public ABI async q:d spills and copies back before callback | PASS | [log L502](runtime_probe/probe.log#L502), [page-019](runtime_probe/pages/page-019.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async b/h/i/q decodes register and aligned overflow words | PASS | [log L450](runtime_probe/probe.log#L450), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async :b/:h/:i/:q constructs exact scalar widths | PASS | [log L452](runtime_probe/probe.log#L452), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async d:d separates input and overflow output arguments | PASS | [log L462](runtime_probe/probe.log#L462), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async entry staging cleanup preserves DMA boundary canaries | PASS | [log L459](runtime_probe/probe.log#L459), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async concurrent scalar and buffer requests keep callbacks separate | PASS | [log L503](runtime_probe/probe.log#L503), [page-019](runtime_probe/pages/page-019.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async request allocation failure releases state without callback | PASS | [log L506](runtime_probe/probe.log#L506), [page-019](runtime_probe/pages/page-019.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async entry scalar cleanup preserves width canaries | PASS | [log L454](runtime_probe/probe.log#L454), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async invalid descriptor completes once with error and releases state | PASS | [log L504](runtime_probe/probe.log#L504), [page-019](runtime_probe/pages/page-019.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async ds decodes input buffer and stack string | PASS | [log L456](runtime_probe/probe.log#L456), [page-017](runtime_probe/pages/page-017.png) |

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
| `ipc-format.sync.cleanup-on-error` | dolphin | partial | IPC format public sync allocation failures release partial formatter state | PASS |
| `ipc-format.recovery-after-failure` | dolphin | partial | IPC format formatter recovers after injected allocation failures | PASS |
| `ipc-format.sync.scalar-copyback-width` | dolphin | partial | IPC format public sync scalar output copyback preserves width canaries | PASS |
| `ipc-format.sync.input-buffer` | dolphin | partial | IPC format public sync ds constructs input buffer and string vectors | PASS |
| `ipc-format.sync.input-string` | dolphin | partial | IPC format public sync ds constructs input buffer and string vectors | PASS |
| `ipc-format.sync.null-output-pointer` | dolphin | partial | IPC format public sync rejects a null scalar output pointer | PASS |
| `ipc-format.sync.invalid-heap` | dolphin | partial | IPC format public sync rejects an unavailable heap | PASS |
| `ipc-format.async.public-abi` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.overflow-arguments` | dolphin | partial | IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.userdata` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.exactly-once` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.copyback-before-callback` | dolphin | partial | IPC format public ABI async q:d spills and copies back before callback | PASS |
| `ipc-format.async.input-scalars` | dolphin | partial | IPC format public async b/h/i/q decodes register and aligned overflow words | PASS |
| `ipc-format.async.output-scalars` | dolphin | partial | IPC format public async :b/:h/:i/:q constructs exact scalar widths | PASS |
| `ipc-format.async.mixed-vectors` | dolphin | partial | IPC format public async d:d separates input and overflow output arguments | PASS |
| `ipc-format.async.dma-output-copyback` | dolphin | partial | IPC format public async entry staging cleanup preserves DMA boundary canaries | PASS |
| `ipc-format.async.concurrent-requests` | dolphin | partial | IPC format public async concurrent scalar and buffer requests keep callbacks separate | PASS |
| `ipc-format.async.cleanup-on-error` | dolphin | partial | IPC format public async request allocation failure releases state without callback | PASS |
| `ipc-format.async.scalar-cleanup-width` | dolphin | partial | IPC format public async entry scalar cleanup preserves width canaries | PASS |
| `ipc-format.async.error-completion` | dolphin | partial | IPC format public async invalid descriptor completes once with error and releases state | PASS |
| `ipc-format.async.input-buffer` | dolphin | partial | IPC format public async ds decodes input buffer and stack string | PASS |
| `ipc-format.async.input-string` | dolphin | partial | IPC format public async ds decodes input buffer and stack string | PASS |
| `ipc-format.async.output-buffer` | dolphin | partial | IPC format public async entry staging cleanup preserves DMA boundary canaries | PASS |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- Dolphin evidence only; no physical console cache-coherency claim.
- The ES descriptor close is fixture cleanup and is not formatter evidence.
- Output boundary checks simulate IOS writes to the formatted vector and check software copyback plus adjacent canaries. They do not prove Dolphin or physical hardware DMA/cache behavior.
- Deterministic allocation faults count probe-only formatter allocations. They check formatter ownership accounting and recovery, but do not force every IOS completion failure.
- Scalar copyback checks simulate IOS writes. The async-entry interception runs cleanup before request submission, so it does not prove asynchronous callback behavior.
- The async error case uses an invalid descriptor to request one IOS error completion. Other IOS error classes remain open.
- Both async requests must still be pending after the second submission. The probe checks their separate callback userdata, outputs, and exactly-once completion.
