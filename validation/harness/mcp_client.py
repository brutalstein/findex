"""Minimal newline-delimited JSON-RPC client for `findex mcp` over stdio."""
from __future__ import annotations

import json
import subprocess
import threading
import time
from pathlib import Path

import psutil


class McpProcess:
    def __init__(self, exe: Path, project: Path, env: dict, legacy_init: bool = True):
        self.proc = subprocess.Popen([str(exe), "mcp"], cwd=project, env=env, stdin=subprocess.PIPE,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.ps = psutil.Process(self.proc.pid)
        self.stderr: list[bytes] = []
        threading.Thread(target=self._drain_stderr, daemon=True).start()
        self.next_id = 0
        if legacy_init:
            self.request("initialize", {"protocolVersion": "2025-11-25", "capabilities": {},
                                        "clientInfo": {"name": "findex-validation", "version": "1"}})
            self.notify("notifications/initialized")

    def _drain_stderr(self):
        for line in self.proc.stderr:
            self.stderr.append(line)

    def notify(self, method: str, params: dict | None = None):
        self._write({"jsonrpc": "2.0", "method": method, **({"params": params} if params else {})})

    def _write(self, message: dict | bytes):
        data = message if isinstance(message, bytes) else json.dumps(message).encode()
        self.proc.stdin.write(data + b"\n")
        self.proc.stdin.flush()

    def raw(self, data: bytes, timeout: float = 120.0) -> dict | None:
        """Send arbitrary bytes and read one response line (adversarial tests)."""
        self.proc.stdin.write(data)
        self.proc.stdin.flush()
        return self._read(timeout)

    def _read(self, timeout: float) -> dict | None:
        result: list = []

        def reader():
            result.append(self.proc.stdout.readline())

        thread = threading.Thread(target=reader, daemon=True)
        thread.start()
        thread.join(timeout)
        if not result or not result[0]:
            return None
        return json.loads(result[0])

    def request(self, method: str, params: dict | None = None, timeout: float = 600.0):
        self.next_id += 1
        message = {"jsonrpc": "2.0", "id": self.next_id, "method": method}
        if params is not None:
            message["params"] = params
        started = time.perf_counter()
        self._write(message)
        response = self._read(timeout)
        return response, time.perf_counter() - started

    def call_tool(self, name: str, arguments: dict, timeout: float = 600.0):
        """Returns (parsed payload or None, raw text, seconds, is_error)."""
        response, seconds = self.request("tools/call", {"name": name, "arguments": arguments}, timeout)
        if response is None or "result" not in response:
            return None, json.dumps(response), seconds, True
        text = response["result"]["content"][0]["text"]
        try:
            payload = json.loads(text)
        except ValueError:
            payload = None
        return payload, text, seconds, bool(response["result"].get("isError"))

    def memory(self) -> dict:
        info = self.ps.memory_info()
        return {"rss": info.rss, "peak_wset": getattr(info, "peak_wset", None),
                "private": getattr(info, "private", None)}

    def cpu_seconds(self) -> float:
        times = self.ps.cpu_times()
        return times.user + times.system

    def io(self) -> dict:
        counters = self.ps.io_counters()
        return {"read_bytes": counters.read_bytes, "write_bytes": counters.write_bytes}

    def close(self) -> int:
        try:
            self.proc.stdin.close()
        except OSError:
            pass
        try:
            return self.proc.wait(timeout=30)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            return self.proc.wait()
