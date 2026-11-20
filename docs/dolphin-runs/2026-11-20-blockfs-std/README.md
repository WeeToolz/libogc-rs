# exFAT through Rust `std::fs` in Dolphin

The read-only `ogc-blockfs` mount coordinator was exercised through the
public Rust `std::fs` APIs in Dolphin 2609a.

| Image | Result | Checks | Image hash unchanged |
| --- | --- | ---: | --- |
| 64 MiB, exFAT at sector zero | Pass | 27 / 27 | Yes |
| 64 MiB, exFAT in primary MBR partition | Pass | 27 / 27 | Yes |
| 8 GiB, file offset above 4 GiB | Pass | 25 / 25 | Yes |

Each run mounts `sd:` through `ogc-blockfs` and uses `std::fs` to list
directories, read and seek files, inspect metadata, reject write and remove
operations, and close the mount. The small images include a fragmented file
and nested directory. The large image checks metadata above 4 GiB and reads
the marker after a 64-bit seek. The small image runs also check that an open
file prevents unmount.

The complete runner, probe, image hashes, logs, and screen captures are saved
in the three sibling directories. Dolphin uses a private image copy and has
Wii SD writes disabled.

The implementation inventory has direct probe evidence for all 12 public
`ogc-blockfs` functions. This work adds no unverified API entries; the repo
still has the same 575 pre-existing unverified entries.

Run with `cargo xtask dolphin-validate --probe wii_std_exfat_probe
--sd-image IMAGE` and pass the Dolphin executable with `--dolphin PATH`.
The probe build needs the MP3 fixtures in `docs/dependencies.md`.

This evidence covers Dolphin's virtual Wii SD. It does not prove physical SD
controller behavior or media removal on a console.
