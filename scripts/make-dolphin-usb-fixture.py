#!/usr/bin/env python3
"""Create a blank MBR/FAT16 image for a Dolphin USB storage fixture on macOS."""
import argparse
import os
from pathlib import Path
import struct
import subprocess

SECTOR = 512


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path, help='new raw image path; existing files are not replaced')
    args = parser.parse_args()
    if args.output.exists():
        parser.error(f'output already exists: {args.output}')

    subprocess.run(['/usr/sbin/diskutil', 'image', 'create', 'blank', '--size', '64M',
                    '--fs', 'MS-DOS', '--volumeName', 'LIBOGCUSB', str(args.output)], check=True)
    with args.output.open('r+b') as image:
        total_sectors = os.fstat(image.fileno()).st_size // SECTOR
        image.seek(SECTOR)
        header = image.read(SECTOR)
        if header[:8] != b'EFI PART':
            raise ValueError('diskutil did not create the expected GPT image')
        entries_lba = struct.unpack_from('<Q', header, 72)[0]
        image.seek(entries_lba * SECTOR)
        entry = image.read(128)
        first, last = struct.unpack_from('<QQ', entry, 32)
        if first < 34 or last < first or last >= total_sectors - 33:
            raise ValueError('unexpected GPT partition bounds')
        image.seek(first * SECTOR)
        boot = bytearray(image.read(SECTOR))
        if boot[510:512] != b'\x55\xaa' or boot[54:62] != b'FAT16   ':
            raise ValueError('diskutil did not create the expected FAT16 filesystem')
        # FAT16's hidden-sector count identifies its MBR partition offset.
        struct.pack_into('<I', boot, 28, first)
        image.seek(first * SECTOR)
        image.write(boot)
        # Replace GPT with a single LBA FAT16 MBR partition. Only this newly
        # created image is written; no host block device is opened.
        mbr = bytearray(SECTOR)
        mbr[446:462] = struct.pack('<B3sB3sII', 0, b'\xfe\xff\xff', 0x0e,
                                   b'\xfe\xff\xff', first, last - first + 1)
        mbr[510:512] = b'\x55\xaa'
        image.seek(0)
        image.write(mbr)
        image.write(bytes((first - 1) * SECTOR))
        image.seek((last + 1) * SECTOR)
        image.write(bytes((total_sectors - last - 1) * SECTOR))
        image.flush()
        os.fsync(image.fileno())
    print(f'created MBR/FAT16 image {args.output}')


if __name__ == '__main__':
    main()
