"""Shared helpers for the Findex validation harness (stdlib + psutil only).

Every run writes into validation/results/<UTC>-<sha>/. Nothing here reads
credentials, Claude session contents or network resources.
"""
from __future__ import annotations

import datetime as _dt
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
VALIDATION = REPO / "validation"
WORK = Path(os.environ.get("FINDEX_VALIDATION_HOME") or Path(os.environ["LOCALAPPDATA"]) / "findex-validation")
CORPORA = WORK / "corpora"

# Pinned, legally redistributable benchmark corpora (shallow clones of tags).
CORPUS_PINS = {
    "findex": ("https://github.com/brutalstein/findex.git", "44f1a38", "Rust/mixed, MIT"),
    "ripgrep": ("https://github.com/BurntSushi/ripgrep.git", "14.1.1", "Rust, MIT/Unlicense"),
    "requests": ("https://github.com/psf/requests.git", "v2.32.3", "Python, Apache-2.0"),
    "zod": ("https://github.com/colinhacks/zod.git", "v3.23.8", "TypeScript, MIT"),
    "fmt": ("https://github.com/fmtlib/fmt.git", "11.0.2", "C++, MIT"),
    "pybind11": ("https://github.com/pybind/pybind11.git", "v2.13.6", "C++/Python mixed, BSD-3"),
    "tokio": ("https://github.com/tokio-rs/tokio.git", "tokio-1.40.0", "Rust large, MIT"),
    "vscode": ("https://github.com/microsoft/vscode.git", "1.94.0", "TypeScript monorepo, MIT"),
}


def utc_stamp() -> str:
    return _dt.datetime.now(_dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def git(*args: str, cwd: Path = REPO) -> str:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=False).stdout.strip()


def results_dir(explicit: str | None = None) -> Path:
    """Reuse FINDEX_RESULTS_DIR so one run-all invocation shares a folder."""
    chosen = explicit or os.environ.get("FINDEX_RESULTS_DIR")
    if chosen:
        path = Path(chosen)
    else:
        path = VALIDATION / "results" / f"{utc_stamp()}-{git('rev-parse', '--short=12', 'HEAD')}"
    path.mkdir(parents=True, exist_ok=True)
    return path


def write_json(path: Path, value) -> None:
    # Results are committed: never leak private machine paths.
    path.write_text(sanitize(json.dumps(value, indent=2, sort_keys=False, default=str)) + "\n", encoding="utf-8",
                    newline="\n")  # LF on every OS: checksums must verify after checkout


def sanitize(text: str) -> str:
    """Replace private machine paths before anything is committed."""
    for marker, label in (
        (str(WORK), "%FINDEX_VALIDATION_HOME%"),
        (os.environ.get("LOCALAPPDATA", "\0"), "%LOCALAPPDATA%"),
        (str(REPO), "%REPO%"),
        (os.environ.get("USERPROFILE", "\0"), "%USERPROFILE%"),
    ):
        if marker and marker != "\0":
            text = text.replace(marker, label).replace(marker.replace("\\", "/"), label)
            text = text.replace(marker.replace("\\", "\\\\"), label)
    return text


class CommandLog:
    """Append-only commands.jsonl: argv, cwd, exit code, duration, timeout."""

    def __init__(self, out: Path):
        self.path = out / "commands.jsonl"

    def run(self, argv: list[str], cwd: Path | None = None, timeout: float | None = None,
            env: dict | None = None, input_text: str | None = None) -> subprocess.CompletedProcess:
        started = time.perf_counter()
        timed_out = False
        try:
            proc = subprocess.run(argv, cwd=cwd, capture_output=True, text=True, encoding="utf-8",
                                  errors="replace", timeout=timeout, env=env, input=input_text)
        except subprocess.TimeoutExpired as exc:
            timed_out = True
            proc = subprocess.CompletedProcess(argv, -1, exc.stdout or "", exc.stderr or "")
        self.record(argv, cwd, proc.returncode, time.perf_counter() - started, timed_out)
        return proc

    def record(self, argv, cwd, code, seconds, timed_out=False, note=None):
        entry = {"utc": utc_stamp(), "argv": [sanitize(str(a)) for a in argv],
                 "cwd": sanitize(str(cwd or Path.cwd())), "exit_code": code,
                 "duration_s": round(seconds, 4), "timed_out": timed_out}
        if note:
            entry["note"] = note
        with self.path.open("a", encoding="utf-8", newline="\n") as handle:
            handle.write(json.dumps(entry) + "\n")


def tool_version(argv: list[str]) -> str | None:
    if not shutil.which(argv[0]):
        return None
    try:
        out = subprocess.run(argv, capture_output=True, text=True, timeout=30)
        return (out.stdout or out.stderr).strip().splitlines()[0]
    except (OSError, subprocess.TimeoutExpired, IndexError):
        return None


def environment() -> dict:
    import psutil

    info: dict = {
        "utc": utc_stamp(),
        "git_sha": git("rev-parse", "HEAD"),
        "git_dirty": bool(git("status", "--porcelain")),
        "git_branch": git("rev-parse", "--abbrev-ref", "HEAD"),
        "os": platform.platform(),
        "windows_build": platform.version(),
        "machine": platform.machine(),
        "logical_processors": psutil.cpu_count(logical=True),
        "physical_cores": psutil.cpu_count(logical=False),
        "ram_bytes": psutil.virtual_memory().total,
        "python": sys.version.split()[0],
        "rustc": tool_version(["rustc", "--version"]),
        "rustc_host": next((line.split(": ", 1)[1] for line in subprocess.run(
            ["rustc", "-vV"], capture_output=True, text=True).stdout.splitlines() if line.startswith("host: ")), None),
        "cargo": tool_version(["cargo", "--version"]),
        "git": tool_version(["git", "--version"]),
        "claude_code": tool_version(["claude", "--version"]),
    }
    if sys.platform == "win32":
        ps = ("Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name;"
              "Get-PhysicalDisk | ForEach-Object { $_.MediaType + ' ' + $_.BusType + ' ' + $_.Model }")
        out = subprocess.run(["powershell", "-NoProfile", "-Command", ps], capture_output=True, text=True)
        lines = [line.strip() for line in out.stdout.splitlines() if line.strip()]
        info["cpu_model"] = lines[0] if lines else None
        info["storage"] = lines[1:]
    # Authentication *availability* only: the method name, never tokens.
    if shutil.which("claude"):
        try:
            status = json.loads(subprocess.run(["claude", "auth", "status"], capture_output=True,
                                               text=True, timeout=60).stdout)
            info["claude_auth"] = {"logged_in": status.get("loggedIn"), "method": status.get("authMethod"),
                                   "plan": status.get("subscriptionType")}
        except (ValueError, OSError, subprocess.TimeoutExpired):
            info["claude_auth"] = {"logged_in": None}
    info["ci_run_url"] = os.environ.get("GITHUB_SERVER_URL", "") and (
        f"{os.environ['GITHUB_SERVER_URL']}/{os.environ.get('GITHUB_REPOSITORY')}/actions/runs/{os.environ.get('GITHUB_RUN_ID')}")
    return info


def percentiles(values: list[float]) -> dict:
    if not values:
        return {"n": 0}
    ordered = sorted(values)
    n = len(ordered)

    def pick(q: float) -> float:
        # Nearest-rank percentile: no interpolation across samples.
        return ordered[min(n - 1, max(0, int(-(-q * n // 1)) - 1))]

    mean = sum(ordered) / n
    variance = sum((v - mean) ** 2 for v in ordered) / (n - 1) if n > 1 else 0.0
    return {"n": n, "mean": mean, "variance": variance, "stdev": variance ** 0.5, "min": ordered[0],
            "p50": pick(0.50), "p95": pick(0.95), "p99": pick(0.99), "max": ordered[-1]}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def write_checksums(out: Path) -> None:
    lines = [f"{sha256_file(p)}  {p.name}" for p in sorted(out.iterdir())
             if p.is_file() and p.name != "checksums.sha256"]
    (out / "checksums.sha256").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")


def findex_binary(label: str) -> Path:
    """`current` = this checkout's release build; `baseline` = frozen 44f1a38 build."""
    if label == "baseline":
        return WORK / "bin" / "baseline-44f1a38" / "findex.exe"
    target = Path(os.environ.get("CARGO_TARGET_DIR") or REPO / "target")
    return target / "release" / "findex.exe"


def isolated_env(state_dir: Path) -> dict:
    """Findex stores its index under %LOCALAPPDATA%; isolate each system/run."""
    env = dict(os.environ)
    state_dir.mkdir(parents=True, exist_ok=True)
    env["LOCALAPPDATA"] = str(state_dir)
    env.pop("FINDEX_CLAUDE_CHILD", None)
    return env
