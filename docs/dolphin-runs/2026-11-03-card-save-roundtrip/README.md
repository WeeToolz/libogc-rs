# CARD save roundtrip in Dolphin

Dolphin 2609a used a private copy of a 16 MiB card image for each run. The
source image remained unchanged, with SHA-256
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.
The Wii DOL SHA-256 was
`d8ebf07f3c63ed7b667d2f2b0f7128ad66922c0b5ec6416201726de3714c5b84`.
The first GameCube DOL SHA-256 was
`2871e391660ae3e6cfa53e4701a30d6e70ef28b0fed38213a3838864ec8d64f8`.
The final GameCube DOL SHA-256 was
`227a6912db86f505856402bcaedb46719c5ff8df8248f2045fa63a42cd981776`.

The Wii probe passed 151 checks and had no failures. It mounted the card,
created a save, wrote and read data, changed status, unmounted and remounted,
verified the data and status, deleted the save, remounted, and verified that
the file was absent and the free block count was restored. The first GameCube
run passed the save checks but had VI timeouts and stopped before its summary.
After GameCube system initialization and transport-only logging were set up,
the final GameCube run passed 91 checks with no failures, including the save
and remount checks. Card formatting and physical removal were not tested.

The [initial Wii run](initial-wii-probe.log) had four failures: entry deletion
used identifiers that differed from the created entry, and the write request
used 512 bytes rather than a full card sector. The probe inputs were corrected.
See the [Wii log](wii-probe.log), [first GameCube log](gamecube-probe.log),
and [final GameCube log](gamecube-final-probe.log).
These results do not establish physical Wii or GameCube behavior.
