# Wii SD capacity query in Dolphin

This run exercises `ogc_wiisd::sector_count()` from the production exFAT SD
probe. Dolphin 2609a starts the SD device and reads the supplied image, but its
SD emulation returns no usable CSD. The API reports `None`; the probe accepts
that result and checks any reported capacity against the image extent.

| Image | Result | Checks | Image hash unchanged |
| --- | --- | ---: | --- |
| exFAT at sector zero | Pass | 28 / 28 | Yes |
| exFAT in an MBR partition | Pass | 28 / 28 | Yes |

The DOL SHA-256 for both runs is
`56113f5434a4f2f220b144388ee719996b392b97a657b4580d70e16e9f022db6`.
Each run records `Wii SD CSD capacity sectors=None`. This proves the API's
unavailable-capacity result and its use from the probe. It does not verify CSD
decoding on a physical card. The driver has unit checks for CSD version 1 and
version 2, but this environment does not run the PowerPC unit-test harness.
Physical Wii validation remains required.

The probe runner copies each source image into a private Dolphin profile and
blocks writes. Each `suite.json` records the source image hash before and after
the run, the DOL hash, Dolphin version, and command arguments.
