# Static C Parity Review

Status: the earlier GameCube-transport finding is superseded at HEAD `4c36294`; the sys crate now contains a direct GameCube DI-register backend alongside the Wii `/dev/di` path. Hardware/state-machine validation remains outside scope.

## Scope

- Public compatibility wrappers in `crates/compatibility/ogc-dvd/src/lib.rs`.
- DVD command block layout and operation paths in `crates/compatibility/ogc-dvd-sys/src/lib.rs`.
- C reference: `../../C-sources/libogc/gc/ogc/dvd.h` and `../../C-sources/libogc/libogc/dvd.c`.

The Rust facade delegates its public functions to `ogc-dvd-sys`; it does not add a separate command implementation.

## Re-reviewed fix

## BUG FIX: `DVD_ReadAbsAsyncForBS` submits the C command

Rust `read_abs_async_for_bs` previously delegated to the normal absolute read and set `cmd` to `0x0001`. C sets `cmd` to `0x0004`, clears `txdsize`, stores the callback, and submits at priority 2 for `DVD_ReadAbsAsyncForBS` (`../../C-sources/libogc/libogc/dvd.c:2230-2242`). The fix now selects `ReadCommand::ForBs`, whose value is `0x0004`, while the ordinary API continues to select `0x0001` (`crates/compatibility/ogc-dvd-sys/src/lib.rs:49-61,1088-1138`). `read_async_cmd` stores the selected command and read parameters in `DvdCmdBlk` before it queues a `RequestKind::Read` at the requested priority. The shared `submit` path sets `state=BUSY`, clears transfer counters, stores the callback, and queues the request (`crates/compatibility/ogc-dvd-sys/src/lib.rs:1099-1119,693-743`). This confirms the command field and the shared queue-submission behavior against C. The compile-time assertions check `0x0004` and `0x0001` (`crates/compatibility/ogc-dvd-sys/src/lib.rs:1390-1393`).

At the first re-review, the Rust worker used the same generic `/dev/di` read path for all read commands and did not use the request's command value. The second fix below now carries that value into the worker and adds the command-specific Wii recovery path.

## Second BUG FIX: BS read failures use Wii motor-stop and cover-close recovery

C routes terminal read errors through its error-state path. For command `0x0004`, the retry callback stops the motor, marks the command for recovery, and waits for the cover-close event (`../../C-sources/libogc/libogc/dvd.c:1028-1049,1737-1739,1661-1667`). On cover close, C clears queued commands and completes the active BS request with result `-4` (`../../C-sources/libogc/libogc/dvd.c:1633-1646`).

The Rust request now carries the selected command into the worker (`crates/compatibility/ogc-dvd-sys/src/lib.rs:179-199,1099-1119`). After a failed DI read, `recover_bs_read` runs only for command `0x0004`: it checks cancellation, stops the motor, and polls Wii DI cover status until the cover closes (`crates/compatibility/ogc-dvd-sys/src/lib.rs:304-336,504-568`). It then completes the active block with `DVD_STATE_COVER_CLOSED` and `DVD_ERROR_COVER_CLOSED`, matching C's state and callback result. It also removes queued work and completes those Rust requests with the same terminal result, so their command blocks do not remain busy (`crates/compatibility/ogc-dvd-sys/src/lib.rs:367-402`). Ordinary absolute reads do not enter this recovery path.

This implements the command-specific behavior that the Wii `/dev/di` backend can support. It does not reproduce the C interrupt state machine's complete hardware error categorization. At that stage, the separate GameCube transport mismatch below remained open; the current-source status section supersedes it.

## Resolved independent re-review finding: BS recovery did not wait for a cover-close event

Implementer: `/root/card_review` (commit `5fc9df7`). Re-reviewer: `thread-current/card_review`.

The fix calls `di::get_cover_register` and treats `DVD_COVER_DISC_INSERTED` as `cover_closed` (`crates/compatibility/ogc-dvd-sys/src/lib.rs:544-551`). When a disc is already inserted after a read failure, this check returns true immediately, and the active BS request completes with `DVD_ERROR_COVER_CLOSED` without waiting for the user to open and close the cover. The test `failed_bs_read_stops_motor_and_waits_for_cover_close` models two successive Boolean values, but it does not test that the code waits for the cover-close event when the inserted bit is already set (`crates/compatibility/ogc-dvd-sys/src/lib.rs:1401-1420`).

C instead calls `DVD_LowWaitCoverClose`, which enables the cover interrupt and returns through `__dvd_statemotorstoppedcb` only after that interrupt (`../../C-sources/libogc/libogc/dvd.c:988-996,1661-1667,2085-2094`). The Wii DI service already uses the `DVD_WAITFORCOVERCLOSE` ioctl as a distinct asynchronous mount stage (`crates/services/ogc-di/src/lib.rs:578-584,612-618`). A status snapshot of the inserted-disc bit is not equivalent to waiting for that event. The fix did not yet match C's BS recovery behavior. The follow-up fix below addresses this mismatch; independent re-review is still required.

## Superseded historical finding: GameCube support was absent from the Rust transport

The Rust sys crate explicitly implements its operations through Wii `/dev/di` (`crates/compatibility/ogc-dvd-sys/src/lib.rs:3-7,16-18,286-318`). The `ogc-di` service opens an IOS device with `ogc_ipc::open_file` and sends ioctl requests (`crates/services/ogc-di/src/lib.rs:16-18,830-847`). It has no transport abstraction or GameCube backend. The C implementation drives the console DVD interface directly, including DI registers and DVD firmware operations (`../../C-sources/libogc/libogc/dvd.c:31-48,2526-2541`). The crates permit PowerPC builds, including GameCube, but there is no GameCube transport behind the current Rust API. Supporting it requires a separate direct DI register and interrupt-state-machine backend; it cannot be enabled by selecting a mode in the existing IOS-only backend. This was the confirmed platform mismatch at the time of the audit; the current-source transport section below supersedes it. No hardware result is claimed.

## Intentional Rust behavior differences / review notes

- Rust validates null pointers, buffer alignment, offsets, and priorities in several submission paths. C often dereferences the command block directly or leaves validation to the command engine. These checks prevent invalid calls and are not reported as parity bugs here.
- Cancellation differs: C `DVD_CancelAllAsync` pauses the DVD queue and returns success (`../../C-sources/libogc/libogc/dvd.c:2343-2352`); Rust advances a cancellation generation and cancels queued or active requests (`crates/compatibility/ogc-dvd-sys/src/lib.rs:1209-1253`). The Rust API also invokes the supplied cancel callback. This is a deliberate implementation difference that changes observable timing and callback behavior.
- Rust async callbacks are delivered by its LWP worker (`crates/compatibility/ogc-dvd-sys/src/lib.rs:3-7,636-658`). C completion callbacks are driven by the DVD interrupt state machine. This changes observable callback timing and serialization; consumers that depend on timing need separate review.

## Validation limits

Validation performed for this fix pass: `cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled the command-value assertions plus regression tests for command propagation, BS failure recovery, ordinary-read behavior, and cancellation. `cargo test -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu --no-run` could not link because the host linker does not support PowerPC linker options; no PowerPC emulator is installed, so the tests could not run. The custom Wii target also has no Rust `test` crate. No hardware verification was performed. This report does not claim exhaustive review of every retry/error-state transition in the C DVD interrupt state machine.

## Follow-up BUG FIX handoff: BS recovery waits for cover open and close

Implementer: `worker_b`. Independent re-review owner: `thread-current/reviewer_1`. C sets the cover interrupt mask in `DVD_LowWaitCoverClose`, then completes the BS request from the cover-close interrupt (`../../C-sources/libogc/libogc/dvd.c:988-996,1661-1667,2085-2094`). The prior Rust check treated the inserted-disc bit as the close event.

Rust now checks the Wii DI inserted-disc status until it becomes clear, then submits the distinct `DVD_WAITFORCOVERCLOSE` ioctl (`0x79`) through `/dev/di`. This avoids accepting the already-inserted state as a close event: libdi also treats the inserted bit as already closed and skips its mount wait (`../../C-sources/libogc/libdi/di.c:391-404`). Recovery completes only after the wait ioctl succeeds. Cancellation is checked while waiting for the disc-absent state and again after the ioctl returns. A regression test starts with the inserted state set, verifies the close ioctl is not called until that state clears, and verifies the ioctl command word.

At the time of this handoff, the GameCube transport finding was still open; it is superseded by the current-source transport section below. `ogc-dvd-sys` routes reads through `ogc-di` and Wii `/dev/di` (`crates/compatibility/ogc-dvd-sys/src/lib.rs:3-7,16-20`; `crates/services/ogc-di/src/lib.rs:24-31`), while the C DVD implementation drives DI registers and its interrupt state machine (`../../C-sources/libogc/libogc/dvd.c:31-48,2526-2541`). The `ogc-di` review is currently active, and a GameCube transport requires a separate direct-register/interrupt backend. I did not expand this fix into that backend. Both DVD crates remain pending until that platform gap is fixed or deliberately resolved and the BS handoff passes independent review.

Validation for this handoff: `rustfmt --edition 2021 --check crates/compatibility/ogc-dvd-sys/src/lib.rs`, `git diff --check`, and `cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu` passed. The PowerPC test binary could not link with the host linker (`ld: unknown options: --as-needed -Bstatic ...`), so the tests were not run. No hardware verification was performed.

## Independent re-review: BS cover-close recovery

Reviewer: `/root/reviewer_1`. Implementer: `worker_b`, commit `5884d1a` (fix changes in `crates/compatibility/ogc-dvd-sys/src/lib.rs`; the commit also includes unrelated ES work).

The follow-up fix resolves the previous mismatch. C routes failed command `0x0004` through motor stop and `DVD_LowWaitCoverClose` (`../../C-sources/libogc/libogc/dvd.c:1043-1048,1661-1667,2085-2094`); its cover interrupt path completes the active BS request with `-4` and clears queued commands (`dvd.c:1633-1646`). Rust first waits until `DVD_COVER_DISC_INSERTED` clears, then submits `DVD_WAITFORCOVERCLOSE` (`0x79`) to `/dev/di`, and reports `DVD_STATE_COVER_CLOSED` / `DVD_ERROR_COVER_CLOSED` only after that ioctl succeeds (`crates/compatibility/ogc-dvd-sys/src/lib.rs:329-414,618-631`). This handles the already-inserted case that libdi otherwise treats as an already completed cover wait (`../../C-sources/libogc/libdi/di.c:391-410`). The ioctl command and 32-byte buffers match the existing DI mount path (`crates/services/ogc-di/src/lib.rs:575-603`). Cancellation checks surround the blocking wait, consistent with the existing cancellation limits recorded above.

Regression tests verify that recovery observes an initially inserted state before invoking the wait operation, that the ioctl command word is `0x79000000`, that ordinary reads bypass this path, and that cancellation prevents or ends recovery (`crates/compatibility/ogc-dvd-sys/src/lib.rs:1474-1527,1545-1582`). No mismatch remains in this bounded BS recovery fix. The Rust backend still does not reproduce C's complete interrupt/error categorization, as noted above, and at that stage the separate GameCube transport mismatch was still open. The current-source transport section below supersedes that platform finding.

Validation: I ran `cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu`; it passed and compiled the tests. The tests were not executed. `git diff --check` passed for this review. No hardware verification was performed.

## 2026-10-02 command submission parity follow-up

Compared again with current `libogc/dvd.c`.

- `DVD_StopStreamAtEndAsync` command `0x08` now reaches the low-level audio-stream stop command (`DVD_AUDIOSTREAM`, subcommand 0, length 0, offset 0) instead of stopping the drive motor. The Wii `/dev/di` transport now exposes the matching `0xE1` audio-stream ioctl path, while GameCube sends the direct `0xE1000000` DI command word.
- automatic cache invalidation now happens once in the shared submission path, before the command is marked busy/queued, for exactly C command IDs `1`, `4`, `5`, and `0x0e`. This includes the disk-ID command (`5`) and removes the old read-worker-only invalidation.

Focused source tests cover the auto-invalidation command set. Hardware DVD audio streaming and cache effects still require target validation.

## 2026-10-02 command-state edge parity follow-up

Current C `DVD_CancelAllAsync` does not cancel queued or active commands and does not invoke its callback argument. It only calls `DVD_Pause()` and returns `1`. Rust now does the same instead of advancing the cancellation generation and synthesizing cancellation callbacks.

`DVD_Pause` state reporting was also tightened to C's current implementation: the pausing/ignored state is entered immediately only when no command is active. Pausing while a command executes leaves that command's status observable and does not cancel it. The previously aligned stream-stop command and submission-stage cache invalidation remain unchanged.

The Rust worker model still differs from C's interrupt-driven DVD state machine in completion timing/error categorization; this follow-up addresses the concrete pause/cancel-all state behavior only.


## 2026-10-02 current-source transport status

The historical GameCube finding above is closed at source level. The current worker branches on console identity and routes GameCube commands through direct DI-register operations rather than IOS `/dev/di`. The old audit remains in this document to preserve review history, but it must not be used as the current status. No real-console verification is claimed.

## Fix handoff — GameCube disk status mapping

Implementer: `/root/worker_b`. Independent reviewer: review thread (pending).

C defines status byte 2 as `DVD_STATUS_DISK_CHANGE` and sends it into cover-closed handling; status byte 5 is `DVD_STATUS_DISK_ID_NOT_READ`, which C includes in `dvd_may_retry` (`../../C-sources/libogc/libogc/dvd.c:102-110,135,726-800,1633-1651`). Rust now maps status 2 to `DVD_STATE_COVER_CLOSED` and returns `DVD_ERROR_COVER_CLOSED` from the GameCube error path. Rust maps status 5 to `DVD_STATE_RETRY`, so it no longer reports `DVD_STATE_WRONG_DISK`. Tests cover both status values in the GameCube status mapping.

This change fixes the public status mapping. Rust still does not reproduce C's complete interrupt driven disk-change/remount sequence, internal retry categorization, or extension-disable/spin-up path for disk ID reads. Those state and mount differences remain open. The Wii `/dev/di` behavior and the separate callback timing differences are unchanged.

`rustfmt --edition 2021 --check crates/compatibility/ogc-dvd-sys/src/lib.rs`, `git diff --check`, and `cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu` passed. The target check compiled the regression tests, but they were not run. No hardware verification is claimed.
