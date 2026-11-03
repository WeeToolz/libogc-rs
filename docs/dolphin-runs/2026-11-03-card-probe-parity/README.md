# CARD probe parity diagnostic

Dolphin 2609a ran `storage_probe` in Wii mode with a private copy of the raw
card image. The DOL SHA-256 was
`4a6bc9d65e48520895dc1870342b08f617908dabed532a41f4467c367b554ed4`.
The source card image SHA-256 was
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.

The probe reported 107 passes and four failures. The CARD presence return and
invalid-channel check passed. The asynchronous mount submission returned busy;
the mount request and its completion checks failed. Save operations did not
run. See [the probe log](probe.log). This result does not verify card
persistence or physical hardware.
