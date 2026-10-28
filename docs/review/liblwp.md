# LWP Review

Date: 2026-10-15
Reviewers: `/root`, `/root/reviewer_f`, `/root/reviewer_e`, `/root/worker_c`, and `/root/review_lwp_parity`

Status: current static source comparison is complete for `ogc-lwp`, `ogc-lwp-driver`, and `ogc-lwp-sys`. The latest independent full pass found no confirmed non-safety parity blocker. Historical findings and fixes below retain their original review context; the 2026-10-15 full re-review resolves stale statements that call fixed findings open. Runtime tests and Wii hardware behavior remain unverified.

## Scope checked

- Rust public handle wrappers and their forwarding behavior in `crates/core/ogc-lwp/src/lib.rs`.
- Rust ABI facade thread and queue functions in `crates/core/ogc-lwp-sys/src/lib.rs`.
- C thread compatibility functions in `C-sources/libogc/libogc/lwpcompat/lwpc_thread.c` and declarations in `C-sources/libogc/gc/ogc/lwp.h`.
- C queue compatibility functions in `C-sources/libogc/libogc/lwpcompat/lwpc_queue.c`.
- C synchronization, semaphore, mailbox, and alarm functions in `lwpc_sync.c`, `lwpc_sem.c`, `lwpc_mq.c`, and `lwpc_alarm.c`, plus Tuxedo `sync.c`, `mailbox.c`, and `tick.c`.

The public C operations declared by `lwp.h`, `mutex.h`, `cond.h`, `semaphore.h`, and `message.h` have corresponding forwarding functions in `ogc-lwp-sys`; the behavioral comparison for thread scheduling, mutexes, condition variables, semaphores, and message queues remains in progress. `ogc-lwp-sys` is a Rust-callable facade, not a set of exported C ABI symbols. The higher-level `ogc-lwp` crate offers Rust handle wrappers. C watchdog entry points `SYS_CreateAlarm`, `SYS_SetAlarm`, `SYS_SetPeriodicAlarm`, `SYS_CancelAlarm`, and `SYS_RemoveAlarm` do not have direct counterparts in the facade; the driver's keyed timers are Rust services and their internal timing behavior is compared separately with `KTickTask`.

For the checked operations, the Rust facade preserves the C success/error conventions and the default 8 KiB thread stack size. Rust validates invalid handles and stack arguments more defensively. The underlying Rust scheduler is a Rust implementation, while the C compatibility layer delegates thread operations to Tuxedo `KThread` routines; the full scheduler behavior has not yet been compared.

## Findings

The Rust wrappers forward the reviewed operations to the driver. The full C wrapper-to-Tuxedo paths expose these confirmed differences:

- **Thread operation results now match C for valid handles.** Rust returns success for repeated suspend and resume calls with no active suspend count, as the C wrappers do.
- **Thread capacity and allocation differ.** C allocates thread objects and optional stacks dynamically in `LWP_CreateThread`. Rust uses a fixed 32-slot table and reserves slots 0–2 for main, idle, and deferred threads, leaving at most 29 user slots (`runtime.rs`, `MAX_THREADS`, `MAIN_THREAD`, `IDLE_THREAD`, `DEFERRED_THREAD`).
- **Mutex handoff now matches C.** `mutex_unlock` transfers ownership to the highest-priority waiter and wakes it. The waiter returns from its lock call with ownership instead of competing to reacquire the mutex (`runtime.rs`, `release_mutex_locked`, `mutex_lock`).
- **Out-of-range priority handling now matches C.** Thread creation and `set_thread_priority` mask the requested priority with `LWP_PRIO_HIGHEST`, as `lwpc_thread.c` does, instead of clamping values above 127.
- **Queue signal semantics now match C.** `queue_signal` only wakes a thread already waiting. It does not save a permit for a later `queue_sleep` (`runtime.rs`, `lwpc_queue.c`).
- **Semaphore maximum behavior now matches C.** Commit `88af436` removed Rust's max validation and limit, matching C `LWP_SemInit` and `LWP_SemPost` (`lwpc_sem.c:9-16,62-75`; `runtime.rs`, `init_semaphore`, `semaphore_post`).
- **Semaphore-destroy waiter behavior now matches C.** `close_semaphore` only frees the compatibility slot and does not wake or cancel blocked waiters. Reinitializing the same numeric slot resets the count while preserving those blocked waiters, matching C's separate slot table and global semaphore queue.
- **FIXED: waiter ordering after a priority change matches the reviewed C behavior.** Rust records `wait_order` when a thread blocks and refreshes it whenever a blocked priority waiter changes effective priority. Since selection orders by effective priority and then `wait_order`, this models C removing and reinserting the waiter after existing peers at its new equal priority. The shared rule applies to queue, mutex, condition-variable, semaphore, and message-queue waiters.
- **Priority inheritance through mutex chains now matches C.** When an effective priority changes, Rust recalculates the blocked thread's owner and continues through blocked mutex owners (`runtime.rs`, `recompute_priority_chain`).
- **Interrupt-return priority preemption is implemented and awaits hardware verification.** Rust now captures the interrupted `PPCContext` in both the external IRQ and decrementer paths, runs the corresponding Rust dispatcher, and lets the LWP runtime select a ready thread with a strictly higher LWP priority before restoring context with `rfi` (`ogc-sys/src/lib.rs`, `ogc-lwp-driver/src/runtime.rs`, `interrupt_return`). Equal and lower priority wakeups keep the interrupted thread running, matching C's strict priority comparison in `KThreadReschedule`. The interrupt-return callback saves the interrupted context before selecting another thread. Full context restoration includes GPR, FPR, paired-single, FPSCR, GQR, and control state. A regression test covers the priority decision. Production PowerPC checks and a HBC DOL build passed; test execution and Wii/Dolphin runtime verification are still pending.
- **FIXED: a zero condition timeout returns immediately.** Rust checks `Some(0)` before changing mutex recursion, releasing the mutex, or enqueueing the caller. It returns the timeout result with the mutex still held, matching `KCondVarWaitTimeoutTicks`.
- **Timed condition-wait error value differs.** C `LWP_CondTimedWait` returns the positive `<errno.h>` `ETIMEDOUT` value when `KCondVarWaitTimeoutTicks` returns false (`libogc/lwpcompat/lwpc_sync.c:162-166`). Rust's `WaitResult::TimedOut` is fixed to `-116` and is returned directly by `cond_wait` (`ogc-lwp-driver/src/runtime.rs:54,70-73,1522-1530`; `ogc-lwp-sys/src/lib.rs:172-175`). The Rust caller therefore receives a negative errno-style value, while the C compatibility function returns positive `ETIMEDOUT`.
- **FIXED: destroying a condition variable leaves its waiters blocked.** Rust `close_cond` now only clears the compatibility slot and preserves the underlying waiter queue, matching C `LWP_CondDestroy`. Reinitializing that slot therefore preserves the queued waiters associated with the same numeric handle.
- **Destroying an owned mutex differs.** C `LWP_MutexDestroy` frees the slot without checking the mutex owner (`lwpc_sync.c:33-40`). Rust `close_mutex` rejects a mutex with an owner (`runtime.rs:1358-1373`). This changes the result when callers destroy a locked mutex; the unchecked C behavior can leave a stale owner in the reused static slot.
- **Invalid condition timespec handling differs.** C passes `tv_sec` and `tv_nsec` directly to `timespec_to_ticks` without validating either field (`gc/ogc/lwp_watchdog.h:44-49`; `lwpc_sync.c:162-166`). Rust rejects negative seconds or nanoseconds outside `[0, 1_000_000_000)` with `LWP_CLOSED` before waiting (`ogc-lwp-sys/src/lib.rs:40-49,172-175`). This is a difference for malformed or out-of-range inputs; C's arithmetic may overflow for such values.
- **Message queue blocking behavior now matches the checked-in C source.** C `MQ_Send` forces `MQ_MSG_NOBLOCK` because its condition is literally `if (true)`; Rust's `message_send` now does the same. C `MQ_Jam` is not implemented; Rust keeps jam as additional API behavior.
- **Message queue capacities differ.** C allocates a mailbox sized to any positive requested count (`lwpc_mq.c`). Rust has a fixed 16-message storage array and rejects counts above 16 (`runtime.rs`, `MAX_MESSAGES`).
- **Message queue flag, exception, and null-output behavior now match C.** Rust tests the no-block bit, forces nonblocking receive when MSR.RI indicates exception context, and consumes a message when the output pointer is null. C `MQ_Send` currently forces nonblocking through an unconditional guard, so Rust's `message_send` also forces nonblocking (`lwpc_mq.c`, `runtime.rs`).
- **Message-queue close waiter behavior now matches C.** `close_message_queue` marks the mailbox slot unused without waking blocked senders or receivers. Those threads remain blocked instead of receiving `LWP_CLOSED`, matching `MQ_Close`, which frees the mailbox without broadcasting its wait queues (`lwpc_mq.c:33-40`; `tuxedo/mailbox.c:48-61`).
- **Joiner limits differ.** C's join queue wakes every thread waiting for the same target (`tuxedo/thread.c`, `KThreadExit` and `KThrQueueUnblockAllByValue`); each `LWP_JoinThread` then frees the target object (`lwpc_thread.c`). Rust permits only one join waiter and returns `LWP_CLOSED` to a second waiter (`runtime.rs`, `join_thread`). The C multiple-join path can free the same object more than once, so this appears to be a Rust safety correction to a C defect rather than a behavior to reproduce.

The C sem and mutex compatibility layers use fixed 64-entry tables; Rust also uses 64 fixed entries for these services. Rust's table of 64 queues matches the C 64-bit slot allocator. Rust message queue handle capacity is 16, while C mailbox handles are pointer-encoded and dynamically allocated. The Rust timer table is fixed at 64, while C alarms allocate from the LWP workspace; callback ordering and the 64-entry limit are recorded as differences above and in `docs/review/liblwp-tick-audit.md` for `tuxedo-sys`.

**Semaphore count parity was corrected in commit `88af436`.** C stores `start` as signed `s32`, decrements before testing for a negative value, ignores `max`, and increments on every post (`lwpc_sem.c:9-16,41-75`). Rust now uses `i32`, ignores `_max`, performs wrapping signed increments/decrements, and exposes negative values as zero (`runtime.rs`, `init_semaphore`, `semaphore_wait`, `semaphore_post`, `semaphore_get_value`). This also matches C's behavior for `start > i32::MAX`, subject to C's signed-overflow behavior at the extreme limits.

**Timer callback order for equal deadlines now matches C.** The Rust timer engine gives a new or periodic-requeued task precedence over existing tasks at the same deadline, as C's intrusive queue does.

The PowerPC `Context` field offsets and `0x2c0` size match `gc/tuxedo/ppc/context.h`. Rust's switch assembly saves and restores GPR, scalar FPR/FPSCR, CR, LR, CTR, XER, GQR, and paired-single state (`crates/core/ogc-lwp-driver/src/arch/powerpc.rs:142-173,199-261`); the earlier paired-single omission finding is stale. Rust enables and saves FPU state eagerly on each switch, while C starts threads without `MSR_FP` and saves/restores FPR and paired-single state through lazy ownership handling (`tuxedo/thread.c:54-69`; `exception_asm.S:124-149`). This changes FPU enable and context-switch strategy. Rust now writes `UINT32_MAX` as the initial back-chain sentinel in the fresh-thread frame, matching C. The trampoline and saved entry/argument handling still reflect Rust's runtime design and remain target-runtime validation items. Scheduler ordering is priority based in both implementations, but Rust uses fixed slots and round-robin cursors while C walks a priority-sorted linked list.

- **The SPRG corruption finding is fixed by the interrupt-return implementation.** Both installed exception stubs now save and restore SPRG2/3 around their Rust dispatch paths. Static and build checks passed. Hardware validation remains outstanding.

C provides per-thread newlib state in `KThread` and returns it through `__getreent()` (`tuxedo/thread_priv.h`, `tuxedo/thread.c`). The initial Rust review found no matching state. The 2026-10-04 follow-up below adds per-thread state and the hook to the Rust LWP driver. The public `KThread` structure remains ABI-incompatible with C, as recorded in `docs/review/libtuxedo.md`.

The Rust layer has additional API surface, including a usable reschedule operation and richer internal scheduler services. C marks `LWP_Reschedule` as not implementable in `lwpc_thread.c`. Record these as Rust additions; do not remove them as part of parity work.

## BUG FIX handoff: select a higher-priority LWP thread on interrupt return

Implementer: `/root`. Independent review and Wii/Dolphin runtime verification remain pending. The decrementer and external IRQ paths now save a full `PPCContext` to a 32-byte-aligned exception stack, dispatch the interrupt, call the LWP interrupt-return hook, and restore the selected context through `rfi`. The LWP hook copies the interrupted context to the current thread, then switches only when the current thread is no longer running or a ready thread has a strictly higher LWP priority. It updates the current thread state and switch counters before returning the selected context pointer. It records debug events without calling the user debug hook from exception return. This avoids calling the ordinary thread context-switch routine from exception context.

The exception stubs preserve SPRG2/3, which Tuxedo uses for its current-context and FP-owner pointers. A Dolphin report showed a write fault at PC `0x510` to `0x804c0884`, the former high-memory scratch word. Early scratch words now use `0x80000390`–`0x800003a0`, immediately after the HBC/libogc exception context at `0x800000d0`; vector entry reaches them through the low alias. Paired-single opcodes match the existing Tuxedo-layout context-switch code. The installer checks each vector stub's exact 72-byte length against its 0x100-byte slot.

Added a scheduler decision test for strictly higher priority versus equal/lower priority and non-ready candidates. PowerPC production checks passed for `ogc-sys`, `ogc-lwp-driver`, `ogc-lwp-sys`, and `ogc-lwp`; LWP test code compiled for the PowerPC Linux target but did not run. The regular HBC release DOL built and converted successfully. Dolphin batch mode ran for 20 seconds without exiting, but produced no runtime log or display, so it does not confirm that the fault is fixed. Real Wii validation remains outstanding. `git diff --check` and package formatting checks passed.

## 2026-10-03 stale-claim review closeout

Reviewer: `/root/reviewer_f`. Original static reviewer: `/root`. Implementer for the runtime parity commits listed above: `notOrrytrout` (commit author). The current source was checked against the cited C compatibility and Tuxedo paths, including waiter selection and priority requeueing, semaphore counts, recursive condition waits, message receive behavior, timer ordering, scheduler wake behavior, and PowerPC context paths. The review note's recorded mismatches remain findings; the waiter-order and semaphore entries were updated to distinguish the parts corrected by `88af436` from the remaining differences. No tests or builds were run during this static review, and no hardware verification is claimed.

The three crates remain open. In addition to the findings above, the newly confirmed priority-change waiter-order mismatch is recorded above. Other existing findings include thread/result and capacity differences, mutex handoff and transitive inheritance, interrupt preemption, condition timeout return value, message queue capacities/flags/null-output behavior and blocking semantics, timer ordering/capacity, eager versus lazy FPU strategy and SPRG state, and newlib per-thread state. This closeout records review evidence only; the primary reviewer owns the ledger update.

## 2026-10-02 condition/mutex edge-case follow-up

The recursive-condition audit's lifecycle/timing differences are now aligned with C: zero-tick waits return before unlocking, condition destruction frees only the handle slot without waking waiters, mutex destruction frees the slot even while owned/contended, reused slots preserve the underlying static synchronization state, and timespec conversion follows C's unchecked wrapping arithmetic. Broader scheduler differences listed above (fixed thread capacity, mutex handoff/fairness, queue permits, priority inheritance/preemption, newlib reentrancy, and others) remain open.

## Independent current-source review of remaining findings

Reviewer: `/root/reviewer_e`. Priority changes still differ: C removes and reinserts a blocked thread when its dynamic priority changes, placing it after existing equal-priority peers (`C-sources/libogc/tuxedo/sync.c:8–50`; `thread_priv.h:65–90`). Rust updates effective priority but retains the original `wait_order` (`ogc-lwp-driver/src/runtime.rs:624–647, 1131–1141, 573–597`). C also propagates priority inheritance through owner chains; Rust boosts only the immediate owner and recomputes direct waiters (`sync.c:39–50`; Rust `runtime.rs:826–841, 1437–1444`).

The earlier paired-single omission is stale: current Rust context-switch assembly saves/restores all paired-single registers, matching C's lazy handler's preservation of those values. Rust still uses eager FP enable and saves FPR/paired-single state on each switch; C enables ownership lazily. At the time of this audit, the decrementer vector still overwrote SPRG2/3; the interrupt-return implementation recorded below now saves and restores these registers.

The existing ABI findings remain: the Rust facade exports no C LWP symbols, uses Rust handle types rather than Tuxedo pointer-complement handles, and has no per-thread newlib `_reent` state or `__getreent` hook. Other note-listed synchronization and message differences were found present; this audit did not revalidate each one in full. `cargo check -p ogc-lwp`, `ogc-lwp-sys`, and `ogc-lwp-driver` with PowerPC `--tests` passed and compiled test code; tests were not run. No hardware verification was performed.

## BUG FIX handoff: requeue blocked waiters after an effective-priority change

Implementer: `/root/worker_a`. Independent re-review is needed; the review thread handles it. The shared waiter selector orders by effective priority and then `wait_order`. When `set_thread_priority`, mutex priority recomputation, or direct priority inheritance changes a blocked waiter's effective priority, Rust now gives it a new wait order. This places it after existing waiters at the new equal-priority level, matching C's remove-and-reinsert behavior. If the effective priority does not change, Rust preserves its old order and FIFO position.

The shared selector is used for queue, mutex, condition-variable, semaphore, and message-queue send/receive waiter masks. Join waits have a single permitted waiter and wake through a separate all-waiters path; sleep, external keyed waits, and deferred work do not use this priority selector. The fix changes ordering only for the affected blocked queue classes. Transitive owner-chain inheritance remains a separate finding.

Added tests for a blocked waiter whose priority changes to match two existing peers, and for an unchanged priority that preserves FIFO order. `cargo check -p ogc-lwp-driver --target powerpc-unknown-linux-gnu --lib` and `cargo check -p ogc-lwp-driver --target powerpc-unknown-linux-gnu --tests` passed; the test target compiled, but tests were not run. `git diff --check` passed. `cargo fmt -p ogc-lwp-driver -- --check` reports existing formatting differences elsewhere in the package. No hardware verification was performed.

## Historical BUG FIX handoff: preserve Tuxedo SPRG2 and SPRG3 in the decrementer vector

Implementer: `/root/worker_a`. Independent reviewer: pending. The Rust decrementer stub now saves the incoming SPRG2/3 values to dedicated scratch words before it uses those SPRGs to carry the interrupted SRR0/SRR1 values. The common handler restores the original SPRGs before restoring GPR11/12 and returning with `rfi`. This preserves Tuxedo's current-context pointer in SPRG2 and FPU-owner pointer in SPRG3 across the Rust decrementer callback. No change was made to `ogc-lwp-driver/src/runtime.rs`.

This earlier fix was superseded by the full exception context handoff above. Its original 17-instruction stub and old implementation details are historical; an intermediate version used 23 instructions. The current stub is 18 instructions (72 bytes). A PowerPC test checks the linked stub size and slot bound.

## BUG FIX handoff: propagate mutex priority inheritance through owner chains

Rust now recalculates inherited priority for the mutex owner and continues through each blocked owner to the next mutex owner when the effective priority changes. This follows C `KThreadUpdateDynamicPrio` in `tuxedo/sync.c`. The walk stops when the effective priority is unchanged, when a thread is not blocked on a mutex, or when it finds an invalid or repeated owner. Mutex acquisition, unlock, and explicit priority changes use the same recalculation path. Wait-order updates still occur through the shared priority update function.

A regression test exercises a three-thread, two-mutex chain and checks that the high-priority waiter's priority reaches both owners. `cargo check -p ogc-lwp-driver --target powerpc-unknown-linux-gnu --tests` and `cargo check -p ogc-lwp-driver --target powerpc-unknown-linux-gnu --lib` passed. `cargo test -p ogc-lwp-driver --lib --target powerpc-unknown-linux-gnu --no-run` could not link because the configured Clang linker rejected the GNU linker flags; the regression test was compiled but not run. `rustfmt --check` and `git diff --check` passed. No hardware verification was performed. Exception-return preemption remains an open separate assembly handoff issue; this change does not alter that path.

## C behavior follow-up: mutex, queue, priority, message, and timer paths

Rust mutex unlock now transfers ownership to the selected highest-priority waiter. The waiter returns from `mutex_lock` with the transferred ownership. Queue signals now wake only an existing waiter and do not create permits. LWP thread priority inputs use C's `& LWP_PRIO_HIGHEST` mask. Suspend/resume wrappers return success for valid handles when the operation is already applied or has no active suspend count.

Message queue behavior now masks `MQ_MSG_NOBLOCK`, consumes a message when the output pointer is null, and forces nonblocking receive when MSR.RI indicates exception context. `MQ_Send` also forces nonblocking, matching the checked-in C source's unconditional `if (true)` guard. Rust timer selection now uses insertion order so a newly scheduled equal-deadline task runs first; periodic requeue assigns a new order as C does. The bounded Rust queue, thread, and timer storage remain capacity differences from C's dynamic allocations.

PowerPC library and test-code checks passed. The added priority-chain, priority-mask, message-flag, null-output consumption, and timer-order tests compile. Test execution remains unavailable because the configured linker cannot link the PowerPC Linux test binary. No hardware verification was performed.

## 2026-10-04 per-thread newlib state follow-up

The Rust driver now owns an initialized DevkitPPC `_reent` block for each of its main, idle, deferred, and user threads. The `__getreent` hook returns the running thread's block and returns the idle block during exception context, matching `tuxedo/thread.c`. Rust devoptab callbacks now use the same current-thread state, and `_impure_ptr` follows the current thread at scheduler handoff. Rust also defines the `_impure_data` and `__sf` backing storage, so the port does not add a C library dependency. The layout and initialization values match the active DevkitPPC `sys/reent.h` ABI (`sizeof(struct _reent) == 512`, `sizeof(__FILE) == 112`); a regression test checks the stream pointers and `rand`/`rand48` initialization. The Wii production build and HBC DOL link passed. LWP test code compiles for PowerPC Linux, but tests were not executed. Wii/Dolphin runtime verification remains outstanding.

The storage-capacity mismatch remains open. Thread storage is still a 32-slot array, message queues still use fixed message storage, and driver timers still use a 64-slot table. The scheduler stores waiters in 32-bit masks, so removing the thread limit requires replacing those masks and updating every waiter-selection path. The driver also has no allocator interface available before and during scheduling. Do not count a larger fixed table as resolving this C parity finding. A Rust-owned allocation service and its startup/memory ownership contract must be designed before converting these stores to growable storage. C semaphores, mutexes, and queue handles use fixed 64-entry slot tables, so those limits already match the checked C implementation and do not need dynamic allocation for parity.

## 2026-10-04 implementation handoff

Implementer: `/root`. Independent static re-review is needed; the review thread handles it. This handoff groups the current driver and interrupt changes: full interrupt-context save and scheduler selection on return; transitive mutex priority inheritance; C-compatible mutex handoff, queue signals, priority masks, message flags and null-output consumption; exception-context nonblocking message receive; timer insertion ordering; and per-thread DevkitPPC `_reent` state with `__getreent` and `_impure_ptr` updates. The LWP probe also checks that system initialization did not clear the floating-point enable bit and that real VI interrupts return with the exception frame intact.

`cargo check -p ogc-sys -p ogc-lwp-driver -p ogc-lwp-sys -p ogc-lwp -p ogc-ipc -p ogc-gx -p ogc-video-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled the target test code. `cargo xtask build-dols` passed after the GameCube DVD compile correction recorded in `libdvd-gc-audit.md`. The broader check that also compiled `ogc-system-sys` tests failed on existing target-test cfg, callback ABI, and unsafe-call errors; those tests did not compile or run. No hardware verification was performed.


## Independent re-review — waiter requeue and interrupt/context handoff

Reviewer: `worker_c`. Implementers: `worker_a` (commit `384529b`, waiter ordering; `cacbff6`, earlier SPRG save) and `/root` (runtime and interrupt/context batch in `fd886ce`). I compared the priority requeue path with C `KThreadUpdateDynamicPrio` and `KThrQueueInsert` (`../../C-sources/libogc/tuxedo/sync.c:8-50`; `thread_priv.h:65-90`). A changed effective priority gives a blocked priority-ordered waiter a new order, so it goes after existing equal-priority peers; an unchanged priority preserves its order. The shared selector covers the queue classes listed in the handoff. I also checked the final interrupt frame offsets against `PPCContext` (`../../C-sources/libogc/gc/tuxedo/ppc/context.h`) and the restore path against `__PPCContextJump` (`../../C-sources/libogc/tuxedo/interrupt_asm.S`). The saved PC/MSR/CR/LR/CTR/XER, GPR, FPSCR/FPR, GQR, and paired-single offsets match the C layout; IRQ and decrementer paths use the same frame and scheduler return hook. The initial SPRG-only fix is superseded by this final path, which leaves SPRG2/3 untouched. I found no confirmed mismatch in these handoffs. The `ogc-lwp-sys` facade has no semantic source change in the reviewed commits; commit `2c26784` only reformats its PowerPC `compile_error!`. The low-level interrupt implementation is in `ogc-sys`, and the scheduler changes are in `ogc-lwp-driver`. This does not close broader LWP findings listed above, including fixed capacities and public C ABI differences. Implementation PowerPC checks and their limits are recorded above; I ran no checks for this static re-review. No hardware verification was performed.

## 2026-10-05 fix handoff: condition timeout return value

Implementer: `/root`. The LWP driver keeps its internal negative timeout status, but the `ogc-lwp-sys::cond_timed_wait` C facade now maps that status to newlib's positive `ETIMEDOUT` value (116), matching `lwpc_sync.c:165`. Success and other driver errors pass through unchanged. The driver already returns for a zero-tick timeout before releasing the mutex, matching `KCondVarWaitTimeoutTicks`; this handoff changes only the facade return value. Added a fixed-value mapping regression test. `cargo check -p ogc-lwp-sys --tests --target powerpc-unknown-linux-gnu` passed; test code compiled but was not run. `cargo fmt -p ogc-lwp-sys -- --check` and scoped `git diff --check` passed. Independent static re-review is needed. Other LWP capacity, ABI, destruction, and queue-state differences remain open. No hardware verification was performed.


## 2026-10-05 fix handoff: semaphore destroy waiters

Implementer: `/root`. C `LWP_SemDestroy` only releases the 64-entry compatibility slot; it does not wake threads already blocked on the global semaphore wait queue. Rust now does the same. `close_semaphore` marks the slot unused without cancelling its waiter mask, and reuse of that numeric slot resets the semaphore count while retaining old blocked waiters. The regression checks that a blocked waiter remains queued across destroy and slot reuse. This fixes the recorded semaphore-destroy waiter mismatch only; other LWP capacity, destruction, ABI, and queue-state findings remain open. Independent compile/static validation is required.

## 2026-10-05 fix handoff: timer half-range due boundary

Implementer: `/root`. Rust timer due detection now matches C `KTickIsSequential` at the exact `2^63` wrap boundary by interpreting the wrapping `deadline - now` difference as signed and treating non-positive values as due. The regression covers equal, future, past, and exact half-range deadlines. This fixes only the due-time comparison edge; the fixed-capacity timer table and the public `KTickTask` field/link differences remain open. Independent compile/static validation is required.


## 2026-10-05 fix handoff: `KTickTask` intrusive links

Implementer: `/root`. The Tuxedo compatibility adapter now maintains `KTickTask.next` with the same intrusive target ordering as C `tuxedo/tick.c`. Start removes an already-active task before reinsertion, equal targets insert ahead of existing equal-target tasks, periodic callbacks dequeue and reinsert the task before the user callback, and stop/one-shot completion remove the task without clearing the removed task's old `next` value. The stop path also preserves `target` and `period` while clearing only `fn_`, matching C. Regression checks cover equal-target insertion, dequeue link repair, periodic pre-callback state, and one-shot callback state. This closes the recorded public `KTickTask` field/link mismatch; fixed timer capacity remains open. Independent compile/static validation is required.


## 2026-10-05 fix handoff: message-queue close waiters

Implementer: `/root`. Rust `close_message_queue` no longer broadcasts cancellation to blocked senders or receivers. It marks the mailbox slot unused and leaves the waiter masks untouched, matching C `MQ_Close`, which frees the mailbox without waking either wait queue. The regression checks that close preserves both blocked-waiter sets. This closes the recorded message-queue close-result mismatch only; message capacity, thread capacity, timer capacity, and legacy ABI differences remain open. Independent compile/static validation is required.

## 2026-10-05 fix handoff: Tuxedo suspended queue/resume state

Implementer: `/root`. The Tuxedo adapter now matches C's public state guards when suspending/resuming and when external queue waits are unblocked or cancelled. A queued thread that is still suspended no longer reports `RUNNING`; resume does not report `RUNNING` while an external queue or mutex wait remains, and repeated suspension does not overwrite a non-running wait state. This closes the recorded queue/resume public-state mismatch. Thread/message/timer capacity, `KThread`/legacy ABI, and join edge behavior remain open. Independent compile/static validation is required.

## 2026-10-06 workspace-backed dynamic mailbox and timer storage

The earlier fixed-capacity mailbox/timer design is replaced with Rust-owned dynamic objects allocated from the same dedicated LWP workspace initialized by `ogc-system-sys`. The system layer registers allocation/free callbacks with `ogc-lwp-driver` immediately after the 64 KiB LWP workspace is initialized and before scheduler initialization. This avoids introducing any requirement for a process-wide Rust global allocator and keeps ownership aligned with C's `__lwp_wkspace_allocate` model.

Message queues no longer have either the 16-mailbox handle limit or the 16-message payload limit. `MQ_Init` allocates mailbox metadata plus a message array sized to the requested positive count. The public handle uses C's 32-bit complemented-pointer form (`~mailbox_ptr`). Rust additionally keeps an intrusive live-mailbox list and validates a handle against that list before dereferencing it; stale/arbitrary non-null handles therefore fail safely instead of reproducing C's invalid-pointer dereference behavior. `MQ_Close` unlinks the mailbox before freeing it, so blocked threads cannot later dereference freed metadata; they remain stranded just as C leaves them unwoken.

Timer state no longer uses a 64-entry array. Active timers are intrusive workspace-backed nodes, while completed/cancelled nodes move to a reusable free list. New registrations reuse a free node before allocating another workspace node. One-shot callbacks unlink/recycle their node before invoking user code, and periodic timers retain the established target/order update behavior. Recycling avoids workspace allocator mutation from decrementer callback context. The remaining LWP capacity blocker is the 32-thread scheduler and its 32-bit waiter masks.

Safety divergences introduced by this storage change:

- mailbox handles are validated against the live intrusive list before dereference; C trusts any non-null/non-`MQ_BOX_NULL` complemented pointer;
- mailbox size multiplication is checked for overflow and allocation failure returns `MQ_ERROR_TOOMANY`/null handle instead of permitting wrapped allocation size;
- closed mailbox metadata is unlinked before free so stale waiters cannot dereference freed storage;
- timer nodes are recycled rather than freed from decrementer/interrupt callback context.

These differences are memory-safety/resource-lifetime additions only. Valid mailbox ordering, send/receive behavior, timer ordering, key replacement, periodic scheduling, and callback semantics remain governed by the existing parity logic.

## 2026-10-06 intrusive waiter-queue conversion

All scheduler waiter bitmaps have been replaced by a single intrusive `WaitQueue` representation. Each thread owns one `wait_next` link, which is sufficient because a thread can be blocked on only one join/queue/mutex/condition/semaphore/mailbox wait at a time. Join waiters and every synchronization object's waiter collection use the same insertion, removal, best-waiter selection, wake-one, and wake-all helpers.

Best-waiter selection retains the existing parity policy: highest effective priority wins, with the existing `wait_order` ticket breaking equal-priority ties. Priority changes still refresh `wait_order` for blocked threads, so the previously fixed equal-priority requeue behavior remains intact. Mutex inheritance walks the intrusive waiter chain instead of scanning a 32-bit mask.

This removes the waiter representation's hard 32-thread ceiling and is the prerequisite for dynamic thread storage. Thread objects themselves are still in the fixed 32-entry table in this commit, so the thread-capacity blocker remains open until the following storage conversion.

## 2026-10-06 dynamic scheduler and legacy-ABI completion pass

The remaining recorded LWP capacity blockers are closed at source level. Thread metadata now uses a `ThreadTable` with the original static first page plus workspace-backed pages allocated in 32-slot chunks on demand. Scheduler scans use the live table capacity. Public `lwp_t` values use a 32-bit complemented-pointer representation and are validated against the live registry before use. Workspace-backed fallback stacks are released after a successful join; metadata pages are retained as reusable high-water capacity rather than freed while the scheduler is live.

All scheduler waiter bitmaps were replaced by one intrusive `WaitQueue` mechanism shared by joins, generic queues, mutexes, condition variables, semaphores, and mailbox send/receive waits. The shared waiter implementation preserves priority ordering and equal-priority requeue behavior without a 32-bit thread mask. Fixed-width mutex-owner state and priority-inheritance visited masks were also removed, so those paths do not introduce a hidden 32- or 255-thread ceiling.

Mailbox payload storage and mailbox metadata are dynamically workspace-backed, with C-style complemented-pointer handles. Timer registrations use intrusive workspace nodes with a reusable free list rather than a 64-entry table. These changes remove the recorded fixed mailbox/message/timer capacities while retaining the existing C-compatible ordering and callback behavior.

The driver thread-entry type is now `extern "C"`, and all Rust-owned LWP worker entrypoints were converted to the same calling convention. `ogc-lwp-sys` exports the complete checked legacy symbol set from `lwp.h`, `mutex.h`, `cond.h`, `semaphore.h`, and `message.h`: 30 `LWP_*` symbols and 5 `MQ_*` symbols. These exports are thin delegates to the existing snake-case facade, so there is still one implementation of each operation and no unresolved libogc C dependency.

Tuxedo-created threads can register caller-owned saved-context and `_reent` storage with the driver. Cooperative context switches mirror the fully saved 0x2c0 PowerPC context at the assembly save point before loading the next thread; interrupt-return saves mirror the exception frame immediately. The embedded `KThread.r` is used directly by `_impure_ptr`/`__getreent`. This closes the prior private-driver-state versus public-Tuxedo-state mismatch without replacing the driver's bootstrap execution model.

### Safety/resource divergences retained

The following differences are intentional safety additions rather than parity blockers:

- arbitrary/stale complemented thread and mailbox handles are validated before dereference instead of trusting pointer-shaped integers;
- allocation-size overflow and workspace allocation failure return resource errors instead of permitting wrapped allocation sizes or invalid pointers;
- the Rust driver permits only one join waiter because the C multiple-join path can free the same thread object multiple times;
- invalid Tuxedo mutex unlock returns instead of reproducing C's infinite-spin deadlock; invalid zero-count recursive unlock is ignored instead of wrapping the recursion counter;
- closed mailbox metadata is unlinked before release so stale handles cannot dereference freed storage; C leaves already-blocked mailbox waiters stranded and Rust preserves that externally visible no-wakeup behavior without use-after-free;
- timer nodes are recycled instead of freed from decrementer/interrupt callback context;
- dynamically allocated thread-table pages are retained for reuse until process lifetime ends, avoiding allocator mutation of scheduler metadata during shrink/reuse cycles.

No confirmed non-safety blocker remains in the recorded `ogc-lwp`, `ogc-lwp-driver`, or `ogc-lwp-sys` scope after this pass. These crates require PowerPC compilation plus independent scheduler/runtime re-review before `reviewed`.

## C update gap and fix — 2026-10-15

C reference: `ff53c4a7b7ba5587d82305e710ca65bce1092e1c`.
Changed contract: commit `adf4c782da8c55c8f17aca2047e128502e7eddc2`.
This supersedes the earlier no-open-gap status for this contract.

C `libogc/lwpcompat/lwpc_thread.c::LWP_SetThreadPriority` now treats
`LWP_THREAD_NULL` as the calling thread. It selects `KThreadGetSelf()` and
changes that thread's priority.

Rust now resolves `LWP_THREAD_NULL` to `CURRENT` while interrupts are disabled,
then uses the same priority update and inheritance-recalculation path as an
explicit handle. Other invalid handles remain ignored. A regression test calls
the production driver entry point and checks null-handle selection, explicit
handle behavior, and invalid-handle rejection. `cargo xtask check-all-runtime`
and `cargo check -p ogc-lwp-driver --target powerpc-unknown-linux-gnu --tests`
pass. The latter compiles, but does not run, the scheduler tests. Independent
parity re-review and Wii scheduling behavior remain pending.

`cargo xtask build-lwp-probe` also passed. Dolphin 2603-415 did not reach the
probe checks: `cargo xtask dolphin-validate --no-build --probe lwp_probe`
timed out at boot stage 3 in `compiler_builtins::u64_div_rem`, before the
probe wrote its log. The run is recorded in
`target/dolphin-validation/1792083837-e882d364e0b9/lwp_probe/result.json`.
This is not a pass for the null-priority regression; Dolphin validation and
Wii scheduling behavior remain open.

## Independent re-review — null priority handle (2026-10-15)

Reviewer: `/root/review_lwp_parity`. Result: **PASS for the bounded C contract**.
C `LWP_SetThreadPriority` maps `LWP_THREAD_NULL` to `KThreadGetSelf()` and
applies the masked public priority
(`../../C-sources/libogc/libogc/lwpcompat/lwpc_thread.c:86-91`). Rust resolves
the sentinel to `CURRENT` with interrupts disabled, uses the existing priority
update and inheritance-recalculation path, and restores the interrupt state on
both valid and invalid handles
(`crates/core/ogc-lwp-driver/src/runtime.rs:2495-2515`). The sys and typed
facades delegate to this driver path. The regression checks null selection,
explicit current-thread selection, and invalid-handle rejection
(`runtime.rs:3816-3832`).

This pass closes the null-priority finding only. The target test code compiled
but did not run, Wii scheduling behavior remains unverified, and the broader
LWP scheduler/runtime parity review remains incomplete.

## Independent full source re-review — LWP crates (2026-10-15)

Reviewer: `/root/review_lwp_parity`. Scope: `ogc-lwp`, `ogc-lwp-driver`, and
`ogc-lwp-sys`, compared with the current C reference at
`../../C-sources/libogc` (including libogc LWP compatibility code and Tuxedo
scheduler, synchronization, mailbox, timer, and PowerPC context code). Result:
**source parity review complete; no confirmed non-safety parity blocker found**.
This re-review supersedes older status sentences above that describe already
corrected behavior as open. Historical fix and review records remain as their
commit evidence. No tests were run for this static pass, and no runtime or Wii
hardware result is claimed.

The public legacy API is covered end-to-end. The C declarations in
`gc/ogc/lwp.h`, `mutex.h`, `cond.h`, `semaphore.h`, and `message.h` map to the
30 `LWP_*` and 5 `MQ_*` exports in
`crates/core/ogc-lwp-sys/src/lib.rs:352-565`. Each exported wrapper delegates
to the snake-case facade, which delegates to `ogc-lwp-driver`; the typed
`ogc-lwp` handles in `crates/core/ogc-lwp/src/lib.rs:60-385` are thin wrappers
over those calls. Handle widths and C ABI signatures use 32-bit handles,
pointer-sized message values, `extern "C"` thread entries, and the target
`TimeSpec` layout. The target newlib defines 64-bit `time_t` and 32-bit `long`
for this ABI (`/opt/devkitpro/devkitPPC/powerpc-eabi/include/sys/_types.h:182-189`,
`sys/_timespec.h:47-50`), matching Rust's `i64`/`i32` fields at
`ogc-lwp-sys/src/lib.rs:66-71`.

I traced thread create, suspend/resume, query, self, priority, yield, and join
through C `libogc/lwpcompat/lwpc_thread.c:10-107` and Tuxedo
`tuxedo/thread.c:55-75,174-192,218-264`, against Rust
`crates/core/ogc-lwp-driver/src/runtime.rs:1762-1839,2334-2565` and its
PowerPC context implementation. The C complemented pointer handles and
dynamic allocation are represented by Rust's 32-bit complemented handles,
live registry, and workspace-backed thread pages (`runtime.rs:261-285` and
thread allocation/lookup paths); the null-priority behavior is independently
re-reviewed above. Priority masking, scheduling order, queue wake behavior,
recursive/non-recursive mutex behavior, owner handoff, transitive priority
inheritance, condition-variable mutex release/reacquisition, timeout status,
semaphore signed counts, and mailbox FIFO/jam/send/receive behavior were
compared with C `lwpc_sync.c:13-190`, `lwpc_sem.c:8-74`,
`lwpc_mq.c:16-88`, `tuxedo/sync.c:8-215`, and `tuxedo/mailbox.c:8-67`.
Rust paths include `runtime.rs:1583-1636,2809-3120,3258-3284`. Current fixes
match the checked C behavior, including
positive `ETIMEDOUT` mapping (`ogc-lwp-sys/src/lib.rs:208-218`), unchecked C
timespec conversion (`:73-83`), and handle-slot-only mutex/condition/semaphore
destruction (`runtime.rs:2809-2824,2925-2940`); earlier note entries that say
these still differ are stale.

Dynamic thread, message, and timer storage removes the earlier capacity
mismatches. Thread pages grow on demand; mailbox metadata and payload storage
are workspace allocated; timer nodes use workspace allocation and a reuse
list (`runtime.rs:261-285, 3670-3725`). Timer deadline ordering and callback
state were compared with `tuxedo/tick.c:94-148`; condition, semaphore, queue, and
mailbox wait queues were compared with Tuxedo's block/unblock paths. Current
PowerPC save/restore code was compared with
`gc/tuxedo/ppc/context.h`, `tuxedo/interrupt_asm.S`, and
`tuxedo/exception_asm.S`. Rust's eager FPU ownership and its scheduler
representation are implementation differences; the saved context and
observable valid-operation scheduling behavior match the checked C paths.

Documented safety/resource deviations remain intentional: Rust checks
complemented handles against live registries before dereference; checks
allocation-size arithmetic and returns allocation errors; uses only one join
waiter to avoid C's multiple-free path; retains/reuses scheduler metadata and
timer nodes where freeing in exception/scheduler paths is unsafe; and rejects
invalid Tuxedo mutex operations instead of reproducing C deadlocks or counter
wrap. Rust-only additions include the usable `LWP_Reschedule` behavior,
`MQ_Jam`, std thread/TLS/futex services, and diagnostic APIs. C's
`LWP_Reschedule` and `MQ_Jam` are declared but unimplemented in the checked-in
reference. These additions do not replace a C contract.

The static source comparison is complete. Target test binaries have compiled
in prior checks, but those tests did not run; this report does not establish
runtime scheduling, interrupt, timer, callback, or Wii hardware behavior.

## 2026-10-28 focused mutex try-lock probe

The LWP probe now checks the public `ogc_lwp::MutexHandle::try_lock` and
`MutexHandle::unlock` contracts: try-lock acquires a free nonrecursive mutex,
reports `Ok(false)` when the same thread already holds it, and unlock releases
the acquired lock. The Rust method maps the driver's success code to `Ok(true)`,
its busy code `1` to `Ok(false)`, and preserves other error codes. This matches the checked C path in
`libogc/lwpcompat/lwpc_sync.c` and `tuxedo/sync.c`, where a try-lock succeeds
only when the owner is null.

The rebuilt current probe passed under Dolphin 2609a. The exact command was
`cargo xtask dolphin-validate --probe lwp_probe --artifact-dir
/Volumes/SSD/WiiDev/Devtools/reports/evidence/libogc-rs-lwp-try-lock-2026-10-28-final`.
Its DOL SHA-256 was
`f8b0cb74dbd71297180cb32cc54dd2370f3383f0733512a4b4652672775ec6fd`; the
report records 127 passes and zero summary failures. All three try-lock checks
passed, including successful unlock. The report and probe log are in that
evidence directory.

An earlier `--no-build` run used a stale Dolphin DOL and is not evidence for
the change. The current Dolphin build reports that the Wiiload argument check
is disabled for this probe build. Its recovery watchdog is also not armed, so
this DOL is not qualified for physical upload. Dolphin does not establish Wii
hardware behavior. The implementation inventory now links this API to the
production-path probe source; this records probe coverage and does not claim a
physical Wii pass. Runtime evidence for this change is emulator-only, and Wii
hardware validation remains open.

### 2026-10-28 typed mutex lifecycle extension

The production-path probe now destroys the original mutex, then uses
`MutexHandle::init` to reinitialize the freed slot. It verifies that the new
mutex can be locked and unlocked, then calls `MutexHandle::destroy`. This
checks slot reuse and that the reinitialized mutex starts unlocked. The checks
passed in a fresh Dolphin run with 134 passes and zero summary failures. The
final command was `cargo xtask dolphin-validate --probe lwp_probe --artifact-dir
/Volumes/SSD/WiiDev/Devtools/reports/evidence/libogc-rs-lwp-mutex-lifecycle-2026-10-28-final`.
DOL SHA-256:
`1844309ae1c797321b7fcb491d60570edacd7ae106d88f9eaec18bba8bea33a3`.
The report and probe log are in that evidence directory.

This remains Dolphin-only runtime evidence. The probe's recovery watchdog is
not armed in this build, and the physical Wii network is not ready. No hardware
run is claimed.

### 2026-10-28 bounded synchronization probe

The synchronization probe now starts its semaphore at count zero. It verifies
that the worker stays blocked before `sem_post`, then checks that `sem_wait`
completes and the worker joins after the post. The earlier probe used an initial
count of one, so its “blocks” check did not prove blocking behavior. The
condition wait now has a two-second bound and records the mutex lock, condition
signal, and unlock results. A separate bounded driver condition wait checks
the timeout result and mutex state after return. Typed semaphore checks read
the initial count, post one permit, read the updated count, consume it through
the sys API, and read zero again. The blocked worker now calls
`SemaphoreHandle::wait`. The nonblocking `cond_wait` sys wrapper remains
without an exact probe record because this batch uses the bounded timed
interface.

The current source passed in Dolphin 2609a with 148 passes and zero summary
failures; the report has no `FAIL` lines. The command was
`cargo xtask dolphin-validate --probe lwp_probe --artifact-dir
/Volumes/SSD/WiiDev/Devtools/reports/evidence/libogc-rs-lwp-sync-v2-2026-10-28`.
DOL SHA-256:
`9974b4751c057f2e838eaff4b28cbe1248cce220fc7487181d9f77716847ffdf`.
The report and probe log are in that evidence directory. The report-enabled
probe build also passed with `cargo xtask build-agent-probes`.

This is Dolphin-only evidence. The recovery watchdog is not armed in this DOL;
no physical Wii run is claimed.
