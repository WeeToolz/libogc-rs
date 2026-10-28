# USB discovery and open/close audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Scope: static comparison of USB device enumeration, descriptor enumeration, open/close, and interface/endpoint descriptor selection. Async event and HID descriptor-array behavior are covered in separate audit notes. No implementation changes, builds, tests, or hardware checks were performed. The shared USB review note and ledger were not edited.

## HID v5 open does not validate/read descriptors in Rust

C `USB_OpenDevice` distinguishes vendor and HID v5 devices (`C-sources/libogc/libogc/usb.c:676-714`). For a vendor v5 device it resumes the device. For a HID v5 device it resumes, calls `USB_GetDescriptors`, frees the returned descriptor tree on success, and suspends/returns the descriptor error if that read fails. It only writes the device ID to `*fd` after this succeeds.

Rust `open_device` (`crates/services/ogc-usb-sys/src/lib.rs:1525-1558`) uses `usb_v5_device_host` for both v5 classes, resumes, then writes the ID and returns success. It does not call `usb_v5_get_descriptors` for the HID host and does not suspend the device if descriptor reading would fail. This changes the success/error path and HID readiness side effect during open.

## HID-only discovery difference

C `USB_GetDeviceList` chooses its legacy `/dev/usb/oh0` path whenever `ven_host == NULL`, before checking `hid_host` (`usb.c:1390-1420`). Its comment and conditions include HID v5 entries only after this early branch, so a system with HID v5 available but no vendor v5 host does not enumerate the HID host's attached-device snapshot through the C v5 branch.

Rust chooses legacy enumeration only when both `vendor` and `hid` are false (`crates/services/ogc-usb-sys/src/v5_layout.rs:18` and `ogc-usb-sys/src/lib.rs:3761-3795`). With HID detected and vendor absent, Rust returns the HID host snapshot. This is an observable discovery difference for the HID-only configuration. It appears to correct the C branch selection, but remains a parity difference.

## Close fallback for unknown positive handles

C `USB_CloseDevice` first tries `USBV5_CloseDevice`; if that returns `IPC_EINVAL` and the supplied handle is positive, it calls `IOS_Close` and clears the caller's value on success (`usb.c:750-761`). The async form similarly falls back to `IOS_CloseAsync` for a positive handle (`:763-778`).

Rust `close_device` treats every non-v0 handle as a v5 device ID and returns `IPC_EINVAL` if `usb_v5_device_host` cannot find it (`ogc-usb-sys/src/lib.rs:1579-1610`). Its `close_device_async` calls this synchronous path for non-v0 values and invokes the callback immediately with that result (`:1632-1649`). Thus a positive IOS handle at or above 0x20 that is not in the current v5 snapshot is closed asynchronously/synchronously by C but rejected by Rust. For a currently listed v5 ID, both paths use the v5 one-shot removal callback and do not close an IOS file handle.

## Enumeration and descriptor-tree behavior checked

For active v5 hosts, the C list logic selects vendor entries when class is not HID and HID entries when class is zero or HID (`usb.c:1422-1444`); Rust uses the same class selection in `get_device_list` (`ogc-usb-sys/src/lib.rs:3773-3795`). Rust bounds the combined output by `num_descr`; C also stops at that bound in its v5 loops. Rust's legacy path clamps returned count to requested capacity; C trusts the count returned by IOS when writing entries, so C can overrun the caller's array if the device service reports more entries than requested. This is a C-side robustness difference, not a Rust parity regression.

For legacy descriptor trees, both parsers walk every configuration, copy interface class-specific extras and endpoints, and account for alternate interface descriptors (`usb.c:895-1042`; `parse_descriptor_tree` at `ogc-usb-sys/src/lib.rs:751-858`). For v5 descriptor trees, both expose one interface per configuration because IOS presents interfaces as separate devices; C documents that behavior at `usb.c:842-845`, and Rust sets `b_num_interfaces = 1` at `ogc-usb-sys/src/lib.rs:1754-1780`. The inspected endpoint parsing preserves endpoint order and converts little-endian packet-size values to host order in both implementations.

## Result

Confirmed Rust/C behavior differences are: HID v5 open omits C's descriptor preflight and suspend-on-failure; a HID-only v5 setup enumerates from the HID snapshot in Rust but takes C's oh0 fallback; and Rust rejects unknown positive handles that C attempts to close through IOS. The v5 class filtering and v5 one-interface descriptor representation match in the inspected paths. No tests were run or compiled, and no hardware verification was performed.

## Fix handoff: HID v5 open descriptor preflight

The current Rust `open_device` path now distinguishes the v5 host kind before returning a device ID. Both v5 host types receive the same best-effort resume step as C. HID devices then run the descriptor preflight through `get_descriptors`; a successful tree is freed before the open succeeds, while a descriptor error suspends the device and returns that error. Vendor v5 devices still return after resume without the HID descriptor round-trip. The shared helper `prepare_v5_device_for_open` keeps this policy in one place.

This resolves the HID v5 open finding at source level. The HID-only discovery branch and unknown-positive-handle close fallback remain separate open differences. No hardware verification is claimed in this handoff.

## 2026-10-02 discovery/close parity follow-up

The two remaining findings in this audit are now aligned with current C.

- Device-list selection falls back to the legacy `/dev/usb/oh0` path whenever the vendor v5 host is absent, even if the HID v5 host is present. The shared `use_legacy_list` policy now reflects C's early `ven_host == NULL` branch.
- Synchronous close now falls back to an IOS close for an unknown positive handle after v5 lookup fails, and async close submits the corresponding IOS asynchronous close instead of returning `IPC_EINVAL`. Known v5 device IDs keep the v5 callback-removal behavior.

The earlier HID descriptor-preflight fix remains present. Hardware USB validation is still pending.
