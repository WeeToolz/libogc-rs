# ISO9660 file runtime probe: open-result diagnostic

This historical Dolphin run found the device, and stat and directory change
worked, but file open failed because a valid pointer-shaped result was
negative. It preceded the `DeviceFile::open_checked` fix. Keep it as
diagnostic evidence; see the later `*-pass` run for the verified result.
