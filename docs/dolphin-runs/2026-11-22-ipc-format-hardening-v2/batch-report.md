# Dolphin evidence report

Probe `runtime_probe` reported `crash`.
Passed checks None. Failed checks None.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge null-call guard | PASS | [log L448](runtime_probe/probe.log#L448), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge copies ES title count to output vector | PASS | [log L481](runtime_probe/probe.log#L481), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::__ogc_ipc_ioctlv_format_bridge | IPC format internal bridge transfers title input and directory output | PASS | [log L488](runtime_probe/probe.log#L488) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync :i returns the ES title count | PASS | [log L483](runtime_probe/probe.log#L483), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync q:d formats title directory | PASS | [log L489](runtime_probe/probe.log#L489) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public ABI sync qi:d decodes the overflow length | Missing |  |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync b/h/i/q decodes register and aligned overflow words | PASS | [log L449](runtime_probe/probe.log#L449), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync :b/:h/:i/:q constructs exact scalar widths | PASS | [log L453](runtime_probe/probe.log#L453), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync ds constructs input buffer and string vectors | PASS | [log L457](runtime_probe/probe.log#L457), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync :d stages an 8-byte output buffer | PASS | [log L459](runtime_probe/probe.log#L459), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync d:d separates input and output vectors | PASS | [log L467](runtime_probe/probe.log#L467), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync accepts empty normal and maximum length strings | PASS | [log L470](runtime_probe/probe.log#L470), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync rejects unknown malformed overlong and unterminated input | PASS | [log L474](runtime_probe/probe.log#L474), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync output copyback preserves DMA boundary canaries | PASS | [log L461](runtime_probe/probe.log#L461), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync allocation failures release partial formatter state | Missing |  |
| ogc-ipc::IOS_IoctlvFormat | IPC format formatter recovers after injected allocation failures | Missing |  |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync scalar output copyback preserves width canaries | PASS | [log L455](runtime_probe/probe.log#L455), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync rejects a null scalar output pointer | PASS | [log L475](runtime_probe/probe.log#L475), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync rejects an unavailable heap | PASS | [log L476](runtime_probe/probe.log#L476), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync input staging reads selected DMA boundary lengths | PASS | [log L463](runtime_probe/probe.log#L463), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync d vectors reserve full cache lines and keep logical lengths | PASS | [log L465](runtime_probe/probe.log#L465), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync iq skips one GPR before aligned q | PASS | [log L450](runtime_probe/probe.log#L450), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync iqq advances from aligned GPR q to stack q | PASS | [log L451](runtime_probe/probe.log#L451), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync d input reaches ES with the complete title ID | PASS | [log L494](runtime_probe/probe.log#L494) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync rejects a null format pointer | PASS | [log L471](runtime_probe/probe.log#L471), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync accepts empty and colon-only formats | PASS | [log L472](runtime_probe/probe.log#L472), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync accepts the maximum valid format length | PASS | [log L473](runtime_probe/probe.log#L473), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync preserves null d pointers and lengths | PASS | [log L469](runtime_probe/probe.log#L469), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormat | IPC format public sync ES output respects DMA boundary lengths and canaries | PASS | [log L490](runtime_probe/probe.log#L490) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public ABI async :i preserves output and callback | PASS | [log L485](runtime_probe/probe.log#L485) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public ABI async q:d spills and copies back before callback | Missing |  |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async b/h/i/q decodes register and aligned overflow words | PASS | [log L452](runtime_probe/probe.log#L452), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async :b/:h/:i/:q constructs exact scalar widths | PASS | [log L454](runtime_probe/probe.log#L454), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async d:d separates input and overflow output arguments | PASS | [log L468](runtime_probe/probe.log#L468), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async entry staging cleanup preserves DMA boundary canaries | PASS | [log L462](runtime_probe/probe.log#L462), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async concurrent scalar and buffer requests keep callbacks separate | Missing |  |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async request allocation failure releases state without callback | Missing |  |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async entry scalar cleanup preserves width canaries | PASS | [log L456](runtime_probe/probe.log#L456), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async invalid descriptor completes once with error and releases state | Missing |  |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async ds decodes input buffer and stack string | PASS | [log L458](runtime_probe/probe.log#L458), [page-017](runtime_probe/pages/page-017.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async entry reads selected DMA boundary lengths | PASS | [log L464](runtime_probe/probe.log#L464), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async d vectors reserve full cache lines and keep logical lengths | PASS | [log L466](runtime_probe/probe.log#L466), [page-018](runtime_probe/pages/page-018.png) |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async d input survives IOS completion | Missing |  |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async d input completes without a user callback | Missing |  |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async submission failure releases request and formatter state | Missing |  |
| ogc-ipc::IOS_IoctlvFormatAsync | IPC format public async ES output copyback preserves DMA boundary canaries | PASS | [log L491](runtime_probe/probe.log#L491) |

## Behavioral contracts

| Contract | Environment | Scope | Required checks | Result |
| --- | --- | --- | --- | --- |
| `ipc-format.internal.null-call-guard` | dolphin | partial | IPC format internal bridge null-call guard | PASS |
| `ipc-format.internal.scalar-output` | dolphin | partial | IPC format internal bridge copies ES title count to output vector | PASS |
| `ipc-format.internal.title-input-buffer-output` | dolphin | partial | IPC format internal bridge transfers title input and directory output | PASS |
| `ipc-format.sync.public-abi` | dolphin | partial | IPC format public ABI sync :i returns the ES title count, IPC format public ABI sync q:d formats title directory, IPC format public ABI sync qi:d decodes the overflow length | FAIL or missing |
| `ipc-format.sync.overflow-arguments` | dolphin | partial | IPC format public ABI sync qi:d decodes the overflow length | FAIL or missing |
| `ipc-format.sync.input-scalars` | dolphin | partial | IPC format public sync b/h/i/q decodes register and aligned overflow words | PASS |
| `ipc-format.sync.output-scalars` | dolphin | partial | IPC format public sync :b/:h/:i/:q constructs exact scalar widths | PASS |
| `ipc-format.sync.input-string-and-buffer` | dolphin | partial | IPC format public sync ds constructs input buffer and string vectors | PASS |
| `ipc-format.sync.output-buffer` | dolphin | partial | IPC format public sync :d stages an 8-byte output buffer | PASS |
| `ipc-format.sync.mixed-vectors` | dolphin | partial | IPC format public sync d:d separates input and output vectors | PASS |
| `ipc-format.sync.string-limits` | dolphin | partial | IPC format public sync accepts empty normal and maximum length strings | PASS |
| `ipc-format.sync.malformed-format` | dolphin | partial | IPC format public sync rejects unknown malformed overlong and unterminated input | PASS |
| `ipc-format.sync.dma-output-copyback` | dolphin | partial | IPC format public sync output copyback preserves DMA boundary canaries | PASS |
| `ipc-format.sync.cleanup-on-error` | dolphin | partial | IPC format public sync allocation failures release partial formatter state | FAIL or missing |
| `ipc-format.recovery-after-failure` | dolphin | partial | IPC format formatter recovers after injected allocation failures | FAIL or missing |
| `ipc-format.sync.scalar-copyback-width` | dolphin | partial | IPC format public sync scalar output copyback preserves width canaries | PASS |
| `ipc-format.sync.input-buffer` | dolphin | partial | IPC format public sync ds constructs input buffer and string vectors | PASS |
| `ipc-format.sync.input-string` | dolphin | partial | IPC format public sync ds constructs input buffer and string vectors | PASS |
| `ipc-format.sync.null-output-pointer` | dolphin | partial | IPC format public sync rejects a null scalar output pointer | PASS |
| `ipc-format.sync.invalid-heap` | dolphin | partial | IPC format public sync rejects an unavailable heap | PASS |
| `ipc-format.sync.dma-input-boundaries` | dolphin | partial | IPC format public sync input staging reads selected DMA boundary lengths | PASS |
| `ipc-format.sync.dma-staging-allocation` | dolphin | partial | IPC format public sync d vectors reserve full cache lines and keep logical lengths | PASS |
| `ipc-format.sync.q-register-alignment` | dolphin | partial | IPC format public sync iq skips one GPR before aligned q | PASS |
| `ipc-format.sync.q-register-to-stack` | dolphin | partial | IPC format public sync iqq advances from aligned GPR q to stack q | PASS |
| `ipc-format.sync.live-input-buffer` | dolphin | partial | IPC format public sync d input reaches ES with the complete title ID | PASS |
| `ipc-format.sync.null-format` | dolphin | partial | IPC format public sync rejects a null format pointer | PASS |
| `ipc-format.sync.zero-vectors` | dolphin | partial | IPC format public sync accepts empty and colon-only formats | PASS |
| `ipc-format.sync.maximum-format` | dolphin | partial | IPC format public sync accepts the maximum valid format length | PASS |
| `ipc-format.sync.null-buffer` | dolphin | partial | IPC format public sync preserves null d pointers and lengths | PASS |
| `ipc-format.sync.ios-output-boundaries` | dolphin | partial | IPC format public sync ES output respects DMA boundary lengths and canaries | PASS |
| `ipc-format.async.public-abi` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | FAIL or missing |
| `ipc-format.async.overflow-arguments` | dolphin | partial | IPC format public ABI async q:d spills and copies back before callback | FAIL or missing |
| `ipc-format.async.userdata` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | FAIL or missing |
| `ipc-format.async.exactly-once` | dolphin | partial | IPC format public ABI async :i preserves output and callback, IPC format public ABI async q:d spills and copies back before callback | FAIL or missing |
| `ipc-format.async.copyback-before-callback` | dolphin | partial | IPC format public ABI async q:d spills and copies back before callback | FAIL or missing |
| `ipc-format.async.input-scalars` | dolphin | partial | IPC format public async b/h/i/q decodes register and aligned overflow words | PASS |
| `ipc-format.async.output-scalars` | dolphin | partial | IPC format public async :b/:h/:i/:q constructs exact scalar widths | PASS |
| `ipc-format.async.mixed-vectors` | dolphin | partial | IPC format public async d:d separates input and overflow output arguments | PASS |
| `ipc-format.async.dma-output-copyback` | dolphin | partial | IPC format public async entry staging cleanup preserves DMA boundary canaries | PASS |
| `ipc-format.async.concurrent-requests` | dolphin | partial | IPC format public async concurrent scalar and buffer requests keep callbacks separate | FAIL or missing |
| `ipc-format.async.cleanup-on-error` | dolphin | partial | IPC format public async request allocation failure releases state without callback | FAIL or missing |
| `ipc-format.async.scalar-cleanup-width` | dolphin | partial | IPC format public async entry scalar cleanup preserves width canaries | PASS |
| `ipc-format.async.error-completion` | dolphin | partial | IPC format public async invalid descriptor completes once with error and releases state | FAIL or missing |
| `ipc-format.async.input-buffer` | dolphin | partial | IPC format public async ds decodes input buffer and stack string | PASS |
| `ipc-format.async.input-string` | dolphin | partial | IPC format public async ds decodes input buffer and stack string | PASS |
| `ipc-format.async.output-buffer` | dolphin | partial | IPC format public async entry staging cleanup preserves DMA boundary canaries | PASS |
| `ipc-format.async.dma-input-boundaries` | dolphin | partial | IPC format public async entry reads selected DMA boundary lengths | PASS |
| `ipc-format.async.dma-staging-allocation` | dolphin | partial | IPC format public async d vectors reserve full cache lines and keep logical lengths | PASS |
| `ipc-format.async.live-input-buffer` | dolphin | partial | IPC format public async d input survives IOS completion | FAIL or missing |
| `ipc-format.async.callback-less` | dolphin | partial | IPC format public async d input completes without a user callback | FAIL or missing |
| `ipc-format.async.submission-failure` | dolphin | partial | IPC format public async submission failure releases request and formatter state | FAIL or missing |
| `ipc-format.async.ios-output-boundaries` | dolphin | partial | IPC format public async ES output copyback preserves DMA boundary canaries | PASS |

## Evidence gaps

- The probe did not pass. See [suite result](suite.json).
- ogc-ipc::IOS_IoctlvFormat — IPC format public ABI sync qi:d decodes the overflow length (Missing)
- ogc-ipc::IOS_IoctlvFormat — IPC format public sync allocation failures release partial formatter state (Missing)
- ogc-ipc::IOS_IoctlvFormat — IPC format formatter recovers after injected allocation failures (Missing)
- ogc-ipc::IOS_IoctlvFormatAsync — IPC format public ABI async q:d spills and copies back before callback (Missing)
- ogc-ipc::IOS_IoctlvFormatAsync — IPC format public async concurrent scalar and buffer requests keep callbacks separate (Missing)
- ogc-ipc::IOS_IoctlvFormatAsync — IPC format public async request allocation failure releases state without callback (Missing)
- ogc-ipc::IOS_IoctlvFormatAsync — IPC format public async invalid descriptor completes once with error and releases state (Missing)
- ogc-ipc::IOS_IoctlvFormatAsync — IPC format public async d input survives IOS completion (Missing)
- ogc-ipc::IOS_IoctlvFormatAsync — IPC format public async d input completes without a user callback (Missing)
- ogc-ipc::IOS_IoctlvFormatAsync — IPC format public async submission failure releases request and formatter state (Missing)

## Coverage limits

- Dolphin evidence only; physical Wii cache and DMA behavior remain unverified.
- The internal bridge cases are compatibility checks, not public ABI evidence.
- Boundary interception hashes every logical input byte and checks the actual workspace allocator request and release sizes.
- Live ES d-input tests prove selected input bytes reach IOS and async ownership survives completion for this title fixture only.
- Null d tests record current parser behavior; upstream compatibility remains open.
- The complete runtime probe assertion count is broader than formatter coverage. Use scenario and contract tables for formatter evidence.
