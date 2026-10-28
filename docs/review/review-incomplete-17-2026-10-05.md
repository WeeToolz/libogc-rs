# Review-incomplete reconciliation — 2026-10-05

Scope: the 17 crates that were marked `review_incomplete` after commit `71808b0`. This pass uses the checked-in Rust source and the available libogc reference tree. C is reference-only. Observable behavior and public ABI must match C except for explicitly documented safety improvements. Architecture-only differences are allowed.

## Result

| Crate | Result | Review conclusion |
| --- | --- | --- |
| `lwip-sys` | `parity_blocked` | The socket-option gaps found in this review are now fixed and await independent re-review: `net_getsockopt` returns C's unconditional `-EINVAL`, and `setsockopt` accepts nullable payloads while preserving the IOS result. The crate remains blocked by the separately recorded receive chunking/error-precedence scope. |
| `modplay` | `reviewed` | Current worker/DMA scheduling, silent initial buffers, fixed 5760-byte DMA length, shutdown/join behavior, parser/effect handling, and ownership are reconciled. Rust-only timing diagnostics are not C public API. |
| `modplay-capi` | `reviewed` | Grouped with `modplay`; current C API/ABI and worker/mixer scheduling scope has no confirmed non-safety mismatch. |
| `ogc-bt-sync` | `reviewed` | Recorded HCI/L2CAP disconnect, CID, queue, controller-capacity, continuation, and callback findings are fixed or reconciled. Remaining controller/IOS timing is runtime validation, not a source parity blocker. |
| `ogc-conf` | `reviewed` | SCv0 save, embedded-NUL parsing, CR-first terminator order, and negative IOS read/write mappings match C. Malformed-input checks are safety behavior. |
| `ogc-conf-sys` | `reviewed` | Constants/layout layer has no separate confirmed mismatch after the completed CONF review. |
| `ogc-fat` | `review_incomplete` | The available libogc tree has block-device transports but no authoritative libfat implementation source. See “Missing reference source” below. |
| `ogc-gx` | `parity_blocked` | Rust still exposes only a subset of the C GX public surface and does not model the full C shadow/dirty-state behavior for TEV/indirect/light and related state. |
| `ogc-gx-sys` | `parity_blocked` | The sys/API surface remains substantially smaller than C `gx.h`; previous struct declaration fixes do not close the missing constants/functions/state surface. |
| `ogc-stm` | `reviewed` | STM's own public scope matches C, including Wii-only availability and lifecycle/result flow. Stricter open/hook failure reporting and leak-avoidance cleanup are safety deviations. The separate GameCube `ogc-ipc` dependency failure is not an STM parity blocker. |
| `ogc-sys` | `reviewed` | No separate low-level ABI mismatch is confirmed. IPC staging/variadic blockers belong to `ogc-ipc`, not this crate. Rust-owned assembly/runtime symbols do not depend on libogc C at link time. |
| `ogc-sysbase` | `reviewed` | Rust owns the devoptab symbols/table, uses the 35-slot layout, and current target type/layout checks match the inspected devkitPPC ABI. Out-of-range default-device rejection is a safety deviation. |
| `ogc-system` | `reviewed` | Startup/reset/arena/SRAM/alarm source review has no confirmed non-safety mismatch. Arena2 publication is through the Rust-owned sys ABI boundary. |
| `ogc-usbgecko` | `reviewed` | Command sequence, callback-backed EXI wait/wakeup/retry, receive zeroing, flush signature, range behavior, and delays are reconciled. Real EXI contention remains hardware validation only. |
| `ogc-video-sys` | `reviewed` | The recorded VIDEO sys/API, timing/register/framebuffer, render-mode constants, and non-interlace origin work is complete for the C surface reviewed. |
| `wiikeyboard-sys` | `parity_blocked` | C stdin handling drains the same public event FIFO, while Rust duplicates symbols into a separate stdin queue. C also performs the successful-open all-LED flash and 200 ms delay before the connected event; Rust omits it. |
| `wpad-sys` | `parity_blocked` | C accepts `WPAD_CHAN_ALL` for Rumble, ControlSpeaker, and SetIdleThresholds; current Rust PowerPC wrappers reject it. Threshold acceptance/normalization also differs. The 8-byte calibration read remains an accepted bounds-safety correction to C's out-of-bounds use. |

## Missing reference source

Only `ogc-fat` needs more reference implementation source before parity can be established.

Required source: the **matching devkitPro/libfat source tree and version** used with this libogc/toolchain. A complete libfat checkout is preferred so the review can cover the actual FAT implementation rather than infer behavior from headers or a binary archive. The needed implementation scope includes:

- volume/partition discovery and BPB/FAT mount logic;
- file open/create/read/write/seek/truncate/flush behavior;
- directory traversal and mutation;
- metadata/stat and free-space behavior;
- devoptab/path dispatch;
- cache and media lifecycle/mount/unmount behavior;
- error/result mapping for the public libfat API.

The existing libogc Wii SD, GameCube SD, USB-storage, and `DISC_INTERFACE` sources are already present and only describe block-device transport. They are not a FAT filesystem reference implementation. Nintendont FatFs and prebuilt `libcustomfat.a` files are not substitutes for the matching libfat source.

## Named blockers found by this pass

### `lwip-sys`

The C `net_getsockopt` implementation returns `-EINVAL` for every call. Current Rust instead services several socket options from locally tracked state. This changes a public return value and output data. C `net_setsockopt` also allows a null `optval` when `optlen` is zero; Rust rejects the null pointer before that C-compatible zero-length case. These are observable non-safety differences.

### `ogc-gx` and `ogc-gx-sys`

The existing GX note already records that the full shadow/dirty-state machine and API surface were not ported. The full pass confirms this is a parity blocker, not merely incomplete review. Examples of C APIs absent from the current Rust surface include `GX_InitLightDistAttn`, `GX_InitLightAttn`, `GX_InitLightAttnA`, `GX_InitLightAttnK`, and `GX_InitLightSpot`, in addition to wider GX constants/functions. Missing public API and state behavior must be ported from the existing `gc/ogc/gx.h` and GX implementation sources; no additional C checkout is required.

### `wiikeyboard-sys`

C's stdin worker obtains characters by consuming `KEYBOARD_GetEvent`, which is the same FIFO exposed to callers. Rust places each decoded event in the public event queue and separately mirrors the symbol into `STDIN_KEY_QUEUE`; enabling stdin therefore does not create the same competition/order between stdin and public event consumers. C also flashes Num/Caps/Scroll LEDs on successful open, sleeps 200 ms, turns them off, and then publishes the connected event. Current Rust publishes connection without that sequence.

### `wpad-sys`

C recursively applies `WPAD_Rumble`, `WPAD_ControlSpeaker`, and `WPAD_SetIdleThresholds` to all controller channels when passed `WPAD_CHAN_ALL`. Current Rust PowerPC wrappers validate the value as a single channel first and return `WPAD_ERR_BAD_CHANNEL`. C also normalizes any negative button idle threshold to `-1` and stores the other threshold arguments directly, while Rust rejects values below its accepted floor. These are public, observable result/state differences.

## Validation boundary

This pass is a source reconciliation. It does not replace hardware validation where a reviewed crate's note calls for it. Hardware-only evidence gaps do not keep a crate in `review_incomplete` when the source parity question is otherwise resolved. Conversely, a named source mismatch moves a crate to `parity_blocked` even if it has not yet been reproduced on hardware.
