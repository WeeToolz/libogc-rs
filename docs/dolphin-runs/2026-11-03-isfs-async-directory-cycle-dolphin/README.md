# ISFS async directory operations

Dolphin 2609a passed `runtime_probe` with 81 checks and 0 failures. The DOL
SHA-256 is
`5878cd9cc16341fc3902c4917962648c3ab9318a81ab05b21abfbc3137ac9500`.

The probe checked that `get_stats_async` returned service stats, then created
an app-owned directory with `create_dir_async`. It queried usage, listed the
directory with `read_dir_async`, and found the new directory entry. It deleted
the directory with `delete_async`. A later async usage query returned
`ISFS_ENOENT` (`-106`) and left both output values unchanged.

This checks async directory creation, usage, listing, and deletion in Dolphin's
private NAND copy. It does not verify async rename, format, or physical Wii
NAND behavior. The full output is in `suite.json` and `runtime_probe/`.
