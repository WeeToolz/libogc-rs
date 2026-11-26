# Static C parity review: EXI fix pass

Date: 2026-10-01

Status: All recorded EXI fixes passed independent static re-review. Hardware verification is outside scope.

## Scope

- `crates/hardware/ogc-exi/src/lib.rs`
- `crates/hardware/ogc-exi-sys/src/lib.rs`
- C references: `../../C-sources/libogc/libogc/exi.c` and `../../C-sources/libogc/gc/ogc/exi.h`

## Fixes

- **BUG FIX — SelectSD device ID validation and cache.** `ExiBus` now stores the last successful device 0 ID. `get_id` validates device presence, returns the cached ID when valid, and stores a new ID only after a successful transfer and a final presence check. `select_sd` calls `get_id` for unattached device 0 before selection. Presence removal or an external interrupt clears the cache. See `crates/hardware/ogc-exi-sys/src/lib.rs:66-88`, `:660-701`, `:756-786`, and `:1004-1071`.
- **BUG FIX — Attach requires a successful probe.** `attach` now calls `probe` before setting the attached state and checks presence again while it holds the EXI lock. See `crates/hardware/ogc-exi-sys/src/lib.rs:1074-1097`.
- **BUG FIX — Deselect checks device 0 after selection.** `deselect` checks device 0 presence after it clears selection and returns failure when the check fails. See `crates/hardware/ogc-exi-sys/src/lib.rs:792-822`.
- **BUG FIX — GameCube register base.** PowerPC EXI register access now selects `0xCC006800` when `sys_get_hollywood_revision()` identifies GameCube/DOL, and `0xCD006800` for Wii/RVL. See `crates/hardware/ogc-exi-sys/src/lib.rs:146-176`.
- **BUG FIX — Empty immediate transfer.** `imm_ex` returns 0 when `n_len` is zero, as C does. See `crates/hardware/ogc-exi-sys/src/lib.rs:939-965`.
- **BUG FIX — Initialization probe reset.** `init` now calls `probe_reset` after it releases the EXI state lock and installs handlers, matching C `__exi_init` startup probes and serial-port ID read. `probe_reset` clears both cached IDs before probing. See `crates/hardware/ogc-exi-sys/src/lib.rs:492-541`.

## Regression checks

Added source-level tests for the model-to-register-base choice, device presence and pending external interrupt rules, SelectSD ID-check conditions, zero-length `imm_ex`, and the Deselect result. The target permits compile checks, but it does not permit safe execution of the hardware-facing PowerPC tests in this host environment.

Commands run:

- `cargo fmt -p ogc-exi-sys`
- `cargo check -p ogc-exi-sys --target powerpc-unknown-linux-gnu --tests` — passed with one pre-existing function-item cast warning in a test helper.
- `cargo check -p ogc-exi --target powerpc-unknown-linux-gnu --tests` — passed. The previous check reported a temporary dead-code warning for the probe helper. The helper is now used.
- `git diff --check` — passed after the final source changes.
- `cargo xtask review-status` — blocked by an unrelated active ledger row at line 18 (`ogc-alloc` has no owner); the EXI rows are valid and already say `re_review`.

No hardware behavior was tested.

## Follow-up debounce fix

Implementer: `worker_c`. Independent reviewer: `/root/worker_a` (accepted).

Rust now keeps a per-channel first-detection timebase value for un-attached device 0 probes. It reports absence until 20 microseconds have elapsed, matching C's `diff_usec(last_exi_idtime[nChn], time) + 10 < 30` check. The threshold uses 40.5 MHz for GameCube and 60.75 MHz for Wii. A missing device or pending external interrupt resets the timestamp; the external interrupt handler and `probe_reset` also clear it. Attached devices skip the debounce, matching C's attached branch. Regression assertions cover each platform threshold and timebase wraparound.

Independent review compared the complete debounce change with C `__exi_probe` (`exi.c:164-207`) and its `EXI_ProbeReset` and external-interrupt paths (`exi.c:719-729, 821-832`). The Rust threshold is 810 GameCube timebase ticks and 1,215 Wii ticks, matching 20 microseconds at the respective 40.5 MHz and 60.75 MHz timebases. The C condition accepts at 20 microseconds because `PPCTicksToUs` truncates and C checks whether the result is below 20; Rust's rounded-up tick threshold has the same boundary. `wrapping_sub` handles timebase rollover. Rust resets the start time when the device is absent or an external interrupt is pending, in the external-interrupt handler, on `probe_reset`, and during `init` through `ExiBus::new`. Attached devices bypass the delay, as C does. No mismatch was found.

The regression tests cover both platform thresholds and rollover. Reset lifecycle paths were checked against C by source inspection; tests do not independently exercise each reset path. This handoff covers the probe insertion debounce only. Other EXI fixes and findings remain governed by their separate review records. No hardware verification is claimed.

## Validation boundary

The source changes and compile checks do not establish register, interrupt, hot-removal, or transfer behavior on hardware. All recorded changes passed independent static re-review; no hardware verification is claimed.

## Independent re-review — `/root/exi_independent_review`

Reviewer: `/root/exi_independent_review`

Implementer: `/root/di_review` (commit `b9f5d7b`)

Compared the changed EXI paths with `../../C-sources/libogc/libogc/exi.c` and `../../C-sources/libogc/gc/ogc/exi.h`. The register base selection matches C's GameCube/Wii compile-time split. The empty `EXI_ImmEx` result, SelectSD ID validation, attach probe requirement, deselect presence result, and initialization probe reset have corresponding changes. The startup sequence resets probe state, checks channels 0 and 1, then reads channel 0 device 2 as C does.

### Finding at the time of the initial independent review (resolved by the follow-up above)

**Confirmed parity gap — probe insertion debounce.** C `__exi_probe` (`exi.c:164-207`) records `last_exi_idtime` on first detection and returns absent until its elapsed-time calculation reaches the approximately 20 microsecond threshold. Rust `device_zero_probe_locked` (`crates/hardware/ogc-exi-sys/src/lib.rs:660-682`) reported present as soon as `EXI_EXT_BIT` was set and `EXI_EXT_IRQ` was clear. Worker_c implemented this missing debounce in commit `00a3df5`; worker_a independently compared it with C and accepted the fix. See “Follow-up debounce fix” above.

No source changes were made during this review. No tests were run; this was a static comparison.
# EXI_LockSync API parity follow-up — 2026-11-26

**Confirmed API gap:** C `EXI_LockSync` first attempts a non-blocking EXI lock, returns 0 in exception context when the channel is busy, and otherwise waits on `exi_sync_queue` until the unlock callback wakes it (`../../C-sources/libogc/libogc/exi.c:291-316`). Rust exposed only `lock` and `unlock`.

The Rust sys crate now exposes `lock_sync`, which uses lazily initialized LWP queues keyed by EXI channel and device. It disables interrupts across the failed-lock-to-sleep path and restores the prior interrupt state after success. The safe EXI crate exposes `lock_sync` and `try_lock_sync`. The shared `bus_probe` exercises immediate acquire/unlock, invalid-channel rejection, and the safe wrapper under Dolphin 2609a. Run `docs/dolphin-runs/2026-11-26-exi-lock-sync-lwp-queue`: 71 checks passed, 0 failed. The PowerPC `cargo check -p ogc-exi` passed.

The initial committed draft used the Tuxedo thread queue. Follow-up inspection showed that not every Rust LWP thread has a Tuxedo thread object, so the draft could not safely represent all callers. The follow-up commit replaces it with the LWP driver's keyed queues before independent review.

Contended blocking and exception-context behavior still need runtime coverage. Physical Wii verification is pending. Independent review of the callback, queue, and retry path is also pending; these APIs are not declared fully parity-reviewed until that review is complete.

## `EXI_Transfer` API parity follow-up — 2026-11-26

**Confirmed API gap:** C exports `EXI_Transfer`, which rejects read/write mode, uses immediate transfers below its alignment-dependent threshold, sends an immediate head to align the pointer, transfers aligned full cache lines by DMA, then sends the tail immediately (`../../C-sources/libogc/gc/ogc/exi.h:277-286`; `../../C-sources/libogc/libogc/exi.c:608-659`). Rust had the lower-level `imm_ex` and `dma` operations but no combined transfer API.

Rust now exposes raw `transfer` and safe-wrapper `try_transfer`. The shared planner follows C's 32-byte alignment and length rules, uses the existing cache helpers, blocks on a channel queue for threaded DMA completion, and uses synchronous DMA in exception context. A Dolphin probe invokes both public wrappers. It performs real immediate head/tail transactions and cache operations, and it injects successful DMA results through a feature-gated hook so Dolphin does not wait for an unconnected EXI DMA peripheral. Dolphin 2609a run `docs/dolphin-runs/2026-11-26-exi-transfer-verified` passed 75 checks; transfer checks covered 12-byte immediate, aligned 64-byte DMA, misaligned 31-byte head plus 32-byte DMA plus 1-byte tail, a 32-byte read, and read/write rejection.

An earlier attempt to wait for real EXI2 DMA in Dolphin timed out in the LWP idle thread because this setup did not produce a DMA completion interrupt. The injected run does not verify the production DMA completion callback/wakeup or exception-context DMA branch. Those runtime checks and physical Wii verification remain pending. Independent static re-review also remains pending.

## `EXI_LockSync` same-device waiter parity — 2026-11-26

**Confirmed defect:** C `EXI_Lock` allocates one entry per blocked request from a shared 32-entry pool (`../../C-sources/libogc/libogc/exi.c:51,145,259-285`). Rust suppressed a new queued callback whenever the same device ID already had an entry, so a second thread calling `EXI_LockSync` for the same channel and device could sleep without a callback to wake it. The first Dolphin contention probe reproduced the hang: [the timeout diagnostic](../dolphin-runs/2026-11-26-exi-lock-sync-one-waiter/bus_probe/timeout-diagnostic.json) placed Dolphin in the LWP idle thread after the log stopped at the non-contended lock checks.

Rust now keeps a shared 32-entry callback pool, preserves FIFO callback order per channel, and allows repeated entries for the same device. The unlock callback now signals one waiter, matching C `KThrQueueUnblockOneByValue`. The production `bus_probe` holds channel 2 while two threads request the same device lock, then verifies that both acquire and release it in turn. The Dolphin 2609a run `docs/dolphin-runs/2026-11-26-exi-lock-sync-callback-pool-final` passed 76 checks, including the contention case. PowerPC `cargo check -p ogc-exi` passed.

This verifies multiple same-device waiters within the 32-entry pool and confirms that the 33rd callback request is dropped. Independent static re-review and physical Wii behavior remain pending.

The comprehensive `runtime_probe` also verifies the raw callback contract. Its first run reported one failure because the probe expected one callback after it queued two requests for the same device. C `EXI_Lock` allocates one queue entry for each request, and each `EXI_Unlock` removes and calls one entry (`exi.c:259-285,325-348`). The probe now checks one callback after each unlock. The updated run `docs/dolphin-runs/2026-11-26-ipc-runtime-api-review-fixed` passed 873 checks with 0 failures under Dolphin 2609a. This corrected a stale test expectation; it did not change production behavior. A follow-up run, `docs/dolphin-runs/2026-11-26-exi-pool-capacity`, passed 874 checks with 0 failures. It queued 33 callbacks and observed the C-sized pool's 32 callbacks, with the overflow request dropped. This verifies callback-pool capacity. The probe covers repeated same-device requests, while `bus_probe` covers two blocked threads waking in turn. Physical Wii behavior remains pending.
