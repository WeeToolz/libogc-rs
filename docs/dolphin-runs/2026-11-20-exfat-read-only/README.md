# Read-only exFAT in Dolphin

The probe uses the production `ogc-exfat` Rust API and the Wii SD driver in
Dolphin 2609a. macOS created the exFAT filesystems for all three images
independently. The fixture script then adds the allocations and data needed
to test nonadjacent clusters and offsets above 4 GiB.

| Image or regression | Pass | Fail | Report |
| --- | ---: | ---: | --- |
| 64 MiB, exFAT at sector zero | 27 | 0 | [API checks and captures](sector-zero/api-report.md) |
| 64 MiB, exFAT at MBR sector one | 27 | 0 | [API checks and captures](mbr/api-report.md) |
| 8 GiB, file offsets above 4 GiB | 13 | 0 | [Probe log](large/exfat_sd_probe/probe.log) and [capture](large/exfat_sd_probe/pages/page-002.png) |
| Existing FAT SD regression | 45 | 0 | [Probe log](fat-regression/fat_sd_probe/probe.log) and [capture](fat-regression/fat_sd_probe/pages/page-005.png) |

All three exFAT runs used the same DOL. Its SHA-256 is
`af17a4aa36d70066723d3856d0b8ad703ff955d67d7377860c10d89075fab2d3`.
Each image hash matched after the run. The write callback recorded no attempts.
The full suite JSON files give the fixture hashes and runner settings.

The 64 MiB images hold contiguous `ReadMe.txt`, fragmented `Fragment.dat`,
and a nested directory. `Fragment.dat` links clusters 8 and 12. It has an
8,192-byte length and 4,098 bytes of valid data. The read starts two bytes
before the cluster boundary and expects `ABCD` followed by four zeros. The
four underlying media bytes at those offsets hold `0xa5`. The probe also
checks case-insensitive lookup, path errors, a failed sector read, and the rejection of
corrupt boot and directory entry checksums.

The 8 GiB image holds contiguous `Large.dat` with a length of 4,294,967,377
bytes. The probe seeks to offset 4,294,967,313 and reads `OVER4G` and two
zeros, then seeks back to the marker at offset zero.

The runner copies each exFAT SD image into its private profile and disables
writes in Dolphin. A write callback in the probe also detects attempted
writes. The SHA-256 values before and after each run are in its result.
The suite and result files refer to the retained logs and captured pages
in this directory. Build and fixture paths in the reports identify the
original run.

The [inventory audit](inventory-audit.json) records six new API entries
with direct probe evidence. The same 575 earlier entries remain unverified.

## Reproduce

Run from the repository root on macOS. Use a new temporary directory. The
large-image test needs space for an 8 GiB fixture and its private copy.
Set `DOLPHIN_BIN` to the Dolphin executable. Set the MP3 fixture variables
in [dependencies.md](../../dependencies.md) for the probe build.

```sh
fixture_dir=$(mktemp -d ../../tmp/libogc-exfat.XXXXXX)
python3 scripts/create-exfat-probe-image.py "$fixture_dir/sector-zero.dmg"
python3 scripts/create-exfat-probe-image.py "$fixture_dir/mbr.dmg" --layout mbr
python3 scripts/create-exfat-probe-image.py "$fixture_dir/large.dmg" --large

cargo xtask dolphin-validate --dolphin "$DOLPHIN_BIN" \
  --probe exfat_sd_probe --sd-image "$fixture_dir/sector-zero.dmg"
cargo xtask dolphin-validate --no-build --dolphin "$DOLPHIN_BIN" \
  --probe exfat_sd_probe --sd-image "$fixture_dir/mbr.dmg"
cargo run --release -p xtask -- dolphin-validate --no-build \
  --dolphin "$DOLPHIN_BIN" --probe exfat_sd_probe --sd-image "$fixture_dir/large.dmg"
cargo xtask dolphin-validate --dolphin "$DOLPHIN_BIN" --probe fat_sd_probe
```

The optimized host runner is faster when it calculates the two full-image
hashes for the 8 GiB fixture. The formatter sets image-specific metadata.
The checks use fixed file data and offsets. A new format can have a different
image hash.

## Limits

The run covers virtual Wii SD with 512-byte sectors. Host tests cover
additional malformed stream and metadata cases, including a cyclic FAT
chain. Physical SD, USB and GameCube media, different sector sizes, hot
removal, and writes remain open. See the [crate and API guide](../../exfat.md).
