# Storage probes with the existing Dolphin runner

On 2026-11-03 the existing `cargo xtask dolphin-validate` runner used Dolphin
2609a with private user profiles and copies of the Wii NAND and memory card.
The source revision was `a4a7c91d3aada564cf6ab99e8c2fb353944c0525` and the
worktree was clean. The runner completed all five selected probes: 811 passes,
zero failures. It reported valid page captures for each probe.

| Probe | Passed | Failed | DOL SHA-256 |
| --- | ---: | ---: | --- |
| [GameCube CARD](card_probe/probe.log) | 202 | 0 | `c9ffa10b58fb515f71c16ea680a3e7dc9a355968d5165f4a11e83987517e3a22` |
| [Wii storage](storage_probe/probe.log) | 323 | 0 | `3adf08d4b15f2954470b74eb78dc8ce51c3117b3e7a366e24189f03f1632a2e4` |
| [FAT and SD](fat_sd_probe/probe.log) | 45 | 0 | `74d5b711abac355a06a8be396ef53fb0bf4fa758b44576553ed9c89dcd4ccb4c` |
| [DVD without disc](dvd_probe/probe.log) | 183 | 0 | `f2e9d6838a54021cd2fefc61639d01fdb5e6a99859e0acfbcd5e2c98953b1eee` |
| [USB without device](usbstorage_capacity_probe/probe.log) | 58 | 0 | `62187ea1bb7f87c243e51db3fa9ef97cafb1af2ff6be1e6dd7e9b60bfdd87817` |

The runner used a private copy of a previously used 16 MiB card image. The
source image SHA-256 was `39fb604967328a744686921b0c0920ed657dbe71c8e7e7eb71d87c1b327fa39a` before and after the run. The [suite report](suite.json) contains the runner arguments, individual results, page captures, and Dolphin version. Full captures and transport files remain in the runner artifact directory recorded there.

The new CARD allocation, file lifecycle, raw asynchronous, and probe geometry
checks passed. The SD Gecko invalid-drive bounds checks and USB closed-handle
checks passed. These runs had no disc, USB storage device, or EXI SD Gecko
adapter. Disc reads, device-backed USB and SD Gecko I/O, card format, and
hotplug were not tested in this run.

The earlier Dolphin version preflight failed in a restricted environment. In
this run the same installed application returned `Dolphin 2609a` and the
runner launched it successfully after that restriction was removed.

Reproduction used `--no-build` with the existing probe DOLs and card fixture:

```sh
LIBOGC_DOLPHIN_CARD_IMAGE=/path/to/card.raw cargo xtask dolphin-validate --no-build \
  --probe card_probe --probe storage_probe --probe fat_sd_probe \
  --probe dvd_probe --probe usbstorage_capacity_probe \
  --dolphin ${DOLPHIN_BIN} \
  --artifact-dir /path/to/new/report
```
