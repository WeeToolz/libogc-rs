# CARD EXI status diagnostic

Dolphin 2609a ran the Wii and GameCube probes with private copies of a 16 MiB
card image. The source image SHA-256 remained
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.
The Wii DOL SHA-256 was
`d9c4b95559de626dd2dcbc53d681017ce05e5d96c07542cf831d6677c47ae436`.
The GameCube DOL SHA-256 was
`a74b489a1e4d4ace35e3b4cad317e13f08f945c0df4bdbb7e0040b12f23ccec0`.

Before the CARD probe, the EXI channel 0 status was `0x00001008` in Wii mode
and `0x00001800` in GameCube mode. Both contain the external-device presence
bit; the latter also contains the external interrupt bit. The raw CARD probe
returned zero and the extended probe returned busy in both runs. Both mount
requests completed with no card, so save operations were skipped. The Wii
probe passed 112 checks. The GameCube probe reported retrace timeouts and did
not reach its summary. The register reading shows a device presence signal; it
does not establish that the image was mounted or that save data persists.
See the [Wii log](wii-probe.log) and [GameCube log](gamecube-probe.log).
