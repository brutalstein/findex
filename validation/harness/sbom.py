"""CycloneDX 1.5 SBOM for the Windows `findex` binary, from `cargo metadata`
and Cargo.lock checksums. Only crates reachable from the `findex` package on
x86_64-pc-windows-msvc (normal + build dependencies) are listed.
Usage: sbom.py <output.cdx.json>
"""
from __future__ import annotations

import datetime
import json
import subprocess
import sys
import tomllib
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TARGET = "x86_64-pc-windows-msvc"


def main() -> int:
    out = Path(sys.argv[1])
    meta = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET],
        cwd=REPO, capture_output=True, text=True, check=True).stdout)
    lock = tomllib.loads((REPO / "Cargo.lock").read_text(encoding="utf-8"))
    checksums = {(p["name"], p["version"]): p.get("checksum") for p in lock["package"]}
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = next(p["id"] for p in meta["packages"] if p["name"] == "findex" and p["source"] is None)
    seen, stack = set(), [root]
    while stack:  # dev-dependencies are not shipped
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        for dep in nodes[current]["deps"]:
            if any(kind["kind"] in (None, "build") for kind in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    ref = lambda p: f"pkg:cargo/{p['name']}@{p['version']}"  # noqa: E731
    components = []
    for pid in sorted(seen - {root}, key=lambda i: (packages[i]["name"], packages[i]["version"])):
        p = packages[pid]
        item = {"type": "library", "bom-ref": ref(p), "name": p["name"], "version": p["version"], "purl": ref(p)}
        if p.get("license"):
            item["licenses"] = [{"expression": p["license"]}]
        if p["source"] is None:
            item["description"] = "workspace crate"
        elif checksums.get((p["name"], p["version"])):
            item["hashes"] = [{"alg": "SHA-256", "content": checksums[(p["name"], p["version"])]}]
        components.append(item)
    app = packages[root]
    bom = {
        "bomFormat": "CycloneDX", "specVersion": "1.5", "serialNumber": f"urn:uuid:{uuid.uuid4()}", "version": 1,
        "metadata": {"timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
                     "tools": {"components": [{"type": "application", "name": "findex validation/harness/sbom.py"}]},
                     "component": {"type": "application", "bom-ref": ref(app), "name": "findex", "version": app["version"],
                                   "purl": ref(app), "licenses": [{"expression": app["license"]}] if app.get("license") else []},
                     "properties": [{"name": "cargo:target", "value": TARGET}]},
        "components": components,
        "dependencies": [{"ref": ref(packages[i]),
                          "dependsOn": sorted({ref(packages[d["pkg"]]) for d in nodes[i]["deps"] if d["pkg"] in seen})}
                         for i in sorted(seen, key=lambda i: packages[i]["name"])],
    }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(bom, indent=2) + "\n", encoding="utf-8")
    unlicensed = [c["name"] for c in components if "licenses" not in c]
    print(f"{len(components)} components; without license metadata: {unlicensed or 'none'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
