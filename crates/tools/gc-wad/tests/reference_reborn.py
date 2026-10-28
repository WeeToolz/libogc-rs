#!/usr/bin/env python3
"""Compare a Rust WAD against HBC Reborn's production Python packer.

This test adapter supplies real AES from macOS CommonCrypto when PyCryptodome
is absent. The Rust packer does not call Python or CommonCrypto. Keys stay in
memory and are never passed on a command line or printed.
"""
import argparse
import ctypes
import hashlib
import importlib
from pathlib import Path
import sys

class NativeAes:
    MODE_CBC = 2
    @staticmethod
    def new(key, mode, iv):
        if mode != NativeAes.MODE_CBC or len(key) != 16 or len(iv) != 16:
            raise ValueError('expected AES-128 CBC')
        return NativeAes(key, iv)
    def __init__(self, key, iv):
        self.key, self.iv = key, iv
    def run(self, data, encrypt):
        if len(data) % 16:
            raise ValueError('unaligned AES data')
        library = ctypes.CDLL('/usr/lib/system/libcommonCrypto.dylib')
        call = library.CCCrypt
        call.argtypes = [ctypes.c_uint, ctypes.c_uint, ctypes.c_uint, ctypes.c_void_p,
                         ctypes.c_size_t, ctypes.c_void_p, ctypes.c_void_p,
                         ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t,
                         ctypes.POINTER(ctypes.c_size_t)]
        call.restype = ctypes.c_int
        output = ctypes.create_string_buffer(len(data) + 16)
        length = ctypes.c_size_t()
        rc = call(0 if encrypt else 1, 0, 0, self.key, len(self.key), self.iv,
                  data, len(data), output, len(output), ctypes.byref(length))
        if rc != 0 or length.value != len(data):
            raise RuntimeError(f'CommonCrypto AES failure {rc}')
        result = output.raw[:length.value]
        if data:
            self.iv = result[-16:] if encrypt else data[-16:]
        return result
    def encrypt(self, data):
        return self.run(data, True)
    def decrypt(self, data):
        return self.run(data, False)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for field in ('reference', 'template', 'contents', 'key', 'rust_wad', 'output'):
        parser.add_argument('--' + field.replace('_', '-'), type=Path, required=True)
    args = parser.parse_args()
    sys.path.insert(0, str(args.reference / 'pywii/Common'))
    wii = importlib.import_module('pywii.wii')
    wii.AES = NativeAes
    common = args.key.read_bytes()
    if len(common) != 16:
        raise ValueError('common key length')
    wii.keys['common-key'] = common
    tmd = wii.WiiTmd((args.template / 'tmd').read_bytes())
    ticket = wii.WiiTik((args.template / 'cetk').read_bytes())
    _, certs = wii.parse_certs((args.template / 'certs').read_bytes())
    if args.output.exists():
        raise FileExistsError(args.output)
    wad = wii.WiiWadMaker(str(args.output), tmd, ticket, certs,
                         (args.contents / 'footer').read_bytes())
    try:
        for record in tmd.get_content_records():
            wad.adddata((args.contents / f'{record.cid:08X}').read_bytes(), record.cid)
        wad.finish()
        for metadata, update in ((wad.tik, wad.updatetik), (wad.tmd, wad.updatetmd)):
            if not metadata.signcheck(wad.certs):
                metadata.null_signature()
                metadata.brute_sha()
                update()
    finally:
        wad.f.close()
    expected, actual = args.output.read_bytes(), args.rust_wad.read_bytes()
    if expected != actual:
        first = next((i for i, (a, b) in enumerate(zip(expected, actual)) if a != b), min(len(expected), len(actual)))
        raise AssertionError(f'WAD differs at {first:#x}; lengths {len(expected)} and {len(actual)}')
    print(f'PASS: Rust and Reborn WAD bytes match ({len(actual)} bytes; SHA-256 {hashlib.sha256(actual).hexdigest()})')

if __name__ == '__main__':
    main()
