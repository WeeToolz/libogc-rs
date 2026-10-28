# USB Gecko Review

## Command completion correction — 2026-10-16

The AESND Dolphin timeout stack reached `send_bytes` while the probe was
already running audio checks. It was not stalled at startup. Rust
`send_command` used `EXI_ImmEx`, which completes its own transfer, followed by
another `EXI_Sync`. The second sync returned failure because no transfer was
pending. Safe sends then retried readiness checks and delivered no report text.
C uses `EXI_Imm` followed by one `EXI_Sync`.

Rust now uses that one-start, one-wait sequence. A shared command engine
returns start, completion, and deselection errors. It aborts if selection fails
and attempts deselection after transfer failure. Two host regression tests
check the device response, one wait, and transfer/cleanup failures through
that production engine.

The complete AESND Dolphin probe now passes with 72 checks and zero failures,
including its screen-data transport check. Ten readable page PNGs were
captured. Report: `/tmp/libogc-dolphin-pcm-corrected/suite.json`. This proves
the emulated Gecko path, not physical USB Gecko hardware or flash writes.

Date: 2026-10-01

Current status (2026-10-05): The reviewed flash sequence, zero-length flash calls, per-command range checks, 20 microsecond program delay, timeout removal, receive-destination zeroing, and `flush` return type match C in the bounded areas documented below. The callback-backed EXI wait queue passed a second independent static review, but the callback/wakeup/retry behavior has no executed regression or DOL coverage. The PowerPC unit-test target still fails to compile because of existing unsafe test helper errors, so those tests have not run. `ogc-usbgecko` remains open. No Wii hardware verification is claimed.

## Scope

- Rust crate manifest: `crates/services/ogc-usbgecko/Cargo.toml`
- Rust implementation: `crates/services/ogc-usbgecko/src/lib.rs`
- C public API: `C-sources/libogc/gc/ogc/usbgecko.h`
- C implementation: `C-sources/libogc/libogc/usbgecko.c`
- Rust EXI forwarding/helper path: `crates/hardware/ogc-exi/src/lib.rs`
- Rust tick and scheduler helpers: `crates/core/ogc-lwp-driver/src/runtime.rs`
- C EXI behavior checked where relevant: `C-sources/libogc/libogc/exi.c` and `gc/ogc/exi.h`.

The Rust crate contains one source module, `src/lib.rs`. Its dependencies are `ogc-exi` and `ogc-lwp-driver`. The C counterparts for the API are all in `usbgecko.c`, with declarations in `usbgecko.h`.

## Public API coverage

Each C function in `usbgecko.h:10-22` has a Rust counterpart. Names use snake case in Rust. Function behavior was followed through the corresponding Rust helpers and C static helpers.

| Rust API and lines | C API and lines | Review result |
|---|---|---|
| `flush` (211-231) | `usb_flush` (`usbgecko.c:141-148`) | Both acquire the channel and drain bytes until receive reports no byte. Rust returns `false` for an invalid channel; for valid channels both drain until receive reports no byte. C returns `void`. |
| `is_gecko_alive` (233-257) | `usb_isgeckoalive` (`150-171`) | Both require EXI device ID zero before sending command `0x9000`, then test response mask `0x0470`. The Rust device-ID parity fix awaits re-review by `/root/reviewer_6`. |
| `receive_buffer` (350-352), `send_buffer` (360-362) | `usb_recvbuffer`, `usb_sendbuffer` (`198-200`, `227-229`) | These call the corresponding `_ex` implementation with retries `-1`, which means no retry limit in C and Rust. Return value is bytes transferred. |
| `recvbuffer_safe` (370-372), `sendbuffer_safe` (380-382) | `usb_recvbuffer_safe`, `usb_sendbuffer_safe` (`258-260`, `289-291`) | Both wait for the receive/send-ready flag before transferring. Retry accounting tracks loop attempts for the safe case. Rust retains its input validation but no longer adds an elapsed-time limit. |
| `recvbuffer_ex` (390-392), `sendbuffer_ex` (400-402) | `usb_recvbuffer_ex`, `usb_sendbuffer_ex` (`173-200`, `202-229`) | Both stop at transfer failure, buffer completion, or the configured retry count and return bytes transferred. Rust adds input validation but no longer adds an elapsed-time limit. |
| `recvbuffer_safe_ex` (410-412), `sendbuffer_safe_ex` (420-422) | `usb_recvbuffer_safe_ex`, `usb_sendbuffer_safe_ex` (`231-260`, `262-291`) | Both combine ready checks with retry accounting; Rust adds validation but no longer adds an elapsed-time limit. |
| `flashread` (430-467) | `usb_flashread` (`315-327`) | Both read bytes through the `0xF...` command, in order, and return success/failure. Rust now checks the 512 KiB limit in each flash command helper, as C does, so a crossing request can process its valid prefix before the first invalid address fails. |
| `flashwrite` (475-511) | `usb_flashwrite` (`329-351`) | Rust and C use the same byte-program command sequence, range check, delay, and readback order in the inspected path. |
| `flashverify` (513-531) | `usb_flashverify` (`353-368`) | Both enter software ID mode, read IDs at addresses 0 and 1, require `BF D7`, then exit software ID mode. Rust's `&&` chain also skips the exit commands after an earlier step fails, as C's chain does. |

## Helper and call-path coverage

- Lock acquisition and release: `GeckoLock::acquire` / `Drop` (Rust lines 20-45), `with_gecko_lock` (55-58), compared with `__usbgecko_exi_wait`, `__usbgecko_exi_unlock`, and `EXI_Lock` / `EXI_Unlock` (C lines 15-41; EXI implementation `exi.c:232-298`). Rust retries `EXI_Lock(..., None)` and yields until it acquires the lock. C lazily initializes one LWP wait queue per EXI channel, registers an unlock callback, and sleeps until it can lock. Both release a successfully acquired lock. The polling/yield scheduling difference remains; no elapsed timeout remains in the current Rust path.
- Transfer and lock retry behavior: current loops continue until the protocol/retry count ends the operation. The earlier Rust-only elapsed-time deadlines have been removed. C sleeps on an LWP queue for EXI lock rather than polling/yielding; fairness and latency are not established by static comparison.
- EXI command setup: Rust `send_command` (60-85) corresponds to C `__send_command` (43-54). Both select device 0 at 32 MHz, transfer a 16-bit read/write command, sync, and deselect. Both attempt all four operations and report failure if any operation fails.
- Flash command helpers: Rust `flash_write_command` (87-121), `flash_read_command` (123-160), software-ID entry/exit (162-172) correspond to C `__flashwritecommand` / `__flashreadcommand` (56-89) and software-ID helpers (293-313). EXI selection at 16 MHz and read/write modes match. Both validate each flash command address at `0x7FFFF`.
- Gecko byte protocol: Rust `recv_byte`, `send_byte`, `check_recv`, and `check_send` correspond to C `__usb_recvbyte`, `__usb_sendbyte`, `__usb_checkrecv`, and `__usb_checksend` (91-139). Command words and status masks match. `recv_byte` clears its output before polling, as C does. Transfer callers stop on failure, so the failed byte is not counted.
- Transfer engines: Rust `transfer_bytes` and `send_bytes` (248-342), compared with the four C loops for raw/safe receive and send (173-291). The byte counters, status checks, success handling, and retry update placement correspond. Rust converts the validated buffer to a slice and checks non-null pointer, positive size, and valid channel before transfer. C does not validate these inputs and has no transfer deadline.
- Flash operations: Rust `flashread`, `flashwrite`, and `flashverify` (430-531), compared with C (315-368). Rust now relies on the same per-command `0x7FFFF` address bound as C instead of prevalidating the complete interval. Rust also waits for 20 microseconds using the PowerPC tick counter before verification, matching C's `usleep(20)` delay contract at source level.
- Rust `USB_FLUSH_COUNT` (line 18) is incremented by `flush` (216) and has no read site in this module. It is a Rust-only counter with no C public counterpart.
- Test-only helpers and tests occupy Rust lines 533-592. Their assertions were read but not executed.

## Confirmed behavior differences

1. **Resolved — flash program setup sequence.** Rust sends `AA@0x5555`, `55@0x2AAA`, `A0@0x5555`, then the target byte, matching C. See the independent review below.

2. **Resolved in the later parity pass — Rust-only lock and transfer deadlines.** `GeckoLock::acquire` bounds EXI lock wait; `flush` and send/receive loops use the half-second transfer deadline (`lib.rs:15,23-37,217-229,248-342`). Flash methods use the one-second timeout only while acquiring the lock (`lib.rs:16,430-531`); their per-byte operations do not check elapsed time. C waits on an LWP queue for EXI lock and has no elapsed timeout in USB Gecko operations (`usbgecko.c:31-41,141-148,173-291,315-368`). The elapsed-time cutoffs were removed. Rust now keeps retrying EXI lock acquisition and no longer terminates flush/send/receive solely because time elapsed. The Rust loop yields rather than sleeping on C's callback-backed queue, so the scheduling mechanism differs but the prior observable timeout is gone.

3. **Resolved — zero-length flash calls with null buffers.** Rust now permits null only when length is zero and returns success without accessing the buffer, matching C (`usbgecko.c:315-327,329-351`; Rust `lib.rs:45-51,477-485,518-526`).

4. **Resolved in the later parity pass — flash address-range handling.** The public flash operations no longer reject the full interval up front. `flash_write_command` and `flash_read_command` now reject addresses above `0x7FFFF`, matching C's helper-level check, and callers advance addresses with wrapping `u32` arithmetic. A request crossing the device boundary can therefore process the same valid prefix before failure.

5. **Resolved in the later parity pass — flash program delay.** C waits with `usleep(20)` before readback (`usbgecko.c:343-345`). Rust now waits until at least 20 microseconds of the 60.75 MHz tick clock have elapsed before issuing the verification read. Hardware validation is still required for device timing.

6. **Resolved — flush result.** C `usb_flush` returns `void`; Rust `flush` now returns unit and reports no result for an invalid channel. The invalid-channel guard is a defensive Rust addition. See the independent review of commit `4149df0` below.

## Intentional additions and uncertain differences

- Rust rejects channels outside 0-2 (`channel_index`, lines 47-49) and null buffers or negative/zero transfer sizes (`transfer_bytes` / `send_bytes`, lines 248-253, 297-303). C indexes the EXI channel state directly and does not validate these cases in USB Gecko functions. This is a defensive Rust addition. For buffer transfer size zero, both ultimately return 0; negative C sizes also transfer no bytes and return 0 due to its `left > 0` loop.
- Rust publicly re-exports its `ogc-exi` dependency as `exi` (`lib.rs:11`); C's USB Gecko header does not expose an equivalent namespace.
- Rust and C both validate each flash command address, so a request crossing the range can perform the same valid prefix before failure.
- **Resolved — EXI lock wait strategy.** Rust now registers the C-style unlock callback and sleeps on a per-channel LWP queue. Exact wakeup latency and hardware scheduling were not measured.
- Rust's C-shaped APIs use `usize` for flash length and Rust `bool` for `flush`; C uses `size_t` and `void`. Rust does not declare `#[no_mangle]` C ABI symbols in this module, so these are Rust crate APIs rather than drop-in exported C symbols.
- C `__flashwritecommand` and `__flashreadcommand` each reject addresses above `0x7FFFF`; Rust's private command helpers also reject each invalid address.

## Fix handoff: `is_gecko_alive` EXI device-ID check

Implementer: `/root/worker_a`. Independent reviewer: `/root/reviewer_6` (review pending).

Rust now calls `exi::read_id(chn, exi::EXI_DEVICE_0)` before acquiring the Gecko EXI lock. An EXI read error or any ID other than zero returns 0 without sending the Gecko probe. A successful zero ID proceeds to the existing `0x9000` command and `0x0470` response-mask check. This matches the order and conditions in C `usb_isgeckoalive` (`usbgecko.c:150-171`). The regression test verifies that failed/nonzero IDs do not send a command and that ID zero does.

Verification: the PowerPC target `cargo check` passed. The host `cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu --tests` did not compile because existing test code has unsafe function-pointer cast and unsafe-call errors. The new regression test did not add compile errors. Tests were not run, and no hardware verification was done.

## Completeness check and limits

Completeness was checked by enumerating every top-level `pub` declaration in the only Rust source module, matching all 15 functions declared in `usbgecko.h`, and tracing each public call through Rust's byte, command, lock, tick, and flash helpers. The full C `usbgecko.c` implementation was read, including all static helpers, public functions, and transfer loops. EXI lock and device-ID behavior was traced into `exi.c`; scheduler tick/yield behavior was traced into `ogc-lwp-driver` source. No other `ogc-usbgecko/src` module or C USB Gecko implementation file was found. The implementation fixes listed above await independent review by `/root/reviewer_6`. The other confirmed differences remain open. No emulator session, EXI device, USB Gecko, or flash hardware was used, so hardware behavior and flash-delay equivalence remain unverified.

## Fix handoff — flash byte programming sequence

Implementer: `worker_c`. Independent reviewer: `/root/reviewer_6` (review pending).

Rust `flashwrite` now sends the C `usb_flashwrite` sequence for each byte: `AA` to `0x5555`, `55` to `0x2AAA`, `A0` to `0x5555`, then the target address and data (`../../C-sources/libogc/libogc/usbgecko.c:329-346`). The sequence stops at the first failed command. A regression test records the commands emitted by the programming helper and checks their order and values.

The delay before readback, finite Rust waits, and flash range prevalidation differences remain open as recorded above. `ogc-usbgecko` remains pending until an independent static reviewer checks this change and all remaining differences are resolved or deliberately documented. No hardware verification is claimed.

Validation: `cargo fmt -p ogc-usbgecko -- --check`, `git diff --check`, and `cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu` passed. The target `--tests` check is blocked by existing unsafe function-pointer casts and unsafe calls in unrelated test helpers, so the regression test did not compile or run in this check. No hardware verification was performed.


## Later parity pass: timeout, range, and program delay

Compared again with the uploaded current `libogc` `usbgecko.c`. The implementation now removes the Rust-only elapsed-time exits from EXI lock acquisition and transfer loops, checks flash bounds per command at `0x7FFFF`, and waits 20 microseconds between flash programming and verification. These changes target the three simple open behavior findings above. They have not been compiled or independently reviewed in this environment. The remaining notable differences are the Rust `flush` return value/input validation and implementation-level EXI waiting strategy; hardware behavior is unverified.


## 2026-10-01 flash-program sequence source recheck

The current Rust source already contains the full C flash byte-program sequence in the shared `flash_program_byte_with` helper: `AA` at `0x5555`, `55` at `0x2AAA`, `A0` at `0x5555`, then the target address/value. `flashwrite` uses that helper, waits 20 microseconds, and verifies the programmed byte. The focused regression records all four commands. No additional source change was needed in this pass.

## Independent re-review: 2026-10-03

**Result: NEEDS_CHANGES.** Reviewer: `reviewer_e`. Re-reviewed `ogc-usbgecko` across device-ID fix `c3e9eea` (implementer `worker_a`), zero-length fix `49f8e2b` (implementer `worker_a`), flash program sequence fix `b891f6c` (implementer `worker_c`), and the current follow-up code in `a573618` for timeout removal, command-level range checks, and program delay. Other previously documented API and EXI scheduling differences remain as recorded above.

The reviewed fixes match the C behavior inspected:

- `is_gecko_alive` obtains the EXI device ID before acquiring the Gecko lock or sending `0x9000`, and proceeds only for a successful ID of zero (`ogc-usbgecko/src/lib.rs:279–293`). This follows C `usb_isgeckoalive`, which checks both `EXI_GetID` success and `id == 0` before locking and probing (`C-sources/libogc/libogc/usbgecko.c:150–170`; `EXI_GetID` success is 1 per `gc/ogc/exi.h:274–282`). Rust `exi::read_id` wraps the same `get_id` status using the 0/1 convention (`ogc-exi/src/lib.rs:163–173`).
- Zero-length `flashread` and `flashwrite` accept a null data pointer, acquire/release the same Gecko lock, perform no buffer access or command, and return 1 (`ogc-usbgecko/src/lib.rs:45–51, 476–508, 517–549`). C initializes `ret` to 1, locks, and skips its `while (ret && length--)` body for zero length (`usbgecko.c:315–327, 329–351`).
- `flash_program_byte` sends `AA@0x5555`, `55@0x2AAA`, `A0@0x5555`, then target address/data, stopping on the first failed command (`ogc-usbgecko/src/lib.rs:135–157`). This matches C's short-circuit sequence (`usbgecko.c:329–346`).
- The current EXI lock and transfer loops no longer stop on elapsed-time thresholds (`ogc-usbgecko/src/lib.rs:22–31, 249–264, 304–380`), matching C's wait/retry loops (`usbgecko.c:31–41, 141–148, 173–291`). Rust yields while polling instead of using C's callback-backed LWP queue, as already documented; no timeout mismatch remains.
- Rust flash read/write command helpers reject each address above `0x7FFFF` (`ogc-usbgecko/src/lib.rs:96–99, 159–162`). Public loops advance with wrapping `u32` arithmetic and do not reject the whole range up front (`:476–497, 517–540`). This matches C's per-command bound and `offset++` behavior (`usbgecko.c:56–89, 315–327, 329–351`), including a valid prefix before the first out-of-range command fails.
- After each successful byte-program sequence, Rust waits for 20 microseconds in timebase ticks before the verification read (`ogc-usbgecko/src/lib.rs:53–62, 529–539`; timebase rate `ogc-lwp-driver/src/runtime.rs:35, 919–921`). C calls `usleep(20)` before its readback (`usbgecko.c:343–345`). Static code shows the requested 20 microseconds; hardware timing was not checked.

One confirmed receive-buffer mismatch remains. C `__usb_recvbyte` writes zero to `*ch` before it sends the poll command and leaves it zero when the Gecko reports no byte or the command fails (`usbgecko.c:103–114`). The C receive loops pass the caller's current destination pointer directly to that function (`usbgecko.c:173–195, 231–255`), so a failed attempt zeroes the next output byte even though the returned transfer count does not include it. Rust `recv_byte` only writes its output reference after a successful ready response (`ogc-usbgecko/src/lib.rs:213–223`), and `transfer_bytes` receives into a temporary then copies it only on success (`:322–327`). Therefore a failed receive leaves the next caller buffer byte unchanged in Rust, while C sets it to zero. This affects `receive_buffer`, `recvbuffer_ex`, `recvbuffer_safe`, and `recvbuffer_safe_ex` whenever a receive attempt returns failure.

The device-ID regression tests the decision helper, and the flash sequence regression records the four commands; the zero-length test checks helper conditions rather than calling either public flash API. There is no regression coverage for timeout removal, boundary-crossing flash ranges, or program-delay placement. `cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu` passed for the library. `cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu --tests` failed on pre-existing test code in `lib.rs:605–628` (unsafe function-pointer casts and unsafe calls); test code did not compile and tests did not run. No hardware verification was performed. The shared worktree had an unrelated modification to `docs/review/libusb.md`; it was left untouched. Ledger changes are left to the coordinating reviewer.

## Independent audit — receive failure output byte (2026-10-03)

Reviewer: `/root/reviewer_f`. Implementer: no receive-zeroing fix commit is present; current receive code history is authored by `notOrrytrout`. Result: **NEEDS_CHANGES**; the previously recorded mismatch is independently confirmed. This is a bounded review of receive failure side effects and adjacent transfer-count behavior.

C `__usb_recvbyte` first writes `0` to the caller's byte (`../../C-sources/libogc/libogc/usbgecko.c:103-110`). If the EXI command fails, it returns with that zero still stored. If the command succeeds but the returned ready bit `0x0800` is clear, it also returns failure without changing the zero (`:110-114`). Only a successful command with the ready bit set replaces the output with `val & 0xff` (`:111-114`). The raw receive loop passes its current destination pointer directly (`:173-195`); the safe receive loop does so after `__usb_checkrecv` reports ready (`:231-255`). On a receive failure both stop without advancing the pointer or decrementing `left`, so the return value remains the successful prefix length, but the next destination byte has been zeroed. In the safe path, a failed readiness check does not call `__usb_recvbyte`, so it does not zero the destination byte.

Rust `recv_byte` writes through its output reference only after `send_command` succeeds and bit `0x0800` is set (`crates/services/ogc-usbgecko/src/lib.rs:213-223`). `transfer_bytes` calls it with a temporary byte and copies that temporary into the caller buffer only after success (`:322-327`). Thus command failure or a clear ready bit leaves the caller's next byte unchanged in Rust, unlike C. This applies to raw receive calls and to safe receive calls only after `check_recv` succeeds. The returned count still matches C's successful prefix count (`usbgecko.c:179-195,237-255`; Rust `lib.rs:304-341`); the mismatch is the failed attempt's buffer write.

The current tests do not exercise receive command failure, a no-byte ready-bit response, destination side effects, or safe receive readiness/failure sequencing (`crates/services/ogc-usbgecko/src/lib.rs:571-689`). Tests cover API references, invalid inputs, flash helpers, and the EXI device-ID probe, not this finding.

I ran `cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu --tests`. The test build failed with 14 existing errors: unsafe function-pointer casts and unsafe calls in test code at `lib.rs:605-628`. Test code did not compile and tests did not run. No hardware verification was performed. The crate remains open with the receive-buffer mismatch confirmed.

## Independent confirmation — receive failure destination byte (2026-10-03)

Reviewer: `/root/reviewer_d`. Implementer: no fix for this receive-output behavior is present; the current `recv_byte` implementation originates in `f1c3012` by `notOrrytrout`. Result: **NEEDS_CHANGES**. I independently traced the byte helper and both receive loops in C, then followed the Rust helper through the public raw and safe wrappers.

C `__usb_recvbyte` stores zero through its output pointer before issuing command `0xA000` (`../../C-sources/libogc/libogc/usbgecko.c:103-110`). It leaves zero on command failure or when response bit `0x0800` is clear, and writes the response low byte only on success (`:111-114`). Raw `usb_recvbuffer_ex` passes its current caller-buffer pointer to this helper and stops on a failed result without advancing that pointer or decrementing `left` (`:173-195`). Safe `usb_recvbuffer_safe_ex` first calls `__usb_checkrecv`; when that readiness command fails it skips the byte helper and leaves the destination alone, but when readiness succeeds it passes the caller pointer to `__usb_recvbyte` (`:231-255`). Both APIs return only the successful prefix length.

Rust `recv_byte` returns before assigning its output reference if `send_command` fails or response bit `0x0800` is clear (`crates/services/ogc-usbgecko/src/lib.rs:213-223`). `transfer_bytes` supplies a temporary byte and copies it to the destination only after a successful receive (`:322-327`); the successful count is `copied` (`:304-341`). Thus the prefix count matches C, but a raw failure leaves the next caller byte unchanged in Rust instead of zeroing it. The same difference occurs in a safe transfer only when `check_recv` succeeds and the subsequent receive command fails or reports no byte. If safe readiness itself fails, both implementations leave the current destination byte unchanged. The public raw/safe wrappers and their `_ex` variants all use this shared Rust transfer path (`:396-397,416-417,436-437,456-457`), so they share the mismatch.

Existing tests do not exercise receive command failure, the `0x0800` clear response, readiness/receive sequencing, destination-buffer side effects, or successful-prefix count in either receive loop (`:571-689`). I ran `cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu --tests`; it failed with 14 pre-existing unsafe function-pointer cast and unsafe-call errors in test code at `lib.rs:605-628`. Tests did not compile or run. No hardware verification was performed. The receive mismatch remains open; the independent result agrees with the separately recorded finding.

## Fix handoff — receive failure destination byte

Implementer: `/root/worker_b`. Independent reviewer: review thread (pending).

`recv_byte` now clears its output before issuing command `0xA000`. The transfer loop passes the caller's next destination byte directly to this helper, so command failure or a response without ready bit `0x0800` leaves that byte zero, while the successful-prefix count advances only after a successful receive. A safe receive readiness failure still skips `recv_byte` and leaves the destination byte unchanged, matching C.

Regression coverage exercises command failure, a clear ready bit, and a successful byte through a testable command helper. `cargo fmt -p ogc-usbgecko -- --check`, `git diff --check`, and `cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu` passed. The target test build remains blocked by 14 pre-existing unsafe function-pointer cast and unsafe-call errors in other test helpers (`lib.rs:608-631`), so tests did not compile or run. No hardware verification is claimed.

## Independent re-review — receive destination and current crate findings

**Result: PASSED for receive destination zeroing; the crate remains open.** Reviewer: `/root/reviewer_d`. Implementer: `/root/worker_b`. Commit: `928872a`.

C `__usb_recvbyte` zeros the destination before sending command `0xA000` and writes the low response byte only if the command succeeds and ready bit `0x0800` is set (`C-sources/libogc/libogc/usbgecko.c:103–114`). Rust's `recv_byte_with` now uses the same order (`crates/services/ogc-usbgecko/src/lib.rs:213–223`). The transfer loop passes the caller's next destination byte and increments the completed count only after success (`lib.rs:299–341`). When readiness fails before the byte command, both leave the destination unchanged. The fix passes static comparison.

The earlier flash-related findings are stale in current source: the AA/55/A0 setup sequence matches C (`usbgecko.c:329–346`; Rust `lib.rs:135–157`), zero-length null-buffer calls return success, address checks happen per command, and the 20 microsecond delay precedes readback (`usbgecko.c:315–351`; Rust `lib.rs:45–61,96–99,477–537`). The elapsed timeout finding is also stale; Rust no longer imposes an elapsed cutoff.

Two differences remain. C `usb_flush` returns `void`, while Rust `flush` returns `bool` (`usbgecko.c:141–148`; Rust `lib.rs:254–268`); this is a Rust API result difference, not a C ABI symbol translation. Rust polls `EXI_Lock` and yields, while C registers a callback and sleeps on an LWP queue (`usbgecko.c:13–40`; `exi.c:240–296`). This changes scheduling; fairness and latency are not established by static inspection.

`cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu` passed. The `--tests` check failed on existing unsafe test helper calls and function-pointer casts at `lib.rs:606–629`; test code did not compile and tests did not run. `git diff --check` for `928872a` passed. No hardware verification was performed.
# Independent re-review of receive destination fix (2026-10-03)

**Result: PASS for commit `928872a` only.** Reviewer: `/root/reviewer_d`. Implementer: `/root/worker_b`.

C `__usb_recvbyte` writes zero before sending the receive command, then overwrites the destination only when the command succeeds and the ready bit is set (`C-sources/libogc/libogc/usbgecko.c:103-114`). Rust `recv_byte_with` follows that order (`crates/services/ogc-usbgecko/src/lib.rs:213-223`); its raw receive loop advances count only on success (`:299-341`). The regression helper covers command failure, not-ready, and success, but not full-transfer side effects/count.

The crate remains open: C `usb_flush` returns void and drains until receive fails; Rust returns bool (`usbgecko.c:141-148`; Rust `lib.rs:254-268`). C waits on a callback-backed LWP queue while Rust polls `EXI_Lock` and yields (`usbgecko.c:13-40`, `exi.c:240-296`; Rust `lib.rs:20-31`), so scheduling/fairness/latency differ. Validation: PowerPC library check passed. PowerPC `--tests` failed to compile on 14 existing unsafe function-pointer cast/call errors (`lib.rs:606-629`); tests did not run. No hardware verification was performed.

## Fix pass: `flush` return parity

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. Rust `flush` now returns unit, matching C `usb_flush` (`gc/ogc/usbgecko.h:10`; `libogc/usbgecko.c:141-148`). It retains the invalid-channel early return and drains input until receive fails. Added a focused test for the `fn(i32)` signature and invalid-channel guard.

Compatibility note: this changes the Rust source API. Rust callers that inspect the previous `bool` result must update. No in-repository caller depended on that result. `cargo fmt -p ogc-usbgecko -- --check` and `cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu` passed. The PowerPC `--tests` check remains blocked by 14 pre-existing unsafe test helper errors at `lib.rs:605-636`; tests did not compile or run. `git diff --check` passed. No hardware verification was performed.

### Independent re-review of flush return fix `4149df0`

**Result: PASS for the bounded flush fix.** Reviewer: `/root/reviewer_f`; implementer: `worker_c`.

C declares `usb_flush` as returning `void` and drains input while `__usb_recvbyte` reports a byte (`C-sources/libogc/gc/ogc/usbgecko.h`; `libogc/usbgecko.c:141-148`). Rust `flush` now returns unit and drains through `with_gecko_lock` until `recv_byte` returns no byte (`crates/services/ogc-usbgecko/src/lib.rs`). Its guard releases the EXI lock when the drain closure exits. The invalid-channel guard is a Rust defensive check; C indexes its channel wait queue directly, so that invalid-channel case has no defined result to match. Both valid-channel paths wait for the lock and drain input.

The scheduling method remains different: C sleeps on an LWP queue notified by the EXI unlock callback; Rust polls `EXI_Lock` and yields (`usbgecko.c:13-40`; `C-sources/libogc/libogc/exi.c:240-296`; Rust `lib.rs:20-31`). This remains an open timing/scheduling finding. Validation: PowerPC library `cargo check` passed; tests did not run. The review note records that the PowerPC test build fails on existing unsafe test helpers. No hardware verification was performed.

## Fix pass: callback-backed EXI wait queue

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed.

For valid EXI channels, `GeckoLock::acquire` now uses one lazily initialized LWP queue per channel. It registers an EXI unlock callback when it attempts to lock and sleeps on that queue when the lock is busy. The callback broadcasts to waiters on that channel. The critical section covers the failed lock attempt and queue sleep setup to prevent a lost wakeup. The invalid-channel guard remains in place.

A focused regression check confirms that a failed lock attempt registers the callback and sleeps on the selected queue. `cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu` passed. The PowerPC test build remains blocked by 14 pre-existing unsafe function-pointer cast/call errors in other test helpers; tests did not compile or run. `git diff --check` passed. No hardware verification was performed.

## Independent re-review — callback-backed EXI wait queue

**Result: PASS for commit `43e7287`.** Reviewer: `/root`. Implementer: `worker_c`.

C initializes a wait queue per channel, disables interrupts while trying `EXI_Lock` and entering the wait, registers `__usbgecko_exi_unlock`, and sleeps until the channel becomes available (`../../C-sources/libogc/libogc/usbgecko.c:13-41`). Rust initializes a queue per valid channel, protects the failed lock-to-sleep sequence with `critical_enter`/`critical_exit`, registers `exi_unlock_callback`, sleeps on the same channel queue, and retries after waking (`crates/services/ogc-usbgecko/src/lib.rs:20-31, 42-76`). The callback broadcasts to waiters for that channel, matching C. The bounded lock-wait fix passes static comparison.

The existing regression exercises the production-shared `lock_or_sleep` helper and checks callback registration and queue sleep. The PowerPC test target still fails to compile on existing unsafe test helper errors, so tests did not run. I ran no checks for this re-review. No hardware verification was performed.

## Independent second review — callback-backed EXI wait queue (`43e7287`)

**Result: PASS for static C parity; runtime lost-wakeup validation remains open.** Reviewer: `/root/reviewer_b`. Implementer: `worker_c`.

C initializes one LWP queue per channel, disables interrupts before attempting `EXI_Lock` with `__usbgecko_exi_unlock`, sleeps on that channel's queue while contended, and restores the prior interrupt state after the lock succeeds (`../../C-sources/libogc/libogc/usbgecko.c:13-41`). Its EXI implementation records the unlock callback for the waiting device and invokes a queued callback on unlock (`../../C-sources/libogc/libogc/exi.c:232-296`). Rust follows the same sequence: it creates a queue per valid channel, enters the IRQ critical section before trying the callback-enabled lock, sleeps on that queue after failure, exits the critical section after sleep returns, and retries (`crates/services/ogc-usbgecko/src/lib.rs:20-31,42-76`). Rust's EXI unlock path releases its state lock before invoking the queued callback, while interrupts remain masked; the callback broadcasts to the channel queue. Rust `queue_sleep` records the waiter and blocks with IRQs disabled, and `queue_broadcast` wakes all waiters under that same single-core IRQ protection (`crates/core/ogc-lwp-driver/src/runtime.rs:1589-1601,1618-1628`). This closes the failed-lock-to-sleep lost-wakeup window by static inspection. Wakeups retry the lock, so broadcast wakeups for multiple waiters do not grant duplicate ownership.

The regression test exercises the shared `lock_or_sleep` helper and checks that contention registers a callback and sleeps on the selected queue (`crates/services/ogc-usbgecko/src/lib.rs:641-663`). It does not invoke the callback, model an unlock racing with sleep, or verify that a waiter wakes and retries. The PowerPC test build is recorded as failing on existing unsafe test-helper errors; tests did not run. No Dolphin DOL or hardware verification covers the queue behavior. Therefore the production sequence passes static review, while the no-lost-wakeup claim has no executed behavioral regression evidence yet. Existing open differences are unchanged: C `usb_flush` returns void while Rust's former return mismatch was fixed separately, and remaining work is tracked above.

## Runtime regression follow-up — callback, wakeup, and retry path

Implementer: `/root`. Independent re-review: needed.

The production lock retry now uses a shared retry helper. The debugger DOL calls that helper with the production callback-registration, queue-sleep, and retry adapter. It obtains the real per-channel production wait queue, invokes the registered production EXI unlock callback, and checks that the callback broadcasts to that queue and the next lock attempt succeeds. The probe controls the lock result and sleep boundary. This exercises callback invocation and retry logic under Dolphin; it does not create a second blocked LWP waiter or validate real EXI contention.

The HID descriptor DOL now also checks that conversion leaves an incomplete trailing entry unchanged. On Dolphin 2603-415, `debugger_probe` passed 16 checks (DOL SHA-256 `267717788906734af2b54cba86196e633def85bc00195d629d0e4f325826b323`), and `hid_descriptor_probe` passed 6 checks (DOL SHA-256 `8602985506489a670033235050133987c2b6cb0df752bdb4356adf120f039a19`). `cargo xtask build-dolphin-probes`, PowerPC library check, and `git diff --check` passed. The USB Gecko PowerPC unit-test target still does not compile because of 14 existing unsafe function-pointer cast and unsafe-call errors; those tests did not run. No Wii hardware verification was performed. The crate remains open for independent review and real EXI scheduling evidence.

## 2026-10-05 completion review

`ogc-usbgecko` is now `reviewed`. The current source and recorded Dolphin regression reconcile the transfer command sequence, callback-backed EXI wait/wakeup/retry path, receive destination behavior, `flush` signature, range checks, and C delays. Real EXI contention and scheduling on Wii hardware remain runtime validation; they are not an unnamed source mismatch.
