# ISFS and ISO9660 runtime follow-up

The `runtime_probe` ran in Dolphin 2609a against a private copy of the Wii
NAND. The final run passed 93 checks with no failures. Its DOL SHA-256 is
`e63366315ca75c6dc6490517b16b5465670e0cf8b5bef43084ae21b530437cb1`.
The build used repository revision `5838021`; the worktree had the probe fix
under test. Target Clippy passed with warnings denied.

The ISO9660 checks passed for the C exports, Rust C wrappers, and Rust mount
facade. They used a controlled in-memory disc interface to check mount and
unmount, volume label, file lookup, open, read, seek, stat, and close.

The ISFS checks passed for synchronous format. The probe created a marker file
and directory, formatted the private NAND, restarted the service, and verified
that the marker paths were absent. The asynchronous format and file-system
checks also passed.

The first run reported 90 passes and two sync-format setup failures. The
earlier async format had removed `/tmp` from the NAND copy. The sync-format
probe did not recreate that parent before it made its marker directory. The
probe now checks the `/tmp` state, recreates it when absent, and records the
setup and format results. The failed run and its summary capture are retained
under [`initial-failure`](initial-failure/).

The final run produced 11 valid text-page captures. I inspected its summary
page, which shows `PASS summary passes=93 fails=0`. The report stores that page
and the complete probe log under [`final-pass`](final-pass/).

## Reproduction

Set `LIBOGC_MAD_PROBE_MP3` and `LIBOGC_MAD_REFERENCE_MP3` to the verified
external fixtures listed in `docs/dependencies.md`, then run:

```sh
cargo xtask dolphin-validate \
  --probe runtime_probe \
  --dolphin <Dolphin executable> \
  --artifact-dir <temporary report directory>
```

The runner builds the DOL and uses a private NAND copy for the probe. This run
does not verify physical Wii NAND behavior or a physical optical drive.
