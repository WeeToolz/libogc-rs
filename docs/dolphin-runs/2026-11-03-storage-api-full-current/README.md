# Current storage probe suite

The existing Dolphin runner ran four current storage probes from revision
`202c621`. Dolphin 2609a started in Wii or GameCube mode as required. The suite
passed 533 checks with no failures.

| Probe | Mode and media | Passed | Failed | DOL SHA-256 |
| --- | --- | ---: | ---: | --- |
| `card_probe` | GameCube mode, copied verified memory card | 98 | 0 | `2ef5fcb2b540b43c93b56f0dc9e62c30ebe72879d92e03ff47c11010cf7be587` |
| `storage_probe` | Wii mode, virtual SD and copied memory card | 217 | 0 | `07e2f31f9e9a60b69d0d27a85be223861e323ca409f0466f2630a15e4c6a3972` |
| `fat_sd_probe` | Wii mode, virtual SD | 40 | 0 | `62d3f82f276ef21008a7b9283068ad38f668c98aa9be701a1cf7e92010ac917e` |
| `dvd_probe` | Wii mode, no disc | 178 | 0 | `75f3c90f958d4f52976d43cb3d778cddad6e9d3bc61c8e3cfb5513b1a508ca54` |

The runner used a separate Dolphin profile for each probe. It copied the card
image into the two profiles that use a memory card. The source image SHA-256
was `f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`
before and after the run. Dolphin's virtual Wii SD card was enabled for the
Wii probes.

## Results

The GameCube CARD probe checked invalid-channel errors, output preservation,
owned async requests, raw sys and wrapper transfers, save creation, data and
status after remount, deletion, free-block recovery, and cleanup. The Wii
storage probe checked CARD save operations, FAT on virtual SD, no-disc DVD and
DI behavior, and the no-device USB path. The FAT/SD probe checked FAT file
operations on virtual SD and no-adapter SD Gecko errors. The DVD probe checked
DVD and DI no-disc behavior, including direct sys calls, async callbacks,
output state, and cleanup.

The runner reported 7, 18, 4, and 13 valid text-page captures, in probe order.
I inspected the summary page for each probe. Each page shows its passing
summary. The report keeps those pages and each probe log. The pages show probe
output; they are not blank or green-screen captures.

## Limits

This run verifies only the listed checks in Dolphin. It does not verify
physical Wii or GameCube behavior. No USB storage device, GameCube SD Gecko
adapter, or disc was present. Therefore:

- USB data transfers and FAT operations on USB media were skipped.
- SD Gecko success-path reads and writes were not tested.
- DVD reads and successful disc or partition operations were not tested.
- CARD format was skipped because it requires explicit destructive approval.
- The media hotplug soak was skipped because the automated runner cannot
  remove and reinsert media.

## Reproduction

Set `LIBOGC_MAD_PROBE_MP3`, `LIBOGC_MAD_REFERENCE_MP3`, and
`LIBOGC_DOLPHIN_CARD_IMAGE` to the verified external fixtures. Then run:

```sh
cargo xtask dolphin-validate \
  --probe card_probe \
  --probe storage_probe \
  --probe fat_sd_probe \
  --probe dvd_probe \
  --dolphin <Dolphin executable> \
  --artifact-dir <temporary report directory>
```

The reference MP3 fixture hash matched `docs/dependencies.md`. The runner
builds the DOLs if needed and copies the memory card and Wii NAND into private
profiles.

## Evidence files

- [`card_probe` log](card_probe/probe.log) and [summary page](card_probe/summary-page.png)
- [`storage_probe` log](storage_probe/probe.log) and [summary page](storage_probe/summary-page.png)
- [`fat_sd_probe` log](fat_sd_probe/probe.log) and [summary page](fat_sd_probe/summary-page.png)
- [`dvd_probe` log](dvd_probe/probe.log) and [summary page](dvd_probe/summary-page.png)
