"""Native Windows upgrade regression; fake Codex client, no model or user settings."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from common import REPO, findex_binary
from mcp_client import McpProcess


@unittest.skipUnless(os.name == "nt", "Windows executable locking")
class CodexInstallerTests(unittest.TestCase):
    def test_locked_runtime_upgrade_repeat_and_corrupt_cache_preserve_configuration(self):
        binary = findex_binary("current").resolve()
        self.assertTrue(binary.is_file(), "Build the release binary before offline validation")
        with tempfile.TemporaryDirectory(prefix="findex-install-") as folder:
            root = Path(folder)
            source = root / "Source plugin with spaces"
            shutil.copytree(REPO / "plugin", source, ignore=shutil.ignore_patterns("bin"))
            (source / "bin").mkdir()
            shutil.copy2(binary, source / "bin/findex.exe")
            appdata = root / "Local App Data"
            installed = appdata / "Findex/codex-plugin-marketplace/plugin"
            (installed / "bin").mkdir(parents=True)
            legacy = installed / "bin/findex.exe"
            shutil.copy2(binary, legacy)
            config = installed / ".mcp.codex.json"
            config.write_text('{"prior":"preserve if runtime validation fails"}', encoding="utf-8")
            fake = root / "fake-codex.ps1"
            fake.write_text("Write-Output '{}'\nexit 0\n", encoding="utf-8")
            env = dict(os.environ, LOCALAPPDATA=str(appdata), CODEX_HOME=str(root / "Codex home"))
            argv = ["powershell.exe", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
                    "-File", str(source / "scripts/install-codex.ps1"), "-Binary", str(binary), "-Codex", str(fake)]

            def install():
                return subprocess.run(argv, env=env, cwd=root, capture_output=True,
                                      text=True, encoding="utf-8", errors="replace", timeout=30)

            old = McpProcess(legacy, root, env)
            current = None
            try:
                upgraded = install()
                self.assertEqual(upgraded.returncode, 0, upgraded.stderr)
                bound = Path(json.loads(config.read_text(encoding="utf-8"))["mcpServers"]["findex"]["command"])
                catalog = appdata / "Findex/codex-plugin-marketplace/.agents/plugins/marketplace.json"
                self.assertEqual(json.loads(catalog.read_text(encoding="utf-8"))["name"], "findex-local")
                digest = hashlib.sha256(binary.read_bytes()).hexdigest()
                self.assertEqual(bound.parent.name, digest)
                self.assertEqual(hashlib.sha256(bound.read_bytes()).hexdigest(), digest)
                self.assertNotEqual(bound, legacy)
                self.assertEqual(old.request("ping")[0]["result"], {})
                current = McpProcess(bound, root, env)
                repeated = install()
                self.assertEqual(repeated.returncode, 0, repeated.stderr)
                self.assertEqual(current.request("ping")[0]["result"], {})
            finally:
                if current:
                    current.close()
                old.close()
            previous = config.read_bytes()
            bound.write_bytes(b"tampered runtime")
            refused = install()
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn("runtime hash mismatch", refused.stderr)
            self.assertEqual(config.read_bytes(), previous)


if __name__ == "__main__":
    unittest.main()
