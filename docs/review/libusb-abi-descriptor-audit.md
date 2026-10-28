# USB ABI and descriptor audit

Reviewer: reviewer-6. Implementer: N/A (first-pass static audit of current code).

## Scope and result

This audit checked `ogc-usb-sys` descriptor structures and USB request callback types against the C public declarations, then traced the classic and USB v5 descriptor parsers. It did not inspect open/close behavior, mouse teardown, string conversion, or `USB_MAXPATH`.

No additional confirmed ABI or descriptor parsing mismatch was found in this scope. The existing multi-entry HID subordinate descriptor question remains uncertain; this audit does not upgrade it to a confirmed mismatch.

## Evidence

- C defines packed endpoint, interface, configuration, device, and HID descriptor structures in `C-sources/libogc/gc/ogc/usb.h:104-173`. Rust uses `#[repr(C, packed)]` for the corresponding descriptor structures in `crates/services/ogc-usb-sys/src/lib.rs:192-265`. The scalar fields, pointer fields, order, and the one-element `descr` member correspond. C's `usb_device_entry` is not packed (`usb.h:175-180`); Rust's matching device entry is `#[repr(C)]` and has the same `i32, u16, u16, u32` fields (`ogc-usb-sys/src/lib.rs:267-274`). C callback ABI is `s32 (*)(s32, void *)` (`usb.h:182`); Rust's `UsbCallback` has matching `i32`, pointer, C calling convention, and return type (`ogc-usb-sys/src/lib.rs:171`).
- C request declarations use `s32` file descriptors/results, `u8` request and endpoint values, `u16` lengths/values/indices, and `u32` descriptor buffer sizes (`usb.h:187-236`). The Rust wrapper entry points retain those widths and signedness in their corresponding parameter types; no request-width difference was found. `UsbControlRequest` is a Rust helper value, not a C-declared object passed through the public C ABI (`ogc-usb-sys/src/lib.rs:180-190`).
- For classic devices, C copies the device descriptor and byte-swaps its multi-byte fields, then reads each configuration's total length, interface descriptors, class-specific extras, and declared endpoints (`C-sources/libogc/libogc/usb.c:895-1036`). Rust's classic path performs the corresponding byte-order conversions and builds the same pointer tree (`ogc-usb-sys/src/lib.rs:712-872`). Rust checks descriptor bounds and expected descriptor types before copying; C often copies directly without those checks. These checks change malformed-input handling but do not establish a differing result for a well-formed descriptor stream.
- For USB v5 devices, C reads the device/configuration/interface data from IOS's aligned response and exposes one interface per configuration (`usb.c:797-880`). Rust's v5 parser uses the same offsets, rounds descriptor lengths to four-byte boundaries, exposes one interface, and copies extra and endpoint descriptors (`ogc-usb-sys/src/lib.rs:1680-1810`). Its bounds and descriptor-type checks add malformed-response rejection. The aligned descriptor traversal and returned interface/endpoint contents match for valid responses.
- **Superseded by the focused follow-up audit:** C's low-level `USB_GetHIDDescriptor` converts additional three-byte subordinate entries when the caller provides enough storage and size; `descr[1]` is the minimum inline entry, not the full API limit. Rust's mismatch and size-based safety contract are recorded in `libusb-hid-descriptor-audit.md`. Commit `218397a` adds the matching bounded conversion; independent review accepts the static behavior but requests a Dolphin probe for the production-shared helper before test sign-off. `USB_GetReportDescriptorSize` still requests only the nine-byte one-entry object and does not establish safe support for multiple descriptors in that convenience function.

## Validation

Static source inspection only. No builds or tests were run, as required for this review task. No hardware verification was performed. No shared USB note or ledger row was changed.
