# Independent `ogc-usbgecko` source audit

Reviewer: `/root/reviewer_6`  
Implementer of the device-ID change: `/root/worker_a`, commit `c3e9eea`.  
Scope: reported device-ID validation, flash program sequence, wait behavior, and flash ranges.  
Validation: source inspection only; no builds or tests run. No hardware verification was performed.

## Device-ID validation: current worktree matches C

C `usb_isgeckoalive` calls `EXI_GetID`, returns false if that call fails or the returned ID is nonzero, then acquires the EXI lock and sends `0x9000` (`../../C-sources/libogc/libogc/usbgecko.c:150-170`). Current Rust `is_gecko_alive` calls `exi::read_id`, returns unless it is `Some(0)`, then acquires the Gecko lock and sends the same probe command (`crates/services/ogc-usbgecko/src/lib.rs:245-259`). The previously reported ID validation difference is fixed in the current worktree and passes this static comparison.

## Confirmed remaining differences

### Flash byte programming omits C's unlock/setup sequence

C `usb_flashwrite` sends the AMD-style sequence `AA@0x5555`, `55@0x2AAA`, `A0@0x5555`, then writes the target byte for each byte (`usbgecko.c:329-346`). Rust's `flashwrite` calls only `flash_write_command` for the target address and value (`lib.rs:489-517`). The helper encodes and sends that same generic `0xE0000000 | (address << 9) | (data << 1)` word twice (`lib.rs:87-120`); it does not send the three setup commands. Readback verification cannot replace the missing programming setup. This is a confirmed command-sequence mismatch.

### Lock and transfer waits have finite deadlines in Rust but can continue indefinitely in C

C initializes a per-channel wait queue and loops in `EXI_Lock`, sleeping until the lock is acquired (`usbgecko.c:21-41`). Its flush loop drains until receive reports no byte (`:141-148`), and its default and safe transfer loops continue until the requested bytes arrive, a transfer fails, or a caller-supplied nonnegative retry count expires (`:173-290`). Default APIs pass `-1`, which means no retry limit. Rust's lock loop returns `None` after a finite tick deadline (`lib.rs:23-37`); flush and transfer loops also stop at `USB_TRANSFER_TIMEOUT_TICKS` (`:15,211-230,262-356`). For the same blocked lock or nonresponding peer, Rust can return before C. This difference is directly visible in control flow.

### Flash range prevalidation changes side effects and edge-case results

C validates each individual flash command address against `0x7FFFF` (`usbgecko.c:56-70,73-89`). `usb_flashread` and `usb_flashwrite` loop byte by byte and stop when a command fails (`:315-350`), so a range that crosses the end can process its in-range prefix before returning failure. Rust checks the complete interval first and rejects `end > 0x80000` before issuing any flash command (`lib.rs:449-455,494-500`). The Rust no-partial-operation behavior is safer, but it does not match C's side effects.

There is also a zero-length edge: C initializes the result to success and does not enter the command loop when `length == 0`, so it returns success even with a null buffer or an offset beyond the flash end (`usbgecko.c:315-326,329-350`). Rust rejects a null buffer before length handling (`lib.rs:444-447,489-492`); with a non-null buffer it still rejects an offset above the end through the interval check (`:449-455,494-500`). For a non-null pointer and in-range offset, both return success without issuing a byte command.

### Rust does not preserve C's explicit 20 microsecond program delay

C calls `usleep(20)` after programming each byte and before readback (`usbgecko.c:338-345`). Rust calls `yield_thread()` at that point (`lib.rs:504-513`). The Rust scheduler implementation requeues the current thread and switches (`crates/core/ogc-lwp-driver/src/runtime.rs:1193-1203`); it does not encode a minimum 20 microsecond delay. Thus the timing contract differs statically. Actual flash compatibility was not verified.

## Handoff

The ID-validation finding is resolved by the current Rust source. Flash program setup, wait deadlines, range behavior, and the C 20 microsecond delay remain confirmed differences. Keep `ogc-usbgecko` open with those findings. No implementation code was changed.
