# USB mouse deinitialization fix

- Implementer: `/root/worker_a`
- Independent review: required; reviewer must be assigned by the coordinator.
- Finding: Rust `mouse::deinit` called the general USB `deinitialize` function after it cleaned up the mouse. C `MOUSE_Deinit` does not shut down the general USB service.
- C reference: `../../C-sources/libogc/libogc/usbmouse.c`, `MOUSE_Init` and `MOUSE_Deinit` at lines 316-379.
- Rust implementation: `crates/services/ogc-usb/src/lib.rs`, `mouse::init` and `mouse::deinit`.

## Change

C `MOUSE_Init` calls `USB_Initialize`, then creates mouse resources. C `MOUSE_Deinit` stops and joins the mouse thread, closes the mouse, flushes queued events, frees mouse resources, destroys its semaphore, and clears the mouse initialized flag. It does not call `USB_Deinitialize`.

Rust `mouse::init` calls the general `initialize` function. Rust `mouse::deinit` now keeps the matching mouse cleanup and clears the mouse initialized flag, but does not call the general `deinitialize` function. Other users can continue to use the USB service after mouse shutdown.

The PowerPC unit test `successful_mouse_cleanup_clears_mouse_initialized_state` covers the pure mouse-state completion step. The general USB service is left active by the `deinit` path, which no longer invokes `deinitialize`.

## Validation

`cargo check -p ogc-usb --target powerpc-unknown-linux-gnu` passed. `cargo test -p ogc-usb --target powerpc-unknown-linux-gnu --no-run` did not compile the test crate because an existing test calls unsafe `try_close_device_async` without an unsafe block (`src/lib.rs:1175`); tests did not run. `cargo fmt --check -p ogc-usb` reports a pre-existing formatting difference on the crate-level `compile_error!` line (`src/lib.rs:4`). No hardware verification is claimed. Other `ogc-usb` findings remain open.

## Current disposition (2026-10-05)

The bounded mouse cleanup findings are resolved by later commits and independent reviews recorded in `docs/review/libusb.md`: Rust continues cleanup after close, semaphore-destroy, semaphore-post, and thread-join errors. The fix leaves the process-wide USB service active. The tests for these error paths did not run because the PowerPC test binary could not be linked with the installed linker. The broader `ogc-usb` review remains open for the other findings in the main note.
