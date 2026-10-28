import importlib.util
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "build-wii-std-sysroot.py"
SPEC = importlib.util.spec_from_file_location("build_wii_std_sysroot", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class WiiStdSysrootTests(unittest.TestCase):
    def test_rejects_each_missing_required_rlib_even_with_rmeta(self):
        required = ("core", "alloc", "compiler_builtins", "std", "panic_abort")
        for missing in required:
            with self.subTest(missing=missing), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary) / "build/hash/out"
                output.mkdir(parents=True)
                for name in required:
                    (output / f"lib{name}-test.rmeta").write_bytes(b"metadata")
                    if name != missing:
                        (output / f"lib{name}-test.rlib").write_bytes(b"library")
                with self.assertRaisesRegex(ValueError, rf"missing required rlibs: {missing}$"):
                    MODULE.collect_artifacts(Path(temporary))

    def test_collects_only_rust_library_artifacts_and_checks_required_set(self):
        with tempfile.TemporaryDirectory() as temporary:
            build = Path(temporary)
            output = build / "core/hash/out"
            output.mkdir(parents=True)
            for name in (
                "libcore-test.rlib",
                "liballoc-test.rlib",
                "libcompiler_builtins-test.rlib",
                "libstd-test.rlib",
                "libpanic_abort-test.rlib",
                "libstd-test.rmeta",
                "not-a-rust-library.txt",
            ):
                (output / name).write_bytes(name.encode())
            producer = build / "wii-std-sysroot-producer/hash/out"
            producer.mkdir(parents=True)
            (producer / "libwii_std_sysroot_producer-test.rlib").write_bytes(b"probe")

            artifacts = MODULE.collect_artifacts(build)

            self.assertEqual(
                set(artifacts),
                {
                    "libcore-test.rlib",
                    "liballoc-test.rlib",
                    "libcompiler_builtins-test.rlib",
                    "libstd-test.rlib",
                    "libpanic_abort-test.rlib",
                    "libstd-test.rmeta",
                },
            )

    def test_install_manifest_detects_tampering_and_reinstalls(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "build/std/hash/out/libstd-test.rlib"
            source.parent.mkdir(parents=True)
            source.write_bytes(b"known standard library bytes")
            target = root / "sysroot/lib/rustlib/powerpc-nintendo-none"
            expected = {
                "schema_version": 1,
                "target": "powerpc-nintendo-none",
                "target_spec_sha256": "fixed target digest",
                "rustflags": ["-C", "panic=abort"],
            }

            manifest = MODULE.install_artifacts({source.name: source}, expected, target)
            self.assertTrue(MODULE.manifest_matches(manifest, expected, target / "lib"))
            installed = target / "lib" / source.name
            installed.write_bytes(b"changed bytes")
            self.assertFalse(MODULE.manifest_matches(manifest, expected, target / "lib"))

            MODULE.install_artifacts({source.name: source}, expected, target)
            self.assertEqual(installed.read_bytes(), b"known standard library bytes")
            self.assertTrue(MODULE.manifest_matches(manifest, expected, target / "lib"))


if __name__ == "__main__":
    unittest.main()
