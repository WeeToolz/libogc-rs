# Read-only exFAT

Use the `ogc-exfat` crate to access an exFAT volume on a Wii or GameCube block
device. The Rust API opens a volume at sector zero or in a primary MBR
partition of type `0x07`. It supports one FAT and checks both boot regions,
the allocation bitmap, and the up-case table. File names retain their stored
case. Lookup uses the volume's up-case table.

The application starts the device before it calls `Volume::mount`. Give the
mount the device's sector count and the size of one sector. The
`DiscInterface` ABI does not give the filesystem this geometry. The Wii SD
driver now exposes `ogc_wiisd::sector_count()`, which decodes the initialized
card's CSD. It returns `None` when the CSD format is unsupported. Dolphin's SD
emulation returns no usable CSD, so image validation must continue to use the
bounded image geometry. Keep the same medium connected and unchanged until
the volume is dropped. Then shut down the device.

Use `visit_dir` to list one directory and `stat` to get the stored name, size,
and type of an entry. Paths are relative to the volume root. Use `/` for
the root. Open a file with `open`, then use `seek` and `read`. The open file
borrows its volume for the duration of the read session. The detached
`ReadHandle` and `DirectoryHandle` types allow multiple open cursors on one
volume. Sizes and offsets use 64 bits. A read returns zeros in the part of a
file past its valid data length, up to the end of the file.

The read-only Wii SD integration selects between FAT and exFAT and registers
the exFAT volume as `sd:` for Newlib and `std::fs`. It supports sector-zero
and primary MBR volumes. File metadata, seeks, and offsets retain 64-bit
values. Write opens and mutations return read-only errors. An open handle
blocks unmount. File and directory operations check the media state and reject
a removed card. The existing FAT backend keeps its own registration and write
behavior.

GPT, extended MBR partitions, TexFAT, boot recovery, exFAT writes, and physical
console acceptance remain open. The reader checks metadata as it uses it. It
does not perform a full filesystem consistency scan. Dolphin 2609a evidence
covers sector-zero, MBR, fragmented-file, large-file, and FAT regression
fixtures; it does not replace hardware validation.

## Crate decision

`ogc-exfat` is a separate backend because exFAT uses an allocation bitmap,
different directory entries, and optional contiguous extents. Both the FAT
and exFAT backends use the `DiscInterface` in `ogc-filesystem-sys`. The exFAT
adapter uses aligned reads and checks the address range before calling the
32-bit ABI. The legacy FAT backend retains its own discovery, cache, device
lifecycle, and write behavior.

The `ogc-filesystem` crate implements the IOS `/dev/fs` service. It is not the
right home for SD volume mounts. The integration milestone needs one owner for
format selection, Newlib registration, and volume handles that both
`ogc-fat` and `ogc-exfat` can use. Keep those backends separate. Use a new
`ogc-blockfs` service crate for that coordinator; do not move the IOS service
or FAT format logic into it. This boundary is needed because the existing FAT
crate owns its Devoptab registration and open-handle lifecycle, while exFAT
needs the same public mount name and lifecycle without changing FAT behavior.

See [exFAT backend evidence](dolphin-runs/2026-11-20-exfat-read-only/README.md),
[Newlib mount evidence](dolphin-runs/2026-11-20-blockfs-newlib/README.md),
[std::fs evidence](dolphin-runs/2026-11-20-blockfs-std/README.md), and the
[FAT regression](dolphin-runs/2026-11-20-blockfs-fat-regression/suite.json)
for reports, images, commands, and limits.
