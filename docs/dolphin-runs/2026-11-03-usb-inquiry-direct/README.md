# USB inquiry error checks

Dolphin 2609a ran `usbstorage_capacity_probe` without a USB storage device.
The DOL SHA-256 was
`722279151f2910c5f3717b1788ebf5340c1c0b1e8261093e4b4f86d94d01c386`.
The probe passed 50 checks with no failures. It called the Rust inquiry API
directly and checked null-handle and unavailable-LUN errors. It also checked
that the failed-open handle remained closed and its buffer pointers remained
null. See the [probe log](probe.log).

This run does not verify a successful SCSI inquiry or physical USB behavior.
