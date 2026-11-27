# Bluetooth status getter in Dolphin

Date: 2026-11-27. Dolphin 2609a. The current `runtime_probe` passed 914
checks with zero failures. Its DOL SHA-256 is
`069f35eea2f4fd34c2bfe27db9b8ad845cbdd95dd16d2497caf2ffd0063722db`.
The [probe log](probe.log) records the checks.

The closed-adapter probe called the public `try_init` and `try_shutdown`
entry points. Both returned `0` in this run. After each call,
`last_status` returned the same result. The probe also confirmed that
`try_shutdown` left the adapter ready for cleanup.

This verifies the getter for successful init and shutdown results in Dolphin.
It does not verify an IOS transport failure or physical Wii behavior.
