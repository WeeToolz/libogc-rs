# IPC formatter evidence status

Generated from `config/ipc-format-contracts.csv` by `cargo xtask inventory`. Status describes the stated contract evidence, not the implementation API classification.

## Status counts

verified: 63 | partial: 2 | blocked: 8 | unverified: 0

`unverified` is the actionable count. `blocked` records requirements that need unavailable prerequisites.

## Contracts

| Contract | API | Status | Remaining evidence |
| --- | --- | --- | --- |
| `ipc-format.async.aes-live-input` | `IOS_IoctlvFormatAsync` | `blocked` | Blocked on physical Wii validation of Broadway cache behavior and IOS DMA coherency; Dolphin functional evidence passes. |
| `ipc-format.async.callback-less` | `IOS_IoctlvFormatAsync` | `blocked` | Blocked on physical Wii validation of Broadway cache behavior and IOS DMA coherency; Dolphin functional evidence passes. |
| `ipc-format.async.callback-reentry` | `IOS_IoctlvFormatAsync` | `verified` | Dolphin verifies callback-originated resubmission, distinct userdata and output, exactly-once callbacks, and balanced request, heap, and workspace ledgers. |
| `ipc-format.async.cleanup-on-error` | `IOS_IoctlvFormatAsync` | `verified` | Request allocation failure before submission; IOS completion failures remain open |
| `ipc-format.async.cleanup-on-error` | `IOS_IoctlvFormatAsync` | `verified` | Request allocation failure before submission; IOS completion failures remain open |
| `ipc-format.async.concurrent-lwps` | `IOS_IoctlvFormatAsync` | `verified` | Dolphin proves formatter construction overlaps on two LWPs and each submits separate IOS state; physical scheduling is outside this stated contract. |
| `ipc-format.async.concurrent-requests` | `IOS_IoctlvFormatAsync` | `verified` | Two IOS completions with distinct userdata and destinations in one run |
| `ipc-format.async.copyback-before-callback` | `IOS_IoctlvFormatAsync` | `verified` | Callback observes one staged output buffer |
| `ipc-format.async.dma-input-boundaries` | `IOS_IoctlvFormatAsync` | `verified` | Hashes every logical input byte and checks actual workspace alloc/free events before IOS submission |
| `ipc-format.async.dma-output-copyback` | `IOS_IoctlvFormatAsync` | `partial` | Async entry ABI decoding and formatter cleanup only; no request submission, callback, or DMA coherency claim |
| `ipc-format.async.dma-staging-allocation` | `IOS_IoctlvFormatAsync` | `verified` | The stated async d-vector staging allocation and release sizes pass in Dolphin; physical cache behavior is outside this allocation contract. |
| `ipc-format.async.error-completion` | `IOS_IoctlvFormatAsync` | `verified` | Invalid descriptor tests one IOS error completion and cleanup path only |
| `ipc-format.async.exactly-once` | `IOS_IoctlvFormatAsync` | `verified` | Exactly-once observed in one completion window |
| `ipc-format.async.exactly-once` | `IOS_IoctlvFormatAsync` | `verified` | Exactly-once observed in one completion window |
| `ipc-format.async.input-buffer` | `IOS_IoctlvFormatAsync` | `verified` | One 4-byte input buffer is staged in a mixed request |
| `ipc-format.async.input-scalars` | `IOS_IoctlvFormatAsync` | `verified` | The named formatter forms and cases in this contract pass in Dolphin; other forms have separate contracts. |
| `ipc-format.async.input-string` | `IOS_IoctlvFormatAsync` | `verified` | One input string on the async variadic overflow area |
| `ipc-format.async.ios-output-boundaries` | `IOS_IoctlvFormatAsync` | `blocked` | Blocked on physical Wii validation of Broadway cache behavior and IOS DMA coherency; Dolphin functional evidence passes. |
| `ipc-format.async.live-input-buffer` | `IOS_IoctlvFormatAsync` | `blocked` | Blocked on physical Wii validation of Broadway cache behavior and IOS DMA coherency; Dolphin functional evidence passes. |
| `ipc-format.async.mixed-vectors` | `IOS_IoctlvFormatAsync` | `verified` | The named formatter forms and cases in this contract pass in Dolphin; other forms have separate contracts. |
| `ipc-format.async.output-buffer` | `IOS_IoctlvFormatAsync` | `verified` | Direct and staged output branches pass async entry interception with cleanup checks; this contract does not claim IOS DMA coherency. |
| `ipc-format.async.output-scalars` | `IOS_IoctlvFormatAsync` | `verified` | The named formatter forms and cases in this contract pass in Dolphin; other forms have separate contracts. |
| `ipc-format.async.overflow-arguments` | `IOS_IoctlvFormatAsync` | `verified` | One stack-passed pointer and length shape only |
| `ipc-format.async.parser-boundaries` | `IOS_IoctlvFormatAsync` | `verified` | Parser interception covers async boundaries without IOS service calls |
| `ipc-format.async.public-abi` | `IOS_IoctlvFormatAsync` | `verified` | One output scalar shape only |
| `ipc-format.async.public-abi` | `IOS_IoctlvFormatAsync` | `verified` | One buffer output shape only |
| `ipc-format.async.request-heap-exhaustion` | `IOS_IoctlvFormatAsync` | `verified` | Exhausts real 4096-byte IOS request heap then releases every held record |
| `ipc-format.async.scalar-cleanup-width` | `IOS_IoctlvFormatAsync` | `verified` | Async entry interception checks formatter cleanup before request submission |
| `ipc-format.async.submission-failure` | `IOS_IoctlvFormatAsync` | `verified` | Probe-injected request submission failure before the request becomes pending |
| `ipc-format.async.submission-failure` | `IOS_IoctlvFormatAsync` | `verified` | Probe-injected request submission failure before the request becomes pending |
| `ipc-format.async.userdata` | `IOS_IoctlvFormatAsync` | `verified` | One sentinel pointer in one completion |
| `ipc-format.async.userdata` | `IOS_IoctlvFormatAsync` | `verified` | One sentinel pointer in one completion |
| `ipc-format.heap.successful-parse` | `IOS_IoctlvFormat` | `verified` | The stated successful parse fixture balances independent Argv and Scalar allocator events. |
| `ipc-format.internal.null-call-guard` | `__ogc_ipc_ioctlv_format_bridge` | `verified` | Null-call defensive guard only |
| `ipc-format.internal.scalar-output` | `__ogc_ipc_ioctlv_format_bridge` | `verified` | One scalar output shape only |
| `ipc-format.internal.title-input-buffer-output` | `__ogc_ipc_ioctlv_format_bridge` | `verified` | One 64-bit input and one output buffer shape only |
| `ipc-format.recovery-after-failure` | `IOS_IoctlvFormat` | `verified` | One intercepted sync scalar request after deterministic allocation failures |
| `ipc-format.sync.aes-live-input` | `IOS_IoctlvFormat` | `blocked` | Blocked on physical Wii validation of Broadway cache behavior and IOS DMA coherency; Dolphin functional evidence passes. |
| `ipc-format.sync.cleanup-on-error` | `IOS_IoctlvFormat` | `verified` | Real State and StagingBuffer workspace exhaustion plus Argv and Scalar heap exhaustion release formatter state with balanced independent ledgers. |
| `ipc-format.sync.dma-input-boundaries` | `IOS_IoctlvFormat` | `verified` | Hashes every logical input byte and checks actual workspace alloc/free events; no IOS submission |
| `ipc-format.sync.dma-output-copyback` | `IOS_IoctlvFormat` | `partial` | Software copyback and canaries only; no Dolphin or hardware DMA coherency claim |
| `ipc-format.sync.dma-staging-allocation` | `IOS_IoctlvFormat` | `verified` | The stated d-vector staging allocation and release sizes pass in Dolphin; physical cache behavior is outside this allocation contract. |
| `ipc-format.sync.heap-failure-paths` | `IOS_IoctlvFormat` | `verified` | Probe role fault and real workspace exhaustion both leave the formatter heap ledger empty and free every held workspace block once. |
| `ipc-format.sync.heap-failure-paths` | `IOS_IoctlvFormat` | `verified` | Role-based fault and real allocator exhaustion both leave the independent formatter heap ledger empty. |
| `ipc-format.sync.heap-failure-paths` | `IOS_IoctlvFormat` | `verified` | Role-based fault and real allocator exhaustion both release the first Scalar exactly once. |
| `ipc-format.sync.heap-failure-paths` | `IOS_IoctlvFormat` | `verified` | Injected and real workspace exhaustion preserve exact Argv cleanup and balance workspace and formatter heap ledgers. |
| `ipc-format.sync.heap-real-exhaustion` | `IOS_IoctlvFormat` | `verified` | Dolphin verifies natural Argv and Scalar allocator failure, exact frees, a zero formatter heap ledger, and recovery on the same heap. |
| `ipc-format.sync.input-buffer` | `IOS_IoctlvFormat` | `verified` | One 4-byte input buffer is staged in a mixed request |
| `ipc-format.sync.input-scalars` | `IOS_IoctlvFormat` | `verified` | The named formatter forms and cases in this contract pass in Dolphin; other forms have separate contracts. |
| `ipc-format.sync.input-string` | `IOS_IoctlvFormat` | `verified` | One input string vector in a mixed request |
| `ipc-format.sync.input-string-and-buffer` | `IOS_IoctlvFormat` | `verified` | The named formatter forms and cases in this contract pass in Dolphin; other forms have separate contracts. |
| `ipc-format.sync.invalid-heap` | `IOS_IoctlvFormat` | `verified` | One unavailable heap identifier is rejected |
| `ipc-format.sync.ios-output-boundaries` | `IOS_IoctlvFormat` | `blocked` | Blocked on physical Wii validation of Broadway cache behavior and IOS DMA coherency; Dolphin functional evidence passes. |
| `ipc-format.sync.large-live-input` | `IOS_IoctlvFormat` | `blocked` | Blocked on physical Wii validation of Broadway cache behavior and IOS DMA coherency; Dolphin functional evidence passes. |
| `ipc-format.sync.live-input-buffer` | `IOS_IoctlvFormat` | `blocked` | Blocked on physical Wii validation of Broadway cache behavior and IOS DMA coherency; Dolphin functional evidence passes. |
| `ipc-format.sync.malformed-format` | `IOS_IoctlvFormat` | `verified` | The named formatter forms and cases in this contract pass in Dolphin; other forms have separate contracts. |
| `ipc-format.sync.maximum-format` | `IOS_IoctlvFormat` | `verified` | 31 scalar input specifiers exercise maximum accepted format and owned-entry capacity |
| `ipc-format.sync.maximum-ownership-workspace` | `IOS_IoctlvFormat` | `verified` | 31 staged d parameters exercise the maximum formatter count; all workspace allocation/free events are recorded and balanced. |
| `ipc-format.sync.mixed-vectors` | `IOS_IoctlvFormat` | `verified` | The named formatter forms and cases in this contract pass in Dolphin; other forms have separate contracts. |
| `ipc-format.sync.null-buffer` | `IOS_IoctlvFormat` | `verified` | Records upstream-compatible behavior: pass a null d pointer with the supplied length, including nonzero lengths |
| `ipc-format.sync.null-format` | `IOS_IoctlvFormat` | `verified` | Public entry reaches parser null-format guard |
| `ipc-format.sync.null-output-pointer` | `IOS_IoctlvFormat` | `verified` | One null scalar output is rejected before IOS submission |
| `ipc-format.sync.output-buffer` | `IOS_IoctlvFormat` | `verified` | One 8-byte output uses staging because its length is not cache-line-sized |
| `ipc-format.sync.output-scalars` | `IOS_IoctlvFormat` | `verified` | The named formatter forms and cases in this contract pass in Dolphin; other forms have separate contracts. |
| `ipc-format.sync.overflow-arguments` | `IOS_IoctlvFormat` | `verified` | One stack-passed length only |
| `ipc-format.sync.public-abi` | `IOS_IoctlvFormat` | `verified` | One output scalar shape only |
| `ipc-format.sync.public-abi` | `IOS_IoctlvFormat` | `verified` | One 64-bit input and output buffer shape only |
| `ipc-format.sync.public-abi` | `IOS_IoctlvFormat` | `verified` | One valid ES overflow request only |
| `ipc-format.sync.q-register-alignment` | `IOS_IoctlvFormat` | `verified` | Compiler-generated call covers a skipped register while q remains in GPRs |
| `ipc-format.sync.q-register-to-stack` | `IOS_IoctlvFormat` | `verified` | Compiler-generated call covers q alignment in registers followed by an overflow q |
| `ipc-format.sync.scalar-copyback-width` | `IOS_IoctlvFormat` | `verified` | Probe-simulated IOS output checks the formatter copy length for each scalar width |
| `ipc-format.sync.string-limits` | `IOS_IoctlvFormat` | `verified` | The named formatter forms and cases in this contract pass in Dolphin; other forms have separate contracts. |
| `ipc-format.sync.zero-vectors` | `IOS_IoctlvFormat` | `verified` | Intercept verifies zero input and output vectors without IOS submission |
