# USB storage ABI audit

Reviewer: reviewer-6. Implementer: N/A (independent static audit).

## Result

The C data structures and field widths match the Rust target layouts on Wii PowerPC. Two public function signatures do not preserve the C declaration contract: `USBStorage_Deinitialize` returns `void` in C but the Rust `deinitialize` returns `i32`, and C `USBStorage_ioctl` is variadic while Rust `ioctl` takes a fixed `*mut RawDeviceCommand`. Also, the Rust functions are ordinary Rust functions rather than C ABI exported symbols, so the Rust crate is not a drop-in C ABI surface.

## Evidence

- C `usbstorage_handle` fields and order are declared in `C-sources/libogc/gc/ogc/usbstorage.h:28-51`. Rust `UsbStorageHandle` uses the same order and target-sized equivalents (`crates/services/ogc-usbstorage-sys/src/lib.rs:40-58`). On Wii PowerPC, `mutex_t` and `syswd_t` are both `u32` (`C-sources/libogc/gc/ogc/mutex.h:48-51`, `system.h:163-166`); pointers are 32-bit. The field types therefore have matching widths and alignment.
+- C `raw_device_command` has a 16-byte command, three one-byte fields, a pointer, and `size_t` (`usbstorage.h:56-63`). Rust `RawDeviceCommand` has the corresponding fields and `usize` data length (`ogc-usbstorage-sys/src/lib.rs:60-69`), matching on 32-bit PowerPC.
+- The C `DISC_INTERFACE` callback signatures use `bool`, `uint32_t` sectors, and mutable/const buffer pointers as appropriate (`C-sources/libogc/gc/ogc/disc_io.h:47-65`). Rust's `DiscInterface` uses C ABI callbacks with matching bool return, `u32` sector arguments, and pointer mutability (`crates/services/ogc-filesystem-sys/src/lib.rs:10-34`). On the 32-bit target, C `unsigned long` fields are 32-bit and correspond to Rust `u32` (`disc_io.h:56-65`).
+- C declares `void USBStorage_Deinitialize(void)` and `s32 USBStorage_ioctl(int request, ...)` (`usbstorage.h:65-76`). Rust's public `deinitialize` returns `i32` (`ogc-usbstorage-sys/src/lib.rs:97-104`), and `ioctl` accepts a fixed `*mut RawDeviceCommand` (`:229-240`). The C implementation obtains the command pointer from varargs (`C-sources/libogc/libogc/usbstorage.c:970-998`). These signatures differ at the public API boundary.
+- The rest of the declared operations preserve scalar widths and pointer constness in corresponding Rust wrappers: open, capacity, read, write, and start/stop (`usbstorage.h:68-81`; `ogc-usbstorage-sys/src/lib.rs:107-112, 247-334`). However, the Rust functions shown are `pub fn` and are not declared `extern "C"` or exported under C symbol names, so they provide a Rust API rather than C-callable ABI entry points.
+
+## Validation
+
+Static source and target-type comparison only. No builds or tests were run. No hardware verification was performed. The shared USB storage note and ledger were not changed.


## 2026-10-06 implementation update

The ABI findings above are historical for the current source. The target sys crate now exports the declared `USBStorage_*` symbols with C calling conventions, preserves C's `void USBStorage_Deinitialize(void)` symbol while retaining the Rust convenience result API, exports the two `__io_usbstorage` globals, and provides a PowerPC assembly entry for variadic `USBStorage_ioctl`. Independent ABI re-review is still required.
