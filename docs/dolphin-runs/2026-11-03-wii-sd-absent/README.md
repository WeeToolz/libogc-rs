# Wii SD absent-media checks in Dolphin

## Result

Dolphin 2609a passed both probes with its virtual Wii SD card disabled:

| Probe | Checks | Failures | DOL SHA-256 |
| --- | ---: | ---: | --- |
| `storage_probe` | 223 | 0 | `df09658076353147a70891437b1a01269d1e3b8720285b9c3f8d79556746d195` |
| `fat_sd_probe` | 21 | 0 | `63ed332631508bfd329607f36f619a2a5f87645a405ea9500843982690ebf1ca` |

The probes confirmed that the Wii SD presence query reports no inserted media
and that FAT mount reports `DeviceUnavailable`. The GameCube SD Gecko checks
also passed for invalid drives and for slot A without an adapter. The storage
probe completed its no-device USB checks and CARD checks.

This run tests absent-media behavior in Dolphin. It does not test successful
Wii SD or GameCube SD Gecko transfers, USB transfers, or physical hardware.

## Reproduction

Set `LIBOGC_MAD_PROBE_MP3` and `LIBOGC_MAD_REFERENCE_MP3` to the verified
external fixtures, then run:

```sh
cargo xtask dolphin-validate \
  --probe storage_probe \
  --probe fat_sd_probe \
  --disable-wii-sd-card \
  --dolphin <path-to-dolphin-executable> \
  --artifact-dir docs/dolphin-runs/2026-11-03-wii-sd-absent
```

The report records the run revision and DOL hashes. Both probe logs and all
text-page captures are in this directory. I inspected the final capture for
each probe; both show the passing summary.

## Evidence

- [`storage_probe` log](storage_probe/probe.log) and [final page](storage_probe/pages/page-018.png)
- [`fat_sd_probe` log](fat_sd_probe/probe.log) and [final page](fat_sd_probe/pages/page-003.png)
- [Suite report](suite.json)
