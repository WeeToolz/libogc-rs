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
