# USB storage volumes

The storage service exposes two independent USB media slots as `usb:` and `usb2:`. The slot numbers follow IOS discovery order. They do not identify fixed physical ports. HBC preserves its saved device indices 0 through 3 and assigns `usb2:` index 4.

Each slot owns its IOS handle, mounted state, LUN, device identity, USB transfer mode, and last-use time. One shared drive engine handles enumeration, open, reads, writes, close, and removal. The engine prevents one slot from selecting the other slot's live IOS device ID. A failed close keeps its handle and identity available for a later close attempt. A removal or close updates only the selected slot.

FAT routes `usb:` and `usb2:` to separate `DiscInterface` values. Both routes use the same FAT mount and file operations. FAT flush resolves the selected interface to slot 0 or slot 1 and returns the corresponding storage flush error.

The report-enabled `usb_volumes_probe` requires two real USB media. Each medium must contain `hbc-usb-probe.bin` with distinct expected byte ranges. The probe alternates reads across the two files, flushes both slots, unmounts `usb:`, and reads `usb2:` again. Missing media, missing fixture data, bad bytes, flush errors, close errors, and cleanup errors fail the report. The probe does not write files or raw sectors.

## Verification

The recovered session's storage-group check passed for the Wii PowerPC target. It checked 15 runtime packages, including `ogc-usbstorage-sys`, `ogc-fat`, the lifecycle crate, and the system probe. The host lifecycle suite passed 14 tests. The report-protocol suite passed 9 tests.

No physical Wii run is recorded for this source. One and two drive configurations, either physical USB socket, removal and reconnect, simultaneous media access, and HID input while both drives are attached remain hardware checks. The probe DOL needs to run on a Wii with both fixture media before hardware acceptance is claimed.
