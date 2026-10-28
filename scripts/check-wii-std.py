#!/usr/bin/env python3
"""Check that an app can use the prepared Wii std sysroot."""

import argparse
import os
from pathlib import Path
import subprocess
import sys


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", default="nightly")
    parser.add_argument("--offline", action="store_true")
    arguments = parser.parse_args()
    rustup = os.environ.get("RUSTUP", "rustup")
    try:
        compiler = subprocess.check_output(
            [sys.executable, str(root / "scripts/build-wii-std-sysroot.py"),
             "--toolchain", arguments.toolchain], text=True).strip()
    except subprocess.CalledProcessError as error:
        return error.returncode
    project = root / "target/wii-std/check"
    (project / "src").mkdir(parents=True, exist_ok=True)
    (project / "Cargo.toml").write_text(
        '[package]\nname="wii-std-foundation-check"\nversion="0.0.0"\n'
        'edition="2024"\n[workspace]\n')
    (project / "src/lib.rs").write_text(
        '#![feature(restricted_std)]\n'
        'pub fn checked_layout() -> std::alloc::Layout {\n'
        '    std::alloc::Layout::new::<[u8; 64]>()\n}\n')
    command = [rustup, "run", arguments.toolchain, "cargo", "check", "--lib",
               "-Z", "json-target-spec", "--target",
               str(root / "targets/powerpc-nintendo-none.json")]
    if arguments.offline:
        command.append("--offline")
    print("Checking Wii std app with " + compiler, flush=True)
    environment = dict(os.environ, RUSTC=compiler)
    return subprocess.run(command, cwd=project, env=environment).returncode


if __name__ == "__main__":
    sys.exit(main())
