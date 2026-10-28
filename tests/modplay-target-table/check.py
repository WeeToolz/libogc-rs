#!/usr/bin/env python3
"""Build and inspect the actual PowerPC FREQTAB_TABLE object bytes."""
from pathlib import Path
import os
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
TMP_ROOT = Path('/Volumes/SSD/WiiDev/tmp')

# Big-endian encoding of libogc libmodplay/freqtab.c, 140 u16 values.
# This fixture is independent of the Rust table and target object.
EXPECTED_TABLE = bytes.fromhex('''
    6b0065005f405a0054c050004b80474043403f803c0038a0358032802fa02d002a60280025c023a0
    219e1fc01e001c501ac0194017d016801530140012e011d010d00fe00f000e280d600ca00be80b40
    0a980a00097008e8086807f00780071406b0065005f505a0054d050104b90475043503f903c1038b
    0358032802fa02d002a60280025c023a021a01fc01e001c501ac0194017d016801530140012e011d
    010d00fe00f000e200d600ca00be00b400aa00a00097008f0087007f00780071006b0065005f005a
    00550050004c004700430040003c0039003600330030002d002a00280026002400220020001e001c
    001b001900180016001500140013001200110010000f000e000d000d000c000b000b000a00090009
''')


def check_table(data):
    if data != EXPECTED_TABLE:
        raise AssertionError(f'PowerPC FREQTAB_TABLE bytes do not match expected values: {data.hex()}')


def command(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def main():
    rustc = command('rustup', 'which', '--toolchain', 'nightly', 'rustc', capture_output=True).stdout.strip()
    host = next(line.removeprefix('host: ') for line in command('rustc', '-vV', capture_output=True).stdout.splitlines() if line.startswith('host: '))
    sysroot = command('rustc', '--print', 'sysroot', capture_output=True).stdout.strip()
    tools = Path(sysroot) / 'lib' / 'rustlib' / host / 'bin'
    TMP_ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='modplay-table-', dir=TMP_ROOT) as temp:
        target = Path(temp) / 'target'
        env = os.environ.copy()
        env['RUSTC'] = rustc
        command('rustup', 'run', 'nightly', 'cargo', 'rustc', '-Z', 'build-std=core,alloc',
                '-Z', 'build-std-features=compiler-builtins-mem', '-Z', 'json-target-spec',
                '--target', str(ROOT / 'targets/powerpc-nintendo-none.json'), '--target-dir', str(target),
                '-p', 'modplay-capi', '--lib', '--', '--emit=obj', cwd=ROOT, env=env)
        objects = list(target.glob('powerpc-nintendo-none/debug/build/modplay-capi/*/out/modplay_capi-*.o'))
        if len(objects) != 1:
            raise AssertionError(f'expected one modplay target object, found {len(objects)}')
        obj = objects[0]
        header = obj.read_bytes()[:20]
        if header[:6] != b'\x7fELF\x01\x02' or header[18:20] != b'\x00\x14':
            raise AssertionError('object is not 32-bit big-endian PowerPC ELF')
        symbols = command(str(tools / 'llvm-objdump'), '-t', str(obj), capture_output=True).stdout
        rows = [line.split() for line in symbols.splitlines() if 'FREQTAB_TABLE' in line and '.rodata.' in line]
        if len(rows) != 1:
            raise AssertionError(f'expected one exported FREQTAB_TABLE symbol, found {len(rows)}')
        row = rows[0]
        section = next(item for item in row if item.startswith('.rodata.'))
        if row[0] != '00000000' or row[-2] != '00000118':
            raise AssertionError(f'unexpected FREQTAB_TABLE symbol offset or size: {row}')
        if not re.fullmatch(r'\.rodata\.[A-Za-z0-9_.$]+', section):
            raise AssertionError(f'unexpected table section: {section!r}')
        output = Path(temp) / 'freqtab.bin'
        command(str(tools / 'llvm-objcopy'), f'--dump-section={section}={output}', str(obj))
        data = output.read_bytes()
        check_table(data)
        print(f'PowerPC FREQTAB_TABLE: {len(data)} bytes, expected big-endian values verified')


if __name__ == '__main__':
    main()
