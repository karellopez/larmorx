# SPDX-License-Identifier: Apache-2.0
"""Re-checks a random-testing trial's findings of ``fsl-topup-original`` with the current build: a
sample of each group (the kind of case, whether the movements are estimated, quadratic splines,
the first failed exact check), each replayed with ``difftest --replay`` (FSL's runs from the
oracle cache, ``use`` mode; a replay's new band runs stored), at most ``--jobs`` at a time, each
replay in its own memory leaf of ``--memory-gib``. Run it inside a memory box
(``scripts/orchestration/boxed.sh``). Prints each group's verdicts and writes every replay's
output and the summary to ``--out``.

    python -P recheck.py <trial dir> --per-group 40 --jobs 8 --out <dir>
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures
import json
import os
import random
import subprocess
import sys
from pathlib import Path

EXACT = ("Exit status", "Files written", "Standard error", "Standard output", "header",
         "identical", "the same levels", "the default run", "D13 fix pass", "the same numbers")  # fmt: skip


def group_of(doc: dict) -> str:
    """The finding's group: the kind, the movements, the splines' order, its first exact
    failure (or "metrics")."""
    r = doc["recipe"]
    f, o = r["features"], r["options"]
    kind = f.get("kind", "?")
    parts = [kind]
    if kind in ("topup", "fmriprep"):
        est = (
            kind == "fmriprep"
            or "--estmov" not in o
            or any(v is True for k, v in f.items() if k.startswith("estmov"))
        )
        parts.append("movements" if est else "no movements")
        if f.get("--splineorder") and f.get("--splineorder value") == "2":
            parts.append("quadratic")
        if f.get("FSL's configuration"):
            parts.append("FSL's configuration")
    else:
        parts.append(str(f.get("--method")))
    exact = [
        c["name"].split(": ")[-1] if ": " in c["name"] else c["name"]
        for c in doc["checks"]
        if not c["passed"] and (c["value"] is None or any(k in c["name"] for k in EXACT))
    ]
    parts.append(exact[0] if exact else "metrics")
    return " / ".join(parts)


def replay(path: Path, out: Path, memory_gib: float) -> dict:
    """The finding's drawn case (not its shrunk one: the trial judged the drawn case) replayed."""
    doc = json.loads(path.read_text(encoding="utf-8"))
    doc.pop("shrunk", None)
    path = out / path.name
    path.write_text(json.dumps(doc), encoding="utf-8")
    p = subprocess.run(
        [sys.executable, "-m", "larmorx_validation", "difftest", "--replay", str(path),
         "--memory-gib", str(memory_gib)],
        capture_output=True, text=True, check=False, timeout=1800,
    )  # fmt: skip
    (out / f"{path.stem}.log").write_text(p.stdout + p.stderr, encoding="utf-8")
    # "<tool> (<implementation>): <verdict> <signature>"
    lines = [ln for ln in p.stdout.splitlines() if ln.startswith("fsl-topup-original (")]
    verdict = lines[-1].split("): ", 1)[1].split(" ", 1) if lines else [f"exit {p.returncode}"]
    return {"finding": path.name, "verdict": verdict[0], "signature": [*verdict, ""][1]}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("trial")
    ap.add_argument("--per-group", type=int, default=40)
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--memory-gib", type=float, default=1.5)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    groups: dict[str, list[Path]] = collections.defaultdict(list)
    for p in sorted(Path(a.trial, "findings").glob("*/*.json")):
        groups[group_of(json.loads(p.read_text(encoding="utf-8")))].append(p)
    rng = random.Random(a.seed)
    todo = []
    for g, ps in sorted(groups.items()):
        todo += [(g, p) for p in rng.sample(ps, min(len(ps), a.per_group))]
    print(
        f"{sum(map(len, groups.values()))} findings in {len(groups)} groups; replaying {len(todo)}"
    )
    results = collections.defaultdict(collections.Counter)
    rows = []
    with concurrent.futures.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(replay, p, out, a.memory_gib): g for g, p in todo}
        for fut in concurrent.futures.as_completed(futs):
            g = futs[fut]
            r = fut.result()
            results[g][r["verdict"]] += 1
            rows.append({"group": g, **r})
    summary = {g: {"findings": len(groups[g]), "replayed": dict(c)} for g, c in results.items()}
    (out / "summary.json").write_text(json.dumps({"summary": summary, "rows": rows}, indent=1))
    for g in sorted(summary):
        print(f"{summary[g]['findings']:5d} {g}: {summary[g]['replayed']}")
    return 0


if __name__ == "__main__":
    os.environ.setdefault("LARMORX_ORACLE_CACHE_MODE", "use")
    raise SystemExit(main())
