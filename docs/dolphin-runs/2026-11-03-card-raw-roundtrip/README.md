# CARD raw read and write in Dolphin

Dolphin 2609a ran both probes with private copies of the same 16 MiB card.
The source image remained unchanged, SHA-256
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.
The Wii DOL SHA-256 was
`e26bb8fae2d995449b6e5eb9f358cdd102b95ea2b5853f446a2e8bdbd9e14eea`;
the GameCube DOL SHA-256 was
`c6e79c903456a60ee9d79f15792007216cfdb118e96b72d904abb083c558c1a3`.

The Wii probe passed 152 checks and the GameCube probe passed 92 checks, with
zero failures. Each wrote a full sector through the raw CARD API, cleared the
aligned read buffer, read the data back, and compared the returned bytes. Each
also remounted, verified the save, deleted it, remounted again, and checked that
the free block count was restored. See the [Wii log](wii-probe.log) and
[GameCube log](gamecube-probe.log). The runs do not prove physical card
behavior, card formatting, or removal handling.
