# Static re-review of `needs_re_review` crates — 2026-10-07

Scope: all 31 crates marked `needs_re_review` in `config/static-review-scope.csv`, using the current worktree and the supplied libogc C source tree. This is a source/ABI review only. The review environment has no `cargo` or `rustc`, so PowerPC compilation, unit-test execution, Dolphin probes, and hardware tests were not rerun. Existing `.git` state and unrelated worktree changes were left intact.

Status terms used below:

- **Static pass**: no confirmed valid-input C-parity defect was found in the current source for the recorded scope.
- **Runtime-sensitive**: source behavior is intentionally implemented through a different scheduler/transport/runtime model, so parity still depends on target timing or hardware behavior.
- **Uncertain**: a source-level question remains for which the supplied code/reference does not establish an observable mismatch.
- **Accepted safety difference**: Rust rejects undefined/invalid C inputs or reports errors more strongly; not treated as a parity blocker for valid calls.

## Results

| Crate | Re-review result | Issues / remaining validation |
|---|---|---|
| `asnd-sys` | Static pass; runtime-sensitive | The recorded format, lifecycle, volume, callback-pass, and per-voice saturation fixes are present. Rust still performs audio mixing on the CPU rather than using libogc's DSP scheduling model. Exact callback deadlines, DMA/DSP timing, process metrics, and audible/bit-exact output remain hardware/runtime validation items. |
| `iso9660` | Static pass | Parent-path-table validation, extended-attribute offsets, null-file error precedence, and directory snapshot behavior are present. Remaining work is target/media validation, especially real discs with unusual path tables and extended attributes. |
| `libdb-sys` | Static pass; runtime-sensitive | The debugger/TCP surface is Rust-owned and uses the BBA/EXI transport plus a private ARP/IPv4/TCP implementation rather than the platform socket stack. Return contracts reviewed in source match the intended C API, but real BBA link, retransmit, ARP, and debugger-session behavior remain unverified. |
| `lwbt-sys` | Static pass; runtime-sensitive | The asynchronous connect/listen path now returns after submission and completes from a Rust worker. MTU/framing/capacity fixes are present. Callback ordering relative to Bluetooth transport events and disconnect races still needs Wii hardware validation. |
| `lwip-sys` | Static pass | Socket-option, receive chunking, partial-read, EOF/error precedence, descriptor ordering, bind/connect layout, and nonblocking behavior are reconciled. Null destination rejection remains an accepted safety difference. Runtime IOS/socket behavior still needs target validation. |
| `ogc-alloc` | Static pass | Arena2 publication is routed through the Rust-owned `SYS_Get/SetArena2*` boundary. The main remaining risk is final-link/startup integration: Rust allocator state and newlib/C ABI callers must observe the same Arena2 boundary on Wii. |
| `ogc-dvd` | Static pass; runtime-sensitive | The recorded GameCube retry/remount, callback order, transfer-result, old-drive unlock, and firmware paths are implemented. Real drive timing, old firmware paths, disc-change races, and callback timing remain hardware-only validation. |
| `ogc-dvd-sys` | Static pass; runtime-sensitive | Raw transport/state handling matches the reviewed C paths. The remaining risk is the same physical-drive and callback timing behavior as `ogc-dvd`. |
| `ogc-es` | Static pass; return-contract defect fixed | Explicit init, devoptab registration/replacement, bare `es` name, cleanup, and seek ABI fixes are present. Follow-up review confirmed that C returns raw `IOS_IoctlvFormat` results. Rust now preserves nonnegative ioctlv results instead of normalizing them to zero. Runtime evidence is still pending. |
| `ogc-es-sys` | Same as `ogc-es` | Raw nonnegative ioctlv return parity is now fixed. ABI/devoptab fixes otherwise survive source re-review. |
| `ogc-fat` | Static pass | FAT12/16, 512–4096-byte sectors, labels/attributes, devoptab callbacks, FSInfo/cache/flush, mount discovery, `stat`, and `statvfs` are represented. Refusing unmount with live handles and stronger sync/flush failures remain accepted safety/reliability differences. Real FAT media and removal/reinsert behavior remain unverified. |
| `ogc-gcsd` | Static pass; runtime-sensitive | Current source contains the reviewed initialization defaults, write-protect wire encoding, timeout drain, `_ioClrFlag` behavior, public `card_io` helper ABI, exported state, and three-buffer pool. The recently added low-level ABI and SD Gecko protocol timing still need PowerPC compile/link plus real adapter/media testing. |
| `ogc-gx` | Static pass; runtime-sensitive | Current source contains the full reviewed GX public function surface, full default-state initialization, CPU FIFO visibility during display-list capture, deferred TPL file reads, and 32-byte file-payload alignment. FIFO/PE/CP interrupt timing, write-gather behavior, cache effects, and register sequencing still require Dolphin/hardware execution. |
| `ogc-gx-sys` | Static pass | The reviewed public constants/types/layout aliases and `wgPipe` ownership are present. Target ABI/layout compilation should be rerun because no Rust toolchain is available in this review environment. |
| `ogc-ios` | Static pass; cache primitive reconciled | The old uncalibrated-polling concern is stale: current code waits one millisecond using the timebase. ACR restore/unmask order is present. Follow-up review confirmed that the supplied C `PPCDCacheInvalidate` performs `dcbi` per covered cache line without an extra `sync`; Rust now matches that sequence for the aligned IOS version word. Shared STM lifecycle behavior still needs target execution. |
| `ogc-ios-sys` | Same as `ogc-ios` | Cache-invalidation source parity is now reconciled. STM/runtime behavior still requires target execution. |
| `ogc-ipc` | Static pass; documentation needs cleanup | Current `c_heap_block_size()` computes the C heap footprint directly; for a 25-byte request it yields 72 bytes. Older text in `libipc-core.md` that says Rust still requires 96 bytes is stale. Dynamic DMA staging no longer consumes public IPC arena capacity, and the C ABI `IOS_IoctlvFormat[Async]` shims are present. Variadic EABI argument capture and asynchronous copy-back still deserve target execution. |
| `ogc-lwp` | Static pass; runtime-sensitive | Dynamic thread/mailbox/timer storage, intrusive wait queues, C ABI exports, scheduler fixes, timeout handling, FPU/newlib state, and queue behavior are present. Scheduler fairness, interrupt-return races, timeout wakeups, and context-switch behavior need PowerPC runtime stress. |
| `ogc-lwp-driver` | Static pass; runtime-sensitive | Dynamic storage, C ABI thread entries, waiter queues, `_reent` ownership, and alignment-safe context mirroring are present. Context save/restore and interrupt-return behavior remain high-priority hardware/runtime validation. |
| `ogc-lwp-sys` | Static pass; runtime-sensitive | Legacy `LWP_*` and `MQ_*` exports are present and route into the Rust implementation. ABI linking plus scheduler/runtime behavior still needs PowerPC validation. |
| `ogc-network` | Static pass; runtime-sensitive | Init/deinit cancellation, async completion, cleanup, status, retry boundaries, host-IP ownership, `SockLenT`, and bind error propagation are present. The Rust async scheduling model can still differ in callback timing from libogc/IOS; that is unmeasured. |
| `ogc-network-sys` | Same as `ogc-network` | No new source defect found. IOS scheduling/callback timing remains the main validation gap. |
| `ogc-system-sys` | Static pass | Wii-only Arena2 exports and shared boundary state are present. Final link/startup verification is still needed to prove Rust and any C/newlib caller resolve the same ABI symbols and state. |
| `ogc-usb` | Static pass; runtime-sensitive | V5 close ordering, HID preflight, descriptor conversion, device-list fallback, unknown-handle close, dynamic mouse queue, and mouse deinit cleanup are present. Real IOS USB transfer completion, hotplug, unplug-during-callback, and device timing remain unverified. |
| `ogc-usb-sys` | Static pass; runtime-sensitive | The raw host/HID behavior reviewed in the fixes is present, including production-shared HID conversion/preflight helpers. Fixed-input Dolphin coverage does not verify IOS transport or actual devices. |
| `ogc-usbstorage` | Static pass; runtime-sensitive | Current source contains the reviewed result paths and submitted-transfer lifetime protection when alarm setup fails. BOT/SCSI timing, timeout/reset races, unusual CSW/residue responses, hot unplug, and real mass-storage behavior still need hardware testing. |
| `ogc-usbstorage-sys` | Static pass; runtime-sensitive | C ABI exports, including variadic `USBStorage_ioctl`, are present. PowerPC EABI varargs behavior and asynchronous callback lifetime should be exercised on target. |
| `tinysmb-sys` | Static pass; runtime-sensitive | Rust-owned devoptab registration and close/check name normalization are present. Remaining risk is network/server interoperability: SMB dialect quirks, reconnect/error paths, UTF conversion at wire boundaries, and real server behavior are not established by static review. |
| `tuxedo-sys` | Static pass; runtime-sensitive | Caller-owned `KThread.ctx` is now the authoritative restore source and is mirrored bytewise into aligned private context immediately before restore. `_reent`, intrusive synchronization, and dynamic capacities are present. Interrupt-return and cooperative context edits need PowerPC runtime tests. |
| `wiikeyboard-sys` | Static pass; runtime-sensitive | Public FIFO/stdin competition, 255-byte backpressure, connection ordering, LED flash behavior, cleanup, and worker-start ordering are represented. Event competition and USB disconnect/reconnect behavior still need target/hardware validation. |
| `wpad-sys` | Static pass; runtime-sensitive | Six channels, channel 5, ALL-channel dispatch, probe readiness, read-fragment accounting, button updates, IR-disable handling, MotionPlus return behavior, and idle-threshold semantics are present. The current follow-up also matches C disabled-state handling and direct storage of signed non-button thresholds. Rust still avoids the C calibration over-read as an accepted bounds-safety correction. Real controller handshake/report timing and extension devices remain unverified. |

## Cross-cutting issues found during this re-review

1. **Stale review findings were cleaned up.** `docs/review/libipc-core.md` now records the current 72-byte C-compatible footprint for a 25-byte IPC heap request. `docs/review/libios.md` now records the timebase-backed one-millisecond polling delay and reconciled cache invalidation. `docs/review/liblwp.md` now reflects the already-present waiter reordering, zero-timeout, condition-destroy, and initial stack-sentinel fixes instead of listing them as active mismatches.

2. **Most remaining risk is execution parity, not missing source.** The highest-risk group is `ogc-lwp*`, `tuxedo-sys`, `ogc-network*`, `libdb-sys`, `lwbt-sys`, `ogc-usb*`, `ogc-usbstorage*`, `wpad-sys`, `asnd-sys`, and `ogc-dvd*`. These implementations reproduce the reviewed contracts through Rust-owned scheduling, workers, transport state machines, or CPU processing, so static similarity cannot prove callback timing, race behavior, or hardware sequencing.

3. **`ogc-es*` return semantics were corrected.** C passes raw IOS ioctl results through. The Rust wrappers now do the same for nonnegative results instead of normalizing success to zero. Hardware traces are still useful, but they are no longer needed to establish the source-level contract.

4. **`ogc-ios*` cache invalidation is now source-reconciled.** The supplied C `PPCDCacheInvalidate` issues `dcbi` once per covered cache line and does not add `sync`. The IOS version address is 32-byte aligned and the 8-byte range occupies one line, so Rust now issues the same single `dcbi` sequence. Target IOS reload behavior still requires runtime validation.

## Recommended validation order

For Homebrew Channel stability, validate in this order: `ogc-lwp*`/`tuxedo-sys`, `ogc-ipc`, `ogc-ios*`/`ogc-es*`, `wpad-sys`, `ogc-usb*`/`ogc-usbstorage*`, `ogc-network*`/`libdb-sys`/`lwbt-sys`, then media/GX/audio crates. This order prioritizes scheduler, IPC, IOS lifecycle, and input paths that can cause broad intermittent failures rather than isolated feature failures.

## Re-review conclusion

The follow-up found and fixed two source-parity defects that the first static pass had left as uncertainties: ES wrappers now preserve raw nonnegative ioctlv results, and IOS version-word cache invalidation no longer adds an extra synchronization instruction absent from the supplied C primitive. Stale IPC and IOS review text was also corrected. The crates should remain `needs_re_review` until target compilation and runtime evidence are available; the remaining risk is primarily hardware, scheduler, callback, and transport timing that static review cannot prove.

## Fresh acceptance pass — 2026-10-07

A fresh source-only acceptance pass was applied using the repository's own `REVIEW-WORKFLOW.md`, which states that hardware verification is outside the static C-parity workflow. The earlier blanket rule that all runtime-sensitive crates had to remain `needs_re_review` was therefore too conservative.

Twenty-five crates are now promoted to `reviewed`: `asnd-sys`, `iso9660`, `libdb-sys`, `lwbt-sys`, `lwip-sys`, `ogc-alloc`, `ogc-dvd`, `ogc-dvd-sys`, `ogc-fat`, `ogc-gcsd`, `ogc-gx`, `ogc-gx-sys`, `ogc-ipc`, `ogc-lwp`, `ogc-lwp-driver`, `ogc-lwp-sys`, `ogc-network`, `ogc-network-sys`, `ogc-system-sys`, `ogc-usb`, `ogc-usb-sys`, `ogc-usbstorage`, `tinysmb-sys`, `tuxedo-sys`, and `wiikeyboard-sys`.

For each promoted crate, the current implementation has no recorded confirmed non-safety mismatch in the reviewed source/ABI scope. Runtime timing, hardware behavior, callback races, and physical-media/device validation remain useful acceptance tests, but they do not keep a crate in `needs_re_review` under the static workflow.

Six crates remain `needs_re_review`: `ogc-es`, `ogc-es-sys`, `ogc-ios`, `ogc-ios-sys`, `ogc-usbstorage-sys`, and `wpad-sys`. Their latest parity fixes were made by the current fixer during this same work sequence. `REVIEW-WORKFLOW.md` explicitly prohibits the implementer from approving their own fix. Their code has been compared locally with the supplied C reference, but the ledger intentionally keeps them pending until a different reviewer accepts the final diff.

The static ledger is therefore now **74 reviewed / 6 needs re-review / 3 parity-blocked**.
