#!/usr/bin/env python3
"""Compare the portable Rust Balance Board math with pinned libogc C math.

The C function and ABI declaration are extracted from the supplied reference,
not from a rewritten model. No C source is linked into production Rust.
"""
import argparse
import hashlib
import os
from pathlib import Path
import random
import subprocess
import tempfile


def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="libogc C checkout")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    reference = args.reference.resolve()
    revision = subprocess.check_output(["git", "-C", reference, "rev-parse", "HEAD"], text=True).strip()
    assert revision == "7d4b8b3e7b845516f0cc2a85274879ddd8cbf142", revision
    c_path = reference / "wiiuse/dynamics.c"
    h_path = reference / "gc/wiiuse/wiiuse.h"
    for path in [c_path, h_path]:
        run(["git", "-C", reference, "diff", "--quiet", "HEAD", "--", path.relative_to(reference)])
        print(f"{path.relative_to(reference)} sha256={hashlib.sha256(path.read_bytes()).hexdigest()}")
    text = c_path.read_text()
    start = text.index("void calc_balanceboard_state(")
    end = text.index("\n}", start) + 2
    function = text[start:end]
    text = h_path.read_text()
    start = text.index("typedef struct wii_board_t {")
    end = text.index("} wii_board_t;", start) + len("} wii_board_t;")
    declaration = text[start:end]
    c_source = """#include <stdio.h>
#include <stdint.h>
#include <string.h>
typedef float f32;
""" + declaration + "\n" + function + r"""
int main(void) {
    short v[16];
    while (scanf("%hd", &v[0]) == 1) {
        for (int i = 1; i < 16; ++i) if (scanf("%hd", &v[i]) != 1) return 2;
        wii_board_t w = {0};
        w.rtr=v[0]; w.rbr=v[1]; w.rtl=v[2]; w.rbl=v[3];
        memcpy(w.ctr,v+4,6); memcpy(w.cbr,v+7,6);
        memcpy(w.ctl,v+10,6); memcpy(w.cbl,v+13,6);
        calc_balanceboard_state(&w);
        float f[6]={w.tr,w.br,w.tl,w.bl,w.x,w.y};
        for(int i=0;i<6;++i){uint32_t bits;memcpy(&bits,&f[i],4);printf("%08x ",bits);}
        puts("");
    }
    return 0;
}
"""
    rust_source = r"""
use std::io::{self, BufRead};
use wiiuse_rs::{BalanceBoardCalibration, BalanceBoardReport};
fn main() {
    for line in io::stdin().lock().lines() {
        let values: Vec<i16> = line.unwrap().split_whitespace().map(|s| s.parse().unwrap()).collect();
        let raw: Vec<u8> = values[..4].iter().flat_map(|v| v.to_be_bytes()).collect();
        let cal = |start: usize| std::array::from_fn(|i| values[start+i] as u16);
        let report = BalanceBoardReport::decode(&raw, &BalanceBoardCalibration {
            top_right: cal(4), bottom_right: cal(7), top_left: cal(10), bottom_left: cal(13),
        }).unwrap();
        let [x,y] = report.center_offsets();
        for v in [report.top_right,report.bottom_right,report.top_left,report.bottom_left,x,y] {
            print!("{:08x} ",v.to_bits());
        }
        println!();
    }
}
"""
    rng = random.Random(0x574949)
    rows = [[50,350,100,300] + [100,200,300]*4]
    for _ in range(4096):
        rows.append([rng.randrange(-32768,32768) for _ in range(4)] + [v for _ in range(4) for v in sorted(rng.sample(range(-32768,32768),3))])
    data = "".join(" ".join(map(str,row))+"\n" for row in rows).encode()
    with tempfile.TemporaryDirectory(prefix="wiiuse-balance-") as directory:
        work = Path(directory)
        (work/"reference.c").write_text(c_source)
        (work/"runner.rs").write_text(rust_source)
        run([os.environ.get("CC", "cc"), "-O2", "-ffp-contract=off", work/"reference.c", "-o", work/"c-reference"])
        rustc = os.environ.get("RUSTC", "rustc")
        run([rustc,"--edition=2024","--crate-name=wiiuse_rs","--crate-type=rlib","-Copt-level=2",root/"crates/services/wiiuse-rs/src/lib.rs","-o",work/"libwiiuse_rs.rlib"])
        run([rustc,"--edition=2024","-Copt-level=2","--extern",f"wiiuse_rs={work/'libwiiuse_rs.rlib'}",work/"runner.rs","-o",work/"rust-runner"])
        c = subprocess.check_output([work/"c-reference"], input=data)
        rust = subprocess.check_output([work/"rust-runner"], input=data)
        assert len(c.splitlines()) == len(rows) == len(rust.splitlines())
        for i, (a,b) in enumerate(zip(c.splitlines(),rust.splitlines())):
            assert a == b, (i,rows[i],a,b)
        print(f"C revision: {revision}")
        print(f"{len(rows)} synthetic reports, {len(rows)*6} float bit patterns, zero mismatches")
        print(f"Output SHA-256: {hashlib.sha256(c).hexdigest()}")


if __name__ == "__main__":
    main()
