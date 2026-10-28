# libstm Review

Date: 2026-10-01

Status: the close-result and repeated-initialization fixes passed independent static review and match the checked C paths. The `HW_RVL` target-gating difference remains open. This is not a hardware verification record.

## Scope and coverage

- Rust crate: `crates/core/ogc-stm`
- Rust source: `crates/core/ogc-stm/src/lib.rs` (the crate has no other source modules)
- C references: `C-sources/libogc/libogc/stm.c` and `C-sources/libogc/gc/ogc/stm.h`
- `stm.h` and `stm.c` are guarded by `HW_RVL`; the checked-in C implementation is for Wii.

The Rust public API was inventoried and mapped to the C declarations:

| Rust API | C API | Result |
| --- | --- | --- |
| `STM_EVENT_RESET`, `STM_EVENT_POWER`, `STM_EINVAL`, `STM_ENOTINIT`, `STM_ENOHANDLER`, `StmCallback` | Header constants and `stmcallback` | Values and callback signature match. |
| `init`, `try_init` | `__STM_Init` | Opens `/dev/stm/immediate` and `/dev/stm/eventhook`, then registers the event hook. |
| `close`, `try_close` | `__STM_Close` | Releases the event hook and closes both descriptors. See confirmed return-value difference below. |
| `register_event_handler` | `STM_RegisterEventHandler` | Returns the prior callback and stores the new callback. |
| `shutdown_to_standby`, `try_shutdown_to_standby` | `STM_ShutdownToStandby` | Clears VI control, passes zero input, and sends ioctl `0x2003`. |
| `shutdown_to_idle`, `try_shutdown_to_idle` | `STM_ShutdownToIdle` | Clears VI control, selects the Hollywood-revision input word, and sends ioctl `0x2004`. |
| `set_led_mode`, `try_set_led_mode` | `STM_SetLedMode` | Passes the full 32-bit mode and sends ioctl `0x6002`. |
| `reboot_system`, `try_reboot_system` | `STM_RebootSystem` | Clears VI control, passes zero input, and sends ioctl `0x2001`. |
| `StmResult`, `ErrorCode` re-export and `try_*` wrappers | No direct C counterpart | Rust convenience API. |

Private helpers `_stm_set_event_hook*` and `_stm_release_event_hook*` map to the C event-hook helpers. The C source also defines `__STM_SetEventHook` and `__STM_ReleaseEventHook` as global symbols, but the header does not declare them; Rust keeps their equivalents private.

The event ioctl uses aligned 32-byte input and output buffers in both versions (`stm.c:64-67, 133-138`; Rust `lib.rs:57-85, 183-219`). The ioctl numbers and data words match (`stm.c:42-53, 189-276`; Rust `lib.rs:48-53, 390-479`). The VI write targets the same register (`stm.c:78, 193-214, 262`; Rust `lib.rs:482-487`).

## Confirmed differences

1. **Repeated initialization performed extra opens and closes in Rust.** C returns immediately when already initialized, before opening either path (`stm.c:80-92`). Rust opened both paths first (`lib.rs:259-270`), then checked `initialized` and closed any newly opened descriptors (`lib.rs:293-310`). It still returned success, but added IOS open/close calls and their side effects. The fix handoff below moves the initialized check before both device opens.
2. **Platform availability differs.** C exposes this API only under `HW_RVL` (`stm.h:34`, `stm.c:31`). Rust gates only on `target_arch = "powerpc"` (`lib.rs:3-4`), so it also compiles the Wii STM API for other PowerPC targets, including GameCube builds if they use this crate.

## Rust-only behavior and fixes

- `init_without_event_hook` is public Rust API with no C header counterpart (`lib.rs:272-283`). It opens the same two devices and marks STM initialized but skips hook registration.
- `try_init`, `try_close`, and the `try_*` operation wrappers convert integer status results to `StmResult`; these are Rust convenience APIs.
- Rust closes the immediate descriptor if opening `/dev/stm/eventhook` fails (`lib.rs:326-332`). C returns failure without closing the already-open immediate descriptor (`stm.c:88-92`). This is a resource-leak fix in Rust, so it intentionally differs from C.
- Rust protects shared STM state with a spin lock. C uses global state without the same lock (`stm.c:57-67`). This is a Rust synchronization adaptation.
- Rust clears VI before checking initialization in shutdown and reboot functions, matching C (`stm.c:193-199, 214-220, 262-268`; Rust `lib.rs:390-400, 413-423, 459-469`).

## Fix pass — implementer `/root`

**BUG FIX — close return behavior.** Before commit `9f660d4`, Rust returned the event-hook release error when both descriptor closes succeeded. C discards that result and returns only descriptor-close errors (`stm.c:103-120`). Rust now ignores the release result and preserves the C behavior. A regression test checks that a release failure does not change a successful close result; the combined-failure test confirms a descriptor-close error is still returned.

### Independent re-review — `thread-current/stm_rereview`

The changed close path matches the C control flow: it ignores the release-hook result, closes both descriptors even when the first close fails, and returns the last descriptor-close error. The release helper's guard conditions also match `__STM_ReleaseEventHook` (`stm.c:141-153`): both require initialized state and a registered handler before issuing the release ioctl. The added regression test covers release failure with successful closes, and the existing test covers release failure plus an immediate-descriptor close failure. The fix is accepted in this scope. The repeated-initialization fix below also passed independent review; see `libstm-rereview.md`. The target-gating difference remains open.

## BUG FIX handoff: repeated initialization

Implementer: `worker_c`. Independent reviewer: `/root/worker_b`.

Both `init()` and Rust-only `init_without_event_hook()` now pass lazy device-open closures to a shared helper. The helper checks `initialized` under the STM lock before it invokes the closure. A repeated call therefore returns success before either `/dev/stm/immediate` or `/dev/stm/eventhook` is opened, matching C `__STM_Init`'s early return at `stm.c:80-92`.

The regression test initializes state with existing descriptors, then calls the helper with a counting opener. It checks that the call returns success, does not invoke the opener, does not close descriptors, and preserves the stored descriptors. `cargo check -p ogc-stm --target powerpc-unknown-linux-gnu --tests` passed. `cargo test -p ogc-stm` could not build on the host because PowerPC-only dependencies reject non-PowerPC targets. The tests were compiled for PowerPC but not run. No hardware verification was performed.

Validation: `cargo check -p ogc-stm --target powerpc-unknown-linux-gnu --tests` passed and compiled the regression checks. `cargo test -p ogc-stm` could not build on the host because its PowerPC-only dependencies reject non-PowerPC targets. `git diff --check` passed. The test binaries were not run.

## Unchecked areas and limits

- No tests or compile checks were run for this documentation-only review.
- No Wii hardware or IOS device behavior was tested.
- The exact behavior of IOS on duplicate open/close calls and on event-hook release failures was not exercised; the differences above follow directly from the checked-in C and Rust control flow.
- The Rust crate's PowerPC target gating was compared with the checked-in C `HW_RVL` guard; support for any other PowerPC target is not established.
- Other C STM ioctl constants in `stm.c` are unused by the public implementation in this source and have no Rust entry point. They were not treated as missing public APIs because `stm.h` declares no corresponding functions.

## Independent re-review: GameCube target availability (`e694fd2`)

**Result: NEEDS_CHANGES.** Reviewer: `reviewer_e`. Implementer: `notOrrytrout` (commit author). This review covers only the target-availability change; the previously reviewed close-result and repeated-init fixes remain separate.

### Static C/Rust availability comparison

C exposes the STM declarations only inside `#if defined(HW_RVL)` in `../../C-sources/libogc/gc/ogc/stm.h:34` (through the header's closing guard), and compiles `stm.c` only inside the same `HW_RVL` guard (`../../C-sources/libogc/libogc/stm.c:31`). Therefore the STM API is unavailable in GameCube/HW_DOL builds and available for Wii/HW_RVL builds.

The target-gating intent is correctly carried into the current Rust sources. The GameCube target profile is `targets/powerpc-nintendo-gamecube.json` and Cargo applies `--cfg=gamecube` in `.cargo/config.toml` under `[target.powerpc-nintendo-gamecube]`. `ogc-stm` places its whole implementation behind `#![cfg(not(gamecube))]` (`crates/core/ogc-stm/src/lib.rs:1–2`), so a direct dependency for GameCube builds has no STM API; the `ogc` facade also gates `pub use ogc_stm as stm` with `#[cfg(not(gamecube))]` (`crates/compatibility/ogc/src/lib.rs:43–45`). With the Wii target, which does not enable `gamecube`, both gates leave the API available. This matches the C compile-time availability split. Cargo's dependency is intentionally unconditional so the same dependency graph can be used and crate-level cfg removes the public implementation on GameCube (`crates/compatibility/ogc/Cargo.toml:41–43`).

### Current target validation blockers

The checked-in GameCube target does set the intended cfg: the attempted nightly target build's verbose rustc invocation included `--cfg=gamecube`. However, the profile-specific `rustflags` replace the `[build].rustflags` array, so the target rustc invocation omitted `--check-cfg=cfg(gamecube)`. Current GameCube checks emitted `unexpected_cfgs` warnings from the gated crates; a `-D warnings` target check will reject them. The global check-cfg entry at `.cargo/config.toml:9–11` therefore does not register the custom cfg for this target profile as intended.

Both direct crate and public facade checks were attempted with the GameCube JSON target and nightly `build-std`:

- `cargo check -p ogc-stm --target ./targets/powerpc-nintendo-gamecube.json` failed while compiling dependency `ogc-ipc`. Its `install_interrupt_handler` still references `irq::IRQ_PI_ACR` and `irq::IM_PI_ACR` without a GameCube cfg (`crates/core/ogc-ipc/src/lib.rs:1124–1135`), while the GameCube IRQ API correctly hides those values. The STM crate itself is gated empty, but Cargo still checks its dependencies.
- `cargo check -p ogc --target ./targets/powerpc-nintendo-gamecube.json` failed on the same unresolved ACR names through the facade dependency graph. The facade's STM export is cfg-hidden as intended, but the facade does not currently compile for the target.

These failures mean the static STM gate is correct, but the claimed usable GameCube target profile is not validated end to end. The IRQ dependency failure is directly observable from the selected target's dependency graph and blocks both requested import paths. Coordinate its implementation with the related IRQ/IPC target work rather than treating a successful STM-only source gate as a passing target build.

### Coverage and validation

No test was executed. The two target checks above failed before producing a usable crate/facade build; they are not passing compile checks. The actual target profile did apply `cfg(gamecube)`, as verified in Cargo's verbose rustc command. No hardware verification was performed. The ledger is unchanged; only this review note was edited.

## Independent re-review: current GameCube gate and build path

Result: **STM API gate PASSES C availability parity; GameCube target build remains NEEDS_CHANGES**. Reviewer: `/root/reviewer_d`. Implementer: `notOrrytrout`, original target commit `e694fd2`, with current custom-cfg gate changes in `a5736189` and facade gate change in `966aef9` (commit authors checked directly).

### C and Rust API availability

C places all STM declarations and constants under `#if defined(HW_RVL)` (`../../C-sources/libogc/gc/ogc/stm.h:31-64`), and the implementation is under the same guard (`../../C-sources/libogc/libogc/stm.c:31`, closed at the end of the file). Thus C does not declare or compile this STM API for GameCube/HW_DOL.

The current Rust direct crate uses `#![cfg(not(gamecube))]` (`crates/core/ogc-stm/src/lib.rs:1-5`), and the `ogc` facade only re-exports `ogc_stm` under the same cfg (`crates/compatibility/ogc/src/lib.rs:43-45`). The GameCube target profile supplies `--cfg=gamecube` (`.cargo/config.toml:26-32`). Its target JSON environment is still `eabi`; the custom cfg is what selects GameCube behavior. This cfg hides the STM Rust API for GameCube and leaves it available for the Wii target, which matches the C `HW_RVL` split. The earlier crate-level compile-error form has been replaced by the crate-level cfg gate, so the current source no longer errors specifically because the target is GameCube.

### Current GameCube target build failure

I reran both target checks using the installed nightly toolchain with `-Z build-std=core,alloc -Z json-target-spec`.

- `cargo check -p ogc-stm --target ./targets/powerpc-nintendo-gamecube.json` fails while compiling `ogc-ipc`, before the gated STM crate can be checked as a usable dependency. Rust `ogc-stm` still lists `ogc-ipc` as an unconditional Cargo dependency (`crates/core/ogc-stm/Cargo.toml:7-10`), and Cargo checks that dependency even though the STM crate body is cfg-disabled. `ogc-ipc::install_interrupt_handler` unconditionally references `irq::IRQ_PI_ACR` and `irq::IM_PI_ACR` (`crates/core/ogc-ipc/src/lib.rs:1124-1135`), but `ogc-irq` hides those ACR identifiers under `#[cfg(not(gamecube))]` (`crates/core/ogc-irq/src/lib.rs:17-20`). The GameCube IRQ crate intentionally omits ACR (`crates/core/ogc-irq-sys/src/lib.rs:46-53`). The resulting errors are in the adjacent IPC/IRQ dependency path, not in the STM gate.
- `cargo check -p ogc` fails at the same `ogc-ipc` references. The facade hides its STM re-export, but `ogc-ipc` remains an unconditional facade dependency (`crates/compatibility/ogc/Cargo.toml:35,42`), so the target dependency graph still compiles the failing IPC crate.
- This dependency mismatch is also visible in C: the checked-in C IPC implementation is itself guarded by `#if defined(HW_RVL)` (`../../C-sources/libogc/libogc/ipc.c:31` through the file's final `#endif`), and the ACR handler installation is inside that guarded source (`ipc.c:805-821`). Rust `ogc-ipc` does not gate that Wii-only handler path for GameCube. Therefore a real GameCube target build/API availability gap remains in the neighboring IPC/IRQ integration, but it is not evidence that the STM API gate is wrong.

### Check-cfg configuration

`.cargo/config.toml` sets `--check-cfg=cfg(gamecube)` under `[build].rustflags` (`.cargo/config.toml:10-13`), but also defines target-specific `rustflags` for GameCube (`:26-32`) without that check-cfg argument. Cargo's verbose rustc invocation used `--cfg=gamecube` but omitted `--check-cfg=cfg(gamecube)`. The facade check consequently emitted `unexpected cfg condition name: gamecube` warnings from `ogc-irq-sys` and `ogc-irq`. Strict warning-deny target builds will fail on those warnings until the custom cfg declaration is included in the selected target flags. This is a build-configuration issue in the GameCube target setup, separate from C/Rust STM API availability.

### Validation

Both nightly GameCube checks were attempted and failed before producing a usable STM crate or facade build, due to unresolved ACR IRQ names in `ogc-ipc`; the facade check also reported missing `check-cfg` registration warnings. No tests were run. The commands did not compile the requested crate/facade successfully. No hardware verification was performed. Only this review note was edited; the ledger was not changed.

## Independent re-review — close result fix

**Result: PASSED for the bounded fix.** Reviewer: `/root/reviewer_d`. Implementer: `/root`. Commit: `9f660d4`.

C `__STM_Close` ignores the event-hook release result, closes both descriptors, and returns the first negative descriptor-close result while continuing cleanup (`C-sources/libogc/libogc/stm.c:103–120`). Rust now ignores `_stm_release_event_hook_locked`'s result, attempts both descriptor closes, preserves close-error precedence, and clears state (`crates/core/ogc-stm/src/lib.rs:369–391`). The helper's state checks and registration clear behavior also match the C release function (`stm.c:141–153`; Rust `lib.rs:229–247`). The regression checks hook-release failure with successful closes and with close failure (`lib.rs:734–795`).

`cargo check -p ogc-stm --target powerpc-unknown-linux-gnu --tests` passed and compiled the test code; tests were not run. It emitted warnings for dead test-only symbols and mutable-static references. `git diff 9f660d4^ 9f660d4 --check` passed. No hardware verification was performed. The separate GameCube build issue through `ogc-ipc` remains open; this close fix does not resolve it.

## 2026-10-05 fix handoff: shared IOS and STM lifecycle

Implementer: `/root`. `ogc-stm` now uses `ogc-sys` for Hollywood revision reads, which allows `ogc-ios-sys` to depend on and reuse the same STM state without a dependency cycle. IOS initialization and shutdown now use STM's real event-hook registration and release paths. STM opens the immediate descriptor before opening eventhook, returns the real negative open error, and skips the eventhook open after immediate-open failure. Event-hook setup failure returns its ioctl error and cleans up opened descriptors. If descriptor cleanup fails, STM keeps the descriptor for a later close retry. These error returns intentionally improve C's behavior: C turns IOS_Open failure into zero, which IOS subsystem initialization treats as success, and ignores event-hook setup failures. The Rust path reports required-operation failures instead.

Regression coverage checks open order, error propagation, failed hook cleanup, and retained descriptors. `cargo check -p ogc-stm --tests --target powerpc-unknown-linux-gnu` passed; tests compiled but did not run. Independent static re-review is needed. The Wii-only API gate and GameCube dependency build remain separate open scope. No hardware verification was performed.

## 2026-10-05 completion review

`ogc-stm` is now `reviewed` for its own C surface. Wii-only API availability, repeated initialization, close result flow, and event-hook lifecycle are reconciled. Rust's stricter open/hook failure reporting and cleanup of resources that C can leak are accepted safety/error-preservation differences. The GameCube build failure described above is in the neighboring `ogc-ipc`/IRQ dependency path and does not keep STM itself incomplete.
