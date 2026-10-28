# Independent re-review handoffs

Date: 2026-10-01. Reviewer: `/root`.

This note records independent source re-reviews of the listed implementation commits. The reviewer compared each change with its C path. Test status and hardware limits are taken from the implementer handoffs. No hardware verification is claimed.

## Accepted fixes

### `ogc-filesystem` async `read_dir` count copy

- Implementer: `/root/worker_b`; commit `2df4a63`.
- C `__isfsReadDirCB` writes the caller count only when the completion result is zero (`libogc/libogc/isfs.c:147-152`). Rust now calls `copy_read_dir_count_on_success`, which applies the same equality check. The caller callback still receives the original result. The helper and its call site match the C behavior.
- Regression tests were added. The PowerPC check passed, but the target test build lacked the `test` crate, so tests did not run. Other checked async query callbacks matched C. No additional confirmed `ogc-filesystem` mismatch remains in the recorded scope; positive-result availability from IOS is unverified.

### `ogc-conf` save signature

- Implementer: `/root/worker_a`; commit `4509018`.
- C `CONF_Init` requires `SCv0` in bytes 0 through 3 (`libogc/conf.c:73-88`), and `__CONF_WriteBuffer` writes the buffer unchanged (`:248-268`). Rust removed the checksum overwrite and now writes the unchanged buffer. Its regression test checks the signature and following data bytes. The implementation matches the C path.
- PowerPC check passed. The test crate compiled but the linker rejected the PowerPC test binary, so tests did not run. The embedded-NUL setting-text parser difference now has a parity fix pending independent re-review; see `libconf.md`.

### `ogc-runtime` ThreadId sentinel

- Implementer: `/root/worker_b`; commit `99c2d1a`.
- C reserves both zero and `0xffffffff` as invalid thread handles (`gc/ogc/lwp.h:52,60-63`; `libogc/lwpcompat/lwpc_thread.c:5-12`). Rust's constructor now returns `None` for `u32::MAX`; `NonZeroU32` already excludes zero. The safe constructor therefore cannot create the C null handle.
- PowerPC check passed. The target test build lacked the `test` crate, so the new regression test did not run. No other confirmed mismatch was found in the recorded runtime contract review.

### `ogc-irq` and `ogc-irq-sys` GameCube ACR registration

- Implementer: `/root/worker_c`; commit `be72ab9`.
- C's GameCube handler table does not contain IRQ 27. Rust's `index` now checks the detected console before both `k_irq_get` and `k_irq_set`, rejecting IRQ 27 on GameCube while retaining it on Wii. The helper test covers those cases and a common IRQ. The runtime behavior matches C.
- PowerPC test-code compilation passed; tests were not run. A later implementation adds a GameCube-specific target profile and removes the ACR names/slot under that target. That newer compile-time visibility fix is documented in `libirq.md` and still needs independent re-review.

### `wiikeyboard-sys` Return symbol

- Implementer: `/root/worker_a`; commit `ca80d33`.
- C maps HID keycode 40 to `KS_Return == 0x0d` (`libwiikeyboard/ukbdmap.c:87`; `gc/wiikeyboard/wsksymdef.h:55`). Rust now maps it to the same value for shifted and unshifted input. The translation change matches C.
- PowerPC check passed. Test compilation remains blocked by existing unsafe-call errors elsewhere in the test module, so tests did not run. Other keyboard findings remain open; see `libwiikeyboard.md`.

### `ogc-aram` reset and reinitialization

- Implementer: `/root/worker_b`; commit `20ec977`.
- C `AR_Reset` clears the initialized flag and a later `AR_Init` registers the ARAM handler and continues (`libogc/aram.c:95-110,207-210`). Rust now accepts the handler if it is the ARAM crate's own existing handler, while it continues to reject a different driver's handler. This permits `AR_Reset` followed by `AR_Init` and retains the stated ownership guard.
- The PowerPC check passed. The target test build lacked the `test` crate, so tests did not run. The remaining Rust defensive differences in IRQ ownership and ARQ initialization errors are recorded in `libaram.md` and are not unintended parity regressions.

### `ogc-usb-sys` v5 asynchronous close result

- Implementer: `/root/worker_c`; commit `d6336b2`.
- C `USB_CloseDeviceAsync` returns `cb(ret, userdata)` when the v5 close path handles the descriptor; it returns the close result when no callback exists (`libogc/usb.c:763-778`). Rust's v5 path now follows those return rules. The non-v5 IOS async-close path remains unchanged. The helper and call sites match C.
- PowerPC check passed. Test compilation is blocked by existing errors elsewhere in the USB test module, so tests did not run. Other USB findings remain open; see `libusb.md`.

### `mad-sys` decoder ABI and initial values

- Implementer: `/root/worker_b`; commit `2fcad9c`.
- C declares the three async fields as `s32` and initializes mode to `-1`, async `pid` to `0`, and async `in`/`out` to `-1` (`libmad/decoder.h:41-50`; `libmad/decoder.c:75-92`). Rust now uses three `i32` fields and those initial values. Its new PowerPC offset/size assertions match the 32-bit C layout, including the outer decoder fields.
- PowerPC check passed. The target test build lacked `std` and the `test` crate, so tests did not run. Other decoder and audio behavior findings remain open; see `libmad.md`.

### `ogc-alloc` interrupt-safe allocator lock

- Implementer: `/root/worker_a`; commit `3f9c2ce`.
- C's `_CPU_ISR_Disable` and `_CPU_ISR_Restore` use `PPCIrqLockByMsr` and `PPCIrqUnlockByMsr` (`gc/ogc/machine/processor.h:83-90`). C's LWP heap uses this pair around shared heap mutation, including error exits (`libogc/lwp_heap.c:16-35,53-93,107-156`). Rust now saves and masks external interrupts before acquiring its atomic allocator lock and releases the lock before restoring the saved state. The guard covers early returns as well as successful operations. The behavior matches the C critical-section ordering and preserves an already-disabled caller state.
- The six allocator host tests and PowerPC check passed. Formatting and scoped whitespace checks passed. The separate MEM2 arena-boundary finding remains open; see `liballoc-cache.md`.


## New implementation follow-ups awaiting re-review

### `ogc-stm` GameCube target availability follow-up

- C exposes STM only under `HW_RVL`. A new `powerpc-nintendo-gamecube` target enables the registered `cfg(gamecube)` switch; `ogc-stm` has no public implementation on that target and the compatibility facade omits its STM re-export. This addresses the prior target-availability finding but has not yet received independent re-review or target compilation in this environment.
