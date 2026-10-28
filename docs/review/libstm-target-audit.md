# STM target-availability audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Scope: independent static cross-check of `ogc-stm` target gating and nearby public exports. No implementation changes, builds, tests, or hardware checks were performed. The shared STM review note and ledger were not edited.

## Finding confirmed: Rust exposes Wii STM on all PowerPC targets

The entire C public STM API is inside `#if defined(HW_RVL)` in `C-sources/libogc/gc/ogc/stm.h:34-64`. That guard includes event and error constants, the callback typedef, and declarations for initialization, close, standby/idle shutdown, LED mode, reboot, and event-handler registration (`stm.h:39-58`). The implementation is also enclosed by `HW_RVL` from `C-sources/libogc/libogc/stm.c:31` through `:281`.

Rust only rejects non-PowerPC compilation at the crate root (`crates/core/ogc-stm/src/lib.rs:3-4`). The source has no Wii console feature or `HW_RVL`-equivalent cfg for production API items, and `crates/core/ogc-stm/Cargo.toml` declares no features. All public STM constants and callback/type aliases are unconditional (`lib.rs:37-55`); production functions `init`, `init_without_event_hook`, `try_init`, `close`, `try_close`, `register_event_handler`, standby/idle, LED mode, and reboot and their `try_*` wrappers are likewise unconditional (`:259-283,360-399,401-490`). The other source `cfg` attributes are test/doctest helpers or tests, not Wii availability gates.

This means a GameCube PowerPC build that depends on `ogc-stm` receives the constants and callable API even though the C header omits them and C has no STM implementation on that target. The crate's own compile-error text explicitly describes its target as “Wii/GameCube PowerPC” (`lib.rs:4`), confirming the arch gate is intended to include both consoles. The C/Rust target-availability mismatch therefore applies to the entire public STM surface, not only the C-named functions.

## Nearby Rust-only exports

`init_without_event_hook` and the `StmResult`/`ErrorCode` convenience types/wrappers have no direct C declarations. They still open or operate Wii STM devices and are also exported under the same PowerPC-only gate. The C implementation opens `/dev/stm/immediate` and `/dev/stm/eventhook` (`C-sources/libogc/libogc/stm.c:80-92`); Rust `init_without_event_hook` also calls the helper that opens those device paths (`lib.rs:268-275` and initialization helpers). These Rust-only additions do not avoid the target mismatch.

## Result

The existing HW_RVL availability finding is upheld and applies to every public `ogc-stm` constant, type, and operation in the current crate, including the Rust-only initializer. Keep the target-availability issue open. No tests were run or compiled; no hardware verification was performed.


## Implementation follow-up

A subsequent implementation adds `targets/powerpc-nintendo-gamecube.json` with a registered `cfg(gamecube)` switch. `ogc-stm` has no public implementation on that target, and the compatibility `ogc` facade does not re-export STM under that cfg. The existing Wii/current target continues to expose STM. This addresses the target-availability finding above; independent re-review and target compilation are still required.
