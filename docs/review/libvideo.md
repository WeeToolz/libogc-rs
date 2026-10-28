# libvideo Review

Date: 2026-10-01

Status: `ogc-video-sys` and `ogc-video` reviews are complete for the recorded scope. The `ogc-video` wrapper fixes for unsigned `console_set_window` clipping and `console_init_ex` buffer reuse were independently accepted in commit `76d00e4`. The two findings below are the original findings and are resolved by the fix pass. No hardware verification was performed.

## Scope

- `crates/hardware/ogc-video`
- `crates/hardware/ogc-video-sys`
- C references: `C-sources/libogc/libogc/video.c`, `C-sources/libogc/libogc/console.c`, and `C-sources/libogc/gc/ogc/video.h`

## Reviewed behavior

- Preferred mode tables and current-mode selection, including NTSC, PAL, MPAL, EURGB60, and progressive-mode choices.
- Current television standard, scan line, next field, retrace waits and callbacks, framebuffer sizing, and framebuffer state publication.
- Console initialization, window bounds, metrics, position, and clear behavior.

## BUG FIX

`consoleInit` in C performs framebuffer allocation and video setup only on its first call. Rust repeated those operations on every call, allocating more framebuffer storage and resetting the active video state. Rust now runs that setup once and keeps the per-call console selection and clearing behavior.

`VIDEO_SetBlack` in C updates the vertical VI registers immediately. Rust previously marked the value dirty and waited for a flush and retrace. Rust now updates the vertical registers under the interrupt guard and clears the pending black update.

## Validation boundary

`cargo xtask check-group hardware` passed. The full-file rustfmt check reports pre-existing formatting differences in the crate-level `compile_error!` attributes; no formatting differences were reported for the changed code. The repository inventory check is currently stale at `ogc-gx::get_tex_buffer_size` due to separate uncommitted GX edits. This review is static and does not record hardware verification.

## Safe wrapper review: `ogc-video` only

This pass reviewed only `crates/hardware/ogc-video/src/lib.rs`. The `ogc-video-sys` implementation remains covered by its earlier review and was not re-reviewed here. C references for this wrapper pass are `../../C-sources/libogc/gc/ogc/video.h`, `../../C-sources/libogc/libogc/video.c`, `../../C-sources/libogc/gc/ogc/console.h`, and `../../C-sources/libogc/libogc/console.c`.

### Wrapper-to-C operation coverage

The VI wrappers delegate to the matching C operations through `sys`: `init`/`VIDEO_Init`, `flush`/`VIDEO_Flush`, `set_black`/`VIDEO_SetBlack`, `get_next_field`/`VIDEO_GetNextField`, `get_current_line`/`VIDEO_GetCurrentLine`, `get_current_tv_mode`/`VIDEO_GetCurrentTvMode`, `configure`/`VIDEO_Configure`, `get_frame_buffer_size`/`VIDEO_GetFrameBufferSize`, `clear_frame_buffer`/`VIDEO_ClearFrameBuffer`, `wait_vsync`/`VIDEO_WaitVSync`, `set_framebuffer`/`VIDEO_SetFramebuffer`, `set_next_framebuffer`/`VIDEO_SetNextFramebuffer`, `set_next_right_framebuffer`/`VIDEO_SetNextRightFramebuffer`, `have_component_cable`/`VIDEO_HaveComponentCable`, `get_preferred_mode`/`VIDEO_GetPreferredMode`, `get_video_scan_mode`/`VIDEO_GetVideoScanMode`, and the pre/post callback setters (`../../C-sources/libogc/gc/ogc/video.h:58-208`; `../../C-sources/libogc/libogc/video.c:2531-2975`; wrapper `crates/hardware/ogc-video/src/lib.rs:298-528`). `get_current_framebuffer` and `get_next_framebuffer` are re-exported from `sys` and map to the C getters (`../../C-sources/libogc/libogc/video.c:2531-2539`; `crates/hardware/ogc-video-sys/src/lib.rs:673-680`). The wrappers preserve the corresponding values and callback returns. The paired sys review covers the hardware implementation.

Console layout types `ConsoleFont`, `PrintConsole`, `ConsolePrint`, and `DebugDevice` match the C field order, callback ABI, and enum values (`../../C-sources/libogc/gc/ogc/console.h:110-189`; wrapper `crates/hardware/ogc-video/src/lib.rs:90-167`). `console_get_default`, `console_select`, `console_set_font`, `console_set_window`, `console_get_metrics`, `console_get_position`, `console_clear`, and `console_init` correspond to `consoleGetDefault`, `consoleSelect`, `consoleSetFont`, `consoleSetWindow`, `CON_GetMetrics`, `CON_GetPosition`, `consoleClear`, and `consoleInit` (`../../C-sources/libogc/libogc/console.c:1123-1206,1210-1245`; wrapper `crates/hardware/ogc-video/src/lib.rs:620-725,837-888`). `console_init_ex` corresponds to `CON_InitEx` (`../../C-sources/libogc/libogc/console.c:1099-1120`; wrapper `crates/hardware/ogc-video/src/lib.rs:893-941`). `console_write`, `console_blit_rgba`, and `console_init_framebuffer` are Rust additions without one-to-one C entry points. `wait_vsync_timeout_us`, `VideoSnapshot`, `VideoHardwareProof`, and the register-match helpers are diagnostics/extensions rather than C `video.h` APIs. The crate does not expose `CON_EnableGecko` or `consoleDebugInit` as safe wrapper functions; this report does not claim full coverage of those separate C console facilities.

### Original confirmed wrapper findings (resolved below)

1. `console_set_window` truncates unsigned coordinates and sizes to signed 32-bit values before clipping. C keeps these parameters unsigned (`../../C-sources/libogc/libogc/console.c:1224-1241`; Rust `crates/hardware/ogc-video/src/lib.rs:664-697`). For example, `x = 0x80000000` passes C's unsigned bounds check and returns without changing the console. Rust casts it to `-2147483648`, passes its signed bounds check, and stores a negative `window_x`. The same truncation affects `y`, `width`, and `height`. This is a confirmed mismatch for valid `u32` arguments above `i32::MAX`.
2. Repeated `console_init_ex` calls cannot release the prior allocation on PowerPC. Rust calls `free_aligned`, but that function is a no-op for the PowerPC target (`crates/hardware/ogc-video/src/lib.rs:255-258`); allocations instead advance a fixed 2 MiB bump pool (`crates/hardware/ogc-video/src/lib.rs:31-32,271-295`). C `CON_InitEx` frees the previous `_console_buffer` before allocating its replacement (`../../C-sources/libogc/libogc/console.c:1099-1118`). Repeated reinitialization therefore consumes another pool allocation each time in Rust and can exhaust the fixed pool, while C reuses the prior storage.

### Intentional safety differences and review limits

- Several raw-pointer C operations use non-null typed references in Rust or return without action for null pointers. These checks make invalid calls safer and are not reported as mismatches.
- The Rust diagnostics and console extensions listed above have no direct C function to compare. `CON_Init` also has no direct safe wrapper with its original argument list; Rust's `console_init_framebuffer` is a different, documented extension.
- This is a source review only. No tests were run and no hardware behavior was checked. The two wrapper findings are assigned to the later fix pass; this does not change the already reviewed `ogc-video-sys` row.

## Fix pass: `ogc-video` wrapper

Both findings above have a fix in `crates/hardware/ogc-video/src/lib.rs`, labeled `BUG FIX`. `console_set_window` now keeps `x`, `y`, `width`, and `height` unsigned through the same bounds and wrapping arithmetic used by C, then casts only when it stores the values into the C `int` fields. Regression tests cover an origin above `i32::MAX` and a large width that C clips to the console boundary.

`console_init_ex` now reuses its existing console allocation when the capacity can hold the next console buffer. When a larger allocation is needed, the PowerPC bump-pool allocation records its range and rewinds the pool cursor if that allocation is still the pool tail; otherwise the existing buffer is replaced and the new capacity is retained for later calls. `console_init_framebuffer` uses the same release path. This prevents repeated same-size and smaller reinitializations from consuming additional pool space while preserving the fixed-pool allocator used by the crate.

`cargo check -p ogc-video --target powerpc-unknown-linux-gnu` and `cargo check -p ogc-video --tests --target powerpc-unknown-linux-gnu` passed. The test check compiles the new regression tests, but the tests were not run because they target PowerPC. `cargo test -p ogc-video --lib` cannot build for the host because its PowerPC-only dependencies reject the host target. The test compilation also required correcting existing test callback ABIs and placing an unsafe frame-buffer-size call in an unsafe block. `git diff --check` passed. The independent re-review below accepted the fix; no hardware behavior was tested.

## Independent re-review: `ogc-video` wrapper

Reviewer: `worker_b`. Implementer: `/root/dvd_review` (commit `76d00e4`). I compared the complete fix diff with C `consoleSetWindow` and `CON_InitEx` (`../../C-sources/libogc/libogc/console.c:1099-1118,1224-1241`). The unsigned bounds and wrapping clipping in `console_set_window` now match C, including values above `i32::MAX`. The console buffer reuse and PowerPC pool-tail release preserve the initialized buffer capacity and prevent repeated same-size initialization from consuming new pool space. The null-mode cleanup and allocation-failure paths were also checked. I found no remaining confirmed mismatch in the recorded wrapper scope. The regression tests compile for the PowerPC target as recorded above; they were not run. No hardware verification was performed.

## 2026-10-04 standard render-mode constants handoff

Implementer: `/root`. Independent static re-review is needed; the review thread handles it. `ogc-video-sys` now exports NTSC progressive, EURGB60 progressive/interlaced, and PAL 528 interlaced render-mode constants based on the canonical C `video.c` mode table. A regression test checks fixed expected mode, framebuffer, field, origin, and filter values. Existing video findings remain open; this change does not close the full sys review. Target library and DOL builds passed. The target test code compiled in the crate check, but tests were not run. No hardware verification was performed.


## Independent re-review — standard render-mode exports

Reviewer: `worker_c`. Implementer: `/root` (commit `fd886ce`). I compared the four new exported mode constants and their fixed-value regression assertions with the C mode table in `../../C-sources/libogc/libogc/video.c:331-365,1243-1276,1408-1474` and the mode structure in `../../C-sources/libogc/gc/ogc/video.h`. The TV mode values, field/framebuffer modes, PAL 528-line geometry and origin, and deflicker filters match the corresponding C table entries. These are Rust API constants and do not claim new C linker symbols. I found no confirmed mismatch in this handoff. The broader VIDEO parity findings remain open. I ran no checks for this static re-review; implementation validation is recorded in the handoff. No hardware verification was performed.

## 2026-10-05 VI origin arithmetic handoff

Implementer: `/root`. Rust now doubles non-interlace `viYOrigin` with 16-bit wrapping arithmetic in `adjusted_display_y_origin`, matching C's assignment to the `u16 HorVer.dispPosY` field (`../../C-sources/libogc/libogc/video.c:56-72,2648-2655`). A regression assertion checks `0x8001 * 2` wraps to `2`. `cargo check -p ogc-video-sys --tests --target powerpc-unknown-linux-gnu` passed; tests were compiled, not run. Independent static review is needed. Broader VIDEO parity remains open.

## Independent static re-review: `ogc-video-sys` VI origin wrapping (2026-10-05)

**Result: PASS for the non-interlace origin arithmetic fix only.** Reviewer: `worker_f`. Implementer: `/root`; commit `7001d38`.

C assigns `rmode->viYOrigin` to the `u16 HorVer.dispPosY` field and, when the low two mode bits equal `VI_NON_INTERLACE`, left-shifts it by one before storing it (`../../C-sources/libogc/libogc/video.c:2643-2646`; the destination field is declared at `video.c:56-72`). C's final conversion to `u16` keeps the low 16 bits. Rust's `adjusted_display_y_origin` uses the same mode condition and `u16::wrapping_mul(2)` (`crates/hardware/ogc-video-sys/src/framebuffer_state.rs:27-33`), which has the same result, including `0x8001 * 2` becoming `2`. Interlaced and progressive modes that do not equal `VI_NON_INTERLACE` leave the origin unchanged. The helper's result is used by `get_next_field` (`ogc-video-sys/src/lib.rs:885-892`). The fixed-value regression checks the overflow case and unchanged interlaced case (`:1286-1290`). I found no mismatch in this bounded arithmetic change.

### Accepted safety deviations

None identified in this arithmetic change.

### Uncovered scope and validation limits

This pass did not compare the full `ogc-video-sys` mode, timing, register, retrace, or framebuffer behavior. The previous render-mode constant review covers only those constants; broader VIDEO system scope remains open. The implementer reports that PowerPC test code compiled; tests were not run. I did not run a fresh build or tests. No hardware verification was performed.

## 2026-10-05 `ogc-video-sys` completion review

The sys crate is now `reviewed`. The earlier note already completed the VIDEO sys/API timing, register, framebuffer, and mode-table comparison; the later standard render-mode additions and non-interlace VI-origin fix were independently reviewed. No confirmed non-safety mismatch remains in the recorded C VIDEO surface.

## 2026-10-08 retrace queue race

C `VIDEO_WaitVSync` masks interrupts across its retrace count read and queue sleep (`C-sources/libogc/libogc/video.c`, `VIDEO_WaitVSync`). Rust previously performed a second count check with interrupts enabled and assumed that `queue_signal` retained an early wake. The production `ogc-lwp-driver::queue_signal` only wakes an existing waiter. An IRQ between the count check and queue entry could therefore be lost.

`ogc-video-sys::wait_v_sync` now uses the shared `retrace_wait::sleep_if_unchanged` engine to mask interrupts across the count check and waiter registration. The retrace handler broadcasts to registered waiters. If the queue is closed, the caller waits for an actual retrace with the existing polling path. Shared-engine tests cover an interrupt raised between the count check and registration, an already advanced counter, and a returned sleep failure with restored interrupt state. These tests run directly from the production module on the host.

This is a confirmed source gap. It is not proof that this race caused the latest Wii stall: that run reached `GX XFB HAS IMAGE`, while the photo showed a later repeated input label. Independent review and real Wii validation remain pending.

## Independent VI queue fix re-review — 2026-10-15

Reviewer: `/root/review_video`. Result: **PASS for the retrace queue fix**.
The interrupt mask now covers the retrace count check and sleep registration,
matching C `VIDEO_WaitVSync`. The previous note's claim that the C API was not
reviewed is stale. Shared-engine tests pass. This is a static/source review;
no Wii run was performed by the reviewer. Keep physical startup validation in
the hardware register, not as an open source re-review.
