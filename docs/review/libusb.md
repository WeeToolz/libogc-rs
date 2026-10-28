# USB Parity Review

Date: 2026-10-01

Current status (2026-10-05): The bounded v5 close callback and mouse cleanup fixes have independent passes recorded below. HID v5 open preflight `6a0f860` and subordinate-descriptor conversion `218397a` passed a second independent review with fixed-input DOL coverage; the first DOL review was by its author and is not counted as independent. Coverage does not claim IOS transport or hardware validation. USB storage commits `38344b5` and `6c84b5f` have bounded policy/helper passes, but actual production GetMaxLUN timeout parity is incomplete because the synchronous control path has no timeout mechanism; a separate close-failure cleanup difference and other BOT findings remain open in `libusbstorage.md`. Keyboard shared-USB and stdin install-order findings are tracked in `libwiikeyboard.md`. No Wii hardware validation is claimed.

## Scope

- `crates/services/ogc-usb/src/lib.rs`: safe and raw USB wrappers, plus the mouse module.
- `crates/services/ogc-usb-sys/src/lib.rs` and `src/v5_layout.rs`: raw API, descriptor parsing, IOS v0/v5 transfers, event watcher, and layout helpers.
- `crates/services/ogc-usb-events/src/lib.rs`: callback and device snapshot state.
- C headers: `C-sources/libogc/gc/ogc/usb.h` and `usbmouse.h`.
- C implementations: `C-sources/libogc/libogc/usb.c` and `usbmouse.c`.

I inventoried every public Rust function, type, and constant in the three crates and compared the USB declarations in both C headers. I traced each declared `USB_*` operation to its C implementation and its Rust sys implementation. I also reviewed the Rust mouse entry points against all `MOUSE_*` C entry points, and mapped the Rust event state machine to the C v5 callbacks and host lifecycle code. Private Rust helpers and private C helpers were reviewed where they implement these public paths. Host simulation branches are not part of the PowerPC C comparison.

### Coverage map

- `ogc-usb-sys/src/lib.rs`: constants, callback and descriptor types, initialization, device open/close, descriptors, HID and report descriptors, all synchronous/asynchronous transfer calls, removal/change notifications, suspend/resume, device listing, configuration/interface/halt calls, and string retrieval. C counterparts are declared in `usb.h:9-234` and implemented in `usb.c:282-1515`.
- `ogc-usb-sys/src/v5_layout.rs`: host classification, device-list selection, descriptor offsets, interrupt headers, and timeouts. These helpers map to C's v5 host/device and transfer paths (`usb.c:191-220,269-279,282-373,797-893,1390-1441`).
- `ogc-usb-events/src/lib.rs`: registration, snapshots, one-shot callback batches, stale generations, failure, close, and stop. The corresponding C state and callbacks are in `usb.c:164-279,724-748,1273-1387,653-674`.
- `ogc-usb/src/lib.rs`: raw forwarding calls and safe wrappers for all USB operations; C API coverage is as above. The mouse module's init/deinit, connected state, event retrieval, and flush operations map to `usbmouse.h:17-22` and `usbmouse.c:281-406`; mouse discovery and transfer flow map to `usbmouse.c:163-279,287-355`.

The C `USB_MAXPATH` macro has no Rust public equivalent, as noted below. Rust's `initialize_vendor` and `deinitialize_vendor` functions are extra APIs with no C declaration. Host-only simulation implementations and Rust's host-side tests were not compared with C, because the C library has no corresponding host simulation module.

## Initial review findings (historical; see current status and later handoffs)

- Opening a v5 HID device does not perform the descriptor check that C performs. C resumes the HID device, calls `USB_GetDescriptors`, frees the descriptors on success, and suspends the device again on failure before it returns an error (`C-sources/libogc/libogc/usb.c:688-701`). Rust resumes a found v5 device and returns its ID immediately (`crates/services/ogc-usb-sys/src/lib.rs:1534-1542`). This changes whether a device with unreadable descriptors can be returned as open.
- Mouse deinitialization also deinitializes the USB service in Rust. C `MOUSE_Deinit` closes the mouse, flushes events, frees mouse resources, and resets its initialized flag; it does not call `USB_Deinitialize` (`C-sources/libogc/libogc/usbmouse.c:358-379`). Rust calls `deinitialize()` after mouse cleanup (`crates/services/ogc-usb/src/lib.rs:1083-1110`). Applications that keep using USB after mouse deinitialization will see different service state.
- **Resolved in the later parity pass for valid buffers: `USB_GetAsciiString` request and conversion behavior.** Rust now requests the full 255-byte string descriptor regardless of the caller output length, clamps the output length to 255 as C does, and accepts any low byte when the UTF-16 high byte is zero. The Rust null/zero-length guard remains as a deliberate safety check for invalid inputs.

### BUG FIX: return the v5 close callback result

C `USB_CloseDeviceAsync` returns `cb(ret, userdata)` when `USBV5_CloseDevice` handles the file descriptor (`../../C-sources/libogc/libogc/usb.c:763-778`). Rust previously called the callback and then returned the close result. The v5 Rust path now returns the callback result when a callback is present and returns the close result when it is absent. The same helper is used by the host compatibility path. Implementer: `/root/worker_c`. Independent re-review is assigned to `/root/reviewer_4`.

Regression coverage passes distinct values for the close result and callback result. It checks that the callback receives the close result while `close_device_async_result` returns the callback's different value; it also checks that no callback returns the close result. `cargo check -p ogc-usb-sys --target powerpc-unknown-linux-gnu` passed. `cargo check -p ogc-usb-sys --tests --target powerpc-unknown-linux-gnu` did not complete because of pre-existing errors elsewhere in the test module: Rust ABI callbacks are passed where C ABI callbacks are required, packed fields are borrowed by assertions, and two unsafe functions are called without unsafe blocks. The regression test was not run. No hardware verification was performed.

## Differences with uncertain intent or target effect

- **Resolved in the later parity pass:** `ogc-usb-sys` now exports `USB_MAXPATH` as 64, matching `IPC_MAXPATH_LEN` in the current C headers.
- The full-descriptor request difference has been removed; Rust now requests 255 bytes before converting, as C does. Device-level validation is still pending.

## Intentional Rust additions and structural differences

- `ogc-usb-events` provides fixed-capacity, generation-aware callback state (`crates/services/ogc-usb-events/src/lib.rs:7-35,80-103`). C keeps the corresponding state in the USB v5 host structure and static callbacks (`C-sources/libogc/libogc/usb.c:164-279`). This is an internal Rust support crate, not a missing C library surface.
- Rust drains pending v5 callbacks with an error during watcher failure and USB deinitialization (`ogc-usb-sys/src/lib.rs:1129-1139,1460-1501`). C deinitialization shuts down and frees each host without invoking its stored removal/change registrations (`usb.c:653-674`). Rust's callback completion behavior is an intentional lifecycle addition, but callback result expectations should be checked by callers.
- Rust mouse events use a fixed queue of 64 entries and replace the oldest event when full (`ogc-usb/src/lib.rs:558-560,648-656`). C allocates one queue node for each event and has no fixed capacity (`usbmouse.c:83-91,306-310`). This is an intentional bounded-storage behavior difference.
- The safe Rust API adds `Result` helpers and slice-based descriptor, device-list, and string operations (`ogc-usb/src/lib.rs:14-22,47-49,93-148,517-553`). These are convenience APIs on top of the raw calls, not C API replacements. Rust also uses idiomatic lowercase function names for the C declarations.
- Rust's device-list path can use an available HID v5 host when the vendor host is absent; the C function enters the legacy OH0 path whenever `ven_host` is null (`C-sources/libogc/libogc/usb.c:1395-1419`; `ogc-usb-sys/src/lib.rs:3777-3815`, with policy in `src/v5_layout.rs:18-20`). The Rust layout test calls out this behavior (`v5_layout.rs:51-59`). This appears to be an intentional correction for HID-only host availability.
- Rust applies explicit cache flush/invalidate operations around DMA transfers and completion callbacks (`ogc-usb-sys/src/lib.rs:603-670,2149-2154,2218-2222`). The corresponding C transfer helpers set IOS vector buffers without equivalent explicit calls (`C-sources/libogc/libogc/usb.c:282-373,375-476,478-561`). This is a Rust-side cache-coherency addition; the hardware effect has not been validated in this review.
- The Rust `UsbHidDesc` represents one subordinate descriptor, like C's `descr[1]` (`ogc-usb-sys/src/lib.rs:256-265`; `C-sources/libogc/gc/ogc/usb.h:162-173`). Rust report-size lookup checks that one entry (`ogc-usb-sys/src/lib.rs:2079-2100`); C loops over `bNumDescriptors` (`usb.c:1088-1112`). The C type itself has one array element, and the C comments state that only one descriptor is supported. This difference for multi-entry HID descriptors remains uncertain and is not reported as a confirmed usable behavior mismatch.

## Independent re-review — async close callback and mouse deinitialization

- Result: **NEEDS_CHANGES**
- Reviewer: `/root/reviewer_f`
- Implementers: `worker_c` for `d6336b2`; `worker_a` for `abbb4b2`.
- Fix commits: `d6336b2` (`Return USB v5 close callback result`) and `abbb4b2` (`Keep USB service active after mouse deinit`).
- Scope reviewed: the changed close-result behavior and mouse cleanup behavior for `ogc-usb`, `ogc-usb-events`, and `ogc-usb-sys`. Other findings in this note remain open.

### v5 async close callback result — targeted fix passes

C `USB_CloseDeviceAsync` first calls `USBV5_CloseDevice(*fd)`. When that is not `IPC_EINVAL`, C returns the callback result if a callback exists, and otherwise returns the close result (`C-sources/libogc/libogc/usb.c:763-778`). `USBV5_CloseDevice` returns `IPC_EINVAL` when the device is not in either host; for a registered device it invokes and removes the per-device removal callback with result zero, ignores that callback's return, then returns zero (`usb.c:724-748`). The outer async close callback is therefore given zero and its own result is returned.

On PowerPC, Rust classifies v5 IDs and delegates to `close_device`; for a v5 host it obtains the removal callback batch, marks the descriptor closed, invokes the removal callback with `USB_OK`, and returns `USB_OK` (`crates/services/ogc-usb-sys/src/lib.rs:1627-1661`; `crates/services/ogc-usb-events/src/lib.rs:223-231`; `ogc-usb-sys/src/lib.rs:1136-1143`). `close_device_async` then passes that close result to `close_device_async_result` and returns the user callback's result; with no callback, the helper returns the close result (`ogc-usb-sys/src/lib.rs:1616-1621,1677-1693`). Null descriptors and invalid nonpositive legacy IDs return before invoking a callback, consistent with C's early returns (`usb.c:763-778`; Rust 1677-1693). Positive non-v5 descriptors continue to use the IOS async close path rather than this immediate v5 helper, matching C's fallback to `IOS_CloseAsync` (`usb.c:774-775`; Rust 1691-1693).

The fix commit's test uses deliberately distinct close and callback results and checks both callback-present and no-callback helper behavior (`ogc-usb-sys/src/lib.rs:4090-4117`). It does not exercise the PowerPC host classification or actual v5 close path, so source comparison remains the evidence for that path. I found no mismatch in the requested v5 callback result change.

### Mouse deinitialization — successful path fix passes; cleanup error path remains different

C `MOUSE_Deinit` returns 1 immediately when not initialized. Otherwise it stops and joins its thread, calls `USBMouse_Close`, flushes the event queue, frees mouse data and state, destroys the semaphore if present, clears its semaphore handle, clears the initialized flag, and returns 1 (`C-sources/libogc/libogc/usbmouse.c:359-379`). `USBMouse_Close` ignores the result of `USB_CloseDevice` and sets the descriptor to -1 (`usbmouse.c:154-161`). C does not deinitialize the USB service from `MOUSE_Deinit`.

The `abbb4b2` change removes the call to Rust's `deinitialize()` from the successful cleanup path and clears `state.inited` through `finish_deinit` (`crates/services/ogc-usb/src/lib.rs:713-716,1069-1092`). This now leaves the USB service active as C does. The close and queue cleanup also occur before state reset (`:1069-1085`), and an already-uninitialized call returns 1 (`:1031-1041`).

However, Rust aborts cleanup when `mouse_close` returns a negative result, before it clears the queue, destroys the semaphore, or clears `inited` (`crates/services/ogc-usb/src/lib.rs:758-767,1069-1092`). C's `USBMouse_Close` discards the close result and `MOUSE_Deinit` continues through all cleanup and clears `_mouse_is_inited` (`usbmouse.c:154-161,359-379`). Rust also returns early if `sem_destroy` fails and retains its semaphore handle and initialized state; C ignores the destroy result, clears `_mousesema`, and clears `_mouse_is_inited` (`usbmouse.c:374-379`; Rust `ogc-usb/src/lib.rs:1077-1092`). These failures produce different return values and state after deinitialization. The targeted service-lifecycle fix is correct on the successful path, but the cleanup error behavior still does not match C; keep the group open for implementer disposition.

The mouse regression test calls only `finish_deinit` on a synthetic state and verifies the initialized flag. It does not exercise `deinit`, close failures, semaphore destruction, or the service remaining initialized (`ogc-usb/src/lib.rs:1137-1148`). The close-result regression test similarly tests the extracted helper, not an actual PowerPC v5 host.

### Validation

`cargo check -p ogc-usb --target powerpc-unknown-linux-gnu` passed, compiling the `ogc-usb`, `ogc-usb-sys`, and `ogc-usb-events` target code. No tests ran in this check. `cargo test -p ogc-usb-sys` did not build on the host because dependent crates reject non-PowerPC builds. `cargo test -p ogc-usb-sys --tests --target powerpc-unknown-linux-gnu --no-run` failed while compiling tests: `v5_layout.rs` refers to `std::vec::Vec` in three locations, but the no-std test build has no `std` crate linked. Thus the close callback regression test did not compile or run in this validation. No mouse tests ran, and no hardware verification was performed.

## Validation boundary

This is a source comparison. The v5 close callback result fix (`d6336b2`) and mouse deinitialization parity fix (`abbb4b2`) are assigned to `/root/reviewer_4` for independent re-review; the other confirmed mismatches above remain open. The regression test was compiled for PowerPC but not run. No hardware checks were run. The checked-in declarations and implementations above form the reviewed scope; this review does not cover downstream users, alternate libogc revisions, or undocumented IOS driver behavior.


## Later parity pass: string descriptors and public path constant

`USB_GetAsciiString` now follows C's valid-input behavior for request sizing and byte conversion: the control request asks for 255 bytes, caller `wLength` is capped at 255 only for output, and a zero UTF-16 high byte permits the low byte without an ASCII-only filter. The existing Rust null/zero-length validation remains a safety extension. `USB_MAXPATH` is also public with value 64, matching `IPC_MAXPATH_LEN`. These changes have not been compiled or hardware-tested in this environment. The v5 HID-open descriptor validation finding remains open.


## 2026-10-01 HID v5 open parity follow-up

The v5 HID open descriptor-preflight finding is resolved in the current source. `open_device` delegates v5 preparation to one helper: vendor devices receive the C-style best-effort resume and return, while HID devices additionally read and free descriptors before success. If descriptor reading fails, Rust suspends the HID device and returns the descriptor error, matching `USB_OpenDevice` in current C. The HID-only enumeration branch and unknown-positive-handle close fallback remain open.

## 2026-10-02 device-list and close fallback follow-up

The remaining discovery/close findings are resolved at source level. Device-list selection now follows C's early legacy fallback whenever the vendor v5 host is absent, even if the HID host exists. Unknown positive handles now fall through to IOS close rather than returning `IPC_EINVAL`; the async path submits IOS close asynchronously. Known v5 IDs retain the v5 callback-removal path. Hardware validation remains pending.

## 2026-10-02 asynchronous mouse-queue follow-up

The remaining fixed-capacity mouse event queue difference is resolved. The mouse wrapper now uses a dynamically growing FIFO (`VecDeque`) instead of a 64-entry ring that overwrote the oldest event. This matches C's observable no-fixed-capacity queue behavior without copying C's linked-list allocator. A regression queues 96 events and verifies that all events are returned in FIFO order.

The v5 watcher teardown/callback-order differences are also resolved by the heavy-correctness pass: change callbacks drain in C's reverse-registration order, and USB deinitialization silently discards pending v5 registrations instead of synthesizing `IPC_ENOENT` callbacks. Hardware hotplug validation remains pending.

## Independent audit: mouse deinitialization failure ordering

- Result: **NEEDS_CHANGES**
- Reviewer: `/root/reviewer_e`
- Implementer: `worker_a`
- Implementation commit reviewed: `abbb4b2` (`Keep USB service active after mouse deinit`); no later source fix for these error paths is present.
- Scope: `MOUSE_Deinit` behavior when USB close or semaphore destruction fails.

C `USBMouse_Close` calls `USB_CloseDevice(&_mouse->fd)` without checking its result, then sets `_mouse->fd = -1` (`../../C-sources/libogc/libogc/usbmouse.c:154-161`). `MOUSE_Deinit` then continues to flush events, free mouse allocations, attempt semaphore destruction, clear the semaphore handle, clear `_mouse_is_inited`, and return 1 (`usbmouse.c:359-379`). In particular, a close error does not stop cleanup. A semaphore-destroy error is also ignored; C still sets `_mousesema` to `LWP_SEM_NULL` and `_mouse_is_inited` to false.

Rust's `mouse_close` propagates a negative `close_device` result before it marks the device disconnected (`crates/services/ogc-usb/src/lib.rs:758-767`). `deinit` returns on that error before clearing the queue or attempting semaphore destruction (`:1069-1088`), and therefore does not call `finish_deinit` to clear `state.inited` (`:713-716,1091-1092`). If `sem_destroy` fails, Rust likewise returns before clearing `state.sem` and before clearing `state.inited` (`:1077-1092`). These outcomes differ from C, which continues cleanup, invalidates the close handle, discards the semaphore handle, and resets its initialized flag regardless of those return values. The successful-path fix in `abbb4b2` does not resolve this failure-path mismatch.

Adjacent ordering also differs before resource cleanup: C ignores the return values of `LWP_SemPost` and `LWP_JoinThread` (`usbmouse.c:363-368`), while Rust returns immediately if `sem_post` or `join_thread` fails (`crates/services/ogc-usb/src/lib.rs:1043-1067`). This is additional evidence that Rust can leave the initialized state and resources intact where C proceeds. This audit does not prescribe a fix; the crate remains open for implementer disposition.

The current mouse regression only invokes `finish_deinit` on a synthetic state and checks that `inited` becomes false (`ogc-usb/src/lib.rs:1140-1147`). It does not call `deinit` or inject close, semaphore-destroy, semaphore-post, or join errors, so it does not exercise the mismatch.

### Validation

`cargo check -p ogc-usb --target powerpc-unknown-linux-gnu` passed. This was a compile check; no tests ran. The regression coverage was inspected but not executed. No hardware verification was performed.

## BUG FIX handoff: `MOUSE_Deinit` cleanup after close or semaphore errors

Implementer: `worker_c`. Independent re-review is needed; reviewer assignment and review remain with the review thread. This addresses only the cleanup mismatch above. It does not change the separate semaphore-post or thread-join error paths.

`MOUSE_Deinit` now uses a cleanup helper that ignores the USB close result, sets the mouse descriptor to `-1`, clears connection state and queued events, ignores semaphore-destroy errors, clears the semaphore handle, and resets `inited` before returning 1. `mouse_close` follows C `USBMouse_Close` by clearing the fd even when `close_device` reports an error. USB service state is unchanged by this fix.

Added focused regression tests that inject a close error and a semaphore-destroy error. Both check that cleanup continues, handles are reset, queued events are cleared, and `inited` becomes false. `cargo check -p ogc-usb --target powerpc-unknown-linux-gnu --lib` passed. `cargo check -p ogc-usb --target powerpc-unknown-linux-gnu --tests` failed on an existing unsafe-call error in the unrelated outer test at `ogc-usb/src/lib.rs:1261`; target test code did not compile and tests were not run. `cargo fmt -p ogc-usb -- --check` reports only the pre-existing crate-level compile-error formatting difference. `git diff --check` passed. No hardware verification was performed.

## Independent re-review — HID v5 host version probe (2026-10-03)

Result: **PASSED for commit `6776624`'s host-version probe change**. Reviewer: `/root/reviewer_f`. Implementer: `notOrrytrout` (commit author). The crate remains open for the other findings in this note. This review checks v5 host classification, adjacent vendor-host handling, HID descriptor preflight, device-list selection, and the changed initialization cleanup path.

### Version probe comparison

C `USB_Initialize` opens and validates the vendor v5 host first. If its `USBV5_IOCTL_GETVERSION` succeeds and returns `0x00050001`, C starts its async device-change request (`../../C-sources/libogc/libogc/usb.c:593-617`). C then opens the HID path and probes request 6 (`USBV4_IOCTL_GETVERSION`) before request 0 (`USBV5_IOCTL_GETVERSION`); a v4 reply or an invalid v5 reply causes it to close the HID fd and discard that host (`usb.c:619-644`; request numbers at `:69-76`).

Rust's `initialize()` also initializes vendor before HID and waits for vendor enumeration before it starts HID (`crates/services/ogc-usb-sys/src/lib.rs:1448-1451,1431-1441`). After the version-probe fix, `start_v5_host` passes whether the vendor host was successfully detected into `v5_layout::check_host_version` (`:1325-1336`). For HID with no validated vendor v5 host, the helper sends request 6 and rejects only the `0x00040001` v4 signature before it sends request 0. For HID when a vendor v5 host is already validated, it skips request 6 and checks request 0 directly (`src/v5_layout.rs:8-20`). This avoids interpreting HID-v5 `ATTACHFINISH` (also request 6) as a synchronous v4-version probe, while preserving C's legacy probe when there is no validated vendor v5 host. Vendor hosts always use request 0 and require successful status plus version `0x00050001`.

On version rejection, Rust closes the temporary fd and leaves the host undetected (`lib.rs:1337-1341`). On success, it sets the detected state, publishes the fd, starts host event state, then submits the device-change request (`:1343-1367`). The changed probe itself adds no resource leak or new cleanup mismatch: the allocated version buffer is RAII-owned, and every invalid-version path closes the fd. Existing later failures in watcher startup/cleanup remain outside this targeted version-probe change.

### Open, descriptors, and enumeration

The changed classifier continues to feed the existing device classification: C searches vendor host first, then HID host (`usb.c:682-706`); Rust classifies v5 IDs by sign and confirms membership in that host's attached-device snapshot (`ogc-usb-sys/src/v5_layout.rs:22-29`; `lib.rs:1387-1395,1564-1599`). A recognized HID device is resumed and must pass `USB_GetDescriptors`; on failure Rust suspends it and returns the descriptor error, matching C's HID preflight and cleanup (`usb.c:688-701`; Rust `lib.rs:1541-1557`). Vendor devices retain best-effort resume without HID descriptor preflight, matching C's vendor branch (`usb.c:685-688`; Rust `lib.rs:1541-1547`). Unknown IDs still fall through to the legacy OH0/OH1 path in both implementations (`usb.c:707-721`; Rust `lib.rs:1584-1599`).

The HID-only device-list policy is unchanged by `6776624`; it is governed by `use_legacy_list`, which selects C's OH0 fallback whenever the vendor host is absent (`src/v5_layout.rs:32-34`). C has the same early fallback on `ven_host == NULL`, even if `hid_host` exists (`usb.c:1390-1419`). When a vendor host exists, both implementations include vendor-host devices except for HID-only filtering and include HID-host devices only for class zero or HID (`usb.c:1421-1440`; Rust `lib.rs:3822-3859`). Thus the probe change does not introduce a host-selection or device-list regression. It does not alter this separate legacy fallback behavior.

### Regression coverage and validation

The new `v5_layout` tests cover skipping request 6 for HID when vendor v5 is valid, preserving the v4 rejection path when there is no vendor v5 host, requiring request 0 to return a valid version, and using request 0 for vendor hosts (`src/v5_layout.rs:65-100`). They test the extracted decision helper, not actual IOS host initialization, open/close cleanup, or HID descriptor preflight. Existing tests cover descriptor layout and device-list policy separately.

`cargo check -p ogc-usb-sys --target powerpc-unknown-linux-gnu` passed. `cargo check -p ogc-usb-sys --tests --target powerpc-unknown-linux-gnu` failed because `v5_layout.rs:66,76,86` uses `std::vec::Vec` in this no-std PowerPC test build without linking `std`. Test code did not compile; tests did not run. No hardware verification was performed.

## Independent re-review — mouse deinitialization cleanup

**Result: PASSED for the bounded fix; `ogc-usb` remains open.** Reviewer: `/root/reviewer_f`. Implementer: `/root/worker_c`. Commit: `e6885f0` (`Match USB mouse deinit failure cleanup`). The grouped `ogc-usb-events` crate has no source change in this commit.

C `USBMouse_Close` ignores `USB_CloseDevice`'s result, resets the descriptor, and `MOUSE_Deinit` continues through event flush, resource release, semaphore destruction, state clearing, and return 1 (`C-sources/libogc/libogc/usbmouse.c:154–161, 359–379`). Rust's `mouse_close_with` likewise ignores the close result and clears connection state (`crates/services/ogc-usb/src/lib.rs:718`); `cleanup_mouse_resources` clears queue/thread state, ignores semaphore-destroy errors, clears the semaphore handle and initialized flag (`:736–753`).

One adjacent mismatch remains: Rust returns early if `sem_post` or `join_thread` fails (`lib.rs:1069–1093`), while C ignores those errors and continues cleanup (`usbmouse.c:363–379`). The helper tests inject close and semaphore-destroy errors, but do not exercise public `deinit` or inject thread/semaphore-post failures (`lib.rs:1155–1227`). `cargo check -p ogc-usb --target powerpc-unknown-linux-gnu --lib` passed. The test check failed to compile on an existing unsafe call at `lib.rs:1261`; tests did not run. No hardware verification was performed.

### BUG FIX handoff: continue mouse cleanup after semaphore-post or thread-join errors

Implementer: `/root/worker_a`. Independent re-review is needed; the review thread handles it. This completes the bounded `MOUSE_Deinit` cleanup parity finding. Rust now ignores `sem_post` and `join_thread` errors, as C does, and continues to close the mouse, flush events, destroy and clear the semaphore, reset thread state, and clear the initialized flag. The helper only posts and joins when the mouse thread is running, matching C's control flow.

Added a regression test that injects failures from both operations and checks that both were called, cleanup completed, and initialized/thread/semaphore state was reset. `cargo check -p ogc-usb --target powerpc-unknown-linux-gnu --lib` passed. The target test compile did not pass because of the existing unsafe call to `try_close_device_async` at `crates/services/ogc-usb/src/lib.rs:1295`; the regression test did not run. No hardware verification was performed.

### Independent re-review of error-path cleanup (`0a225c6`)

**Result: PASS for the bounded cleanup fix.** Reviewer: `/root/reviewer_f`; implementer: `/root/worker_a`.

C ignores `LWP_SemPost` and `LWP_JoinThread` results, then continues close, queue flush, data release, semaphore destruction, state reset, and success return (`C-sources/libogc/libogc/usbmouse.c:359-379`). Rust now also ignores both results and continues through cleanup (`crates/services/ogc-usb/src/lib.rs:736-778,1082-1095`). The regression injects both errors and checks that both calls occur and state cleanup completes (`lib.rs:1227-1261`). Valid initialized-state handling matches; Rust skips post/join if the corresponding resource is null.

Validation: the library-only PowerPC check passed. The target test build failed on an existing unsafe call in the outer test, so the regression did not compile and no tests ran. Reviewer_f's diff check passed. No hardware verification was performed.

### BUG FIX handoff: cover HID v5 open descriptor preflight

Implementer: `/root/worker_a`. Independent reviewer: pending. The current `prepare_v5_device_for_open` behavior already matched C's HID v5 preflight on static comparison; this commit changes no open behavior. It extracts the existing decision and cleanup sequence into a focused helper so regression coverage can check it directly. HID open resumes first, validates descriptors, frees them on nonnegative completion, and suspends and returns the descriptor error on failure. Vendor v5 open remains best-effort resume without HID descriptor validation. This handoff covers only HID preflight behavior.

Added regressions for successful descriptor validation/free, descriptor failure/suspend/error propagation, and the vendor path's lack of HID descriptor calls. Added a test-only `std` import so the existing `v5_layout` unit tests compile in this no-std crate. `cargo check -p ogc-usb-sys --target powerpc-unknown-linux-gnu` and `cargo check -p ogc-usb-sys --tests --target powerpc-unknown-linux-gnu` passed. `cargo test -p ogc-usb-sys --target powerpc-unknown-linux-gnu --no-run` compiled the test code but failed at linking because the configured host linker rejects the PowerPC options. Tests did not run. No hardware verification was performed.

### BUG FIX handoff: convert every HID descriptor length that fits the caller buffer

Implementer: `worker_a`. Independent reviewer: pending. This fixes the low-level API mismatch confirmed in `docs/review/libusb-hid-descriptor-audit.md`: C `USB_GetHIDDescriptor` converts each subordinate descriptor's `wDescriptorLength` when its complete three-byte entry is within the caller's `size`; Rust converted only the first entry. The production conversion now walks complete entries in the same order and leaves an incomplete trailing entry unchanged. The safety contract now requires writable storage for all `size` bytes, which is the requirement of the underlying descriptor read.

Added production-helper regressions for multiple complete entries and a truncated final entry. `rustfmt --edition 2021 crates/services/ogc-usb-sys/src/lib.rs`, `cargo check -p ogc-usb-sys --tests --target powerpc-unknown-linux-gnu`, and `git diff --check` passed. The tests compiled but did not run. No Dolphin probe was added because a fabricated HID descriptor would not validate the hardware descriptor path, and no known Dolphin device exposes multiple subordinate descriptors for a real-device check. No hardware verification was performed.

### Independent re-review — NEEDS CHANGES

Reviewer: `/root/reviewer_b`. Implementer: `worker_a`. The conversion logic matches C for the bounded fix: both convert `bcdHID` and the first `wDescriptorLength`, then convert later complete three-byte entries while the count and caller-provided size allow it (`../../C-sources/libogc/libogc/usb.c:1067-1086`; `crates/services/ogc-usb-sys/src/lib.rs:2126-2170`). The Rust safety contract now correctly requires writable storage for all `size` bytes because the descriptor read copies that full length. The fixed byte fixtures cover multiple entries and a truncated final entry. I found no static parity defect in this conversion.

The handoff is not ready for sign-off because the regressions were only compiled and not run, and there is no DOL regression. The project requires tests runnable as DOLs in Dolphin. The conversion is a production helper that can be shared with a focused DOL probe using fixed descriptor bytes; no HID device or hardware transport is needed to verify the byte conversion. Keep hardware descriptor acquisition outside the probe claim. No tests were run by this reviewer, and no hardware verification was performed.

### Follow-up DOL coverage

Commit `87e2eeb` adds the requested fixed-input DOL coverage for both the descriptor conversion and HID v5 open preflight helpers. Dolphin 2603-415 passed all five checks with no failures. The probe calls production-shared helpers and makes no claim about IOS transport or hardware behavior. The independent reviewer should recheck the new probe and then record the bounded sign-off.


### Independent re-review — fixed-input HID DOL coverage

Result: **PASS for the bounded probe and shared-helper coverage**. Reviewer: `worker_c`. Implementer: `worker_a` (production-shared helpers in commits `6a0f860` and `218397a`; DOL coverage in `87e2eeb`). I inspected the probe, helper call paths, fixed expected bytes, and the C implementations. `convert_hid_descriptor_lengths` is the helper called by production `get_hid_descriptor`; it converts `bcdHID` and the first descriptor length, then advances through later three-byte subordinate entries only when the full entry fits the caller buffer. This matches C `USB_GetHIDDescriptor` (`../../C-sources/libogc/libogc/usb.c:1067-1086`). The probe uses three complete entries with independent expected big-endian bytes for the version and all lengths. Its expected bytes match the C conversions on the PowerPC target.

The probe calls the same `prepare_hid_v5_device_with` helper used by the production v5 open path. Its callbacks verify the order and results for best-effort resume followed by descriptor retrieval/free on success, and descriptor failure followed by suspension with the original error returned. These outcomes match C `USB_OpenDevice` (`usb.c:676-704`). The probe uses fixed deterministic results and does not claim IOS descriptor transport or real-device coverage. The HID helper is directly exercised, but the DOL does not cover a truncated trailing entry; the compiled unit regression covers that boundary.

I inspected the existing report at `target/dolphin-validation/1791173913-6293e07e0d73/hid_descriptor_probe/result.json`: status `pass`, five passes, zero failures, DOL SHA-256 `64b7ce82bee60e7f5ff6754797d7ad24cf9961c18d124f7749e67b8f26a035c8`. Its probe log contains the complete-entry conversion, HID success/free, and HID failure/suspend checks. I did not rerun the probe or tests. This pass covers only the fixed-input probe and its production-shared policy/conversion helpers; IOS transport, actual HID devices, and Wii hardware remain unverified.

### Second independent re-review — HID DOL and production-shared helpers

**Result: PASS for the bounded helper and DOL coverage.** Reviewer: `/root/reviewer_b`. Implementer: `worker_a` for the helper fixes and `worker_c` for DOL commit `87e2eeb`. The earlier DOL review above was written by `worker_c`, so it was not independent for that commit. Reviewer_b separately checked the fixed expected bytes, C conversion behavior, production helper call paths, preflight callback order, and the DOL assertions. No mismatch was found in the bounded scope. The DOL records five passes and zero failures on Dolphin 2603-415; it does not exercise IOS transport or hardware. The truncated-entry case remains covered only by a compiled, unrun PowerPC unit test. No hardware verification is claimed.

## 2026-10-06 source-status reconciliation

The named USB source blockers are closed in the current tree. V5 close callback return behavior, teardown callback order, HID v5 preflight, descriptor-length conversion, device-list fallback, unknown-handle close behavior, dynamic mouse event storage, and mouse deinitialization failure cleanup have fixes with bounded independent passes; the fixed-input HID behavior also has passing DOL evidence. Remaining IOS transport, real-device, and hotplug timing questions are runtime validation scope rather than confirmed source mismatches. `ogc-usb` and `ogc-usb-sys` move to `needs_re_review`.
