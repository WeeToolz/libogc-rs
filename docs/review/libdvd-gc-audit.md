# Independent GameCube DVD transport audit

Reviewer: `/root/reviewer_6`  
Scope: unresolved GameCube transport finding for `ogc-dvd` and `ogc-dvd-sys`; BS cover-close re-review is outside this audit.  
Result: **superseded by the current source.** This file preserves the earlier audit; HEAD `4c36294` now contains the GameCube direct-register transport described in the update below.

## Rust transport

`ogc-dvd-sys` identifies itself as a facade backed by Wii `/dev/di` and imports `ogc_di` (`crates/compatibility/ogc-dvd-sys/src/lib.rs:3-20`). Its request worker dispatches mount, reset/spin, motor, reads, and seek through `di::*` calls (`:495-531,600-663`). The DI service has a fixed path `c"/dev/di"` (`crates/services/ogc-di/src/lib.rs:59`); `init` calls `open_di`, which opens that path with `open_file` (`:308-313,830-847`). There is no console-specific branch in either crate selecting a GameCube register backend, and both only gate on `target_arch = "powerpc"` (`ogc-dvd-sys/src/lib.rs:9-12`; `ogc-di/src/lib.rs:10-11`). Thus a GameCube PowerPC build still uses the Wii IOS device transport.

## C transport

C `dvd.c` chooses the physical DI register base at compile time: `0xCD006000` for `HW_RVL` (Wii) and `0xCC006000` for `HW_DOL` (GameCube) (`../../C-sources/libogc/libogc/dvd.c:343-349`). The low-level command path writes command words, transfer size, buffer address, and start/DMA bits through `_diReg` (for example `dvd.c:564-573`); normal DVD initialization installs `__DVDInterruptHandler` on `IRQ_PI_DI` and unmasks it (`:2608-2623`). The shared C state machine dispatches commands to these low-level operations (`:1473-1503`). C therefore accesses the GameCube DI hardware directly and receives completion/error events through the DI interrupt path rather than opening `/dev/di`.

## Conclusion

At the time of this audit, the statement that the Rust transport lacked GameCube support was accurate for these crates. The Rust facade exposes a PowerPC build for both consoles, but its only transport is Wii IOS `/dev/di`; C selects direct GameCube DI registers and an interrupt-driven backend for GameCube. Both `ogc-dvd` and `ogc-dvd-sys` were left `pending,finding` for this platform mismatch at that time. This audit does not assess Wii behavior or the already-reviewed BS cover-close fix. No implementation changes, builds, tests, or hardware verification were performed.


## 2026-10-02 current-source correction

The conclusion above is historical and no longer applies to HEAD `4c36294`. `ogc-dvd-sys` now has a console split in the shared request worker: GameCube operations use the direct DI register backend at `0xCC006000`, including reset, disk-ID, read, seek/status, motor, and error paths, while Wii continues to use `/dev/di`. Focused source tests pin the GameCube read command words and drive-status mapping. The remaining DVD caveat is hardware/state-machine validation, not absence of a GameCube transport.

## Independent re-review of GameCube transport fix

Reviewer: `/root/reviewer_d`. Implementer: `worker_c`. Implementation commit: `6888c7ba` (`Add GameCube DVD DI transport`); reviewed the current source after subsequent DVD changes. Scope: the new GameCube DI transport and its claimed register/status parity. The separate earlier BS cover-close finding and full DVD state-machine parity remain open and are outside this bounded re-review.

### Result: NEEDS_CHANGES

The direct register base and read command words match C: Rust targets `0xCC006000` and `gc_read_command_words` emits `[0xA8000000, offset_words, length, length]` (`crates/compatibility/ogc-dvd-sys/src/lib.rs:74-82,299-306`). C uses `0xCC006000` for `HW_DOL` and `DVD_READSECTOR` writes the same command, word offset, transfer length, DMA address, length register, and `DMA|START` (`../../C-sources/libogc/libogc/dvd.c:343-349,560-573`). Disk-ID (`0xA8000040`), inquiry (`0x12000000`), seek (`0xAB000000`), request-error (`0xE0000000`), stop motor (`0xE3000000`), and control-motor (`0xFE110000`) command constants also correspond to C's definitions and register setup (`dvd.c:75-86,1984-2013,2029-2077,1880-1887`; Rust `ogc-dvd-sys/src/lib.rs:413-460`).

The GameCube status mapping is not C-equivalent. Rust maps drive status byte `5` to `DVD_STATE_WRONG_DISK` (`crates/compatibility/ogc-dvd-sys/src/lib.rs:309-317`), and its regression test asserts this (`:1831-1837`). C defines status `5` as `DVD_STATUS_DISK_ID_NOT_READ`, while status `2` means `DVD_STATUS_DISK_CHANGE` (`../../C-sources/libogc/libogc/dvd.c:102-110`). C treats status 5 as recoverable: `dvd_may_retry` explicitly includes `DISK_ID_NOT_READ`, and its error path treats this as a spin-up/retry condition (`dvd.c:135,726-800,1144-1180`). C treats status 2 as disk change and enters the cover-closed/remount path (`dvd.c:726-800,1633-1651`). Rust instead falls through to `DVD_STATE_RETRY` for status 2 and reports the wrong-disk state for status 5, changing observable `DVD_GetDriveStatus`/error results and preventing equivalent retry/remount handling (`ogc-dvd-sys/src/lib.rs:473-478,557-572`). The test currently codifies the status-5 mismatch and omits status 2; it does not validate parity despite its name.

The implementation note correctly calls out additional unimplemented C behavior: C installs the DI interrupt handler and uses interrupt-delivered completion, cover events, and alarm timeouts (`../../C-sources/libogc/libogc/dvd.c:2608-2623,1320-1388`), while Rust polls the start bit and status register (`crates/compatibility/ogc-dvd-sys/src/lib.rs:337-370`). C's mount sequence includes spin-up handling, drive unlock, firmware/model checks and patch flow (`dvd.c:1144-1303,1760-1808,2561-2594`); Rust's `gc_di_mount` only resets, delays, spins up, reads status, disk ID, and inquiry (`ogc-dvd-sys/src/lib.rs:480-521`). These documented gaps mean the transport is partial and the crates must remain open even after the concrete status mapping is corrected.

Validation: `cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu` passed, compiling the target-only regression tests; the tests did not run. `cargo check -p ogc-dvd --target powerpc-unknown-linux-gnu` passed. No hardware verification was performed. Ledger not edited; coordinator owns the ledger update.

## Fix handoff — GameCube disk status mapping

Implementer: `/root/worker_b`. Implementation commit: `4ab6a2f` (`Fix GameCube DVD status mapping`). Independent re-review: needed; review thread owns it.

The fix maps status byte 2 (`DVD_STATUS_DISK_CHANGE`) to `DVD_STATE_COVER_CLOSED`, with `DVD_ERROR_COVER_CLOSED` in the GameCube error path. It maps status byte 5 (`DVD_STATUS_DISK_ID_NOT_READ`) to `DVD_STATE_RETRY` instead of `DVD_STATE_WRONG_DISK`. The regression now checks both values (`crates/compatibility/ogc-dvd-sys/src/lib.rs:309-318,1830-1845`). This aligns the exposed state mapping with C's disk-change and retry statuses. The interrupt-delivered cover/remount flow, complete internal retry categorization, and disk-ID-read extension-disable/spin-up behavior remain open.

Validation: `rustfmt --edition 2021 --check crates/compatibility/ogc-dvd-sys/src/lib.rs`, `git diff --check`, and `cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu` passed. `cargo check -p ogc-dvd --target powerpc-unknown-linux-gnu` passed. Tests compiled but were not run. No hardware verification was performed. Ledger not edited; coordinator owns the ledger update.

## Independent current-source audit — DVD state machine

Reviewer: `/root/reviewer_e`. The status mapping for byte 2 to `COVER_CLOSED` and byte 5 to `RETRY` passes its bounded comparison (`ogc-dvd-sys/src/lib.rs:309–318`; C `dvd.c:102–110, 135`). The earlier claim that no GameCube transport exists is stale: Rust currently reads/writes the DI registers in `lib.rs:321–470` and has a GameCube mount path at `:479–519`.

The state machine remains incomplete. C's `__dvd_stategettingerrorcb` reads DI status/error, categorizes retryable failures, may reissue the command, stop the motor, wait for cover close, seek/retry reads, and route disk changes through remount (`dvd.c:516–541, 726–800, 762–800, 789–792`). Rust `execute` performs one `gc_di_read` for the request, maps one status snapshot, then completes once (`ogc-dvd-sys/src/lib.rs:399–405, 556–575, 870–900, 974–989`). Status 2 always becomes `COVER_CLOSED` without command-sensitive queue handling, spin-up, or remount; status 5 becomes `RETRY` but does not resubmit or disable extensions/spin up for disk-ID reads.

C chunks reads at `0x80000`, advances the buffer and `currtxsize`, and can preserve partial progress (`dvd.c:1473–1490`). Rust submits the full request as one GameCube DMA and sets progress only on total success (`lib.rs:399–405, 888–890, 693–717, 974–989`). Mount behavior is also partial: C hard-resets, waits 1.15 seconds, then uses an async spin-up/ID callback with retries (`dvd.c:2526–2569`); Rust performs a synchronous reset/delay and checks only selected statuses (`lib.rs:479–519, 751–762`). C transitions on interrupt callbacks; Rust serializes work in a worker and calls completion after synchronous transport (`dvd.c:726–800, 1473–1490`; Rust `lib.rs:693–717, 1006–1027`).

`cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled tests; tests did not run. No hardware verification was performed. The separate BS recovery cover-close event mismatch remains open.

## Independent re-review — BS recovery with an inserted disc

**Result: PASSED; the recorded mismatch is stale.** Reviewer: `/root/reviewer_f`. Follow-up commit: `5884d1a`.

Rust `recover_bs_read` stops the motor, polls until the cover reports open, and only then calls `wait_cover_close`; it completes the BS request as cover-closed only when that wait succeeds (`crates/compatibility/ogc-dvd-sys/src/lib.rs:643–685`). The production path uses Wii DI cover status and `DVD_WAITFORCOVERCLOSE` ioctl `0x79` (`lib.rs:603–628, 902–927`). Thus an initially inserted disc no longer makes Rust complete immediately. C handles failed command `0x0004` by stopping the motor and arming the cover-close interrupt (`C-sources/libogc/libogc/dvd.c:1040–1049, 1661–1667, 2085–2094`); the interrupt completes with `-4` and clears queued work (`:1633–1646`).

The regression starts with the cover inserted, then observes the open state before allowing the close wait (`ogc-dvd-sys/src/lib.rs:1770–1795`). Reviewer_f ran no checks; the note records PowerPC test code compiled, but tests did not run. No hardware verification was performed. Rust still polls and uses an IOS wait rather than C's interrupt state machine; the broader DVD retry, remount, transfer-progress, and callback timing findings remain open.

## Implementer handoff — GameCube read chunking and progress

Implementer: `/root/worker_b`. Independent re-review: needed; review thread owns it.

C reads GameCube DVD requests in chunks no larger than `0x80000` bytes. Each command advances its destination buffer and disc offset by the bytes already transferred, and the command block retains transfer progress across chunks (`../../C-sources/libogc/libogc/dvd.c:837–838, 1473–1490`). Rust now uses the same maximum chunk size for GameCube reads, advances the destination and word offset for each successful chunk, and reports completed-chunk bytes through `currtxsize`/`txdsize` if a later chunk fails. Wii reads keep their existing path. This fix does not add the wider C interrupt, retry, or mount behavior; those findings remain open.

Regression tests check multiple chunk sizes and offsets, and check that a second-chunk error preserves the first chunk's progress. `rustfmt --edition 2021 --check crates/compatibility/ogc-dvd-sys/src/lib.rs`, `git diff --check`, and `cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu` passed. The target test code compiled; tests were not run. No hardware verification was performed. The shared ledger is managed by the coordinator.
# Independent re-review of GameCube read chunk/progress fix `d9987e6`

**Result: NEEDS CHANGES.** Reviewer: `/root/reviewer_e`; implementer: `worker_b`.

The C-sized chunk limit and progress across fully completed chunks match: C submits at most `0x80000` bytes and advances buffer/disc offset by completed bytes (`C-sources/libogc/libogc/dvd.c:1483-1489`); Rust uses the same maximum, pointer/offset advancement, and completion copyback (`crates/compatibility/ogc-dvd-sys/src/lib.rs:400-428,693-717,888-896,1014-1030`).

A partial transfer within a chunk that ends with an error is still lost in Rust. C updates `txdsize` by `currtxsize - _diReg[6]` before branching on cancellation/error/success (`dvd.c:837-838`). Rust's `gc_di_command` does not read the remaining-length register on completion/error, and `run_gc_read_chunks` reports only earlier successful chunks (`ogc-dvd-sys/src/lib.rs:321-369,377-389`). The test that fails the second chunk expects only the first full chunk (`:1890-1914`), so it does not cover bytes transferred within the failed chunk. This remains a concrete mismatch in progress reported to the callback.

Validation: `cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled tests; tests did not run. `rustfmt --check` and `git diff d9987e6^ d9987e6 --check` passed. No hardware verification was performed. Existing retry, mount and callback findings remain open.

## BUG FIX handoff: preserve progress within a failed GameCube read chunk

Implementer: `/root/worker_a`. Independent re-review is needed; the review thread handles it. The GameCube DI completion path now reads the remaining transfer-length register and computes bytes transferred for both success and error completions. The chunk runner adds the failed chunk's reported partial progress to earlier completed chunks before it completes the request, matching C's `currtxsize - _diReg[6]` update. It clamps the register-derived count to the requested chunk size. Cache invalidation after completion covers the bytes reported as transferred. Wii read behavior is unchanged.

Added coverage for converting the remaining-length register to transferred bytes and for preserving a partial count from the failed second chunk. `rustfmt --edition 2021 --check crates/compatibility/ogc-dvd-sys/src/lib.rs` and `git diff --check` passed. `cargo check -p ogc-dvd-sys --tests --target powerpc-unknown-linux-gnu` is blocked before compiling this crate by existing `iso9660` i32/u64 field mismatches at `src/lib.rs:741-749`; the DVD test target did not compile and tests did not run. No hardware verification was performed.

## Build correction handoff

Implementer: `/root`. Independent static re-review is needed; the review thread handles it. The new GameCube transfer-progress implementation used a non-const `min` call in a `const fn` and returned an `i32` from a chunk closure that must return `(status, transferred)`. The code now uses a const-compatible bound and returns `(-22, 0)` on checked offset overflow. `cargo xtask build-dols` passed after these compile corrections. This does not resolve the remaining DVD behavior findings. No hardware verification was performed.

## Independent re-review — partial progress in failed GameCube read chunks

**Result: PASS for the recorded partial-progress fix.** Reviewer: `/root/worker_b`. Implementer: `/root/worker_a`; root corrected the follow-up build errors.

C updates transferred progress from `currtxsize - _diReg[6]` before it handles the completion/error result, and chunks reads at `0x80000` bytes while advancing the buffer and offset by progress (`../../C-sources/libogc/libogc/dvd.c:837–838,1473–1490`). Current Rust reads register 6 after the DI command finishes, bounds the remaining count to the current chunk, invalidates the transferred buffer range, and returns the transferred byte count together with both success and DI error results (`crates/compatibility/ogc-dvd-sys/src/lib.rs:310–319,377–395`). The chunk runner adds a failed chunk's partial count to earlier completed chunks (`:435–440`). The regression covers the independent register-to-byte calculation, including a remaining count larger than the transfer, and a partial second-chunk failure (`:1911–1964`). The reported mismatch is fixed. Wider interrupt, retry, mount, and callback parity gaps remain open.

No checks or tests were run during this independent pass. The handoff records `cargo xtask build-dols` passed after compile corrections; the target test build was blocked by existing `iso9660` type errors, so tests did not run. No hardware verification was performed.


## 2026-10-06 implementation reconciliation — remaining GameCube DVD source blockers

The current Rust source now reconciles the named retry/remount and callback-order findings from the reviews above. GameCube absolute reads preserve partial transfer progress, retry from the transferred offset, handle retryable drive states, wait for media transitions, spin the drive when required, and verify the remounted disk ID before resuming. A different disk terminates the active command with the C fatal result/state instead of continuing the old request. Disk-ID and seek requests use the same media-change recovery model where C resumes those commands.

Mount now follows the C GameCube sequence more closely. It performs the hard reset and 1.15 second delay, probes the disk ID, and, when the drive needs the legacy extension path, performs inquiry, the two MATSHITA/DVD-GAME unlock commands, the model-specific 448-byte firmware upload for models 04/06/08/Q08, the firmware function call, extension enable, accept/up spin command, and status setup before the authoritative disk-ID read. The four drive firmware byte sequences are Rust-owned static data mirrored from libogc; there is no C runtime dependency. Unknown models are not sent an undefined firmware payload.

Successful read callbacks now receive the transferred byte count, and successful disk-ID/mount callbacks receive `DVD_DISKIDSIZE`, matching the C callback contract. Completion marks the block terminal and clears the active request before invoking the user callback; scheduler waiters are released after the callback, matching C's observable callback/state order without calling user code under the Rust queue mutex.

The Rust transport still polls GameCube DI completion rather than reproducing libogc's interrupt implementation. Under the project parity rule this is an implementation difference, not a remaining source blocker: the named externally observable state, retry/remount, progress, result, and callback-order mismatches are now represented. Runtime timing and hardware behavior still require re-review.
