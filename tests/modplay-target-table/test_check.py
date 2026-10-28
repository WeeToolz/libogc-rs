import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location('table_check', Path(__file__).with_name('check.py'))
table_check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(table_check)


class FrequencyTableCheckTests(unittest.TestCase):
    def test_rejects_a_change_at_every_byte(self):
        expected = table_check.EXPECTED_TABLE
        self.assertEqual(len(expected), 280)
        table_check.check_table(expected)
        for index in range(len(expected)):
            changed = bytearray(expected)
            changed[index] ^= 1
            with self.subTest(index=index), self.assertRaises(AssertionError):
                table_check.check_table(changed)

    def test_rejects_truncated_and_extended_tables(self):
        for data in (table_check.EXPECTED_TABLE[:-1], table_check.EXPECTED_TABLE + b'\0'):
            with self.subTest(length=len(data)), self.assertRaises(AssertionError):
                table_check.check_table(data)


if __name__ == '__main__':
    unittest.main()
