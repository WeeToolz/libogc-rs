# libmodplay Review

Date: 2026-07-02

Status: phase 0 reviewed, ABI tooling preserved, hardware playback still open.

## Scope

- `crates/media/modplay`
- `crates/abi/modplay-capi`

## Current implementation

`modplay` contains the shared tracker data and frequency table behavior. `modplay-capi` provides the C-compatible stateful player surface, including MOD loading from caller-owned bytes or files, runtime channel state, SFX channel allocation, playback wrappers, time requests, mixer entry points, and callback dispatch.

The C API supports both caller-owned buffers via `mod_set_mod` and owned buffers created by `mod_load`. Free paths distinguish those ownership modes so caller memory is not released by the library while internally allocated module data is cleared.

## Verified coverage

- Frequency table byte order, table length, and anchor values are pinned by `crates/media/modplay/tests/tables.rs`.
- Parser tests cover 31-instrument MOD metadata and pattern parsing, plus fallback handling for the 15-instrument legacy layout.
- State tests cover SFX channel clamping, shift value behavior, arpeggio processing, public wrapper calls, pattern break behavior, pause/request/mixing time paths, and callback invocation.
- Ownership tests cover freeing caller-owned module bytes separately from owned buffers loaded from disk.
- Mixer coverage exercises `mod_player` state mutation and callback flow; host execution is preserved only for `xtask` ABI/layout tooling.

## Remaining validation

- ABI/tooling checks do not prove audible output quality, DMA timing, or interaction with the Wii audio interrupt path.
- Validate tracker playback on Wii with real-world MOD files, malformed fixtures, looping, effects, and callback behavior.
- Treat broader corpus and external ABI policy as post-validation release hardening.

## Phase 0 position

The phase 0 bar is met for documented API behavior, ABI layout coverage, and regression coverage. Hardware playback on Wii remains a validation gate, not an assumed property of host tooling.

## Independent static review (2026-10-03)

Reviewer: `reviewer_d`. Implementer recorded in the implementation history: `notOrrytrout`. Relevant implementation commits include `621fcab` (audio DMA backend) and `8864287` (callback ABI). C reviewed: `../../C-sources/libogc/libmodplay/gcmodplay.c`, especially `player`, `dmaCallback`, and `SndBufStart`. Rust reviewed: `crates/abi/modplay-capi/src/lib.rs`, especially `backend_start_state`, `start_audio_dma`, and `modplay_dma_callback`.

### Finding: audio mixing runs in the DMA callback in Rust

- C `player` waits on `player_queue` and invokes `sndBuffer.callback` from the LWP worker thread (`gcmodplay.c:42-64`). C `dmaCallback` switches buffers and signals that worker (`gcmodplay.c:92-108`). `SndBufStart` initializes two silent buffers, starts the worker, then starts DMA (`gcmodplay.c:136-170`).
- Rust `start_audio_dma` invokes the mix callback synchronously for both buffers before it starts DMA (`modplay-capi/src/lib.rs:1855-1865`). Later, `modplay_dma_callback` invokes the mixer directly while handling the DMA callback (`modplay-capi/src/lib.rs:1871-1898`). No worker thread or deferred work queue exists in this path.
- This changes both callback execution context and initial buffer contents/timing. In particular, a user `MOD.callback` reached by `MOD_Player` runs on the C worker but can run in the Rust DMA interrupt callback. This can cause interrupt-context restrictions and changes callback timing. The behavior does not match C and must be addressed or explicitly accepted as a deliberate platform-specific divergence before approval.

Static review found no issue with the recent callback function-pointer ABI correction: C declares both callback fields as ordinary C function pointers (`gc/modplay/modplay.h` and `gc/gcmodplay.h`); Rust now uses `unsafe extern "C" fn` in `Mod.callback` and `ModSndBuf.callback` (`modplay-capi/src/lib.rs:180,195`), and the exported mix callback uses that ABI (`:1514`).

Validation: source comparison only. No tests were run. No hardware verification was performed. Result: `needs_changes`; the audio callback scheduling mismatch remains open for both `modplay` and `modplay-capi`.

## Independent current-source re-review

Reviewer: `/root/reviewer_d`. Backend commits reviewed in context: `621fcab`, `373426f`, and `8864287`. The callback-context mismatch remains. C's `player` waits on an LWP queue and runs the mixer on a worker (`C-sources/libogc/libmodplay/gcmodplay.c:42–64`). The DMA callback only toggles buffers, starts the next fixed-size transfer, and signals the worker (`:92–108`). `SndBufStart` zeros both buffers before starting DMA (`:136–170`). Rust invokes callbacks synchronously for both buffers before DMA starts, then invokes the mixer from the DMA interrupt path (`crates/abi/modplay-capi/src/lib.rs:1831–1901`; mixer path `:1514–1523, 1654–1677`; audio IRQ `crates/hardware/ogc-audio-sys/src/lib.rs:373–390`). This changes callback context and advances the mixer twice before initial DMA, while C begins with silent buffers.

Rust also derives transfer length from samples, channels, and sample width (`modplay-capi/src/lib.rs:1835–1845, 1859, 1863, 1882–1897`); C always transfers and zeroes `SNDBUFFERSIZE` (5760 bytes; `gcmodplay.c:15, 55, 80, 102, 148–162`). Rust shutdown clears flags, stops DMA, and removes its callback without joining a worker (`modplay-capi/src/lib.rs:1815–1828`); C disables DMA, deregisters its callback, signals and joins the worker (`gcmodplay.c:173–186`).

The callback ABI correction is valid: C uses ordinary function pointers, and Rust now uses `unsafe extern "C" fn`. `cargo check -p modplay-capi -p modplay --target powerpc-unknown-linux-gnu` passed. The test targets failed to compile because of existing `*const u8` versus `*const i8` mismatches (`modplay-capi/src/lib.rs:2288`; `tests/state.rs:192,245`); tests did not run. No hardware verification was performed.

## BUG FIX handoff: match C audio worker scheduling

Implementer: `/root/worker_c`. Independent static re-review is needed; the review thread handles it. This change addresses C `gcmodplay.c` worker scheduling, initial silence, DMA length, and worker shutdown order. No hardware verification was performed.

Rust now creates an LWP queue and worker before it starts DMA. It zeros and flushes both 5760-byte buffers through a helper shared with the regression test. The DMA callback now switches buffers, starts the next fixed-size 5760-byte transfer, and signals the worker; only the worker invokes the mix callback after it wakes. The worker reports queue-wait failure to the backend, clears the active playback flag so DMA does not restart, and exits without mixing. A later stop joins the exited worker and closes its queue, then returns an error to the caller while setting the public `ModPlay.playing` field to stopped. The backend keeps the worker-failure result until cleanup completes. Start returns an error if queue or thread creation fails, with allocated queue state cleaned up.

Shutdown stops DMA, removes the DMA callback, signals and joins the worker, then closes its queue. If join fails, Rust returns an error and retains the queue, thread, and stack state so a later stop can retry. `stop_state` and fallible unload propagate this error. Initialization attempts to stop and join an existing player before it clears player state.

Added production-shared worker-loop tests for mixing only after a signal and stopping before a post-stop mix, fixed transfer length and buffer rotation, wait-failure state and result reporting, zero contents in both initial DMA buffers, shutdown ordering, and join-failure queue retention. Corrected existing NUL-path pointer casts in the `modplay-capi` tests so the test targets compile against the crate's `*const i8` API.

Validation passed: `cargo test -p modplay-capi` (37 tests), `cargo test -p modplay` (2 tests), `cargo check -p modplay-capi -p modplay --target powerpc-unknown-linux-gnu`, `cargo check -p modplay-capi --tests --target powerpc-unknown-linux-gnu`, formatting, and `git diff --check`. Host tests exercise the production-shared scheduling and shutdown decisions; they do not execute the PowerPC LWP or audio hardware paths. No hardware verification was performed.

### Dolphin DOL regression probe

Added `modplay_probe`, a production-path DOL probe. It loads a valid caller-owned MOD image, starts playback through `modplay-capi`, and records the production `MOD.callback` context and `mixingbuflen`. It checks that the public playback state changes on start and stop, mixing occurs on a worker LWP, each reported buffer is 5760 bytes, stop joins the worker and prevents later callbacks, restart works, and unload leaves caller-owned bytes intact. The production-shared host tests cover worker wake-up and silent initial buffers; the DOL probe does not treat an immediate post-start callback count as a contract because the first DMA completion may occur before the probe samples it. Dolphin cannot force a real LWP queue-wait failure without replacing the production queue, so the error result and cleanup path are covered by the production-shared host worker test.

Build and run commands:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe modplay_probe
```

Result: both commands passed on 2026-10-04. Dolphin `2603-415` reported `modplay_probe` as `pass` (17 passes, 0 failures). This is emulator evidence only, not hardware verification. The full probe build also required two probe-infrastructure compile corrections: use `unsafe extern "C" fn` for the reset callbacks stored in C-ABI function pointers, and handle `FatError::AlreadyExists` in the exhaustive mount-status match. These do not change the ModPlay implementation.

### Worker error handling follow-up

Implementer: `/root/worker_c`. Independent static re-review is needed; the review thread handles it.

On an LWP queue-wait failure, the worker now sets `PLAYER_WORKER_FAILED` and clears `SND_PLAYING`. This makes the DMA callback stop scheduling transfers. The next stop joins the exited worker, removes the callback, closes the queue, clears public `ModPlay.playing`, and returns an error. Stop reads the worker-failure flag after a successful join, so an error raised while join waits cannot be lost. If join fails, the error flag, queue, thread handle, and public active state remain so a later stop can retry safely. The probe confirms the public state on normal start/stop. Dolphin cannot cause a real queue-wait failure without replacing the production queue; host coverage uses the same worker result and failure-state helpers used by production and forces a failure during join. It also verifies both production-start buffers are zero before DMA.

Follow-up validation passed: `cargo test -p modplay-capi` (37 tests), PowerPC-target `cargo check -p modplay-capi` and `cargo check -p modplay-capi --tests`, `cargo xtask build-dolphin-probes`, and `cargo xtask dolphin-validate --no-build --probe modplay_probe` (Dolphin `2603-415`, 17 passes, 0 failures). No hardware verification was performed.

Race-review correction: the first follow-up read `PLAYER_WORKER_FAILED` before joining. Independent reviewer `/root/modplay_review` found that a queue-wait failure during the join could then be cleared and reported as success. The production shutdown helper now samples the flag only after a successful join and queue cleanup; a regression sets the failure during the join closure and checks that shutdown reports the error. Implementer: `/root/worker_c`. A further independent static re-review is needed.

## Independent re-review: report worker failure after join (`e72b056`)

**Result: PASS for this bounded race fix.** Reviewer: `worker_a`. Implementer: `worker_c`.

C joins its audio worker during `SndBufStop` before it returns (`../../C-sources/libogc/libmodplay/gcmodplay.c:173–186`). Rust now samples `PLAYER_WORKER_FAILED` only after a successful join and queue cleanup, so a queue-wait error that occurs while the join waits is not lost. A failed join still returns before closing the queue or releasing the thread state. The regression makes the join closure set the worker-failed flag, then checks that shutdown observes it and returns the error result.

I inspected the complete production and test diff. No checks or tests were run during this re-review; the implementer's note records 37 passing `modplay-capi` tests, two passing `modplay` tests, successful PowerPC checks, and a passing Dolphin probe. No hardware verification was performed. Other ModPlay scheduling and audio differences recorded earlier remain open.

## 2026-10-05 completion review

The current ModPlay source scope is closed as `reviewed` for both `modplay` and `modplay-capi`. The later worker implementation resolves the earlier callback-context, silent-buffer, fixed-transfer-length, and shutdown/join findings. Current parser/effect/ownership and public C API/ABI coverage has no confirmed non-safety mismatch. Rust-only mixing/request timing diagnostics are not C public API. Wii hardware playback remains a validation task, not a source-parity blocker.
