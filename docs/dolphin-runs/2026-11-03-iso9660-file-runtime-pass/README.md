# ISO9660 file runtime probe

Dolphin 2609a passed the `runtime_probe` with 62 checks and 0 failures. The
DOL SHA-256 is
`2375239a9e4977bc76ff16197ab7650912aaf6c71457b00b93d4ccc404ce2c25`.

The probe used a controlled in-memory disc image. It checked the C mount and
the Rust mount. For each mount, it checked devoptab lookup, file open, expected
file bytes, seek, read after seek, close, file metadata, and change to the
root directory. It also checked invalid C arguments, descriptor and path
table read failures, and a truncated path table.

This run followed two fixes found by the probe. The ISO mount now uses a bare
device name for devoptab lookup and a colon-terminated path for device
registration and removal. `DeviceFile::open_checked` treats only `-1` as the
newlib open failure value because a valid pointer-shaped file handle can be
negative when returned as an integer on PowerPC.

This run does not verify a physical optical drive or physical disc.
