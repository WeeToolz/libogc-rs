# GameCube memory-card format in Dolphin

Dolphin 2609a ran `card_probe` in GameCube mode with
`--format-private-card`. The runner copied the verified 16 MiB image into its
private profile. The probe created and reopened `FORMATMARK`, formatted slot A,
then mounted the card again and confirmed that `FORMATMARK` was gone. It also
checked the free-block count and final unmount. The [probe log](probe.log)
reports 111 passes and no failures.

The source image SHA-256 was
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8` both
before and after the run. The DOL SHA-256 was
`59e657b6b2bf83f0d0434f558b890580377bc5a204c23426b02b5e1c66559d59`.

This verifies Dolphin's virtual GameCube card format path. It does not verify
physical cards or card removal and hotplug behavior.
