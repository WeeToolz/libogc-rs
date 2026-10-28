#!/usr/bin/env python3
"""Prepare pinned Rust library sources and an isolated compiler sysroot."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile


def tree_digest(root, *, generated_lock=False):
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*")):
        if not path.is_file() or (generated_lock and path == root / "Cargo.lock"):
            continue
        name = path.relative_to(root).as_posix().encode()
        digest.update(len(name).to_bytes(8, "big"))
        digest.update(name)
        digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()


def checked_output(*arguments):
    return subprocess.check_output(arguments, text=True).strip()


def link_compiler_libraries(original_sysroot, staged_sysroot):
    rustlib = staged_sysroot / "lib/rustlib"
    rustlib.mkdir(parents=True)
    for entry in (original_sysroot / "lib").iterdir():
        if entry.name != "rustlib":
            (rustlib.parent / entry.name).symlink_to(entry, target_is_directory=entry.is_dir())
    for entry in (original_sysroot / "lib/rustlib").iterdir():
        if entry.name != "src":
            (rustlib / entry.name).symlink_to(entry, target_is_directory=entry.is_dir())
    return rustlib


def compiler_wrapper(compiler, sysroot, platform=sys.platform):
    # A shell interpreter can remove Cargo's DYLD_* variables on macOS.
    # Set the compiler's own library search path after the shell has started.
    variable = "DYLD_FALLBACK_LIBRARY_PATH" if platform == "darwin" else "LD_LIBRARY_PATH"
    return ("#!/bin/sh\nexport " + variable + "=" + shlex.quote(str(sysroot / "lib"))
            + '${' + variable + ':+:"$' + variable + '"}\nexec '
            + shlex.quote(str(compiler)) + " --sysroot " + shlex.quote(str(sysroot)) + ' "$@"\n')


def prepare(root, toolchain):
    configuration = root / "rust-std"
    pin = json.loads((configuration / "source-lock.json").read_text())
    rustup = os.environ.get("RUSTUP", "rustup")
    compiler = Path(checked_output(rustup, "which", "--toolchain", toolchain, "rustc"))
    version = checked_output(str(compiler), "-Vv")
    expected = "commit-hash: " + pin["compiler_commit"]
    if expected not in version.splitlines():
        raise ValueError("compiler commit differs from rust-std/source-lock.json")
    original_sysroot = Path(checked_output(str(compiler), "--print", "sysroot"))
    original_library = original_sysroot / "lib/rustlib/src/rust/library"
    if not original_library.is_dir():
        raise ValueError("matching rust-src is not installed")
    if tree_digest(original_library) != pin["library_sha256"]:
        raise ValueError("installed Rust library source does not match its pinned SHA-256")

    shared_sources = {
        "crates/services/ogc-std-runtime/src/fs_abi.rs": "std/src/sys/wii_fs_abi.rs",
        "crates/core/ogc-sysbase/src/error.rs": "std/src/sys/io/error/newlib.rs",
        "crates/services/ogc-network-sys/src/socket_error.rs": "std/src/sys/net/connection/wii_error.rs",
        "crates/services/ogc-network-sys/src/socket_abi.rs": "std/src/sys/wii_net_abi.rs",
    }
    inputs = hashlib.sha256((tree_digest(configuration)
                             + "".join(hashlib.sha256((root / name).read_bytes()).hexdigest() for name in sorted(shared_sources))
                             + hashlib.sha256(Path(__file__).read_bytes()).hexdigest()).encode()).hexdigest()
    location = hashlib.sha256(str(root).encode()).hexdigest()[:8]
    destination = root / "target/wii-std" / (inputs[:16] + "-" + location)
    metadata_path = destination / "preparation.json"
    if destination.exists():
        if not metadata_path.is_file():
            raise ValueError("existing output has no preparation manifest: " + str(destination))
        metadata = json.loads(metadata_path.read_text())
        library = destination / "sysroot/lib/rustlib/src/rust/library"
        if metadata["inputs_sha256"] != inputs or metadata["repository"] != str(root):
            raise ValueError("existing output preparation inputs differ")
        if tree_digest(library, generated_lock=True) != metadata["prepared_library_sha256"]:
            raise ValueError("prepared Rust library source has changed; preserve and inspect " + str(destination))
        wrapper = destination / "rustc"
        if hashlib.sha256(wrapper.read_bytes()).hexdigest() != metadata["wrapper_sha256"]:
            raise ValueError("prepared compiler wrapper has changed: " + str(wrapper))
        return wrapper

    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="prepare-", dir=destination.parent) as temporary:
        staged = Path(temporary) / "output"
        staged.mkdir()
        # rustc also uses the sysroot's native libraries when it starts tools
        # such as rust-lld. Keep those compiler libraries available in the
        # isolated sysroot without copying or changing the installed files.
        rustlib = link_compiler_libraries(original_sysroot, staged / "sysroot")
        library = rustlib / "src/rust/library"
        shutil.copytree(original_library, library)
        for patch in sorted((configuration / "patches").glob("*.diff")):
            subprocess.run(["git", "apply", "--check", str(patch)], cwd=library, check=True)
            subprocess.run(["git", "apply", str(patch)], cwd=library, check=True)
        shutil.copytree(configuration / "overlay", library, dirs_exist_ok=True)
        # Compile the same pure error classifier in std. Stateful services stay
        # in the application's Rust provider; they are not duplicated in std.
        for name, target in shared_sources.items():
            shutil.copyfile(root / name, library / target)

        wrapper = staged / "rustc"
        wrapper.write_text(compiler_wrapper(compiler, destination / "sysroot"))
        wrapper.chmod(0o755)
        metadata = {
            "compiler_commit": pin["compiler_commit"],
            "upstream_library_sha256": pin["library_sha256"],
            "inputs_sha256": inputs,
            "prepared_library_sha256": tree_digest(library, generated_lock=True),
            "wrapper_sha256": hashlib.sha256(wrapper.read_bytes()).hexdigest(),
            "repository": str(root),
        }
        (staged / "preparation.json").write_text(json.dumps(metadata, indent=2) + "\n")
        staged.rename(destination)
    return destination / "rustc"


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", default="nightly")
    arguments = parser.parse_args()
    try:
        wrapper = prepare(root, arguments.toolchain)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, "Wii std source preparation failed: " + str(error) + "\n")
    print(wrapper)


if __name__ == "__main__":
    main()
