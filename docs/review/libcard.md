# Static C Parity Review: CARD

Status: independent static re-review passed. Repeated initialization preserves the first identifiers and mounted-card state, matching C `CARD_Init`. Hardware verification was not performed.

## Scope

- `ogc-card`: typed safe wrappers and raw facade in `crates/compatibility/ogc-card/src/lib.rs`.
- `ogc-card-sys`: ABI types and dispatch in `crates/services/ogc-card-sys/src/lib.rs`; PowerPC backend in `crates/services/ogc-card-sys/src/wii_backend.rs`.
- `ogc-card-metadata`: system-block checksum, copy-selection, and commit helpers in `crates/services/ogc-card-metadata/src/lib.rs`.
- C reference: `../../C-sources/libogc/gc/ogc/card.h` and `../../C-sources/libogc/libogc/card.c`.

Hardware behavior was not checked.

## Finding and fix

### Repeated initialization clears mounted-card state — fixed and re-reviewed

Before the fix, `ogc-card-sys::init` called `wii_backend::init` on every invocation. The backend cleared both slots' `SLOT_PRESENT`, `SLOT_MOUNTED`, workarea, error, geometry, and free-block state.

The C `CARD_Init` returns `CARD_ERROR_READY` immediately when `card_inited` is already set, before it changes identifiers or resets card state (`../../C-sources/libogc/libogc/card.c:2422-2426`). The previous Rust behavior therefore made later card operations report an absent/unmounted card and replaced the game/company identifiers.

**BUG FIX:** `ogc-card-sys::init` now uses an atomic one-time initialization state. Only the successful first call starts the backend and stores identifiers; later calls return `CARD_ERROR_READY` without resetting the slots or changing identifiers (`crates/services/ogc-card-sys/src/lib.rs:279-318`, `367-378`). Failed backend initialization resets the guard so a later call can retry.

Regression check: `repeated_init_keeps_first_identifiers_and_does_not_reinitialize_backend` verifies that the initialization closure runs only once and that a second call preserves the first identifiers. `cargo check -p ogc-card-sys --tests --target powerpc-unknown-linux-gnu` passed, including test compilation. The test could not be executed: no PowerPC emulator is installed, and `cargo test --no-run` could not link its PowerPC test binary because the host linker does not support the target's linker options. The custom `powerpc-nintendo-none` target also lacks an installed `core` library. No hardware verification was done.

## Compared behavior and limits

- `ogc-card-sys` constants, C-layout public structs, callback type, and raw-operation surface were compared with declarations in `card.h`. The visible fields and constants match the corresponding C declarations. Rust adds validation for typed game/company identifiers.
- The raw wrapper forwards operations to the Wii backend. The backend implements CARD operations directly over EXI rather than calling C `card.c`; this review traced initialization, probing, mounting, file reads/writes, directory enumeration, status access, and metadata commit paths. The repeated-init mismatch above is confirmed.
- `ogc-card-metadata` encodes the same 8 KiB header/directory/FAT block layout and checksum ranges used in `card.c` (`__card_checksum`, `__card_checkdir`, `__card_checkfat`, `__card_updatefat`, `__card_updatedir`). It validates buffer sizes and free-block counts. Its generation ordering is wrap-aware (`src/lib.rs:201-216`), while C compares generation counters with ordinary unsigned `<` (`card.c:614`, `670`). This is a deliberate robustness difference at counter wrap; no defect was established from static inspection.
- The compatibility crate adds owned asynchronous and slice APIs beyond C. Their added ownership and aligned-buffer behavior is outside direct C API parity. No separate confirmed defect was established in those additions during this pass.

This is a source review only. No hardware verification was performed, and no tests were run as part of this read-only review pass.


## Fix re-review

I compared commit `70a5d92` with C `CARD_Init` (`../../C-sources/libogc/libogc/card.c:2422-2445`) and inspected the complete source diff. The atomic initialization state returns `CARD_ERROR_READY` after successful initialization without running the backend again or changing the first game/company identifiers. A failed initialization returns the state to not-started so the caller can retry. This preserves the C early-return behavior and prevents the confirmed mounted-slot reset. The added regression check covers one-time initialization and identifier preservation. No confirmed finding remains in the recorded scope. The test compiled for the PowerPC Linux target but did not run; no hardware verification is recorded.

### Independent second re-review

I independently inspected the complete fix diff and `repeated_init_keeps_first_identifiers_and_does_not_reinitialize_backend` (`crates/services/ogc-card-sys/src/lib.rs:894-926`). C returns immediately when `card_inited` is set, before identifier updates and slot resets (`../../C-sources/libogc/libogc/card.c:2422-2445`). Rust's `CARD_INIT_COMPLETE` path also returns `CARD_ERROR_READY` without calling the closure; the first successful call stores the identifiers only after backend initialization succeeds (`crates/services/ogc-card-sys/src/lib.rs:286-318,367-378`). The test verifies one initialization call and preserves the first identifiers on the second call. I found no confirmed mismatch in this fix. No tests were run during this read-only pass; the prior fix-pass validation remains as recorded above. Hardware behavior remains unverified.
