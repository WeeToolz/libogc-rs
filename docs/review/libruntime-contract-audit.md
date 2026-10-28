# `ogc-runtime` public contract audit addendum

Reviewer: `thread-current/reviewer_4`  
Scope: public items outside the previously audited `ThreadId` invariant. This is a contract-surface comparison only; the crate has no implementation backend or C ABI exports.

## Result

No additional mismatch was confirmed. `ogc-runtime` defines shared Rust-side contracts (`driver.rs:7-53`, `error.rs:3-54`, `scheduler.rs:21-71`) rather than translating or re-exporting C symbols. Its `lib.rs:3-7` states that role; `Cargo.toml` has no dependencies. The named traits therefore describe Rust abstractions and do not claim one-to-one C signatures.

- `Lifecycle` and `SubsystemState` define a shared state model. C init/close APIs are subsystem-specific (for example STM's `__STM_Init` and `__STM_Close` in `../../C-sources/libogc/gc/ogc/stm.h:52-58`). The trait does not encode a conflicting C ABI or return-code mapping.
- `BlockDevice` uses block size/count and read/write/flush operations. C's closest media surface is the callback table `DISC_INTERFACE` (`../../C-sources/libogc/gc/ogc/disc_io.h:50-67`), which uses sector callbacks and boolean results. The Rust trait has no C-layout representation and is an internal storage abstraction, so its wider count and `Result` form do not create an ABI mismatch.
- `AsyncOperation` uses Rust `Poll` and `WiiError`, rather than C callback-plus-userdata completion. C USB async calls use `usbcallback(s32 result, void *usrdata)` (`../../C-sources/libogc/gc/ogc/usb.h:182,206-228`). This is an abstraction difference that an adapter must bridge, not a direct C API mismatch in this crate.
- `DebugTransport` and `HidTransport` are protocol-agnostic I/O traits, not C handle layouts. C USB transport functions have device handles, endpoint/request parameters, buffers, and callback arguments; the traits intentionally hide those details.
- `WiiError` and the subsystem error newtypes retain Rust-side error categories. The crate contains no conversion to legacy C status values; the public docs place such translation at compatibility boundaries (`lib.rs:5-7`, `error.rs:15-36`). No status mismatch can be established within this crate alone.
- `Deadline`, `MonotonicClock`, `TimerService`, `CallbackContext`, `DeferredExecutor`, and `InterruptWake` define scheduler contracts. C exposes hardware-time helpers (`../../C-sources/libogc/gc/ogc/lwp_watchdog.h:12-24,32-74`) and alarm callbacks/APIs (`../../C-sources/libogc/gc/ogc/system.h:221,310-348`). These are not identical interfaces. The LWP adapter maps the clock to `runtime::now_ticks` and timer deadlines to its timer service (`crates/core/ogc-lwp-driver/src/contracts.rs:11-23,72-105`).
- `WakeToken` is a public tuple value, but the LWP adapter validates both zero and `LWP_TQUEUE_NULL` before signaling/registering (`contracts.rs:48-53,63-68,80-99`). `LWP_TQUEUE_NULL` is `0xffffffff` and the C compatibility implementation ignores zero and this sentinel (`../../C-sources/libogc/gc/ogc/lwp.h:52-53`; `../../C-sources/libogc/libogc/lwpcompat/lwpc_queue.c:20-48`). The raw token type permits invalid values, but the adapter boundary rejects them. This is not an additional confirmed mismatch.

## Existing finding remains open

The separate `ThreadId` audit remains valid: `ThreadId::new` rejects zero but accepts `u32::MAX`, which C defines as `LWP_THREAD_NULL`. See `docs/review/libruntime-audit.md` and `docs/review/libruntime.md`. This addendum does not close or change that finding.

No implementation files were changed. No builds or tests were run. No hardware verification was performed.
