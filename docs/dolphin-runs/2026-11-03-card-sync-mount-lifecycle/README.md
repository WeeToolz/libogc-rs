# CARD sync mount lifecycle checks in Dolphin

Dolphin 2609a ran `storage_probe` in Wii mode and `card_probe` in GameCube
mode. Each run used a private copy of the verified 16 MiB card image. Both
modes passed the synchronous mount and unmount checks for the compatibility
wrapper and the direct sys API. After each unmount, `free_blocks` returned
`CARD_ERROR_NOCARD`.

`storage_probe` passed 227 checks. `card_probe` passed 108 checks. Both runs
reported zero failures. The [Wii log](storage_probe.log) and
[GameCube log](card_probe.log) contain the results.

The probe DOL SHA-256 values were:

- Wii `storage_probe`: `bf0b669a8a8e367126d536bb187d6f55a0268868bf3558edd55c9e3bc0dad3b0`
- GameCube `card_probe`: `51df120ceaf54c4428f010e5c63c398788d856dd82401758b8ab1276248259e8`

The card image SHA-256 after the runs was
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.
The source image is copied into each private Dolphin profile before the
probe runs.

The first run failed one check in each mode because it used `sector_size` to
test whether a card was mounted. The logs are preserved as
[Wii failure evidence](failed-storage_probe-state-check.log) and
[GameCube failure evidence](failed-card_probe-state-check.log). The API returns
detected card geometry while the card is not mounted, so the probe now uses
`free_blocks`, which returns `CARD_ERROR_NOCARD` after unmount.

The successful run verified mount and unmount behavior in Dolphin. It did not
test physical card removal or the detach callback.
