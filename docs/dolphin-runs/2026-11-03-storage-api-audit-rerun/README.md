# Storage API Dolphin audit rerun

## Run setup

The existing `cargo xtask dolphin-validate` runner started Dolphin 2609a and
ran four storage probes from revision `0a8558207b0d5b693e35aef538762125ff5868a6`.
The runner used isolated user profiles and checked every captured page for
visible content. It reported valid page captures for all four probes. The
summary pages are readable. The complete probe logs are in this directory.

| Probe | Mode and media | Checks | DOL SHA-256 |
| --- | --- | ---: | --- |
| `card_probe` | GameCube mode, copied virtual memory card | 93 passed, 0 failed | `14a82c480cf7f3434efecfa23c08d7823dbfb0a76f20bcde4f38b44eaa7f513c` |
| `storage_probe` | Wii mode, Dolphin virtual SD and copied virtual memory card | 153 passed, 0 failed | `620fe317b5065d0c44483c4c04fdaafbbb6dfa46caa3cad9c666ccd070feb47f` |
| `fat_sd_probe` | Wii mode, Dolphin virtual SD | 40 passed, 0 failed | `6ec009904313c4d9d4a4cbc803069e34c6bac00ec662b67b20d486b9067d9775` |
| `dvd_probe` | Wii mode, no disc inserted | 120 passed, 0 failed | `c3d007bee885bfe5d1899e57a6355a6fdbd01e2a3b526cc705e59b1f7f9dab64` |

The supplied memory-card image was a copy of the verified 16 MiB template with
SHA-256 `f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.
The runner kept each probe's profile separate.

## Limits

These results prove only the checks recorded by each probe on Dolphin 2609a.
The storage probe skipped the media hotplug soak because Dolphin automation
has no operator to remove and reinsert media. The run had no USB mass-storage
device or SD Gecko adapter. The DVD probe checked no-disc behavior and closed
descriptor errors; it did not read a disc. These results do not prove physical
Wii behavior, successful DVD reads, or behavior with physical USB, SD Gecko, or
memory-card devices. The run does not prove every storage inventory entry.

## Captures

The runner captured 7 pages for `card_probe`, 14 for `storage_probe`, 4 for
`fat_sd_probe`, and 10 for `dvd_probe`. It accepted every page as readable.
This report keeps the first and summary page for each probe, with the complete
probe log. The temporary suite JSON is not included because it contains local
machine paths.

- [CARD GameCube first page](card_probe/first-page.png) and [summary](card_probe/summary-page.png)
- [Storage first page](storage_probe/first-page.png) and [summary](storage_probe/summary-page.png)
- [FAT first page](fat_sd_probe/first-page.png) and [summary](fat_sd_probe/summary-page.png)
- [DVD first page](dvd_probe/first-page.png) and [summary](dvd_probe/summary-page.png)
