#!/usr/bin/env python3
"""Create a synthetic 64 MiB SD image for the existing exFAT SD probe.

The first type-07 MBR partition is not exFAT. The second is a complete,
writable test volume. No existing device or input image is modified.
The compressed up-case table covers ASCII and maps other units to themselves;
this is a diagnostic fixture, not a general-purpose exFAT formatter.
"""
import argparse
from pathlib import Path
import struct

SECTOR = 512
SECTORS = 131072
BASE = 2048
LENGTH = SECTORS - BASE
CLUSTER_SECTORS = 8
CLUSTER = SECTOR * CLUSTER_SECTORS
FAT = 24
HEAP = 256
COUNT = (LENGTH - HEAP) // CLUSTER_SECTORS
FAT_LENGTH = ((COUNT + 2) * 4 + SECTOR - 1) // SECTOR


def checksum(data, bits):
    result = 0
    for byte in data:
        result = ((result >> 1) | ((result & 1) << (bits - 1))) + byte
        result &= (1 << bits) - 1
    return result


def record(name, cluster, length, valid=None, attributes=0x20, contiguous=True):
    text = name.encode('utf-16le')
    assert 0 < len(text) <= 30
    entry = bytearray(96)
    entry[:2] = b'\x85\x02'
    struct.pack_into('<H', entry, 4, attributes)
    entry[32:36] = bytes((0xc0, 3 if contiguous else 1, 0, len(text) // 2))
    struct.pack_into('<H', entry, 36, checksum(name.upper().encode('utf-16le'), 16))
    struct.pack_into('<Q', entry, 40, length if valid is None else valid)
    struct.pack_into('<IQ', entry, 52, cluster, length)
    entry[64] = 0xc1
    entry[66:66 + len(text)] = text
    struct.pack_into('<H', entry, 2, checksum(entry[:2] + entry[4:], 16))
    return entry


def create(path):
    assert FAT + FAT_LENGTH <= HEAP
    boot = bytearray(SECTOR * 12)
    boot[:11] = b'\xeb\x76\x90EXFAT   '
    struct.pack_into('<QQIIIII', boot, 64, BASE, LENGTH, FAT, FAT_LENGTH, HEAP, COUNT, 2)
    struct.pack_into('<IH', boot, 100, 0x10101010, 0x0100)
    boot[108:113] = bytes((9, 3, 1, 0x80, 0xff))
    for sector in range(9):
        boot[(sector + 1) * SECTOR - 2:(sector + 1) * SECTOR] = b'\x55\xaa'
    value = checksum((v for i, v in enumerate(boot[:11 * SECTOR]) if i not in (106, 107, 112)), 32)
    boot[11 * SECTOR:] = struct.pack('<I', value) * 128
    backup = bytearray(boot)
    backup[106] = 2  # stale backup dirty flag
    mbr = bytearray(SECTOR)
    for slot, start, length in ((0, 32, 16), (2, BASE, LENGTH)):
        offset = 446 + slot * 16
        mbr[offset + 4] = 7
        struct.pack_into('<II', mbr, offset + 8, start, length)
    mbr[510:] = b'\x55\xaa'
    # The first partition deliberately contains no exFAT boot record.
    upcase = b''.join(struct.pack('<H', i - 32 if 97 <= i <= 122 else i) for i in range(128))
    upcase += struct.pack('<HH', 0xffff, 65536 - 128)
    bitmap = bytearray((COUNT + 7) // 8)
    for cluster in (2, 3, 4, 5, 6, 7, 8, 10):
        bit = cluster - 2
        bitmap[bit // 8] |= 1 << (bit % 8)
    fat = bytearray(FAT_LENGTH * SECTOR)
    for cluster, next_cluster in ((0, 0xfffffff8), (1, 0xffffffff), (2, 0xffffffff),
                                  (3, 0xffffffff), (4, 0xffffffff), (6, 10), (10, 0xffffffff)):
        struct.pack_into('<I', fat, cluster * 4, next_cluster)
    root = bytearray(64)
    root[0], root[32] = 0x81, 0x82
    struct.pack_into('<IQ', root, 20, 3, len(bitmap))
    struct.pack_into('<I', root, 36, checksum(upcase, 32))
    struct.pack_into('<IQ', root, 52, 4, len(upcase))
    payload = b'libogc-rs exFAT fixture\n'
    root += record('ReadMe.txt', 5, len(payload))
    root += record('Fragment.dat', 6, CLUSTER * 2, CLUSTER + 2, 0x21, False)
    root += record('Sub', 7, CLUSTER, attributes=0x10)
    sub = record('Child.txt', 8, 6)
    with path.open('xb') as image:
        image.truncate(SECTORS * SECTOR)
        def put(lba, data):
            assert lba * SECTOR + len(data) <= SECTORS * SECTOR
            image.seek(lba * SECTOR)
            image.write(data)
        def cluster_data(cluster, data):
            assert len(data) <= CLUSTER
            put(BASE + HEAP + (cluster - 2) * CLUSTER_SECTORS, data)
        put(0, mbr)
        put(BASE, boot)
        put(BASE + 12, backup)
        put(BASE + FAT, fat)
        cluster_data(2, root)
        cluster_data(3, bitmap)
        cluster_data(4, upcase)
        cluster_data(5, payload)
        cluster_data(6, b'A' * (CLUSTER - 1) + b'B')
        cluster_data(7, sub)
        cluster_data(8, b'child\n')
        cluster_data(10, b'CD' + b'\xa5' * (CLUSTER - 2))
    print(f'{path}: exFAT at LBA {BASE}, preceding non-exFAT candidate at LBA 32')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    create(parser.parse_args().output)
