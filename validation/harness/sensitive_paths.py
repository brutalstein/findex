"""Native Windows confidential-path admission checks. Zero model/API sessions."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def run(exe: Path, project: Path, env: dict[str, str], *argv: str) -> subprocess.CompletedProcess[str]:
    p = subprocess.run(
        [str(exe), *argv], cwd=project, env=env,
        capture_output=True, text=True, timeout=180,
        encoding="utf-8", errors="replace",
    )
    if p.returncode != 0:
        raise AssertionError(f"{argv}: {p.returncode}: {p.stderr[-900:]}")
    return p


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path)
    options = parser.parse_args()
    exe = options.binary.resolve(strict=True)
    report: dict[str, object] = {"model_sessions": 0, "paid_api_calls": 0}
    sensitive = [
        ".aws/credentials", ".kube/config", ".docker/config.json",
        ".ssh/id_custom", ".azure/accessTokens.json",
        ".gcloud/application_default_credentials.json",
        ".terraform/terraform.tfstate", "config/prod.tfvars",
        "config/backup.tfstate.backup", "config/.npmrc",
        "config/.pypirc", "config/.netrc", "config/.envrc",
        "config/team.keystore", "config/vault.kdbx",
        "config/service-account.json", "config/firebase-adminsdk-prod.json",
    ]
    with tempfile.TemporaryDirectory(prefix="findex-secret-boundary-") as tmp:
        root = Path(tmp)
        project = root / "workspace"
        project.mkdir()
        env = os.environ.copy()
        env["LOCALAPPDATA"] = str(root / "state")
        Path(env["LOCALAPPDATA"]).mkdir()
        (project / "src").mkdir()
        (project / "src" / "legitimate.rs").write_text(
            "pub fn ordinary_navigation_marker() {}\n", encoding="utf-8"
        )
        for n, relative in enumerate(sensitive):
            dest = project / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_text(
                f"PRIVATE_DO_NOT_INDEX_{n:03d}\n",
                encoding="utf-8",
            )
        indexed = run(exe, project, env, "index")
        assert "Indexed 1 changed" in indexed.stdout, indexed.stdout
        health = run(exe, project, env, "index-check")
        assert "1 files, 1 documents" in health.stdout, health.stdout
        report["normal_source_admitted"] = "PASS"
        report["secrets_excluded_at_scan"] = len(sensitive)
        for n in range(len(sensitive)):
            result = run(exe, project, env, "search", f"PRIVATE_DO_NOT_INDEX_{n:03d}")
            assert "0 hits" in result.stdout, f"{sensitive[n]}: {result.stdout}"
        legit = run(exe, project, env, "search", "ordinary_navigation_marker")
        assert "src/legitimate.rs" in legit.stdout, legit.stdout
        report["all_sensitive_markers_absent_from_search"] = "PASS"
        report["ordinary_code_still_found"] = "PASS"
    report["status"] = "PASS"
    out = json.dumps(report, indent=2)
    if options.output:
        options.output.parent.mkdir(parents=True, exist_ok=True)
        options.output.write_text(out + "\n", encoding="utf-8")
    print(out)


if __name__ == "__main__":
    main()
