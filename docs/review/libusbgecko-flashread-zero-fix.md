# USB Gecko zero-length flash fix

- Implementer: `/root/worker_a`
- Independent review: required; assign a reviewer other than the implementer.
- Finding: Rust rejected null buffers before it checked the flash length. C's flash read and write loops do not access the buffer when length is zero and return success.
- C reference: `../../C-sources/libogc/libogc/usbgecko.c`, `usb_flashread` and `usb_flashwrite` at lines 315-351.
- Rust source: `crates/services/ogc-usbgecko/src/lib.rs`, `flashread` and `flashwrite`.

## Change

C initializes its result to 1, acquires the EXI lock, then processes bytes only while `length` is nonzero. With length zero, it makes no buffer access and returns 1. Rust now accepts a null buffer when length is zero, still validates the EXI channel and acquires the Gecko lock, then returns 1 before address checks or slice construction. Positive lengths still require a non-null buffer.

The regression test checks the shared buffer rule and the zero-length success result used by both flash APIs. It does not access EXI hardware.

## Validation

`cargo check -p ogc-usbgecko --target powerpc-unknown-linux-gnu`, `cargo fmt --check -p ogc-usbgecko`, and scoped `git diff --check` passed. `cargo test -p ogc-usbgecko --target powerpc-unknown-linux-gnu --no-run` is blocked by 14 pre-existing test errors: unsafe function-pointer casts and unsafe calls in other test functions in `src/lib.rs`. The new regression test reported no error, but tests did not run. No hardware verification is claimed. Other `ogc-usbgecko` differences remain open.
