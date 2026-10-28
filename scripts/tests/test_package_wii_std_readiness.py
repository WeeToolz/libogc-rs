import importlib.util
from pathlib import Path
import unittest
import xml.etree.ElementTree as ET

SCRIPT = Path(__file__).resolve().parents[1] / "package-wii-std-readiness.py"
spec = importlib.util.spec_from_file_location("readiness", SCRIPT)
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


class MetadataTests(unittest.TestCase):
    def test_loader_arguments_round_trip_and_xml_escapes_input(self):
        root = ET.fromstring(package.metadata(-14400, "test&host.example:8080", "normal"))
        self.assertEqual([item.text for item in root.findall("arguments/arg")], ["--mode=normal", "--utc-offset-seconds=-14400", "--network-endpoint=test&host.example:8080", "--media=both", "--focus=all"])

    def test_declared_hardware_labels_are_explicit_loader_arguments(self):
        root = ET.fromstring(package.metadata(0, "example.com:80", "normal", "RVL-101 (USA)", "RVK-CPU-01"))
        values = [item.text for item in root.findall("arguments/arg")]
        self.assertIn("--console-model=RVL-101 (USA)", values)
        self.assertIn("--motherboard=RVK-CPU-01", values)
        with self.assertRaises(ValueError):
            package.metadata(0, "example.com:80", "normal", motherboard="board\nspoof")

    def test_selected_medium_is_preserved_and_invalid_selection_is_rejected(self):
        root = ET.fromstring(package.metadata(0, "example.com:80", "normal", media="usb"))
        self.assertIn("--media=usb", [item.text for item in root.findall("arguments/arg")])
        with self.assertRaises(ValueError):
            package.metadata(0, "example.com:80", "normal", media="unknown")

    def test_invalid_settings_do_not_create_a_package(self):
        for arguments in [(86400, "example.com:80", "normal"), (0, "example.com:0", "normal"), (0, "example.com:65536", "normal"), (0, "example.com", "normal"), (0, "example.com:80", "invalid")]:
            with self.assertRaises(ValueError):
                package.metadata(*arguments)


if __name__ == "__main__":
    unittest.main()
