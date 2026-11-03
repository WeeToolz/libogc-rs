# USB storage direct error checks

Dolphin 2609a ran `usbstorage_capacity_probe` without a USB storage device.
The DOL SHA-256 was
`ec040a772982d23e13c11ffc3c1600527b0060218fbe010fb7c39e0d53cd9239`.
The probe passed 49 checks with no failures. It called the USB storage facade
and system open, close, read, and write APIs. It checked no-device or null-handle
errors, handle cleanup after failed open, and unchanged read-buffer bytes. See
the [probe log](probe.log).

This run does not verify successful USB transfers, persistence, or physical
device behavior.
