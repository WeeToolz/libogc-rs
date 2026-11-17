import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location(
    "api_probe_batch", Path(__file__).resolve().parents[1] / "api-probe-batch.py"
)
batch = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(batch)


class BatchTests(unittest.TestCase):
    def test_selects_exact_entry_and_rejects_ambiguity(self):
        case = {"crate": "example", "path": "src/lib.rs", "symbol": "operation"}
        row = {
            "crate": "example", "target_implementation_file": "crates/example/src/lib.rs",
            "symbol": "operation", "line": "12", "implementation_class": "unverified",
        }
        second = {**row, "line": "30"}
        with self.assertRaisesRegex(ValueError, "Select one"):
            batch.select_cases({"cases": [case]}, [row, second])
        case["line"] = 12
        self.assertEqual(batch.select_cases({"cases": [case]}, [row, second]), [(case, row)])
        with self.assertRaisesRegex(ValueError, "already verified"):
            batch.select_cases({"cases": [case]}, [{**row, "implementation_class": "ios"}])

    def test_run_requires_exact_success_line(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            log = directory / "probe.log"
            result = {
                "probe": "runtime_probe", "status": "pass", "passes": 1, "fails": 0,
                "dol_sha256": "example", "probe_log": str(log),
            }
            (directory / "suite.json").write_text(json.dumps({"passed": True, "results": [result]}))
            manifest = {"probe": "runtime_probe", "cases": [{"symbol": "f", "checks": ["known label"]}]}
            run_info = {"exit_code": 0, "dol_sha256": "example"}
            log.write_text("PASS known label suffix\n")
            with self.assertRaisesRegex(ValueError, "did not pass"):
                batch.verify_run(directory, manifest, run_info)
            log.write_text("PASS known label\n")
            self.assertEqual(batch.verify_run(directory, manifest, run_info), result)
            run_info["exit_code"] = 1
            with self.assertRaisesRegex(ValueError, "passing run"):
                batch.verify_run(directory, manifest, run_info)


if __name__ == "__main__":
    unittest.main()
