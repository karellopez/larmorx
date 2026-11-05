# SPDX-License-Identifier: Apache-2.0
"""Replays a random-testing trial's findings of FLIRT's clean-room original (``difftest
--replay``, the oracle cache in ``use`` mode: FSL's runs served), at most 8 at a time, filtered
by program, and prints one line per finding: its case number, the program, the verdict now, the
first failed check. Memory findings are skipped. The clean-room catch-up (2026-11-05; after
``cleanroom-fast/replay_findings.py``).

    LARMORX_FLIRT_BIN=target/release/larmorx scripts/orchestration/boxed.sh 24 \\
        python -P validation/investigations/cleanroom-flirt/catchup_replay.py \\
        <trial>/findings/fsl-flirt-original --programs avscale,midtrans [--only 58,103] [--jobs 8]
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures
import json
import os
import subprocess
import sys
from pathlib import Path


def replay(path: Path) -> tuple[str, str]:
    p = subprocess.run(
        [sys.executable, "-m", "larmorx_validation", "difftest", "--replay", str(path)],
        capture_output=True,
        text=True,
        env={**os.environ, "LARMORX_ORACLE_CACHE_MODE": "use"},
        check=False,
    )
    lines = [ln for ln in p.stdout.splitlines() if ln.strip()]
    head = next((ln for ln in lines if ln.startswith("fsl-flirt-original")), "")
    detail = next((ln.strip() for ln in lines if ln.startswith("  ")), "")
    if not head:
        head = f"(exit {p.returncode}) {(p.stderr.strip().splitlines() or [''])[-1][:200]}"
    return head, detail[:400]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("findings")
    ap.add_argument("--programs", default="", help="programs, comma-separated (all by default)")
    ap.add_argument("--only", default="", help="case numbers, comma-separated")
    ap.add_argument("--jobs", type=int, default=8)
    a = ap.parse_args()
    programs = {x for x in a.programs.split(",") if x}
    only = {int(x) for x in a.only.split(",") if x}
    todo = []
    for f in sorted(Path(a.findings).glob("*.json")):
        doc = json.loads(f.read_text(encoding="utf-8"))
        if doc.get("verdict") == "memory" or "memory" in (doc.get("signature") or []):
            continue
        program = (doc.get("shrunk") or doc["recipe"])["extra"]["tool"]
        if (programs and program not in programs) or (only and doc["n"] not in only):
            continue
        todo.append((doc["n"], program, f))
    with concurrent.futures.ThreadPoolExecutor(max_workers=min(8, a.jobs)) as ex:
        results = list(ex.map(lambda t: (t[0], t[1], *replay(t[2])), todo))
    tally: collections.Counter = collections.Counter()
    for n, program, head, detail in results:
        verdict = "agree" if " agree" in head else "differ"
        tally[(program, verdict)] += 1
        print(f"{n} {program}: {head}")
        if detail and verdict != "agree":
            print(f"    {detail}")
    print("TALLY", dict(sorted(tally.items())))


if __name__ == "__main__":
    main()
