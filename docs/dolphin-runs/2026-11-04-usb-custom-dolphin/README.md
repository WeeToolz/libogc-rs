# Opt-in macOS USB capture in Dolphin

A separate Dolphin checkout at upstream revision
`df4bd441dec5d6f3cff606eea76f183963e97a44` was built with
[scripts/patches/dolphin-macos-opt-in-usb-capture.patch](../../../scripts/patches/dolphin-macos-opt-in-usb-capture.patch).
The patch enables libusb auto-detach on macOS only when the environment
variable `DOLPHIN_MACOS_CAPTURE_USB` exactly matches the USB device's
lowercase `vvvv:pppp` ID. The normal Dolphin passthrough whitelist also applies.
Apply this zero-context patch to the stated Dolphin revision with
`git apply --unidiff-zero /path/to/dolphin-macos-opt-in-usb-capture.patch`.
libusb's capture path re-enumerates the device and its release path requests
that the host driver return. This behavior needs a successful privileged run
and explicit restoration evidence; it is not yet verified here.

The custom build used system libusb 1.0.30 and built successfully with Vulkan
disabled. Its executable printed `Dolphin df4bd44`; SHA-256 was
`6d7e54307776f02ffa2ec8458cf9149332a102929ad605efa9dd1fcfe5ef2e8f`.

A non-administrator Dolphin run selected `14cd:1212`, enabled the new opt-in
path, and then failed to claim the interface. The [Dolphin USB log](dolphin-usb.log)
and [probe log](probe.log) record 9 passes and one failure, before a sector
read. The Mac volume was unmounted for the run and remounted afterward; no
sector write was requested. A direct earlier libusb diagnostic showed the
kernel driver active and a non-administrator claim returning access denied.

The session has no cached administrator credentials and no valid code-signing
identity. A previous ad-hoc helper with the restricted capture entitlement was
rejected by macOS before it ran. Therefore this build has not proved device
capture, data reads, or restoration after capture. It does not modify the
installed Dolphin application. Use a disposable test device and verify its
identity and recovery before a privileged run.
