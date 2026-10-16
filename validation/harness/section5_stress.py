"""Offline deterministic Windows index durability stress.

No Claude, external APIs, downloads or paid services. Uses the real release CLI,
an isolated LOCALAPPDATA, 1,200 files by default and multiple separate processes.
All measurements and failures are recorded; this is a workload smoke test and
NOT evidence for untested 100k-file/SMB/cloud-only deployments.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def invoke(exe: Path, workspace: Path, env: dict[str, str],
           *args: str, timeout: int = 180) -> tuple[str, float]:
    began = time.perf_counter()
    proc = subprocess.run(
        [str(exe), *args], cwd=workspace, env=env, capture_output=True,
        text=True, timeout=timeout, encoding="utf-8", errors="replace",
    )
    duration = time.perf_counter() - began
    if proc.returncode != 0:
        raise AssertionError(
            f"{args[0]} exited {proc.returncode}: {proc.stderr[-1200:]}"
        )
    return proc.stdout, round(duration, 4)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("executable", type=Path)
    parser.add_argument("--files", type=int, default=1200)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    if not 200 <= args.files <= 20000:
        parser.error("files must be between 200 and 20000")
    exe = args.executable.resolve(strict=True)
    report: dict = {"files_requested": args.files,
                    "model_sessions": 0, "provider_cost": 0,
                    "status": "NOT_RUN", "steps": {}}
    with tempfile.TemporaryDirectory(prefix="findex-section5-") as directory:
        root = Path(directory)
        workspace = root / "workspace"
        pkg = workspace / "pkg"
        pkg.mkdir(parents=True)
        env = os.environ.copy()
        env["LOCALAPPDATA"] = str(root / "localappdata")
        (root / "localappdata").mkdir()
        for i in range(args.files):
            (pkg / f"{i:05}.rs").write_text(
                f"pub fn marker_original_{i:05}() -> u32 {{ {i} }}\n",
                encoding="utf-8",
            )
        initial, elapsed = invoke(exe, workspace, env, "index")
        report["steps"]["cold_scan_s"] = elapsed
        assert f"Indexed {args.files} changed" in initial, initial
        health, elapsed = invoke(exe, workspace, env, "index-check")
        report["steps"]["initial_health_s"] = elapsed
        assert f"{args.files} files, {args.files} documents" in health, health

        update_count = min(130, args.files // 5)
        delete_count = min(80, args.files // 8)
        rename_count = min(60, args.files // 10)
        for i in range(update_count):
            (pkg / f"{i:05}.rs").write_text(
                f"pub fn marker_updated_{i:05}() -> u32 {{ {i + 1} }}\n",
                encoding="utf-8",
            )

        # Explicitly target known paths without walking the whole repository.
        paths = [f"pkg/{i:05}.rs" for i in range(update_count)]
        refreshed, elapsed = invoke(exe, workspace, env, "refresh", *paths)
        report["steps"]["known_paths_refresh_s"] = elapsed
        assert f"Refreshed {update_count} changed" in refreshed, refreshed

        for i in range(update_count, update_count + delete_count):
            (pkg / f"{i:05}.rs").unlink()
        for i in range(update_count + delete_count,
                       update_count + delete_count + rename_count):
            (pkg / f"{i:05}.rs").rename(pkg / f"renamed_{i:05}.rs")

        reconciled, elapsed = invoke(exe, workspace, env, "index")
        report["steps"]["mass_change_reconcile_s"] = elapsed
        expected_files = args.files - delete_count
        assert f"{rename_count} changed" in reconciled, reconciled
        expected_removed = delete_count + rename_count
        assert f"{expected_removed} removed" in reconciled, reconciled
        health, elapsed = invoke(exe, workspace, env, "index-check")
        report["steps"]["mass_change_health_s"] = elapsed
        assert f"{expected_files} files, {expected_files} documents" in health

        stale, _ = invoke(exe, workspace, env, "search", "marker_original_00000")
        fresh, _ = invoke(exe, workspace, env, "search", "marker_updated_00000")
        assert "0 hits" in stale, stale
        assert "pkg/00000.rs" in fresh, fresh

        # Concurrent process-level reconciliation must not corrupt the database.
        procs = [
            subprocess.Popen([str(exe), "index"], cwd=workspace, env=env,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             text=True)
            for _ in range(2)
        ]
        statuses = []
        for proc in procs:
            out, err = proc.communicate(timeout=180)
            statuses.append({"code": proc.returncode, "stdout": out[:300],
                             "error": err[:300]})
        report["steps"]["concurrent_reconciles"] = statuses
        assert all(x["code"] == 0 for x in statuses), statuses
        health, elapsed = invoke(exe, workspace, env, "index-check")
        assert f"{expected_files} files, {expected_files} documents" in health
        report["steps"]["post_concurrency_health_s"] = elapsed
        report["status"] = "PASS"
        report["coverage"] = {
            "initial_files": args.files, "updated": update_count,
            "deleted": delete_count, "renamed": rename_count,
            "remaining_files": expected_files, "concurrent_clients": 2,
        }
    raw = json.dumps(report, ensure_ascii=False, indent=2)
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(raw + "\n", encoding="utf-8")
    print(raw)
    return 0


if __name__ == "__main__":
    sys.exit(main())
