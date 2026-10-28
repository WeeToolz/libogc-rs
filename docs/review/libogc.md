# libogc Review

Date: 2026-07-02

Status: phase 0 reviewed with strict host and Wii PowerPC build coverage.

## Scope

- Workspace-level Rust replacement and wrapper surface
- `*_sys` ABI compatibility boundaries
- Probe/example build behavior for the Wii target

## Current implementation

The workspace contains Rust replacements and compatibility layers for selected libogc subsystems. The phase 0 surface is intentionally split between host-testable behavior and Wii PowerPC behavior that must be validated on hardware.

Recent strict-build work removed warnings-as-errors failures across PowerPC builds, fixed missing allocator linkage for examples, replaced unresolved framebuffer allocation in `ogc-system-sys`, and added targeted target-only allowances where compatibility code still depends on libogc-style statics or unreachable target configuration.

## Verified coverage

- `cargo xtask verify` passes.
- `cargo xtask build-dols` passes.
- `RUSTFLAGS='-D warnings' cargo xtask build-dols` passes.
- Nightly PowerPC `build-std` checks pass for previously failing `wpad-sys` and `tinysmb-sys` target paths.
- `cargo xtask phase0-readiness` verifies that required review and decision documents are present.

## Remaining validation

- `phase0-readiness` is a mechanical guard. It confirms required files exist; it does not judge whether the content is sufficient.
- Host-model tests do not establish hardware equivalence for DI, audio, WPAD, networking, filesystem, or long-running SMB behavior.
- Some PowerPC-only warning allowances remain as compatibility debt, but they are not current blockers after strict target builds pass.
- ABI and semver policy are post-validation release decisions, not current implementation blockers.

## Phase 0 position

The phase 0 bar is met for reproducible verification, strict build cleanliness, and documented risk separation. Real Wii hardware validation is the remaining gate.

## First-pass static review: `libdb-sys` and `wpad-sys`

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Review type: source comparison only; no tests or builds run.

### `libdb-sys` — confirmed mismatch

- C `libdb/debug.c:586-590` implements `_break()` by emitting the PowerPC trap instruction when the debugger is initialized. `debug.c:593-616` initializes the breakpoint table, selects the USB or TCP interface, installs `c_debug_handler` as the panic/exception handler, and marks debugging initialized when interface setup succeeds.
- Rust `crates/compatibility/libdb-sys/src/lib.rs:328-358` implements `_break()` only by incrementing `DEBUG_BREAKS`; it never traps. `debug_init()` sets up an interface and marks it active, but does not install an exception handler or run the GDB stub. Therefore a caller's `_break()` does not enter the C debugger, and exceptions cannot be reported or serviced through the initialized Rust interface. The debugger's primary observable behavior is absent.
- C exposes `DEBUG_Init` and `_break` (`debug.c:586,593`). Rust now exports `DEBUG_Init`, but `_break` previously had only a Rust-mangled symbol. The `_break` export is added in the current handoff below. The missing trap and exception handler remain a separate behavior mismatch.

C files inspected: `libdb/debug.c`, `libdb/debug_if.h`, `libdb/geckousb.c`, `libdb/tcpip.c`, `libdb/tcpip.h`. Rust inspected: `crates/compatibility/libdb-sys/src/lib.rs`, `Cargo.toml`. The TCP/USB transport paths exist in Rust, but they do not compensate for the missing debug handler and trap behavior.

### `wpad-sys` — confirmed API parity mismatches

- C defines `WPAD_CHAN_UNKNOWN` as numeric channel 5 and `WPAD_MAX_DEVICES` as 6 (`gc/wiiuse/wpad.h:39-48`). C APIs accept channels from zero up to, but not including, `WPAD_MAX_DEVICES`; for example `WPAD_SetMotionPlus` checks this at `wiiuse/wpad.c:1554`. Rust sets `WPAD_MAX_DEVICES` to 6 but `valid_chan()` only accepts `0..=WPAD_BALANCE_BOARD` (0 through 4) (`crates/services/wpad-sys/src/lib.rs:614-620`). Rust therefore returns `WPAD_ERR_BAD_CHANNEL` for channel 5 where C accepts it. The Rust all-channel loops also iterate six entries, including index 5, so this rejection is inconsistent with its own aggregate behavior.
- C `WPAD_SetMotionPlus(WPAD_CHAN_ALL, enable)` iterates every channel from zero through `WPAD_MAX_DEVICES - 1` and returns success if the per-channel calls succeed (`wiiuse/wpad.c:1541-1554`). Rust `set_motion_plus()` calls `valid_chan()` before dispatching to the PowerPC backend and does not special-case `WPAD_CHAN_ALL` (`crates/services/wpad-sys/src/lib.rs:1070-1075`), so `-1` returns `WPAD_ERR_BAD_CHANNEL` instead of updating all channels.

C files inspected: `gc/wiiuse/wpad.c`, `gc/wiiuse/wpad.h`. Rust inspected: `crates/services/wpad-sys/src/lib.rs`, `crates/services/wpad-sys/src/powerpc_backend.rs`. Review is limited to source paths inspected above; other WPAD behavior remains unreviewed.

### Handoff

Both crates remain open with confirmed findings. No implementation changes were made. No tests or builds were run, as required for this review thread. The shared ledger had pre-existing staged changes and concurrent worktree edits, so the claim and result rows are left unstaged for safe integration; no review commit was created.

## Independent re-review: `libdb-sys` TCP and callback changes

Result: **NEEDS_CHANGES**. Reviewer: `/root/reviewer_d`. Implementer for the reviewed changes: `notOrrytrout`, commits `c6f02b1` (TCP write contract), `62a56d4` (blocking socket waits), and `8864287` (callback ABI). I inspected each commit's `libdb-sys` diff and compared current code with C `libdb/debug.c`, `debug_if.h`, `tcpip.c`, `tcpip.h`, and `geckousb.c`.

### Re-verified prior `libdb-sys` findings

- **The GDB exception handler and trap behavior are still absent.** C `DEBUG_Init` initializes breakpoints, selects an interface, installs `c_debug_handler` in `PPCExcptCurPanicFn`, and sets `dbg_initialized` when setup succeeds (`../../C-sources/libogc/libdb/debug.c:593-616`). C `_break` executes the PowerPC trap instruction only when initialized (`debug.c:586-591`), which reaches the debugger handler (`debug.c:410-428`). Rust `debug_init` only sets `CURRENT_DEVICE` and `DEBUG_ACTIVE` (`crates/compatibility/libdb-sys/src/lib.rs:335-359`), and Rust `_break` only increments a counter (`lib.rs:328-333`). It does not trap or service GDB commands. This is a confirmed primary behavior mismatch.
- **The `_break` export is fixed; its trap behavior remains open.** Rust now exports `_break` with the C ABI alongside `DEBUG_Init` and the ten public TCP symbols. The regression test type-checks its no-argument C ABI signature. The function retains its counter-only behavior because Rust does not have the GDB exception handler needed to service the PowerPC trap safely.

Implementer: `worker_a`. Fix status: the `_break` C ABI export is fixed; needs independent review.

Validation: `cargo check -p libdb-sys --tests --target powerpc-unknown-linux-gnu` passed; this compiles target test code, but tests did not run. A target object was emitted and `nm -g` confirmed `_break` is a global text export. `git diff --check` passed. No hardware verification was performed.

### Re-review of the claimed TCP fixes

- **Write completion convention now matches for successful full writes.** C `tcpip_write` chunks by `uip_tcp_sndbuf`, polls while the send buffer is empty, then returns `UIP_ERR_OK` (zero) after the full requested length (`../../C-sources/libogc/libdb/tcpip.c:404-429`). Rust `tcpip_write` loops over partial `network::write` results and returns zero when all bytes are written (`crates/compatibility/libdb-sys/src/lib.rs:602-633`). The callback now delegates to it (`lib.rs:268-296`). This corrects the previous positive-byte-count return mismatch for normal complete writes.
- **Blocking after readiness timeouts now matches the unbounded C wait intent.** C `tcpip_accept` and `tcpip_read` repeatedly call `__tcpip_poll()` until a connection/data is available (`tcpip.c:341-356,359-401`); C `tcpip_write` polls until send capacity exists (`tcpip.c:414-429`). Rust `wait_for_socket_event_blocking` retries every select timeout until the result is nonzero (`lib.rs:105-112`), and accept/read/write use it (`lib.rs:538-589,614-631`). Thus a zero readiness result no longer turns these operations into an early failure. A negative `select` result still returns an error in Rust; C's internal poll loop has no corresponding error return.
- **Write error behavior is still different.** C advances the buffer and remaining length even if `uip_tcp_write` reports an error, and returns `UIP_ERR_OK` after the loop (`tcpip.c:419-429`; only successful writes trigger `uip_tcpoutput`). Rust closes the socket and returns `-1` if `network::write` returns zero, a negative value, or more than the requested chunk (`lib.rs:622-629`). This is a confirmed error-path difference; it may be a deliberate reliability fix, but it is not C behavior.
- **The TCP backend and address setup do not match C.** C initializes the uIP/BBA stack and configures its network interface with the supplied local IP, netmask, and gateway (`tcpip.c:237-285`); `DEBUG_Init` derives those arguments from the public TCP address strings (`debug.c:601-609`), whose weak defaults are empty strings (`tcpip.c:35-37`). Rust `tcpip_init` explicitly discards all three address pointers and instead calls `network::init`, creates a system socket, and binds to `INADDR_ANY` (`crates/compatibility/libdb-sys/src/lib.rs:377-413,114-124,387-409`). The C source also creates a BBA/uIP netif rather than a socket in the platform network stack. The public initialization behavior and transport path therefore differ. This is a broader backend parity gap beyond the three bounded fixes.
- **TCP callback results differ when there is no active handle (fixed; needs independent review).** C `readtcpip` and `writetcpip` return zero when `device->fhndl < 0` (`tcpip.c:113-127`). Rust previously returned `-1` for both callbacks. Rust now returns zero for a TCP interface with a negative handle before validating or using the data buffer. Regression coverage checks both callback results in that state (`crates/compatibility/libdb-sys/src/lib.rs`).

Implementer: `worker_a`. Fix status: fixed; needs independent review.

Validation: `cargo check -p libdb-sys --tests --target powerpc-unknown-linux-gnu` passed; this compiles target test code, but tests did not run. `git diff --check` passed. No hardware verification was performed.
- **USB callback transfer results differed (fixed; needs independent review).** C `usbread` and `usbwrite` return the raw `__usb_recvbuffer`/`__usb_sendbuffer` result (`geckousb.c:166-178`); those helpers return zero when EXI lock fails (`geckousb.c:103-123,125-145`). Rust now returns the transfer helper's result, including zero, for copied and static USB interfaces (`crates/compatibility/libdb-sys/src/lib.rs`).

### Callback ABI and validation

The callback ABI fix passes: C `struct dbginterface` contains ordinary C function pointers with the listed argument/result widths (`../../C-sources/libogc/libdb/debug_if.h:9-18`), and Rust's `Dbg*Fn` aliases now use `unsafe extern "C" fn` with matching signatures (`crates/compatibility/libdb-sys/src/lib.rs:20-37`).

`cargo check -p libdb-sys --tests --target powerpc-unknown-linux-gnu` passed in the prior review; this compiled target test code, but tests did not run. No hardware verification was performed. The confirmed debugger/export, backend/address, USB callback, and write-error differences remain open.

## Independent re-review: debugger and TCP exports

**Result: PASS for the bounded exports and no-handle callback fix in `d90db0c`; `libdb-sys` remains open.** Reviewer: `reviewer_f`. Implementer: `worker_a`.

C declares `_break(void)` and `DEBUG_Init(s32, s32)` in `../../C-sources/libogc/libdb/debug.c:586–616`, and declares the ten public TCP entry points in `../../C-sources/libogc/libdb/tcpip.h:10–20`. Rust now exports these names with `#[no_mangle] extern "C"` and matching target-width integer and pointer shapes (`crates/compatibility/libdb-sys/src/lib.rs:340–378,396–402,436–597,630–631`). The callback signatures match C `struct dbginterface` (`debug_if.h:9–18`). C `readtcpip` and `writetcpip` return zero for a negative file handle (`tcpip.c:113–127`); Rust now does this for the static TCP interface (`lib.rs:225–230,274–284`). The ABI type checks and no-handle tests cover these changes (`lib.rs:704–722,752 onward`).

The initial copied-interface fix did not fully match C: it still selected TCP versus USB behavior by checking the interface object's address, and it could apply the TCP no-handle result to a copied USB interface. The follow-up below assigns distinct callback functions to USB and TCP interfaces. Rust `_break` still increments a counter instead of executing C's trap instruction; `DEBUG_Init` does not install the GDB exception handler; and TCP initialization ignores the supplied IP settings and has other error-path differences. These prevent marking the crate reviewed.

### Follow-up handoff: copied TCP interface with no handle

**Status: fixed; needs independent review.** Implementer: `worker_a`.

C assigns separate functions to the USB and TCP callback fields (`../../C-sources/libogc/libdb/debug_if.h`, `geckousb.c:161–178`, and `tcpip.c:113–127`). Rust now does the same. Its TCP callbacks check the handle and use it without requiring the interface pointer to equal the static `TCP_IFACE`; its USB callbacks follow the USB path without a static `USB_IFACE` identity check (`crates/compatibility/libdb-sys/src/lib.rs`). Regression tests invoke callback fields on copied TCP and USB interfaces. The copied TCP callbacks return zero for a negative handle. The copied USB callbacks return the USB helper's raw transfer result, including zero when the channel is invalid and the helper cannot acquire its transport lock.


`cargo check -p libdb-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled the test code; tests did not run. `git diff d90db0c^ d90db0c --check` passed. No hardware verification was performed. The bounded exports pass; keep `libdb-sys` in a finding state for the remaining differences.
# Independent `libdb-sys` parity audit (2026-10-03)

**Result: NEEDS CHANGES.** Reviewer: `/root/reviewer_e`. Implementer of the exported-symbol/no-handle fix: not identified in the available handoff; commit `d90db0c`.

The fix's exported entry points and `DbgInterface` callback ABI pass the bounded check. Broader mismatches remain: C installs the debugger panic handler and `_break` traps into it, while Rust installs no handler and `_break` only increments a counter (`C-sources/libogc/libdb/debug.c:410-428,586-616`; `crates/compatibility/libdb-sys/src/lib.rs:340-378`). C's TCP setup uses configured local IP, netmask, and gateway and its timer helpers drive the uIP timer; Rust uses generic networking with `INADDR_ANY` and timer helpers only set an unread flag (`C-sources/libogc/libdb/debug.c:593-616`, `libdb/tcpip.c:210-228,237-285,443-463`; Rust `lib.rs:396-434,463-480`).

Other confirmed behavior differences: TCP write failures are reported as `-1` in Rust but C returns `UIP_ERR_OK` (`tcpip.c:404-430`; Rust `lib.rs:631-662`); Rust may return the active accepted socket instead of dequeuing the next queued socket (`tcpip.c:341-357`; Rust `lib.rs:565-591`); USB callbacks map zero-byte results to `-1` in Rust (`geckousb.c:103-145,166-178`; Rust `lib.rs:239-245,295-300`); copied interfaces with a negative handle do not take C's no-handle path in Rust (`tcpip.c:113-127`; Rust `lib.rs:225-230,274-283`); and Rust collapses TCP bind errors to `-1` (`tcpip.c:308-324`; Rust `lib.rs:531-562`). Backlog and polling behavior remain backend-dependent or uncertain.

Validation: `cargo check -p libdb-sys --tests --target powerpc-unknown-linux-gnu` passed; tests compiled but did not run. No hardware verification was performed.

## Follow-up handoff: negative TCP read size

**Status: fixed; needs independent review.** Implementer: `worker_a`.

The TCP callback now rejects `size < 0` before socket readiness or `network::read`, using a shared validator with regression coverage (`crates/compatibility/libdb-sys/src/lib.rs`). This is a deliberate safe difference: C passes the signed callback size to `tcpip_read`, which accepts an unsigned length (`../../C-sources/libogc/libdb/tcpip.c:113–117,359–400`); a negative size can become an excessively large length. Rust returns `-1` and does not pass that length to the network syscall.

## Follow-up handoff: USB callback transfer results

**Status: fixed; needs independent review.** Implementer: `worker_a`.

C `usbread` and `usbwrite` return the result from `__usb_recvbuffer` and `__usb_sendbuffer` without conversion (`../../C-sources/libogc/libdb/geckousb.c:103–145,166–178`). Those helpers return zero when `EXI_Lock` fails. Rust no longer performs an extra `is_gecko_alive` probe before the transfer, and it preserves zero and positive byte counts from the real `ogc-usbgecko` transfer helpers. The USB transfer-result helper is shared with the callbacks and covered by a unit test (`crates/compatibility/libdb-sys/src/lib.rs`).

The Dolphin `debugger_probe` copies the production USB interface, sets its channel to `-1`, then calls its read and write callback fields. The production transfer helpers return zero for that invalid channel, and both callback results stay zero. The helper check also verifies that zero stays zero. `cargo xtask dolphin-validate --probe debugger_probe` passed with `PASS summary passes=15 fails=0`; report: `target/dolphin-validation/1791172318-e72b056b9111/suite.json`. `cargo check -p libdb-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled tests; it did not run them. This was Dolphin validation, not hardware verification.

### Independent bounded review of USB callback transfer results

**Result: PASS for commit `a5c5555`; broader `libdb-sys` findings remain open.** Reviewer: `reviewer_a`. Implementer: `worker_a`.

C `usbread` and `usbwrite` return the raw values from `__usb_recvbuffer` and `__usb_sendbuffer` (`../../C-sources/libogc/libdb/geckousb.c:166–178`). Those helpers return zero when `EXI_Lock` fails (`geckousb.c:103–145`); otherwise they return the number of bytes transferred. Rust now preserves zero and positive transfer counts for both copied and static USB interfaces (`crates/compatibility/libdb-sys/src/lib.rs:55–62,287–300,328–341`). The underlying Rust transfer helpers return only nonnegative counts, matching this C result range (`crates/services/ogc-usbgecko/src/lib.rs:367–455`). The pre-transfer `is_gecko_alive` check was removed; C's read/write callbacks do not perform that check.

The DOL `debugger_probe` copies the production USB interface, sets `fhndl` to `-1`, invokes its read and write callbacks, and checks that both return zero (`probes/ogc-system-probe/src/debugger_probe.rs:85–112`). Dolphin report `target/dolphin-validation/1791172318-e72b056b9111/suite.json` records Dolphin 2603-415, exit code 0, and 15 passes with 0 failures. This is emulator evidence, not hardware verification. The host unit test also checks the result helper for `-1`, zero, and a positive byte count; the PowerPC test target compiled in the earlier handoff, but those tests did not run.

Rust rejects negative callback sizes with `-1` (`lib.rs:292–293,333–334`), while C's transfer helpers return zero for a negative size if the EXI lock succeeds because the transfer loop does not run. This guard predates `a5c5555` and is retained as a deliberate safe-input divergence. The debugger trap/handler and TCP backend/address differences above remain open; this bounded PASS does not close `libdb-sys` review.

## BUG FIX handoff: preserve TCP bind errors

Implementer: `thread-current/root`. Independent static re-review required.

C `tcpip_bind` returns the underlying `uip_tcp_bind` result unchanged (`../../C-sources/libogc/libdb/tcpip.c:296-324`). Rust previously converted every negative `network::bind` result to `-1`, losing the backend error. Rust now returns the exact bind result and marks `TCP_BOUND` only when the result is zero. The regression exercises the shared production result handler with a fixed `-98` error and success, and checks that only success changes bound state. Other `libdb-sys` findings remain open.

### Independent re-review: NEEDS CHANGES

Reviewer: `reviewer_gamma`. Implementer: `thread-current/root`.

The wrapper now preserves the value returned by `network::bind`, but the production call path has already collapsed backend errors. `ogc-network::bind` calls `ogc-network-sys::net_bind` and returns `SOCKET_ERROR` (`-1`) for every negative backend result (`crates/services/ogc-network/src/lib.rs:483-491`). In contrast, C's `uip_tcp_bind` returns `UIP_ERR_USE` (`-10`) when the requested address and port conflict, and `tcpip_bind` returns that value unchanged (`../../C-sources/libogc/libdb/uIP/uip_tcp.c:560-582`; `libdb/tcpip.c:296-324`). Therefore `finish_tcpip_bind` cannot preserve this C error through the current network wrapper.

The regression calls `finish_tcpip_bind(-98)` directly, so it proves the helper preserves its input and updates `TCP_BOUND` only on success. It does not exercise `network::bind` or prove that the real production path preserves a C bind error. The fix remains open until the production error path can preserve the relevant bind result, with regression coverage through the shared production path. No tests were run during this re-review; the handoff states that target validation was pending. No hardware verification was performed.

## Follow-up handoff: preserve bind errors through `ogc-network`

Implementer: `thread-current/root`. Independent static re-review required.

`ogc-network::bind` now preserves negative results returned by `ogc-network-sys::net_bind` instead of converting them to `-1`. Its production wrapper uses the tested result helper, with coverage for `UIP_ERR_USE` (`-10`) and success. `libdb-sys::tcpip_bind` preserves that result and marks the socket bound only on success; its regression now uses the same C error value and checks state. This addresses reviewer_gamma's finding that the lower-level backend error was lost before reaching `libdb-sys`.

Primary regression: `probes/ogc-system-probe/src/network_probe.rs` now binds two TCP sockets to the same candidate address and port and checks that the public `ogc-network::bind` path returns `-EADDRINUSE` (`-98`), not a generic `-1`. `cargo check -p ogc-network --tests --target powerpc-unknown-linux-gnu` and `cargo check -p libdb-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled target unit-test code; those tests did not run. `cargo xtask dolphin-validate --probe network_probe` compiled the DOL source but could not link: `ogc-sysbase` references native `AddDevice`, `FindDevice`, and `RemoveDevice` symbols. Adding stock devkitPro `libsysbase.a` resolves those symbols but creates duplicate definitions of `devoptab_list` and `__getreent`, so no DOL was produced or executed. No hardware verification was performed.

### Independent re-review of `libdb-sys`: NEEDS CHANGES

Reviewer: `reviewer_gamma`. Implementer: `thread-current/root`.

The Rust wrapper now passes the production `network::bind` result through and changes `TCP_BOUND` only when that result is zero (`crates/compatibility/libdb-sys/src/lib.rs:564-599`). However, this does not yet match C `libdb`'s duplicate-bind result. C `tcpip_bind` uses its own uIP stack and returns `uip_tcp_bind`'s `UIP_ERR_USE` (`-10`) for an address/port conflict (`../../C-sources/libogc/libdb/tcpip.c:308-324`; `libdb/uIP/uip_tcp.c:560-582`). Rust's production path calls the Wii IOS networking API, whose duplicate bind is reported as `-EADDRINUSE` (`-98`), as checked by the network DOL probe. The `libdb-sys` regression calls `finish_tcpip_bind(-10)` directly; it does not exercise the production bind path or establish that Rust returns the C `-10` result. The helper's state update is correct, but its error-parity finding remains open.

The target unit-test checks compiled but did not run. The network DOL source compiled but did not link because of unresolved `AddDevice`, `FindDevice`, and `RemoveDevice` symbols in `ogc-sysbase`; the stock `libsysbase.a` workaround causes duplicate `devoptab_list` and `__getreent`. The DOL was not produced or executed. No hardware verification was performed.

## BUG FIX handoff: translate network address conflict to uIP error

Implementer: `thread-current/root`. Independent static re-review required.

C `tcpip_bind` uses uIP and returns `UIP_ERR_USE` (`-10`) when an address/port is already in use. Rust uses the IOS network stack, which reports the equivalent condition as `-EADDRINUSE` (`-98`). `tcpip_bind` now maps only that specific error to `UIP_ERR_USE`, preserves other backend errors, and still marks `TCP_BOUND` only after a zero result. A target unit test checks both the `-98` translation and the bound-state transition. The network DOL probe checks that generic `ogc-network::bind` returns `-98` and the libdb wrapper returns `-10` for the same conflict.

`cargo check -p libdb-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled test code; tests did not run. The DOL source compiled, but the DOL could not link because of the native devoptab symbols described above. No DOL execution or hardware verification is claimed.

### Independent re-review: PASS for duplicate-bind error parity (`ef6b5e6`)

Reviewer: `reviewer_gamma`. Implementer: `thread-current/root`.

C `libdb::tcpip_bind` returns uIP's `UIP_ERR_USE` (`-10`) for an address/port conflict (`../../C-sources/libogc/libdb/tcpip.c:308-324`; `libdb/uIP/uip_tcp.c:560-582`). The Wii networking API returns `-EADDRINUSE` (`-98`) for the same conflict (`../../C-sources/libogc/libogc/network_wii.c:743-760`). Rust `finish_tcpip_bind` maps that specific backend error to `UIP_ERR_USE`, returns all other nonzero results unchanged, and sets `TCP_BOUND` only for result zero (`crates/compatibility/libdb-sys/src/lib.rs:593-604`). Its test checks the `-98` to `-10` mapping, preservation of an existing `-10`, failure state, and successful bound-state transition (`lib.rs:798-808`).

The network DOL case finds a port that binds on one socket, checks that a duplicate generic bind returns `-98`, then calls production `libdb_sys::tcpip_bind` on a third socket with the same address and checks for `-10`. It logs cleanup results for all three sockets (`probes/ogc-system-probe/src/network_probe.rs:179-250`). `cargo check -p libdb-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled unit tests; tests did not run. The DOL source compiled, but the DOL could not link because `ogc-sysbase` has unresolved `AddDevice`, `FindDevice`, and `RemoveDevice`; stock `libsysbase.a` resolves these but duplicates `devoptab_list` and `__getreent`. The DOL was not produced or executed. No hardware verification was performed. Other `libdb-sys` findings remain open.

## Deliberate Rust correction: TCP write errors

C `tcpip_write` advances its buffer and returns `UIP_ERR_OK` even when `uip_tcp_write` fails; it only skips `uip_tcpoutput` for that failed chunk (`../../C-sources/libogc/libdb/tcpip.c:404-429`). Rust returns `-1` when its production `network::write` fails or reports an invalid byte count (`crates/compatibility/libdb-sys/src/lib.rs:663-698`). This difference is intentional: reporting success after a failed write would hide a failed debugger transport operation. The C error-path mismatch is documented as a Rust correctness fix and does not require reproducing C's success-on-failure behavior. Other `libdb-sys` parity findings, including the missing GDB trap/handler and the BBA/uIP backend difference, remain open.

## Bounded parity handoff: `_break` trap instruction

Implementer: `thread-current/root`. Independent static re-review required.

C `_break` returns immediately until `DEBUG_Init` succeeds, then executes the PowerPC trap instruction at the globally visible `__breakinst` symbol. The debugger handler recognizes that exact PC and advances past it after servicing the exception (`../../C-sources/libogc/libdb/debug.c:410-428,586-616`). Rust `_break` now uses a distinct `DEBUG_INITIALIZED` guard, publishes the same `__breakinst` assembly symbol, and calls it so the stopped PC is the public trap address. The existing break counter remains as internal instrumentation; it is not part of the C ABI.

This is only a bounded trap-emission fix. `libdb-sys` remains `parity_blocked` because Rust still does not install or implement the GDB exception handler, and the TCP debugger backend still uses the generic network stack rather than C's uIP/BBA path. No crate status changes were made.

## 2026-10-08 HBC power-off path review

C initializes `/dev/stm/immediate` with IOS open mode zero (`../../C-sources/libogc/libogc/stm.c:80-91`). The standalone Rust `ogc-stm` backend also uses `IPC_OPEN_NONE`. The `ogc-system-sys` private STM implementation instead used `IPC_OPEN_RW` in both `stm_init_local` and `stm_ioctl`. Both now use `IPC_OPEN_NONE`, matching C for initialization, reboot, standby, idle, and LED requests. No persistent storage operation was changed.

HBC's Shutdown action selects `SYS_POWEROFF`. It can abort before the STM call on saved-settings, storage, disc, or receiver cleanup errors. The receiver stop joins its worker without an overall time limit; a worker blocked in a synchronous IOS request can hold up exit. If the final STM request fails, the existing C-compatible reset API cannot return an error and proceeds through fallback subsystem teardown; HBC then spins. These remain diagnostic/recovery limitations, not resolved findings.

PowerPC library compilation for `ogc-system-sys` and `ogc-system` passed. The full target test compilation failed on existing test errors (including unsafe calls and type/name errors); it was not marked passing. A real hardware regression must confirm power-off after entry through HBC, including the network-active path. That check was deferred because the user is away and cannot recover the Wii. No DOL upload or power-off request was made. The broader inventory/completeness audit remains blocked by the stale `ogc-network::bind_addr` evidence reference recorded in `libnetwork.md`.

## BUG FIX — release test selection and probe startup isolation (2026-10-15)

The release check found two tool defects. `test-host` omitted the existing DI retry and video state test suites. They now run with the other standalone host suites. A regression scans direct `tests/*/Cargo.toml` paths and fails if a suite is absent from the selected manifests.

The full probe build failed to link `__wii_std_initialize` and `__wii_std_runtime_return` in system probe binaries. A single Cargo invocation selected both the LWP and system probe packages. Cargo merged the LWP dependency's `ogc-sys/wii-std` feature into system probes that do not link the runtime provider. `probe_build_invocations` now selects each package in a separate command. It retains the same compiler, target, flags, output directory, requested features, and required artifacts. A regression covers all example/report selection combinations and requires one package per invocation.

The full host command passed for 25 packages. All probe DOLs and the configured example DOLs built successfully after the fix. Wii runtime compilation passed for 83 packages. Target ABI inspection and the hardware-register evidence validator passed. Builds are compile and link evidence only; no probe ran on Wii hardware during this pass. Existing warnings remain in example source and dependency manifests. The runtime completeness gate and open source reviews still prevent release acceptance.
