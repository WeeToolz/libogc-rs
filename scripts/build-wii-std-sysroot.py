#!/usr/bin/env python3
"""Build and install the pinned Rust libraries for the Wii target."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


TARGET = "powerpc-nintendo-none"
BUILD_STD = ["std", "panic_abort"]
BUILD_STD_FEATURES = ["compiler-builtins-mem"]


def sha256_bytes(value):
    return hashlib.sha256(value).hexdigest()


def sha256_file(path):
    return sha256_bytes(path.read_bytes())


def tree_digest(root):
    digest = hashlib.sha256()
    if not root.exists():
        return digest.hexdigest()
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        name = path.relative_to(root).as_posix().encode()
        digest.update(len(name).to_bytes(8, "big"))
        digest.update(name)
        digest.update(bytes.fromhex(sha256_file(path)))
    return digest.hexdigest()


def rustflags(root):
    inherited = os.environ.get("CARGO_ENCODED_RUSTFLAGS")
    if inherited is not None:
        return [flag for flag in inherited.split("\x1f") if flag]
    flags = os.environ.get("RUSTFLAGS", "").split()
    flags.extend([
        "--check-cfg=cfg(gamecube)",
        "-C",
        "panic=abort",
        "-C",
        "link-arg=-T" + str(root / "targets/powerpc-nintendo-none.ld"),
    ])
    return flags


def collect_artifacts(build_root):
    artifacts = {}
    for path in sorted(build_root.glob("**/out/lib*")):
        if path.suffix not in (".rlib", ".rmeta") or not path.is_file():
            continue
        # The probe crate forces Cargo to build std, but it is not a sysroot
        # library and must not be installed beside Rust's built-in crates.
        if path.parent.parent.parent.name == "wii-std-sysroot-producer":
            continue
        name = path.name
        previous = artifacts.get(name)
        if previous is not None and sha256_file(previous) != sha256_file(path):
            raise ValueError("multiple different Rust libraries have the same name: " + name)
        artifacts[name] = path
    required = {"core", "alloc", "compiler_builtins", "std", "panic_abort"}
    present = {name.split("-", 1)[0][3:] for name in artifacts if name.endswith(".rlib")}
    missing = sorted(required - present)
    if missing:
        raise ValueError("Rust library build is missing required rlibs: " + ", ".join(missing))
    return artifacts


def manifest_matches(manifest_path, expected, library_dir):
    if not manifest_path.is_file() or not library_dir.is_dir():
        return False
    try:
        manifest = json.loads(manifest_path.read_text())
        if any(manifest.get(key) != value for key, value in expected.items()):
            return False
        actual = {}
        for path in sorted(library_dir.iterdir()):
            if path.is_file() and path.suffix in (".rlib", ".rmeta"):
                actual[path.name] = sha256_file(path)
        return actual == manifest.get("artifacts_sha256")
    except (OSError, ValueError, TypeError):
        return False


def install_artifacts(artifacts, expected, target_root):
    library_dir = target_root / "lib"
    manifest_path = target_root / "wii-std-sysroot.json"
    if manifest_matches(manifest_path, expected, library_dir):
        return manifest_path

    stage_root = target_root.with_name(target_root.name + ".stage-" + str(os.getpid()))
    backup_root = target_root.with_name(target_root.name + ".backup-" + str(os.getpid()))
    if stage_root.exists():
        shutil.rmtree(stage_root)
    if backup_root.exists():
        shutil.rmtree(backup_root)
    staged_lib = stage_root / "lib"
    staged_lib.mkdir(parents=True)
    hashes = {}
    for name, source in artifacts.items():
        destination = staged_lib / name
        shutil.copyfile(source, destination)
        hashes[name] = sha256_file(destination)
    manifest = dict(expected)
    manifest["artifacts_sha256"] = hashes
    (stage_root / "wii-std-sysroot.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n"
    )

    moved_old = False
    try:
        if target_root.exists():
            target_root.rename(backup_root)
            moved_old = True
        stage_root.rename(target_root)
    except OSError:
        if moved_old and backup_root.exists() and not target_root.exists():
            backup_root.rename(target_root)
        raise
    if backup_root.exists():
        shutil.rmtree(backup_root)
    return manifest_path


def build_project(project):
    project.mkdir(parents=True, exist_ok=True)
    (project / "src").mkdir(exist_ok=True)
    (project / "Cargo.toml").write_text(
        '[package]\nname = "wii-std-sysroot-producer"\nversion = "0.0.0"\n'
        'edition = "2024"\n[workspace]\n'
    )
    (project / "src/lib.rs").write_text(
        "#![feature(restricted_std)]\n"
        "pub fn sysroot_probe() -> std::alloc::Layout {\n"
        "    std::alloc::Layout::new::<[u8; 64]>()\n"
        "}\n"
    )


def prepare_compiler(root, toolchain):
    script = root / "scripts/prepare-wii-std.py"
    return subprocess.check_output(
        [sys.executable, str(script), "--toolchain", toolchain], text=True
    ).strip()


def build_sysroot(root, toolchain, compiler=None):
    wrapper = Path(compiler or prepare_compiler(root, toolchain))
    prepared_root = wrapper.parent
    sysroot = prepared_root / "sysroot"
    preparation = json.loads((prepared_root / "preparation.json").read_text())
    target_spec = root / "targets/powerpc-nintendo-none.json"
    flags = rustflags(root)
    expected = {
        "schema_version": 1,
        "target": TARGET,
        "producer_package": "wii-std-sysroot-producer",
        "profile": "release",
        "compiler_commit": preparation["compiler_commit"],
        "upstream_library_sha256": preparation["upstream_library_sha256"],
        "prepared_library_sha256": preparation["prepared_library_sha256"],
        "builder_sha256": sha256_file(Path(__file__)),
        "patches_sha256": tree_digest(root / "rust-std/patches"),
        "overlay_sha256": tree_digest(root / "rust-std/overlay"),
        "target_spec_sha256": sha256_file(target_spec),
        "rustflags": flags,
        "cargo_flags": ["--offline", "--release", "--lib"],
        "build_std": BUILD_STD,
        "build_std_features": BUILD_STD_FEATURES,
    }
    fingerprint = sha256_bytes(json.dumps(expected, sort_keys=True).encode())[:20]
    target_dir = root / "target/xtask/wii-std-sysroot-build" / fingerprint
    project = root / "target/xtask/wii-std-sysroot-project"
    output_root = target_dir / TARGET / "release" / "build"
    target_root = sysroot / "lib/rustlib" / TARGET

    if not manifest_matches(target_root / "wii-std-sysroot.json", expected, target_root / "lib"):
        build_project(project)
        command = [
            os.environ.get("RUSTUP", "rustup"), "run", toolchain, "cargo", "build", "--offline", "--release",
            "--lib", "-Z", "build-std=" + ",".join(BUILD_STD),
            "-Z", "build-std-features=" + ",".join(BUILD_STD_FEATURES),
            "-Z", "json-target-spec", "--target", str(target_spec),
            "--target-dir", str(target_dir),
        ]
        environment = dict(os.environ)
        environment["RUSTC"] = str(wrapper)
        environment["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
        environment.pop("RUSTFLAGS", None)
        subprocess.run(command, cwd=project, env=environment, check=True, stdout=sys.stderr)
        artifacts = collect_artifacts(output_root)
        install_artifacts(artifacts, expected, target_root)

    if not manifest_matches(target_root / "wii-std-sysroot.json", expected, target_root / "lib"):
        raise ValueError("installed Wii Rust sysroot failed its integrity check")
    return wrapper


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", default="nightly")
    parser.add_argument("--compiler")
    arguments = parser.parse_args()
    try:
        compiler = build_sysroot(root, arguments.toolchain, arguments.compiler)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, "Wii Rust sysroot build failed: " + str(error) + "\n")
    if arguments.compiler is None:
        print(compiler)


if __name__ == "__main__":
    main()
