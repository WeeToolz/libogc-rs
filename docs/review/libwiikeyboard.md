# `wiikeyboard-sys` static review

Reviewer: `/root/reviewer_6`  
Implementer: `notOrrytrout` (latest relevant implementation commit `fb2e6a7`; related layout and lifecycle changes include `3c63c53`, `38e5565`, and `9a32ff7`)  
Review type: first-pass static comparison.  
Validation: no build or tests run, as instructed. No hardware verification was performed.

## Scope and source paths

Rust implementation: `crates/compatibility/wiikeyboard-sys/src/lib.rs`.  
C public interfaces: `../../C-sources/libogc/gc/wiikeyboard/keyboard.h` and `usbkeyboard.h`.  
C behavior: `../../C-sources/libogc/libwiikeyboard/keyboard.c`, `usbkeyboard.c`, and `ukbdmap.c`.

The Rust public functions are Rust APIs rather than exported C symbols. The event structs use `repr(C)` and callback aliases use the C ABI after `fb2e6a7`; the event field layout and enum numeric values match the corresponding C declarations in the checked areas. The review found confirmed behavior mismatches below, so the crate remains open.

## Confirmed findings

### Keyboard initialization does not start the C discovery and polling loop

C `KEYBOARD_Init` initializes USB and the keyboard heap, loads the keymap, creates `_kbd_thread`, and optionally creates the stdin buffer thread (`../../C-sources/libogc/libwiikeyboard/keyboard.c:481-602`). `_kbd_thread_func` scans for an attached keyboard and calls `USBKeyboard_Scan` every 10 ms, retrying device discovery periodically (`:407-424`; constants at `:52-55`).

Rust `init` initializes USB if needed, selects a layout, marks the keyboard initialized, and installs stdin (`crates/compatibility/wiikeyboard-sys/src/lib.rs:1231-1278`). It does not start a poller or discover a device. `open` and `scan` are separate operations that a caller must invoke (`:962-1002`, `:1069-1177`), and no crate-internal loop invokes them. Thus calling the Rust high-level `init` does not provide the C behavior of automatic discovery and ongoing scan/event generation. This is an observable lifecycle and hotplug difference.

### Lock keys do not update modifiers or LEDs

C maps Caps Lock, Hold Screen/Scroll Lock, and Num Lock to keycodes 57, 71, and 83 (`../../C-sources/libogc/libwiikeyboard/ukbdmap.c:104`, `:117-119`, `:130`). `keyboard.c` examines the mapped keysym and toggles `MOD_CAPSLOCK`, `MOD_HOLDSCREEN`, and `MOD_NUMLOCK`, then updates the corresponding USB LED (`:214-268`).

Rust `modifier_mask_for_keycode` handles only HID modifier keycodes 224-231 and returns zero for lock keys (`crates/compatibility/wiikeyboard-sys/src/lib.rs:284-295`). `submit_usb_event` only updates modifier state when this helper returns a nonzero mask (`:586-608`); there is no lock-key toggle or automatic LED update. Caps Lock therefore does not affect subsequent letter translation, and all three lock LEDs remain unchanged after lock-key presses. This differs from C.

### Symbol translation omits C function-key symbols and changes Return

C's US keyboard map maps HID keycode 58 to `KS_Cmd_Screen0`/`KS_f1`, and keycodes 59-69 to the remaining F-key pairs (`../../C-sources/libogc/libwiikeyboard/ukbdmap.c:105-116`). The C event path handles `KS_GROUP_Function` and stores the symbol in the event (`../../C-sources/libogc/libwiikeyboard/keyboard.c:338-370`). Rust's `US_LAYOUT` covers only keycodes 4-56 (`crates/compatibility/wiikeyboard-sys/src/lib.rs:298-352`), `translate_symbol` returns zero for unlisted codes (`:354-383`), and `submit_usb_event` emits the resulting zero symbol for function-key presses (`:610-632`). This loses C's function-key symbols and makes the stdin callback skip them (`:633-638`).

There is also a direct printable-symbol mismatch: C defines `KS_Return` as `0x0d` (`../../C-sources/libogc/gc/wiikeyboard/wsksymdef.h:55`) and maps HID code 40 to it (`../../C-sources/libogc/libwiikeyboard/ukbdmap.c:87`), while Rust maps keycode 40 to line feed `0x0a` (`crates/compatibility/wiikeyboard-sys/src/lib.rs:335`). The resulting event and stdin byte differ.

### Rust silently drops events after its fixed ring fills

C `_kbd_addEvent` allocates a node and appends it to the LWP queue (`../../C-sources/libogc/libwiikeyboard/keyboard.c:154-161`); the queue has no fixed 31-event capacity. Rust's event ring has 32 slots but reserves one slot to distinguish full from empty, so it stores at most 31 events (`crates/compatibility/wiikeyboard-sys/src/lib.rs:47`, `:170-181`). `submit_usb_event` ignores `queue_push`'s `false` result (`:627-632`), and `push_connected_event` does the same (`:641-649`). When consumers do not drain quickly enough, Rust silently loses keyboard events that C retains in its dynamically allocated queue, until allocation fails.

### Resolved in later parity pass: USB disconnect callback no longer depends on keyboard-layer initialization

C `USBKeyboard_Open` registers the USB removal callback (`../../C-sources/libogc/libwiikeyboard/usbkeyboard.c:375-379`). On removal, `_disconnect` always calls `_submit(USBKEYBOARD_DISCONNECTED, 0)` (`:105-115`), and `_submit` invokes the callback whenever it exists (`:93-103`).

Rust `powerpc_disconnect_callback` now always calls `submit_usb_event`. That function invokes the USB-layer callback before its high-level `KEYBOARD_INITIALIZED` gate, so callers that opened the USB keyboard layer without `KEYBOARD_Init` now receive the disconnect callback as C does. High-level queued keyboard events remain gated by keyboard initialization.

## Existing regression coverage

The source contains PowerPC-only tests for ABI shapes, selected layout characters, host scan behavior, event dequeue/flush, and LEDs (`crates/compatibility/wiikeyboard-sys/src/lib.rs:1459-1932`). Before this fix, it had no coverage for function-key keysyms, Return's CR value, lock-key state/LED behavior, event-ring overflow, automatic discovery/polling, or USB-layer-only disconnect callbacks.

### Return-key parity fix

**BUG FIX.** C defines `KS_Return` as `0x0d` and maps HID keycode 40 to that symbol (`../../C-sources/libogc/gc/wiikeyboard/wsksymdef.h:55`; `../../C-sources/libogc/libwiikeyboard/ukbdmap.c:87`). Rust previously mapped the key to `0x0a`. `/root/worker_a` changed the US fallback mapping to `0x0d` and added `return_key_matches_libogc_carriage_return_symbol`, which checks unshifted and shifted translation for every selected layout. Independent re-review by `/root/reviewer_f` passed this fix; evidence and validation are below. Other keyboard findings remain open.

## Review state

`wiikeyboard-sys` remains `pending,finding`, owned by `/root/reviewer_6`. Confirmed behavior differences are listed above. No implementation edits were made.


## Later parity pass: USB-layer disconnect callback

The PowerPC removal callback no longer suppresses the USB keyboard callback when the higher-level keyboard layer has not been initialized. This is a direct source-level alignment with C `_disconnect`/`_submit`. Build and hardware validation remain pending. Other findings in this review remain open.

## 2026-10-02 polling and lock-key parity fix

Compared with current `libwiikeyboard/keyboard.c`, `usbkeyboard.c`, and `ukbdmap.c`.

- High-level `init` now starts one PowerPC LWP that scans every 10 ms and retries keyboard discovery every 300 polls, matching C's discovery/poll cadence. `deinit` signals and joins that worker before USB shutdown.
- Caps Lock (57), Scroll Lock/Hold Screen (71), and Num Lock (83) now toggle the matching modifier state on key press and drive the matching USB keyboard LED. The lock transition is centralized in one helper and has focused regression coverage.
- Caps Lock therefore participates in the existing letter translation path, as it does in C.

Those historical function-key and fixed-event-queue findings are superseded by the follow-ups below. Hardware hotplug/poll timing remains unverified.

## 2026-10-02 remaining keyboard parity follow-up

The two remaining concrete keyboard findings are addressed in the current source.

- HID F1-F12 keycodes now select C's `KS_Cmd_Screen0..KS_Cmd_Screen11` symbols when unshifted and `KS_f1..KS_f12` when shifted; the separate public `KS_F1..KS_F20` constant range remains available.
- the fixed 32-slot/31-event ring was replaced by a dynamically growing FIFO, so the keyboard layer no longer silently drops events when the old ring fills. A regression queues 64 events and verifies FIFO delivery beyond the old limit.
- the earlier automatic polling and lock-key work was rechecked after this change: Caps/Scroll/Num lock still toggle C's modifier bits on press and update the corresponding USB LED; the poll worker remains the sole automatic scan/discovery loop.

Allocation failure for the dynamic event queue follows the Rust allocator's global OOM behavior rather than silently losing input. Hardware keyboard timing remains unverified.


## 2026-10-02 compiled-list verification and stdin buffering follow-up

The compiled baseline confirms the earlier function-key, automatic-polling, and lock-key changes. One secondary fixed buffer remained after that work: the stdin bridge still used a 256-byte ring and returned without enqueueing when full. C also has a 256-byte stdin ring, but its buffer thread does not remove the next dynamically allocated keyboard event until that ring has space, so the event is retained rather than silently lost.

Rust now uses the same generic locked dynamic FIFO abstraction for both public keyboard events and stdin bytes. The old fixed stdin ring/head/tail storage is removed, so neither Rust keyboard queue silently drops input because of a fixed queue capacity. A regression queues all 256 byte values before reading any back and verifies FIFO delivery. The automatic polling and Caps/Scroll/Num-lock modifier/LED behavior remain unchanged.

## Independent re-review: Return-key parity fix

**Result: PASSED** for commit `ca80d33` (`Match keyboard Return symbol to libogc`). Reviewer: `/root/reviewer_f`. Implementer: `/root/worker_a`. This result covers the HID Return key mapping only; the separate confirmed findings above remain open.

- C defines `KS_Return` as `0x0d` (`../../C-sources/libogc/gc/wiikeyboard/wsksymdef.h:51-56`) and maps HID keycode 40 to it (`../../C-sources/libogc/libwiikeyboard/ukbdmap.c:87`). The locale keymaps build on the US map (`ukbdmap.c:1075-1087`), and no locale-specific Return override was found. Therefore the C mapping is carriage return in the inspected layouts.
- The reviewed commit changes only the US fallback pair for keycode 40 from line feed to `0x000d` (`crates/compatibility/wiikeyboard-sys/src/lib.rs:390-395`). `translate_symbol` first checks locale-specific pairs, then falls back to `US_LAYOUT`; no locale table overrides keycode 40 (`:420-453,455-580`). The value is unchanged by Shift because both entries are `0x000d`, as in C's single-symbol mapping for this key.
- Rust stores the translated symbol in the pressed `KeyboardEvent` (`lib.rs:704-726`) and, when stdin is enabled, queues its low byte for `keyboard_stdin_read` to return and then notify the registered keypress callback (`:266-289,727-731`). C stores the translated `ksym` in the event (`../../C-sources/libogc/libwiikeyboard/keyboard.c:338-370`); the stdin buffer thread queues the pressed event's `symbol` (`:433-446`), and `_keyboardRead` returns that byte and invokes the keypress callback (`:449-460`). For Return, both paths now expose byte `0x0d` through event and stdin paths.
- The added regression checks shifted and unshifted Return translation for all seven Rust-selected layouts (`lib.rs:1703-1710`). It checks translation directly; it does not exercise event queue or stdin delivery end to end.

Validation: `cargo check -p wiikeyboard-sys --target powerpc-unknown-linux-gnu` passed. `cargo test -p wiikeyboard-sys --no-run --target powerpc-unknown-linux-gnu` did not compile the test crate: compilation stopped on ten unsafe-call errors at unrelated existing tests (`open`, `get_event`, and `init` calls around lines 2056-2153). The Return regression test did not run. No hardware verification was performed.

## Independent re-review of broader keyboard follow-ups

**Result: NEEDS_CHANGES** for the broader follow-up set. Reviewer: `/root/reviewer_f`. The relevant implementation commits `4aa6399`, `966aef9`, `59a3105`, and `4c36294` list `notOrrytrout` as author/committer in git metadata. The earlier Return fix `ca80d33` remains **PASSED** separately above. The polling and lock-state work is partially correct, and the dynamic queues remove their former fixed-capacity drop behavior, but the current C comparison leaves the following mismatches.

### Passed portions of the follow-up

- Rust starts a polling thread from `init`, checks for a keyboard at startup and then every 300 iterations, scans every loop, sleeps for approximately 10 ms, and joins the worker during `deinit` (`crates/compatibility/wiikeyboard-sys/src/lib.rs:1322-1374,1380-1409,1432-1438`). This matches C `_kbd_thread_func`'s immediate discovery, 300-turn retry, scan, and 10 ms delay (`../../C-sources/libogc/libwiikeyboard/keyboard.c:407-424`, constants at `:52-55`), and C deinit's join-before-close order (`:605-621`).
- Lock transitions use the correct keycodes, modifier bits, and LED indices; a press toggles the bit and writes the resulting LED state (`lib.rs:642-655,686-702`). C toggles Caps/Num/Hold-Screen on press and sets the corresponding LED state (`keyboard.c:227-231,257-267`; `update_modifier` at `:164-174`).
- The public event queue and stdin byte queue are now dynamically growing `VecDeque`s, so their old 31-event and 255-byte ring limits no longer silently discard entries (`lib.rs:200-260`). The added tests exercise FIFO retention beyond those former bounds (`:1672-1701`). C's public event queue is also dynamically allocated (`keyboard.c:155-161`). These tests exercise the queue helpers only, not the full scan/callback concurrency path.

### Confirmed mismatches and open findings

1. **Function-key symbols ignored the C keymap's shifted and unshifted values; fix awaits independent re-review.** C maps HID 58 to `KS_Cmd_Screen0` when unshifted and `KS_f1` when shifted, with corresponding pairs through keycode 69 (`../../C-sources/libogc/libwiikeyboard/ukbdmap.c:105-116`). The C translation selects `group[0]` or `group[1]` from the active Shift state and copies the selected symbol into the event (`keyboard.c:270-290,338-370`). Before the fix, Rust's `function_key_symbol` ignored modifiers and always returned uppercase `KS_F1..KS_F12` (`crates/compatibility/wiikeyboard-sys/src/lib.rs:412-423`), so keycode 58 returned `0xf340` in both states instead of C's unshifted command value `0xf400` or shifted `KS_f1 == 0xf300` (`wsksymdef.h:554-585,575-586,630-641`). The earlier regression test covered only unshifted input. This affected the public event symbol and the low byte queued for stdin (`:704-731`). The fix and new coverage are recorded below.
2. **Modifier-key event symbols are lost, including the lock symbols whose state was fixed.** C's US map defines Caps Lock, Hold Screen, and Num Lock (`ukbdmap.c:104,117-130`); `keyboard.c` writes the chosen keysym into `event.symbol` after modifier handling (`:211-230,257-268,270-290,367-370`). Rust has no mappings for keycodes 57, 71, 83, or the modifier keycodes in `translate_symbol`; the US table ends at 56 and the only later special mappings are F1-F12, so the resulting `KeyboardEvent.symbol` is zero (`lib.rs:385-423,721-726`). For example, C emits `KS_Caps_Lock` (`wsksymdef.h:488`) while Rust emits zero for keycode 57. Lock state and LED transitions now match on press, but the event symbol and corresponding stdin byte do not.
3. **The Rust stdin bridge does not consume events from the public event FIFO as C does.** When C `KEYBOARD_Init` receives a keypress callback, it creates `_kbd_buf_thread` (`keyboard.c:576-599`). That worker removes each next event from the same `_queue` via `KEYBOARD_GetEvent`, subject to stdin-buffer space, and enqueues only pressed symbols (`:433-446`). Rust instead enqueues a public `KeyboardEvent` and independently pushes the pressed symbol to `STDIN_KEY_QUEUE` from `submit_usb_event` (`lib.rs:721-731`); the stdin reader drains only that second queue (`:266-289`). With stdin enabled, Rust therefore lets `get_event` observe events that C's stdin worker consumes, and it has different event ordering/competition semantics. The queue-growth tests do not cover this behavior.
4. **Keyboard discovery omits C's connection LED flash.** After a successful open, C turns on the Num/Caps/Scroll LEDs, waits 200 ms, turns them off, resets modifier/held state, and queues a connected event (`keyboard.c:375-404`). Rust's `powerpc_find_keyboard` resets state and emits a connected event, but it does not perform the LED on/off sequence (`lib.rs:980-1006`). This changes the visible connection behavior and delays the first scan differently. No regression covers the startup feedback sequence.
5. **A failed poll-thread creation can leave stdin replaced when C would not install it.** C installs `std_in` only after the polling thread and optional buffer thread have both been created successfully (`keyboard.c:560-600`). Rust replaces stdin before `start_keyboard_thread` and returns on a thread-creation error after clearing its callback/init flag, without restoring the previous stdin device (`lib.rs:1393-1407`). A failing `create_thread` can therefore leave `STDIN_DEVICE` installed with no keyboard worker; C returns `-6` before changing stdin. No regression injects this failure.
6. **`deinit` shuts down the shared USB service although C's keyboard deinitializer does not.** C `KEYBOARD_Deinit` calls `USBKeyboard_Close`, flushes events, then calls `USBKeyboard_Deinitialize` (`keyboard.c:605-629`); that USB-keyboard deinitializer is a no-op returning `IPC_OK` (`../../C-sources/libogc/libwiikeyboard/usbkeyboard.c:218-221`) and does not call `USB_Deinitialize`. Rust `deinit` calls `deinitialize` (`lib.rs:1436-1455`), which calls `usb::deinitialize()` and clears `USB_INITIALIZED` on success (`:1030-1040`). The Rust API thus tears down the shared USB service as part of keyboard teardown, unlike this C path. Commit `4c36294` makes the public result return `1` as C does but does not make the underlying teardown behavior match. No regression covers another USB client across keyboard deinitialization.

### Validation and limits

`cargo check -p wiikeyboard-sys --target powerpc-unknown-linux-gnu` passed on current source. `cargo test -p wiikeyboard-sys --no-run --target powerpc-unknown-linux-gnu` failed to compile the test crate with ten unsafe-call errors in existing tests at `lib.rs:2056-2153`; no tests ran or compiled as a test binary. Thus none of the newly added regression tests were executed here. No hardware verification was performed. These findings keep `wiikeyboard-sys` open for implementation and another independent review.

## Fix handoff: shifted and unshifted function-key symbols

**BUG FIX.** Implementer: `worker_a`. Independent re-review is needed; the review thread will handle it. Other keyboard findings remain open.

Rust now selects the unshifted `KS_Cmd_Screen0..KS_Cmd_Screen11` range for HID keycodes 58–69 and the shifted `KS_f1..KS_f12` range when either Shift key or Shift Lock is active. Caps Lock alone does not change the command symbol, matching C's `MOD_ANYSHIFT` selection in `keyboard.c`. The updated regression checks all 12 keycodes with left Shift, right Shift, Shift Lock, Caps Lock alone, and no modifiers.

Validation: `cargo check -p wiikeyboard-sys --target powerpc-unknown-linux-gnu` passed. `cargo test -p wiikeyboard-sys --no-run --target powerpc-unknown-linux-gnu` did not compile because of ten existing unsafe-call errors in other tests (`lib.rs:2082-2179`); therefore no tests ran. No hardware verification was performed.

## Modifier and lock-key event symbols

**BUG FIX.** Implementer: `/root/worker_c`. Commit: `b7ecb49`. Independent re-review is needed; the review thread will handle it.

Rust now emits the modifier and lock keysyms from the C US keymap for HID codes 57, 71, 83, and 224–231. For the left/right Control and Alt positions, it selects the C keymap's command symbol without Shift and the Control/Alt symbol with Shift. Caps Lock and left Shift event tests check the public queued `KeyboardEvent.symbol` and modifier state. C source: `../../C-sources/libogc/libwiikeyboard/ukbdmap.c:104,117-130,162-169`; symbol values: `../../C-sources/libogc/gc/wiikeyboard/wsksymdef.h:484-500`.

Validation: `cargo check -p wiikeyboard-sys --target powerpc-unknown-linux-gnu` passed. `cargo fmt -p wiikeyboard-sys -- --check` passed after formatting. `cargo test -p wiikeyboard-sys --no-run --target powerpc-unknown-linux-gnu` did not compile because of ten existing unsafe-call errors in other tests (`lib.rs:2139-2236`); therefore no tests ran. No hardware verification was performed.

## Shared USB lifecycle handoff

**BUG FIX.** Implementer: `/root/worker_c`. Independent re-review is needed; the review thread will handle it.

`KEYBOARD_Deinit` must close the keyboard and clear keyboard state without deinitializing the process-wide USB service. Rust `deinit` no longer calls the public USB `deinitialize` path after `try_close`. A PowerPC regression initializes the keyboard and checks that the shared initialized state remains set after keyboard teardown. The keyboard Dolphin probe also calls `usb::device_list` after keyboard deinit and records whether the real USB service remains available.

Build/run coverage: `cargo xtask build-dolphin-probes` passed. `cargo xtask dolphin-validate --no-build --probe keyboard_probe` passed under Dolphin 2603-415; the probe reported `PASS usb service available after keyboard deinit` and 13 total passes with 0 failures. `cargo fmt -p wiikeyboard-sys -- --check` and `cargo check -p wiikeyboard-sys --target powerpc-unknown-linux-gnu` passed. `cargo test -p wiikeyboard-sys --no-run --target powerpc-unknown-linux-gnu` compiled the test crate but could not link because the installed `ld` does not support the emitted GNU linker options; tests did not run. No hardware verification is claimed.

## Keyboard close failure cleanup follow-up

The previous fix validated the shared USB core service. It did not test close-failure cleanup, and its probe claim did not identify that scope precisely. The probe now labels its check as a USB core device-list check; it does not prove that the USB v5 service remains active.

PowerPC `try_close` now clears keyboard callbacks, connection state, LEDs, and scan state even when closing the device handle fails. It preserves the fd if the backend leaves it open, so a caller can retry. `deinit` completes the local cleanup and returns the close error, with that error also available through `last_status`, instead of reporting the C constant success result. C's `USBKeyboard_Close` is `void` and discards clear-halt/close results; `KEYBOARD_Deinit` returns 1 regardless (`../../C-sources/libogc/libwiikeyboard/usbkeyboard.c:387-399`, `keyboard.c:606-629`). The Rust error result follows its public error-reporting contract while retaining the C cleanup order.

A PowerPC regression injects a close failure at the production-shared close helper and verifies the error, retained fd, and cleared keyboard state. `cargo fmt -p wiikeyboard-sys -- --check`, `cargo check -p wiikeyboard-sys --target powerpc-unknown-linux-gnu`, and `cargo xtask build-dolphin-probes` passed. `cargo xtask dolphin-validate --no-build --probe keyboard_probe` passed under Dolphin 2603-415 with 13 passes and 0 failures, including `PASS usb core device list works after keyboard deinit`. The test target compiled but did not link: the installed `ld` rejects the generated GNU linker options, so the Rust regression did not run. The Dolphin probe checks the USB core device-list operation only; no v5 assertion or hardware verification is claimed. Independent review is needed.

### Independent re-review — PASSED

Reviewer: `/root/worker_b`.

The close helper calls `clear_halt` and then attempts `close_device_handle` even if clearing the halt fails. It clears keyboard callbacks, connection flags, LEDs, and scan state after either result. When close fails, the USB backend leaves the fd unchanged (`../../C-sources/libogc/libogc/usb.c:750-760`); Rust retains it for a later `try_close` or `open` retry. An `open` attempt checks `try_close` first and does not acquire a replacement handle while that close still fails (`crates/compatibility/wiikeyboard-sys/src/lib.rs:1129-1142`). `deinit` stops and joins its worker before cleanup and completes event and callback cleanup even when close reports an error. This preserves the cleanup order in C `KEYBOARD_Deinit` (`../../C-sources/libogc/libwiikeyboard/keyboard.c:606-629`) and C `USBKeyboard_Close`'s clear-halt-then-close order (`usbkeyboard.c:387-399`). Rust's error return is an intentional reporting extension; C discards both close results and returns 1.

The DOL probe only demonstrates that `usb::device_list` returns successfully after keyboard deinit. It does not exercise the close-failure branch, prove USB v5 availability, or verify hardware behavior. I rebuilt and ran it under Dolphin 2603-415: 13 passes, 0 failures; DOL SHA-256 `d268eaa675ad3ce9889579e221ae5ee341c82860cf7d1a4193f6f8234314c653`. `cargo fmt -p wiikeyboard-sys -- --check` and `cargo check -p wiikeyboard-sys --tests --target powerpc-unknown-linux-gnu` passed. The test target did not link because the installed linker rejects GNU options, so the regression did not run. No hardware verification was performed.

## BUG FIX handoff — Install stdin only after worker creation succeeds

Implementer: `/root/worker_b`. Independent review: needed; coordinator owns the ledger handoff.

C installs its stdin device only after successful polling-thread creation (and, when configured, successful buffer-thread creation) (`../../C-sources/libogc/libwiikeyboard/keyboard.c:560-600`). Rust previously replaced stdin before starting its polling thread, so a thread-creation error returned `-6` while leaving the keyboard stdin device installed. Rust now starts the worker first and runs the stdin installation only after the worker reports success. The shared startup helper returns the original create error and skips the install closure on failure; the prior stdin device therefore remains active. Normal stdin setup still runs only when a keypress callback is configured.

A regression injects a `-6` worker-creation result through the production-shared startup helper and checks that the previous stdin device value remains unchanged. The keyboard DOL probe calls that same helper with a controlled failure result and checks that its stdin installer does not run. This exercises the post-create decision and cleanup ordering; it does not cause the Dolphin LWP implementation itself to fail. `cargo fmt -p wiikeyboard-sys`, probe formatting, `cargo check -p wiikeyboard-sys --tests --target powerpc-unknown-linux-gnu`, and `cargo xtask build-dolphin-probes` passed. `cargo xtask dolphin-validate --no-build --probe keyboard_probe` passed under Dolphin 2603-415 with 14 passes and 0 failures; DOL SHA-256 is `97742109bb2d3444974126eabc3bd00fbbcff0ab030e0b26394c265d1a718743`. PowerPC tests compiled but did not run because the test binary does not link with the installed linker; see the independent review above for the linker error. No hardware verification was performed. The shared ledger is managed by the coordinator.

## Independent re-review — stdin install order (`6293e07`, `591dc4c`)

**Result: PASS for the bounded worker-creation failure fix.** Reviewer: `worker_a`. Implementer: `worker_b`.

C calls `LWP_CreateThread` before it assigns `devoptab_list[STD_IN]` to `std_in`; if poll-thread creation fails, `KEYBOARD_Init` closes the keyboard and returns `-6` without replacing stdin (`../../C-sources/libogc/libwiikeyboard/keyboard.c:560–600`). Rust's `init` calls `start_worker_before_stdin_install(start_keyboard_thread, ...)`, and the helper calls the stdin-install closure only when the worker returns zero (`crates/compatibility/wiikeyboard-sys/src/lib.rs:1425–1467,1491–1510`). The production `start_keyboard_thread` maps a failed `lwp::create_thread` result to `-6` (`:1425–1448`). Therefore this failure path skips `set_stdin_device` and preserves the prior stdin device. The PowerPC regression injects `-6` into this same helper and checks that its install closure does not change a sentinel prior device value.

The probe calls the same production-shared helper with a controlled `-6`; its assertion that the prior sentinel remains unchanged matches C's no-install behavior. It does not force Dolphin's LWP thread creation to fail, so it validates the post-create decision, not the real LWP failure mechanism. The helper does not test C's optional second buffer-thread creation; Rust has no separate stdin buffer thread in this path, and that broader stdin architecture difference remains outside this fix.

I inspected both complete implementation diffs and the probe. The recorded `cargo check -p wiikeyboard-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled tests, but the tests did not run because the target test binary did not link with the installed linker. The recorded Dolphin 2603-415 run passed 14 checks with 0 failures. No checks or tests were rerun during this review. No hardware verification was performed.

## 2026-10-05 full-scope classification

`wiikeyboard-sys` is now `parity_blocked`. Two current non-safety differences remain after the accepted bounded fixes. First, C's stdin worker consumes the same public `KEYBOARD_GetEvent` FIFO, while Rust mirrors decoded symbols into a separate stdin queue and leaves the public event queued; this changes event competition and ordering when stdin is enabled. Second, C flashes all three keyboard LEDs, waits 200 ms, clears them, and only then publishes the connected event after a successful open; current Rust omits that observable sequence. No additional C source is required to fix these findings.


## 2026-10-05 final stdin and connection-order parity pass

The two non-safety findings from the full-scope classification are now fixed in production code. When stdin integration is enabled, the PowerPC keyboard worker consumes the same public keyboard-event FIFO used by `KEYBOARD_GetEvent`, matching C's `_kbd_buf_thread_func`. It consumes at most one event per 10 ms poll while the stdin buffer has room, discards non-pressed events after consuming them, and buffers pressed symbols. The stdin buffer now applies C's 255-byte effective ring capacity and stops consuming the public FIFO while full, preserving C's backpressure instead of allowing an unbounded private mirror queue. The read callback still runs when a buffered byte is consumed, matching `_keyboardRead`.

After a successful PowerPC keyboard open, Rust now performs C's observable LED sequence before publishing `KEYBOARD_CONNECTED`: all Num/Caps/Scroll LEDs on, wait 200 ms, all three off, then enqueue the connected event. LED transport failures are deliberately ignored in this sequence because C ignores those `USBKeyboard_SetLed` results.

No confirmed non-safety parity blocker remains in the recorded `wiikeyboard-sys` scope. The crate moves to `needs_re_review` pending PowerPC compilation and independent validation of event competition/backpressure and connection-event ordering.
