#!/usr/bin/env python3
"""Create the synthetic Wii disc used by the Dolphin DVD probe."""
import argparse
from pathlib import Path

DISC_SIZE = 4_699_979_776
SECTOR_SIZE = 2048
PAYLOAD_OFFSET = 0x2000


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path, help='new image path; existing files are not replaced')
    args = parser.parse_args()

    with args.output.open('xb') as image:
        image.truncate(DISC_SIZE)
        image.seek(0)
        image.write(b'TST1TS\0\0')
        image.seek(0x18)
        image.write(bytes.fromhex('5d1c9ea3'))
        image.seek(PAYLOAD_OFFSET)
        image.write(bytes((11 + 37 * index) & 0xff for index in range(SECTOR_SIZE)))
    print(f'created {args.output} ({DISC_SIZE} bytes)')


if __name__ == '__main__':
    main()
