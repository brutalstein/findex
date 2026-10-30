"""A runtime that needs a compiler's DLL is not a portable runtime."""
import os
import sys
import unittest
from pathlib import Path

from pe_imports import imported_dlls, require_system_only


@unittest.skipUnless(sys.platform == "win32", "PE files are checked on Windows")
class PeImportTests(unittest.TestCase):
    def test_a_windows_system_program_imports_only_system_dlls(self):
        cmd = Path(os.environ["SystemRoot"]) / "System32" / "cmd.exe"
        self.assertIn("ntdll.dll", [name.lower() for name in imported_dlls(cmd)])
        require_system_only(cmd)

    def test_a_program_bound_to_a_side_by_side_dll_is_rejected_by_name(self):
        # python.exe imports python3x.dll, which lives next to it, not in System32.
        with self.assertRaises(ValueError) as caught:
            require_system_only(Path(sys.executable))
        self.assertIn("python3", str(caught.exception).lower())
        self.assertIn("MSVC", str(caught.exception))

    def test_a_file_that_is_not_a_pe_image_is_an_error(self):
        with self.assertRaises(ValueError):
            imported_dlls(Path(__file__))


if __name__ == "__main__":
    unittest.main()
