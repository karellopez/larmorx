# SPDX-License-Identifier: Apache-2.0
"""The morning report of a night's random tests (``report.md``) and its numbers
(``summary.json``, which the documentation site's exporter reads), from ``results.jsonl`` and
``findings/``.

Per tool: the cases run, their verdicts (agree; explained by glibc's maths; differ; timed out;
skipped; the harness's own errors), the options and input features covered, the time; then
every difference, grouped by its signature (the checks that failed), with the number of cases
that showed it, the smallest reproducer found and how to replay it; and what needs a decision.
"""

from __future__ import annotations

import json
from collections import Counter, defaultdict
from pathlib import Path

from larmorx_validation.difftest import core

VERDICT_ORDER = (core.AGREE, core.EXPLAINED, core.NOTICE, "undefined", core.DIFFER, "timeout", "crash", core.SKIP, "harness-error", "draw-error")


def load(out: Path) -> tuple[dict, list[dict]]:
    info = json.loads((out / "run.json").read_text(encoding="utf-8")) if (out / "run.json").is_file() else {}
    lines = []
    path = out / "results.jsonl"
    if path.is_file():
        for ln in path.read_text(encoding="utf-8").splitlines():
            if ln.strip():
                lines.append(json.loads(ln))
    return info, lines


def findings(out: Path) -> list[dict]:
    found = []
    for p in sorted((out / "findings").glob("*/*.json")):
        f = json.loads(p.read_text(encoding="utf-8"))
        f["_file"] = str(p.relative_to(out))
        found.append(f)
    return found


def summarise(out: Path) -> dict:
    info, lines = load(out)
    per: dict[str, dict] = {}
    for line in lines:
        t = per.setdefault(line["tool"], {"cases": 0, "verdicts": Counter(), "impl_verdicts": defaultdict(Counter), "options": Counter(), "features": Counter(), "seconds": 0.0})
        t["cases"] += 1
        t["verdicts"][line.get("verdict", "?")] += 1
        for impl, e in (line.get("impls") or {}).items():
            t["impl_verdicts"][impl][e.get("verdict", "?")] += 1
        for o in line.get("options", []):
            t["options"][o] += 1
        for k, v in (line.get("features") or {}).items():
            if isinstance(v, str | bool) or k in ("dimension", "dtype", "qform_code", "sform_code", "slice_code"):
                t["features"][f"{k}={v}"] += 1
        t["seconds"] += float(line.get("seconds") or 0.0)
    groups: dict[tuple, list[dict]] = defaultdict(list)
    for f in findings(out):
        groups[(f["tool"], f["implementation"], tuple(f["signature"]))].append(f)
    diffs = []
    for (tool, impl, sig), fs in sorted(groups.items()):
        best = min(fs, key=lambda f: (_size(f["shrunk"]), f["n"]))
        diffs.append(
            {
                "tool": tool,
                "implementation": impl,
                "signature": list(sig),
                "cases": len(fs),
                "example": best["_file"],
                "shrunk_args": " ".join(core.arguments(best["shrunk"]))[:400],
                "shrunk_inputs": {k: _describe_input(v) for k, v in best["shrunk"]["inputs"].items()},
                "detail": (best.get("shrunk_detail") or best.get("detail") or "")[:600],
                "moves": best.get("shrink_moves", []),
                "replay": f"python -m larmorx_validation difftest --replay {best['_file']}",
                "case": f"{tool}:{best['seed']}:{best['n']}",
            }
        )
    notices: dict[tuple[str, str], list[str]] = defaultdict(list)
    for line in lines:
        for impl, e in (line.get("impls") or {}).items():
            if e.get("verdict") == core.NOTICE:
                for sig in e.get("signature", []):
                    for entry in sig.removeprefix("notice: ").split(", "):
                        notices[(line["tool"], entry)].append(f"{line['tool']}:{line.get('seed')}:{line.get('n')}")
    tools = {}
    for name, t in sorted(per.items()):
        tools[name] = {
            "cases": t["cases"],
            "verdicts": {k: t["verdicts"][k] for k in VERDICT_ORDER if t["verdicts"][k]},
            "implementations": {i: {k: c[k] for k in VERDICT_ORDER if c[k]} for i, c in t["impl_verdicts"].items()},
            "options_covered": len(t["options"]),
            "options": dict(sorted(t["options"].items())),
            "features_covered": len(t["features"]),
            "seconds": round(t["seconds"], 1),
            "differences": [d for d in diffs if d["tool"] == name],
            "notices": {entry: {"cases": len(cs), "example": cs[0]} for (tool, entry), cs in sorted(notices.items()) if tool == name},
        }
    unclear = [
        {"tool": ln["tool"], "case": f"{ln['tool']}:{ln.get('seed')}:{ln.get('n')}", "verdict": ln.get("verdict"), "detail": str(ln.get("detail", ""))[:300]}
        for ln in lines
        if ln.get("verdict") in ("timeout", "crash", "harness-error", "draw-error")
    ]
    return {
        "schema": 1,
        "date": info.get("started", "")[:10],
        "seed": info.get("seed"),
        "budget_s": info.get("budget_s"),
        "elapsed_s": info.get("elapsed_s"),
        "jobs": info.get("jobs"),
        "threads": info.get("threads"),
        "larmorx_commit": info.get("environment", {}).get("larmorx_commit"),
        "unavailable": info.get("unavailable", {}),
        "cases": sum(t["cases"] for t in tools.values()),
        "tools": tools,
        "unclear": unclear,
    }


def _size(recipe: dict) -> tuple:
    vox = 0
    for spec in recipe.get("inputs", {}).values():
        shape = spec.get("shape") or [1]
        n = 1
        for s in shape:
            n *= int(s)
        vox += n
    return (len(core.arguments(recipe)), vox)


def _describe_input(spec: dict) -> str:
    kind = spec.get("kind", "nifti")
    if kind != "nifti":
        return kind
    g = spec.get("geometry", {})
    parts = [
        "x".join(map(str, spec.get("shape", []))),
        spec.get("dtype", ""),
        spec.get("content", {}).get("type", ""),
        f"q{g.get('qform_code')}/s{g.get('sform_code')}",
    ]
    if g.get("quaternion") not in (None, [1, 0, 0, 0]):
        parts.append(f"quaternion {g['quaternion']}")
    if spec.get("scaling"):
        parts.append(f"slope {spec['scaling']['slope']} inter {spec['scaling']['inter']}")
    if spec.get("values", {}).get("specials"):
        parts.append("specials " + ",".join(spec["values"]["specials"]))
    return " ".join(p for p in parts if p)


def write(out: Path) -> Path:
    s = summarise(out)
    (out / "summary.json").write_text(json.dumps(s, indent=1, sort_keys=True), encoding="utf-8")
    lines = [
        f"# Differential random testing: {s['date']}",
        "",
        f"Seed `{s['seed']}`, budget {round((s['budget_s'] or 0) / 3600, 2)} h (ran {round((s['elapsed_s'] or 0) / 3600, 2)} h), "
        f"{s['jobs']} jobs at {s['threads']} thread(s), larmorx `{s['larmorx_commit']}`; {s['cases']} cases.",
        "",
        "Verdicts: *agree* (the parity runner passed, bit-identical where the tool must be), *explained* "
        "(differs from the original as built only through glibc's maths: identical with CORE-MATH preloaded), "
        "*notice* (a D13 notice where the correction changed nothing visible), *undefined* (the original's "
        "values depend on memory it never wrote or on the clock, a defect larmorx names: not comparable), *differ* (a finding: shrunk "
        "below), *timeout* and *crash* (unclear: listed at the end).",
        "",
        "| Tool | Cases | Agree | Explained | Notice | Undefined | Differ | Timeout | Crash | Skipped | Harness | Options covered | Time (s) |",
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|",
    ]
    for name, t in s["tools"].items():
        v = t["verdicts"]
        lines.append(
            f"| {name} | {t['cases']} | {v.get('agree', 0)} | {v.get('explained', 0)} | {v.get('notice', 0)} | {v.get('undefined', 0)} | {v.get('differ', 0)} | "
            f"{v.get('timeout', 0)} | {v.get('crash', 0)} | {v.get('skipped', 0)} | {v.get('harness-error', 0) + v.get('draw-error', 0)} | "
            f"{t['options_covered']} | {t['seconds']:.0f} |"
        )
    if s["unavailable"]:
        lines += ["", "Not run (an oracle missing): " + "; ".join(f"{k}: {v}" for k, v in s["unavailable"].items())]
    lines += ["", "## Differences", ""]
    any_diff = False
    for name, t in s["tools"].items():
        for d in t["differences"]:
            any_diff = True
            lines += [
                f"### {name} ({d['implementation']}): {', '.join(d['signature'])}",
                "",
                f"- {d['cases']} case(s); smallest reproducer `{d['case']}` ({d['example']}), after "
                f"{len(d['moves'])} shrinking moves",
                f"- arguments: `{d['shrunk_args']}`",
                "- inputs: " + "; ".join(f"{k}: {v}" for k, v in d["shrunk_inputs"].items()),
                f"- what differs: {d['detail']}",
                f"- replay: `{d['replay']}`",
                "",
            ]
    if not any_diff:
        lines.append("None.")
    lines += [
        "",
        "## D13 notices without a visible change",
        "",
        "A correction was named where it changed nothing this input shows (D13: a notice fires exactly "
        "where the result differs). Not a mistake in a result; each to be made precise or declared.",
        "",
    ]
    any_notice = False
    for name, t in s["tools"].items():
        for entry, n in t["notices"].items():
            any_notice = True
            lines.append(f"- {name}: {entry} in {n['cases']} case(s), e.g. `{n['example']}`")
    if not any_notice:
        lines.append("None.")
    if s["unclear"]:
        lines += ["", "## Unclear (for the orchestrator)", ""]
        for u in s["unclear"][:200]:
            lines.append(f"- `{u['case']}`: {u['verdict']}: {u['detail']}")
    path = out / "report.md"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


__all__ = ["findings", "load", "summarise", "write"]
