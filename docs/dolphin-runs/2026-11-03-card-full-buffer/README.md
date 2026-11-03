# CARD full-buffer checks in Dolphin

The CARD probe now checks every byte returned by its 512-byte owned async read and every byte in its safe synchronous sector read. The async write fills the sector with `0x5a`, except byte 1, which is `0xa5`. The sync write uses `0x3c`, `0xc3`, then zeroes. The probe also verifies persistence after remount and deletes its save.

The existing Dolphin runner built the changed probe on base revision `ef516ea` and ran Dolphin 2609a. The worktree was dirty with the probe change. Each run used an isolated profile and a copy of the verified 16 MiB card image. The source image SHA-256 was `f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8` before and after the run. The external MP3 fixture matched the hash in `docs/dependencies.md`.

| Probe | Mode | DOL SHA-256 | Result |
| --- | --- | --- | --- |
| `card_probe` | GameCube | `6c94aada2d1f5351b2a645c97a2ca2ffbba4ec7cb61cec3f45e22f3e50774dd9` | 98 passed, 0 failed |
| `storage_probe` | Wii | `463219a684dd2acbdfcd87b02b5f25e3550becb07cb15155966bc06fe849d614` | 217 passed, 0 failed |

Both logs report mount submission and completion status 0, passing async and safe sync round trips, data after remount, deletion after remount, free-block recovery, and final unmount. The GameCube process exited with code 0. The runner recorded no exit code for the Wii process, but reported a passing probe summary. Both summary captures were inspected.

The results cover Dolphin virtual cards. Card format and physical removal were skipped. The Wii probe also skipped USB transfers, SD Gecko transfers, disc reads, and hotplug. The results do not establish physical hardware behavior.

To repeat, set `LIBOGC_MAD_PROBE_MP3`, `LIBOGC_MAD_REFERENCE_MP3`, and `LIBOGC_DOLPHIN_CARD_IMAGE` to verified external fixtures, then run:

```sh
cargo xtask dolphin-validate --probe card_probe --probe storage_probe \
  --dolphin <Dolphin executable> --artifact-dir <temporary report directory>
```

- [GameCube log](card_probe/probe.log) and [summary](card_probe/summary.png)
- [Wii storage log](storage_probe/probe.log) and [summary](storage_probe/summary.png)
