# Wii FAT file and path probe

Dolphin 2609a passed `wii_std_fs_probe` with 99 checks and 0 failures. The DOL
SHA-256 is
`51d1dc129173a4c427340fad52540bcb36f4fdd4fc05efddaa3c394e82ed226f`.

The probe used Dolphin's private virtual Wii SD card. It checked FAT mount,
directory traversal, path lookup and canonicalization, create and rename
operations, file reads and writes, metadata and attributes, shared file
cursors, seek and resize errors, timestamps, read-only access, persistence
after unmount and remount, cleanup, and final unmount. The probe creates files
with `create_new`, removes the files it owns, and reports close failures.

This is Dolphin evidence for the virtual SD path. It does not validate physical
Wii SD hardware, USB storage, or GameCube memory cards. The run does not prove
that each of the 104 FAT and filesystem inventory entries passed; use the
probe log to identify the operations covered here.

## Reproduction

```sh
LIBOGC_MAD_PROBE_MP3=[libogc-rs fixtures]/layer3-stereo.mp3 \
LIBOGC_MAD_REFERENCE_MP3=[libogc-rs fixtures]/libmad-stereo-mpeg1.mp3 \
cargo xtask dolphin-validate --probe wii_std_fs_probe \
  --artifact-dir docs/dolphin-runs/2026-11-03-wii-std-fs-storage
```

The machine report, probe log, page captures, and Dolphin logs are in this
directory.
