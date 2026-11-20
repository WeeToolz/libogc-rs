#!/usr/bin/env python3
"""Create a private exFAT SD image for the Dolphin probe on macOS."""

import argparse
from pathlib import Path
import struct
import subprocess
import tempfile

SECTOR_BYTES = 512
LARGE_MARKER = (1 << 32) + 17


def u32(data, offset):
    return struct.unpack_from("<I", data, offset)[0]


def u64(data, offset):
    return struct.unpack_from("<Q", data, offset)[0]


class Fixture:
    """Edit only the files created by the formatter for this probe."""

    def __init__(self, image):
        self.image = image
        boot = self.read(0, SECTOR_BYTES)
        self.base = 0
        if boot[3:11] != b"EXFAT   ":
            if boot[510:] != b"\x55\xaa" or boot[450] != 7:
                raise ValueError("expected an exFAT boot record or a primary MBR partition")
            self.base = u32(boot, 454) * SECTOR_BYTES
            boot = self.read(self.base, SECTOR_BYTES)
        if boot[3:11] != b"EXFAT   " or boot[108] != 9 or boot[110] != 1:
            raise ValueError("expected an exFAT volume with 512-byte sectors and one FAT")
        self.cluster_bytes = SECTOR_BYTES << boot[109]
        self.fat = self.base + u32(boot, 80) * SECTOR_BYTES
        self.heap = self.base + u32(boot, 88) * SECTOR_BYTES
        self.cluster_count = u32(boot, 92)
        self.root = self.cluster_offset(u32(boot, 96))
        directory = self.read(self.root, self.cluster_bytes)
        self.entries = {}
        bitmap = None
        slot = 0
        while slot < len(directory) and directory[slot]:
            kind = directory[slot]
            if kind == 0x81:
                bitmap = directory[slot : slot + 32]
            if kind == 0x85:
                end = slot + (1 + directory[slot + 1]) * 32
                record = bytearray(directory[slot:end])
                name_size = record[35] * 2
                name_bytes = b"".join(
                    record[start + 2 : start + 32]
                    for start in range(64, 64 + 32 * ((name_size + 29) // 30), 32)
                )
                self.entries[name_bytes[:name_size].decode("utf-16le")] = (self.root + slot, record)
                slot = end
            else:
                slot += 32
        if bitmap is None:
            raise ValueError("formatter did not create an allocation bitmap")
        length = u64(bitmap, 24)
        cluster = u32(bitmap, 20)
        self.bitmap_offsets = []
        self.bitmap = bytearray()
        while length:
            offset = self.cluster_offset(cluster)
            count = min(length, self.cluster_bytes)
            self.bitmap_offsets.append((offset, count))
            self.bitmap.extend(self.read(offset, count))
            cluster = u32(self.read(self.fat + cluster * 4, 4), 0)
            length -= count
        if cluster != 0xFFFFFFFF or len(self.bitmap) * 8 < self.cluster_count:
            raise ValueError("unexpected formatter bitmap chain")

    def read(self, offset, size):
        self.image.seek(offset)
        data = self.image.read(size)
        if len(data) != size:
            raise ValueError("unexpected end of image")
        return data

    def write(self, offset, data):
        if offset < 0 or offset + len(data) > self.image.seek(0, 2):
            raise ValueError("fixture write exceeds the image")
        self.image.seek(offset)
        self.image.write(data)

    def cluster_offset(self, cluster):
        if not 2 <= cluster < self.cluster_count + 2:
            raise ValueError(f"invalid fixture cluster {cluster}")
        return self.heap + (cluster - 2) * self.cluster_bytes

    def free_run(self, count, start=2):
        run = 0
        for cluster in range(start, self.cluster_count + 2):
            bit = cluster - 2
            if self.bitmap[bit // 8] & (1 << (bit % 8)):
                run = 0
                continue
            run += 1
            if run == count:
                return cluster - count + 1
        raise ValueError("image has no free extent of the requested size")

    def set_allocated(self, cluster, count, allocated=True):
        for bit in range(cluster - 2, cluster - 2 + count):
            index, shift = divmod(bit, 8)
            if allocated:
                self.bitmap[index] |= 1 << shift
            else:
                self.bitmap[index] &= ~(1 << shift)

    def update_entry(self, offset, record, first, length, valid, contiguous):
        record[33] = 3 if contiguous else 1
        struct.pack_into("<Q", record, 40, valid)
        struct.pack_into("<IQ", record, 52, first, length)
        checksum = 0
        for index, byte in enumerate(record):
            if index not in (2, 3):
                checksum = (((checksum >> 1) | ((checksum & 1) << 15)) + byte) & 0xFFFF
        struct.pack_into("<H", record, 2, checksum)
        self.write(offset, record)

    def make_fragmented(self):
        offset, record = self.entries["Fragment.dat"]
        if u64(record, 56) != 1 or u64(record, 40) != 1:
            raise ValueError("unexpected Fragment.dat seed")
        first = u32(record, 52)
        second = self.free_run(1, first + 2)
        self.set_allocated(second, 1)
        self.write(self.fat + first * 4, struct.pack("<I", second))
        self.write(self.fat + second * 4, struct.pack("<I", 0xFFFFFFFF))
        self.write(self.cluster_offset(first), b"A" * (self.cluster_bytes - 1) + b"B")
        self.write(self.cluster_offset(second), b"CD" + b"\xa5" * (self.cluster_bytes - 2))
        # Keep a fragmented file readable while testing write protection.
        record[4] |= 1
        self.update_entry(
            offset, record, first, self.cluster_bytes * 2, self.cluster_bytes + 2, False
        )

    def make_large(self):
        offset, record = self.entries["Large.dat"]
        old = u32(record, 52)
        if not 0 < u64(record, 56) <= self.cluster_bytes:
            raise ValueError("unexpected Large.dat seed")
        size = LARGE_MARKER + 64
        count = (size + self.cluster_bytes - 1) // self.cluster_bytes
        first = self.free_run(count)
        self.set_allocated(first, count)
        self.set_allocated(old, 1, False)
        self.write(self.fat + old * 4, b"\0" * 4)
        beginning = self.cluster_offset(first)
        self.write(beginning, b"HEAD")
        self.write(beginning + LARGE_MARKER, b"OVER4G\xa5\xa5")
        self.update_entry(offset, record, first, size, LARGE_MARKER + 6, True)

    def save_bitmap(self):
        start = 0
        for offset, count in self.bitmap_offsets:
            self.write(offset, self.bitmap[start : start + count])
            start += count


def create(output, layout, large):
    if output.suffix != ".dmg" or output.exists():
        raise ValueError("give a new .dmg path in an existing directory")
    with tempfile.TemporaryDirectory(dir=output.parent) as temp:
        source = Path(temp)
        if large:
            (source / "Large.dat").write_bytes(b"L")
        else:
            (source / "ReadMe.txt").write_bytes(b"libogc-rs exFAT fixture\n")
            (source / "Fragment.dat").write_bytes(b"F")
            sub = source / "Sub"
            sub.mkdir()
            (sub / "Child.txt").write_bytes(b"child\n")
        subprocess.run(
            [
                "hdiutil", "create", "-srcfolder", str(source), "-size", "8g" if large else "64m",
                "-layout", "MBRSPUD" if layout == "mbr" else "NONE",
                "-fs", "ExFAT", "-format", "UDRW", "-volname", "OGCTEST", str(output),
            ],
            check=True,
        )
    with output.open("r+b") as image:
        fixture = Fixture(image)
        if large:
            fixture.make_large()
        else:
            fixture.make_fragmented()
            # The backup flag may be stale. Leave the main boot sector clean.
            flag = fixture.base + 12 * SECTOR_BYTES + 106
            fixture.write(flag, bytes([fixture.read(flag, 1)[0] | 2]))
        fixture.save_bitmap()
    print(f"{output}: exFAT at LBA {fixture.base // SECTOR_BYTES}, cluster bytes {fixture.cluster_bytes}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--layout", choices=("sector-zero", "mbr"), default="sector-zero")
    parser.add_argument("--large", action="store_true", help="include a large file on an 8 GiB image")
    args = parser.parse_args()
    try:
        create(args.output.resolve(), args.layout, args.large)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
