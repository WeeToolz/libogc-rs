# USB inquiry error checks

Dolphin 2609a ran `usbstorage_capacity_probe` without a USB storage device.
The DOL SHA-256 was
`722279151f2910c5f3717b1788ebf5340c1c0b1e8261093e4b4f86d94d01c386`.
The probe passed 50 checks with no failures. It called the Rust inquiry API
directly and checked null-handle and unavailable-LUN errors. It also checked
that the failed-open handle remained closed and its buffer pointers remained
null. See the [probe log](probe.log).

This run does not verify a successful SCSI inquiry or physical USB behavior.

## API path inventory rerun

A fresh Dolphin 2609a run passed 50 checks with 0 failures. Its DOL SHA-256 is
`b7f22f2851b8b0c71fdf57ecc7e9c524abebaf9b4b7969ec93759acefcb93c0b`. The
runner accepted all 5 captured pages. The [probe log](api-path-audit-rerun/probe.log)
and [summary capture](api-path-audit-rerun/summary-page.png) are retained.

The no-device checks covered raw and safe facade errors, output preservation,
closed-handle state, inquiry errors, and deinitialization status. The probe
also checked pure USB transfer and capacity helpers. It did not test a
successful SCSI inquiry, transfer, or physical USB device.
