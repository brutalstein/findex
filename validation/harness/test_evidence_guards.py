"""Offline regressions: no model process, authentication, or provider access."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import codex_integration
import tool_comparison as local
import tool_comparison_live as live


class EvidenceGuardsTests(unittest.TestCase):
    def test_fixture_requires_success_and_exact_definition_source(self):
        definition = "definition fixture.rs:1 [function codex_plugin_fixture] pub fn codex_plugin_fixture() {}"
        result = {"isError": False, "content": [{"type": "text", "text": definition}]}
        self.assertTrue(codex_integration.fixture_evidence(result))
        self.assertFalse(codex_integration.fixture_evidence(dict(result, isError=True)))
        self.assertFalse(codex_integration.fixture_evidence({"isError": False, "content": [
            {"type": "text", "text": "Error: fixture.rs codex_plugin_fixture could not be read"}]}))
        self.assertFalse(codex_integration.fixture_evidence({"isError": False, "content": [
            {"type": "text", "text": definition.replace("fixture.rs:1", "fixture.rs:2") }]}))

    def test_claude_requires_exit_usage_and_successful_supplied_tool(self):
        query = {"id": "q", "expected": [["a.rs", 1]]}
        events = [
            {"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "call", "name": "mcp__findex__findex_search"}]}},
            {"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "call", "content": "source", "is_error": False}]}},
            {"type": "result", "is_error": False, "usage": {"input_tokens": 10, "output_tokens": 3}, "result": '{"path":"a.rs","line":1}'}]
        session = {"events": events, "exit_code": 0, "timed_out": False, "wall_s": 0}
        good = live.claude_record(session, "findex", query)
        self.assertTrue(good["valid"])
        self.assertTrue(good["correct"])
        self.assertEqual(good["mcp_calls"], 1)
        for changed in (dict(session, events=[]), dict(session, exit_code=1),
                        dict(session, events=events[:-1]), dict(session, timed_out=True)):
            with self.subTest(changed=changed):
                self.assertFalse(live.claude_record(changed, "findex", query)["valid"])
        events[1]["message"]["content"][0]["is_error"] = True
        bad = live.claude_record(session, "findex", query)
        self.assertFalse(bad["valid"])
        self.assertFalse(bad["correct"])
        self.assertEqual(bad["mcp_calls"], 0)
        self.assertEqual(bad["summary"]["mcp_call_count"], 0)
        events[1]["message"]["content"][0]["is_error"] = False
        self.assertFalse(live.claude_record(session, "graphify", query)["valid"])

    def test_codex_does_not_count_completed_error_results(self):
        query = {"id": "q", "expected": [["a.rs", 1]]}
        events = [{"type": "item.completed", "item": {"type": "mcp_tool_call", "server": "findex", "status": "completed",
                   "result": {"isError": True}, "error": None}},
                  {"type": "turn.completed", "usage": {"input_tokens": 10, "output_tokens": 3}}]
        record = live.codex_record(0, events, "findex", query)
        self.assertFalse(record["valid"])
        self.assertEqual(record["mcp_calls"], 0)
        events[0]["item"]["result"] = {"content": [{"type": "text", "text": "Tool requires approval"}]}
        self.assertFalse(live.codex_record(0, events, "findex", query)["valid"])

    def test_pending_attempt_is_persisted_and_resume_never_repeats_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            saved = Path(tmp) / "live-results.json"
            results = {"sessions": [], "started": 0}
            with self.assertRaises(KeyboardInterrupt):
                with live.trial_attempt(saved, results, {"key": "q", "client": "codex"}, 12):
                    disk = json.loads(saved.read_text())
                    self.assertEqual(disk["started"], 1)
                    self.assertEqual(disk["sessions"][0]["status"], "pending")
                    raise KeyboardInterrupt()
            with self.assertRaises(SystemExit):
                live.load_results(saved)
            self.assertIn("stopped", json.loads(saved.read_text()))

    def test_spawn_failure_and_timeout_are_terminal_and_cap_is_preserved(self):
        for error in (OSError("synthetic spawn failure"), subprocess.TimeoutExpired("synthetic", 1)):
            with self.subTest(error=error), tempfile.TemporaryDirectory() as tmp:
                saved = Path(tmp) / "live-results.json"
                results = {"sessions": [], "started": 11}
                with live.trial_attempt(saved, results, {"key": "q", "client": "codex"}, 12):
                    raise error
                disk = json.loads(saved.read_text())
                self.assertEqual(disk["started"], 12)
                self.assertEqual(disk["sessions"][0]["status"], "invalid")
                self.assertIn("stopped", disk)
                with self.assertRaises(SystemExit):
                    with live.trial_attempt(saved, results, {"key": "another", "client": "codex"}, 12):
                        self.fail("cap allowed another attempt")

    def test_corpus_freeze_refuses_modified_and_untracked_sources(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            (root / "fixture.rs").write_text("fn original() {}\n")
            for args in (["add", "."], ["-c", "user.name=offline", "-c", "user.email=offline@local", "commit", "-qm", "fixture"]):
                subprocess.run(["git", *args], cwd=root, check=True)
            identity = local.corpus_identity(root)
            self.assertIn("fixture.rs", identity["admitted_sha256"])
            (root / "fixture.rs").write_text("fn changed() {}\n")
            with self.assertRaises(ValueError):
                local.corpus_identity(root)
            (root / "fixture.rs").write_text("fn original() {}\n")
            (root / "untracked.rs").write_text("fn extra() {}\n")
            with self.assertRaises(ValueError):
                local.corpus_identity(root)

    def test_tool_identity_records_hash_and_version_and_run_refuses_drift(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            fake = root / "tool.exe"
            fake.write_bytes(b"synthetic executable")
            with patch.object(local, "invoke", return_value=("tool 1.0\n", 0)):
                before = local.tool_identity(fake)
                fake.write_bytes(b"changed executable")
                after = local.tool_identity(fake)
            self.assertEqual(before["version"], "tool 1.0")
            self.assertNotEqual(before["sha256"], after["sha256"])
            manifest = {"identity": {"tools": {"findex": before}, "corpora": {}}}
            local.require_frozen_identity(manifest, manifest["identity"], root)
            with self.assertRaises(ValueError):
                local.require_frozen_identity(manifest, {"tools": {"findex": after}, "corpora": {}}, root)
            changed_version = dict(before, version="tool 2.0")
            with self.assertRaises(ValueError):
                local.require_frozen_identity(manifest, {"tools": {"findex": changed_version}, "corpora": {}}, root)
            with self.assertRaises(ValueError):
                local.require_frozen_identity(manifest, {"tools": {"findex": before}, "corpora": {"new": {}}}, root)
            with self.assertRaisesRegex(ValueError, "historical|Historical|legacy|Legacy"):
                local.require_frozen_identity({}, manifest["identity"], root)
            (root / "local-results.json").write_text("historical measurement")
            with self.assertRaises(ValueError):
                local.require_frozen_identity(manifest, manifest["identity"], root)


if __name__ == "__main__":
    unittest.main()
