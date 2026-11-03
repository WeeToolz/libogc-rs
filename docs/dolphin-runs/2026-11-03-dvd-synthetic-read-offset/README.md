# Synthetic DVD offset read

Dolphin 2609a ran the current `dvd_probe` in Wii mode with the generated sparse
disc image. The log records a 2048-byte synchronous read at byte offset
`0x2000`. The first four bytes and the last byte matched the fixed payload.
Disc ID, synchronous sector-zero, and asynchronous sector-zero data checks
also passed.

The [probe log](probe.log) reports 79 passes and 10 failures. DVD mount still
returns `-1`. Later DI checks expect no disc, so they fail while the synthetic
disc is present. This run proves the listed data reads only. It does not prove
DVD mount, a valid disc filesystem, partition operations, or physical drive
behavior.

DOL SHA-256:
`1f113149240811518e4cb57ec38792ecfd1422f94ac00806930b271d5b4fe23e`.
