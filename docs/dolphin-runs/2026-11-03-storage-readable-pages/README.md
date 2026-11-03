# Storage probes with readable page captures

Dolphin 2609a ran the probes below with the blank-page check enabled. The
runner used a private profile and copies of the NAND, virtual SD card, and
the configured 16 MiB memory card image. The source card SHA-256 remained
`f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`.

| Probe | Passes | Failures | Pages | DOL SHA-256 |
| --- | ---: | ---: | ---: | --- |
| `card_probe` | 93 | 0 | 7 | `56d0631185a8edef1f594a79b63d8f4040b5f4539f482f9f8d263a2ff6f3a309` |
| `storage_probe` | 153 | 0 | 14 | `cfec6f1e3bafd85271462db7243d7845cfd4da8c7ff2fdacd1c2a2953c3b5010` |
| `fat_sd_probe` | 39 | 0 | 4 | `47281d6a07ae469804bbea3980b98b78b288729dc58766d0ef7bf3945b31d775` |
| `runtime_probe` | 88 | 0 | 10 | `440b7bb02af0cccacfa76b890c03c5f740235132f595d05279e6cebf7811a42b` |
| `dvd_probe` | 120 | 0 | 10 | `52f4a0174d9f69eb434adc778874fe5971894b91f6c8c07b5e312b6693930cbc` |

Each probe directory retains its log, first page, and summary page. The first
and summary pages were inspected and contain readable text. Intermediate pages
were checked by the runner for the known blank-green pattern; they were not
all visually inspected. `runtime_probe` used the private NAND for format
checks. The runs did not verify physical storage, USB media transfers, a disc
read with media, card formatting, or physical card removal.
