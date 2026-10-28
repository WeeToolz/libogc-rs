# LWP tick and alarm timer audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Scope: static cross-check of the Rust LWP driver timer engine and `KTickTask` adapter against C `tuxedo/tick.c` and `libogc/lwpcompat/lwpc_alarm.c`. No implementation changes, builds, or tests were made. The shared LWP note and ledger were not edited.

## Behavior that matches

- **Alarm replacement by identity:** C `KTickTaskStart` dequeues an already-active task by task pointer, then sets a new target/period and enqueues it (`C-sources/libogc/tuxedo/tick.c:94-112`). Rust timers replace an existing entry with the same key when `timer_start_inner` writes a new state (`crates/core/ogc-lwp-driver/src/runtime.rs:2012-2045`). The Tuxedo adapter keys each task by its pointer (`crates/compatibility/tuxedo-sys/src/lib.rs:704-729`), matching the C identity model.
- **Periodic due-task catch-up in the underlying engine:** C advances periodic `target` by exactly one period, re-enqueues, and loops while tasks remain due (`tick.c:74-91`). Rust advances the timer deadline by exactly one period and rescans due timers until none remain (`runtime.rs:733-769`). The underlying alarm/timer schedule therefore catches up missed periods rather than skipping directly to `now + period`.
- **Wraparound due comparison now matches C.** Rust uses the same signed interpretation of the wrapping `target - now` difference as `KTickIsSequential`, including the exact half-range boundary.
- **Callback context:** C routes decrementer exceptions through `KIrqDispatch` to `KTickTaskRun` (`C-sources/libogc/tuxedo/interrupt.c:96-110`), so tick and alarm callbacks run in decrementer exception context. Rust runs timer callbacks directly from `decrementer_tick` while its exception handler is active (`runtime.rs:806-810`); its API documents interrupts-disabled exception context (`:1968-1990`). The execution context matches in kind. Neither callback path is a normal thread callback.
- **Alarm adapter:** C `lwpc_alarm_cb` forwards the alarm pointer and saved argument to the registered callback (`libogc/lwpcompat/lwpc_alarm.c:14-18`). Rust `SYS_SetAlarm`/`SYS_SetPeriodicAlarm` register the callback and argument directly as keyed timer state (`crates/core/ogc-system-sys/src/lib.rs:2913-2963`), so argument forwarding is equivalent.

## `KTickTask` adapter parity follow-up

The previously recorded public-state differences are now fixed. Before invoking a callback, Rust updates a periodic task's public `target` by one period and reinserts it, or clears `fn_` for a one-shot task, matching `KTickTaskRun`. `k_tick_task_stop` clears only `fn_`, leaving the other public fields at their C-equivalent last values.

Rust now also maintains the public intrusive `next` links with the same target ordering as C. Starting a task removes an existing active instance before reinsertion; equal targets insert before existing equal-target tasks; periodic callbacks dequeue and reinsert before calling the callback; stopping or completing a one-shot removes the task without clearing its old `next` field, matching the observable C structure behavior. The scheduler still uses its separate keyed timer engine internally.

## Additional timer-engine differences

- **Equal-deadline order now matches C.** C inserts a task before existing tasks with equal `target`, so a newly started or periodic-requeued task runs first (`tick.c:12-40,74-91`). Rust assigns a new order to each timer registration and periodic requeue, then selects the newest timer first when deadlines match (`runtime.rs`, `timer_precedes`).
- **Fixed capacity versus intrusive C list.** Rust has `MAX_TIMERS = 64` (`runtime.rs:44`) and returns `LWP_CLOSED` when all slots are used (`:2015-2032`). C's tick queue is an intrusive linked list with no fixed task-count limit (`tick.c:5,31-40`). The alarm wrapper returns the timer-start result (`ogc-system-sys/src/lib.rs:2928-2934,2957-2963`), so reaching the Rust limit makes a valid C-style alarm registration fail.
- **Exact half-range wrap boundary is fixed.** `timer_due` now evaluates the wrapping `deadline - now` difference as `i64` and treats non-positive values as due, matching C at `2^63` as well as ordinary wraparound. A regression test covers the boundary.

## Result

The low-level Rust timer engine matches C periodic catch-up, callback context, equal-deadline ordering, and due-time wraparound comparison. The `KTickTask` adapter now preserves the checked C-visible task fields and intrusive links. Timer capacity remains different. The original audit was static-only; later fixes are recorded in the shared LWP review.
