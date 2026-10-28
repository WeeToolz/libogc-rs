# Independent `wpad-sys` Findings Cross-check

Status: **historical/superseded** for the four findings below. HEAD `4c36294` has six slots, channel-5/all-channel handling, and handshake-gated probe readiness. The original evidence is retained for review history.

Date: 2026-10-01

Reviewer: `/root/reviewer_5`  
Existing reviewer: `/root/reviewer_6`; implementer recorded in the main note as `notOrrytrout`.

Scope: independent static verification of the four findings in `docs/review/libwpad-sys.md`, plus nearby channel validation and API behavior. No implementation files, main review note, or ledger rows were changed. No builds, tests, or hardware verification were performed.

## Sources inspected

- Rust: `crates/services/wpad-sys/src/lib.rs`, `crates/services/wpad-sys/src/powerpc_backend.rs`.
- C header: `../../C-sources/libogc/gc/wiiuse/wpad.h`.
- C implementation: `../../C-sources/libogc/wiiuse/wpad.c`.

## Existing findings

### Channel 5 is rejected: confirmed

C defines `WPAD_CHAN_UNKNOWN` as 5 and `WPAD_MAX_DEVICES` as 6 (`gc/wiiuse/wpad.h:39-48`). The normal channel bounds in C include 0 through 5; `WPAD_SetMotionPlus` checks `chan >= WPAD_MAX_DEVICES` (`wiiuse/wpad.c:1541-1555`), and `WPAD_Probe` uses the same upper bound (`:1609-1645`). Rust's `valid_chan` accepts only 0 through `WPAD_BALANCE_BOARD` (0 through 4) (`lib.rs:614-620`). The same helper guards the common `set_motion_plus`, host-compatible `probe`, and many other channel APIs. On PowerPC, `powerpc_backend::probe` and the backend setter also call `valid_chan` or receive its validated index, so channel 5 remains unavailable there. This is a broad public channel range mismatch, not only a probe issue.

### MotionPlus all-channel request: confirmed

C `WPAD_SetMotionPlus(WPAD_CHAN_ALL, enable)` iterates all channel values from 0 up to `WPAD_MAX_DEVICES - 1` and calls the per-channel operation (`wiiuse/wpad.c:1541-1552`). Rust `set_motion_plus` validates a single channel before dispatch (`lib.rs:1070-1082`); `WPAD_CHAN_ALL` is `-1`, so both the PowerPC and host-compatible paths return `WPAD_ERR_BAD_CHANNEL`. The PowerPC backend `set_motion_plus` accepts a `usize` slot and only operates on one Wii Remote (`powerpc_backend.rs:685-708`), with no all-channel forwarding.

### PowerPC backend capacity: confirmed

C allocates WPAD per-device arrays using `WPAD_MAX_DEVICES` and initializes Wiiuse with that six-device count (`wiiuse/wpad.c:93-95,1288`). Rust declares six public device slots but sets `MAX_POWERPC_SLOTS` to four (`powerpc_backend.rs:109`) and its slot accessors reject slot indices 4 and higher (`:274-287`). Public APIs admit channel 4 through `valid_chan`, so they can report a different result after passing channel validation but fail backend slot lookup; channel 5 is already rejected by `valid_chan`. The device-capacity mismatch is present.

### Probe readiness during Wii Remote handshake: confirmed

C `WPAD_Probe` returns `WPAD_ERR_NOT_READY` if WPAD is disabled, and for a connected remote returns success only after `WIIMOTE_STATE_HANDSHAKE_COMPLETE` (`wiiuse/wpad.c:1609-1645`). Rust sets `WPAD_CONNECTED` as soon as the HID connect event arrives and then starts `initialize_wii_remote` (`powerpc_backend.rs:1286-1308`). Its PowerPC `probe` checks only the connected flag before returning success and the current expansion field (`:1362-1384`); it does not check `WPAD_GLOBAL_STATUS` or a handshake-complete state. During the initialization interval it can return success where C returns not-ready. Note that the Rust path does separately delay decoded data through initialization steps; this does not add the missing probe readiness check.

## Nearby channel and API checks

- C `WPAD_SetDataFormat` supports `WPAD_CHAN_ALL` and forwards across all six devices (implementation around `wiiuse/wpad.c:1500-1538`). Rust host compatibility handles `WPAD_CHAN_ALL` (`lib.rs:1055-1068`), but the PowerPC backend also forwards across `0..WPAD_MAX_DEVICES` (`powerpc_backend.rs:1335-1359`) and then encounters the four-slot limit: channels 4 and 5 are not applied. This is another consequence of the confirmed capacity mismatch, not a separate finding.
- C `WPAD_SetVRes`, `WPAD_SetDataFormat`, `WPAD_SetMotionPlus`, and selected rumble/event APIs contain their own all-channel loops (`wiiuse/wpad.c:1396-1405,1426-1438,1503-1513,1541-1552,1575-1579,1800-1880`). Current Rust behavior varies by API. This audit did not enumerate every API's ALL semantics exhaustively, but the two examples checked above establish the reported `SetMotionPlus` difference and a related `SetDataFormat` capacity consequence.
- C probe also guards on WPAD initialization state, while the Rust probe's PowerPC implementation does not test the global enabled/initializing state. If Rust's connected state can persist or be observed after shutdown or before initialization, this could be an additional readiness mismatch. I did not establish a reachable call sequence for that case, so this is not recorded as a confirmed separate finding here.

## Result

All four findings in the existing `libwpad-sys.md` were independently confirmed against current source. No separate additional confirmed finding was established in the limited nearby API pass. The main review note and ledger remain unchanged; `wpad-sys` should remain pending with findings open.

## 2026-10-02 superseding source note

The findings recorded by this older audit no longer describe the current Rust source. The current implementation has six PowerPC device slots, accepts channel 5, dispatches all-channel operations across six slots, and gates `WPAD_Probe` readiness on the completed initial handshake. See `libwpad-sys.md` for the implementation handoff and current status. Hardware validation remains open.
