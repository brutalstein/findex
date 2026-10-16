"""The default validation run must never start a live model client.

Every Python process started below loads an instrumented `sitecustomize`
that records each real process spawn (CPython audit events) and blocks any
spawn of `claude` or `findex claude`. A `claude.cmd`/`claude.bat` shim first
on PATH records launches from PowerShell. Usage: test_live_gate.py
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HARNESS = Path(__file__).resolve().parent
SCRIPTS = HARNESS.parent / "scripts"
sys.path.insert(0, str(HARNESS))

from claude_session import APPROVAL, APPROVAL_ENV, live_model_gate, run_claude, subscription_status  # noqa: E402

SITECUSTOMIZE = r'''
import json, os, sys
_log = os.environ["FINDEX_SPAWN_LOG"]

def _name(value):
    return os.path.splitext(os.path.basename(str(value)))[0].lower()

def _hook(event, args):
    if event == "subprocess.Popen":
        argv = [args[1]] if isinstance(args[1], (str, bytes)) else list(args[1])
        words = [str(a) for a in argv] or [str(args[0])]
        if len(argv) == 1:
            words = str(argv[0]).split()
    elif event == "os.system":
        words = str(args[0]).split()
    else:
        return
    first = _name(words[0]) if words else ""
    live = first == "claude" or (first == "findex" and len(words) > 1 and words[1] == "claude")
    with open(_log, "a", encoding="utf-8") as f:
        f.write(json.dumps({"event": event, "program": first, "live": live}) + "\n")
    if live:
        raise RuntimeError("spawn shim: live model client blocked")

sys.addaudithook(_hook)
'''

STATUS_OK = {"loggedIn": True, "authMethod": "claude.ai", "apiProvider": "firstParty", "subscriptionType": "pro",
             "apiKeySource": "none"}


class Gate(unittest.TestCase):
    def test_off_by_default(self):
        self.assertTrue(live_model_gate(False, {}).startswith("NOT_RUN"))

    def test_ci_never_runs_live_sessions(self):
        env = {APPROVAL_ENV: APPROVAL, "GITHUB_ACTIONS": "true"}
        self.assertTrue(live_model_gate(True, env, STATUS_OK).startswith("BLOCKED"))

    def test_flag_alone_is_not_approval(self):
        self.assertTrue(live_model_gate(True, {}, STATUS_OK).startswith("BLOCKED"))

    def test_api_billing_variables_block(self):
        env = {APPROVAL_ENV: APPROVAL, "ANTHROPIC_API_KEY": "x"}
        self.assertIn("ANTHROPIC_API_KEY", live_model_gate(True, env, STATUS_OK))

    def test_approved_subscription_passes(self):
        self.assertIsNone(live_model_gate(True, {APPROVAL_ENV: APPROVAL}, STATUS_OK))

    def test_ambiguous_sign_in_fails_closed(self):
        self.assertIsNone(subscription_status(STATUS_OK))
        for change in ({"loggedIn": False}, {"authMethod": "apiKey"}, {"apiProvider": "bedrock"},
                       {"subscriptionType": None}, {"apiKeySource": "apiKeyHelper"}):
            self.assertTrue(subscription_status({**STATUS_OK, **change}).startswith("BLOCKED"), change)
        self.assertTrue(subscription_status({}).startswith("BLOCKED"))


class NoLiveSpawn(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="findex-live-gate-"))
        site, shim = self.tmp / "site", self.tmp / "shim"
        site.mkdir()
        shim.mkdir()
        (site / "sitecustomize.py").write_text(SITECUSTOMIZE, encoding="utf-8")
        self.spawn_log, self.shim_log = self.tmp / "spawns.jsonl", self.tmp / "shim.log"
        for ext in ("cmd", "bat"):
            (shim / f"claude.{ext}").write_text(f"@echo shim>>\"{self.shim_log}\"\r\n@exit /b 97\r\n", encoding="ascii")
        self.env = {k: v for k, v in os.environ.items() if k != APPROVAL_ENV}
        self.env.update(PYTHONPATH=str(site), FINDEX_SPAWN_LOG=str(self.spawn_log),
                        PATH=f"{shim}{os.pathsep}{os.environ['PATH']}")

    def spawns(self) -> list[dict]:
        if not self.spawn_log.exists():
            return []
        return [json.loads(line) for line in self.spawn_log.read_text(encoding="utf-8").splitlines()]

    def assert_no_live_client(self):
        self.assertEqual([s for s in self.spawns() if s["live"]], [])
        self.assertFalse(self.shim_log.exists(), "PATH shim for claude was launched")

    def test_shim_detects_a_live_spawn(self):
        # The instrumentation itself must see and block a real attempt.
        code = "import subprocess\ntry:\n    subprocess.run(['claude', '--version'])\nexcept RuntimeError:\n    pass"
        subprocess.run([sys.executable, "-c", code], env=self.env, check=True)
        self.assertEqual([s["program"] for s in self.spawns() if s["live"]], ["claude"])

    def test_study_without_flag_starts_no_process(self):
        out = self.tmp / "ab"
        proc = subprocess.run([sys.executable, str(HARNESS / "claude_ab.py"), "--out", str(out)], env=self.env,
                              capture_output=True, text=True)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(json.loads((out / "tokens.json").read_text(encoding="utf-8"))["status"], "NOT_RUN")
        self.assertEqual(self.spawns(), [])
        self.assert_no_live_client()

    def test_study_in_ci_is_blocked_even_with_flag_and_approval(self):
        out = self.tmp / "ci"
        env = dict(self.env, CI="true", **{APPROVAL_ENV: APPROVAL})
        proc = subprocess.run([sys.executable, str(HARNESS / "claude_ab.py"), "--enable-live-model-tests",
                               "--out", str(out)], env=env, capture_output=True, text=True)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(json.loads((out / "tokens.json").read_text(encoding="utf-8"))["status"], "BLOCKED")
        self.assert_no_live_client()

    def test_default_validation_runner_starts_no_live_client(self):
        out = self.tmp / "all"
        proc = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                               str(SCRIPTS / "run-all.ps1"), "-Steps", "integration,claude-ab", "-OutDir", str(out)],
                              env=self.env, capture_output=True, text=True)
        self.assertEqual(json.loads((out / "tokens.json").read_text(encoding="utf-8"))["status"], "NOT_RUN",
                         proc.stdout + proc.stderr)
        checks = json.loads((out / "compatibility.json").read_text(encoding="utf-8"))["checks"]
        self.assertIn("NOT_RUN", [c["status"] for c in checks if c["name"] == "real Claude Code session"])
        self.assertGreater(len(self.spawns()), 0, "instrumentation saw no harness process")
        self.assert_no_live_client()


class SpawnPoint(unittest.TestCase):
    def test_run_claude_refuses_without_the_gate(self):
        # Fresh interpreter: the gate state is per process.
        code = ("import sys; sys.path.insert(0, sys.argv[1]); import claude_session as c\n"
                "try:\n    c.run_claude(None, 'p', model='m', max_turns=1, budget_usd=0, allowed_tools=[])\n"
                "except RuntimeError as e:\n    print('refused', e)")
        out = subprocess.run([sys.executable, "-c", code, str(HARNESS)], capture_output=True, text=True).stdout
        self.assertTrue(out.startswith("refused"), out)
        self.assertTrue(callable(run_claude))


if __name__ == "__main__":
    unittest.main(verbosity=2)
