# Current CARD mount and save repeat

The existing Dolphin runner tested revision `43b800abf8fcd189a7b675cd7d7d42a86db11696` with Dolphin 2609a. Each probe used a private profile and a separate copy of the verified 16 MiB card image. The source image SHA-256 was `f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8` before and after the run. The external MP3 fixture matched the hash in `docs/dependencies.md`.

| Probe | Mode | DOL SHA-256 | Result |
| --- | --- | --- | --- |
| `card_probe` | GameCube | `2ef5fcb2b540b43c93b56f0dc9e62c30ebe72879d92e03ff47c11010cf7be587` | 98 passed, 0 failed |
| `storage_probe` | Wii | `07e2f31f9e9a60b69d0d27a85be223861e323ca409f0466f2630a15e4c6a3972` | 217 passed, 0 failed |

In both logs, async mount submission and completion returned 0. The probes created a save, checked async and sync read/write, unmounted and remounted, verified data and status, deleted the save, and checked free-block recovery and final unmount. The runner reported both probes as passing. The GameCube process exited with code 0; the runner recorded no exit code for the Wii process. Both captured summary pages were inspected and show the reported results.

The Wii storage probe skipped card format, media hotplug, USB transfers, SD Gecko transfers, and disc reads. These results apply to the tested Dolphin profiles and virtual card. They do not verify physical hardware.

To repeat, set `LIBOGC_MAD_PROBE_MP3`, `LIBOGC_MAD_REFERENCE_MP3`, and `LIBOGC_DOLPHIN_CARD_IMAGE` to the verified external fixtures, then run:

```sh
cargo xtask dolphin-validate --probe card_probe --probe storage_probe \
  --dolphin <Dolphin executable> --artifact-dir <temporary report directory>
```

- [GameCube log](card_probe/probe.log) and [summary](card_probe/summary.png)
- [Wii storage log](storage_probe/probe.log) and [summary](storage_probe/summary.png)
