"""Convert the standalone diagnostic ELF's load segments to a DOL."""
import pathlib, struct, sys
src, dst = map(pathlib.Path, sys.argv[1:3])
e = src.read_bytes()
assert e[:6] == b'\x7fELF\x01\x02', 'expected a big-endian ELF32'
phoff = struct.unpack_from('>I', e, 28)[0]
entsz, n = struct.unpack_from('>HH', e, 42)
entry = struct.unpack_from('>I', e, 24)[0]
h = bytearray(256)
out = bytearray(h)
ti = di = 0
bss = []
for i in range(n):
    typ, off, addr, _, filesz, memsz, flags, _ = struct.unpack_from('>8I', e, phoff+i*entsz)
    if typ != 1:
        continue
    if filesz:
        if flags & 1:
            assert ti < 7
            idx = ti; ti += 1
        else:
            assert di < 11
            idx = 7+di; di += 1
        struct.pack_into('>I', h, 4*idx, len(out))
        struct.pack_into('>I', h, 0x48+4*idx, addr)
        struct.pack_into('>I', h, 0x90+4*idx, filesz)
        out.extend(e[off:off+filesz])
        out.extend(bytes((-len(out))%32))
    if memsz > filesz:
        bss.append((addr+filesz, memsz-filesz))
assert len(bss) == 1, 'expected one contiguous BSS region'
struct.pack_into('>III', h, 0xd8, bss[0][0], bss[0][1], entry)
out[:256] = h
dst.write_bytes(out)
