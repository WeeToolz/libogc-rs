# ISFS successful file cycle and cleanup

Dolphin 2609a passed `runtime_probe` with 73 checks and 0 failures. The DOL
SHA-256 is
`e414580e1b4d4862df336f2547a74827ba2d9068fc251a3468eba6cb9fdeea4d`.

The probe used an app-owned path in Dolphin's private NAND copy. It checked
successful service stats, directory and file creation, file open, write, seek,
read, file stats, close, attribute get and set, directory usage, and directory
listing. It then deleted the test file and directory. A later open of the file
and usage query for the directory both returned `ISFS_ENOENT` (`-106`).

The probe also tried to rename the file. Dolphin 2609a returned
`ISFS_EINVAL` (`-101`) for this valid `/tmp` path, so this run does not verify
successful rename behavior. The rename result is recorded in `probe.log` as an
observation and does not count as a passing rename check.

This run checks these sync operations in Dolphin's emulated NAND. It does not
verify successful async operations, formatting, or physical Wii NAND behavior.
The full output is in `suite.json` and `runtime_probe/`.
