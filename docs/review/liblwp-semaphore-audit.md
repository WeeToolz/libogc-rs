# Independent LWP Semaphore Audit

Date: 2026-10-01

Reviewer: `/root/reviewer_5`  
Scope: `ogc-lwp-driver` semaphore initialization, count limits, wait/post transitions, waiter ordering, and destroy/reinitialize behavior against the C LWP compatibility semaphore implementation. This addendum is separate from the shared LWP note. No implementation or shared note/ledger files were changed. No builds, tests, or hardware verification were performed.

## Sources inspected

- Rust: `crates/core/ogc-lwp-driver/src/runtime.rs`, `crates/core/ogc-lwp-sys/src/lib.rs`, `crates/core/ogc-lwp/src/lib.rs`.
- C: `../../C-sources/libogc/libogc/lwpcompat/lwpc_sem.c`, `../../C-sources/libogc/libogc/lwpcompat/lwpc_common.h`, `../../C-sources/libogc/libogc/lwpcompat/lwpc_thread.c`, `../../C-sources/libogc/tuxedo/thread.c`, `../../C-sources/libogc/tuxedo/thread_priv.h`.

## Confirmed differences

### `max` is ignored in C but enforced in Rust

C `LWP_SemInit` allocates a handle and stores `start`; its `max` argument is unused (`lwpc_sem.c:8-19`). `LWP_SemPost` increments the signed count and returns success even when it passes the supplied maximum (`:59-73`). Rust rejects `max == 0` and `start > max` (`runtime.rs:1572-1575`) and refuses a post when `value == max` (`:1647-1653`). Thus `LWP_SemInit(..., 0, 0)` and `start > max` succeed in C but fail in Rust; at `value == max`, C post succeeds and increments while Rust returns `LWP_CLOSED` without changing the value.

### Zero-count underflow is represented differently; ordinary API observations agree

C uses an `s32` count and decrements it on every wait before blocking when the result is negative (`lwpc_sem.c:31-45`). Posts increment the negative count and wake one waiter while the prior count was negative (`:59-73`). Rust keeps `value` at zero when a waiter blocks and stores the blocked thread in a waiter bitmap (`runtime.rs:1624-1633`); post wakes a waiter without incrementing the value, or increments the value when none wait (`:1637-1656`). `get_value` exposes zero for C negative counts (`lwpc_sem.c:48-56`) and the Rust stored count (`runtime.rs:1663-1679`), so normal wait/get/post sequences produce the same public nonnegative count. The internal underflow is an implementation difference, but I found no separate observable mismatch from it for the ordinary supported range.

For unusually large `start` values, C stores the `u32` argument into an `s32` slot (`lwpc_sem.c:5,17`); on the PowerPC two's-complement target a value above `INT32_MAX` becomes negative and `get_value` then reports zero. Rust stores the full `u32` and returns it directly. This is an additional edge-value result mismatch, while ordinary semaphore counts are aligned.

### Equal-priority wake order differs

C's semaphore wait queue is a Tuxedo `KThrQueue`; `KThrQueueInsert` inserts by priority and advances past existing equal-priority waiters (`tuxedo/thread_priv.h:65-92`), preserving FIFO order among equal priorities. `LWP_SemPost` calls `KThrQueueUnblockOneByValue` to wake the first matching waiter (`lwpc_sem.c:65-70`; `tuxedo/thread.c:253-291`). Rust chooses the matching waiter with the greatest effective priority, breaking ties by the lowest thread index because it scans indices from zero and replaces the current choice only for a strictly greater priority (`runtime.rs:572-588,591-604`). For equal-priority threads that arrive in a different order than their index order, post wakes a different thread. Priority direction is consistent with the C LWP adapter: C maps public priority `p` to `127 - p` (`lwpc_thread.c:39,80-85`), while Rust stores bounded `p` and chooses its maximum (`runtime.rs:976-977,489-513`). The tie-break ordering is the confirmed mismatch.

### Destroying a semaphore wakes waiters in Rust; C leaves them queued

C `LWP_SemDestroy` only validates null sentinels, frees the slot bit, and returns success (`lwpc_sem.c:21-29`). It does not find or remove waiters from the global semaphore queue, so blocked waiters remain stranded after destroy. Reinitializing the same slot resets the count without removing those old queue nodes. Rust close validates that the slot is in use, wakes all registered waiters with `Cancelled`, and clears the semaphore state (`runtime.rs:1596-1610`). A blocked Rust `semaphore_wait` then returns `LWP_CLOSED` (`:1629-1633`). This is a clear behavioral difference; Rust avoids the stranded-waiter behavior of C.

### Reinitialization and stale handles

C assigns each semaphore the one-based slot value and reuses the first free slot (`lwpc_sem.c:8-18`, shared allocator `lwpc_common.h:5-21`). Rust handles are also `index + 1` and decode without a generation counter (`runtime.rs:359-365,1579-1586`). Therefore a caller-held old handle can alias a newly initialized semaphore in the same slot in both implementations. The waiter consequences differ because C does not cancel queued waiters on destroy, while Rust wakes them before clearing the slot. This audit does not claim stale-handle safety for either API.

## Result

The `max` parameter, equal-priority wake ordering, and destroy-with-blocked-waiters semantics differ from C. Full-range initial counts also differ above `INT32_MAX` because of C's signed storage. Ordinary underflow-based wait/post/get behavior agrees at the public nonnegative value level. Rust and C both reuse numeric slot handles, so stale handle aliasing after reinitialization remains possible in both. Keep the LWP crates open for independent findings already present elsewhere in the review.

## Fix handoff: semaphore C semantics and equal-priority FIFO

Implementer: current parity pass. Independent re-review: pending.

The semaphore implementation now follows the inspected C compatibility layer more closely:

- `max` is accepted but ignored by initialization, so `max == 0` and `start > max` are no longer rejected solely for that reason.
- The stored counter is signed, initialized with the same `u32` to `s32` bit interpretation, decremented before wait, and incremented before post wake decisions. `get_value` reports zero for a negative counter as C does.
- Posts are no longer capped at `max`.
- Blocked threads now receive a monotonically increasing wait-order value. Waiter selection still prefers the effective priority, but equal-priority waiters are selected by arrival order instead of thread-slot index. This matches `KThrQueueInsert`, which advances past existing equal-priority waiters.

The generic equal-priority tie-break applies to the driver's priority waiter selection, which is consistent with the C `KThrQueue` ordering model used by the compatibility primitives. Destruction while threads are blocked remains a separate lifecycle edge case and is not claimed as resolved here. No hardware validation is claimed.
