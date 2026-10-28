# USB Gecko flash program setup re-review

- Implementer: `worker_c`, commit `b891f6c`
- Independent reviewer: `/root`
- C source: `../../C-sources/libogc/libogc/usbgecko.c:329-346`
- Rust source: `crates/services/ogc-usbgecko/src/lib.rs`

C `usb_flashwrite` sends the three AMD byte-program setup commands before the target write: `AA@0x5555`, `55@0x2AAA`, and `A0@0x5555`. Rust's `flash_program_byte` now emits those commands in this order, then emits the requested target address and byte. It stops on the first failed command, as C's short-circuit sequence does. The new helper regression test checks all four address/data pairs.

I found no mismatch in the flash program setup fix. The implementation note records that the regression test did not compile because of existing unrelated unsafe test code, so the assertions have not run. Other documented differences, including the readback delay, finite waits, range handling, zero-length null-buffer behavior, and API shape, remain open. No hardware verification was performed.
