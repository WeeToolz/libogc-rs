# STM open findings independent audit

Reviewer: `thread-current/reviewer_4`  
Implementer for repeated-init change: `worker_c`  
Implementation commit: `bc1d7e7` (`Match STM repeated init early return`)  
Scope: repeated initialization and PowerPC target availability only. The previously accepted close-result fix was not reviewed here.

## Repeated initialization: fix matches C for repeated calls

C `__STM_Init` returns `1` immediately when `__stm_initialized == 1` at `../../C-sources/libogc/libogc/stm.c:80-82`; both `IOS_Open` calls occur afterward at lines 88-92. The committed Rust implementation routes `init()` and `init_without_event_hook()` through `init_with_openers` at `crates/core/ogc-stm/src/lib.rs:259-275`. That helper checks `state.initialized` while holding `STM_LOCK` and returns `1` before invoking `open_devices` at lines 339-357. The repeat-call behavior therefore no longer opens or closes extra descriptors and matches C's early return. The added `repeated_initialization_returns_before_opening_devices` check exercises the helper's no-open path (`lib.rs:688-729`), but it was not run or compiled during this review.

## Target availability: mismatch remains

C guards both the public STM declarations and implementation with `#if defined(HW_RVL)` (`../../C-sources/libogc/gc/ogc/stm.h:34-35`; `../../C-sources/libogc/libogc/stm.c:31-32`). Rust's only crate-level target guard is `#[cfg(not(target_arch = "powerpc"))] compile_error!` (`crates/core/ogc-stm/src/lib.rs:3-4`), and `crates/core/ogc-stm/Cargo.toml` declares no Wii/GameCube feature gate. Therefore any PowerPC build that includes this crate, including a GameCube build, gets the STM API even though C exposes it only for `HW_RVL` builds. This finding remains open.

## Result and validation

The repeated-init finding passes independent source review for the committed fix. The HW_RVL target-gating mismatch remains confirmed, so `ogc-stm` stays pending with `finding` status. No implementation files were changed. No builds or tests were run, and no hardware verification was performed.


## Later implementation follow-up

After this audit, the repository added a separate GameCube target profile (`targets/powerpc-nintendo-gamecube.json`) with a registered `cfg(gamecube)` switch. `ogc-stm` now compiles as an empty crate for that target and the compatibility facade omits its STM re-export there. This is intended to resolve the `HW_RVL` availability mismatch identified above. It is newer than this independent review and therefore still requires a separate re-review.
