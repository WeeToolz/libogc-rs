# USB Gecko flash behavior audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Scope: independent static cross-check of remaining flash and wait behavior in `ogc-usbgecko`, excluding the already-fixed EXI device-ID check. No implementation changes, builds, tests, or hardware checks were performed. The shared `libusbgecko.md` note and ledger were not edited.

## Confirmed differences

### Flash write omits the AMD byte-program unlock sequence

C `usb_flashwrite` acquires the EXI lock, then for each byte sends four commands in order: `AA` at `0x5555`, `55` at `0x2AAA`, `A0` at `0x5555`, then the target address/value. It waits 20 microseconds and reads the byte back (`C-sources/libogc/libogc/usbgecko.c:329-350`). Rust `flashwrite` calls `flash_write_command` only for the target address/value, yields once, and reads back (`crates/services/ogc-usbgecko/src/lib.rs:489-524`). `flash_write_command` emits only the generic `0xE...` EXI word twice (`:87-120`). It does not issue the three setup commands. Readback cannot substitute for entering the flash byte-program operation. This is a confirmed flash-program behavior mismatch.

The software-ID entry/exit helpers in Rust do issue their own AA/55/90 and AA/55/F0 commands (`lib.rs:162-172`), but `flashwrite` does not call those helpers and those commands select software-ID mode, not the missing per-byte program setup.

### Program delay is not equivalent

After each successful program command sequence, C calls `usleep(20)` before readback (`usbgecko.c:343-345`). Rust calls `yield_thread()` with no duration argument (`lib.rs:504-512`). A scheduler yield does not encode or guarantee the same 20-microsecond wait. The source therefore leaves a static timing mismatch; hardware behavior was not tested.

### Zero-length flash requests differ

C initializes the result to success and skips its loop when `length == 0`; it does not dereference the buffer or validate `offset` (`usbgecko.c:315-327,329-350`). Therefore zero-length calls succeed even with a null buffer or an out-of-range offset, once the C lock operation returns. Rust rejects a null pointer before checking `length` (`lib.rs:444-447,489-492`), so null plus zero length fails. With a non-null buffer, Rust still checks `offset + length <= 0x80000` (`:449-455,494-500`); a zero-length request with an offset above the flash limit fails although C performs no address operation and returns success. These are confirmed return-value differences.

### Wait bounds differ, with operation-specific limits

C's EXI lock helper sleeps until `EXI_Lock` succeeds and has no timeout (`usbgecko.c:31-41`). Rust retries the lock and returns `None` after the configured deadline (`lib.rs:22-38`; transfer deadline is half a second and flash lock deadline is one second at `:14-16`). Thus Rust can return where C continues waiting for lock ownership.

C receive/send safe loops are bounded only by the optional retry counter; their raw `-1` wrappers can continue indefinitely while the peer does not progress (`usbgecko.c:173-200,202-229,231-290`). Rust transfer and flush loops add a half-second elapsed-time condition (`lib.rs:211-230,262-350`). This changes how long calls wait and the partial byte count returned when the peer stalls.

For flash methods, Rust's one-second timeout bounds only lock acquisition (`lib.rs:449-450,494-495,532-533`). The flash byte loops do not check a deadline after acquiring the lock (`:457-475,502-517,537-542`), so the general transfer timeout should not be described as a flash-operation timeout.

## Additional range behavior

Rust rejects an entire `offset + length` span before issuing EXI commands (`lib.rs:449-455,494-500`). C validates each individual command address (`usbgecko.c:61-62,78-79`) as it processes bytes (`:320-323,336-346`). A span that crosses the end of flash can therefore leave a successfully processed prefix in C, while Rust rejects it before any flash command. Rust's prevalidation is safer, but its side effects and return point differ.

## Result

The missing program setup sequence, unproven replacement of `usleep(20)` with one scheduler yield, zero-length/null/range return differences, and C-versus-Rust wait bounds are confirmed from the current source. No shared review note or ledger row was changed. No tests were run or compiled, and no hardware verification was performed.
