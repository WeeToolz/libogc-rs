# LWP recursive mutex and condition wait audit

Static comparison of the recursive mutex and condition-variable wait paths in
the Rust port and the C compatibility layer. No builds or tests were run.

## Sources inspected

- C compatibility layer: `C-sources/libogc/libogc/lwpcompat/lwpc_sync.c`
  (`LWP_MutexInit` through `LWP_CondTimedWait`, lines 13–198).
- C Tuxedo synchronization: `C-sources/libogc/tuxedo/sync.c`
  (condition wait and timed wait, lines 168–220) and
  `C-sources/libogc/gc/tuxedo/sync.h` (`KRMutex` and recursive operations).
- C time conversion: `C-sources/libogc/gc/ogc/lwp_watchdog.h`, lines 44–50.
- Rust driver: `crates/core/ogc-lwp-driver/src/runtime.rs`,
  `init_mutex`/`close_mutex`/lock operations (1333–1449),
  `init_cond`/`close_cond`/`cond_wait` (1456–1534).
- Rust ABI wrapper: `crates/core/ogc-lwp-sys/src/lib.rs`,
  `TimeSpec`/`relative_ticks` (33–51) and `cond_timed_wait` (170–177).

## Matching behavior

For a valid recursive mutex owned by the calling thread, the recursive depth
is fully released during a condition wait and restored after mutex
reacquisition. C saves `KRMutex.counter`, sets it to zero, waits on the
underlying `KMutex`, and restores the saved value (`lwpc_sync.c`,
`LWP_CondTimedWait`, lines 172–198). Rust saves `MutexState.recursion`, reduces
it to one before `release_mutex_locked`, reacquires via `mutex_lock`, then
restores the saved depth (`runtime.rs`, lines 1512–1531). This provides the
same observable recursive ownership after a normal signal or timeout.

Both implementations reacquire the supplied mutex before returning from an
actual wait. The C Tuxedo wait functions re-lock `KMutex` before exit
(`tuxedo/sync.c`, lines 168–181 and 194–220); Rust calls `mutex_lock` after
`block_current_locked` (`runtime.rs`, lines 1522–1526).

## Confirmed differences

### Zero-duration timed wait

C `KCondVarWaitTimeoutTicks` returns false immediately when `timeout_ticks`
is zero (`tuxedo/sync.c`, timed wait, lines 194–198), before removing the
mutex owner or putting the thread on the condition queue. The compatibility
wrapper maps false to `ETIMEDOUT` (`lwpc_sync.c`, lines 162–170), after its
temporary recursive-counter adjustment. Thus a zero-timeout call returns
with the mutex still held and does not yield through the condition queue.

Rust converts a zero timespec to `Some(0)` (`ogc-lwp-sys/src/lib.rs`, lines
48–50), but `cond_wait` releases the mutex, queues the caller, and blocks with
a deadline equal to the current time (`ogc-lwp-driver/src/runtime.rs`, lines
1512–1522). It later reacquires the mutex and returns the timeout result.
This creates an observable scheduling/interleaving difference: another
thread can acquire the mutex during the Rust call, while the C path does not
release it for a zero timeout.

### Condition destruction with waiters

C `LWP_CondDestroy` only frees the condition slot (`lwpc_sync.c`, lines
125–133); it does not remove or wake threads waiting on the corresponding
Tuxedo condition queue. Rust `close_cond` wakes all current condition waiters
with `WaitResult::Cancelled` and clears the state (`runtime.rs`, lines
1478–1492); a blocked `cond_wait` then returns `LWP_CLOSED` after attempting
to reacquire the mutex (lines 1522–1532). Destroying a condition with waiters
therefore has different wake and return behavior. The C slot allocator also
permits reuse without clearing the old queue; a subsequent signal to the
reused token may interact with those old waiters.

### Mutex destruction while locked

C `LWP_MutexDestroy` frees a valid handle slot without checking whether its
mutex is locked or has waiters (`lwpc_sync.c`, lines 33–41). Rust
`close_mutex` returns `LWP_CLOSED` while an owner exists and only clears an
unowned mutex (`runtime.rs`, lines 1358–1372). C mutex initialization also
does not explicitly clear the static underlying mutex object (`lwpc_sync.c`,
lines 13–31), so destroying and reusing a locked C handle can expose stale
state. Rust rejects that operation. This is a real API behavior difference,
with Rust applying stricter lifecycle validation.

### Timespec validation

C converts `tv_sec` and `tv_nsec` directly to ticks without validating their
ranges (`lwp_watchdog.h`, lines 44–50; `lwpc_sync.c`, lines 162–170). Rust
rejects negative seconds and nanoseconds outside `[0, 1_000_000_000)` with
`LWP_CLOSED` before entering the wait (`ogc-lwp-sys/src/lib.rs`, lines
40–50, 172–177). For an invalid timespec, Rust returns while the caller still
owns the mutex; C behavior follows its unchecked integer arithmetic and is
not equivalent. This is an input-validation difference rather than a
normal-timeout mismatch.

## Review result

The recursive-depth save/release/reacquire/restore sequence matches for
ordinary valid waits. The zero-duration timeout path and condition destruction
with active waiters differ. Mutex destruction while owned and malformed
timespec behavior also differ, with Rust rejecting cases that C permits or
processes without validation. No implementation changes, builds, tests, or
hardware checks were performed.

## 2026-10-02 lifecycle/timing parity fix

The confirmed edge-case differences in this audit are now aligned with the current C compatibility layer.

- A zero-tick condition wait returns timeout before releasing the supplied mutex, matching `KCondVarWaitTimeoutTicks`.
- Condition destruction now frees only the compatibility slot and leaves queued waiters attached to the underlying static condition object. Reinitializing the same slot preserves that queue, matching C's static `KCondVar` reuse behavior.
- Mutex destruction now frees the handle slot even when owned or contended. Reinitialization changes only the compatibility `used`/recursive flags and preserves the underlying mutex state, matching C's static `KRMutex` table behavior.
- `TimeSpec` conversion no longer rejects negative/out-of-range fields. It follows C's unchecked unsigned conversion/wrapping arithmetic.

These semantics intentionally reproduce C's permissive/stale-slot behavior, including behavior that is less defensive than the earlier Rust implementation. Runtime scheduler tests and hardware validation remain pending.
