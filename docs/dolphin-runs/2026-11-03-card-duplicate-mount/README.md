# Duplicate CARD mount checks in Dolphin

Dolphin 2609a ran `storage_probe` in Wii mode and `card_probe` in GameCube
mode. Each probe used a private copy of the verified 16 MiB card image. Both
probes submitted `card::mount_async_owned` while its mount owner was active.
Both calls returned `CARD_ERROR_BUSY`, and the original mount owner still
reported that the card was attached.

`storage_probe` passed 219 checks. `card_probe` passed 100 checks. Both runs
reported zero failures. The [Wii log](storage_probe.log) and
[GameCube log](card_probe.log) contain the assertions and summaries.

The probe DOL SHA-256 values were:

- Wii `storage_probe`: `f61cd9c82d4e4755c8320fd8b33cbe6116d0442ba05339b752335b871bf610ec`
- GameCube `card_probe`: `b9148e1a82099051dd34231bca3863cbb9bd0b27c4e8536308acf6f316e3acc2`

The source card image SHA-256 was
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8` before
and after both runs. The command used the existing runner:

```sh
LIBOGC_DOLPHIN_CARD_IMAGE=/path/to/verified-card.raw \
  cargo xtask dolphin-validate \
    --probe storage_probe --probe card_probe \
    --dolphin /path/to/Dolphin \
    --artifact-dir target/dolphin-card-duplicate-mount
```

This verifies duplicate async mount rejection in Dolphin. It does not test
physical card removal or the detach callback.
