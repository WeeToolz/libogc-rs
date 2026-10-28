# Additional EXI interrupt and cleanup audit

Reviewer: `/root/reviewer_6`  
Scope: additional interrupt dispatch, callback context, and cleanup comparison for `ogc-exi` / `ogc-exi-sys`. The recorded insertion debounce finding is excluded. The shared `docs/review/libexi.md` and ledger were not changed.

## Confirmed mismatch: queued unlock callbacks run after interrupts are restored

C `EXI_Unlock` disables interrupts before changing the lock state and invoking a queued callback, then restores the prior interrupt state after the callback returns (`../../C-sources/libogc/libogc/exi.c:264-297`). Rust `unlock` updates the state while holding `EXI_LOCK`, explicitly drops the guard at `crates/hardware/ogc-exi-sys/src/lib.rs:623`, and invokes the queued callback afterward (`:624-628`). On a normal thread call where interrupts were enabled at entry, the Rust guard restores that enabled state before the callback. The callback can therefore be interrupted, and other EXI activity can interleave before it runs; C invokes the same callback with interrupts disabled. This is an observable callback ordering/context difference.

## Confirmed mismatch: channel 0 and channel 2 EXI interrupt masks are not coupled like C

C couples EXI0 and EXI2 interrupt masks in the channel-0 branch: it masks both and unmasks both when channel 0 is unlocked and either channel 0 or channel 2 has an EXI callback (`../../C-sources/libogc/libogc/exi.c:121-138`). Rust's `refresh_exi_interrupt` updates only the IRQ line for the requested channel and bases it on that channel's lock and callback (`crates/hardware/ogc-exi-sys/src/lib.rs:467-477`). Therefore registering a callback on channel 0 while channel 0 is unlocked causes C to unmask EXI2 as well, while Rust unmasks only EXI0. Registering a channel-2 callback while channel 0 is locked also differs: C's channel-0 branch keeps both lines masked, while Rust can unmask EXI2 based on channel 2's own state. Rust also does not implement C's separate channel-2 `__exi_setinterrupts` branch, which gates EXI2 on the presence of `IRQ_PI_DEBUG` (`exi.c:134-137`); Rust applies its generic callback/lock rule to channel 2.

## Compared paths with no additional mismatch confirmed

- **EXI transfer-complete callback basics:** C masks and clears the TC interrupt before consuming the callback, reads the immediate register for an immediate transfer, clears transfer flags, then invokes the callback (`../../C-sources/libogc/libogc/exi.c:778-814`). Rust masks and clears TC, captures the immediate word, clears transfer state, releases its state lock, and invokes the callback (`crates/hardware/ogc-exi-sys/src/lib.rs:362-381,324-339`). The normal registered-callback completion order matches at this level.
- **External interrupt cleanup:** C masks the IRQ, clears the EXI external status bit, clears `EXI_FLAG_ATTACH`, then invokes the callback (`exi.c:816-835`). Rust performs the corresponding mask, status clear, attach/cache/timestamp cleanup, and then calls the registered callback (`crates/hardware/ogc-exi-sys/src/lib.rs:384-407`). Cache/timestamp invalidation is Rust's support for its probe model; no additional mismatch is claimed here.
- **Probe result after a transient external IRQ:** C clears a pending external IRQ and resets its detection timestamp before applying the insertion debounce (`exi.c:174-197`). Rust clears its pending IRQ and debounce start and reports no device for that call, then starts a new debounce on a later probe (`crates/hardware/ogc-exi-sys/src/lib.rs:703-735`). Both paths suppress a positive result until a later stable probe; exact return behavior for `probe_ex` was not found to differ in this case.

## Validation limits

This was a source-only comparison. No builds, tests, or hardware checks were performed. No implementation changes were made.

## Follow-up implementation — EXI interrupt/callback parity

Implementation pass: 2026-10-01.

The two confirmed differences above are now addressed in `ogc-exi-sys` by source comparison with current `libogc/libogc/exi.c`:

- `unlock` releases the Rust software spin lock before invoking a queued callback, but keeps the caller's external-interrupt state disabled until the callback returns. This preserves C's callback interrupt context without deadlocking when the callback re-enters EXI.
- EXI interrupt refresh now follows C's channel-specific rules. Channel 0 masks/unmasks EXI0 and EXI2 together based on channel-0 lock state and EXI0/EXI2 callbacks; channel 1 is handled independently; channel 2 uses the debugger-handler gate. Registering an EXI callback on channel 2 refreshes the channel-0 pair as C does.

These changes need build/test and independent review. No hardware result is claimed.
