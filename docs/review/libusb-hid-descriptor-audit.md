# USB HID multi-descriptor audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Scope: resolve whether C HID descriptor conversion handles entries beyond `descr[0]` and compare it with Rust's fixed one-entry type. No implementation changes, builds, tests, or hardware checks were performed. The shared USB review note and ledger were not edited.

## C representation and size semantics

`C-sources/libogc/gc/ogc/usb.h:164-173` declares `usb_hiddesc` with `bNumDescriptors` followed by packed `descr[1]`; each descriptor item is three bytes (`bDescriptorType` plus 16-bit `wDescriptorLength`). `USB_DT_HID_SIZE` is 9 (`usb.h:70`), the size of the fixed header plus the first item. The one-element array is therefore the struct's minimum inline representation, while the API's `size` argument provides room for additional items.

`USB_GetHIDDescriptor` (`C-sources/libogc/libogc/usb.c:1067-1086`) passes the caller's `size` to `USB_GetGenericDescriptor`, converts the first entry's length, then subtracts nine and converts later entry lengths while `i < bNumDescriptors && size >= 3`. Thus the C conversion routine does support multiple descriptor entries when the caller supplies a buffer and size large enough for them. The descriptor read also requests that full size. The caller must allocate sufficient trailing storage; passing only a nine-byte object while claiming a larger size would overrun it.

The C convenience caller `USB_GetReportDescriptorSize` (`usb.c:1091-1114`) passes only `USB_DT_HID_SIZE` and has a local one-entry `usb_hiddesc`. Its comment says it currently supports one descriptor, although it loops to `bNumDescriptors` without a size bound. For `bNumDescriptors > 1`, that loop reads beyond the local object. This caller therefore does not safely establish multi-entry support; the lower-level conversion's size-aware API does.

## Rust comparison

Rust `UsbHidDesc` (`crates/services/ogc-usb-sys/src/lib.rs:255-264`) is `repr(C, packed)` with exactly one descriptor item and has size 9. On PowerPC, `get_hid_descriptor` (`:2062-2078`) accepts an arbitrary `size` and passes the pointer through to `get_generic_descriptor`, but after success only converts `bcd_hid` and the single `descr_w_descriptor_length`. It does not convert later entries. As with C, the raw pointer plus size can address caller-provided trailing storage, but the Rust type does not model that storage and the conversion path does not process it. The safety comment only asks for a writable `UsbHidDesc`, rather than writable storage of `size` bytes; this is too weak when `size > 9` because `get_generic_descriptor` writes `size` bytes.

Rust `get_report_descriptor_size` (`:2079-2101`) always supplies `size_of::<UsbHidDesc>()` (9) and checks only the single modeled descriptor. This corresponds to the C helper's declared one-entry/9-byte behavior, though C's unbounded loop makes its multiple-entry case an unsafe overread rather than useful support.

## Result

The uncertainty resolves as follows: C's low-level `USB_GetHIDDescriptor` supports and converts multiple entries if the caller supplies a sufficiently large buffer and `size`; `descr[1]` is the minimum inline storage, not a fixed-length array. Rust exposes only one entry and converts only that entry, so its low-level function does not preserve that C behavior for extended descriptors. The higher-level report-size helper in both implementations requests only nine bytes, and C explicitly comments that this helper currently supports one descriptor; its multi-entry loop is not safely bounded. This is a concrete lower-level API coverage mismatch and a separate Rust safety-contract gap for caller-supplied `size > 9`.

No tests were run or compiled, and no hardware verification was performed.

## Implementation disposition

Commit `218397a` now converts the first subordinate descriptor length and each later complete three-byte entry that fits within the caller's `size`. Its safety contract requires writable storage for all `size` bytes. Independent review recorded in `docs/review/libusb.md` found the bounded conversion consistent with C. The review requests a focused Dolphin DOL probe using the production-shared conversion helper before sign-off; this probe can use fixed descriptor bytes and does not need a USB device. No hardware descriptor acquisition is claimed.
