# Writable exFAT in Dolphin

Dolphin 2609a exercised writable exFAT through the `ogc-exfat`, `ogc-blockfs`,
Newlib, and `std::fs` APIs. Every run used the runner's private SD image copy
with `--allow-sd-image-writes`. The source fixtures stayed unchanged. The suite
reports record the source and private-copy SHA-256 values, probe results, logs,
and page captures.

| Earlier run | Layout | Probe checks | Result |
| --- | --- | ---: | --- |
| `final-backend-sector-zero` | 64 MiB, exFAT at sector zero | 50 / 50 | Pass |
| `final-backend-mbr` | 64 MiB, exFAT in a primary MBR partition | 50 / 50 | Pass |
| `final-std-sector-zero` | 64 MiB, exFAT at sector zero | 28 / 28 | Pass |
| `final-std-mbr` | 64 MiB, exFAT in a primary MBR partition | 28 / 28 | Pass |
| `std-large` | 8 GiB, file larger than 4 GiB | 25 / 25 | Pass |

The earlier 8 GiB run overwrote the file at offset zero and read above 4 GiB.
The later `final-std-large` attempt has no completed suite report.

The backend checks cover read-only write rejection, writable mount, creating a
file, the borrowed and detached file writers, length refresh across handles,
file growth, truncate, zero-fill, entry-set persistence, and remount. The
Newlib and `std::fs` checks cover truncate, write, append, create-new collision,
flush, sparse-gap zero-fill, large-file overwrite, and persistence after
unmount and remount. The small fixtures also test a contiguous file, a
fragmented file, and a nested directory.

The first std sector-zero run failed two incorrect probe length expectations.
The probe was corrected to expect the `set_len` size and appended size. The
passing runs above use the corrected checks. The earlier run is retained
under `std-sector-zero/` as the failure record; `std-sector-zero-retry/` is an
intermediate passing run.

## Write review runs

The 64 MiB fixtures contain a read-only fragmented file and a stale dirty flag
in the backup boot sector. The backend and Newlib probe checks that the mount
accepts this layout, the file remains readable, and writable opens reject it.
Newlib and `std::fs` also check that read-only handles reject writes, write-only
handles reject reads, and read-write handles allow both operations.

| Layout | Backend and Newlib | Rust `std::fs` |
| --- | --- | --- |
| Sector zero | [53 / 53](review-sector-zero-exfat_sd_probe/suite.json) | [31 / 31](review-sector-zero-wii_std_exfat_probe/suite.json) |
| Primary MBR partition | [53 / 53](review-mbr-exfat_sd_probe/suite.json) | [31 / 31](review-mbr-wii_std_exfat_probe/suite.json) |

The [8 GiB run](review-large/suite.json) passed 25 checks. It wrote four bytes
above the 4 GiB boundary and read them back after remount.

Host tests check that an open handle follows a new FAT chain after another
handle truncates and regrows a file, and that insufficient free space leaves
the entire image unchanged.

These results cover Dolphin's virtual Wii SD. They do not establish behavior
with a physical Wii SD controller or a physical card.
