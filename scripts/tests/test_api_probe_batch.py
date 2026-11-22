from argparse import Namespace
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "api_probe_batch", Path(__file__).resolve().parents[1] / "api-probe-batch.py"
)
batch = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(batch)


class BatchTests(unittest.TestCase):
    def test_active_search_state_keeps_apis_partial(self):
        self.assertIn("active-search-state", batch.PARTIAL_SCOPES)

    def test_no_pending_interrupt_keeps_dispatch_api_partial(self):
        self.assertIn("no-pending-interrupt", batch.PARTIAL_SCOPES)

    def test_contracts_require_exact_catalog_checks(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "config").mkdir()
            (root / batch.CONTRACTS).write_text(
                "contract_id,crate,symbol,probe,evidence_path,function,api_path,check_label,environment,scope,limitation\n"
                "ipc-format.sync,ogc-ipc,IOS_IoctlvFormat,runtime_probe,probes/main.rs,probe_format,IOS_IoctlvFormat,check A,dolphin,partial,limited\n"
            )
            manifest = {"probe": "runtime_probe", "cases": [{
                "crate": "ogc-ipc", "symbol": "IOS_IoctlvFormat", "function": "probe_format",
                "evidence_path": "probes/main.rs", "api_path": "IOS_IoctlvFormat",
                "checks": ["check A"], "contracts": [{
                    "id": "ipc-format.sync", "checks": ["check A"],
                    "environment": "dolphin", "scope": "partial",
                }],
            }]}
            with patch.object(batch, "ROOT", root):
                batch.validate_checks(manifest)
                manifest["cases"][0]["contracts"][0]["checks"] = []
                with self.assertRaisesRegex(ValueError, "incorrect check labels"):
                    batch.validate_checks(manifest)
                manifest["cases"][0]["contracts"][0]["checks"] = ["check A"]
                manifest["cases"][0]["api_path"] = "another::function"
                with self.assertRaisesRegex(ValueError, "does not match the catalog"):
                    batch.validate_checks(manifest)

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
        with self.assertRaisesRegex(ValueError, "requested verification mode"):
            batch.select_cases({"cases": [case]}, [{**row, "implementation_class": "ios"}])

    def test_manifest_fills_package_relative_path_and_rejects_ambiguity(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            package = root / "nested" / "package"
            package.mkdir(parents=True)
            (package / "Cargo.toml").write_text('[package]\nname = "example"\n')
            row = {
                "crate": "example", "target_implementation_file": "nested/package/src/test.rs",
                "symbol": "operation", "line": "12", "implementation_class": "unverified",
            }
            args = Namespace(
                crate="example", probe="runtime_probe", symbols=["operation"], source=None,
                evidence_path="probes/main.rs", function="probe", api_prefix="example",
            )
            with patch.object(batch, "ROOT", root):
                rows = [row, {**row, "line": "30"}]
                with self.assertRaisesRegex(ValueError, "src/test.rs:30"):
                    batch.manifest_draft(args, rows)
                args.symbols = ["operation@12"]
                case, = batch.manifest_draft(args, rows)["cases"]
            self.assertEqual(case["path"], "src/test.rs")
            self.assertEqual(case["line"], 12)
            self.assertEqual(case["api_path"], "example::operation")
            self.assertEqual(case["checks"], [])

    def test_report_links_original_failure_and_compares_check_counts(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            probe = "runtime_probe"
            manifest = {"probe": probe, "cases": [{
                "crate": "example", "symbol": "api",
                "checks": ["known check", "missing check"],
                "contracts": [{
                    "id": "example.sync", "checks": ["known check"],
                    "environment": "dolphin", "scope": "partial",
                }],
            }], "limits": ["Dolphin only."]}
            for name, status in [("old", "FAIL"), ("new", "PASS")]:
                directory = root / name
                directory.mkdir()
                log = directory / "probe.log"
                log.write_text(
                    f"-- page 2 --\n{status} known check\n"
                    "FAILED CHECKS (retained from all pages)\nFAIL known check\n"
                )
                screenshot = directory / "page-003.png"
                screenshot.touch()
                (directory / "suite.json").write_text(json.dumps({"results": [{
                    "probe": probe, "status": "pass" if status == "PASS" else "probe_fail",
                    "passes": int(status == "PASS"), "fails": int(status == "FAIL"),
                    "probe_log": str(log), "pages_valid": True,
                    "page_frames": [str(screenshot)],
                    "page_capture_markers": ["OGC_DOLPHIN_PAGE_CAPTURE page=2 capture=3"],
                }]}))
            report = batch.evidence_report(root / "new", manifest, root / "old")
            self.assertIn("[log L2](probe.log#L2), [page-003](page-003.png)", report)
            self.assertIn("| known check | 0 pass, 1 fail | 1 pass, 0 fail |", report)
            self.assertIn("missing check (Missing)", report)
            self.assertIn("Dolphin only.", report)
            self.assertIn("## Behavioral contracts", report)
            self.assertIn("`example.sync`", report)

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
            log.write_text("PASS known label\nFAIL known label\n")
            with self.assertRaisesRegex(ValueError, "did not pass"):
                batch.verify_run(directory, manifest, run_info)
            run_info["exit_code"] = 1
            with self.assertRaisesRegex(ValueError, "passing run"):
                batch.verify_run(directory, manifest, run_info)

    def test_record_evidence_replaces_partial_cache_scopes(self):
        for scope in (
            "zero-tag-count", "zero-length", "empty-queue", "no-pending-interrupt",
            "repeated-initialization", "no-event-hook", "synchronous-scalar-output", "scalar-output",
            "scalar-and-buffer"
        ):
            with self.subTest(scope=scope), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                evidence_path = Path("config/evidence.csv")
                target = root / evidence_path
                target.parent.mkdir(parents=True)
                target.write_text(
                    "crate,path,symbol,line,classification,evidence_kind,evidence_path,"
                    "evidence_function,api_path,evidence_scope\n"
                    "example,src/lib.rs,api,12,hardware,hardware-probe,probes/old.rs,"
                    f"probe,example::api,{scope}\n"
                )
                case = {
                    "path": "src/lib.rs", "evidence_path": "probes/new.rs",
                    "function": "probe_api", "api_path": "example::api",
                }
                row = {
                    "crate": "example", "symbol": "api", "line": "12",
                    "declared_classification": "hardware",
                }

                with patch.object(batch, "ROOT", root), patch.object(batch, "EVIDENCE", evidence_path):
                    batch.record_evidence([(case, row)])

                contents = target.read_text().splitlines()
                self.assertEqual(len(contents), 2)
                self.assertIn("probes/new.rs", contents[1])
                self.assertNotIn(scope, contents[1])


if __name__ == "__main__":
    unittest.main()
