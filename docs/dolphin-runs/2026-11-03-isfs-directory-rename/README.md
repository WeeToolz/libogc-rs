# ISFS directory rename runtime check

`runtime_probe` passed 97 checks with no failures in Dolphin 2609a. The runner
used a private copy of the Wii NAND. The DOL SHA-256 is
`be1fa018a665b7ea5ac11abf9f77acce80ec4798a96c3ceafc90c7dbcd9847fd`.
The DOL was built from revision `df22fd4` with the rename checks under test.

The probe called the production `ogc-filesystem::rename` and
`rename_async` APIs. Both renamed an empty directory successfully. It checked
that the source path was absent, the destination path existed, the async
callback reported success, and cleanup removed both paths. The full log and
the inspected summary page are in this directory. The page shows
`PASS summary passes=97 fails=0`.

The probe also tried sync and async file renames with changed basenames. Both
returned `ISFS_EINVAL`; it checked that each source remained, each destination
stayed absent, and cleanup succeeded. Dolphin's current host backend rejects a
changed basename for files while it supports directory renames
([Dolphin `HostFileSystem::Rename`](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/IOS/FS/HostBackend/FS.cpp)).
The installed Dolphin result is direct evidence for that run. File rename
behavior on real Wii hardware remains open. The C implementation contract uses
one rename payload with separate old and new fixed-size paths
([libogc `ISFS_Rename`](https://github.com/devkitPro/libogc/blob/master/libogc/isfs.c));
the Rust implementation follows this layout.

## Reproduction

Set `LIBOGC_MAD_PROBE_MP3` and `LIBOGC_MAD_REFERENCE_MP3` to the verified
external fixtures listed in `docs/dependencies.md`, then run:

```sh
cargo xtask dolphin-validate \
  --probe runtime_probe \
  --dolphin <Dolphin executable> \
  --artifact-dir <temporary report directory>
```

The run does not verify physical Wii NAND behavior. It does not prove
successful file rename on Wii.
