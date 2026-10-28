# All-crate C/Rust parity scan — 2026-10-05

Scope: all 83 workspace crates listed in `config/static-review-scope.csv`. This scan reconciles each crate against its current source-review note and the mapped libogc/libmad C implementation. Observable behavior and public ABI must match C. Rust architecture may differ. A behavior difference is accepted only when it is a deliberate safety improvement; accepted safety deviations are identified explicitly and do not count as parity blockers.

This is a static source-review/status reconciliation. `reviewed` is a parity result, not a synonym for “looked at.” A crate is `reviewed` only when parity is established for the required scope, with any accepted difference limited to a documented safety improvement. Open work is split by reason instead of using the former generic status.

## Status model

- `reviewed`: parity is established for the required scope; documented safety-only deviations are allowed.
- `parity_blocked`: one or more confirmed non-safety behavior or ABI mismatches remain. This takes precedence over re-review or incomplete-scope work.
- `needs_re_review`: no confirmed blocker is recorded, but a fix or bounded change still needs independent parity verification.
- `review_incomplete`: no confirmed blocker is recorded, but the required source comparison or validation scope is incomplete. This does not itself assert a mismatch.

`work_status` is separate. It records current activity (`idle`, `in_review`, `finding`, `fix_in_progress`, `re_review`, or `doc_update`). For example, a `parity_blocked` crate can have `work_status=re_review` while one bounded fix is being checked and other confirmed blockers remain.

## Result

- Workspace crates checked: **83**.
- `reviewed`: **49**.
- `parity_blocked`: **20**.
- `needs_re_review`: **14**.
- `review_incomplete`: **0**.
- No open row uses a generic catch-all status. Unfinished scope is not recorded as a confirmed finding unless the evidence names a mismatch.

## Parity-blocked crates

| Crate | Work status | Current result / remaining work |
| --- | --- | --- |

| `tuxedo-sys` | `finding` | One confirmed blocker remains: `KThread.ctx` is layout-correct and receives alignment-safe saved-context mirrors, but the scheduler restores from private context storage rather than treating the caller-owned public field as authoritative. A caller edit to suspended `KThread.ctx` can therefore differ from C. `_reent`, intrusive queues/synchronization, fixed-map removal, and header helpers are implemented. |

| `aesnd` | `finding` | Confirmed observable parity blockers: Rust CPU mixing changes C DSP request/refill timing, callback cadence, buffering/process metrics, and exact sample timing. Earlier silence/default-frequency/format findings are fixed. Implementation and validation are still required. |

| `aesnd-sys` | `finding` | Remaining confirmed parity scope is exact staged-stream/PB cursor semantics and DSP process-metric equivalence. Audio-callback order, two-buffer stream notification lookahead, voice-order scheduling, and per-voice saturation are now aligned at source level. Earlier silent-playback and frequency findings are fixed. |



| `libdb-sys` | `finding` | Confirmed parity blockers: debugger trap/exception-handler behavior is absent; TCP initialization uses the platform socket stack instead of C uIP/BBA address setup; write-error behavior differs. Export/error-map fixes passed. USB/debug transport paths still require source-scope completion. |

| `mad-sys` | `finding` | Confirmed parity blockers remain in decoder callback/control-flow semantics, frame/header data exposed to callbacks/filters, synthesis options/output, lifecycle/state ownership, version metadata, and codec algorithm fidelity. stream_skip accumulation and bounded ABI/error fixes passed. |

| `ogc-dvd` | `finding` | Confirmed remaining parity areas: C retry/remount behavior, chunk/progress semantics, and callback state/error ordering differ or are not yet reconciled. Earlier BS recovery fix passed. No unnamed mismatch is asserted. |

| `ogc-dvd-sys` | `finding` | Grouped DVD sys layer remains parity-blocked by named retry/remount, chunk/progress, callback-state, and error-order parity work. Partial-progress fix passed. |

| `ogc-es` | `finding` | The bare-name and duplicate-registration fixes passed independent review (worker_f fixes, worker_e reviewer; commits 9289c94/a6019c0, review notes ed3ec4e/f78d6c3). Broader ES initialization and ioctl result behavior remains open for source review. |

| `ogc-es-sys` | `finding` | Grouped with ogc-es. Both devoptab name and duplicate-registration fixes passed independent review. Broader ES sys/API return and initialization behavior remains open. |


| `ogc-gcsd` | `finding` | Confirmed remaining parity blockers: initialization default state and missing C helper API surface. Token-timeout drain, write-protect transfer bytes, CRC delay, lock waiting, command encoding, and related protocol fixes passed. |



| `ogc-ios` | `re_review` | Shared STM lifecycle fix is present and awaits independent review. Remaining confirmed IOS parity blocker: ACR interrupt restore/unmask ordering around IPC reinitialize/version query differs from C; extra version-poll cache invalidation remains behaviorally uncertain. |

| `ogc-ios-sys` | `re_review` | Grouped with ogc-ios. Shared STM/event-hook lifecycle is fixed and awaits review. ACR restore/unmask ordering around IPC reinitialize/version query remains a confirmed source-order difference; cache invalidation effect remains uncertain. |

| `ogc-ipc` | `re_review` | Confirmed remaining parity blockers: Rust staging-heap capacity differs from C and variadic IOS_IoctlvFormat-style API coverage is incomplete. C-sized heap block allocation is fixed and awaits review; fragmentation edge cases remain review scope. |




| `ogc-network` | `finding` | Confirmed remaining parity scope includes asynchronous initialization/lifecycle sequencing and residual API error/return behavior. Deinit status, host-IP descriptor/retry/yield behavior, initial status, retry boundary, and bind negative-result fixes passed. No unnamed mismatch is asserted. |

| `ogc-network-sys` | `finding` | Grouped with ogc-network. Async initialization/lifecycle and residual API error/return behavior remain the named parity scope; recent descriptor/status/retry fixes passed. |

| `ogc-usb` | `finding` | Confirmed remaining parity scope includes USB event/callback lifecycle/order and remaining host/HID API behavior outside accepted v5-open and descriptor fixes. Where teardown adds safety notifications, classify them explicitly as safety deviations rather than parity. No unnamed mismatch is asserted. |

| `ogc-usb-sys` | `finding` | Grouped USB sys layer remains parity-blocked by event/callback lifecycle/order and residual host/HID behavior. HID descriptor conversion and fixed-input probe fixes passed; no unnamed mismatch is asserted. |

| `ogc-usbstorage` | `finding` | The Inquiry retry and nullable ReadCapacity fixes passed bounded independent review (worker_e implementation, worker_f reviewer; commits 3e4ac90 and e094ea5). Other confirmed close/suspend/reset/shutdown error-result and async transfer lifetime findings remain open. |

| `ogc-usbstorage-sys` | `finding` | The Inquiry retry and nullable ReadCapacity fixes passed bounded independent review (worker_e implementation, worker_f reviewer; commits 3e4ac90 and e094ea5). Other confirmed error masking and async transfer lifetime findings remain open. |

| `tinysmb-sys` | `finding` | Confirmed parity blockers: AddDevice/devoptab callback dispatch and close-name normalization differ from C; broader SMB protocol behavior remains review scope. Global duplicate-name FindDevice behavior is fixed. |




## Crates needing re-review

| Crate | Work status | Current result / remaining work |
| --- | --- | --- |
| `ogc-lwp` | `re_review` | No confirmed non-safety blocker remains in the recorded scope. Dynamic thread/mailbox/timer storage, intrusive waiters, C-ABI callbacks, and legacy C exports are implemented. Independent PowerPC/runtime re-review is required. |
| `ogc-lwp-driver` | `re_review` | No confirmed non-safety blocker remains in the recorded driver scope. Dynamic thread pages, intrusive waiters, dynamic timer/mailbox storage, C-ABI callbacks, caller-owned Tuxedo `_reent`, and alignment-safe saved-context mirroring are implemented. Independent scheduler/runtime re-review is required. |
| `ogc-lwp-sys` | `re_review` | No confirmed non-safety blocker remains in the recorded sys scope. The checked 30 `LWP_*` and 5 `MQ_*` C symbols are Rust-owned delegates with no libogc C linkage. Independent target re-review is required. |
| `ogc-gx-sys` | `re_review` | No confirmed non-safety blocker remains in the recorded sys scope. All public `GX_*` macros from `gx.h` are present, exact C typedef names share the verified Rust layouts, and `wgPipe` is exposed without unresolved C symbols. Independent target re-review is required. |
| `ogc-gx` | `re_review` | No confirmed non-safety blocker remains in the recorded scope. The public function/signature surface is complete, `GX_Init` now runs the full libogc default-state sequence, and display-list capture exposes/restores C-compatible temporary CPU-FIFO state. Independent PowerPC/runtime GX re-review is required. |
| `asnd-sys` | `re_review` | No confirmed non-safety blocker remains in the recorded ASND scope. The CPU mixer now uses complete voice passes, callback-before-pass ordering, and per-voice 16-bit saturation. Independent PowerPC/runtime audio re-review is required. |
| `iso9660` | `re_review` | No confirmed non-safety blocker remains in the recorded ISO9660 scope. Path-table parents are validated against already-defined entries and directory contents are captured at diropen. Independent target/media re-review is required. |

| `lwip-sys` | `re_review` | No confirmed non-safety blocker remains in the recorded scope. Socket-option parity is fixed, and receive now matches C's 32 KiB chunking and later EOF/error/overflow precedence. Null destination rejection remains an accepted safety deviation. Independent PowerPC/runtime re-review is required. |

| `lwbt-sys` | `re_review` | Async connection/listen fix 176b8ea is implemented: entry points return after submission and a Rust worker delivers completion callbacks after transport setup. The prior MTU, framing, capacity, and disconnect fixes remain in place. Independent parity re-review and hardware/runtime callback validation are required. |

| `ogc-fat` | `re_review` | No confirmed non-safety blocker remains in the recorded libfat scope. FAT12/FAT16, 512-4096 byte logical sectors, volume labels, DOS attributes, devoptab callbacks, errno mapping, FSInfo/cache/flush/media lifecycle, mount APIs, partition discovery, and stat/statvfs behavior are ported or reconciled. Live-handle unmount refusal, malformed-root rejection, and loud sync/flush failures are accepted safety deviations. Independent media re-review is required. |

| `ogc-alloc` | `re_review` | Arena2 boundary handoff is implemented without libogc C linkage: initialize_wii publishes through its required setter callback, and ogc-system-sys now exports Rust-owned SYS_Get/SetArena2* ABI symbols backed by the same boundary state. Independent re-review is required. |

| `ogc-system-sys` | `re_review` | Wii-only Arena2 API gating and Rust-owned SYS_Get/SetArena2* C ABI exports are implemented. Rust and linked newlib callers share the same IRQ-guarded boundary state without unresolved libogc C symbols. Independent re-review is required. |

| `wpad-sys` | `re_review` | WPAD_CHAN_ALL dispatch and SetIdleThresholds now match C, including disabled-state handling, button normalization, and direct signed non-button threshold storage. The calibration over-read correction remains an accepted bounds-safety deviation. Runtime controller validation is still required. |

| `wiikeyboard-sys` | `re_review` | Final stdin/public-FIFO competition and successful-open LED flash ordering fixes are present and await independent re-review. No confirmed non-safety blocker remains in the recorded scope. |


## Crates with incomplete review

| Crate | Work status | Current result / remaining work |
| --- | --- | --- |


## Reviewed crates

| Crate | Work status | Current result / remaining work |
| --- | --- | --- |

| `abi-probe` | `idle` | No declared layout mismatch found; C compiler size/offset comparison remains unverified; see note. |

| `gc-binary` | `idle` | Rust-only checked binary helpers; primitive endian/alignment behavior agrees where comparable; no public C counterpart. |

| `gc-elf` | `idle` | Independent re-review by reviewer_f accepts the LegacyElf2Dol BSS accumulation fix; earlier tail and empty-image fixes also passed. Host tests passed. |

| `modplay` | `idle` | Full current-source reconciliation closes the recorded ModPlay parity scope. C worker scheduling, silent initial buffers, fixed 5760-byte DMA transfers, shutdown/join ordering, callbacks, parser/effect behavior, and ownership are represented; Rust-only timing diagnostics are not C public API. Hardware playback remains validation, not a parity blocker. |

| `modplay-capi` | `idle` | Grouped with modplay. Current C API/ABI and worker/mixer scheduling scope is reconciled; no confirmed non-safety mismatch remains. Hardware playback remains separate validation. |

| `mp3player-sys` | `idle` | Independent re-review by reviewer_d accepts player fixes in default AUDIO and sndlib paths; MAD codec fidelity is a separate documented scope. |

| `ogc` | `idle` | Reviewer_f passed the facade API additions, reviewer_e found no additional include omissions, and reviewer_b passed GameCube IOS/IPC/system startup/reset gates; no confirmed umbrella mismatch remains. |

| `ogc-aes-sys` | `idle` | Static review recorded in the linked note. |

| `ogc-aram` | `idle` | Independent re-review by reviewer_f accepts worker_b reset/re-initialization fix 20ec977; defensive failure-path differences remain documented. |

| `ogc-audio` | `idle` | Static review recorded in the linked note. |

| `ogc-audio-sys` | `idle` | Static review recorded in the linked note. |

| `ogc-bt-sync` | `idle` | Current HCI/L2CAP source review reconciles the recorded disconnect, CID, queue, controller-capacity, configuration-continuation, and callback behavior. No confirmed non-safety source mismatch remains; controller/IOS timing remains hardware validation. |

| `ogc-cache` | `idle` | reviewer_d independently passed bd602f7: L2Enhance and helper APIs are Wii-only as in C; no other recorded cache mismatch remains. Target test code compiled, not run. |

| `ogc-card` | `idle` | Independent re-review confirms repeated CARD_Init preserves identifiers and mounted-card state. |

| `ogc-card-metadata` | `idle` | Grouped CARD metadata crate; no metadata implementation changes in the fix pass. |

| `ogc-card-sys` | `idle` | Independent re-review confirms repeated CARD_Init preserves identifiers and mounted-card state. |

| `ogc-conf` | `idle` | Current source reconciliation confirms the SCv0 save, embedded-NUL parsing, CR-first terminator precedence, and negative read/write error mappings now match C. Malformed-input bounds checks are safety behavior. No confirmed non-safety mismatch remains. |

| `ogc-conf-sys` | `idle` | Constants/layout layer grouped with ogc-conf. No separate sys-layer mismatch is confirmed after the completed CONF source review. |

| `ogc-core` | `idle` | Static review recorded in the linked note. |

| `ogc-di` | `idle` | Independent re-review passed the recorded alignment errno, direct async IOS submission, legacy exports, and result-order fixes. Unsupported callbacks/writes remain documented safety deviations. PowerPC test code compiled; tests and runtime callback behavior were not executed. |

| `ogc-dsp` | `idle` | Independent re-review complete; task volatility and layout fixes match C. No hardware verification. |

| `ogc-dsp-sys` | `idle` | Independent re-review complete; task volatility and layout fixes match C. Target tests compiled but did not run. |

| `ogc-exi` | `idle` | reviewer_f verified debounce commit 00a3df5 matches C's 20µs insertion delay; earlier libexi-probe-audit conclusion after b9f5d7b is stale. No hardware verification. |

| `ogc-exi-sys` | `idle` | Grouped EXI: 00a3df5 adds C-matching per-platform 20µs debounce; threshold tests exist but were not run in this audit. |

| `ogc-filesystem` | `idle` | Full recorded ISFS review is closed. Negative raw-descriptor close behavior matches C. Deinitialize preserves a real IOS close failure instead of hiding it as C does; that difference is an accepted cleanup-error safety deviation. No confirmed non-safety mismatch remains. |
| `ogc-filesystem-sys` | `idle` | DiscInterface layout and ISO path constant mapped to C declarations; no mismatch confirmed. |

| `ogc-gu` | `idle` | Static review recorded in the linked note. |

| `ogc-gu-sys` | `idle` | Static review recorded in the linked note. |

| `ogc-input` | `idle` | worker_a commit 0fd06ca passes reviewer_e: failed SI register/unregister still stores the requested callback and returns the old one; lock prevents an IRQ ordering gap. Rust-only callback invocation is documented. |

| `ogc-input-sys` | `idle` | Independent re-review passes 0fd06ca against PAD_SetSamplingCallback; failed register/removal state and previous-callback result match C. Tests compiled but did not run. |

| `ogc-irq` | `idle` | Reviewer_e passed commits a573618 and be72ab9 against C __wii__ ACR visibility, IRQ table sizes, mask encoding, and pending-cause handling; target tests could not compile. |

| `ogc-irq-sys` | `idle` | Grouped with ogc-irq; reviewer_e passed ACR ID count, masks, pending-cause code, and target cfg parity; target tests could not compile. |

| `ogc-runtime` | `idle` | Reviewer_f PASSED root fix 99c2d1a: ThreadId excludes both zero and C LWP_THREAD_NULL; no additional mismatch in recorded runtime scope. Tests compiled but did not run. |

| `ogc-sha` | `idle` | Static comparison found no confirmed digest or context mismatch; software backend and typed alignment are documented differences. |

| `ogc-si` | `idle` | Re-review complete against SI C APIs and implementation; no additional confirmed mismatch found. |

| `ogc-si-sys` | `idle` | Re-review complete against SI C APIs and implementation; no additional confirmed mismatch found. |

| `ogc-stm` | `idle` | STM source parity is established for its own public scope: Wii-only availability, repeated init, close result flow, lifecycle and event-hook behavior are reconciled. Stricter open/hook failure reporting and leak-avoidance cleanup are accepted safety deviations. The separate GameCube ogc-ipc dependency build issue is not an STM parity blocker. |

| `ogc-sys` | `idle` | Low-level ogc-sys ABI/source scope has no separate confirmed non-safety mismatch. IPC variadic/staging-heap blockers belong to ogc-ipc; Rust-owned runtime/assembly symbols are internal implementations, not unresolved libogc C dependencies. |

| `ogc-sysbase` | `idle` | Current ABI/devoptab source review is reconciled: Rust owns AddDevice/FindDevice/RemoveDevice/GetDeviceOpTab/setDefaultDevice/devoptab_list, uses the 35-slot table, and current target type widths/layouts match the checked devkitPPC ABI. Out-of-range default-device rejection is a safety deviation. |

| `ogc-system` | `idle` | Startup/reset/arena/SRAM/alarm source review has no confirmed non-safety mismatch. Wii-only Arena2 publication is provided by the Rust-owned ABI boundary in ogc-system-sys; allocation-failure resilience and input checks are accepted safety behavior. |

| `ogc-usb-events` | `idle` | Grouped with ogc-usb; no separate event-crate change was needed. Mouse cleanup fix passed independent review in 0a225c6. |

| `ogc-usbgecko` | `idle` | Current source and recorded Dolphin regression reconcile command sequencing, callback-backed EXI wait/wakeup/retry behavior, receive zeroing, flush signature, range and delay behavior. No confirmed non-safety mismatch remains; real EXI contention on hardware is separate validation. |

| `ogc-usbstorage-lifecycle` | `idle` | reviewer_a passed worker_b close cleanup fix 6b95b4b; close cleanup matches C; lifecycle API and documented flush addition remain scoped. |

| `ogc-video` | `idle` | Independent re-review by worker_b accepts fix 76d00e4; implemented by /root/dvd_review. |

| `ogc-video-sys` | `idle` | The recorded VIDEO sys/API review is complete: timing/register/framebuffer behavior and standard mode constants are reconciled, including the independently reviewed non-interlace VI origin fix. No confirmed non-safety mismatch remains. |

| `ogc-wd` | `idle` | Reviewer_d passed c964188; no other confirmed mismatch in the recorded WD scope. Intentional safety differences and timing limits are documented. |

| `ogc-wiilaunch` | `idle` | Static comparison found no confirmed unintended mismatch in launch-state layouts or serialized behavior. |

| `ogc-wiilaunch-state` | `idle` | Grouped with ogc-wiilaunch; stricter Rust persistence and error checks are documented. |

| `ogc-wiisd` | `idle` | Static review recorded in the linked note. |


## Closure rule

A crate can move to `reviewed` only when the required source-review scope establishes C-equivalent observable behavior and ABI, except for explicitly documented safety improvements. Architecture-only differences are acceptable when they do not change observable behavior. Independent re-review must pass after a parity fix before closure. Hardware acceptance remains a separate gate where the crate note requires it.
