"""Generate the frozen symbol-navigation golden dataset.

Ground truth is derived with per-language regular expressions written for
this harness, deliberately *independent* of Findex's Tree-sitter extractor,
and kept only when a name has at most two definitions in the whole corpus
(both are then relevant answers).
The dataset is split deterministically into `dev` (tuning allowed) and
`heldout` (never used for tuning).
"""
from __future__ import annotations

import hashlib
import json
import random
import re
import sys
from collections import defaultdict
from pathlib import Path

from common import CORPORA, VALIDATION, git

SEED = 20261011
PER_CORPUS = 40
GOLDEN_CORPORA = ["findex", "ripgrep", "requests", "zod", "fmt", "pybind11", "tokio"]
EXCLUDED_DIRS = {".git", ".findex", "node_modules", "target", "vendor", "dist", "build", ".venv", "__pycache__"}
MAX_BYTES = 512 * 1024

KEYWORDS = {"if", "for", "while", "switch", "return", "else", "do", "catch", "sizeof", "decltype", "operator"}
PATTERNS = {
    ".rs": [re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:default\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?"
                       r"(?:extern\s+\"[^\"]*\"\s+)?fn\s+([A-Za-z_]\w*)"),
            re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum|trait|union|type)\s+([A-Za-z_]\w*)")],
    ".py": [re.compile(r"^\s*(?:async\s+)?def\s+([A-Za-z_]\w*)"), re.compile(r"^\s*class\s+([A-Za-z_]\w*)")],
    ".ts": [re.compile(r"^\s*(?:export\s+)?(?:default\s+)?(?:declare\s+)?(?:async\s+)?function\*?\s+([A-Za-z_$][\w$]*)"),
            re.compile(r"^\s*(?:export\s+)?(?:default\s+)?(?:declare\s+)?(?:abstract\s+)?class\s+([A-Za-z_$][\w$]*)"),
            re.compile(r"^\s*(?:export\s+)?(?:declare\s+)?(?:interface|enum|type)\s+([A-Za-z_$][\w$]*)\b"),
            re.compile(r"^\s*(?:export\s+)?const\s+([A-Za-z_$][\w$]*)\s*=\s*(?:async\s*)?(?:\([^)]*\)|[A-Za-z_$][\w$]*)\s*=>")],
    ".cpp": [re.compile(r"^\s*(?:template\s*<[^>]*>\s*)?(?:class|struct)\s+(?:FMT_API\s+|PYBIND11_EXPORT\s+)?([A-Za-z_]\w*)\s*(?:final\s*)?(?::[^;{]*)?\{"),
             re.compile(r"^(?:[A-Za-z_][\w:<>,\*&~\s]*?\s+[\*&]*)?(?:[A-Za-z_]\w*::)*([A-Za-z_]\w*)\s*\([^;{}]*\)\s*(?:const\s*)?(?:noexcept\s*)?(?:override\s*)?\{\s*$")],
}
EXTENSIONS = {".rs": ".rs", ".py": ".py", ".pyi": ".py", ".ts": ".ts", ".tsx": ".ts", ".js": ".ts", ".mjs": ".ts",
              ".cc": ".cpp", ".cpp": ".cpp", ".cxx": ".cpp", ".h": ".cpp", ".hpp": ".cpp", ".hh": ".cpp"}
SENSITIVE = re.compile(r"(^\.env($|\.)|^id_rsa$|^id_ed25519$|^credentials\.json$|^secrets\.json$|\.(pem|p12|pfx|key)$)", re.I)


def admitted_files(root: Path) -> list[str]:
    """Approximate Findex admission: tracked, not hidden/excluded, text, <=512 KiB."""
    files = []
    for rel in git("ls-files", cwd=root).splitlines():
        parts = rel.split("/")
        if any(p.lower() in EXCLUDED_DIRS or p.startswith(".") for p in parts) or SENSITIVE.search(parts[-1]):
            continue
        path = root / rel
        try:
            data = path.read_bytes()
        except OSError:
            continue
        if len(data) > MAX_BYTES or b"\0" in data:
            continue
        try:
            data.decode("utf-8")
        except UnicodeDecodeError:
            continue
        files.append(rel)
    return files


def split_words(name: str) -> list[str]:
    spaced = re.sub(r"([a-z0-9])([A-Z])", r"\1 \2", name)
    spaced = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1 \2", spaced)
    return [w.lower() for w in re.split(r"[_\s$]+", spaced) if w]


def definitions(root: Path, files: list[str]) -> dict[str, list[tuple[str, int]]]:
    found: dict[str, list[tuple[str, int]]] = defaultdict(list)
    for rel in files:
        family = EXTENSIONS.get(Path(rel).suffix.lower())
        if not family:
            continue
        text = (root / rel).read_text(encoding="utf-8")
        for number, line in enumerate(text.splitlines(), 1):
            for pattern in PATTERNS[family]:
                match = pattern.match(line)
                if match and match.group(1) not in KEYWORDS:
                    found[match.group(1)].append((rel, number))
                    break
    return found


def split_for(corpus: str, name: str) -> str:
    bucket = int(hashlib.sha256(f"{corpus}:{name}".encode()).hexdigest(), 16) % 10
    return "heldout" if bucket < 3 else "dev"


WORD = re.compile(r"[A-Za-z]{5,}")
STOP = {"return", "const", "while", "function", "public", "private", "static", "string", "class", "struct",
        "import", "export", "default", "unsigned", "template", "typename", "include", "define", "false",
        "assert", "where", "should", "would", "which", "there", "their", "these", "other", "value", "error"}


def line_terms(corpus: str, root: Path, files: list[str], rng: random.Random, wanted: int) -> list[dict]:
    """Non-definition queries: three rare words from one random source line.

    Kept only when that word set co-occurs on a single line of exactly one
    file, so the expected location is unambiguous without human labelling.
    """
    texts = {rel: (root / rel).read_text(encoding="utf-8").splitlines() for rel in files
             if Path(rel).suffix.lower() in EXTENSIONS}
    doc_freq: dict[str, int] = defaultdict(int)
    for lines in texts.values():
        for word in {w.lower() for line in lines for w in WORD.findall(line)}:
            doc_freq[word] += 1
    rows, attempts = [], 0
    ordered = sorted(texts)
    while len(rows) < wanted and attempts < wanted * 200 and ordered:
        attempts += 1
        rel = rng.choice(ordered)
        lines = texts[rel]
        if not lines:
            continue
        number = rng.randrange(len(lines))
        words = list(dict.fromkeys(w.lower() for w in WORD.findall(lines[number]) if w.lower() not in STOP))
        words = [w for w in words if doc_freq[w] >= 2]  # each word alone is ambiguous
        if len(words) < 3:
            continue
        chosen = sorted(rng.sample(words, 3))
        matches = [(p, i + 1) for p, ls in texts.items() for i, line in enumerate(ls)
                   if all(w in line.lower() for w in chosen)]
        if len(matches) != 1 or any(r["query"] == " ".join(chosen) for r in rows):
            continue
        rows.append({"corpus": corpus, "name": None, "expected": [list(matches[0])], "query": " ".join(chosen),
                     "id": f"{corpus}:line:{len(rows)}", "type": "line_terms",
                     "split": split_for(corpus, " ".join(chosen)), "commit": git("rev-parse", "HEAD", cwd=root)})
    return rows


def main() -> int:
    out = VALIDATION / "golden" / "symbol_navigation.jsonl"
    rng = random.Random(SEED)
    rows = []
    for corpus in GOLDEN_CORPORA:
        root = CORPORA / corpus
        files = admitted_files(root)
        defs = definitions(root, files)
        by_lower = defaultdict(int)
        for name, places in defs.items():
            by_lower[name.lower()] += len(places)
        candidates = sorted(
            name for name, places in defs.items()
            if len(places) <= 2 and by_lower[name.lower()] == len(places) and len(name) >= 4
            and not name.lower().startswith("test") and not name.isupper())
        chosen = rng.sample(candidates, min(PER_CORPUS, len(candidates)))
        for name in sorted(chosen):
            # At most two definitions (e.g. mirrored source trees); either is correct.
            base = {"corpus": corpus, "name": name, "expected": [list(place) for place in defs[name]],
                    "split": split_for(corpus, name), "commit": git("rev-parse", "HEAD", cwd=root)}
            rows.append({**base, "id": f"{corpus}:{name}:exact", "type": "symbol_exact", "query": name})
            words = split_words(name)
            if len(words) >= 2:
                rows.append({**base, "id": f"{corpus}:{name}:words", "type": "identifier_words",
                             "query": " ".join(words)})
        concept = line_terms(corpus, root, files, rng, 20)
        rows.extend(concept)
        print(f"{corpus}: files={len(files)} unique_defs={len(candidates)} sampled={len(chosen)} "
              f"line_terms={len(concept)}", file=sys.stderr)
    out.write_text("".join(json.dumps(r) + "\n" for r in rows), encoding="utf-8")
    print(f"wrote {len(rows)} queries to {out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
