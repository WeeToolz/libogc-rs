# Wireless driver wrapper review

Status: the confirmed scan-size limit finding was fixed by `/root/worker_a` and accepted by independent reviewer `worker_b`. PowerPC target checks pass for the production crates; the WD test build is blocked by two existing test compile errors (a stale `deinit` function signature cast and a missing unsafe block). Tests were not run. No hardware verification was done.

## Scope and source map

Reviewed all production source in `crates/services/ogc-wd/src/lib.rs` (lines 1–683). Target-gated test code at lines 684–858 was read for claimed coverage but not executed. Public crate metadata is in `crates/services/ogc-wd/Cargo.toml`.

The matching checked-in C API is `../../C-sources/libogc/gc/ogc/wd.h` (lines 35–168) and its Wii-gated implementation is `../../C-sources/libogc/libogc/wd.c` (lines 28–245). Rust timing and platform detection also depend on `ogc-system-sys::{sys_time, sys_get_hollywood_revision}` (`crates/core/ogc-system-sys/src/lib.rs:3810–3816` and its existing revision helper), IPC request handling in `ogc-ipc`, and Wii timer constants from the C clock headers. The header's `<gccore.h>` transitively supplies platform structure and clock definitions; it does not contain the `usleep` implementation. `wd.c` only declares `extern void usleep(u32)` at line 40.

## Confirmed behavior mismatches

### `WD_GetNumberOfIEs` counted a zero-length terminator

C checks the current IE header's length before counting it and stops when the length is zero (`libogc/wd.c:187-202`). Rust carried the previous IE's length into the next loop condition, so it counted a zero-length entry after a nonzero IE. For a list containing IE lengths 2, 1, then 0, C returns 2 and Rust returned 3.

Implementer: `worker_a`. Fix status: fixed; needs independent review.

Rust now checks each current IE length and stops before counting a zero-length entry. The existing regression uses the 2, 1, 0 list and now expects 2, matching C. This only aligns the terminator behavior; Rust's backing-length and truncation checks remain intentional safety behavior.

Validation: `cargo check -p ogc-wd --target powerpc-unknown-linux-gnu` passed. `cargo check -p ogc-wd --target powerpc-unknown-linux-gnu --tests` is blocked by pre-existing errors in the WD test module: a stale `deinit` function-pointer cast and a missing unsafe block. Tests were not run. `git diff --check` passed. No hardware verification was performed.



## Fix handoff: scan buffer size limit

Implementer: `/root/worker_a`. Independent reviewer: `worker_b` (accepted).

C `WD_Scan` uses an aligned stack buffer sized from the full `u16 buffsize` range (`wd.c:151-169`). Rust's scan buffer uses the same `buffsize + 2` data size, but the fixed `ogc-ipc` IPC heap was only 8192 bytes and rejected larger heap requests before the scan ioctl. The IPC heap capacity is now 65,600 bytes. That supports the largest Rust allocation: `u16::MAX + 2` bytes, rounded up to 32-byte alignment, plus the 32-byte allocator header. `ogc-wd` uses a named helper for its `buffsize + 64` heap request, and a regression test verifies that the maximum `u16` request fits the IPC heap capacity. The IPC allocator also has a test for the largest aligned allocation plus header.

This increases the static IPC heap storage by 57,408 bytes for all builds using `ogc-ipc`. It keeps scan buffers aligned and in the IPC heap used for IOS request vectors. Rust now permits the full `u16` scan size at the allocator boundary; whether C succeeds for the largest values still depends on caller stack capacity and IOS behavior.

Verification: `cargo check -p ogc-wd -p ogc-ipc --target powerpc-unknown-linux-gnu` passed. The IPC tests compile, but `cargo check -p ogc-wd -p ogc-ipc --target powerpc-unknown-linux-gnu --tests` is blocked by existing errors in WD tests: `deinit` is cast to the old `fn()` signature although it returns `i32`, and `set_default_scan_parameters` is called without an unsafe block. Tests were not executed. No hardware verification was performed.

## Differences and Rust-only behavior

- **Target check:** Rust rejects wireless-driver calls in GameCube builds with a compile-time target check. Wii builds, including Dolphin's Wii IOS, call the real `/dev/net/ncd/manage` and `/dev/net/wd/command` services. C includes this implementation only when `HW_RVL` is defined (`wd.c:28–29`), so the build-target check preserves that platform split while allowing Wii IOS emulators to handle the requests.
- **NCD lock errors:** C lock opens `/dev/net/ncd/manage`, issues ioctlv command 1, copies the lock ID, and closes without checking open/ioctl/close errors (`wd.c:46–64`). Rust checks open and ioctl/close results and returns a negative error (`ogc-wd/src/lib.rs:156–162, 205–220`). On lock errors C may read the cleared/stale common buffer and return an ID; Rust returns an error.
- **NCD unlock errors:** C checks open only, ignores ioctl and close results, and returns the output word (`wd.c:66–88`). Rust checks the operation/close status and maps failure to `u32::MAX` (`ogc-wd/src/lib.rs:223–244`). Successful vector counts and payloads match: one 32-byte input common buffer and one four-byte output result (`wd.c:70–82`; Rust lines 230–239).
- **Init/deinit:** C `WD_Init` opens once with mode `0x10000 | mode`, returns 0 if already open, otherwise collapses open failure to `-1`; `WD_Deinit` is void and ignores close errors (`wd.c:103–116`). Rust uses the same path, mode, and already-initialized behavior (`ogc-wd/src/lib.rs:247–266`), but preserves an IOS open error and returns a status from `deinit`, including close failure (`lib.rs:268–292`). Rust atomics and a `WD_CLOSING` state (`lib.rs:131–143, 247–292`) add protection around a shared fd. Concurrent init/deinit sequences were not runtime-tested.
- **Get info:** C locks NCD, opens WD in AOSS scan mode, issues GET_INFO, copies the result, then deinitializes/unlocks. It ignores the ioctl result and returns success after the sequence; init failure returns early without unlocking (`wd.c:131–149`). Rust rejects null output, checks every operation, uses a zeroed aligned temporary, always attempts deinit/unlock after a successful init, and copies to caller memory only if all operations succeed (`ogc-wd/src/lib.rs:294–341`). Cleanup and error propagation are intentional safety changes.
- **Scan:** Both require the WD fd to be initialized, use a zeroed 0x4e-byte aligned settings block with the first 0x1a bytes copied from `ScanParameters`, zero a `buffsize + 2` output block, issue `IOCTLV_WD_SCAN` with one input and one output vector, wait 100 ms, then copy `buffsize` bytes (`wd.c:151–171`; Rust `ogc-wd/src/lib.rs:347–398`). C ignores ioctl failure and returns 0; Rust propagates ioctl and allocation errors and only copies after successful ioctl. Rust uses aligned IPC heap staging instead of a variable length stack array.
- **Scan delay:** C calls `usleep(100000)` (`wd.c:168`). Rust busy-waits until 100 × 60,750 timebase ticks have elapsed (`ogc-wd/src/lib.rs:195–203`). The C clock headers define the Wii timebase as 60.75 MHz: `../../C-sources/libogc/gc/tuxedo/ppc/clock.h:5–13` and `../../C-sources/libogc/gc/tuxedo/ppc/clock.h:80–93`; thus the requested Rust delay is 6,075,000 ticks, nominally 100 ms. Exact scheduling and sleep/wakeup differences cannot be compared because the C `usleep` implementation is outside the checked-in source subset.
- **Scan once:** C locks, initializes, calls scan, deinitializes, and unlocks, but returns early on init failure without unlocking and ignores scan/deinit/unlock errors (`wd.c:174–185`). Rust preserves the successful order and returns the first operation/cleanup error while attempting cleanup (`ogc-wd/src/lib.rs:405–430`).
- **RSSI:** Both use the same unsigned RSSI thresholds: `>= 0xc4` strong, `>= 0xb5` normal, `>= 0xab` fair, otherwise weak (`wd.c:118–129`; `ogc-wd/src/lib.rs:491–503`). Rust returns weak for a null pointer, while C dereferences it.
- **Information elements:** Rust validates the descriptor backing length, IE-list length, complete headers, and complete IE payloads before reading, and checks destination capacity before copying (`ogc-wd/src/lib.rs:551–682`). C helpers traverse an unbounded caller pointer and trust `IEs_length`; `WD_GetIE` only checks for a null output pointer and does not ensure the output is large enough (`wd.c:187–243`). Rust now stops before counting a zero-length IE terminator, as C does. Other Rust backing-length checks remain Rust-only hardening and can reject truncated/malformed buffers that C would read past.
- **Typed Rust helpers:** Rust adds `Result` wrappers, safe slice APIs, a default-value constructor for scan parameters, scan buffer length clamping to `u16::MAX`, and public names for mode/opcode constants (`ogc-wd/src/lib.rs:10–40, 472–549, 625–682`). The C API has only raw pointer/status functions (`wd.h:153–168`). Rust's `deinit` returns `i32` where C is `void`.

## Layout and API coverage

- `WdInfo` matches the C `WDInfo` field sequence and widths (`wd.h:93–103`; `ogc-wd/src/lib.rs:42–68`). Its expected size is 144 bytes on the 32-bit PPC ABI; there is no explicit target assertion in this crate.
- `ScanParameters` matches C order and widths, including the six-byte BSSID between two `u16`s (`wd.h:107–118`; Rust `lib.rs:70–94`). Expected size is 52 bytes.
- `BssDescriptor` maps C's nested two-word rate set to adjacent `rate_set_basic`/`rate_set_support` fields, preserving byte order and offsets (`wd.h:122–141`; Rust `lib.rs:96–113`). Expected size is 62 bytes. `IeHdr` maps C's two one-byte fields (`wd.h:145–149`; Rust `lib.rs:115–120`). The C source provides no static layout assertions, and the Rust tests do not assert these sizes.
- Opcode constants match `wd.h:38–59`; signal levels match `wd.h:71–76`; the public mode values match `wd.h:80–89` (`ogc-wd/src/lib.rs:13–40`). Rust exposes only the used mode values, not `NA0`/`NA1`/`NA2`/`NA3`.
- Public C calls map to `ncd_lock_wireless_driver`, `ncd_unlock_wireless_driver`, `init`, `deinit`, `get_info`, `get_radio_level`, `scan`, `scan_once`, `get_number_of_ies`, `get_ie_length`, `get_ie`, and `set_default_scan_parameters` (`wd.h:153–168`; Rust implementations at `ogc-wd/src/lib.rs:205–430, 491–523, 594–670`). Rust additionally offers slice/result wrappers.

## Uncertain or unchecked

- No Wii hardware was available for this review. NCD lock protocol results, IOS response behavior, WD ioctl buffer contents, scan results, and timing are source-only conclusions.
- The actual C `usleep` implementation was not found under the checked-in `C-sources/libogc` tree. Rust's tick arithmetic corresponds to 100 ms at the declared Wii timebase, but the exact C scheduler behavior and real elapsed-time jitter are unverified.
- The Rust calls `sys_get_hollywood_revision` as its Wii check (`ogc-wd/src/lib.rs:145–148`). Its low-memory detection and behavior under all launchers were not re-reviewed in this task.
- C's stack-based scan buffer can be large when `buffsize` is large. C does not define an explicit maximum; whether the largest `u16` values succeed depends on stack availability and IOS behavior. Rust now supports the full public `u16` size through its IPC heap. Actual C success near the `u16` maximum still depends on caller stack capacity and IOS behavior.
- Raw ioctl error handling differs by design in several places. Compatibility expectations for callers that rely on C's unconditional zero return from `WD_GetInfo`/`WD_Scan`/`WD_ScanOnce` are not established.
- HBC's first Wii scan after removing the runtime Hollywood-revision guard returned `IPC_EINVAL` (`-4`). HBC now uses the detailed scan API to identify whether the failing request is the NCD lock, WD open, scan ioctl, close, or unlock. The returned stage from a rebuilt DOL is still required to locate this error.
- The Rust fd atomics do not serialize two simultaneous `init` calls across open/store; the C global fd is also unsynchronized. This concurrency edge was identified by source inspection but not reproduced or classified as a confirmed bug.
- Source tests cover default scan values, maximum scan heap sizing, IE traversal, and fail-closed host stubs (`ogc-wd/src/lib.rs:712–857`). They were not executed because the WD test target has the compile errors listed above.


## Independent re-review: scan buffer size limit

Reviewer: `worker_b`. Implementer: `/root/worker_a`.

The capacity calculations match the code paths. C `WD_Scan` allocates `buffsize + 2` bytes aligned to 32 bytes and sends `buffsize` bytes in its output vector (`wd.c:151-169`). Rust requests `buffsize + 64` bytes from `HeapId::create` and allocates `buffsize + 2` bytes. For `buffsize == u16::MAX`, the allocation size is 65,537 bytes; the IPC allocator rounds this to 65,568 bytes and adds its 32-byte header, for a total of 65,600 bytes. The new `IPC_HEAP_MAX_BYTES` and aligned static heap have exactly that capacity. The heap allocator initializes and accounts for the full aligned static capacity, and `HeapId::create` accepts the 65,599-byte request. The WD and IPC regression checks encode these maxima. I found no remaining mismatch in the scan-size limit finding.

PowerPC production checks passed per the handoff. The WD test check is blocked by unrelated existing WD test compile errors, so the tests were not run. No hardware verification was performed. Other WD differences in the note remain open.
