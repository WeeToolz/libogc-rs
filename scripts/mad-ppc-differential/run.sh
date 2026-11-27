#!/usr/bin/env bash
# Isolated Linux diagnostic; never changes the supplied repositories.
set -euo pipefail
MODE="${1:-codec}"
case "$MODE" in codec|abi) ;; *) echo 'Use codec or abi' >&2; exit 2;; esac
: "${LIBOGC_C_SOURCE:?Set this to the C reference root}"
LIBOGC_RS_SOURCE="${LIBOGC_RS_SOURCE:-$(cd "$(dirname "$0")/../.." && pwd)}"
: "${MAD_FIXTURE_DIR:?Set this to the directory containing the two MP3 fixtures}"
: "${DOLPHIN_BIN:?Set this to the Dolphin executable or extracted AppRun}"
BASE="$(cd "$(dirname "$0")" && pwd)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/mad-ppc-diagnostic.XXXXXX")"
export LIBOGC_C_SOURCE LIBOGC_RS_SOURCE MAD_FIXTURE_DIR DOLPHIN_BIN WORK BASE MODE
python3 - <<'PY'
import hashlib,json,os,pathlib,shutil
root=pathlib.Path(os.environ['WORK']); bundle=pathlib.Path(os.environ['BASE'])
sources={'c_reference':pathlib.Path(os.environ['LIBOGC_C_SOURCE']), 'rust_archive':pathlib.Path(os.environ['LIBOGC_RS_SOURCE']), 'fixtures':pathlib.Path(os.environ['MAD_FIXTURE_DIR'])}
manifest=json.loads((bundle/'source-sha256.json').read_text())
for name,expected in manifest.items():
    key,rel=name.split('/',1)
    if key=='candidate': key='rust_archive'
    elif f'candidate/{rel}' in manifest and key=='rust_archive': continue
    path=sources[key]/rel
    if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest()!=expected:
        raise SystemExit(f'Source or fixture mismatch: {name}; review the new source before applying this candidate')
    if key=='fixtures':
        dst=root/'libogc-rs-fixtures'/rel;dst.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(path,dst)
print('All codec source hashes and fixtures match')
PY
BUILD="$WORK/work/full-ppc"
mkdir -p "$BUILD/src" "$BUILD/.cargo"
if [ "$MODE" = abi ]; then
  cp "$BASE/harness/abi-main.rs" "$BUILD/src/main.rs"
else
  cp "$BASE/harness/main.rs" "$BUILD/src/main.rs"
fi
cp "$BASE/harness/"{records.c,link.ld,start.S,powerpc-nintendo-none.json} "$BUILD/"
cd "$BUILD"
cat > Cargo.toml <<'EOF'
[package]
name = "mad-full-target-diagnostic"
version = "0.0.0"
edition = "2024"
[dependencies]
[profile.release]
panic = "abort"
opt-level = 2
EOF
python3 - <<'PY'
import json,os,pathlib,shutil
root=pathlib.Path(os.environ['LIBOGC_RS_SOURCE']).resolve()
mode=os.environ['MODE']
name,dep=('mad-sys',root/'crates/media/mad-sys') if mode=='abi' else ('libmad-rs',root/'vendor/libmad-rs')
p=pathlib.Path('Cargo.toml');p.write_text(p.read_text().replace('[dependencies]', '[dependencies]\n'+name+' = { path = '+json.dumps(str(dep))+' }'))
linker=shutil.which(os.environ.get('LD_LLD','ld.lld'))
if not linker: raise SystemExit('ld.lld not found')
s='[build]\ntarget = "powerpc-nintendo-none.json"\n[target.powerpc-nintendo-none]\nlinker = '+json.dumps(linker)+'\nrustflags = ["-C", "link-arg=-Tlink.ld", "-C", "link-arg=reference.o", "-C", "link-arg=start.o", "-C", "link-arg=--gc-sections"]\n'
if os.environ.get('CARGO_VENDOR_DIR'):
    s+='[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = '+json.dumps(str(pathlib.Path(os.environ['CARGO_VENDOR_DIR']).resolve()))+'\n'
pathlib.Path('.cargo/config.toml').write_text(s)
PY
for name in records bit fixed frame huffman layer12 layer3 stream synth timer; do
  src="$LIBOGC_C_SOURCE/libmad/$name.c"
  if [ "$name" = records ]; then src=records.c; fi
  "${CLANG:-clang}" --target=powerpc-none-eabi -mcpu=750 -mbig-endian -ffreestanding -fno-builtin -fwrapv -ffunction-sections -fdata-sections -O2 -DHAVE_CONFIG_H -DHW_RVL -DNDEBUG -Wno-gnu-designator -I"$BASE/harness/include" -I"$LIBOGC_C_SOURCE/gc" -I"$LIBOGC_C_SOURCE/libmad" -c "$src" -o "$name.o"
done
"${LD_LLD:-ld.lld}" -r -m elf32ppc {records,bit,fixed,frame,huffman,layer12,layer3,stream,synth,timer}.o -o reference.o
"${CLANG:-clang}" --target=powerpc-none-eabi -mcpu=750 -c start.S -o start.o
cargo -Z json-target-spec -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem build --offline --release
ELF=target/powerpc-nintendo-none/release/mad-full-target-diagnostic
python3 "$BASE/harness/elf-to-dol.py" "$ELF" mad-full-diagnostic.dol
python3 "$BASE/harness/capture.py" "$ELF" mad-full-diagnostic.dol "$WORK/result" candidate
printf 'Diagnostic results: %s\n' "$WORK/result"
