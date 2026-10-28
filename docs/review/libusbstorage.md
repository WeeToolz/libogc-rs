# USB storage parity review

Status: source coverage is recorded below. Independent re-review by `worker_d` on 2026-10-05 found two public API parity mismatches and failure paths that conflict with the Real Implementation Policy. Keep `ogc-usbstorage` and `ogc-usbstorage-sys` open. Rust line references are relative to `RustPorts/libogc-rs`.

## Scope and coverage

Compared the three crate manifests and every Rust source file:

- `crates/services/ogc-usbstorage/src/lib.rs`: all public wrappers, `Capacity`, result helpers, and crate tests (lines 1-298).
- `crates/services/ogc-usbstorage-sys/src/lib.rs`: all constants, C-layout types, exported functions, disc interface callbacks, and tests (lines 1-577).
- `crates/services/ogc-usbstorage-sys/src/wii_backend.rs`: all target operations from initialization and device enumeration through SCSI transport, LUN mount, raw ioctl, disc callbacks, removal, and shutdown (lines 1-1614).
- `crates/services/ogc-usbstorage-lifecycle/src/lib.rs`: full generic close policy and its tests (lines 1-145).
- C API header: `C-sources/libogc/gc/ogc/usbstorage.h` (lines 1-96).
- C implementation: `C-sources/libogc/libogc/usbstorage.c` (lines 1-1011), including its non-header `USBStorage_Inquiry` symbol and the `DISC_INTERFACE` callbacks.

I listed every Rust module and public item, then traced every target public operation to its backend function and compared the matching C function or callback. Private transport helpers were reviewed with their callers. The review covers the Rust and libogc files above. It does not audit the internals of the transitive `ogc-usb-sys`, LWP, system, or filesystem crates; their low-level USB, mutex, alarm, and ABI behavior remains unchecked here.

## Confirmed behavior or interface differences

- **Fixed, needs independent re-review:** C `USBStorage_Open` performs a BOT reset for the non-USB2 path. Rust now derives the same mode from the bulk-out packet size and descriptor value and performs the reset on the same path. Rust intentionally preserves a negative open-time reset result instead of allowing the later GetMaxLUN request to overwrite it; that narrower difference is an accepted safety/error-preservation deviation.
- **Fixed with a safety bound, needs independent re-review:** Rust now allocates the per-handle sector-size table from the reported LUN count like C and no longer imposes the old fixed LUN/backend-slot storage. A GetMaxLUN byte of `255` would require a count of 256, which cannot be represented by the public `u8 max_lun` field; Rust rejects that unrepresentable count instead of wrapping it to zero. This is an accepted bounds-safety deviation.
- **Fixed, needs independent re-review:** `UsbStorageHandle` now owns the live mutex, alarm, dynamically allocated sector-size table, and 32-byte-aligned transfer buffer represented by its public fields. Open initializes those resources and close/open-failure cleanup destroys/frees them. The former four-handle process-global slot table and shared transfer mutex are removed.
- **Accepted safety deviation:** C close always attempts resource cleanup and returns zero (`usbstorage.c:552-574`) without issuing a cache flush. Rust deliberately flushes each mounted LUN before transport close to reduce the risk of losing cached writes. Flush and transport-close failures do not prevent cleanup or change the successful close result, so the extra flush is treated as a data-integrity safety addition rather than a parity blocker. Invalid handles or transfer-mutex acquisition failure can still return an error before those operations begin.
- **Fixed, needs independent re-review:** the DISC startup callback now performs the full Rust-owned USB initialization before storage initialization, matching C's `USB_Initialize` then `USBStorage_Initialize` order. Public storage initialize/deinitialize no longer start or stop the USB host subsystem themselves, matching C's subsystem boundary.
- **Fixed, needs independent re-review:** BOT retry termination and CSW residue result handling already match C, and this completion pass restores C's transfer chunk selection: 4 KiB in the USB1 path and 16 KiB in the USB2 path. Rust may still use its aligned per-handle bounce buffer where C can transfer directly from an already-suitable caller buffer; that is an internal implementation/architecture difference and does not change the command bytes, chunk sizes, or returned transfer result.
- **Fixed, needs independent re-review:** BOT reset uses the C one-second control-request timeout and the same 60 ms / 10 ms / 10 ms reset-and-halt-clear delays through the Rust-owned asynchronous control path.
- **Fixed, needs independent re-review:** C LUN mount waits 50 microseconds, retries clear-errors after a reset, ignores the reset/second-clear/Inquiry results, and returns the final Read Capacity result (`usbstorage.c:592-615`). Rust now follows that sequence through a production-shared helper: intermediate reset, clear, and Inquiry failures do not short-circuit Read Capacity, and invalid capacity still maps to `USBSTORAGE_INVALID_LUN`.
- **Fixed, needs independent re-review:** C `USBStorage_Read` and `USBStorage_Write` remount after more than 60 seconds idle and use a temporary 10-second BOT timeout for that remount and the following transfer. Rust now uses the same decision and timeout scope without mutating a process-global timeout. Rust retains pointer/range/arithmetic validation as accepted safety checks.
- **Fixed, needs independent re-review:** Read/Write now apply C's exact CSW status rule: nonzero status maps to `USBSTORAGE_ESTATUS` only when the BOT cycle result is positive.
- **Fixed, needs independent re-review:** C `USBStorage_StartStop` gives non-immediate commands a 10-second CSW timeout and immediate commands the default timeout (`usbstorage.c:691-721`). Rust now uses an asynchronous Rust-owned CSW read with the same timeout selection. To satisfy the repository safety policy, Rust reports a failed endpoint halt-clear if timeout/error cleanup itself fails instead of discarding that cleanup error as C does.
- **Fixed, needs independent re-review:** C `USBStorage_ReadCapacity` copies the raw READ CAPACITY last-LBA field into `n_sectors`; Rust now matches that exact result instead of converting it to a sector count with `last_lba + 1`. The shared production decoder and capacity probe regression use the same raw-field semantics.
- C `USBStorage_Suspend` ignores the USB suspend result and returns success after setting the flag (`usbstorage.c:797-809`). Rust now matches this result/state behavior: it attempts the Rust-owned USB suspend operation, ignores that transport result, marks the handle suspended, and returns `USBSTORAGE_OK`. This bounded fix still needs independent review and hardware validation.
- For an already-mounted non-DVD device, C `__usbstorage_IsInserted` returns true without checking device enumeration (`usbstorage.c:821-843`). Rust now follows that mounted-path rule and relies on the asynchronous device-removal callback to clear `DISC_MOUNTED`. The bounded change still needs independent review and hardware validation.
- C `__usbstorage_ClearStatus` always returns true (`usbstorage.c:950-953`). Rust now returns the same result on Wii, without sending TEST UNIT READY or checking mount state (`wii_backend.rs:1680-1689`).
- **Fixed, needs independent re-review:** C `__usbstorage_Shutdown` ignores close errors and always returns true (`usbstorage.c:955-960`). Rust now attempts the Rust-owned close path when a device is open, ignores that close result for the `DISC_INTERFACE` shutdown return value, resets global disc state, and returns true like C. A regression injects a close failure and verifies success plus state reset.
- The header declares variadic `USBStorage_ioctl(int request, ...)` and C extracts `raw_device_command *` (`usbstorage.h:76`; `usbstorage.c:970-998`). Rust exposes a fixed `ioctl(request, *mut RawDeviceCommand)` signature (`ogc-usbstorage-sys/src/lib.rs:230-244`; `wii_backend.rs:1159-1195`). This is not ABI-call-compatible with the variadic C function, even though the request and data structure behavior align for the supported request.
- **Fixed:** C exports `USBStorage_Inquiry` as a global function in the implementation, even though it is not declared in the header (`usbstorage.c:617-650`). Rust now exposes the matching Rust-owned `inquiry` API and routes it through the production SCSI INQUIRY transport instead of keeping that operation private. Independent compile/re-review is still required.

## Differences that need further validation

- Rust uses Rust-owned allocation and LWP yielding instead of C's private storage heap and wait queue. Those are implementation architecture differences; public storage initialization now has the same subsystem boundary and per-handle resources are allocated during open.
- C registers an asynchronous device-removal callback on each opened device (`usbstorage.c:532-550`). Rust now registers the Rust-owned `ogc-usb-sys` removal callback from `open`, and the callback clears `DISC_MOUNTED` like C's `__usb_deviceremoved_cb`. Registration failure is recorded while open preserves C's success behavior. Callback delivery/cancellation races still need hardware validation.
- C and Rust now use the same USB2 predicate and corresponding 4 KiB/16 KiB BOT transfer chunk sizes. Hardware validation is still useful for IOS timing and device compatibility.
- Rust now owns the per-handle alarm and submits asynchronous V0/V5 bulk requests with the C-compatible timeout values. The waiting mechanism uses Rust LWP yielding instead of C's private wait queue; hardware callback/timing races still require runtime validation.
- C `USBStorage_ReadCapacity` copies big-endian response bytes into native `u32` fields (`usbstorage.c:652-669`); on Wii PowerPC this is big-endian. Rust explicitly decodes big-endian values (`wii_backend.rs:768-807`). The target result matches by inspection, but the tests do not establish hardware response behavior.

## Rust-only additions

- The safe-ish wrapper crate adds `try_*` helpers that map integer statuses to `Result`, plus `Capacity` (`ogc-usbstorage/src/lib.rs:9-29,67-80,87-265`). These are Rust API conveniences, not C symbols.
- Rust exposes explicit `flush`/`flush_default` and uses SCSI SYNCHRONIZE CACHE before close (`ogc-usbstorage-sys/src/lib.rs:138-164`; `wii_backend.rs:886-954`). C has no matching API or close flush.
- Rust adds the generic `CloseOperations` lifecycle policy crate to make the flush-before-close sequence independently testable (`ogc-usbstorage-lifecycle/src/lib.rs:1-145`).
- Rust's host-configuration stubs and compile-time PowerPC restriction (`ogc-usbstorage-sys/src/lib.rs:1-5,88-336`) support builds/tests. They have no matching C runtime API.
- The former fixed backend-slot/handle-count addition has been removed. Remaining Rust-only validation is documented as safety behavior rather than capacity parity work.

## Parity observed

- All documented error constants and BOT request/signature/SCSI command values match the C header/source (`usbstorage.h:15-40`; `usbstorage.c:73-101` and matching constants in `wii_backend.rs:12-49`).
- For the ordinary path, the header's Initialize, Deinitialize, Open, Close, Reset, GetMaxLUN, MountLUN, Suspend, IsDVD, ReadCapacity, Read, Write, and StartStop operations have matching Rust API operations. `DiscInterface` callbacks and feature/type values also map to `__io_usbstorage` (`usbstorage.h:65-86`; `usbstorage.c:1000-1009`; `ogc-usbstorage-sys/src/lib.rs:339-375`). The individual behavior differences above still apply.
- BOT CBW/CSW signatures, tag checking, command fields, sense codes, Inquiry/Capacity/Read10/Write10/StartStop commands, and LUN selection align at the protocol-field level (`usbstorage.c:226-284,352-384,617-721,723-795`; `wii_backend.rs:500-562,696-807,1222-1384`). Rust adds stricter validation and differs in retry, timeout, buffer, residue, and result handling as listed above.

## Unchecked areas

All Rust modules and exported items in the three claimed crates and the corresponding C header/implementation were inspected. The exact low-level USB request behavior, LWP mutex/queue/alarm behavior, device-removal callback races, and actual hardware timing remain unchecked because they are implemented in transitive crates or require Wii hardware. This is a completed source parity review with these explicit limits; it does not establish runtime equivalence.

## Independent re-review: close continues after flush errors

Reviewer: `/root/reviewer_d`. Implementer: `/root/worker_b`. Implementation commit: `dabc917` (`Continue USB storage close after flush errors`). Scope: the close-policy change in `ogc-usbstorage-lifecycle` and its use by the Wii storage backend. The other confirmed USB-storage differences above remain open; this bounded fix does not close the three-crate review.

### Result: PASSED

C declares `USBStorage_Close` as returning `s32` (`../../C-sources/libogc/gc/ogc/usbstorage.h:69`). It clears mounted/LUN/VID/PID state, attempts device close if open, destroys the per-handle mutex and alarm, frees sector-size and transfer buffers, zeroes the handle, sets `usb_fd=-1`, and returns `0` (`../../C-sources/libogc/libogc/usbstorage.c:552-574`). It performs no close-time cache flush.

Rust's lifecycle policy iterates the reported LUN range, skips unmounted LUNs, calls `flush_lun` for every mounted LUN while discarding each flush result, and then calls `close_transport` (`crates/services/ogc-usbstorage-lifecycle/src/lib.rs:12-25`). Its adapter defines a LUN as mounted when the cached sector size is nonzero and maps flush to `synchronize_cache_locked`; transport close maps to `usb::close_device` (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:995-1015`). Therefore a negative flush result no longer prevents later LUN flush attempts or transport close, and it does not become the close return value. If transport close succeeds, backend close releases the slot, resets the handle, clears the global mounted/capacity/device state, and returns `USBSTORAGE_OK` (`wii_backend.rs:1018-1033`). This matches C's success result and cleanup outcome when the Rust-only flush operation fails but transport close succeeds.

The order remains flush each mounted LUN, then close transport. This is an intentional Rust addition because C does not flush at close. The fix changes only whether flush errors short-circuit this existing sequence; the lifecycle trait does not mutate the mounted-LUN predicate or close result itself. At this revision, a negative transport-close result was returned before slot/handle/global cleanup (`wii_backend.rs:1018-1031`), unlike C's unconditional cleanup and success return (`usbstorage.c:559-573`). That separate mismatch remained open at this revision; the later handoff below records its fix. The Rust path also left global state marked mounted during the added flush/transport operations and cleared it only after successful transport close; the later handoff moves global state clearing before close operations.

Regression coverage verifies order, that both mounted-LUN flushes are attempted despite errors, that transport close is still attempted, that flush errors do not replace successful close result, and that transport-close errors remain returned (`ogc-usbstorage-lifecycle/src/lib.rs:77-127`). I ran `cargo test -p ogc-usbstorage-lifecycle`: all six unit tests passed; doc tests contained zero tests. No target check or hardware verification was performed in this re-review. The implementer's prior PowerPC `cargo check` compiled the adapter but is not a test run. Ledger not edited; coordinator owns the ledger update.

## Fix handoff — close transport failure cleanup

Implementer: `/root/worker_b`. Implementation commit: `6b95b4b` (`Match USB storage close cleanup behavior`). Independent re-review: needed; review thread owns it.

C clears mounted, LUN, VID, and PID state before attempting the USB device close, ignores the transport-close result, frees its resources, zeroes the handle, sets `usb_fd` to `-1`, and returns zero (`../../C-sources/libogc/libogc/usbstorage.c:552-574`). The Rust close policy now ignores the transport-close result after it attempts all existing mounted-LUN flushes. The backend clears global mounted/capacity/device state before close operations and always releases the slot, resets the handle with `usb_fd = -1`, and returns `USBSTORAGE_OK` after the policy completes, including when `close_device` reports failure.

Regression coverage checks that a negative transport-close result does not alter the policy sequence or result, and that close cleanup clears the handle, slot, and global disc state. The previously recorded flush behavior remains: Rust attempts flushes before transport close and ignores flush failures; C does not flush at close. Invalid handles and failure to acquire Rust's transfer mutex can still return errors before transport close; this handoff covers the transport-close failure path. The independent review of the earlier flush-error behavior remains separate.

Validation: `cargo test -p ogc-usbstorage-lifecycle` passed all six unit tests. Formatting and `git diff --check` passed. `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled the backend regression test. The tests were not run. No hardware verification was performed. Ledger not edited; coordinator owns it.

## Independent re-review — close cleanup after transport failure

Reviewer: `/root/reviewer_a`. Implementer: `/root/worker_b`. Implementation commit: `6b95b4b` (`Match USB storage close cleanup behavior`); handoff note commit: `d27ffb6`. Scope: close handling when a mounted-LUN flush or USB transport close reports an error. This review checks the close-cleanup handoff only; other USB-storage findings above remain open.

### Result: PASSED

C `USBStorage_Close` clears global mounted/LUN/VID/PID state before the USB close attempt, calls `USB_CloseDevice` only for an open handle and ignores its result, then destroys/frees handle resources, zeroes the handle, sets `usb_fd = -1`, and returns `0` (`../../C-sources/libogc/libogc/usbstorage.c:552-574`). The Rust backend now clears its corresponding global mounted/capacity/device/disc state before the cleanup sequence (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:168-174,1101-1103`). Its close policy attempts each LUN with nonzero cached sector size, ignores flush results, then attempts `usb::close_device` and ignores its result (`crates/services/ogc-usbstorage-lifecycle/src/lib.rs:12-22`; backend adapter at `wii_backend.rs:1110-1127`). It then releases and clears the backend slot, resets the handle, sets `usb_fd = -1`, and clears global state again (`wii_backend.rs:412-420,1129-1131`), and returns `USBSTORAGE_OK` (`wii_backend.rs:1133`). Thus transport-close failure no longer prevents cleanup or changes the close result, matching C's cleanup and result for this case.

Rust still flushes mounted LUNs before transport close, while C does not flush at close. This is an accepted data-integrity safety deviation; its failures do not prevent transport close or cleanup. Clearing global state before these flushes does not block them: the adapter checks the handle's `max_lun` and slot sector-size array, and `synchronize_cache_locked` does not consult the global mounted flags. The lifecycle unit tests cover attempts after flush failures and disregard of a negative transport-close result (`ogc-usbstorage-lifecycle/src/lib.rs:77-119`). The backend cleanup regression checks slot release, handle reset, and global state reset (`wii_backend.rs:305-330`), but calls the cleanup helper directly rather than injecting a transport failure through `close`; the control flow in `close` nevertheless unconditionally calls that helper after the policy returns.

Validation: I ran `cargo test -p ogc-usbstorage-lifecycle`; all six unit tests passed and doc tests had zero tests. The PowerPC backend test target was reported by the implementer as compiled with `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu`; those tests were not run. No hardware verification was performed. Ledger not edited; coordinator owns it. The lifecycle close-cleanup handoff passes independent static review; this result does not close the crate's other recorded findings.

## Fix handoff — BOT reset timing and endpoint-clear order

Implementer: `/root/worker_b`. Implementation commit: `88b6840` (`Match USB storage BOT reset timing`). Independent re-review: needed; review thread owns it.

C sends the class BOT reset, restores the prior `usbtimeout` immediately after the control request returns, sleeps 60 ms, clears the IN halt, sleeps 10 ms, clears the OUT halt, sleeps 10 ms, then returns the class-reset result (`../../C-sources/libogc/libogc/usbstorage.c:386-396`). Rust now performs the same endpoint-clear order and delays. It continues through both endpoint clears and delays even if the class reset or a halt clear fails, ignores halt-clear results, and returns the class-reset result, matching C. Helper-level regressions check the exact six-step order and delays, continued calls after reset errors, and that clear-halt errors do not replace the class-reset result.

Rust `ogc_usb_sys::write_ctrl_msg` is a synchronous control API with no per-request timeout input or mutable timeout setting. This change therefore does not implement C's temporary one-second timeout override or timeout restoration; that difference remains open. Other BOT timeout and reset-retry behavior remain outside this bounded fix.

`cargo fmt -p ogc-usbstorage-sys -- --check`, `rustfmt --edition 2021 --check crates/services/ogc-usbstorage-sys/src/wii_backend.rs`, `git diff --check`, and `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed. The target check compiled the helper regression test, but tests were not run. No hardware verification is claimed. Ledger not edited; coordinator owns it.

## Fix handoff — ClearStatus result

Implementer: `/root/worker_c`. Independent review: needed; review thread owns it.

C's `__usbstorage_ClearStatus` returns true without checking device, mount, or command status (`../../C-sources/libogc/libogc/usbstorage.c:950-953`). Rust now returns true on Wii through the same unconditional result. A focused regression checks the result helper. This does not change the platform guard: non-Wii host calls still return false because this backend is Wii-specific.

Validation: `cargo fmt -p ogc-usbstorage-sys -- --check`, `git diff --check`, and `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed. The test target compiled; tests were not run. No hardware verification was performed. Ledger not edited; coordinator owns it.

## Independent re-review — BOT reset timing and endpoint-clear order

**Result: NEEDS_CHANGES.** Reviewer: `/root/reviewer_d`. Implementer: `/root/worker_b`. Implementation commit: `88b6840` (`Match USB storage BOT reset timing`).

The reset sequence matches C: issue the class reset, wait 60 ms, clear the IN halt, wait 10 ms, clear the OUT halt, wait 10 ms, and return the class-reset result. C continues through endpoint-clear errors (`usbstorage.c:386–396`); Rust's `run_bot_reset_sequence` does the same (`ogc-usbstorage-sys/src/wii_backend.rs:574`).

Two differences remain in this reset path. First, `mount_lun` calls `mass_storage_reset` directly after `clear_errors` fails (`wii_backend.rs:1174`), without `TransferGuard`. C calls public `USBStorage_Reset`, which takes the device mutex (`usbstorage.c:576, 601`). A Rust transfer can interleave with this fallback reset. Second, Rust's synchronous `usb::write_ctrl_msg` has no request timeout (`wii_backend.rs:584`; `ogc-usb-sys/src/lib.rs:3598`), while C temporarily sets a one-second timeout and uses its alarm-backed control helper (`usbstorage.c:180, 388`).

Helper-level tests check order and result handling, but do not exercise lock coverage or the real USB timeout. The PowerPC test target compiled; tests were not run. `git diff --check` passed. No hardware verification was performed. Other storage parity differences in this note remain open.

## Independent re-review — ClearStatus result

**Result: PASSED for the bounded fix.** Reviewer: `/root/reviewer_e`. Implementer: `/root/worker_c`. Implementation commit: `ff52dca`.

C compiles USB storage only for `HW_RVL` and its `__usbstorage_ClearStatus` returns true without checking state or issuing I/O (`usbstorage.c:30, 950–953`). Rust's interface callback delegates to the Wii backend; on Wii it returns true unconditionally, with no command or state check (`ogc-usbstorage-sys/src/lib.rs:341–350, 406–409`; `wii_backend.rs:122–124, 1684–1693`). This matches the supported C platform path. Rust's broader PowerPC interface surface has no C GameCube implementation counterpart and predates this fix.

The helper test checks only the result, not the public callback or absence of I/O. Static call-path inspection confirms those behaviors. PowerPC checks passed; the target test code compiled, but tests were not run. Formatting and `git diff --check` passed. No hardware verification was performed. Other storage findings remain open.

## Independent current-source audit of remaining storage behavior

Reviewer: `/root/reviewer_e`. The ClearStatus fix and reset sequence order/result pass their bounded checks. The previously recorded reset timeout and unguarded mount fallback remain confirmed (`usbstorage.c:386–396, 576–606`; Rust `wii_backend.rs:574–606, 1134–1145, 1174–1180`).

Additional current differences remain:

- C performs an open-time BOT reset when the interface is not USB2; Rust proceeds from interface open to GetMaxLUN without this mode check/reset (`usbstorage.c:449–465, 493–495`; Rust `wii_backend.rs:480–541, 928–991`).
- C stops retrying a BOT cycle after `USBSTORAGE_ETIMEDOUT`; Rust retries any negative CBW/data/CSW result after reset (`usbstorage.c:294–340`; Rust `wii_backend.rs:754–802`). C selects a 4 KiB chunk for USB1 and 16 KiB for USB2, while Rust always uses a 16 KiB buffer (`usbstorage.c:92–93, 296–299, 460–463`; Rust `wii_backend.rs:42–56, 672–703`).
- C reports residue but does not turn nonzero residue into a cycle error; Rust returns `USBSTORAGE_EDATARESIDUE`, including raw ioctl where C passes no residue output (`usbstorage.c:258–284, 286–349, 970–988`; Rust `wii_backend.rs:788–800, 1275–1293`). For Read/Write, C rejects nonzero CSW status only when result is positive; Rust rejects it for any nonnegative result (`usbstorage.c:747–753, 784–790`; Rust `wii_backend.rs:1375–1379, 1438–1442`). StartStop checks status in both.
- MountLUN error precedence differs: C ignores reset and Inquiry results and returns ReadCapacity's result; Rust returns early on reset, second clear-error, or Inquiry failure (`usbstorage.c:592–614`; Rust `wii_backend.rs:1174–1194`). C remounts idle devices after 60 seconds with a temporary 10-second timeout; Rust has no last-use/remount behavior (`usbstorage.c:743–755, 780–792`; Rust `wii_backend.rs:1320–1444`).
- C registers asynchronous device-removal notification at open and clears mounted state in the callback (`usbstorage.c:136–139, 536`). Rust now does the same through the Rust-owned USB removal-notify API; the old polling-only removal behavior is fixed. The older eight-LUN limit cited here is also stale: current Rust supports the full C-representable `u8` count range. Rust still aborts open on configuration get/set errors that C ignores (`usbstorage.c:401, 486–491`). This is an intentional safety deviation: Rust stops on a failed USB configuration operation and returns the transport error instead of continuing with uncertain device state.
- Shutdown/deinitialize return and cleanup paths differ: C ignores close errors, continues queue cleanup, and clears initialized state; Rust returns false or early on close/deinit failure (`usbstorage.c:955–968`; Rust `wii_backend.rs:459–477, 1695–1705`). The public handle fields are also not runtime-equivalent if handles cross the C/Rust boundary: C stores per-handle mutex/alarm/buffers, while Rust uses placeholders and process-global backend slots (`usbstorage.h:28–51`; Rust `lib.rs:42–60`, `wii_backend.rs:42–56, 386–409`).

The earlier close fix passes its bounded cleanup case. Rust's extra flush-before-close is an accepted data-integrity safety deviation, not a parity blocker. `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled tests, but tests were not run. No hardware verification was performed. C/Rust behavior for whether `usb::initialize_vendor` fully replaces C's `USB_Initialize` remains outside this audit.
### Additional findings from the independent current-source audit

Reviewer: `/root/reviewer_e`. C ReadCapacity does not request CSW status and accepts nonnegative cycle results (`C-sources/libogc/libogc/usbstorage.c:652-668`); Rust rejects nonzero CSW status (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:877-915`). C ignores GetConfiguration/SetConfiguration results, while Rust aborts open on either failure (`usbstorage.c:486-491`; `wii_backend.rs:951-969`). This difference is accepted as a safety deviation: Rust stops on the failed configuration operation and reports the error rather than continuing with uncertain device state. The bounded ClearStatus fix and BOT reset sequence order/result pass; reset timeout and transfer-guard differences remain. Validation: `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed; test code compiled but tests did not run. No hardware verification was performed.

### Independent re-review of MountLUN reset guard (`01b07a6`)

**Result: PASS for the transfer-guard fix.** Reviewer: `/root/reviewer_d`; implementer: `/root/worker_b`.

C's `USBStorage_MountLUN` fallback calls public `USBStorage_Reset`, which locks the per-handle mutex around the reset (`C-sources/libogc/libogc/usbstorage.c:576-606`). Rust's first `clear_errors` call releases its guard before the fallback acquires `TransferGuard` around `mass_storage_reset`; retry reacquires through `cycle()` (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:800-804,1197-1220`). The focused regression verifies that the operation runs with the guard held and that the guard releases after return. No lock-order or cleanup regression was found.

The one-second C control timeout remains missing: C saves `usbtimeout`, sets it to one second for reset, then restores it (`usbstorage.c:386-396`); Rust calls `usb::write_ctrl_msg` without a timeout override (`wii_backend.rs:625-648`; `ogc-usb-sys/src/lib.rs:3579-3623`). Validation: PowerPC `cargo check --tests` passed and compiled tests; tests did not run. No hardware verification was performed.

## Fix handoff — Apply C's one-second BOT reset control timeout

Implementer: `/root/worker_b`. Independent re-review: needed; review thread owns it.

C's `__usbstorage_reset` saves the configured `usbtimeout`, sets it to one second while `__USB_CtrlMsgTimeout` waits for the asynchronous class-reset control request, then restores the previous value before the 60 ms reset delay and endpoint clears (`../../C-sources/libogc/libogc/usbstorage.c:180-193,386-396`). Rust previously submitted the reset through synchronous `usb::write_ctrl_msg`, which has no per-request timeout. Rust now submits the reset through the asynchronous control API and waits for its completion with a one-second deadline. The timeout is scoped to the reset request, so it cannot change the timeout behavior of other USB requests. BOT reset order, endpoint clear handling, and following delays are unchanged.

Regression tests check that the reset deadline equals one second, returns `USBSTORAGE_ETIMEDOUT` when no completion arrives by the deadline, and returns a completed request result before timeout. `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu`, `rustfmt --edition 2021 --check crates/services/ogc-usbstorage-sys/src/wii_backend.rs`, and `git diff --check` passed. The PowerPC test target compiled; tests were not run. No hardware verification was performed.

## Fix handoff — Guard the MountLUN reset fallback

Implementer: `/root/worker_b`. Independent re-review: needed; review thread owns it.

C `USBStorage_MountLUN` calls public `USBStorage_Reset` when its first ClearErrors call fails (`../../C-sources/libogc/libogc/usbstorage.c:592–606`). `USBStorage_Reset` locks the device mutex around the BOT reset (`usbstorage.c:576–584`). Rust previously called `mass_storage_reset` directly on this fallback path, so another transfer could overlap the reset. Rust now runs the fallback reset under `TransferGuard`, using the same guard helper as the public reset wrapper. The separate one-second control-timeout difference remains open.

A regression test checks that the reset operation runs while its guard is held and that the guard is released afterward. `cargo fmt -p ogc-usbstorage-sys`, `git diff --check`, and `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed. The PowerPC test target compiled; tests were not run. No hardware verification was performed. The shared ledger is managed by the coordinator.

## Fix handoff — Stop BOT retries after timeout

Implementer: `/root/worker_b`. Independent re-review: needed; review thread owns it.

C `__cycle` stops before another attempt when the failed cycle result is `USBSTORAGE_ETIMEDOUT`; a reset timeout also makes the cycle result terminal (`../../C-sources/libogc/libogc/usbstorage.c:286-340`). Rust now ends the cycle after a transfer or reset timeout. Other negative cycle results remain retryable. C also retries those results when the reset returns a non-timeout error, while preserving the cycle error; Rust now follows that behavior instead of returning the reset error. The shared retry decision is used for CBW, data, and CSW failures.

Regression tests cover transfer timeout, reset timeout, retryable errors after a successful reset, and retryable errors after a non-timeout reset error. `cargo fmt -p ogc-usbstorage-sys -- --check`, scoped `git diff --check`, and `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed. The PowerPC test target compiled; tests were not run. No hardware verification was performed. The shared ledger is managed by the coordinator.

## BUG FIX handoff — Match ReadCapacity CSW-status handling

Implementer: `/root/worker_b`. Independent reviewer: `/root/reviewer_b`.

C `USBStorage_ReadCapacity` checks only whether `__cycle` returned a negative result. It copies the response and returns success for every nonnegative cycle result, without requesting or checking CSW status (`../../C-sources/libogc/libogc/usbstorage.c:652-668`). Rust previously returned `USBSTORAGE_ESTATUS` when ReadCapacity received a nonzero CSW status (`wii_backend.rs:1050-1088`). Rust now uses the cycle result only: negative transport/cycle errors still return, while a nonnegative result proceeds to parse the capacity response.

Unit regressions cover a successful cycle with nonzero CSW status and preservation of a negative cycle result. The cycle-result handling and response decoding use a shared production helper. A focused DOL probe uses that helper with fixed response bytes; it checks the returned capacity and that a negative BOT result remains an error. It does not simulate USB transport.

### Independent re-review — PASSED

The helper is used directly by the production `read_capacity_inner` path (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:1083-1104`); it ignores CSW status and preserves negative cycle results (`crates/services/ogc-usbstorage-sys/src/lib.rs:43-64`). This matches C's ReadCapacity status handling (`../../C-sources/libogc/libogc/usbstorage.c:652-668`). The DOL probe is registered by `51e168f` and exercises the same helper with fixed expected bytes (`probes/ogc-system-probe/src/usbstorage_capacity_probe.rs:1-23`). Dolphin artifact `target/dolphin-validation/1791171006-6243c315380d/suite.json` records Dolphin 2603-415, `passed: true`, 4 passes, and 0 failures. Its DOL SHA-256 is `437325eb866d9c9fd2972795069aee15690cf92c458f90d8dd4d18e9d01ea9df`. This verifies the probe in Dolphin, not on Wii hardware. The PowerPC unit tests compiled but were not run.

The READ CAPACITY sector-count difference is now fixed: C copies the raw last-LBA field directly to `n_sectors`, and Rust now does the same. The production decoder, regression, and Dolphin probe expectation all use the raw field value. No CSW-residue mismatch remains after the follow-up below. The shared ledger is managed by the coordinator.

## BUG FIX follow-up — Preserve the BOT result when CSW residue is nonzero

Implementer: `/root/worker_b`. Independent re-review: needed; coordinator owns the ledger handoff.

C `__cycle` writes CSW residue only through its optional `_dataResidue` output pointer and does not convert nonzero residue into an error. All C call sites pass `NULL`, including ReadCapacity (`../../C-sources/libogc/libogc/usbstorage.c:286-349,362-371,628-668,750-787,987`). Rust previously changed a successful CSW result into `USBSTORAGE_EDATARESIDUE` whenever residue was nonzero. Rust now preserves the BOT cycle result through `bot_csw_cycle_result`, which the production cycle path calls after `read_csw`.

The ReadCapacity DOL probe calls the same helper as production with nonzero residue, then passes the result to the shared ReadCapacity decoder. It checks success, decoded sector size, and sector count while also using nonzero CSW status. A negative BOT result remains an error. `cargo fmt -p ogc-usbstorage-sys`, scoped `git diff --check`, PowerPC test-target compilation, and the focused xtask artifact-registration test passed. The full xtask format check found an existing formatting diff in `xtask/src/inventory.rs`; I did not edit that file. `cargo xtask build-dolphin-probes` is blocked by the concurrent `modplay-capi` error at `crates/abi/modplay-capi/src/lib.rs:534` (function `mod_set_mod_ptr` has no return expression). Therefore, this follow-up DOL was not rebuilt or run in Dolphin yet. No hardware verification was performed. The shared ledger is managed by the coordinator.

## BUG FIX handoff — Perform the open-time BOT reset for USB 1

Implementer: `/root/worker_b`. Independent re-review: needed; coordinator owns the ledger handoff.

C sets `usb2_mode` from the bulk OUT endpoint packet size and the open device fd: USB 2 mode requires packet size greater than 64 and `(fd >= 0x20 || fd < -1)` (`../../C-sources/libogc/libogc/usbstorage.c:449-465`). After setting configuration and alternate interface, C calls `USBStorage_Reset` when this condition is false, before GetMaxLUN (`usbstorage.c:486-500`). Rust now reads the selected bulk OUT endpoint packet size, uses the same predicate, and performs the BOT reset under `TransferGuard` when required. Rust returns a reset error and closes the open handle if reset fails; C overwrites the reset result with the later GetMaxLUN result. This difference preserves the real reset failure instead of reporting open success after reset failure.

The backend regression and DOL probe exercise the same production mode predicate with USB 1, USB 2, and fd boundary values. The probe checks only the decision; it cannot validate an actual USB reset without a device transport, so it does not simulate one. `cargo fmt -p ogc-usbstorage-sys`, probe `rustfmt --check`, scoped `git diff --check`, PowerPC `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu`, and the focused xtask artifact-registration test passed. `cargo xtask build-dolphin-probes` passed. `cargo xtask dolphin-validate --no-build --probe usbstorage_capacity_probe` passed on Dolphin 2603-415 with 5 passes and 0 failures; DOL SHA-256 is `3f6aa9195c35c7a66a31a4b652e2d9797218d41cb882fb0881a7696d8eaaa65c`. The probe verifies the predicate in Dolphin, not the actual USB reset transport. No hardware verification was performed. The shared ledger is managed by the coordinator.

## BUG FIX handoff — Support the C-representable GetMaxLUN range

Implementer: `/root/worker_b`. Independent review: needed; coordinator owns the ledger handoff.

C reads one byte as the highest LUN index, adds one, stores the count in the `u8` `max_lun` field, and allocates that many `u32` sector sizes (`../../C-sources/libogc/libogc/usbstorage.c:499-515`; `../../C-sources/libogc/gc/ogc/usbstorage.h:38`). Therefore reported indices 0 through 254 represent counts 1 through 255. A reported index of 255 wraps the C count to zero; Rust rejects this unrepresentable count instead of wrapping. Rust previously rejected all counts above eight. It now uses a 255-entry per-slot sector-size array and accepts the full representable range. For a non-timeout GetMaxLUN error, C continues with one LUN; a timeout is an exception and must abort open with `USBSTORAGE_ETIMEDOUT`.

The PowerPC unit test and the existing storage DOL probe call the same production conversion helper. They cover reported indices 0, 7, 8, 254, and 255, including the previous limit boundary and the maximum representable count. `cargo fmt -p ogc-usbstorage-sys`, probe formatting, scoped `git diff HEAD --check`, and `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed. The PowerPC test target compiled but the tests were not run. `cargo xtask build-dolphin-probes` passed, and `cargo xtask dolphin-validate --no-build --probe usbstorage_capacity_probe` passed on Dolphin 2603-415 with six passes and zero failures; DOL SHA-256 is `5ab64489faf8a5052345f421ce8e26f9157beeaf6473740d5eae2a3f7a46e6e4`. The probe checks count conversion only; it cannot query a real USB device in Dolphin. No hardware verification was performed. The shared ledger is managed by the coordinator.

### Independent review and open GetMaxLUN timeout finding

Independent reviewer: `/root/worker_a`. Commit `38344b5` **passes** for the representable LUN-count change. The review confirmed the 255-count bound, fixed expected values, shared production conversion helper, and Dolphin result (6 passes, 0 failures; SHA-256 `5ab64489faf8a5052345f421ce8e26f9157beeaf6473740d5eae2a3f7a46e6e4`). The probe validates count conversion only, not a physical GetMaxLUN request.

The review also found an adjacent, pre-existing mismatch: C returns `USBSTORAGE_ETIMEDOUT` from `USBStorage_Open` when the GetMaxLUN control request times out (`../../C-sources/libogc/libogc/usbstorage.c:499-509`). Rust currently treats every `class_get_max_lun` error as the one-LUN fallback and can report successful open (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:1187-1192`). This remains open and is assigned to `/root/worker_a`.

## BUG FIX handoff — Preserve GetMaxLUN timeout during open

Implementer: `/root/worker_a`. Independent review: needed.

The open path now aborts and returns `USBSTORAGE_ETIMEDOUT` when the GetMaxLUN request times out, matching C's timeout branch. Other GetMaxLUN errors still fall back to one LUN, and successful responses use the checked conversion from reported maximum index to representable count. The conversion/error policy is a production-shared helper, used by the PowerPC regression and the storage capacity DOL probe.

`cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled the test target; the tests did not run. `cargo xtask build-dolphin-probes` passed. `cargo xtask dolphin-validate --probe usbstorage_capacity_probe` passed on Dolphin 2603-415 with 7 passes and 0 failures; report `target/dolphin-validation/1791173727-cf4dfcf47a25/suite.json`, DOL SHA-256 `434c767ae0883d90871364984619bb1164862cbb7489d828922af4f0c4aa740f`. This validates the decision helper, not a physical GetMaxLUN request. No hardware verification was performed.

### Independent re-review — GetMaxLUN helper classification PASSED; production timeout remains open

Reviewer: `/root/reviewer_b`. Implementer: `/root/worker_a`. C assigns one LUN on any negative GetMaxLUN result, but specifically aborts open and returns `USBSTORAGE_ETIMEDOUT` for a timeout (`../../C-sources/libogc/libogc/usbstorage.c:499-509`). Rust's `lun_count_after_get_max_lun` preserves that timeout, maps other errors to one LUN, and converts successful highest-LUN indices through the existing checked count helper (`crates/services/ogc-usbstorage-sys/src/lib.rs:33-55`). The production `open` path calls that helper and routes its error through `abort_open` before slot allocation (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:1200-1208`). This matches the C timeout/fallback decision for normal cleanup success.

DOL coverage calls the same helper with fixed timeout, non-timeout error, and success inputs (`probes/ogc-system-probe/src/usbstorage_capacity_probe.rs:17-25`). Dolphin artifact `target/dolphin-validation/1791173727-cf4dfcf47a25/suite.json` records Dolphin 2603-415, 7 passes, 0 failures, and DOL SHA-256 `434c767ae0883d90871364984619bb1164862cbb7489d828922af4f0c4aa740f`; its base revision is `cf4dfcf` with `dirty_worktree: true`. The probe validates the policy helper, not a real control request or open/close transport sequence. PowerPC tests compiled but did not run. No hardware verification was performed.

Related cleanup difference: if `abort_open`'s USB close itself fails, Rust returns that close error before resetting the handle (`wii_backend.rs:1098-1106`). C's open error path calls `USBStorage_Close`, ignores the device-close result, clears the handle, and returns the original timeout (`usbstorage.c:538-555`). This is a distinct close-failure case outside the GetMaxLUN timeout/fallback decision; preserve it as an open difference or document intentional recovery semantics.

### Second independent review — GetMaxLUN timeout path still needs changes

Reviewer: `/root/reviewer_b`. This review supersedes the earlier helper-only PASS for production timeout parity. `class_get_max_lun` calls `usb::read_ctrl_msg` and forwards any negative result unchanged (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:702-718`). The wrapper calls the synchronous `ogc-usb-sys::read_ctrl_msg`; its v0 and v5 branches issue direct control-message ioctls and do not set a timer, create an async callback, or translate an elapsed transfer into `USBSTORAGE_ETIMEDOUT` (`crates/services/ogc-usb/src/lib.rs:342-360`; `ogc-usb-sys/src/lib.rs:2478-2535,3235-3265`).

C `__USB_CtrlMsgTimeout` submits an asynchronous control request, installs an alarm, and writes `USBSTORAGE_ETIMEDOUT` from the alarm callback when the wait expires (`../../C-sources/libogc/libogc/usbstorage.c:145,175-197`). Rust's helper only recognizes that custom value if a lower layer already returns it. Current production code does not produce it through its own timeout mechanism, so it can block in the synchronous ioctl or treat a generic negative result as the one-LUN fallback. The Dolphin and unit checks verify classification of fixed inputs only; they do not validate the production timeout. The open-time timeout fix is **not complete** until Rust supplies equivalent bounded waiting or the lower layer is shown to enforce and return the same timeout. No hardware verification was performed.

## BUG FIX follow-up — timed GetMaxLUN control request

Implementer: `/root`. Independent re-review: needed.

`class_get_max_lun` now submits the real asynchronous USB control request and waits for its completion with the C default timeout of two seconds (`USBSTORAGE_TIMEOUT` in `../../C-sources/libogc/libogc/usbstorage.c:88,175-197`). It returns `USBSTORAGE_ETIMEDOUT` when the deadline expires, so the existing open policy aborts instead of treating the timeout as a one-LUN fallback. Other control errors still use C's one-LUN fallback. The DMA output byte and callback state use a 32-byte-aligned static buffer. A request that completes after the caller times out retains valid storage until its callback runs; the context cannot be reused while that request is still pending.

The production wait loop uses `wait_for_get_max_lun_control_with`; unit coverage and the storage capacity DOL exercise its timeout and completion branches. PowerPC `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled the tests. `cargo test -p ogc-usbstorage-sys --target powerpc-unknown-linux-gnu --no-run` could not link because the installed linker rejects the emitted GNU linker flags, so tests did not run. `cargo xtask build-dolphin-probes` passed. `usbstorage_capacity_probe` passed 9 checks on Dolphin 2603-415 (DOL SHA-256 `0f95372c54839d023534147b9b7523e32d11596a3f05cc7bd8adf43fe85fc305`). The DOL tests the production deadline engine, not IOS control completion with a physical USB device. No hardware verification was performed. Other storage findings remain open, and independent review is required.


## Independent re-review — stop BOT retries after timeout

Reviewer: `worker_c`. Implementer: `worker_b` (commit `0147421`). I inspected the complete `cycle_locked` diff and the shared retry helper. C `__cycle` breaks before a further attempt when the prior cycle result is `USBSTORAGE_ETIMEDOUT`, promotes a reset timeout to the returned timeout, retries other negative results, and retains the cycle error when reset has a different negative result (`../../C-sources/libogc/libogc/usbstorage.c:286-340`). Rust now does the same for CBW, data, and CSW failures. The regression cases cover each retry decision. I found no remaining mismatch in this handoff. Other USB storage findings, including the one-second control timeout, remain open. I ran no checks for this static re-review; implementation validation is recorded above. No hardware verification was performed.

## 2026-10-05 fix handoff — preserve open error across cleanup close failure

Implementer: `/root`. C's open-error path calls `USBStorage_Close`, ignores any USB device-close failure, clears the handle, and returns the original open error. Rust `abort_open` now follows the same result and cleanup rule: it always attempts `usb::close_device`, resets the handle with `usb_fd = -1`, and returns the original open error even when the close operation fails. The production path and the regression share `abort_open_with`; the regression injects a negative close result and verifies one close attempt, full handle reset, and preservation of `USBSTORAGE_ETIMEDOUT`.

PowerPC test code compiled with `cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu`; tests did not run. Independent static review is required. This closes the recorded `abort_open` close-failure mismatch only. The timed GetMaxLUN production change still needs independent review, and the other named USB-storage parity differences in this note remain open or require explicit safety classification before the crate can be marked reviewed.


### Independent re-review — abort-open close-failure cleanup PASSED

Reviewer: `review_usb_cleanup`. Implementer: `/root` (commit `3011abe`). C's `free_and_return` path calls `USBStorage_Close` once on a negative open result, then returns the original `retval` (`../../C-sources/libogc/libogc/usbstorage.c:538-555`). `USBStorage_Close` calls `USB_CloseDevice` only when `usb_fd != -1`, ignores that result, clears the handle, and sets `usb_fd` to `-1` (`usbstorage.c:552-574`). Rust's production `abort_open` invokes `abort_open_with` with `usb::close_device`; the shared helper calls the close operation once, discards only its cleanup result, resets the complete handle to default with `usb_fd = -1`, and returns the original open error (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:1322-1335`). Every caller is reached after successful `open_device`, so the descriptor is live when cleanup starts. The regression directly calls this production-shared helper, injects a failing close, and checks one close attempt, the original `USBSTORAGE_ETIMEDOUT`, and reset descriptor/configuration/LUN state. I found no mismatch in this cleanup handoff. PowerPC test code compiled; tests did not run. No hardware verification was performed. This review does not cover the timed GetMaxLUN implementation or other open storage findings.

## 2026-10-05 fix handoff — mounted-device removal parity

Implementer: OpenAI. Independent re-review: needed.

C registers `USB_DeviceRemovalNotifyAsync` after a successful open. Its callback only clears the global mounted flag. While that flag remains set, `__usbstorage_IsInserted` returns true immediately for non-DVD media and uses `USBStorage_ReadCapacity` only for DVD media. Current Rust now registers the Rust-owned `ogc-usb-sys::device_removal_notify_async` callback from `open`; the callback clears `DISC_MOUNTED`. The mounted `is_inserted` path no longer enumerates USB devices for non-DVD media and uses the existing DVD capacity check only when the cached sector size is 2048. This matches the C state transition and removes the earlier device-id/VID/PID polling difference from the mounted path.

C does not fail `USBStorage_Open` when removal-notify registration fails. Current Rust aborts open on that failure only for the registered default-disc handles, where removal notification is required to keep the published mounted state valid. For caller-owned handles, Rust records the registration failure and preserves C's open result. This is a deliberate safety difference for the default disc path. Regression coverage checks mounted non-DVD and DVD presence behavior and the removal callback state change. The current branch still needs independent review and hardware validation.

## 2026-10-05 fix handoff — suspend result/state parity

Implementer: OpenAI. Independent re-review: needed.

C `USBStorage_Suspend` returns success after attempting `USB_SuspendDevice` and sets `dev->suspended = 1` regardless of the USB suspend result. Rust now follows the same externally visible contract. The production path uses `suspend_with`: an already-suspended handle returns success without another USB request; otherwise Rust calls the Rust-owned `ogc-usb` suspend operation, intentionally discards only that operation result to match C, sets the suspended flag, and returns `USBSTORAGE_OK`.

The regression injects a negative USB suspend result and checks that the call still succeeds and the handle becomes suspended. It also checks that a second suspend does not invoke USB again. PowerPC compile/status validation and independent review are still required. Other named USB-storage differences remain open.


## BUG FIX handoff — expose USBStorage_Inquiry parity surface

Implementer: `thread-current/root`. Independent review: needed.

The C implementation exports `USBStorage_Inquiry` even though `usbstorage.h` does not declare it. Rust now exposes `ogc_usbstorage_sys::inquiry` and uses the same production SCSI INQUIRY path used by LUN mounting. The public entry validates the handle/LUN and resolves the registered backend slot before issuing the command. No C symbol is imported; the implementation remains Rust-owned.

No hardware verification was performed in this environment. PowerPC compilation and `cargo xtask review-status` are required before this bounded finding can be closed.
## 2026-10-05 fix handoff — MountLUN final-result parity

Implementer: OpenAI. Independent re-review: needed.

C `USBStorage_MountLUN` waits 50 microseconds, calls clear-errors, performs a reset and a second clear when the first clear fails, then calls Inquiry and finally Read Capacity. The reset result, second clear result, and Inquiry result are overwritten; the public result is the final Read Capacity result, except that a successful capacity with a sector size below 512 or zero sectors returns `INVALID_LUN`. Rust now uses the same sequence. The 50-microsecond delay is implemented with the LWP tick clock, rounded up to at least one tick.

The regression uses the production-shared sequence helper. It injects failures in clear-errors, reset, and Inquiry and verifies that Read Capacity still runs and its success result is returned. Separate cases verify final Read Capacity errors and invalid-capacity classification. PowerPC compile/status validation and independent review are still required. Other USB-storage findings remain open.


## 2026-10-05 safety classification — configuration-operation failures

Rust intentionally differs from C when `USBStorage_Open` cannot read or set the active USB configuration. C ignores `USB_GetConfiguration`/`USB_SetConfiguration` failures and continues. Rust closes the newly opened device, resets the handle, and returns the transport error. This difference is accepted under the repository safety policy because continuing after a failed configuration operation can use an unknown device configuration. It is not a remaining parity blocker.


## 2026-10-05 fix handoff — Read/Write CSW-status result parity

Implementer: OpenAI. Independent re-review: needed.

C `USBStorage_Read` and `USBStorage_Write` convert a nonzero CSW status to `USBSTORAGE_ESTATUS` only when the BOT cycle result is positive. A zero cycle result is returned unchanged even when CSW status is nonzero; negative cycle errors are also preserved. Rust previously treated any nonnegative cycle result with nonzero status as `USBSTORAGE_ESTATUS`. The shared Rust result helper now uses C's exact `result > 0 && status != 0` rule for both Read and Write.

A focused regression covers zero, positive, and negative cycle results with both zero and nonzero CSW status. PowerPC compilation and independent re-review are still required. Other USB-storage findings remain open.


## 2026-10-05 fix handoff — StartStop CSW timeout parity

Implementer: OpenAI. Independent re-review: needed.

C `USBStorage_StartStop` uses the normal two-second USB-storage timeout for immediate commands and a ten-second CSW timeout for non-immediate commands. Rust previously used the synchronous bulk-read path with one timeout behavior for both. The production StartStop path now submits the CSW read asynchronously, waits through the Rust LWP timing path, and selects two or ten seconds from the `imm` flag. The CSW buffer is static and generation-guarded so a late completion cannot write into expired stack storage or be mistaken for a newer request.

C clears the endpoint halt after a timed or failed bulk request and discards the clear result. Rust performs the same cleanup but returns a halt-clear failure when cleanup fails. That narrower error behavior is an intentional safety deviation under `AGENTS.md`; the timeout selection itself now matches C. A focused regression verifies the two-second and ten-second selection. PowerPC compilation and independent re-review are still required.


## 2026-10-05 fix handoff — restore idle Read/Write remount decision

C tracks the last USB-storage use time globally. If more than 60 seconds have elapsed, `USBStorage_Read` and `USBStorage_Write` call `USBStorage_MountLUN` before issuing the transfer, then update the last-use time after the transfer. The DISC startup path deliberately initializes the timestamp to 100 seconds in the past so the first sector I/O performs this wake-up remount.

Rust now applies the same remount decision. The production path stores last-use time in whole seconds, primes it to `now - 100` when the DISC path mounts a LUN, attempts `mount_lun` when the elapsed time is greater than 60 seconds, and updates the timestamp after each Read/Write cycle. The remount result remains non-authoritative, matching C, which proceeds with the requested transfer after the wake-up attempt. Regression coverage checks the strict `> 60` threshold.

This handoff does **not** close the full idle-I/O finding. C also changes its USB transfer timeout to 10 seconds for the idle remount and following Read/Write operation, then restores the default timeout. Rust's ordinary BOT Read/Write path still lacks that temporary timeout selection. Independent compile/re-review is required.


## 2026-10-05 completion pass — handle resources, idle timeout, and safety classifications

Starting from `89dba72`, the completion pass removes the process-global four-handle backend table and shared transfer mutex. Each `UsbStorageHandle` now owns the live mutex and alarm represented by its public fields, plus a dynamically allocated sector-size table and 32-byte-aligned transfer buffer. Open-failure and close cleanup release those resources. The reported LUN count is allocated dynamically up to the representable `u8` count; a reported maximum LUN of 255 is rejected instead of wrapping a 256-LUN count, which is an accepted bounds-safety deviation.

The BOT transport now uses C's USB-mode predicate to select 4 KiB transfers for the USB1 path and 16 KiB transfers for the USB2 path. V5 bulk operations have a Rust-owned asynchronous submission path so the storage layer can apply its per-handle alarm to ordinary CBW, data, and CSW transfers. Read/Write use the default two-second timeout normally and the C-compatible ten-second timeout for both the idle remount and the following transfer after more than 60 seconds idle. BOT reset keeps its C-compatible one-second control timeout.

The open-time BOT reset error remains intentionally stricter than C. C can overwrite that reset failure with the later GetMaxLUN result; Rust preserves the reset failure and aborts open. This is an accepted safety/error-preservation deviation. The existing early aborts for failed GetConfiguration, SetConfiguration, and SetAlternativeInterface are also accepted safety deviations because continuing would use an unknown device/interface state. The close-time SYNCHRONIZE CACHE operation remains an accepted data-integrity safety addition, and StartStop still reports a failed halt-clear cleanup instead of discarding that cleanup failure.

The DISC startup boundary now matches C: full USB initialization occurs before storage initialization. Public storage initialize/deinitialize do not themselves start or stop the USB host subsystem. The fixed Inquiry, MountLUN result flow, removal callback state, Suspend result/state, shutdown result, Read/Write CSW status, StartStop timeout, and raw ReadCapacity semantics remain recorded above and have prior PowerPC compile validation as reported by the project owner. This completion pass itself requires PowerPC compilation and independent re-review before the crate can move from `needs_re_review` to `reviewed`.

## 2026-10-05 independent re-review — API results and failure paths

**Result: NEEDS_CHANGES. Reviewer: `worker_d`.** I compared the current implementations in `crates/services/ogc-usbstorage-sys/src/wii_backend.rs`, the safe wrappers in `crates/services/ogc-usbstorage/src/lib.rs`, and lifecycle policy in `crates/services/ogc-usbstorage-lifecycle/src/lib.rs` with `../../C-sources/libogc/libogc/usbstorage.c` and `../../C-sources/libogc/gc/ogc/usbstorage.h`. The reported USB1 reset predicate, per-handle resource ownership, transfer chunks, idle timeout, startup boundary, and bounded LUN behavior are consistent with the compared C code, subject to the differences below.

### Confirmed C/Rust mismatches

- **Inquiry has fewer total attempts in Rust.** C `USBStorage_Inquiry` retries the complete `__cycle` up to two times (`usbstorage.c:617-632`); each `__cycle` itself can make four BOT attempts (`usbstorage.c:286-340`). Rust `inquiry_inner_with_timeout` invokes `cycle_with_timeout` only once (`wii_backend.rs:1782-1805`), whose loop makes four attempts (`wii_backend.rs:1634-1703`). If all four first-cycle attempts fail and a later retry would succeed, C makes another four attempts and can return the peripheral type while Rust returns the first cycle's error. This affects the exported Rust `inquiry` API and is a confirmed result mismatch.
- **Read Capacity rejects output pointers that C permits to be null.** C copies `n_sectors` and `sector_size` independently only when each pointer is non-null (`usbstorage.c:652-668`). Rust `read_capacity` rejects either null output pointer before sending the command (`wii_backend.rs:2310-2331`), although `read_capacity_inner_with_timeout` already supports either output as null (`wii_backend.rs:1807-1850`). A caller that requests only one value succeeds in C and receives `USBSTORAGE_ENOINTERFACE` in Rust. The high-level `ogc-usbstorage` crate re-exports this function.

### Failure paths that conflict with the Real Implementation Policy

- `close_policy::close` discards every flush and transport-close result (`crates/services/ogc-usbstorage-lifecycle/src/lib.rs:12-25`). The backend then frees/resets the handle and returns `USBSTORAGE_OK` (`wii_backend.rs:2058-2103`). If `usb::close_device` fails, cleanup loses the live descriptor when it resets the handle; callers still receive success. If SYNCHRONIZE CACHE fails, callers also receive success even though cached writes may not be durable. C has similar behavior (`usbstorage.c:552-574`), but the Rust project policy requires visible operation failures and does not permit success after a required operation fails. The extra close-time flush is an intentional data-integrity measure, but suppressing its error weakens that measure.
- `suspend_with` discards `suspend_device`'s result, sets `suspended = 1`, and returns `USBSTORAGE_OK` (`wii_backend.rs:2220-2230`). C also ignores `USB_SuspendDevice` (`usbstorage.c:797-805`), but the current Rust policy requires the transport result to be checked and returned. A failed suspend must not leave the handle marked suspended.
- `run_bot_reset_sequence` ignores both endpoint `clear_halt` results and returns only the class-reset result (`wii_backend.rs:1075-1082`). C ignores these results (`usbstorage.c:386-394`), but the Rust policy requires failures from required recovery steps to remain visible. A successful class reset with a failed halt clear currently returns success.
- `shutdown_with` ignores `close` failure, clears mount identity, and returns `true` (`wii_backend.rs:2723-2742`). C's DISC callback also returns `true` after attempting close (`usbstorage.c:950-968`), but this Rust success result hides a failed required close. Keep the safety deviation from C only if the public callback can expose the cleanup failure through another supported channel; none was identified in this comparison.
- `bulk_message_with_timeout` submits an asynchronous USB request before arming its alarm (`wii_backend.rs:1383-1430`). If `sys_set_alarm` fails, Rust returns immediately at lines 1428-1429 while the submitted USB request can still complete and use the handle or caller buffer. This leaves operation state live after the API reports failure. C's `__USB_BlkMsgTimeout` waits after submission and its timeout setup has no result return (`usbstorage.c:120-141`); Rust must keep the request and its buffers valid until completion or a supported cancellation finishes.

### Accepted safety deviations

Rust rejects a reported maximum LUN byte of 255 because `max_lun` is a `u8` count and 256 cannot be represented. Rust also aborts open and preserves the first real reset/configuration/interface error instead of continuing with uncertain device state or allowing a later result to overwrite it. The checked buffer/range validation remains a valid safety difference. The close-time cache flush is an intentional data-integrity addition, but its failure must be surfaced as described above.

### Uncovered scope and validation limits

The low-level USB transport, LWP mutex/alarm guarantees, device-removal races, and physical-device behavior remain outside this static review. I did not run builds or tests. Earlier PowerPC checks and Dolphin probes in this note are reported implementation evidence; they do not verify physical USB control/BOT behavior. No hardware verification was performed.

## Independent re-review — Inquiry retries and nullable ReadCapacity outputs (2026-10-05)

**Result: PASS for commit `3e4ac90`.** Reviewer: `worker_f`. Implementer: `worker_e`.

C `USBStorage_Inquiry` clears its 36-byte response and calls `__cycle` up to two times, stopping on a nonnegative result and returning the response peripheral type (`../../C-sources/libogc/libogc/usbstorage.c:617-638`). Rust `inquiry_inner_with_timeout` now calls `inquiry_with_cycles`; that helper clears the response before each of two calls, returns the peripheral type on the first nonnegative complete BOT cycle, and otherwise returns the second error (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:1830-1867`). Each closure call enters `cycle_with_timeout`, whose production retry loop remains the complete inner BOT-cycle operation (`:1664-1707`). The regression tests cover retry success on the second cycle and early success on the first (`:489-518`).

C `USBStorage_ReadCapacity` copies each output independently only when its pointer is non-null (`usbstorage.c:652-668`). Rust no longer rejects null output pointers at the public entry (`wii_backend.rs:2385-2407`); after a successful decode, `write_capacity_outputs` writes each non-null pointer independently (`:1888-1932`). Its regression checks sector-size-only, sector-count-only, and both-null cases (`:521-543`). Device, LUN, and backing-handle validation remain in place before the operation.

`cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu` passed and type-checked target test code; tests did not run. This pass covers only the Inquiry retry count/result and nullable ReadCapacity outputs. The broader USB storage findings and low-level transport behavior remain open. No hardware verification was performed.


## 2026-10-06 fix handoff — C ABI ownership and async lifetime

Implementer: OpenAI. Independent re-review: required.

The Rust sys crate now owns the libogc USB-storage ABI surface rather than exposing only Rust-callable equivalents. Target builds export `USBStorage_Initialize`, `USBStorage_Deinitialize` with C's `void` result, `USBStorage_Open`, `USBStorage_Close`, `USBStorage_Reset`, `USBStorage_GetMaxLUN`, `USBStorage_MountLUN`, `USBStorage_Suspend`, `USBStorage_IsDVD`, `USBStorage_ReadCapacity`, `USBStorage_Inquiry`, `USBStorage_Read`, `USBStorage_Write`, and `USBStorage_StartStop`. It also exports `__io_usbstorage` and `__io_usbstorage_sector_size` under the C symbol names. The existing Rust APIs remain available, including the Rust status-returning `deinitialize` convenience API.

`USBStorage_ioctl(int request, ...)` is exported through a PowerPC assembly entry point. For the supported `B_RAW_DEVICE_COMMAND` request, the first variadic argument is a `raw_device_command *`; PowerPC EABI places that pointer in the same second argument register used by the fixed Rust bridge. The assembly entry therefore tail-branches to the Rust-owned request implementation without depending on unstable Rust variadic definitions.

The asynchronous bulk path also closes the recorded lifetime hole. The USB request is submitted before the alarm is armed. If `SYS_SetAlarm` fails, Rust no longer returns while the USB callback can still access the handle or caller buffer. It yields until that already-submitted request completes, keeping the borrowed storage live, then returns the alarm-setup error. This is a narrow safety behavior for a failure mode that C's alarm API does not expose as a returnable setup failure.

The bounded ABI and control-request completion findings were addressed, but the full current transport source still needs independent review. In particular, async bulk transfers could leave a callback using the handle after timeout, and the prior deinitialization helper test did not match the production close-failure path. Keep `ogc-usbstorage` and `ogc-usbstorage-sys` at `needs_re_review`; callback timing and physical USB behavior also remain open.

## 2026-10-07 deinitialization cleanup parity follow-up

A fresh comparison against `USBStorage_Deinitialize` and `__usbstorage_Shutdown` found one remaining current-source cleanup mismatch. C ignores the `USBStorage_Close` result, continues queue/subsystem teardown, and clears the initialized state. Rust previously returned early from `deinitialize()` when `close()` failed, leaving subsystem state initialized. The backend now ignores the close result during deinitialization and always clears its initialized, mounted, capacity, and DISC state, while the Rust convenience function returns `USBSTORAGE_OK`. The exported C ABI function remains `void`, matching the header.

A focused regression injects a close failure and verifies that teardown still completes. USB callback timing, PowerPC varargs behavior, and physical-device transport remain runtime-validation scope.

## Independent re-review — deinitialization close failure (2026-10-07)

**Result: NEEDS_CHANGES under the repository implementation policy.** Reviewer: `/root` in a new review thread. The current behavior matches C's `USBStorage_Deinitialize`/`__usbstorage_Shutdown` result flow (`../../C-sources/libogc/libogc/usbstorage.c:955–966`), but the Rust convenience API returns an `i32` and converts a failed required device close into `USBSTORAGE_OK` (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:1008–1028`). This discards the close failure and reports success after cleanup failed, which conflicts with the repository Real Implementation Policy. The public C ABI may remain `void`, but the Rust API must preserve the close error or expose it through a supported error channel. PowerPC target test code compiled with `RUSTFLAGS='-Aunsafe-op-in-unsafe-fn'`; tests were not run. The crate remains blocked on this policy finding.

## 2026-10-07 implementation handoff — preserve close errors

Implementer: `/root`. The shared close sequencing helper now returns the first negative result from mounted-LUN flushes or transport close. It still attempts every required flush and always attempts transport close, so an earlier failure does not skip cleanup. `wii_backend::close` returns that result after releasing its transfer guard. It frees handle resources only after the USB layer invalidates the file descriptor; on close failure it keeps the handle resources and descriptor so a later deinitialize can retry. The Rust `deinitialize` API completes subsystem-state teardown and returns the close error; the C ABI remains `void` as required by the C declaration. The disc shutdown callback also clears its state and reports `false` when close fails, following the repository policy that a failed required operation cannot report success.

Regression coverage checks first-error preservation, continued close after flush errors, returned transport-close errors, and state teardown when deinitialization close fails. `cargo test -p ogc-usbstorage-lifecycle` passed all 7 tests. `RUSTFLAGS='-Aunsafe-op-in-unsafe-fn' cargo check -p ogc-usbstorage-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled the target tests; the Wii-only test binary could not run on the host. The policy finding is fixed, and independent review of the full USB storage diff remains required. No hardware verification was performed.

## Independent close and failure-path review (2026-10-15)

Reviewer: `/root/release_parity_review`. Result: **PASS for close-error propagation; NEEDS_CHANGES for the remaining failure paths**. The shared `ogc-usbstorage-lifecycle::close` returns the first failed flush or transport close and still attempts later cleanup. The backend returns that result and retains resources if the descriptor remains live (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:2255-2302`). `deinitialize_with` completes state teardown and returns a close error (`:1011-1025`); `shutdown_with` returns false after a failed close (`:2950-2958`). These explicit error results are valid safety differences from C's unconditional cleanup success (`../../C-sources/libogc/libogc/usbstorage.c:552-574,955-966`).

Two confirmed implementation-policy failures remain. `suspend_with` discards `suspend_device` failure and marks the handle suspended (`wii_backend.rs:2419-2428`). `run_bot_reset_sequence` discards both required endpoint halt-clear results and can return success when recovery failed (`:1187-1195`). C has the same unsafe result handling, but the Real Implementation Policy requires real operation failures to be returned. The old handoff's dismissal of these findings does not satisfy that policy. The tests `suspend_ignores_usb_error_and_marks_handle_suspended_like_c` (`:362`) and `bot_reset_returns_reset_result_when_endpoint_clear_fails` (`:478`) require that invalid success behavior. Fix the production path and replace those expectations with visible error and correct-state checks. Reset must still perform required cleanup after an earlier error and preserve the first error.

`cargo test -p ogc-usbstorage-lifecycle` passed all 7 tests. PowerPC library and target test compilation passed in the joint audio/storage check recorded above. Target tests did not run. No physical USB, timeout, callback, or device-removal behavior was tested. The sys crate remains `parity_blocked`; this pass does not approve all transport behavior.

## BUG FIX — suspend and reset failure results (2026-10-15)

Implementer: `release-quality/root`. Original auditor and required independent reviewer: `/root/release_parity_review`.

The backend now uses `ogc-usbstorage-lifecycle::transport` for suspend and BOT reset. Suspend returns the USB error and leaves the handle state unchanged after failure. A later call can retry. Reset performs the class request, the 60 ms delay, both endpoint halt clears, and their 10 ms delays. It returns the first failed operation after all recovery steps finish. The BOT transfer retry path now returns a failed reset result instead of continuing transfers with uncertain endpoint state.

This is a deliberate safety difference from C. C ignores suspend and endpoint-clear failures. Rust preserves those failures so the caller can detect an incomplete state change or failed recovery. The real USB requests and recovery order remain in the production backend. No transport model replaces them.

The shared production engine passed all 11 host tests. Failure tests cover every reset step, multiple reset failures, failed suspend state, successful retry, and repeated suspend. PowerPC target test compilation passed with warnings denied. Target tests did not run. The full Wii runtime compile check passed. Physical USB state, timing, and device removal remain unverified. Independent re-review is required; this fix does not approve the complete transport.

## Independent bounded transport-failure review (2026-10-15)

Reviewer: `/root/release_parity_review`. Implementer: `release-quality/root`. Fix: `15dfdda`; implementation claim: `f43c923`; independent review claim: `a1b6f09`. Result: **PASS for this bounded fix**.

The production backend now calls the shared transport engine directly. Reset performs the same class request, 60 ms delay, IN halt clear, 10 ms delay, OUT halt clear, and final 10 ms delay as C (`../../C-sources/libogc/libogc/usbstorage.c:386-395`; `crates/services/ogc-usbstorage-lifecycle/src/transport.rs:12-28`; backend `wii_backend.rs:1435-1449`). It checks each result and retains the first failure while completing recovery. All three BOT failure branches check the returned reset result through `bot_cycle_retry_error` and stop retrying after recovery fails (`wii_backend.rs:1178-1187,1778-1782,1797-1801,1814-1818`).

Suspend calls the real USB operation before committing `suspended = 1`; failure returns unchanged state and permits another request (`transport.rs:31-41`; backend `wii_backend.rs:2402-2406`). Already-suspended handles keep C's successful no-op behavior. These differences from C's ignored halt-clear and suspend errors are accepted safety changes. They expose failed operations without replacing USB communication or changing the recovery request order. The prior suspend/reset policy findings are closed. The previous close-error acceptance remains valid.

I inspected the complete fix diff, including regressions and file modes. `cargo test -p ogc-usbstorage-lifecycle` passed all 11 tests. The reset tests inject failure at each shared production operation boundary and verify final errors plus the complete sequence. The suspend test verifies state, retry, and suppression of a duplicate request only after success. `RUSTFLAGS='--check-cfg=cfg(gamecube) -D warnings' cargo check -p ogc-usbstorage-sys --target powerpc-unknown-linux-gnu --tests` passed. Target tests did not run. Scoped commit whitespace validation passed. No hardware was used.

The dirty `xtask/src/host_tests.rs` change was also inspected without modification. Its two selection tests passed. The new omission test checks each direct `tests/*/Cargo.toml` manifest against selected manifests, so removing an existing suite from the list causes failure. This check does not claim discovery of arbitrary nested directories or hardware test suites.

`ogc-usbstorage-lifecycle` is reviewed for its close and shared transport scope. `ogc-usbstorage-sys` is `review_incomplete`: no confirmed blocker remains in this bounded fix, but this pass does not complete the full transport and C ABI source comparison called for by the older implementation handoff. Physical transfer timing, PowerPC variadic calls, device removal, and USB failure behavior remain unverified on hardware.

## Runnable physical transport coverage (2026-10-15)

`usbstorage_transport_probe.dol` now opens a physical device through production
APIs. It reads capacity and sector zero, performs three BOT reset and
suspend/automatic-resume cycles, then closes and reopens the device. Each
verification read starts with bytes that differ from the saved physical data,
so an unchanged buffer cannot pass. Reset duration must include the required
60/10/10 ms delays. Capacity, full sector bytes, suspended state, close results,
and released resources are checked. No raw write or filesystem mount is
requested. Production close still performs its required cache flush.

A missing device or failed setup is a failed prerequisite with dependent cases
UNTESTED. A logger-owned USB mount is rejected before device reset or suspend.
The handle has permanent storage so failed cleanup does not destroy its state.
No artificial transport failure or replacement medium is used. The probe checks
returned results and subsequent physical I/O; it does not measure the USB
port's electrical suspend state. Deterministic failure injection remains host
coverage. New physical results remain `not-tested` in the hardware register.

## Full BOT source re-review — 2026-10-15

Reviewer: `/root/review_usb_final`. Result: **NEEDS CHANGES**. ABI declarations,
calling conventions, handle layout, `DISC_INTERFACE`, and variadic ioctl
exports matched the C header. Three behavior differences remain:

- Rust stores reset, GetMaxLUN, and StartStop completion state globally. C
  stores the result, alarm, and lock in each `usbstorage_handle`
  (`usbstorage.h:44-46; usbstorage.c:128-142`). Concurrent handles can consume
  another request's result, time out, or leave shared state stuck after timeout.
  Move completion state to each handle or a per-request owner.
- On a short positive read, C copies the returned prefix from its bounce buffer
  to the caller (`usbstorage.c:311-324`). Rust copies only after the whole chunk
  succeeds (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:1683-1718`).
  Preserve the same caller-buffer side effect.
- C retries a failed BOT cycle unless the cycle or reset result is
  `USBSTORAGE_ETIMEDOUT` and preserves the cycle error (`usbstorage.c:286-344`).
  Rust returns any negative reset result. Keep this only as a documented
  recovery-safety deviation; current text claiming parity is stale.

The re-review was read-only. No tests or hardware checks were run. Target
callback races and USB timing remain unverified.

## BUG FIX handoff — per-handle completion and BOT retry behavior

Implementer: `/root`. Independent reviewer: `/root/review_usb_final`.

The three findings above are fixed in the production path:

- **BUG FIX — Per-request completion state.** Reset, GetMaxLUN, and StartStop
  now own their result and DMA buffer in an aligned allocation for that request.
  Concurrent handles do not share result state or buffers. On timeout, the
  allocation stays alive until the late callback schedules cleanup on the LWP
  deferred worker. If that enqueue fails, Rust retains the allocation and
  increments `async_control_cleanup_failures`; the public C-layout handle stays
  unchanged.
- **BUG FIX — Preserve short-read bytes.** The read path copies every positive
  byte count from the bounce buffer before it returns `USBSTORAGE_EDATARESIDUE`
  for a short chunk. `copy_read_result` is shared by the production transfer
  path and the USB storage capacity DOL probe.
- **BUG FIX — Match BOT retry result handling.** The retry rule stops when the
  cycle or reset times out. Other reset errors do not replace the failed cycle
  result and do not stop the next retry. `bot_cycle_retry_error` is shared by
  the production cycle and the DOL probe.

The PowerPC test target compiles with
`cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu`.
Host `cargo test -p ogc-usbstorage-sys` cannot run because this crate and its
dependencies reject non-PowerPC targets. The updated DOL builds, but the latest
Dolphin run timed out before the first frame (`frames=0`); it did not validate
the new checks. No real Wii hardware run was made. The independent source
re-review must inspect the full diff and the timeout path before the ledger can
mark this crate reviewed.

## Independent re-review — per-request completion and BOT retry fix

Reviewer: `/root/review_usb_final`. Implementation commit: `df0d75f`.
Result: **NEEDS CHANGES**.

The three recorded findings are fixed for the reviewed cases. Reset,
GetMaxLUN, and StartStop each use an aligned per-request allocation for
completion state and DMA data (`wii_backend.rs:1099-1119, 1154-1178,
1208-1238`). A timeout keeps the allocation alive for a late callback. The
callback queues cleanup on the LWP deferred worker; if enqueue fails, the
allocation stays live and `async_control_cleanup_failures` increments
(`wii_backend.rs:109-155`). Positive short reads copy the received prefix
before returning `USBSTORAGE_EDATARESIDUE` (`:1500-1505`). The retry rule stops
on a transfer or reset timeout and otherwise preserves the original cycle
error while retrying, matching C (`:57-63, 1580-1630`; C
`../../C-sources/libogc/libogc/usbstorage.c:286-344`).

One confirmed edge case remains. For a nonzero requested chunk, a zero-byte
read returns `USBSTORAGE_OK` from `copy_read_result`, then transfer processing
continues (`wii_backend.rs:67-82, 1500-1505`). A zero-byte write also returns
zero because the write branch only maps positive short results to
`USBSTORAGE_EDATARESIDUE` (`:1474-1488`). C maps every non-full data transfer
to `USBSTORAGE_EDATARESIDUE` unless the result is `USBSTORAGE_ETIMEDOUT`
(`usbstorage.c:313-330`). Thus either Rust path can continue to CSW after a
zero-byte data transfer and may report success. Keep
`ogc-usbstorage-sys` parity-blocked until both directions return the C error
and have regression coverage.

`cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu`
passed and compiled the target tests; I did not run them. No Dolphin execution
or Wii hardware verification is claimed. The reported Dolphin run timed out
before frame 0, so it did not validate the DOL. The asynchronous timeout path
retains storage safely if no completion callback arrives, but that case was
not exercised; a request that never calls back can leave the allocation
retained. No source or test files were changed in this re-review.

## Zero-byte transfer fix handoff — 2026-10-15

**BUG FIX:** Read and write data chunks now pass their byte count through the
same transfer-result mapping. A zero-byte completion for a nonzero chunk
returns `USBSTORAGE_EDATARESIDUE`; negative errors, including timeouts, remain
visible to the caller. Positive short reads still copy their received prefix
before returning the residue error (`crates/services/ogc-usbstorage-sys/src/lib.rs::transfer_result`
and `copy_read_result`; `wii_backend.rs::transfer_data`).

Keeping a non-timeout negative transport error differs from C, which maps it
to `USBSTORAGE_EDATARESIDUE` (`../../C-sources/libogc/libogc/usbstorage.c`,
data-stage result mapping). Rust keeps the original result because the
repository error-propagation policy requires useful operation errors to reach
the caller. The target test and DOL probe assert this policy difference.

Regression checks cover zero-byte reads and writes in the target unit tests and
DOL probe. PowerPC test compilation and the DOL build passed. Tests did not run;
no Dolphin or Wii hardware result is claimed. The fix awaits independent
re-review by `/root/review_usb_final`. The crate remains `parity_blocked` until
the reviewer confirms this finding is resolved and other recorded findings
are cleared.

## Independent re-review — zero-byte transfer result fix

Reviewer: `/root/review_usb_final`. Implementation commit: `6e755df`.
Result: **NEEDS CHANGES for regression coverage and result documentation**.

The zero-byte behavior is fixed in both production directions. A write chunk
uses `transfer_result`, and a read chunk uses `copy_read_result`, which calls
the same helper (`wii_backend.rs:1474-1505`). For a nonzero expected chunk, a
received count of zero maps to `USBSTORAGE_EDATARESIDUE`. Negative results,
including `USBSTORAGE_ETIMEDOUT`, remain negative. The unit tests and DOL
probe cover zero-byte reads and writes and timeout preservation
(`lib.rs:781-801`; `probes/ogc-system-probe/src/usbstorage_capacity_probe.rs:34-45`).
This matches C for zero-byte results: C maps any non-full data transfer to
`USBSTORAGE_EDATARESIDUE` unless it is a timeout
(`../../C-sources/libogc/libogc/usbstorage.c:313-330`).

One requested behavior is not directly covered by a regression: neither the
target test nor DOL probe asserts that a non-timeout negative transport result
is returned unchanged. Source inspection shows that `transfer_result` does
preserve it. This differs from C, which maps non-timeout negative transfer
results to `USBSTORAGE_EDATARESIDUE`. Keeping the original transport error is
required by the repository error-propagation policy, so this is an intentional
safety/policy difference; update the helper documentation to say so and add a
fixed regression for negative-error preservation before closing this
handoff.

`cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu`
passed and compiled the target tests; they did not run. `cargo xtask
review-status` and `git diff --check` passed. The DOL has not been validated in
Dolphin: the latest run timed out before frame 0. No Wii hardware result is
claimed. This bounded re-review does not clear earlier USB-storage findings
or complete physical callback/timing validation.

The USB storage implementation evidence line references were refreshed on
2026-10-15, and `cargo xtask inventory` regenerated the inventory and status
files. The source audit passes with tracked keyboard queue audit debt. The
stub audit reaches completeness checking but fails on 2,354 API entries that
lack accepted implementation evidence.

## Independent re-review — transfer-result policy follow-up

Reviewer: `/root/review_usb_final`. Implementation commits:
`70addd0`, `a6c7bd8`.
Result: **PASS for the bounded transfer-result finding**.

The target regression now checks that a non-timeout negative result such as
`-41` remains `-41`; the DOL probe checks the same production helper
(`crates/services/ogc-usbstorage-sys/src/lib.rs:794-799`;
`probes/ogc-system-probe/src/usbstorage_capacity_probe.rs:43-47`). The review
note records the difference from C: C maps non-timeout negative data-stage
results to `USBSTORAGE_EDATARESIDUE`, while Rust preserves the transport error
as required by the repository error-propagation policy. Zero-byte read/write
results map to `USBSTORAGE_EDATARESIDUE`, and timeout results remain
`USBSTORAGE_ETIMEDOUT`.

`cargo check -p ogc-usbstorage-sys --tests --target powerpc-unknown-linux-gnu`
passed and compiled the target tests; they did not run. `cargo xtask
review-status` and `git diff --check` passed. The DOL was not validated in
Dolphin; the latest run timed out before frame 0. No Wii hardware result is
claimed. This result closes the zero-byte and negative-error result handoff
only. The crate remains parity-blocked on the separate open USB findings
recorded above, including asynchronous timeout cleanup lifetime and physical
callback/timing validation.

## Bulk callback lifetime and deinitialization follow-up — 2026-10-28

An independent source review of current HEAD `ac10c1c` found that the bulk
request timeout callback could return while the USB completion callback still
held the handle pointer. It also found that production deinitialization returned
on close failure before clearing subsystem state. The earlier helper test did
not cover the production close loop. See the pre-fix review result from
`/root/bt_event_queue_rereview` for the exact source trace.

The Rust bulk path now records pending request and alarm-cancellation states in
the shared `ogc-usbstorage-lifecycle::bulk` state machine. A timed-out operation
returns `USBSTORAGE_ETIMEDOUT`, but the handle stays unavailable until the late
request callback drains. A failed alarm cancellation returns `USBSTORAGE_EINIT`
and keeps the handle unavailable until the alarm callback drains. `close`,
reset, suspend, future transfers, and static-drive reopen check this state. The
callback does not access request-owned transfer buffers. It still needs target
timing validation. If a submitted request never calls back after alarm setup
fails, the current path waits for that callback to protect the handle and can
remain blocked.

Production deinitialization now tries every open static drive, preserves the
first close error, and keeps subsystem state when any close fails. It clears the
state only after all closes succeed. The C ABI deinitialization function is
`void`. C callers can read the latest result through the additive
`USBStorage_GetLastDeinitializeStatus` export. The result is zero before the
first C deinitialization call, then records that call's actual return code.
The host lifecycle test checks the atomic status handoff; the C export and its
linkage still require target runtime validation. The host lifecycle tests cover
the shared bulk state transitions. PowerPC checks compile the target regression
and production path, but do not run them on hardware.

Verification on 2026-10-28: `cargo test -p ogc-usbstorage-lifecycle` passed
21 tests; `cargo check -p ogc-usbstorage-sys --tests --target
powerpc-unknown-linux-gnu` and the matching `ogc-usbstorage` target check
passed. `cargo xtask audit-source` passed with existing keyboard queue audit
debt. Independent post-fix source review: **PASS for the scoped bulk lifetime and
deinitialization changes**, reviewer `/root/bt_event_queue_rereview`. The
reviewer confirmed the callback gates, retryable deinitialization state, and
status accessor source path. The new C export still needs target linkage and
runtime evidence. Physical USB validation is pending, and broader USB storage
findings remain open. Keep both crate-level ledger entries at
`needs_re_review`.
