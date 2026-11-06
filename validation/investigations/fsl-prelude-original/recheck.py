# SPDX-License-Identifier: Apache-2.0
"""Re-checks every finding of a random-testing trial of ``fsl-prelude-original`` with the current
build: each drawn case (not its shrunk one: the trial judged the drawn case) replayed with
``difftest --replay`` (FSL's runs from the oracle cache, ``use`` mode), at most ``--jobs`` at a
time, each in its own memory leaf of ``--memory-gib``. Run it inside a memory box
(``scripts/orchestration/boxed.sh``). Prints the verdicts by the trial's signature and writes every
replay's output and the summary (``summary.json``) to ``--out``.

    python -P recheck.py <trial dir> --jobs 6 --out <dir>
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures
import json
import subprocess
import sys
from pathlib import Path

TOOL = "fsl-prelude-original"


def replay(path: Path, out: Path, memory_gib: float) -> dict:
    doc = json.loads(path.read_text(encoding="utf-8"))
    doc.pop("shrunk", None)
    target = out / path.name
    target.write_text(json.dumps(doc), encoding="utf-8")
    p = subprocess.run(
        [sys.executable, "-m", "larmorx_validation", "difftest", "--replay", str(target),
         "--memory-gib", str(memory_gib)],
        capture_output=True, text=True, check=False, timeout=1800,
    )  # fmt: skip
    (out / f"{target.stem}.log").write_text(p.stdout + p.stderr, encoding="utf-8")
    lines = [ln for ln in p.stdout.splitlines() if ln.startswith(f"{TOOL} (")]
    verdict = lines[-1].split("): ", 1)[1].split(" ", 1) if lines else [f"exit {p.returncode}"]
    return {
        "finding": path.name,
        "trial": " / ".join(doc.get("signature") or []),
        "verdict": verdict[0],
        "signature": [*verdict, ""][1],
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("trial", type=Path)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--memory-gib", type=float, default=3.0)
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    files = sorted((a.trial / "findings" / TOOL).glob("*.json"))
    with concurrent.futures.ThreadPoolExecutor(a.jobs) as pool:
        results = list(pool.map(lambda f: replay(f, a.out, a.memory_gib), files))
    counts = collections.Counter((r["trial"], r["verdict"], r["signature"]) for r in results)
    for (trial, verdict, sig), n in sorted(counts.items(), key=lambda kv: -kv[1]):
        print(f"{n:6d}  trial: {trial or '-'}  now: {verdict} {sig}")
    (a.out / "summary.json").write_text(json.dumps(results, indent=1), encoding="utf-8")
    print(f"{len(results)} findings re-checked")
    return 0


if __name__ == "__main__":
    sys.exit(main())
