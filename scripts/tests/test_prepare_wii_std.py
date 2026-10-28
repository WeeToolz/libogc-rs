"""Regressions for the isolated compiler sysroot used by real Wii builds."""

import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "prepare_wii_std", Path(__file__).resolve().parents[1] / "prepare-wii-std.py")
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)


class PreparationTests(unittest.TestCase):
    def test_native_libraries_and_tools_are_linked_without_aliasing_sources(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            original = root / "installed"
            (original / "lib/rustlib/host/bin").mkdir(parents=True)
            source = original / "lib/rustlib/src/rust/library"
            source.mkdir(parents=True)
            (source / "pinned.rs").write_bytes(b"pinned source")
            native = original / "lib/libLLVM.dylib"
            native.write_bytes(b"compiler support library")
            linker = original / "lib/rustlib/host/bin/rust-lld"
            linker.write_bytes(b"linker")
            staged = root / "staged"
            rustlib = prepare.link_compiler_libraries(original, staged)
            self.assertTrue((staged / "lib/libLLVM.dylib").is_symlink())
            self.assertEqual((staged / "lib/libLLVM.dylib").resolve(), native.resolve())
            self.assertEqual((rustlib / "host/bin/rust-lld").read_bytes(), b"linker")
            self.assertFalse((rustlib / "src").exists())
            self.assertEqual((source / "pinned.rs").read_bytes(), b"pinned source")

    def test_wrapper_preserves_arguments_and_sets_compiler_library_path(self):
        with tempfile.TemporaryDirectory(prefix="wii std '") as temporary:
            root = Path(temporary)
            compiler = root / "compiler"
            compiler.write_text('#!/bin/sh\nprintf "%s\\n" "$LD_LIBRARY_PATH" "$@"\n')
            compiler.chmod(0o755)
            sysroot = root / "sysroot"
            wrapper = root / "rustc"
            # LD_LIBRARY_PATH lets this transport test run on every POSIX host.
            # The real macOS rust-lld link checks the DYLD variant separately.
            wrapper.write_text(prepare.compiler_wrapper(compiler, sysroot, "linux"))
            wrapper.chmod(0o755)
            output = subprocess.check_output(
                [str(wrapper), "argument with spaces", "$(literal)"], text=True,
                env=dict(os.environ, LD_LIBRARY_PATH="/existing/compiler/lib"))
            self.assertEqual(output.splitlines(), [str(sysroot / "lib") + ":/existing/compiler/lib",
                                                  "--sysroot", str(sysroot),
                                                  "argument with spaces", "$(literal)"])


if __name__ == "__main__":
    unittest.main()
