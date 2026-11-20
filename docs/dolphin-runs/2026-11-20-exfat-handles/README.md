# Independent exFAT handles in Dolphin

The production `ogc-exfat` backend now offers detached file and directory
handles. They keep separate cursors, permit multiple files to stay open, and
reject a handle that belongs to another volume. Dropping a handle closes it
without I/O.

| Image | Result | Checks | Image hash unchanged |
| --- | --- | ---: | --- |
| 64 MiB, exFAT at sector zero | Pass | 33 / 33 | Yes |
| 8 GiB, file offset above 4 GiB | Pass | 16 / 16 | Yes |

The sector-zero run opened two files and two directory handles at once, read
both files with separate cursors, read across fragmented clusters, and checked
that a second volume rejects the first volume's handle. The large-image run
used a detached handle to seek to `(1 << 32) + 17` and read the expected marker
and zero-filled tail. The DOL hashes and full probe logs are saved beside each
run. `suite.json` records image hashes and runner settings.

Run with:

```sh
cargo run --release -p xtask -- dolphin-validate \
  --probe exfat_sd_probe --sd-image /path/to/sector-zero.dmg \
  --artifact-dir target/dolphin-validation/exfat-handles-sector-zero

cargo run --release -p xtask -- dolphin-validate \
  --probe exfat_sd_probe --sd-image /path/to/large.dmg \
  --artifact-dir target/dolphin-validation/exfat-handles-large
```

Set `LIBOGC_MAD_REFERENCE_MP3`, `LIBOGC_MAD_PROBE_MP3`, and pass
`--dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin` when needed.
This proves the backend handle APIs in Dolphin. Newlib and `std::fs` adapter
integration and physical Wii behavior remain open.
