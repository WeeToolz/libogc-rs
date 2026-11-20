# exFAT

The `ogc-exfat` crate reads and writes exFAT volumes on a Wii block device.
It finds a volume at sector zero or in a primary MBR partition of type
`0x07`. It validates the main and backup boot regions, the allocation bitmap,
and the up-case table. File lookup uses the volume's up-case table. File sizes
and offsets use 64 bits.

The application starts the device before it mounts the volume. Give the mount
the device's sector count and sector size. `DiscInterface` does not provide
this geometry. The Wii SD driver exposes `ogc_wiisd::sector_count()`. It returns
`None` when the CSD format is unsupported. Dolphin does not provide a usable
CSD, so the probe runner uses the bounded image size. Keep the medium connected
while the volume is open, then shut down the device after dropping the volume.

Use `Volume::mount` for a read-only mount. Use `Volume::mount_writable` to
write. Writable mounts check the dirty flag in the main boot sector. The
mount sets this flag before it changes file data or metadata. A successful
operation clears the flag after all its sector writes complete. If a sector
write fails, the mount blocks further writes and leaves the dirty flag set
if it was written. If a file cannot grow because the volume lacks free space,
the operation returns an error before it changes the volume.

Newlib and `std::fs` enforce the access mode of each file handle. Writable
opens reject files with the exFAT read-only attribute. Metadata reports that
attribute.

Writable mounts support creating files in directories with free entry slots,
overwriting and extending files, appending, truncating, bitmap allocation,
contiguous-to-FAT-chain conversion, fragmented allocation, and freeing
clusters when a file shrinks. New data and newly exposed gaps are zero-filled.
Directory entry sets and their checksums are updated. A directory that has no
free entry slots cannot grow yet. Delete, rename, mkdir, TexFAT, GPT, extended
MBR partitions, and boot recovery are not supported. This is not a full
filesystem consistency scan.

The Wii SD integration selects FAT or exFAT and registers exFAT as `sd:` for
Newlib and `std::fs`. File and directory handles keep the mount busy. Operations
check media state and propagate write failures. Sector writes are synchronous;
the Wii SD interface has no separate flush callback. FAT continues to use its
existing backend.

## Crate boundary

`ogc-exfat` remains separate from `ogc-fat`: exFAT uses an allocation bitmap,
different directory entries, and optional contiguous extents. Both use the
existing `DiscInterface`. `ogc-blockfs` owns SD format selection, Newlib
registration, and mount handles. It does not move the IOS `/dev/fs` service or
FAT format logic.

## Evidence

Dolphin 2609a runs use fresh private image copies. Writable runs require the
explicit `--allow-sd-image-writes` option. The source fixture remains unchanged;
the runner records the private copy's before and after hashes. Dolphin evidence
covers sector-zero and MBR images, fragmented and large files, Newlib, and
`std::fs`. It does not prove physical Wii SD behavior.

See the [writable exFAT evidence](dolphin-runs/2026-11-20-exfat-writable/README.md),
[read-only backend evidence](dolphin-runs/2026-11-20-exfat-read-only/README.md),
[Newlib mount evidence](dolphin-runs/2026-11-20-blockfs-newlib/README.md),
[`std::fs` evidence](dolphin-runs/2026-11-20-blockfs-std/README.md), and the
[FAT regression](dolphin-runs/2026-11-20-blockfs-fat-regression/suite.json).
