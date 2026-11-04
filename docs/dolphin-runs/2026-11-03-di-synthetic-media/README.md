# Direct DI calls with a synthetic disc

Dolphin 2609a ran `dvd_probe` in Wii mode with the generated sparse GameCube
image. The direct DI probe opened the transport. However, `DI_GetStatus` did
not report an inserted disc, the disc-ID byte read returned `-5`, and the Rust
`read_dvd` call returned `1` without the expected header bytes. The C
`DI_ReadDVD` call returned the same result and buffer as the Rust call.

The [log](probe.log) reports 23 passes and 4 failures, including the DVD mount
check. These results do not prove direct DI reads from this synthetic GC image.
The DVD wrapper can read known data from the image, but that does not establish
that the lower `/dev/di` path can read it. A valid Wii image or a supported
Dolphin media setup is still needed for direct DI and partition success.

The probe source now adds Rust and C asynchronous DI read checks. They run only
when direct DI returns the expected `TST1TS` ID and sector data. This saved run
did not meet that condition, so it provides no result for the new callback
checks. The updated Wii `dvd_probe` DOL has SHA-256
`a90c41cdcc835cf9564dd4b278d6a23b3a36a6dfbb4f65ddc34217a936118d25`; it has
not run in Dolphin.

Image SHA-256:
`6546ce0e4ca90e037ea961e6c13fd10ad24a83ade3be91d79003dba716ba4848`.
DOL SHA-256:
`5c140ab89fb347eae8657a3201cf7ccac8e36d333c484f42849cb5f100304134`.
