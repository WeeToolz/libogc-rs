# ISFS synchronous format

Dolphin 2609a passed `runtime_probe` with 85 checks and 0 failures. The DOL
SHA-256 is
`2a0f1688bd3d217914fda8d26689f4092b32886bc3852f9f52995ddffa05009d`.

The probe created a marker directory and file in Dolphin's isolated NAND copy,
called the synchronous `format` API, then checked that both markers were
absent. The API returned `ISFS_OK` (`0`), and cleanup checks passed. The test
ran last because format clears NAND state.

Async format remains unverified. This run does not prove format behavior on a
physical Wii.
