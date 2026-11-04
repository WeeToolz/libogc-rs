# Dolphin USB passthrough read attempt

Dolphin 2609a ran the automation build of `usbstorage_transport_probe` with
one explicitly selected USB mass-storage device, VID/PID `14cd:1212`. The
FAT32 volume was unmounted on the Mac before each run and remounted after it.
The automation path skips the probe's explicit reset and suspend cycles and
does not issue sector writes or mount a filesystem. The production USB open
and LUN initialization paths can issue a recovery reset if required; close
can issue a cache flush.

The second run reported 9 passes and 1 failure. The probe enumerated the
specified VID/PID but `storage::open` returned `-5`, before any capacity or
sector read. Dolphin's [USB log](dolphin-usb.log) reports that it failed to
claim all interfaces and returned `LIBUSB_ERROR_NOT_FOUND`. The exact cause
of that claim failure is not established. The [probe log](probe.log) retains
the outcome. This does not establish whether USB loading works on a Wii or
whether the flash drive's data can be read through Dolphin.

The DOL SHA-256 was
`9ce9625398421409fd7d6ab49d1d751f50d033bf9b87ed08aaf7d7bca3380ab2`.
It was built from revision `06a197e` with the uncommitted USB read-only probe
and runner changes subsequently committed with this report. The runner used
its own private Dolphin profile and USB Gecko log transport. Both attempts
ended with the Mac volume remounted.

## Follow-up isolation: macOS interface ownership

A direct call to the installed libusb 1.0.30 C API, with the volume unmounted,
returned these results for the same device:

```text
libusb_init 0
open True
kernel_driver_active 1
claim_interface -3 LIBUSB_ERROR_ACCESS
```

No detach, reset, data transfer, or sector write was requested by this host
check. The volume was remounted after the check. This reproduces the host
interface-access problem independently of the Rust guest.

The C reference `C-sources/libogc/libogc/usb.c` (`USB_OpenDevice`) and Rust
`ogc-usb-sys::open_device` both treat vendor-device resume as best effort and
then expose the device handle. This source comparison does not identify a
Rust port gap in that preflight. The storage-open failure happens before its
mass-storage descriptor and SCSI paths can run.

Dolphin's upstream `LibusbDevice::ClaimAllInterfaces` explicitly skips kernel
driver detach on macOS. libusb's Darwin backend requires root privilege or
`com.apple.vm.device-access` authorization to capture a driver-owned device;
that capture re-enumerates the USB device. A valid code fix must address the
host capture path and restore the host driver afterward. It has not yet been
implemented or verified. References:

- https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/IOS/USB/LibusbDevice.cpp
- https://github.com/libusb/libusb/blob/master/libusb/os/darwin_usb.c

## Authorized macOS capture check

With the user's permission to temporarily capture the device and preserve its
contents, a small host libusb helper was built with the macOS USB capture
entitlement. It was designed to detach and reclaim interface 0, then restore
the kernel driver, without sending storage commands. The private check
unmounted the confirmed CAPTURES volume first and remounted it in an exit
handler. The helper was terminated with exit status 137 before it printed any
libusb results. The macOS security log reports that its ad-hoc signature could
not validate the restricted entitlement and the kernel rejected execution.
Thus no detach, USB capture, or Dolphin read was established by this check.

`sudo -n true` also reported that a password is required. The remaining host
capture path needs an authorized administrator session or a valid signed
capture entitlement, and a verified recovery procedure for the device. Do not
format or erase the drive.
