# libsystem Review

Status: static comparison completed for `ogc-system` and `ogc-system-sys` against `C-sources/libogc/libogc/system.c` and `C-sources/libogc/gc/ogc/system.h`.

## Confirmed mismatches

No confirmed behavioral or public ABI/layout mismatch was found in the paths reviewed. The matched paths include early `SYS_Init` ordering and IPL guards (`system.c:967-997`; `startup_plan.rs`), `SYS_PreMain` ordering (`system.c:999-1015`; `sys_pre_main_with_operations` in `ogc-system-sys/src/lib.rs`), main-loop conditions (`system.c:230-249`), exit routing (`system.c:251-265`), reset cleanup and platform terminal actions (`system.c:1040-1157`), arena allocation and bounds (`system.c:1185-1316`), framebuffer allocation size (`system.c:1347`; Rust `sys_allocate_framebuffer`), SRAM configuration accessors (`system.c:1508-1732`), and the exposed C structure layouts in `system.h` and `ogc-system-sys/src/lib.rs`.

## Uncertain differences

- The Rust crate models `TimeSpec.tv_sec` as `i64` and `tv_nsec` as `i32` (`ogc-system-sys/src/lib.rs:712-715`). The C API takes `struct timespec *` (`system.h:319-339`). The reference checkout does not include the toolchain's `time.h` definition, so this review cannot confirm the target C struct's field width or layout. Verify against the exact devkitPPC newlib headers before treating this as an ABI mismatch.
- The C `SYS_AllocateFramebuffer` directly returns `memalign(32, VIDEO_GetFrameBufferSize(rmode))` (`system.c:1347-1350`). Rust has an additional aligned static framebuffer pool fallback in `ogc-system/src/lib.rs` (`allocate_framebuffer` and `framebuffer_pool_alloc`) when the low-arena allocation fails. This changes allocation-failure behavior, but appears to be a deliberate Rust resilience addition, not an accidental parity claim.

## Intentional Rust additions

- Host snapshots, probe state, injectable startup/reset operations, and host implementations support verification without hardware. They are not part of the C public API.
- The high-level `ogc-system` crate adds safe wrappers and SRAM field helpers around the low-level interface. These do not alter the C struct layouts.
- Rust rejects invalid arena alignment/size and out-of-range wireless-ID channels. C assumes valid input (the wireless accessor indexes the four-element array directly); the Rust checks are defensive additions.

## 2026-10-04 implementation handoff

Implementer: `/root`. Independent static re-review is needed; the review thread handles it. The SRAM MSR lock path now saves both external-interrupt and floating-point enable state and restores those saved bits through `ogc_sys` helpers. `SYS_Init` records the first initialization stage where floating point is disabled, and `ogc-system` exposes that diagnostic to the LWP probe. This adds observability for the LWP context contract; it does not change initialization ordering or claim a C API match for the diagnostic.

The target library build and `cargo xtask build-dols` passed. The PowerPC `ogc-system-sys` test target does not compile because its existing tests have target cfg, callback ABI, and unsafe-call errors; those tests were not run. No hardware verification was performed.


## Independent re-review — initialization diagnostics and SRAM MSR restore

Reviewer: `worker_c`. Implementer: `/root` (commit `fd886ce`, with preceding shared changes recorded in `2c26784`). I inspected the complete `ogc-system` and `ogc-system-sys` diff and compared SRAM access with C `__SYS_LockSram`/`__SYS_UnlockSram` call paths and `SYS_Init` ordering in `../../C-sources/libogc/libogc/system.c:967-997,1508-1732`. The diagnostic records the first observed stage with MSR.FP clear and does not change startup operation order. The SRAM critical section now saves and restores both MSR.EE and MSR.FP while preserving unrelated current MSR bits. I found no confirmed mismatch in this handoff. The broader static-review scope and earlier uncertain ABI/allocation differences remain open as recorded above. I ran no checks for this static re-review; implementation validation is recorded in the handoff. No hardware verification was performed.


## 2026-10-05 Rust-owned Arena2 setter

Implementer: `/root`. `ogc-system-sys::sys_set_arena2_lo` no longer imports or calls the C `SYS_SetArena2Lo` symbol. On Wii it disables interrupts, updates the Rust-owned MEM2 low-boundary state, and restores interrupts, matching the reference setter's observable operation without linking C. Independent target compilation and re-review are still required.

## Independent re-review — Rust-owned Arena 2 setter (2026-10-05)

Reviewer: `worker_e`. On the Wii profile, `sys_set_arena2_lo` masks interrupts, stores the new address in `SYS_ARENA2_LO`, and restores the prior interrupt state (`crates/core/ogc-system-sys/src/lib.rs:3786-3800`). The Rust getter, size calculation, and low-arena allocator all use that same state (`:3778-3780,3829-3859`). This matches the bounded state-update and interrupt-preservation behavior of C `SYS_SetArena2Lo` (`../../C-sources/libogc/libogc/system.c:1252-1259`) within the Rust-owned runtime. It intentionally does not call or define C's private `__sysarena2lo` state; C `_sbrk_r` continues to use the C setter/getter pair (`sbrk.c:27-68`), so mixed Rust/C allocator coordination is not established by this port.

One target-availability mismatch remains in the surrounding Arena 2 API. C declares the Arena 2 low/high getters and setters and size function only inside `#if defined(HW_RVL)` (`../../C-sources/libogc/gc/ogc/system.h:410-419`). Rust exposes `sys_get_arena2_lo`, `sys_set_arena2_lo`, `sys_get_arena2_hi`, `sys_set_arena2_hi`, `sys_get_arena2_size`, and `sys_alloc_arena2_mem_lo` without a GameCube cfg (`crates/core/ogc-system-sys/src/lib.rs:3778-3859`); the GameCube branch can update the Rust atomic. This makes those Rust APIs available where the C API is absent. Keep this target-surface difference open unless the extra GameCube API is documented as an intentional Rust-only addition.

Result: the bounded Rust-owned setter update passes static comparison for its internal state and IRQ ordering. The crate remains open for the target-availability mismatch and for the mixed C/newlib boundary question. `cargo check -p ogc-system-sys --target powerpc-unknown-linux-gnu --lib` passed. This check does not prove final application symbol resolution or mixed-runtime behavior. No tests were run and no hardware verification was performed.

## 2026-10-05 Arena2 API and ABI fix handoff

Implementer: `notOrrytrout`, commit `fe1735c`. Arena2 APIs are now Wii-only, matching C's `HW_RVL` surface. `ogc-system-sys` owns the public `SYS_GetArena2Lo`, `SYS_SetArena2Lo`, `SYS_GetArena2Hi`, `SYS_SetArena2Hi`, `SYS_GetArena2Size`, and `SYS_AllocArena2MemLo` C ABI symbols instead of importing them from libogc. The exported ABI functions and Rust wrappers share the same Arena2 low/high state and guard boundary access with the existing IRQ save/restore path.

This resolves the prior mixed-boundary concern under the project rule that the C tree is reference-only. A linked newlib allocator that calls the public `SYS_*Arena2*` functions resolves those calls to the Rust definitions, so it shares the Rust boundary without an unresolved libogc C symbol. The private C `__sysarena2lo` variable is not part of the runtime and is not required. `SYS_AllocArena2MemLo` advances the same low boundary through the Rust-owned setter.

Independent re-review should verify Wii/GameCube API availability, exported symbol ownership at final link, IRQ preservation, and Arena2 allocation arithmetic. No hardware validation is claimed by this handoff.

## 2026-10-05 completion review

`ogc-system` is now `reviewed`. The inspected startup, reset, Arena, SRAM, alarm, and allocation paths have no confirmed non-safety mismatch. Arena2 publication is supplied by the Rust-owned `ogc-system-sys` ABI boundary, so no unresolved libogc C symbol is part of this result. Defensive pointer/allocation checks remain safety behavior. `ogc-system-sys` has a separate `needs_re_review` row for its recent Arena2 ABI change.

## 2026-10-07 font encoding report fix

The Dolphin `general_probe` artifact reported font encoding `0xFFFFFFFF`, outside the C API's only valid values, 0 and 1. `SYS_GetFontEncoding` in C returns a cached value only when it is at most 1; otherwise it recalculates using `VIDEO_GetCurrentTvMode() == VI_NTSC` and `_viReg[55] & 0x0002` (`../../../../C-sources/libogc/libogc/system.c:1352-1365`). Rust now uses the same VI status bit rather than the component-cable bit, rejects invalid cached values, and falls back to the calculated value if an invalid concurrent cache value is observed. A regression test covers valid and invalid cached values. `cargo run -p xtask -- build-dolphin-probes` compiled the updated production probe. Target unit tests did not run: the custom `no_std` PowerPC target has no `test` crate, and the Dolphin validation attempt stopped in IOS pre-main before emitting a probe page. A passing runtime probe is still required.

## Wii System Menu return remains open — 2026-10-15

AESND run `1792110541-513590000` sent a final report, returned from report-network
cleanup and watchdog cleanup, and displayed `INFO exit entering System Menu
reset`. The user reported advancing VI frames and no menu return or reset
callback marker. DSP shutdown runs before those callbacks. The current DOL
adds DSP wait markers through the existing reset debug hook. The actual menu
return is unverified and must not be inferred from a request log or a complete
probe report. No reset sequence or persistent launch data was changed by this
diagnostic addition.

The user's C Priiloader setting is `Return To: Autoboot`, with autoboot set to
HBC. Priiloader handles `TYPE_RETURN` by applying this setting, so a successful
libogc System Menu launch can return to HBC on this console. New reset trace
events record the return-title launch result and IOS subsystem shutdown result
when these operations return. A successful title launch transfers control, so
it will not emit a result event. AESND run `1792111385-301512000` still showed
the final probe page after it logged the System Menu return request. It had 7
failed checks (2 DSP sample checks, 4 stream sample checks, and page screenshot
delivery) and does not prove whether the return-title launch or later IOS
cleanup failed.

The next run, `1792111703-453368000`, stopped in DSP shutdown stage 2. The
screen showed `INFO reset stage=7 priority=2 result=2052`; priority 2 is the
mailbox wait, and 2052 (`0x0804`) is the control-register value. The C and Rust
shutdown sequences use the same mailbox busy wait. A new diagnostic event
records the mailbox register value before that wait. The exact mailbox value
and System Menu handoff result remain unknown because the report connection
reset before the final report arrived.

Run `1792112482-437835000` uploaded build
`e832fe3d18f99b0a524312b5568e005fe9866840efc852ef297ac5c0a128ae3b`. The host
accepted no report. The user's screen photo shows page 11 with 46 passes, 8
failures, and `FAIL final report sent to host`. This confirms the probe reached
its final summary, but it does not provide the DSP mailbox value. The summary
path returned before the requested System Menu reset when report delivery
failed. The probe now records that error, keeps the recovery watchdog armed,
and continues to the requested exit path. The reset result remains unverified
until a new hardware run returns its report or screen diagnostics. The revised
DOL `b0ffd2fecec448cdb566cd893da7376631ee20c6a29ac87f9726c8bf66547239`
built with `cargo xtask build-agent-probes`. Two standalone tests of the shared
report-failure exit decision passed. The focused Dolphin run started IOS
pre-main but produced no probe output before it stopped; it is inconclusive and
does not validate this agent-report exit path.

Run `1792113309-473420000` returned a complete report, but the upload endpoint
did not return after the requested System Menu reset. The user photo shows
stage 2's mailbox value `32881` (`0x8071`); bit `0x8000` is still set. The frame
counter and step marker advanced, so the CPU was polling rather than frozen.
The reset path is waiting for the DSP mailbox busy bit to clear. The probe had
also canceled its recovery watchdog before entering this wait. The probe now
keeps that watchdog armed during a requested system reset. Hardware validation
of the watchdog return remains open.
