"""Four explicitly gated subscription starts: old/new Findex, two real bugs.

Reuses the existing native sandbox preflight, process recorder and source-only
host grader. Never enables competitor servers or retries a model session.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "validation/harness"))
import bugfix_live as pilot
from bugfix_fixtures import TASKS, fixture_env, run_repro
from common import write_json
from mcp_client import McpProcess

APPROVAL_ENV = "FINDEX_CODEX_PAIR_APPROVAL"
APPROVAL = "I approve 4 Codex subscription bugfix sessions"
OUT = Path(__file__).resolve().parent / "live"
WORK = ROOT / "target/retrieval-delivery-live"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def configure():
    manifest = json.loads((OUT / "pilot-manifest.json").read_text())
    binaries = {key: ROOT / path for key, path in manifest["frozen_tools"].items() if key != "codex"}
    binaries["codex"] = Path(os.environ["CODEX_APP_RUNTIME_CLI_PATH"])
    pilot.OUT, pilot.CAP, pilot.TOOLS = OUT, 4, ("findex",)
    pilot.locations = lambda: binaries
    pilot.server = lambda trial: {"command": str(ROOT / trial["binary"]), "args": ["mcp"],
        "env": {"LOCALAPPDATA": str((ROOT / trial["root"]).parent / "findex-state")}}
    def native_env(trial):
        env = pilot.bugfix_env(fixture_env((ROOT / trial["root"]).parent / "host-state"))
        env["LOCALAPPDATA"] = os.environ["LOCALAPPDATA"]
        return env
    pilot.native_env = native_env
    return manifest


def prepare():
    OUT.mkdir()
    (OUT / "raw").mkdir()
    WORK.mkdir()
    tools = {"codex": Path(os.environ["CODEX_APP_RUNTIME_CLI_PATH"]),
             "findex": ROOT / "target/release/findex.exe",
             "baseline": ROOT / "target/retrieval-integrity/baseline/findex.exe"}
    frozen = {}
    for name, source in tools.items():
        if name == "codex":
            frozen[name] = source
            continue
        target = WORK / "bin" / name / "findex.exe"
        target.parent.mkdir(parents=True)
        shutil.copyfile(source, target)
        frozen[name] = target
    # Preserve the explicit project policy; do not change the user's config.
    profile = {"model": "gpt-5.6-sol", "model_reasoning_effort": "high", "service_tier": "priority"}
    manifest = {"authorization": "User instructed direct delivery after the four omitted delivery actions; at most four new subscription starts.",
        "profile": profile, "max_sessions": 4, "timeout_seconds": 300,
        "semantic_preparation": "not_requested", "frozen_tools": {},
        "tool_sha256": {name: digest(path) for name, path in frozen.items()}, "trials": [],
        "limits": ["Two reused curated tasks; not a fresh holdout or broad superiority test.",
                   "One run per task and binary, no retries, no API billing or model fallback.",
                   "Compiler-backed semantic and 100k native tests remain separate evidence."]}
    for name, path in frozen.items():
        manifest["frozen_tools"][name] = str(path) if name == "codex" else path.relative_to(ROOT).as_posix()
    # configure needs repo-relative tool paths; the native CLI path is absolute.
    write_json(OUT / "pilot-manifest.json", manifest)
    pilot.OUT, pilot.CAP, pilot.TOOLS = OUT, 4, ("findex",)
    for index, (task, spec) in enumerate(TASKS.items()):
        for arm in (("baseline", "candidate") if index == 0 else ("candidate", "baseline")):
            started = time.monotonic()
            project = WORK / task / arm / "project"
            seed = ROOT / "target/quality/live-bugfix/seeds" / task
            shutil.copytree(seed, project, ignore=shutil.ignore_patterns(".git", "target", "node_modules", "graphify-out", "__pycache__", "*.pyc"))
            (project / "repro.py").write_text(spec["repro"], encoding="utf-8", newline="\n")
            for argv in (["git", "init", "-q"], ["git", "add", "."],
                         ["git", "-c", "user.name=Findex validation", "-c", "user.email=validation@local.invalid", "commit", "-qm", "Frozen bugfix fixture"]):
                subprocess.run(argv, cwd=project, check=True, capture_output=True)
            env = fixture_env(project.parent / "grader-state")
            red = run_repro(project, env)
            (OUT / "raw" / f"{task}-{arm}-red.log").write_text(red.stdout + red.stderr, encoding="utf-8")
            expected = "FileExistsError" if task.startswith("requests") else "AssertionError"
            assert red.returncode != 0 and expected in red.stderr, red.stderr[-1200:]
            binary = frozen["baseline" if arm == "baseline" else "findex"]
            client = McpProcess(binary, project, dict(env, LOCALAPPDATA=str(project.parent / "findex-state")))
            try:
                response = client.call_tool("findex_context", {"task": "certificate extraction" if task.startswith("requests") else "fish completions", "budget_tokens": 1200})
                assert not response[3], response[1]
            finally:
                client.close()
            git = lambda *args: subprocess.run(["git", *args], cwd=project, check=True, capture_output=True, text=True).stdout.strip()
            manifest["trials"].append({"key": task + ":" + arm, "task": task, "tool": "findex", "arm": arm,
                "binary": binary.relative_to(ROOT).as_posix(), "root": project.relative_to(ROOT).as_posix(),
                "baseline_head": git("rev-parse", "HEAD"), "source_tree": git("write-tree"),
                "repro_sha256": digest(project / "repro.py"), "setup_seconds": round(time.monotonic() - started, 3)})
            write_json(OUT / "pilot-manifest.json", manifest)
            print("Frozen red oracle:", task, arm, flush=True)
    for task in TASKS:
        assert len({t["source_tree"] for t in manifest["trials"] if t["task"] == task}) == 1


def summary():
    path = OUT / "live-results.json"
    if not path.exists():
        return
    results = json.loads(path.read_text())
    arms = {}
    for arm in ("baseline", "candidate"):
        rows = [r for r in results["sessions"] if r["key"].endswith(":" + arm)]
        arms[arm] = {"starts": len(rows), "valid": sum(bool(r.get("valid")) for r in rows),
            "correct": sum(bool(r.get("correct")) for r in rows),
            "usage_complete": all(bool(r.get("usage")) for r in rows),
            "total_tokens": sum(r.get("usage", {}).get("input_tokens", 0) + r.get("usage", {}).get("output_tokens", 0) for r in rows),
            "seconds": sum(r.get("seconds", 0) for r in rows)}
    write_json(OUT / "paired-summary.json", {"arms": arms, "actual_starts": results["started"],
        "stopped": results.get("stopped"), "limits": "Two reused tasks, one repetition. Incomplete usage prevents whole-study token comparison."})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("prepare", "preflight", "run"))
    parser.add_argument("--enable-live-model-tests", action="store_true")
    args = parser.parse_args()
    if args.mode == "prepare":
        prepare()
        return
    configure()
    if args.mode == "preflight":
        pilot.preflight()
        return
    if not args.enable_live_model_tests or os.environ.get(APPROVAL_ENV) != APPROVAL:
        raise SystemExit("NOT_RUN: explicit four-session live flag and subscription approval required")
    if not (OUT / "authorization.json").exists():
        write_json(OUT / "authorization.json", {"new_starts": 4, "earlier_started_sessions": 0, "scope": "Baseline/candidate Findex only"})
    try:
        pilot.run_live()
    finally:
        summary()


if __name__ == "__main__":
    main()
