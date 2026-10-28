# GameCube ACR IRQ API audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Crates: `ogc-irq`, `ogc-irq-sys`  
Scope: independent static verification of the existing GameCube ACR API mismatch. No implementation changes, builds, tests, or hardware checks were performed.

## Finding confirmed: Wii-only ACR identifier and mask are public on GameCube

C conditionally declares both `IM_PI_ACR` and the `IRQ_PI_ACR` enum member only for Wii (`C-sources/libogc/gc/tuxedo/interrupt.h:47-51,84-87`). Consequently, on GameCube `IRQ_MAX` follows `IRQ_PI_CP` (numeric value 26), and the C handler table has 27 entries (`C-sources/libogc/tuxedo/interrupt.c:13`). There is no C enum member or mask constant for ACR on that target.

Rust's shared `ogc-irq-sys` crate always exports `IRQ_PI_ACR = 27` and sets `IRQ_COUNT` to 28 (`crates/core/ogc-irq-sys/src/lib.rs:46-48`). `ogc-irq` re-exports that ID and always defines `IM_PI_ACR` (`crates/core/ogc-irq/src/lib.rs:9-17`). These declarations are available for both Wii and GameCube PowerPC crate builds, so the public API differs from the target-specific C header.

The existing runtime mask correction only covers hardware mask writes. `platform_irq_mask` removes the ACR bit on GameCube (`ogc-irq-sys/src/lib.rs:235-247`), and `k_irq_enable`/`k_irq_disable` pass masks through that filter before modifying hardware (`:676-702`). Cause decoding also returns ACR only when `is_wii()` is true (`:652-654`). These guards prevent a GameCube ACR hardware event from being enabled or dispatched.

They do not correct handler-table/API availability. `index` accepts any ID below the unconditional `IRQ_COUNT` (`:250` onward, referenced by `k_irq_get` at `:705-708` and `k_irq_set` at `:710-730`). Therefore GameCube callers can get/set ID 27 and receive a stored callback value, even though C's GameCube `IRQ_MAX` excludes that ID and C has no corresponding ACR interrupt. The mask filter cannot make this identifier equivalent to the C API; it only makes it inert with respect to hardware delivery.

## Result

The prior finding is upheld: the ACR interrupt ID and mask should be considered Wii-only to match the C declaration surface. Rust's GameCube runtime filtering is useful and correct for hardware mask/cause behavior, but it does not remove the public GameCube API mismatch or the extra handler-table slot. Keep `ogc-irq` and `ogc-irq-sys` open for this finding. This note adds independent source evidence; it does not change the shared review note or ledger.


## Implementation follow-up

A subsequent fix adds `targets/powerpc-nintendo-gamecube.json` with a registered `cfg(gamecube)` switch. `ogc-irq-sys` now keeps IRQ value 27 private for shared register logic but does not export `IRQ_PI_ACR` on that target, uses 27 handler slots instead of 28, omits ACR from `IM_PI`, and excludes ACR cause decoding. `ogc-irq` also omits `IRQ_PI_ACR` and `IM_PI_ACR`. This directly addresses the finding above against the current C `__gamecube__`/`__wii__` split. The implementation follow-up has not been independently re-reviewed or hardware-tested.
