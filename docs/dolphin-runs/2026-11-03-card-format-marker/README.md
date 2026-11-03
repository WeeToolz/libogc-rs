# Memory-card format with a marker save

Dolphin 2609a ran `storage_probe` in Wii mode with `--format-private-card`
and a private copy of the verified 16 MiB card. The probe created, closed,
and reopened `FORMATMARK`, then formatted the card. After unmount and remount,
opening the marker returned `CARD_ERROR_NOFILE`. The free-block and final
unmount checks also passed. The [log](probe.log) reports 231 passes and no
failures.

The DOL SHA-256 was
`68803b46293229137594a7d4fa713f9abf9d65baa44011538521f9d8a008f032`.
The source card SHA-256 before and after the run was
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.
The runner and probe changes were based on revision `644d1c4`.

This checks the Dolphin virtual card in Wii mode. It does not test format in
GameCube mode, card removal, or a physical card.
