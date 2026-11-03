# CARD close and persistence checks

Dolphin 2609a ran both probes with private copies of a 16 MiB card image. The
source image remained unchanged, SHA-256
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.
The GameCube DOL SHA-256 was
`58d0bf4e78047a160d9ba3d7aacd6aff0bc002698707325cd763b73c095ced8d`;
the Wii DOL SHA-256 was
`b2d70d0daf321b3ca851ef604cab0e51e43ecea07aa12b81ecc2adbe582132be`.

The GameCube probe passed 93 checks and the Wii probe passed 153 checks, with
no failures. Each checked the created entry's close result and compared all
512 bytes read after remount. Each deleted the save and verified free-block
recovery after another remount. See the [GameCube log](gamecube-probe.log)
and [Wii log](wii-probe.log). Physical card behavior remains unverified.
