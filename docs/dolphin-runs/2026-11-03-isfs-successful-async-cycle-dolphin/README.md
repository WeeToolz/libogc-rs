# ISFS successful async file cycle

Dolphin 2609a passed `runtime_probe` with 78 checks and 0 failures. The DOL
SHA-256 is
`2fbf6d572c09dc644a5cdb60e8d3579d55a199b8467dbb28b69a60f273d68cb0`.

The async test used a separate app-owned file in Dolphin's private NAND copy.
It waited for each callback before it read output values or released borrowed
buffers. It checked successful file creation, async open and descriptor return,
write, seek, read, file stats, attribute get and set, usage, directory listing,
close, and delete. The read returned the written bytes. After deletion, open
returned `ISFS_ENOENT` (`-106`).

This run does not check async directory creation, rename, format, or physical
Wii NAND behavior. Dolphin returned `ISFS_EINVAL` for sync rename in the
[sync file-cycle run](../2026-11-03-isfs-successful-file-cycle-cleanup/README.md);
successful rename remains unverified. The full async output is in `suite.json`
and `runtime_probe/`.
