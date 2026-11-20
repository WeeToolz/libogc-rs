# Read-only exFAT

Use the `ogc-exfat` crate to access an exFAT volume on a Wii or GameCube block
device. The Rust API opens a volume at sector zero or in a primary MBR
partition of type `0x07`. It supports one FAT and checks both boot regions,
the allocation bitmap, and the up-case table. File names retain their stored
case. Lookup uses the volume's up-case table.

The application starts the device before it calls `Volume::mount`. Give the
mount the device's sector count and the size of one sector. The
`DiscInterface` ABI does not give the filesystem this geometry. Keep the same
medium connected and unchanged until the volume is dropped. Then shut down
the device.

Use `visit_dir` to list one directory and `stat` to get the stored name, size,
and type of an entry. Paths are relative to the volume root. Use `/` for
the root. Open a file with `open`, then use `seek` and `read`. The open file
borrows its volume for the duration of the read session. Sizes and offsets
use 64 bits. A read returns zeros in the part of a file past its valid data
length, up to the end of the file.

This milestone supplies a Rust API. Newlib device registration, GPT,
extended MBR partitions, TexFAT, boot recovery, writes, and physical console
acceptance are open. The reader checks metadata as it uses it. It does not
perform a full filesystem consistency scan.

## Crate decision

`ogc-exfat` is a separate backend because exFAT uses an allocation bitmap,
different directory entries, and optional contiguous extents. Both the FAT
and exFAT backends use the `DiscInterface` in `ogc-filesystem-sys`. The exFAT
adapter uses aligned reads and checks the address range before calling the
32-bit ABI. The legacy FAT backend retains its own discovery, cache, device
lifecycle, and write behavior.

The two implementations do not need a new shared storage crate for this
milestone. The common device ABI already has a home. Revisit alignment and
partition utilities when a second caller needs the same behavior and the
legacy behavior can be kept with one implementation.

See the [Dolphin evidence](dolphin-runs/2026-11-20-exfat-read-only/README.md) for
the tested images, commands, and limits.
