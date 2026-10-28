# USB async event callback ordering and lifecycle audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Scope: compare USB v5 device-change and removal callback registration, dispatch, close, and teardown behavior in C and Rust. No implementation changes, builds, tests, or hardware checks were performed. The shared USB note and ledger were not edited.

## Callback dispatch context and sequence

C starts the v5 device-change ioctl during `USB_Initialize` (`C-sources/libogc/libogc/usb.c:588-650`). The IOS completion callback `__usbv5_devicechangeCB` compares the returned device list, invokes removal callbacks, then submits `USBV5_IOCTL_ATTACHFINISH` (`usb.c:191-220`). Its completion handler detaches the pending change-callback list before invoking it, then re-arms `GETDEVICECHANGE` (`usb.c:166-188`). Thus removal callbacks run in the device-change IOCTL completion context before change callbacks, and change callbacks run in the attach-finish completion context.

Rust follows the same broad asynchronous phases. `usb_v5_devicechange_cb` updates the event snapshot and invokes removal callbacks before submitting `ATTACHFINISH` (`crates/services/ogc-usb-sys/src/lib.rs:1179-1258`); `usb_v5_attachfinish_cb` drains/invokes change callbacks and re-arms the watcher (`:1132-1177`). Both callbacks are entered through the IOS async completion path. Rust invokes user callbacks after leaving the event-state lock, which avoids calling user code under that lock.

## Change callback ordering mismatch

C `add_devicechange_cb` prepends each registration to a linked list (`usb.c:223-236`). `__usbv5_attachfinishCB` walks that list from its head (`:176-183`), so callbacks registered A then B are invoked B then A.

Rust `EventState::register_change` puts each callback into the first empty slot (`crates/services/ogc-usb-events/src/lib.rs:166-182`). `finish_change` drains the array in ascending index order, and `drain` preserves that order (`:194-202,289-298`). Therefore registrations A then B run A then B. The callback order differs when multiple change callbacks are pending at one attach-finish completion.

## Removal and close lifecycle

C allows one pending removal callback per device ID, rejects a duplicate, and requires the device to be present (`usb.c:1273-1291`). On observed removal it calls the callback with result 0 and clears the callback slot (`:191-215`). Closing a v5 device also calls and clears its pending removal callback with result 0 before setting the caller's fd to -1 (`:731-760`). Rust event state similarly rejects unknown/duplicate device registrations and emits a one-shot callback with `USB_OK` on removal or `close_device` (`ogc-usb-events/src/lib.rs:132-163,204-211`; `ogc-usb-sys/src/lib.rs:1579-1602`). This path matches the C success result and one-shot lifecycle.

## Teardown difference

C `USB_Deinitialize` shuts down and closes each host fd, then frees the host allocation (`usb.c:653-674`). It does not walk or invoke the pending removal callbacks or `device_change_notify` list before freeing the host. Rust `deinitialize_hosts` stops each active event state, drains all pending removal/change callbacks, shuts down and closes the fd, then invokes those callbacks with `IPC_ENOENT` (`ogc-usb-sys/src/lib.rs:1440-1501`; state draining in `ogc-usb-events/src/lib.rs:231-265`). Rust therefore adds teardown notifications that the C path does not provide. This may be a deliberate lifetime-safety improvement, but it is observable callback behavior and must be documented as such if retained.

## Repeated initialization

C `USB_Initialize` only opens and initializes host objects when their global pointer is null (`usb.c:588-651`), so another call while hosts remain allocated does not restart watchers or clear registrations. Rust `start_v5_host` returns immediately when a host fd is already present (`ogc-usb-sys/src/lib.rs:1271-1274`), and `initialize_hosts` still calls its wait helper; it does not reset the event state in that repeated-init case. This appears consistent for an already-running host. After deinitialization, both implementations can initialize again; Rust's `EventState::stop` advances its generation before a later start, preventing stale IOS completions from mutating the new state (`ogc-usb-events/src/lib.rs:231-237`).

## Result

The v5 watcher callback phases, removal callback result, duplicate/unknown removal checks, close-triggered one-shot removal, and repeated initialization behavior match at the inspected paths. Two observable lifecycle differences remain: Rust invokes pending callbacks with `IPC_ENOENT` on deinitialization where C silently frees the host state, and Rust invokes multiple change callbacks in registration order while C's prepended list invokes them in reverse registration order. No tests were run or compiled, and no hardware verification was performed.

## 2026-10-02 ordering/teardown fix

Both observable differences from this audit are now aligned with current C. Change registrations still occupy fixed-capacity storage, but `finish_change` drains them in reverse registration order so A then B dispatches B then A, matching C's prepend-and-walk list. USB v5 deinitialization now clears pending removal/change registrations silently and advances the event generation; it no longer invokes those callbacks with synthetic `IPC_ENOENT` results. Focused event-state tests cover reverse ordering and silent teardown/stale-generation behavior.

## 2026-10-07 host watcher startup order

C `USB_Initialize` submits `GETDEVICECHANGE` for the vendor host and then for the HID host before it returns (`C-sources/libogc/libogc/usb.c:588-650`). Rust previously waited for the vendor host's first device-change completion or timeout before it started the HID host. `initialize_hosts` now starts all requested v5 hosts first and then waits for each host. This matches the C request ordering and allows both IOS watchers to run at the same time.

The HBC Rust PowerPC release build completed without warnings after this change. The user reports a Wii `DeviceChange` callback result of `-1` with zero candidates, while the same drive works in C HBC. The updated DOL is on CARROT for another hardware probe. USB enumeration remains hardware-unverified until that probe confirms a device snapshot and mount.
