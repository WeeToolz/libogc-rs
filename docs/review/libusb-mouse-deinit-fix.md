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

## Current behavior

The mouse returns the semaphore or join error if it cannot confirm that
the worker stopped. A failed wake skips the join so the caller does not
wait on a worker that can still be asleep. Either error keeps the device
descriptor, semaphore, and thread handle for a later shutdown attempt.
Initialization reports busy while the stop request is pending. After a
confirmed join, the existing cleanup runs and the process-wide USB service
stays active.

This is a deliberate safety difference from C, which continues to release
worker resources after either stop error. The host tests inject wake and
join errors through the same stop helper used by the production path. The
Dolphin lifecycle probe exercises successful shutdown without a physical
USB mouse. An attached mouse and a pending physical USB read need separate
hardware evidence. The older parity reviews describe the previous
behavior.
