# LWP interrupt return and dispatch audit

Reviewer: `/root/reviewer_6`  
Scope: separate source audit of interrupt wake/preemption and exception-return state in `ogc-lwp-driver`, compared with Tuxedo `thread.c` and `interrupt_asm.S`. The shared `docs/review/liblwp.md` and ledger were not changed. Context register save/restore field layout is outside this sub-audit.

## Confirmed mismatch: decrementer handler corrupts Tuxedo SPRG state

The Rust decrementer vector copies the interrupted SRR0 and SRR1 into SPRG2 and SPRG3 (`crates/abi/ogc-sys/src/lib.rs:140-152`) and later uses those SPRGs to restore SRR0/SRR1 before `rfi` (`:251-259`). It does not save and restore the previous contents of SPRG2/3. In the C Tuxedo runtime, `KThreadInit` stores the current `KThread*` in both SPRG2 and SPRG3 (`../../C-sources/libogc/tuxedo/thread.c:32-51`). The C external IRQ handler reads SPRG2 as the current thread context and SPRG3 as the floating-point owner while saving and restoring interrupted thread state (`../../C-sources/libogc/tuxedo/interrupt_asm.S:9-10,55-57,80-87`).

Therefore, when Rust's installed decrementer vector runs in an environment that still uses the C Tuxedo IRQ handler, exception return leaves SPRG2/3 holding the interrupted PC/MSR instead of their Tuxedo context pointers. A subsequent external interrupt can use those corrupted values as context pointers. Restoring SRR0/SRR1 with `rfi` does not restore the separate SPRG state. This is a concrete cross-runtime machine-state incompatibility beyond the Context struct layout comparison.

## Confirmed dispatch-order difference: awakened higher-priority thread waits for a later scheduler point

C queue wakeup calls `KThreadReschedule` for the highest-priority unblocked thread (`../../C-sources/libogc/tuxedo/thread.c:253-285`; selection and pending dispatch policy are in `thread_priv.h:120-127`). In exception context, `KThreadSwitchTo` stores the candidate in `__ppc_next_ctx` and returns (`thread.c:83-97`). After `KIrqDispatch`, C's IRQ assembly checks that pending context and switches the current context before `rfi` (`../../C-sources/libogc/tuxedo/interrupt_asm.S:50-74,75-111`).

Rust timeout processing wakes a blocked thread by changing it to `Ready` and recalculating the decrementer (`crates/core/ogc-lwp-driver/src/runtime.rs:553-570`). Its decrementer callback calls `process_timeouts_locked` (`runtime.rs:806-811`), and the Rust decrementer exception stub always restores the interrupted PC/MSR and returns to that thread; it does not select a ready thread or switch contexts during exception return (`crates/abi/ogc-sys/src/lib.rs:209-259`). The existing LWP note already records this no-preemption behavior. This audit independently confirms it from the exception-return path: a newly readied higher-priority thread does not run until the interrupted thread next reaches a Rust scheduling point.

## Validation limits

This was a source-only comparison. No builds, tests, or hardware checks were performed. No implementation changes were made.
