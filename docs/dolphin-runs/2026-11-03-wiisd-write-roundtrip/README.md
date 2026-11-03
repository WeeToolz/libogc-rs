# Wii SD virtual-card write round trip

Dolphin 2609a passed `wiisd_probe` with 12 checks and 0 failures. The DOL
SHA-256 is
`dd4be0b0ec2ec7b88d2637685b5e7e7bc4cd5a6b3a4f413a38afb2c07c6959a8`.

The Dolphin-only check saved sector zero from the runner's private virtual SD
card, wrote a deterministic 512-byte pattern, and checked the bytes through
`read_sectors`. It then wrote the saved sector back and verified every byte.
The probe also checked startup, insertion state, sector-zero read, the
zero-sector no-data path, shutdown, and restart.

This verifies the emulated path and restoration. It does not prove physical Wii
SD transport or physical-card behavior. The non-zero write is excluded from
physical probe builds.
