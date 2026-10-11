"""Transparent stdio proxy: Claude Code <-> findex.exe mcp.

Records only protocol metadata (direction, method, id, protocolVersion,
tool name, byte size, error code) to FINDEX_TRACE_FILE. Tool arguments and
results are not stored. Usage (as an MCP server command):
    python mcp_trace_proxy.py <findex.exe> mcp
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import threading
import time

TRACE = os.environ.get("FINDEX_TRACE_FILE")
LOCK = threading.Lock()


def record(direction: str, line: bytes) -> None:
    if not TRACE:
        return
    entry = {"t": round(time.time(), 3), "dir": direction, "bytes": len(line)}
    try:
        message = json.loads(line)
        if isinstance(message, dict):
            entry.update({k: message.get(k) for k in ("id", "method") if k in message})
            params = message.get("params") or {}
            if isinstance(params, dict):
                if "protocolVersion" in params:
                    entry["protocolVersion"] = params["protocolVersion"]
                meta = params.get("_meta") or {}
                if "io.modelcontextprotocol/protocolVersion" in meta:
                    entry["metaProtocolVersion"] = meta["io.modelcontextprotocol/protocolVersion"]
                if message.get("method") == "tools/call":
                    entry["tool"] = params.get("name")
                if "clientInfo" in params:
                    entry["clientInfo"] = params["clientInfo"]
            result = message.get("result")
            if isinstance(result, dict):
                for key in ("protocolVersion", "isError"):
                    if key in result:
                        entry[key] = result[key]
                if "tools" in result:
                    entry["tools"] = [tool.get("name") for tool in result["tools"]]
            if "error" in message:
                entry["error_code"] = message["error"].get("code")
    except ValueError:
        entry["unparsed"] = True
    with LOCK, open(TRACE, "a", encoding="utf-8") as handle:
        handle.write(json.dumps(entry) + "\n")


def main() -> int:
    child = subprocess.Popen(sys.argv[1:], stdin=subprocess.PIPE, stdout=subprocess.PIPE)

    def pump_out():
        for line in child.stdout:
            record("server->client", line)
            sys.stdout.buffer.write(line)
            sys.stdout.buffer.flush()

    thread = threading.Thread(target=pump_out, daemon=True)
    thread.start()
    for line in sys.stdin.buffer:
        record("client->server", line)
        child.stdin.write(line)
        child.stdin.flush()
    child.stdin.close()
    code = child.wait()
    thread.join(timeout=5)
    return code


if __name__ == "__main__":
    raise SystemExit(main())
