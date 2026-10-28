# `wpad-sys` static review

Reviewer: `/root/reviewer_6`
Implementer: `notOrrytrout`; latest relevant WPAD behavior commits `1ef870b` (button event coalescing), `a61a289` (scan event count), and `e0cc13b` (chunked audio streaming).
Review type: independent first-pass source audit followed by an implementation handoff.
Implementer for the fixes below: `/root/worker_a`. Independent re-review is assigned to `/root/reviewer_6`.
Validation: PowerPC production and test-code checks passed. A PowerPC `cargo test --no-run` attempt reached linking but the host linker rejected the PowerPC test binary options, so tests could not be executed. `cargo fmt --check` reports formatting differences in existing code in both touched source files. No hardware verification was performed.

## Scope and sources

Rust: `crates/services/wpad-sys/src/lib.rs`, `crates/services/wpad-sys/src/powerpc_backend.rs`, and `crates/services/wpad-sys/src/wii_remote.rs`. This review covers public channel validation, MotionPlus all-channel behavior, backend slot count, and probe readiness.

C: `../../C-sources/libogc/gc/wiiuse/wpad.h`, `../../C-sources/libogc/wiiuse/wpad.c`, and `../../C-sources/libogc/lwbt/bte.c` for HID channel closure behavior.

## Findings fixed; assigned for independent review

### Channel 5 was rejected although C treats it as a valid channel

C defines `WPAD_CHAN_UNKNOWN` as value 5 and `WPAD_MAX_DEVICES` as 6 (`gc/wiiuse/wpad.h:39-48`). Its per-channel APIs accept indices from zero through five by checking `chan >= WPAD_MAX_DEVICES`; for example `WPAD_SetMotionPlus` uses that range check (`wiiuse/wpad.c:1541-1555`). Before the fix, Rust defined six-channel constants and storage (`lib.rs:80-89`) but `valid_chan` accepted only channels 0 through `WPAD_BALANCE_BOARD` (0 through 4) (`lib.rs:614-620`). APIs using this helper therefore returned `WPAD_ERR_BAD_CHANNEL` for channel 5. The fix changes validation to the C range, zero through `WPAD_MAX_DEVICES - 1`.

### `WPAD_SetMotionPlus(WPAD_CHAN_ALL, ...)` did not update all channels

C explicitly handles `WPAD_CHAN_ALL` by iterating from channel zero to `WPAD_MAX_DEVICES - 1` and forwarding the operation to each channel (`wiiuse/wpad.c:1541-1552`). Before the fix, Rust's public `set_motion_plus` called `valid_chan` before dispatching to the PowerPC backend and had no all-channel branch (`lib.rs:1070-1081`). Since `WPAD_CHAN_ALL` is `-1` (`lib.rs:80-88`), it returned `WPAD_ERR_BAD_CHANNEL`. The fix dispatches across all six channels and skips empty PowerPC slots, as C does when the channel entry is null.

### PowerPC backend exposed four slots where C allocates six

C declares its WPAD state arrays using `WPAD_MAX_DEVICES` and initializes the Wiiuse library with that same six-device count (`wiiuse/wpad.c:93-95`, `:1288`). Before the fix, the PowerPC backend set `MAX_POWERPC_SLOTS` to four despite `WPAD_MAX_DEVICES` being six, and sized controller state from that smaller value. The fix sizes backend state from `WPAD_MAX_DEVICES`; it also changes the stream-timer token layout so its slot encoding still works for six slots.

### `WPAD_Probe` reported success before the Wii Remote handshake completed

C `WPAD_Probe` first requires WPAD initialization, then returns `WPAD_ERR_NOT_READY` for a connected remote whose handshake is incomplete (`wiiuse/wpad.c:1609-1645`). Before the fix, the PowerPC backend set the public connected flag as soon as the HID connection event arrived, then began Wii Remote initialization (`powerpc_backend.rs:1305-1308`). The public `probe` checked only `WPAD_CONNECTED`, so it could return success before calibration completed. The fix checks global initialization and a handshake-complete flag set when the initial calibration read completes.

## Review of prior findings and relevant fixes

The two channel findings previously recorded in `docs/review/libogc.md` are addressed by this fix. The commits `1ef870b`, `a61a289`, and `e0cc13b` do not cover channel validation, MotionPlus all-channel dispatch, slot capacity, or probe readiness.

## Coverage and limitations

The read-only audit covers the four behaviors listed above. The wider WPAD implementation, including full event decoding, IR, speaker commands, lifecycle, and callback ordering, remains outside scope. The implementation checks and hardware boundary are recorded in the fix handoff below.

## Fix handoff

Implementer: `/root/worker_a`. Independent reviewer: `/root/reviewer_6` (review pending).

- `valid_chan` now accepts `0..WPAD_MAX_DEVICES`, including channel 5, consistent with C's range checks.
- The PowerPC controller state arrays now use all six `WPAD_MAX_DEVICES` slots. The audio-stream timer token now reserves three low bits for the slot index, so slots 4 and 5 decode correctly; the former two-bit mask only worked for four slots.
- `WPAD_SetMotionPlus(WPAD_CHAN_ALL, ...)` now dispatches across all six channels in order. Empty PowerPC slots are skipped, as C does when the corresponding `__wpads[chan]` entry is null; the first operation error from an active Wii Remote is returned.
- The PowerPC `WPAD_Probe` path now returns `WPAD_ERR_NOT_READY` while WPAD is disabled or a connected Wii Remote has not completed its initial calibration read. The handshake-complete flag is set only when that read completes, matching C's `WIIMOTE_STATE_HANDSHAKE_COMPLETE` transition.
- The BT host now handles individual control and interrupt channel closure events. It retains WPAD connection state after the first channel closes and clears the slot after both channels close, matching libogc BTE's disconnect callback after both L2CAP PCBs close (`lwbt/bte.c:1059-1081`).

Regression checks cover channel 5 acceptance and out-of-range rejection, all-six-channel dispatch order, six backend slots, audio timer-token round trips for channel 5, probe status before/after handshake, and waiting for both HID channels to close. `cargo check -p wpad-sys --target powerpc-unknown-linux-gnu` and the same command with `--tests` both passed. `cargo test -p wpad-sys --target powerpc-unknown-linux-gnu --no-run` did not complete because the host linker rejected PowerPC linker options. Tests were not run. `cargo fmt -p wpad-sys -- --check` reports formatting differences in existing code in the touched source files. No hardware verification was performed.

## Review state

The four WPAD findings above are fixed and await independent static review. No unrelated WPAD behavior was intentionally changed. The broader WPAD scope listed in the original review remains outside this handoff.

## 2026-10-02 current-source verification

The requested channel/probe parity was rechecked against the current C tree before this parity batch. No new source patch was needed:

- `WPAD_MAX_DEVICES` is six and PowerPC slot storage uses all six entries.
- channel 5 is accepted by the common channel validator.
- all-channel operations iterate all six C-visible channels.
- PowerPC `WPAD_Probe` requires the initial Wii Remote handshake/calibration completion before reporting the controller ready.

The existing regression checks cover these behaviors. This is source verification only; hardware validation remains pending.

## 2026-10-02 deeper initial-handshake/report parity

Compared with current `wiiuse/io.c`, `wiiuse/ir.c`, and `wiiuse_internal.h`.

This section records the earlier implementation handoff summary. The independent comparison below refines it against the C control flow and records the remaining Wii U Pro and read-report differences.

The PowerPC Wii Remote startup path is now serialized like C: request status first; when an attachment is present, read the six-byte expansion ID at `0x04A400FA`; detect the Wii U Pro ID `0xA4200120`; then read accelerometer calibration; only after calibration completes set the initial handshake-complete state and request status/report setup. Rust no longer starts calibration, report-mode setup, and status in parallel, so `WPAD_Probe` cannot become ready from a calibration reply that arrived before C's status/ID stages.

Wii U Pro controllers now preserve the special initial-ID state and skip the normal IR-enable register sequence, matching C's status-only IR handling for that controller.

An adjacent report-assembly bug was also fixed: Wii Remote `0x21` read reports carry only the low 16 bits of a memory address. The shared assembler now compares that low 16-bit offset against expansion-space requests such as `0x04A4xxxx`, allowing expansion ID/calibration reads to complete instead of being rejected as out-of-range. A focused regression pins this behavior.

Hardware handshake sequencing across original remotes, extensions, and Wii U Pro controllers remains the validation gate.

## 2026-10-12 Wii Bluetooth startup parity check

The application DOL run on 2026-10-12 uploaded and reported `application mode=MENU`, but it did not report Bluetooth startup stages or a Wii Remote connection. This confirms that the program started. It does not confirm that the Rust Bluetooth host became ready or that the Remote reconnected.

### Adapter firmware patch: ported, hardware check pending

In C, `WPAD_Init` calls `BTE_InitCore` (`../../C-sources/libogc/wiiuse/wpad.c:1288-1323`). After core initialization, C reads stored link keys, applies the Bluetooth firmware patch, then calls `BTE_InitSub` (`wpad.c:405-453`). The patch sends the vendor start command, 184-byte patch block, and 92-byte patch block, then resets and reinitializes the controller (`../../C-sources/libogc/lwbt/bte.c:61-90,444-468,1444-1537`).

`crates/services/wiiuse-rs/src/adapter_patch.rs` now owns the transport-independent patch sequence and both firmware blocks. `ogc-bt-sync` sends the vendor commands only after the stored-key command completes, checks each HCI completion, resets the adapter, repeats host setup, and enters `HostStage::Ready` only after that setup succeeds. No C dependency is added. Fixed CRC-32 checks pin all 184 and 92 payload bytes against the values extracted from `bte.c` (`f13806d1` and `070a34e3`).

This port does not prove that the missing patch caused the observed disconnect. A Wii Remote can disconnect when the new DOL takes control of Bluetooth; the saved run has no HCI or controller-stage report to show whether it later reconnected. Hardware behavior is still unverified.

### Paired-key loading and retry handling: implemented, Wii check pending

The Bluetooth startup path now validates the four Read Stored Link Key return bytes (`Max_Num_Keys`, `Num_Keys_Read`) and waits for the matching Return Link Keys record count. It ignores zeroed key fields, as required by the Bluetooth specification, and accepts a nonzero key field only as a legacy-controller compatibility path; libogc's `hci_return_link_key_evt` copies these fields into its key cache (`../../../../C-sources/libogc/lwbt/hci.c:2033-2058`). The adapter firmware used by this Wii has not been checked to see which behavior it uses.

WPAD loads `BT.CDIF` guest keys before host polling and keeps a volatile guest list for the current WPAD session. It reverses the configured BD_ADDR and key bytes when moving between SYSCONF and HCI order, matching `__readlinkkey_finished` and `__wpad_linkkeyreqCB` (`../../../../C-sources/libogc/wiiuse/wpad.c:427-447,1040-1068`). A guest Link Key Notification updates only the volatile HCI key cache. The connected event carries a guest key to WPAD after the Wii Remote link is ready; WPAD then adds the guest record to its volatile list. `WPAD_Search` clears that list, and shutdown writes the current list to SYSCONF, matching C's init/search/shutdown lifecycle. Normal pairing writes the key to the adapter's stored-key table. The exact configuration save and target behavior remain unverified on hardware.


### Required follow-up

- The new crate `crates/services/wiiuse-rs` owns the initial Remote handshake and adapter patch state machines. `wpad-sys` uses the handshake for status, expansion ID, calibration, retry, and ready transitions. `ogc-bt-sync` uses the patch state machine for vendor commands and post-reset configuration. This is a focused Wiiuse port; it does not port the full library.
- The calibration decoder now lives in `wiiuse-rs/src/calibration.rs`; WPAD maps the result into its runtime type. It requires eight bytes and uses fixed low-bit packing. C stores raw calibration data; Rust retains WPAD's existing nonzero fallback and saturating subtraction so malformed values do not create zero divisors or unsigned wraparound in motion calculations. Host tests cover valid bytes, truncation, zero values, and subtraction underflow.
- The independent reviewer found that WPAD's old read assembler counted fragment lengths without tracking byte offsets. It could accept repeated fragments as a complete calibration block. Assembly now lives in `wiiuse-rs/src/read_assembly.rs`, tracks each requested byte, accepts identical duplicates without increasing the count, rejects conflicting overlaps, and supports fragments that arrive out of order. Host tests cover those cases and reject bytes beyond the requested range. The production WPAD backend uses this same assembler. Later review confirms recovery for failed read handling and locally rejected command submission. The 2026-10-28 staged command-queue fix also handles decoded `0x22` ACK results and requests fresh status after a failed ACK. That scoped source change passed independent re-review; real-controller validation and broader `wiiuse-rs` source review remain pending.
- application now records Bluetooth startup stages plus connection and disconnection events in its agent report. The hook reports that the patch stage was reached; only real hardware can confirm that the adapter accepted the firmware and a Remote reconnected.
- Build the application DOL and run it on Wii hardware with a Remote. Host-side tests cannot confirm adapter firmware behavior or the real reconnect.

## Independent re-review — current channel, handshake, and report behavior

- Result: **NEEDS_CHANGES**
- Reviewer: `/root/reviewer_f`
- Handoff implementer: `worker_a`, commit `cf2c092` (`Fix WPAD six-channel parity and readiness`).
- Later direct WPAD behavior change reviewed: commit `8acd74a` (`Fix heavy runtime parity gaps`), which adds the staged initial expansion-ID handshake, Wii U Pro state, and low-16-bit read-report addressing. I also reviewed the later Bluetooth HID channel-disconnect changes that feed the WPAD slot cleanup path (`7204103`, `1bfc6f9`, `e10a965`).
- Scope: channel 5 validation; `WPAD_CHAN_ALL` MotionPlus dispatch; end-to-end backend slot capacity; probe readiness and output-pointer behavior; initial calibration / Wii U Pro handshake; read-report assembly and button side effects; closing one or both HID channels.
- C inspected: `../../C-sources/libogc/gc/wiiuse/wpad.h`, `../../C-sources/libogc/wiiuse/wpad.c`, `../../C-sources/libogc/wiiuse/wiiuse.c`, `../../C-sources/libogc/wiiuse/io.c`, `../../C-sources/libogc/wiiuse/events.c`, `../../C-sources/libogc/wiiuse/ir.c`, `../../C-sources/libogc/wiiuse/wiiuse_internal.h`, and `../../C-sources/libogc/lwbt/bte.c`.
- Rust inspected: `crates/services/wpad-sys/src/lib.rs`, `powerpc_backend.rs`, and `wii_remote.rs`, plus `crates/services/ogc-bt-sync/src/hid_host.rs` and `crates/services/ogc-bt-sync/src/protocol.rs` where they determine the actual HID slot capacity and channel-close events.

### Fixes confirmed against C

- The channel constants and `valid_chan` now agree with C's enum and range: channel 5 is a valid index and `WPAD_MAX_DEVICES` is 6 (`wpad.h:39-51`; Rust `lib.rs:80-89,614-620`).
- Rust's `WPAD_CHAN_ALL` MotionPlus wrapper iterates channels 0 through 5 and returns the first negative result, as C's recursive loop does (`wpad.c:1541-1552`; Rust `lib.rs:1070-1082,1118-1125`). The disabled-service guard on the all-channel call also matches C's disabled result (`wpad.c:1557-1560`; Rust `lib.rs:1071-1075`). The test checks the generic dispatch helper visits all six indices in order (`lib.rs:1918-1934`), but it does not run the PowerPC backend for six active slots.
- The WPAD-side controller, stream, activity, and report arrays are now dimensioned from six `WPAD_MAX_DEVICES` entries (`powerpc_backend.rs:48-49,79,115,167`; Rust public state in `lib.rs:423-534`). C also creates six Wiiuse structures and six WPAD entries (`wpad.h:39-48`; `wpad.c:93-95,1288-1290`; `wiiuse.c:94-105`). The timer token now encodes three slot bits, so index 5 round-trips (`powerpc_backend.rs:111-113,169-175`; test at `:1625-1631`).
- `WPAD_Probe` now gates a connected Wii Remote on the initial calibration read completing. C requires enabled WPAD, a connected controller, and `WIIMOTE_STATE_HANDSHAKE_COMPLETE` (`wpad.c:1609-1645`); Rust checks global disabled state, connected state, and `wii_handshake_complete` (`powerpc_backend.rs:327-337,1481-1508`). The calibration completion path sets the flag only after a complete read result (`powerpc_backend.rs:1239-1248`).
- Rust now starts the normal Wii Remote sequence with status, conditionally requests the expansion ID, then requests calibration and a final status/report refresh (`powerpc_backend.rs:1035-1077,1122-1140,1142-1182,1224-1282`). C's `wiiuse_handshake` also starts with status, checks the attachment bit, reads the six-byte expansion ID when present, reads calibration, marks the handshake complete, then issues status (`io.c:23-67`). The low-16-bit address base for report `0x21` matches C's truncation of the requested address before subtracting the report offset (`events.c:57-66`; Rust `wii_remote.rs:192-220`).
- For link shutdown, C keeps the BTE link in disconnecting state and notifies its client only after both L2CAP control and data PCBs are null (`bte.c:1059-1081`). Rust's shared host emits individual control/interrupt close events, and WPAD clears the slot only after both channel bits have been observed (`ogc-bt-sync/src/hid_host.rs:1158-1185,1210-1237`; `wpad-sys/src/powerpc_backend.rs:318-325,1435-1449`). This matches the two-channel completion condition for those paths.

### Remaining confirmed mismatches

#### Six-controller host capacity — fixed and independently reviewed

The HID host now allocates six links, matching C's six WPAD slots. Independent reviewer `reviewer_beta` passed the change in `b69ca98`. Its regression allocates slots 0 through 5 and checks that a seventh allocation fails. See `docs/review/libbt-sync-peer-disconnect.md` for the full comparison.

#### Probe output and expansion mapping — fixed and independently reviewed

Both the PowerPC backend and host wrapper now preserve `type` on invalid, disabled, disconnected, and incomplete-handshake results. On success, unknown expansion types map to `WPAD_EXP_NONE`, matching C's `WPAD_Probe` switch. Independent reviewer `reviewer_alpha` passed this behavior in `e2052ef`; the public wrapper tests cover error output preservation and MotionPlus mapping.

#### Single-channel MotionPlus results — fixed and independently reviewed

The PowerPC backend now returns `WPAD_ERR_NOT_READY` when disabled and success for inactive or non-Wii-Remote slots, matching C's allocated `__wpads` behavior. Independent re-reviews passed the disabled/inactive result in `1ffaeab` and the active non-Wii-Remote result in `0f276a2`.

#### Wii U Pro handshake does not follow C's special path (fixed; needs independent re-review)

C detects the Wii U Pro ID in the initial expansion-ID callback, zeroes the calibration buffer, sets `WIIMOTE_STATE_WIIU_PRO`, and falls through directly to calibration decoding and handshake completion; it does not issue a second accelerometer-calibration read (`wiiuse/io.c:31-66`). Rust detects the same ID and records `slot.wii_u_pro` (`wpad-sys/src/powerpc_backend.rs:1224-1237`), but every completed `InitialExpansionId` read then calls `request_wii_calibration` unconditionally (`:1268-1271`). That requests another eight bytes and delays readiness; Rust also decodes those fetched bytes instead of C's zeroed Pro calibration buffer. This is a protocol sequence and state difference on the specifically supported Wii U Pro path.

Implementer: `/root/worker_a`. Independent static re-review is needed; the review thread owns it.

The Pro ID path now sets `cal_zero` and `cal_g` to zero, marks the handshake complete when the ID read completes, and chooses the final status request as its next action. It does not request or decode a second calibration block. A normal remote still proceeds from the expansion ID to the calibration read. Regression tests cover the selected follow-up action, handshake state and timing, Pro calibration contents, and preservation of the smoothing alpha.

Validation: `cargo check -p wpad-sys --target powerpc-unknown-linux-gnu --tests` and `git diff --check` passed. The tests were type-checked but not run. No hardware verification was performed. Other WPAD findings in this note remain open.

## Independent re-review — Wii U Pro initial handshake

- Result: **PASSED** for the Wii U Pro handshake fix only.
- Reviewer: `/root/reviewer_c`.
- Implementer: `worker_a`.
- Implementation commit: `06116ff` (`Match Wii U Pro WPAD handshake`).
- C inspected: `../../C-sources/libogc/wiiuse/io.c`, `wiiuse_handshake` (lines 16–67), and `../../C-sources/libogc/wiiuse/wiiuse_internal.h` (`EXP_ID_CODE_CLASSIC_WIIU_PRO`, `WIIMOTE_STATE_WIIU_PRO`).
- Rust inspected: `crates/services/wpad-sys/src/powerpc_backend.rs`, `complete_initial_expansion_id` (lines 140–151), `request_wii_calibration` (lines 1076 onward), `handle_wii_read_report` (lines 1228–1301), and the regression tests (lines 1677–1706).

C detects the Pro ID in the initial expansion-ID callback, zeroes the eight-byte data buffer, sets the Pro state, and falls through to calibration decoding and handshake completion without issuing the ordinary accelerometer-calibration read (`io.c:31–67`). Rust now detects the same ID, sets `cal_zero` and `cal_g` to zero, marks handshake completion, and selects the final status request as the follow-up. `handle_wii_read_report` only requests calibration for the `CalibrationRead` result; the Pro result requests status directly. The normal remote still takes the calibration-read path. This matches the C order and resulting zero calibration values for the targeted Pro path. The subsequent status command is consistent with C's `wiiuse_status(wm, NULL)` after marking handshake complete.

The new tests cover Pro-versus-standard ID follow-up selection, Pro calibration zeros, handshake state, preservation of `st_alpha`, and unchanged standard-remote defaults (`powerpc_backend.rs:1677–1706`). They test the factored handshake decision and state mutation, not the complete HID report/command path; static inspection confirms the handler dispatches the returned action as described above. This leaves a modest integration-coverage gap, but no mismatch in the reviewed implementation.

Validation: `cargo check -p wpad-sys --target powerpc-unknown-linux-gnu --tests` passed during this re-review; the target-gated tests compiled but did not run. No hardware verification was performed. Other WPAD findings listed above remain open, and this result does not mark the crate complete.

The subsequent IR handling is also only partly specialized. C's `wiiuse_set_ir` returns after requesting status when IR is already disabled, and its enable path requests status for a Wii U Pro controller rather than programming the IR registers (`ir.c:191-203`; `io.c:40-43`). Rust's `enable_wii_ir` has a Pro branch that requests status (`powerpc_backend.rs:984-989`), but its default no-IR path calls `disable_wii_ir`, which always sends two IR commands before requesting status (`:1021-1032`). On the initial default `WPAD_FMT_BTNS` path C's `__wpad_setfmt` calls `wiiuse_set_ir(wm, 0)` (`wpad.c:222-239`); Rust's final report refresh follows the unconditional disable path (`powerpc_backend.rs:1184-1195,954-982`). This adds controller writes on initial setup, including for a Pro controller whose IR is already disabled.

**Implementer handoff — IR disable writes:** Fixed; needs independent static re-review. Implementer: `/root/worker_a`. C `wiiuse_set_ir` skips the two IR disable commands when `WIIMOTE_STATE_IR` is clear, then requests status (`ir.c:191-203`). Rust now checks the slot's `ir_enabled` state and sends the two disable commands only when IR was enabled; it still requests status and records IR as disabled. A focused test checks the command decision for both states. This addresses the already-off disable mismatch. Other IR behavior noted above remains open for separate review.

Validation: `cargo check -p wpad-sys --target powerpc-unknown-linux-gnu --tests` passed; the target-gated tests were type-checked but not run. `git diff --check` passed. No hardware verification was performed.

#### Calibration request length differs from C

C issues the initial accelerometer calibration read with length 7 (`wiiuse/io.c:48-50`), then reads `data[7]` while decoding that result (`io.c:53-59`). Rust requests eight bytes so the decoder receives every byte it accesses (`powerpc_backend.rs:1057-1075`). This remains a deliberate safe difference from C's out-of-bounds read. It is documented rather than copied.

#### `0x21` read reports — fixed and independently reviewed

C calls `wiiuse_pressed_buttons` for every read report and decrements the outstanding byte count by each packet's payload length (`wiiuse/events.c:29-76`). Rust now does the same while retaining offset-based writes. Independent reviewer `reviewer_alpha` passed the production path and regressions in `7106e49`; the tests cover duplicate/out-of-order fragment accounting and button transitions when no read is pending.

### Regression coverage and validation

The current tests check six-link host allocation, public probe output preservation, single-channel MotionPlus result codes, Pro handshake decisions, IR disable decisions, low-16-bit expansion reads, `0x21` byte-count accounting, read-report button transitions, and report decoders. Some checks exercise factored helpers instead of the full hardware command sequence. PowerPC test code compiles, but it does not run in the current environment.

`cargo check -p wpad-sys --target powerpc-unknown-linux-gnu` passed. `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` passed and type-checked the target-gated tests; tests did not run. `cargo test -p wpad-sys --target powerpc-unknown-linux-gnu --no-run` compiled the crate and test code but failed at linking because the host linker rejected the PowerPC options (`-m32`, `--as-needed`, and related flags). No tests ran and no hardware verification was performed.

The fixes confirmed above do not close the remaining mismatches. Keep `wpad-sys` open for implementation follow-up; broader WPAD findings remain open as recorded earlier.

### Independent re-review of redundant IR-disable fix `633f5cd`

**Result: PASS for the IR-disable fix only.** Reviewer: `/root/reviewer_f`; implementer: `/root/worker_a`.

C checks its tracked `WIIMOTE_STATE_IR` bit and skips the two disable writes when IR is already off, then requests status (`C-sources/libogc/wiiuse/ir.c:197-212`; state updates in `events.c:138-151`). Rust `disable_wii_ir` now checks `slot.ir_enabled`, skips the commands when false, requests status, and updates the tracked flag from command/status outcomes (`crates/services/wpad-sys/src/powerpc_backend.rs:1033-1058,1154,1182`). The Wii U Pro enable path also requests status without programming IR registers, as C does. The regression test checks command choice from enabled/disabled state but not the full command queue.

Reviewer_f ran no checks. The recorded PowerPC `cargo check --tests` passed and compiled test code; tests did not run. No hardware verification was performed. A separate re-review of `76675c1` (`WPAD_CHAN_ALL` MotionPlus test/dispatch handoff) remains queued, and broader WPAD mismatches keep the crate open.

## Implementer handoff: `WPAD_CHAN_ALL` MotionPlus dispatch

Implementer: `worker_b`. Status: fixed; needs independent review. Implementation/test commit: `76675c1` (`Test WPAD all-channel MotionPlus dispatch`). Direct comparison with C `WPAD_SetMotionPlus` (`../../C-sources/libogc/wiiuse/wpad.c:1541–1566`) confirms that the public Rust `set_motion_plus(WPAD_CHAN_ALL, ...)` branch and six-channel order already existed in `cf2c092`; it runs each channel from zero through five, skips empty PowerPC slots like C's null `__wpads[chan]` entries, stops on the first negative error, and returns success otherwise. This handoff adds a focused test around the MotionPlus aggregate dispatch helper. It records each `(channel, enable)` call and checks all six channels in order with the requested enable value. The previous implementation was therefore present before this handoff; this commit adds direct regression coverage and keeps the C comparison with the review thread.

Validation: `cargo check -p wpad-sys --target powerpc-unknown-linux-gnu --tests` passed; target tests compiled but were not run. `git diff --check` passed. `cargo fmt -p wpad-sys -- --check` reports pre-existing formatting differences in the touched source files. No hardware verification was performed. Independent static re-review is needed; other WPAD findings remain open.

### Independent re-review of `WPAD_CHAN_ALL` MotionPlus dispatch (`76675c1`)

**Result: PASS for the bounded dispatch/test change.** Reviewer: `/root/reviewer_e`; implementer: worker_b.

C handles `WPAD_CHAN_ALL` in channel order 0 through 5, stops at the first negative result, and otherwise returns `WPAD_ERR_NONE` (`C-sources/libogc/wiiuse/wpad.c:1541-1566`). Rust's wrapper dispatches through all six channel slots in order, treats absent PowerPC slots as success, stops on the first negative error, and preserves the requested enable value (`crates/services/ogc-wpad-sys/src/lib.rs:1070-1082,1118-1130`; `powerpc_backend.rs:794-799`). Commit `76675c1` adds a helper test for order and enable value; it tests the helper rather than the full backend.

A separate single-channel mismatch remains: C returns `WPAD_ERR_NOT_READY` when WPAD is disabled and success for an allocated but inactive slot, while Rust can return `WPAD_ERR_NO_CONTROLLER` for an inactive slot (`wpad.c:1554-1566`; Rust `lib.rs:1084-1088`, `powerpc_backend.rs:745-748`). This does not affect the all-channel wrapper's absent-slot handling, but keeps broader WPAD parity open. Validation: PowerPC `cargo check --tests` passed and compiled tests; tests did not run. No hardware verification was performed.

## Implementer handoff — Single-channel MotionPlus return codes

Implementer: `/root/worker_b`. Independent re-review: needed; review thread owns it.

C `WPAD_SetMotionPlus` checks disabled state and returns `WPAD_ERR_NOT_READY`. If WPAD is enabled and the channel has no active controller object, it completes as a no-op and returns `WPAD_ERR_NONE` (`../../C-sources/libogc/wiiuse/wpad.c:1554–1566`). Rust's PowerPC backend previously returned `WPAD_ERR_NO_CONTROLLER` when the slot was inactive. The backend now returns `WPAD_ERR_NOT_READY` while disabled and `WPAD_ERR_NONE` for an inactive slot. Active controller handling is unchanged. This is separate from the other WPAD differences recorded above.

Regression checks cover both return cases and confirm active slots proceed to normal handling. `git diff --check` and `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` passed. The test target compiled; tests were not run. `cargo fmt -p wpad-sys -- --check` reports formatting differences in pre-existing code in both touched source files. No hardware verification was performed. The shared ledger is managed by the coordinator.

### Independent re-review of disabled/inactive return fix `1ffaeab`

**Result: PASS for the bounded return-code fix.** Reviewer: `/root/reviewer_d`; implementer: `worker_b`.

C `WPAD_SetMotionPlus` validates the channel, returns `WPAD_ERR_NOT_READY` when WPAD is disabled, and returns success for an allocated but inactive slot without calling the controller routine (`C-sources/libogc/wiiuse/wpad.c:1541-1566`; channel slots are allocated in `wiiuse/wiiuse.c:94-138`). Rust validates the index in the public wrapper and runs `motion_plus_preflight`; it returns the same disabled and inactive results before the active Wii Remote path (`crates/services/wpad-sys/src/lib.rs:1084-1088`; `crates/services/wpad-sys/src/powerpc_backend.rs:745-751,800-808`). The helper tests cover the conditions but do not invoke the public/backend function.

A separate active-slot mismatch remains: for an active non-Wii-Remote `ControllerKind`, Rust returns `WPAD_ERR_NO_CONTROLLER`, while C calls `wiiuse_set_motion_plus` for any non-null channel pointer. This does not fail the bounded disabled/inactive fix, but keeps broader WPAD parity open. Validation: `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled tests; tests did not run. No hardware verification was performed.

## Implementer handoff — Active non-Wii-Remote MotionPlus result

Implementer: `/root/worker_b`. Independent re-review: needed; review thread owns it.

C validates the channel and disabled state, then calls `wiiuse_set_motion_plus` for any allocated channel object and returns `WPAD_ERR_NONE` (`../../C-sources/libogc/wiiuse/wpad.c:1541–1566`). Rust previously returned `WPAD_ERR_NO_CONTROLLER` for an active `Generic` or `Switch` slot because the backend accepts only `WiiRemote` slots for its report operations. The public call now returns success for an active non-Wii-Remote slot, matching the C return code. Existing disabled and inactive behavior remains unchanged, and active Wii Remote requests continue through the existing implementation. This handoff covers the return-code difference; controller-specific asynchronous effects remain outside this finding.

Regression coverage checks success for an active non-Wii-Remote slot alongside the existing disabled, inactive, and active Wii Remote preflight cases. `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` and `git diff --check` passed. The PowerPC test target compiled; tests were not run. `cargo fmt -p wpad-sys -- --check` reports formatting differences in pre-existing code in both touched source files. No hardware verification was performed. The shared ledger is managed by the coordinator.


## Independent re-review — active non-Wii-Remote MotionPlus result

Reviewer: `worker_c`. Implementer: `worker_b` (commit `0f276a2`). I inspected the complete `set_motion_plus` and preflight diff against C `WPAD_SetMotionPlus` (`../../C-sources/libogc/wiiuse/wpad.c:1541-1566`). For a valid channel while WPAD is enabled, C calls the controller routine only when the channel pointer is non-null and returns success for a null slot or any allocated controller type. Rust now returns success for inactive and active non-Wii-Remote slots, while active Wii Remote slots continue to the request path; disabled state still returns `WPAD_ERR_NOT_READY`. I found no mismatch in this return-code handoff. The helper regression tests do not invoke the public backend function, but source inspection confirms the function uses the tested preflight result. Broader WPAD findings remain open. I ran no checks for this static re-review; implementation validation is recorded above. No hardware verification was performed.


## Implementer handoff: preserve `WPAD_Probe` output on errors

Implementer: `thread-current/root`. Independent reviewer: pending assignment. C writes the caller's `type` output only when the channel has a connected controller whose handshake is complete (`../../C-sources/libogc/wiiuse/wpad.c:1609-1643`). Rust now preserves the output for invalid channels, disabled state, incomplete handshake, and disconnected channels. A focused helper regression verifies that error results leave a sentinel unchanged and success writes the expansion value. Other WPAD differences remain open. Independent static re-review is required.

Validation: `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled test code; tests did not run. No hardware verification was performed.

### Independent re-review of probe output parity (`b69ca98`)

**Result: NEEDS_CHANGES.** Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

The PowerPC backend preserves the output on all error returns, matching C. The host compatibility entry point still wrote zero while WPAD was disabled or the controller was disconnected, and it checked disabled state before validating the channel. The original regression only exercised the helper, not the public wrapper. This gap is now fixed: the wrapper validates the channel first, returns `WPAD_ERR_NOT_READY` without writing while disabled, and writes the expansion type only on successful connected probes. The existing host test now checks sentinel preservation for invalid channel, disconnected channel, and disabled WPAD.

Validation: PowerPC `cargo check --tests` passed and compiled tests; they did not run. The host test command is blocked by the crate's PowerPC-only compile guard. No hardware verification was performed.

## Implementer handoff: public `WPAD_Probe` output preservation

Implementer: `thread-current/root`. Independent reviewer: pending. The host compatibility implementation now follows the C validation order and does not modify `type` unless the probe succeeds. The regression extends the public API test to cover invalid, disconnected, and disabled results with a sentinel output. Other WPAD findings remain open.

### Independent re-review follow-up: successful probe type mapping

Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

C writes only recognized expansion types and reports `WPAD_EXP_NONE` for other type values. The first wrapper correction preserved the output on errors but wrote the raw type on success. The host wrapper now uses the same `probe_expansion_type_for_wii` mapping as the PowerPC backend, and the existing MotionPlus probe expectation now checks `WPAD_EXP_NONE`. Independent re-review is pending.

Validation: `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled target test code; tests did not run. The host test command remains blocked by the crate's PowerPC-only compile guard. No hardware verification was performed.

### Independent re-review: probe output parity (`e2052ef`)

**Result: PASS for output and expansion-type parity.** Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

The public host wrapper validates the channel before disabled state, preserves the caller's output on invalid, disabled, and disconnected results, and writes a successful expansion type through `probe_expansion_type_for_wii`. Unsupported values map to `WPAD_EXP_NONE`, as in C. The public API regression checks sentinel preservation and the MotionPlus `WPAD_EXP_NONE` result. Reviewer_alpha ran no tests; PowerPC test code compiled but did not run. Other WPAD findings remain open.

## Implementer handoff: `0x21` read fragment accounting and button state

Implementer: `thread-current/root`. Independent reviewer: pending. Read assembly now counts payload bytes across reports instead of completing from the highest written offset. This matches C's decrementing remaining-byte counter, including its completion behavior when fragments overlap or leave gaps. The read-report path also updates WPAD button held/down/up state before checking for a pending read, matching C's `wiiuse_pressed_buttons` call at the start of `event_data_read`. Regression cases cover out-of-order duplicate payload fragments and button press/release transitions.

Validation: `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled target tests; tests did not run. Host test execution is blocked by the crate's PowerPC-only compile guard. No hardware verification was performed.

### Independent re-review: read report accounting and buttons (`7106e49`)

**Result: PASS for the `0x21` read-report fix.** Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

C updates button state at the start of `event_data_read`, before checking for a pending command. Rust now applies button held/down/up transitions before the pending-read check. C decrements remaining bytes by every fragment length and completes when the requested total is reached; Rust now counts fragment bytes while retaining offset-based writes. The regressions cover repeated/out-of-order fragment lengths and button press/release changes. Rust retains bounds checks for malformed offsets, a safer error path than C's unchecked copy.

PowerPC test code compiled but did not run. No hardware verification was performed. Other WPAD findings remain open.


### Independent re-review: initial IR-disable behavior

**Result: PASS for the initial already-disabled IR path only.** Reviewer: `reviewer_beta`. Implementer: `/root/worker_a` (commit `633f5cd`).

C `wiiuse_set_ir(wm, 0)` checks `WIIMOTE_STATE_IR`; when it is clear, it skips both `WM_CMD_IR` and `WM_CMD_IR_2` and requests status (`../../C-sources/libogc/wiiuse/ir.c:191-212`). C updates the state from status reports (`../../C-sources/libogc/wiiuse/events.c:138-151`). Rust's initial format refresh reaches `disable_wii_ir`; it reads `slot.ir_enabled`, sends both commands only when true, then requests status and clears the tracked flag (`crates/services/wpad-sys/src/powerpc_backend.rs:954-982,1063-1083`). Status report handling refreshes `slot.ir_enabled` from the device report before the normal reporting path (`:1198-1206`). This matches the scoped already-disabled command behavior.

The regression `ir_disable_skips_register_commands_when_ir_is_already_off` tests only the factored command-decision helper for false and true; it does not invoke `disable_wii_ir` or inspect the emitted command queue. Static inspection confirms the production function uses the helper and still requests status in either case. Per the implementation handoff, PowerPC test code compiled but the target-gated tests did not run; I ran no checks. No hardware verification was performed. This review does not cover other IR behavior or close remaining WPAD findings.

## 2026-10-05 full-scope classification

`wpad-sys` is now `parity_blocked`. Current C accepts `WPAD_CHAN_ALL` for `WPAD_Rumble`, `WPAD_ControlSpeaker`, and `WPAD_SetIdleThresholds` by dispatching to every channel. The Rust PowerPC wrappers validate the argument as a single channel first and return `WPAD_ERR_BAD_CHANNEL`. Threshold handling also differs: C normalizes any negative button threshold to `-1` and stores the other threshold arguments directly, while Rust rejects values below its accepted floor. These are observable non-safety differences. The prior 8-byte calibration read remains an accepted bounds-safety correction to C's 7-byte out-of-bounds use. No additional C source is needed.


## 2026-10-05 `WPAD_CHAN_ALL` and idle-threshold parity fix

C dispatches `WPAD_Rumble`, `WPAD_ControlSpeaker`, and `WPAD_SetIdleThresholds` across channels 0 through `WPAD_MAX_DEVICES - 1` when the caller passes `WPAD_CHAN_ALL`, stopping on the first negative result. Rust now follows the same aggregate dispatch. On PowerPC, Rumble and ControlSpeaker treat empty controller slots as successful no-ops during the aggregate call, matching C's null-`__wpads[chan]` behavior, while active-slot transport errors still propagate.

C stores the button idle threshold as `-1` for any negative input and as `0` for any non-negative input. Rust now uses the same button-threshold normalization. For the IR, accelerometer, joystick, balance-board, and MotionPlus thresholds, Rust continues to reject values below `WPAD_THRESH_IGNORE` instead of storing arbitrary values below the public ignore sentinel. That stricter input validation is retained as an accepted safety deviation rather than a parity blocker. The existing calibration over-read correction remains an accepted bounds-safety deviation.

The previous confirmed non-safety blockers from the full-scope classification are therefore fixed. `wpad-sys` moves to `needs_re_review` pending independent source review and PowerPC/runtime validation.

## 2026-10-07 idle-threshold parity follow-up

A fresh comparison against `WPAD_SetIdleThresholds` found two remaining current-source differences in the previous handoff. C validates the channel, returns `WPAD_ERR_NOT_READY` when the global WPAD state is disabled, normalizes only the button threshold (`btns < 0 ? -1 : 0`), and stores the other five signed threshold values directly. Rust now follows that behavior for both the PowerPC path and the compatibility path. Values below `WPAD_THRESH_IGNORE` for IR, accelerometer, joystick, balance-board, and MotionPlus thresholds are no longer rejected. A regression also checks that a post-shutdown call returns `WPAD_ERR_NOT_READY`.

The 8-byte calibration read remains an intentional bounds-safety correction to C's 7-byte over-read. Controller handshake/report timing and extension-device behavior remain runtime-validation scope.

## Independent re-review — idle-threshold behavior (2026-10-07)

**Result: PASSED for the current idle-threshold fix.** Reviewer: `/root` in a new review thread. C validates the channel, returns `WPAD_ERR_NOT_READY` when WPAD is disabled, stores the button threshold as `-1` or `0`, and stores the five other threshold values directly (`../../C-sources/libogc/wiiuse/wpad.c:1827–1859`). Rust now follows that behavior, including negative non-button threshold values and the disabled-state result, in `crates/services/wpad-sys/src/lib.rs`. The all-channel path still processes channels in order and stops on the first negative result, as C does. PowerPC target test code compiled with `RUSTFLAGS='-Aunsafe-op-in-unsafe-fn'`; tests were not run. Hardware controller behavior was not tested.

## 2026-10-12 Wiiuse button-report port

`wiiuse-rs/src/buttons.rs` now decodes the big-endian button word with C's `WIIMOTE_BUTTON_ALL` mask (`0x1f9f`) and calculates current, held, pressed, and released states using the same formulas as `C-sources/wiiuse/src/events.c:326-355`. The WPAD production path uses this shared behavior for normal input reports and memory-read reports. A memory-read report updates the base 16-bit Wii Remote buttons while retaining expansion-button bits in the combined WPAD state. Regression coverage checks big-endian decoding, mask filtering, truncated data, transitions, and an expansion button held across a read report.

Independent reviewer `/root/wiiuse_calibration_review` passed the change against C and confirmed the expansion-button preservation. `cargo test -p wiiuse-rs -p ogc-bt-sync` passed (68 tests); `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu`, `cargo xtask check-group input`, `cargo xtask audit-source`, `cargo xtask check-inventory`, and `git diff --check` passed. PowerPC target tests compiled but did not run. No hardware test was performed. This adds one Wiiuse behavior slice; it does not claim full Wiiuse parity.

## 2026-10-12 Wiiuse status-report decoder

`wiiuse-rs/src/status.rs` decodes the full `0x20` HID report. It reads status flags from full-report byte 3 and battery level from byte 6. The C `event_status` handler receives a slice after the report ID, so these correspond to its offsets 2 and 5 in `C-sources/libogc/wiiuse/events.c`. The production WPAD handler uses the shared decoder. Tests cover each flag, the battery byte, short reports, and wrong report IDs.

Independent reviewer `/root/wiiuse_calibration_review` passed the decoder against the C parser and confirmed the full-report offset adjustment. `cargo test -p wiiuse-rs -p ogc-bt-sync` passed (71 tests); `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu`, `cargo xtask check-group input`, `cargo xtask audit-source`, `cargo xtask check-inventory`, and `git diff --check` passed. PowerPC test code compiled but did not run. No Wii hardware test was performed.

## 2026-10-12 Nunchuk button transitions

The shared `ButtonState::decode_active_low` handles the Nunchuk's active-low Z/C bits with mask `0x03`. WPAD now uses this production helper for byte 5 of the six-byte Nunchuk report, then publishes those extension bits with the base buttons. C uses the same active-low mask and held/released formulas in `C-sources/wiiuse/src/nunchuk.c`; libogc's Nunchuk event handler also reads byte 5. Tests cover Z/C held, pressed, and released transitions.

Independent reviewer `/root/wiiuse_calibration_review` passed the C comparison and confirmed that WPAD retains the previous extension-button state before publishing the new report. `cargo test -p wiiuse-rs -p ogc-bt-sync` passed (72 tests); PowerPC test code compiled, the input package check and source audit passed, and the inventory and diff checks passed. No hardware test was performed.

## 2026-10-12 Classic Controller and Guitar Hero 3 button transitions

The shared active-low word decoder uses C's `0xfeff` mask and big-endian bytes 4 and 5 for Classic Controller and Guitar Hero 3 reports. Their expansion structs store raw low-word current, previous, held, and released button values. WPAD shifts expansion buttons into its combined high-word button state when it publishes the report. This matches `classic.c`, `guitar_hero_3.c`, and `wpad.c`.

Independent reviewer `/root/wiiuse_calibration_review` passed the source comparison and the production report-path tests. The tests check expansion fields and combined `btns_h`, `btns_d`, and `btns_u`. `cargo test -p wiiuse-rs -p ogc-bt-sync` passed (73 tests); `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` compiled the target-gated tests, and the input package check passed. Target-gated tests did not run. No Wii hardware test was performed.

## 2026-10-12 Wii Balance Board calibration and load reports

`wiiuse-rs` now decodes the three level-major calibration groups, including the upstream byte-offset-16 fallback, and applies Wiiuse's piecewise 0-to-34 conversion to top-right, bottom-right, top-left, and bottom-left sensor values. If both calibration blocks start with `0xff`, WPAD retains the expansion handshake and reissues the 224-byte calibration read. The report path requires the full eight-byte sensor payload and updates both raw and calibrated `WiiBoard` fields.

Independent reviewer `/root/wiiuse_calibration_review` passed the calibration ordering, fallback, retry transition, report offsets, and production report-path test. `cargo test -p wiiuse-rs -p ogc-bt-sync` passed (76 tests); `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu`, `cargo xtask check-group input`, `cargo xtask audit-source`, `cargo xtask inventory`, `cargo xtask check-inventory`, and `git diff --check` passed. PowerPC test code compiled but did not run. No Wii hardware test was performed.

## 2026-10-12 Guitar Hero 3 analog report conversion

`wiiuse-rs` now maps the five-bit touch value to libogc Wiiuse's touch-bar codes and calculates the whammy value as `(raw - 0x0f) / 11`. The calculation keeps negative values below the calibrated range and values above 1.0 above the range. WPAD uses this shared decoder in its production Guitar Hero 3 report path.

Independent reviewer `/root/wiiuse_calibration_review` passed the conversion against libogc's `guitar_hero_3.c`, `wpad.c`, and header constants. The older standalone `C-sources/wiiuse/src` uses a different whammy byte encoding, so libogc Wiiuse is the matching reference for WPAD. `cargo test -p wiiuse-rs -p ogc-bt-sync` passed (78 tests); the PowerPC test code compiled, and the input package check passed. No Wii hardware test was performed.

## 2026-10-12 Nunchuk stick and accelerometer reports

`NunchukReport` decodes the six-byte expansion payload into stick X/Y, packed ten-bit accelerometer X/Y/Z values, and the active-low button byte. WPAD uses the decoder before applying its joystick calibration and orientation calculations. This matches libogc's unconditional ten-bit extraction. Upstream Wiiuse only uses ten-bit extraction when `WIIUSE_ACCEL_10BIT` is defined.

Independent reviewer `/root/wiiuse_calibration_review` passed the payload offsets, packed bit extraction, and production WPAD report test. `cargo test -p wiiuse-rs -p ogc-bt-sync` passed (80 tests); `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu`, `cargo xtask check-group input`, `cargo xtask audit-source`, and `git diff --check` passed. The target-gated test code compiled but did not run. No Wii hardware test was performed.

## 2026-10-12 Nunchuk calibration

`NunchukCalibration` decodes the 224-byte calibration read, including the 16-byte prefix fallback, packed ten-bit accelerometer values, joystick bounds and centers, Wiiuse defaults, and C-compatible 16-bit subtraction. A repeated `0xff` prefix returns a retry request. WPAD uses this shared decoder and reissues the expansion calibration read when the retry marker is present.

Independent reviewer `/root/wiiuse_calibration_review` passed the source comparison against `C-sources/libogc/wiiuse/nunchuk.c`, including the WPAD retry path. The review first found that the shared decoder accepted retry-marked data; the decoder now rejects that block and a regression test checks it. `cargo test -p wiiuse-rs` passed (33 tests), and `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` compiled the target-gated production tests. Target tests did not run. No Wii hardware test was performed.

## 2026-10-12 Classic Controller calibration

`ClassicCalibration` now owns the libogc Wiiuse calibration rules. It checks the 224-byte read, detects Wii U Pro data, applies the byte-offset-16 prefix fallback, scales left and right stick fields, and repairs invalid min, center, and max ranges per axis. WPAD uses the shared decoder to fill both stick calibrations and the controller type.

Independent reviewer `/root/wiiuse_calibration_review` passed the C comparison, including field order, scaling, Pro marker, and fallback behavior. `cargo test -p wiiuse-rs` passed (37 tests); `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` compiled the target-gated production tests. Target tests did not run. No Wii hardware test was performed.

## 2026-10-12 Classic Controller input reports

`ClassicReport` decodes the standard six-byte report and the eleven-byte Wii U Pro report. It returns active-low button transitions, stick positions, trigger values, and Pro charging, wired, and battery state. Pro stick positions retain the byte truncation in libogc's `vec2b_t`. WPAD reads the required report length and publishes Pro stick buttons in the low button word while it maps Classic buttons into the high word, as libogc does.

Independent reviewer `/root/wiiuse_calibration_review` passed the layouts, arithmetic, masks, transitions, status bits, payload offsets, and WPAD merge. The reviewer found two small gaps: invalid prior button bits were exposed, and one expected button value reused the decoder expression. Both were fixed and the reviewer confirmed the fixes. `cargo test -p wiiuse-rs` passed (40 tests), and `cargo check -p wpad-sys --tests --target powerpc-unknown-linux-gnu` compiled the target-gated production tests. Target tests did not run. No Wii hardware test was performed.

## 2026-10-12 Motion Plus input reports

`MotionPlusReport` decodes six-byte expansion payloads into pitch, roll, and
yaw values, the extension-data flag, and the speed flags. WPAD uses this shared
decoder in its production report path. The extraction matches
`motion_plus_event` in `C-sources/libogc/wiiuse/motion_plus.c`; all decoded axis
values fit the positive range of the C `short` fields.

Independent reviewer `/root/wiiuse_calibration_review` passed the source
comparison, payload bounds, signed values, and WPAD mapping. Tests cover a
fixed report, flags, short input rejection, and production report shaping. No
Wii hardware test was performed.

## 2026-10-12 Guitar Hero 3 complete input reports

`GuitarHero3Report` owns the six-byte payload decoding previously split between
WPAD and the shared analog converter. Production WPAD uses the same Rust engine
that the host tests exercise. The engine checks payload bounds, decodes the
big-endian active-low button word with mask `0xfeff`, applies six-bit stick and
five-bit analog masks, and reuses the existing touch and whammy conversion.
Invalid previous button bits are removed, as with the other shared report
engines; normal C state has already been masked by its previous report.

Independent reviewer `/root/wiiuse_calibration_review` passed the comparison
against libogc's `guitar_hero_3.c`, `guitar_hero_3.h`, `wiiuse.h`, and `wpad.c`.
Fixed-value tests cover raw fields, button edges, unused bits, analog output,
and short payloads. The production regression checks WPAD fields and combined
button output. All 44 shared crate tests passed; PowerPC tests compiled but did
not run. The input package check, source audit, formatting, and diff check
passed. No hardware run was performed.

### Wii Remote ACK and expansion command order — fixed; broader review remains open

C reference: `C-sources/libogc/wiiuse/events.c` (`event_ack`),
`io.c` (`wiiuse_handshake_expansion`), and `wiiuse.c`
(`wiiuse_send_next_command`). The previous Rust path decoded and discarded
ACKs, read the ACK command and status one byte early, accepted a short report,
and submitted both encryption writes and the calibration read before earlier
commands completed.

Fix completed (2026-10-28): `wiiuse-rs/src/command_queue.rs` now provides a
fixed-capacity per-Remote command queue, exact five-byte ACK decoding, wire
encoders, and a staged expansion setup state machine. `wpad-sys` routes ACK,
status, and complete read replies through this production queue. Failed ACKs
append a fresh status request after queued commands, matching C FIFO order.
Expansion writes and the calibration read are submitted after the matching
prior completion. If the queue is full, removing its completed command frees
one slot for recovery status and preserves all other pending commands.

Regression tests cover ACK offsets and bounds, command ordering, queue
pressure, recovery status order, full-queue preservation, exact wire command
bytes, and expansion stages. The independent re-review by
`/root/bt_event_queue_rereview` passed this scoped source comparison on
2026-10-28. `cargo test -p wiiuse-rs` passed 52 tests;
`cargo xtask check-group input` compiled all 13 Wii runtime packages; and
`cargo xtask test-host` passed 70 xtask tests. `cargo xtask audit-stubs`
still reports the repository's 2,372 unverified APIs. These checks do not
establish Wii controller behavior. Hardware validation and broader
`wiiuse-rs` source review remain open.

## 2026-10-12 Bluetooth discovery and Wii Remote reconnect

This review compares the production path in `ogc-bt-sync` and `wpad-sys` with
`C-sources/libogc/lwbt/bte.c`, `C-sources/libogc/lwbt/hci.c`,
`C-sources/libogc/wiiuse/wpad.c`, and `C-sources/libogc/wiiuse/io_wii.c`.
It covers adapter setup, inquiry, incoming and outgoing ACL connections,
authentication, pairing PIN selection, and the control and interrupt HID
channels.

| Contract | Implementation | Independent source review | Wii hardware |
| --- | --- | --- | --- |
| BT-CLASS-01: decode the Bluetooth major class from bits 8 through 12 | Fixed `ClassOfDevice::major` to use the low five bits of byte 1. The previous shift rejected peripheral devices such as the Wii Remote before connection setup. Fixed-value tests use the wire bytes `[0x04, 0x25, 0x00]`. | Passed | Pending |
| BT-ADAPTER-01: set the final class, page timeout, name, scan mode, and event filter | Final HCI setup now matches the libogc values: class `[0, 4, 0x48]`, timeout `0x8000`, local name `Wii`, page scan enabled, followed by event filter `[1, 0]`. The host reports ready after the filter command completes. | Passed | Pending |
| BT-CONNECT-01: accept incoming Wii Remote connections and establish HID channels | Incoming requests use master role `0`; connection completion requests authentication. Incoming links listen for both peer channels. Outgoing links start the interrupt channel only after control-channel configuration completes. | Passed | Pending |
| BT-PAIR-01: authenticate and select the correct PIN for normal and guest pairing | Successful authentication applies link policy `0x0005`. Normal pairing uses the adapter address; temporary guest pairing uses the peer address. Tests cover authentication success and failure, and verify the command-complete event includes the connection handle. | Passed | Pending |
| BT-SEARCH-01: discover standard HID devices and support Wii guest pairing | General HID search keeps GIAC one-shot inquiry. Wii guest search uses the libogc LIAC periodic inquiry parameters and exits the periodic inquiry on stop. The application probe alternates these searches. | Passed | Pending |

The independent source reviewer checked these changes against the C paths and
found no remaining blocking issue in this scope. The review also caught and
corrected the test response for Write Link Policy: HCI returns Command Complete
with a connection handle. `cargo test -p ogc-bt-sync` covers both HID channels,
an input report, incoming connections, discovery, PIN selection, and
authentication success and failure.

The Wii Remote did not reconnect in the user's latest hardware run. The
class-of-device filter defect was real and is fixed, but the available report
does not prove it was the only cause. Hardware validation of the rebuilt DOL
remains pending. The bounded guest-search window, retained per-link guest PIN
state, and requirement to observe authentication completion are Rust safety
and lifecycle choices. Full `WPAD_Search` parity also remains open: C clears
existing active and guest state before guest search, while Rust preserves
connected controllers. Broader Bluetooth parity has not been audited as
complete.

The Dolphin Wii Remote smoke test found another initialization mismatch after
this source review: Rust skipped libogc's second `BTE_InitCore` setup before
reading stored keys, then reused that setup after the patch reset. This sent
HCI Set Event Mask (`0x0c01`) during the `BTE_InitSub` phase, where libogc
instead writes the inquiry and page scan modes, Wii class, page timeout, and
local name before enabling page scan. Rust now follows those separate core
and sub setup sequences. Packet tests assert both command orders and payloads.
The corrected Dolphin smoke run reached `BLUETOOTH READY`, then timed out on
HCI Reject Connection Request (`0x040a`). It did not connect a Remote or
deliver the emulated Remote's A input. The smoke test failed and saved the
Bluetooth report. Wii hardware validation remains pending.

The source audit, package tests, PowerPC test compilation, input package check,
and Wii DOL build are recorded for this pass. The PowerPC tests were compiled
but not run on hardware. Hardware results must be recorded separately from
source review and packet-level tests.

## 2026-10-15 example DOL discovery and guest pairing

The example package now enables `wii-controllers` by default. The shared
`ControllerManager` starts with Wii Remote guest pairing, alternates it with
general Bluetooth discovery, and retries after a 60-frame idle gap once an
inquiry ends. The scan loop continues during gameplay. An incoming unallocated
Remote request during guest pairing now creates a guest link and uses the peer
address for its PIN. A guest request cannot replace an outgoing link already
pending for the same address.

Regression test `unallocated_incoming_remote_during_guest_search_uses_guest_pin`
checks acceptance, guest-link state, and the peer PIN. The existing
`incoming_request_during_guest_search_cannot_replace_outgoing_link` test checks
the competing-link case. `cargo test -p ogc-bt-sync` passed 93 tests;
`cargo test -p ogc-controller` passed 76 unit tests, 12 HID descriptor tests,
and 4 application direction tests. Nine application/example DOLs built with the
explicit `wii-controllers` feature and have manifests that record that feature.

No Wii Remote reconnect or mid-game pairing run was performed after this change.
The earlier user run did not reconnect the Remote, and the Dolphin smoke test did
not connect one. Therefore hardware acceptance remains pending. The combined
std probe's USB IOS/HID checks found zero HID devices and do not test WPAD or
Remote input. Use the dedicated connection-hold probe and an app DOL for the
hardware test; see [controller hardware acceptance](../controller-hardware-acceptance.md).


## Controller project: shared packets and normalized input (2026-10-12)

The Bluetooth host now uses the production Switch packet module for low-power,
report-mode and read-only calibration setup. It checks matching acknowledgements
and SPI reply echoes before reporting a ready connection.
`send_report` returns false if the ACL queue rejects the packet. Host regressions
check packet bytes, channel, ACK/NACK/timeout and queue failure. Controller
extraction and normalized-state changes are recorded in
[the controller behavior ledger](libcontroller.md). Focused lifecycle and
calibration reviews found and corrected specific defects. Broader independent
review and real Wii evidence remain open. Ordered raw Bluetooth events now reach
the production manager through the shared queue. Low-level PAD/WPAD C APIs
retain their existing contract; the manager provides the error/reset policy.

## Independent Bluetooth/WPAD and Wiiuse re-review — 2026-10-15

Reviewers: `/root/review_bt_wpad` and `/root/review_wiiuse`.

Confirmed `ogc-bt-sync` issue: `HidHost::emit` drops events at queue capacity
and only records a diagnostic. The report path ignores the failed emit result.
This can lose an ordered input, connect, or disconnect event. Preserve the
event or return a production error that the caller handles without losing
state.

### BUG FIX handoff: ordered HID events under queue pressure — 2026-10-28

Implementer: `thread-current/root`. Independent re-reviewer:
`/root/bt_event_queue_rereview` — **PASS for the scoped event queue fix**
(2026-10-28).

`HidHost::emit` now retains each event. The Wii transport checks
`has_event_capacity` before it consumes either a completed HCI event read or
ACL read, and it does not submit another read while the ordered event queue is
full. A completed transfer stays owned by its request until the consumer makes
space. This preserves input order and keeps the USB transport from discarding
bytes that it has already accepted.

The regression `acl_connection_failure_preserves_all_events_when_queue_has_one_slot`
fills the queue to 63 reports, then feeds a real L2CAP connection response to
`HidHost::receive_acl`. The production handler disconnects the link and emits
both `Disconnected` and `Failed`; the test checks both events, their order, and
the unchanged reports. The queue reaches 65 because one accepted protocol
packet can publish more than one event. Backpressure stops later reads until
the consumer drains the queue. The independent reviewer confirmed that the
production poll path retains completed HCI and ACL requests and starts no new
reads at the high water mark. The reviewer also noted that direct callers of
the public `receive_event` and `receive_acl` methods must follow the
`has_event_capacity` contract; the method comments now state this requirement.

The C path handles each completed USB interrupt or bulk read inline through
`hci_event_handler` or `hci_acldata_handler` in
`../../../C-sources/libogc/lwbt/physbusif.c`. It has no equivalent Rust-side
bounded event FIFO that can silently discard an already processed event.
Rust keeps its ordered queue for the split transport/consumer architecture and
now applies backpressure before it consumes more input.

`cargo fmt -p ogc-bt-sync -- --check` and `cargo test -p ogc-bt-sync` passed;
the package ran 98 tests with no failures or ignored tests. The independent
review was source-only and ran no tests. No Wii controller check was performed
for this fix. The broader `ogc-bt-sync` source scope and runtime/hardware
behavior remain open.

The `WPAD_Disconnect` mismatch is closed by the scoped fix below. The separate
`wpad-sys` source scope remains incomplete.

Confirmed `wiiuse-rs` gaps: the `0x22` ACK path discards a failed decoded ACK
and does not request fresh status as C `event_ack` does
(`C-sources/libogc/wiiuse/events.c:90-103`). Expansion initialization submits
both enable writes and the calibration read without waiting for each prior
operation to complete; C advances from each completion callback
(`wiiuse/io.c:85-100`).

Scoped fix and independent source re-review passed on 2026-10-28. The Rust ACK
decoder now reads command and status after both button bytes, and a bounded
production command queue advances only after matching ACKs or complete status
and read replies. A failed ACK appends status recovery after pending work and
does not discard that work when the queue was full. The expansion setup state
submits each write and calibration read after the matching prior completion.
See the handoff above for tests and verification. This closes these two
findings only; it does not complete the broader `wiiuse-rs` review.

The bounded status offsets, ordinary read assembly, and the reviewed Nunchuk,
Classic/Wii U Pro, GH3, Balance Board, and Motion Plus report offsets matched
the inspected C handlers. The two findings below have scoped fixes and
independent source passes. Broader crate parity and hardware acceptance remain
open.

### BUG FIX handoff: WPAD disconnect completion — 2026-10-28

Implementer: `thread-current/root`. Independent re-reviewer:
`/root/bt_event_queue_rereview` — **PASS for the scoped disconnect fix**
(2026-10-28).

C `WPAD_Disconnect` validates the channel, returns `WPAD_ERR_NOT_READY` unless
WPAD is enabled, requests disconnect when the Remote is connected, and waits
3000 times for 50 microseconds for the used bit to clear. Rust previously
returned success after HCI command submission and returned
`WPAD_ERR_NO_CONTROLLER` for an inactive slot.

The production path now checks the enabled state first. An inactive slot or
absent adapter link returns success, as C does for an already disconnected
Remote. A request or host error remains an error. After request submission,
Rust polls the adapter's live slot link while yielding for the same 150 ms
bound. The host removes the link only after it processes the HCI
Disconnection Complete event. Rust returns success only after that state change.
If the bound expires while the link remains, Rust returns
`WPAD_ERR_TRANSFER`. This is a deliberate safety difference from C, which
returns success after the timeout even if the link remains. The Rust result
therefore reports final completion instead of request acceptance.

The production host-state query is exercised by
`disconnect_request_waits_for_adapter_completion`, which checks that the link
remains after request submission and clears only after the HCI completion
event. `cargo test -p ogc-bt-sync` passed 98 tests, `cargo xtask check-group
input` compiled 13 Wii runtime packages, and `cargo xtask test-host` passed 70
tests. The independent reviewer confirmed the timeout and state behavior from
source; they did not run tests. No physical Wii disconnect test was performed.
Broader `wpad-sys` source parity and hardware validation remain open.

### WPAD_Search active Wii Remote teardown — scoped fix — 2026-10-28

C `WPAD_Search` calls `__wpad_disconnect` for each active device, clears the
WPAD-visible active state, and queues periodic inquiry afterward
(`../../C-sources/libogc/wiiuse/wpad.c:1188-1207`). Rust now identifies active
Wii Remote HCI links, preflights room for all disconnect commands and inquiry,
queues each HCI disconnect before periodic inquiry, and leaves each transport
link alive until its disconnect event arrives. It hides the corresponding WPAD
input state while teardown is pending and discards late reports. Other HID
families remain connected. Queue-capacity failure leaves the HCI queue and
search state unchanged.

The host regression checks disconnect-before-inquiry order, queue exhaustion
without partial commands, preservation of other HID links, guest-key removal,
and registered-key retention. A queued `Connected` event for a slot already
pending search disconnect is ignored before it can restore WPAD state or add a
guest record. `cargo test -p ogc-bt-sync` passed 102 tests. `cargo check -p wpad-sys --tests --target
powerpc-unknown-linux-gnu` compiled the target-gated tests; it did not run them.
`cargo xtask check-group input` compiled 13 Wii runtime packages. Independent
source reviewer `/root/bt_event_queue_rereview` passed the active-link handoff.
No controller or hardware validation was performed.

Guest-key cache lifecycle now has a scoped implementation and independent
source pass. Broader `WPAD_Search` parity and hardware validation remain open;
`wpad-sys` remains `parity_blocked`.

C handles a guest `Link Key Notification` by updating the volatile
`__wpad_guest_keys` cache and does not call `BTE_WriteStoredLinkKey`; normal
pairing writes to the adapter key store (`../../C-sources/libogc/wiiuse/wpad.c:
1074-1112`). C `WPAD_Search` clears both guest records and guest keys, and
`WPAD_Shutdown` saves the working guest list (`wpad.c:1188-1207,1755-1762`).

The scoped Rust fix retains key origin, avoids adapter stored-key writes for
guest pairing, keeps the guest list in guarded volatile state, and updates it
only after a guest Wii Remote link is ready. It also ignores a queued old
connection event after search has marked that slot for disconnect. Init loads the configured list and
clears the pending SYSCONF value; search clears volatile guest keys and records
after successful search preflight; shutdown saves the current list and returns
configuration errors. The independent source review passed this scope. Rust
starts adapter close before saving the guest list, matching C's operation order.
Host tests cover key-cache behavior and ready-event content; PowerPC test code
compiled but did not run. SYSCONF persistence and real-controller behavior
remain unverified.
