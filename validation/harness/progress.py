"""Render docs/PROGRESS.md and its SVG charts from validation/rounds.json.

rounds.json holds only pointers to frozen result files; every number below is read
from those files, so the page cannot drift from the evidence. `--check` fails when
the committed page or charts are stale.
"""
from __future__ import annotations

import argparse
import json
from html import escape
from pathlib import Path

from common import REPO

ROUNDS = REPO / "validation/rounds.json"
OUT_MD = REPO / "docs/PROGRESS.md"
OUT_SVG = REPO / "docs/assets"
NAMES = {"findex": "Findex", "graphify": "Graphify", "cbm": "codebase-memory", "gitgrep": "git grep"}
DASH = {"findex": "", "graphify": "6 3", "cbm": "2 3", "gitgrep": "9 3 2 3"}
TOLERANCE = 0.02  # relative; closer than this is reported as a tie


def read(path: str) -> dict:
    return json.loads((REPO / path).read_text(encoding="utf-8"))


def definition(path: str) -> dict:
    scopes = read(path)["summary"]
    out: dict = {}
    for src, tool in (("findex-compact", "findex"), ("codebase-tree", "cbm"), ("graphify-mcp", "graphify")):
        held, everything = scopes["new-55"][src], scopes["all-115"][src]
        out[tool] = {"hit1": held["hit_at_1"], "mrr": held["mrr"], "tokens": held["mean_raw_tokens"],
                     "ms": held["latency_s"]["p50"] * 1000, "tokens_all": everything["mean_raw_tokens"]}
    return out


def localization(path: str) -> dict:
    systems = dict(read(path)["systems"])
    rivals = (REPO / path).with_name("localization-rivals.json")  # measured offline on the same tasks
    if rivals.exists():
        systems.update(json.loads(rivals.read_text(encoding="utf-8"))["systems"])
    out: dict = {}
    for src, row in systems.items():
        tool = {"gitgrep-words": "gitgrep", "findex-context": "findex"}.get(src) or (
            "graphify" if src.startswith("graphify") else "cbm" if src.startswith("codebase") else None)
        if tool:
            a = row["all"]
            out[tool] = {"hit1": a["hit@1"], "recall10": a["gold_recall@10"], "mrr": a["mrr"],
                         "bytes": a["output_bytes"]["mean"]}
    return out


def live(path: str) -> dict:
    summary = read(path)["summary"]
    out: dict = {}
    for src, tool in (("findex", "findex"), ("graphify", "graphify"), ("codebase-memory", "cbm")):
        c = summary[src]
        if c["sessions"]:
            out[tool] = {"success": c["correct"] / c["sessions"], "valid": c["valid"] / c["sessions"],
                         "tokens": c["total_tokens"] / c["sessions"], "sessions": c["sessions"]}
    return out


# panel = (metric key, title, direction (+1 higher is better), number format, target)
LANES = {
    "definition": {"title": "Definition lookup", "extract": definition,
                   "sub": "55 held-out queries; Findex compact vs codebase-memory tree vs Graphify MCP",
                   "panels": [("hit1", "Right file, first try (hit@1)", 1, ".3f", None),
                              ("tokens", "Tokens per query", -1, ",.0f", None),
                              ("ms", "Median latency (ms)", -1, ".1f", None)],
                   "extra": [("tokens_all", "Tokens per query, all 115 queries", -1, ",.0f", None)]},
    "localization": {"title": "Issue localization", "extract": localization,
                     "sub": "65 reused historical issue titles at a 1,200-token rendered budget",
                     "panels": [("hit1", "Gold file ranked first (hit@1)", 1, ".3f", 0.60),
                                ("recall10", "Gold recall in top 10", 1, ".3f", 0.90),
                                ("mrr", "Mean reciprocal rank", 1, ".3f", None)]},
    "localization_fresh": {"title": "Issue localization, fresh holdout", "extract": localization,
                           "sub": "55 issue titles frozen before any system was scored; 8,000-token rendered budget",
                           "panels": [("hit1", "Gold file ranked first (hit@1)", 1, ".3f", 0.60),
                                      ("recall10", "Gold recall in top 10", 1, ".3f", 0.90),
                                      ("mrr", "Mean reciprocal rank", 1, ".3f", None)]},
}
for _lane, _title, _sub in (
        ("navigation_claude", "Live navigation, Claude Code",
         "Claude Haiku via subscription; tokens include cache reads; success counts invalid sessions as failures"),
        ("localization_claude", "Live issue localization, Claude Code",
         "Claude Haiku via subscription names the files to change from an issue title; hit@1 counts invalid sessions as failures"),
        ("navigation_codex", "Live navigation, Codex",
         "Codex via ChatGPT subscription; success counts invalid sessions as failures"),
        ("bugfix_codex", "Live bugfix, Codex",
         "Real reused bugs graded by independent Windows regression tests; success counts invalid sessions as failures")):
    LANES[_lane] = {"title": _title, "extract": live, "sub": _sub,
                    "panels": [("success", "Tasks solved", 1, ".2f", None),
                               ("tokens", "Total tokens per session", -1, ",.0f", None)]}


def collect() -> list[dict]:
    rounds = read("validation/rounds.json")["rounds"]
    for r in rounds:
        r["data"] = {}
        for lane, spec in LANES.items():
            if lane in r.get("lanes", {}):
                r["data"][lane] = spec["extract"](r["lanes"][lane])
    return rounds


def number(value: float, fmt: str) -> str:
    return format(value, fmt)


def verdict(findex: float, rivals: dict[str, float], direction: int) -> str:
    if not rivals:
        return "no rival measured"
    best = max(rivals.items(), key=lambda kv: kv[1] * direction)
    gap = (findex - best[1]) * direction
    if abs(findex - best[1]) <= TOLERANCE * max(abs(best[1]), 1e-9):
        return f"ties {NAMES[best[0]]}"
    return f"leads {NAMES[best[0]]}" if gap > 0 else f"TRAILS {NAMES[best[0]]}"


def series_for(rounds: list[dict], lane: str, key: str) -> tuple[list[str], dict]:
    ids = [r["id"] for r in rounds if lane in r["data"]]
    tools = sorted({t for r in rounds if lane in r["data"] for t in r["data"][lane]}, key=list(NAMES).index)
    return ids, {t: [next((r["data"][lane][t].get(key) for r in rounds if r["id"] == i and t in r["data"][lane]), None)
                     for i in ids] for t in tools}


STYLE = """<style>
svg{--bg:#fcfbf8;--ink:#1f2328;--muted:#5d5c58;--grid:#e6e3dc;--findex:#2a78d6;--graphify:#6b6a65;--cbm:#a19f97;--gitgrep:#898781;--target:#c2410c}
@media (prefers-color-scheme: dark){svg{--bg:#14181f;--ink:#e6e8eb;--muted:#a3a19a;--grid:#2b313a;--findex:#3987e5;--graphify:#a3a19a;--cbm:#6e6c66;--gitgrep:#898781;--target:#fb923c}}
text{font:12px system-ui,-apple-system,Segoe UI,sans-serif;fill:var(--muted)}
.t{font-size:13px;font-weight:600;fill:var(--ink)}.h{font-size:15px;font-weight:700;fill:var(--ink)}
.g{stroke:var(--grid);stroke-width:1}
</style>"""


def panel_svg(x0: float, rounds_ids: list[str], series: dict, spec: tuple, y0: float = 58) -> str:
    key, title, direction, fmt, target = spec
    pw, ph, left, right, top, bottom = 330, 200, 44, 96, 40, 28
    px, py, plot_w, plot_h = x0 + left, y0 + top, pw - left - right, ph - top - bottom
    values = [v for s in series.values() for v in s if v is not None] + ([target] if target else [])
    hi = 1.0 if fmt.startswith(".") and max(values, default=0) <= 1.0 else max(values, default=1) * 1.12
    xs = [px + (plot_w * (i + .5) / len(rounds_ids)) for i in range(len(rounds_ids))]
    y = lambda v: py + plot_h * (1 - v / hi)
    out = [f'<text class="t" x="{x0 + 6}" y="{y0 + 14}">{escape(title)}</text>',
           f'<text x="{x0 + 6}" y="{y0 + 28}">{"higher" if direction > 0 else "lower"} is better</text>']
    for k in range(5):
        v = hi * k / 4
        out.append(f'<line class="g" x1="{px}" x2="{px + plot_w}" y1="{y(v):.1f}" y2="{y(v):.1f}"/>'
                   f'<text x="{px - 6}" y="{y(v) + 4:.1f}" text-anchor="end">{number(v, fmt if v else "g")}</text>')
    for x, rid in zip(xs, rounds_ids):
        out.append(f'<text x="{x:.1f}" y="{py + plot_h + 18}" text-anchor="middle">{escape(rid)}</text>')
    ends = []
    if target:
        out.append(f'<line x1="{px}" x2="{px + plot_w}" y1="{y(target):.1f}" y2="{y(target):.1f}" '
                   f'stroke="var(--target)" stroke-width="1.5" stroke-dasharray="4 4"/>')
        ends.append([y(target), "target", f"target {number(target, fmt)}"])
    for tool, vals in series.items():
        pts = [(x, y(v), v) for x, v in zip(xs, vals) if v is not None]
        if not pts:
            continue
        color = f"var(--{tool})"
        width = 2.5 if tool == "findex" else 1.75
        if len(pts) > 1:
            out.append(f'<polyline fill="none" stroke="{color}" stroke-width="{width}" stroke-dasharray="{DASH[tool]}" '
                       f'points="{" ".join(f"{a:.1f},{b:.1f}" for a, b, _ in pts)}"/>')
        for a, b, v in pts:
            out.append(f'<circle cx="{a:.1f}" cy="{b:.1f}" r="4.5" fill="{color}" stroke="var(--bg)" stroke-width="2">'
                       f'<title>{NAMES[tool]}: {number(v, fmt)}</title></circle>')
        ends.append([pts[-1][1], tool, f"{'cbm' if tool == 'cbm' else NAMES[tool]} {number(pts[-1][2], fmt)}"])
    ends.sort()
    for i in range(1, len(ends)):  # keep direct labels from overlapping
        ends[i][0] = max(ends[i][0], ends[i - 1][0] + 13)
    for ly, tool, label in ends:
        out.append(f'<text x="{px + plot_w + 8}" y="{ly + 4:.1f}" style="fill:var(--{tool});font-weight:{600 if tool == "findex" else 400}">'
                   f'{escape(label)}</text>')
    return "".join(out)


def lane_svg(lane: str, rounds: list[dict]) -> str:
    spec = LANES[lane]
    ids = [r["id"] for r in rounds if lane in r["data"]]
    width = 16 + 340 * len(spec["panels"])
    tools = sorted({t for r in rounds if lane in r["data"] for t in r["data"][lane]}, key=list(NAMES).index)
    legend, lx = [], 22
    for t in tools:
        legend.append(f'<line x1="{lx}" x2="{lx + 26}" y1="44" y2="44" stroke="var(--{t})" stroke-width="2.5" '
                      f'stroke-dasharray="{DASH[t]}"/><text x="{lx + 32}" y="48">{NAMES[t]}{" (cbm)" if t == "cbm" else ""}</text>')
        lx += 48 + 7 * len(NAMES[t]) + (6 * 6 if t == "cbm" else 0)
    body = "".join(panel_svg(8 + 340 * i, ids, series_for(rounds, lane, p[0])[1], p) for i, p in enumerate(spec["panels"]))
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} 270" width="100%" role="img" '
            f'aria-label="{escape(spec["title"])} across measurement rounds">{STYLE}'
            f'<rect width="{width}" height="270" rx="10" fill="var(--bg)"/>'
            f'<text class="h" x="22" y="26">{escape(spec["title"])}</text>'
            f'<text x="{22 + 9 * len(spec["title"])}" y="26">{escape(spec["sub"])}</text>{"".join(legend)}{body}</svg>\n')


def delta(rounds: list[dict], lane: str, key: str, fmt: str, direction: int) -> str:
    seen = [r["data"][lane]["findex"][key] for r in rounds if lane in r["data"] and "findex" in r["data"][lane]]
    if len(seen) < 2:
        return "first measurement"
    change = seen[-1] - seen[-2]
    if abs(change) < 1e-9:
        return "no change"
    return f'{"▲" if change * direction > 0 else "▼"} {change:+{fmt}} vs previous round'


def markdown(rounds: list[dict]) -> str:
    lines = ["# Progress by measurement round", "",
             "Generated by `python validation/harness/progress.py` from `validation/rounds.json`; every number is read from "
             "the frozen result file linked in its round. A lane appears in a round only when it was measured then. "
             "Lines connect measured rounds only. Small live samples are noisy and are not evidence of general superiority.", "",
             "## Scoreboard (latest measurement per lane)", "",
             "| Lane | Metric | Findex | Best rival | Verdict | Change |", "|---|---|---:|---:|---|---|"]
    for lane, spec in LANES.items():
        measured = [r for r in rounds if lane in r["data"] and "findex" in r["data"][lane]]
        if not measured:
            continue
        latest = measured[-1]
        for key, title, direction, fmt, target in spec["panels"] + spec.get("extra", []):
            if key not in latest["data"][lane]["findex"]:
                continue
            findex = latest["data"][lane]["findex"][key]
            rivals = {t: v[key] for t, v in latest["data"][lane].items() if t != "findex" and key in v}
            best = max(rivals.items(), key=lambda kv: kv[1] * direction) if rivals else None
            note = verdict(findex, rivals, direction)
            if target is not None:
                note += f"; target {number(target, fmt)} {'met' if findex >= target else 'NOT met'}"
            lines.append(f"| {spec['title']} ({latest['id']}) | {title} | **{number(findex, fmt)}** | "
                         f"{number(best[1], fmt) + ' (' + NAMES[best[0]] + ')' if best else '—'} | {note} | "
                         f"{delta(rounds, lane, key, fmt, direction)} |")
    for lane, spec in LANES.items():
        measured = [r for r in rounds if lane in r["data"]]
        if not measured:
            continue
        lines += ["", f"## {spec['title']}", "", spec["sub"] + ".", "",
                  f"![{spec['title']} across rounds](assets/progress-{lane}.svg)", ""]
        keys = [p for p in spec["panels"]]
        tools = sorted({t for r in measured for t in r["data"][lane]}, key=list(NAMES).index)
        lines += ["| Round | Date | " + " | ".join(f"{NAMES[t]} {k[0]}" for t in tools for k in keys[:2]) + " | Evidence |",
                  "|---|---|" + "---:|" * (len(tools) * 2) + "---|"]
        for r in measured:
            cells = []
            for t in tools:
                for k in keys[:2]:
                    v = r["data"][lane].get(t, {}).get(k[0])
                    cells.append("—" if v is None else number(v, k[3]))
            lines.append(f"| {r['id']} | {r['date']} | " + " | ".join(cells) + f" | [`{Path(r['lanes'][lane]).parent.name}`](../{r['lanes'][lane]}) |")
    lines += ["", "## What each round changed", ""]
    lines += [f"- **{r['id']}** ({r['date']}): {r['note']}" for r in rounds]
    return "\n".join(lines) + "\n"


def render() -> dict[Path, str]:
    rounds = collect()
    files = {OUT_MD: markdown(rounds)}
    for lane in LANES:
        if any(lane in r["data"] for r in rounds):
            files[OUT_SVG / f"progress-{lane}.svg"] = lane_svg(lane, rounds)
    return files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if committed outputs are stale")
    args = parser.parse_args()
    files = render()
    stale = [p for p, text in files.items() if not p.exists() or p.read_text(encoding="utf-8") != text]
    if args.check:
        print("stale: " + ", ".join(str(p.relative_to(REPO)) for p in stale) if stale else "progress outputs current")
        return 1 if stale else 0
    for path, text in files.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="\n")
    print("wrote " + ", ".join(str(p.relative_to(REPO)) for p in files))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
