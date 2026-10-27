"""Cloud-only (OneDrive Files On-Demand style) placeholders are never downloaded.

Registers a throwaway sync root with the Windows Cloud Files API (the API
OneDrive, Dropbox and Google Drive use), connects a provider that serves file
contents and counts every download (FETCH_DATA) request, then runs the native
Findex binary against it. A cloud-only file must not be read, and so never
downloaded, by indexing, queries or the MCP server; a file that is made local
is indexed; a file that goes back to the cloud leaves the index.

The sync root lives in a temporary directory outside any real sync client's
folder and is unregistered afterwards. LOCALAPPDATA is isolated.
"""
from __future__ import annotations

import argparse
import ctypes
from ctypes import wintypes as w
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import uuid

RECALL_ON_DATA_ACCESS = 0x0040_0000
cf = ctypes.WinDLL("cldapi.dll")
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
kernel32.CreateFileW.restype = w.HANDLE


class GUID(ctypes.Structure):
    _fields_ = [("d1", w.DWORD), ("d2", w.WORD), ("d3", w.WORD), ("d4", ctypes.c_ubyte * 8)]


class SyncRegistration(ctypes.Structure):
    _fields_ = [("StructSize", w.ULONG), ("ProviderName", w.LPCWSTR), ("ProviderVersion", w.LPCWSTR),
                ("SyncRootIdentity", ctypes.c_void_p), ("SyncRootIdentityLength", w.DWORD),
                ("FileIdentity", ctypes.c_void_p), ("FileIdentityLength", w.DWORD), ("ProviderId", GUID)]


class SyncPolicies(ctypes.Structure):
    _fields_ = [("StructSize", w.ULONG), ("HydrationPrimary", w.USHORT), ("HydrationModifier", w.USHORT),
                ("PopulationPrimary", w.USHORT), ("PopulationModifier", w.USHORT), ("InSync", w.ULONG),
                ("HardLink", w.ULONG), ("PlaceholderManagement", w.ULONG)]


class BasicInfo(ctypes.Structure):
    _fields_ = [("Creation", ctypes.c_longlong), ("Access", ctypes.c_longlong),
                ("Write", ctypes.c_longlong), ("Change", ctypes.c_longlong), ("Attributes", w.DWORD)]


class FsMetadata(ctypes.Structure):
    _fields_ = [("Basic", BasicInfo), ("FileSize", ctypes.c_longlong)]


class PlaceholderCreateInfo(ctypes.Structure):
    _fields_ = [("RelativeFileName", w.LPCWSTR), ("Metadata", FsMetadata), ("FileIdentity", ctypes.c_void_p),
                ("FileIdentityLength", w.DWORD), ("Flags", w.ULONG), ("Result", ctypes.c_long),
                ("CreateUsn", ctypes.c_longlong)]


class ProcessInfo(ctypes.Structure):
    _fields_ = [("StructSize", w.ULONG), ("ProcessId", w.ULONG), ("ImagePath", w.LPCWSTR)]


class CallbackInfo(ctypes.Structure):
    _fields_ = [("StructSize", w.ULONG), ("ConnectionKey", ctypes.c_longlong), ("CallbackContext", ctypes.c_void_p),
                ("VolumeGuidName", w.LPCWSTR), ("VolumeDosName", w.LPCWSTR), ("VolumeSerialNumber", w.ULONG),
                ("SyncRootFileId", ctypes.c_longlong), ("SyncRootIdentity", ctypes.c_void_p),
                ("SyncRootIdentityLength", w.ULONG), ("FileId", ctypes.c_longlong), ("FileSize", ctypes.c_longlong),
                ("FileIdentity", ctypes.c_void_p), ("FileIdentityLength", w.ULONG), ("NormalizedPath", w.LPCWSTR),
                ("TransferKey", ctypes.c_longlong), ("PriorityHint", ctypes.c_ubyte),
                ("CorrelationVector", ctypes.c_void_p), ("ProcessInfo", ctypes.POINTER(ProcessInfo)),
                ("RequestKey", ctypes.c_longlong)]


class OperationInfo(ctypes.Structure):
    _fields_ = [("StructSize", w.ULONG), ("Type", w.ULONG), ("ConnectionKey", ctypes.c_longlong),
                ("TransferKey", ctypes.c_longlong), ("CorrelationVector", ctypes.c_void_p),
                ("SyncStatus", ctypes.c_void_p), ("RequestKey", ctypes.c_longlong)]


class TransferData(ctypes.Structure):
    _fields_ = [("Flags", w.ULONG), ("CompletionStatus", ctypes.c_long), ("Buffer", ctypes.c_void_p),
                ("Offset", ctypes.c_longlong), ("Length", ctypes.c_longlong)]


class OperationParameters(ctypes.Structure):
    _fields_ = [("ParamSize", w.ULONG), ("Transfer", TransferData)]


class CallbackRegistration(ctypes.Structure):
    _fields_ = [("Type", w.ULONG), ("Callback", ctypes.c_void_p)]


CALLBACK = ctypes.WINFUNCTYPE(None, ctypes.POINTER(CallbackInfo), ctypes.c_void_p)


def check(hr: int, what: str) -> None:
    if hr != 0:
        raise OSError(f"{what} failed: 0x{hr & 0xFFFFFFFF:08X}")


class Provider:
    """A connected sync provider that serves `contents` and logs every download."""

    def __init__(self, root: Path, contents: dict[str, bytes]):
        self.root, self.contents = root, contents
        self.fetches: list[dict[str, object]] = []
        self.lock = threading.Lock()
        self.identity = ctypes.create_string_buffer(b"findex-validation")
        self.callback = CALLBACK(self.on_fetch)
        self.key = ctypes.c_longlong()

    def register(self) -> None:
        reg = SyncRegistration(ctypes.sizeof(SyncRegistration), "FindexValidation", "1.0",
                               ctypes.cast(self.identity, ctypes.c_void_p), len(self.identity.raw), None, 0)
        u = uuid.uuid4()
        reg.ProviderId = GUID(u.fields[0], u.fields[1], u.fields[2], (ctypes.c_ubyte * 8)(*u.bytes[8:]))
        # Full hydration on read; ALWAYS_FULL population so listing needs no callback.
        policies = SyncPolicies(ctypes.sizeof(SyncPolicies), 2, 0, 3, 0, 0, 0, 0)
        check(cf.CfRegisterSyncRoot(str(self.root), ctypes.byref(reg), ctypes.byref(policies), 0),
              "CfRegisterSyncRoot")
        table = (CallbackRegistration * 2)()
        table[0].Type, table[0].Callback = 0, ctypes.cast(self.callback, ctypes.c_void_p)  # FETCH_DATA
        table[1].Type = 0xFFFFFFFF
        # REQUIRE_PROCESS_INFO | REQUIRE_FULL_FILE_PATH
        check(cf.CfConnectSyncRoot(str(self.root), table, None, 0x6, ctypes.byref(self.key)),
              "CfConnectSyncRoot")

    def create(self, names: list[str]) -> None:
        items = (PlaceholderCreateInfo * len(names))()
        for item, name in zip(items, names):
            item.RelativeFileName = name
            item.Metadata.Basic.Write = 133_000_000_000_000_000
            item.Metadata.Basic.Attributes = 0x80
            item.Metadata.FileSize = len(self.contents[name])
            item.FileIdentity = ctypes.cast(self.identity, ctypes.c_void_p)
            item.FileIdentityLength = len(self.identity.raw)
            item.Flags = 0x2  # MARK_IN_SYNC
        done = w.DWORD()
        check(cf.CfCreatePlaceholders(str(self.root), items, len(names), 0, ctypes.byref(done)),
              "CfCreatePlaceholders")
        assert done.value == len(names) and all(i.Result == 0 for i in items)

    def on_fetch(self, info_ptr, _params) -> None:
        info = info_ptr.contents
        name = Path(info.NormalizedPath or "").name
        process = info.ProcessInfo.contents if info.ProcessInfo else None
        with self.lock:
            self.fetches.append({"file": name,
                                 "process": Path(process.ImagePath).name if process and process.ImagePath else None})
        data = self.contents.get(name, b"")
        buffer = ctypes.create_string_buffer(data, len(data))
        op = OperationInfo(ctypes.sizeof(OperationInfo), 0, info.ConnectionKey, info.TransferKey,
                           info.CorrelationVector, None, info.RequestKey)  # TRANSFER_DATA
        param = OperationParameters(8 + ctypes.sizeof(TransferData))
        param.Transfer = TransferData(0, 0, ctypes.cast(buffer, ctypes.c_void_p), 0, len(data))
        cf.CfExecute(ctypes.byref(op), ctypes.byref(param))

    def dehydrate(self, path: Path) -> None:
        handle = kernel32.CreateFileW(str(path), 0x100, 7, None, 3, 0, None)  # FILE_WRITE_ATTRIBUTES
        assert handle and handle != w.HANDLE(-1).value, ctypes.get_last_error()
        try:
            check(cf.CfDehydratePlaceholder(w.HANDLE(handle), ctypes.c_longlong(0), ctypes.c_longlong(-1), 0, None),
                  "CfDehydratePlaceholder")
        finally:
            kernel32.CloseHandle(w.HANDLE(handle))

    def close(self) -> None:
        cf.CfDisconnectSyncRoot(self.key)
        cf.CfUnregisterSyncRoot(str(self.root))

    def downloads(self) -> list[dict[str, object]]:
        with self.lock:
            taken, self.fetches = self.fetches, []
        return taken


def cloud_only(path: Path) -> bool:
    return bool(os.stat(path, follow_symlinks=False).st_file_attributes & RECALL_ON_DATA_ACCESS)


def run(exe: Path, cwd: Path, env: dict[str, str], *args: str, stdin: str | None = None) -> str:
    done = subprocess.run([str(exe), *args], cwd=cwd, env=env, input=stdin, text=True,
                          capture_output=True, timeout=120, encoding="utf-8", errors="replace")
    assert done.returncode == 0, (args, done.stdout, done.stderr)
    return done.stdout


def mcp(exe: Path, cwd: Path, env: dict[str, str], calls: list[tuple[str, dict[str, object]]]) -> list[str]:
    lines = ['{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}',
             '{"jsonrpc":"2.0","method":"notifications/initialized"}']
    for i, (name, arguments) in enumerate(calls, 1):
        lines.append(json.dumps({"jsonrpc": "2.0", "id": i, "method": "tools/call",
                                 "params": {"name": name, "arguments": arguments}}))
    replies = [json.loads(line) for line in run(exe, cwd, env, "mcp", stdin="\n".join(lines) + "\n").splitlines()]
    return [r["result"]["content"][0]["text"] for r in replies if r.get("id")]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path)
    opts = parser.parse_args()
    exe = opts.binary.resolve(strict=True)
    checks: dict[str, dict[str, object]] = {}
    contents = {
        "cloud_a.rs": b"pub fn cloud_only_marker() {}\n",
        "cloud_b.rs": b"fn caller() { cloud_only_marker(); }\n",
        "later.rs": b"pub fn later_local_marker() {}\n",
    }
    with tempfile.TemporaryDirectory(prefix="findex-cloud-", ignore_cleanup_errors=True) as temp:
        root = Path(temp) / "project"
        root.mkdir()
        env = os.environ.copy()
        env["LOCALAPPDATA"] = str(Path(temp) / "state")
        Path(env["LOCALAPPDATA"]).mkdir()
        provider = Provider(root, contents)
        provider.register()
        try:
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            (root / "local.rs").write_text("pub fn local_marker() {}\n", encoding="utf-8")
            for number in range(12):  # Exercise queued local reads alongside placeholders.
                (root / f"local_{number}.rs").write_text(f"fn local_{number}() {{}}\n", encoding="utf-8")
            provider.create(list(contents))
            assert all(cloud_only(root / n) for n in contents)

            indexed = run(exe, root, env, "index")
            checks["index"] = {"output": indexed.strip(), "downloads": provider.downloads(),
                               "still_cloud_only": all(cloud_only(root / n) for n in contents)}
            run(exe, root, env, "search", "cloud_only_marker")
            run(exe, root, env, "context", "who calls cloud_only_marker")
            checks["cli_queries"] = {"downloads": provider.downloads()}
            texts = mcp(exe, root, env, [("findex_search", {"query": "cloud_only_marker"}),
                                         ("findex_context", {"task": "who calls cloud_only_marker"}),
                                         ("findex_find_symbol", {"name": "cloud_only_marker"}),
                                         ("findex_search", {"query": "local_marker"})])
            checks["mcp"] = {"downloads": provider.downloads(), "local_file_found": "local.rs" in texts[3]}

            (root / "later.rs").read_bytes()  # the user opens the file: one download
            opened = provider.downloads()
            found = run(exe, root, env, "search", "later_local_marker")
            checks["made_local"] = {"user_download": opened, "downloads": provider.downloads(),
                                    "found": "later.rs" in found}

            provider.dehydrate(root / "later.rs")
            assert cloud_only(root / "later.rs")
            gone = run(exe, root, env, "search", "later_local_marker")
            checks["back_to_cloud"] = {"downloads": provider.downloads(), "found": "later.rs" in gone}
        finally:
            provider.close()

    passed = (not checks["index"]["downloads"] and checks["index"]["still_cloud_only"]
              and not checks["cli_queries"]["downloads"] and not checks["mcp"]["downloads"]
              and checks["mcp"]["local_file_found"]
              and len(checks["made_local"]["user_download"]) == 1 and not checks["made_local"]["downloads"]
              and checks["made_local"]["found"]
              and not checks["back_to_cloud"]["downloads"] and not checks["back_to_cloud"]["found"])
    result = {"model_sessions": 0, "paid_calls": 0, "verdict": "PASS" if passed else "FAIL", "checks": checks}
    text = json.dumps(result, indent=2)
    if opts.output:
        opts.output.write_text(text + "\n", encoding="utf-8")
    print(text)
    raise SystemExit(0 if passed else 1)


if __name__ == "__main__":
    main()
