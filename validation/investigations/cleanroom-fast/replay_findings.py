# SPDX-License-Identifier: Apache-2.0
"""Replays a random-testing trial's findings of FAST's clean-room original
(``difftest --replay``, the oracle cache in ``use`` mode: FSL's runs served), at most 8 at a time,
and prints one line per finding: its case number, the verdict now, the failed checks. Memory
findings are skipped (CLAUDE.md: one at a time, by hand). A development tool of FAST's clean-room
original (relay 4, 2026-11-03).

    LARMORX_FAST_BIN=target/release/larmorx scripts/orchestration/boxed.sh 24 \\
        python -P validation/investigations/cleanroom-fast/replay_findings.py \\
        <trial>/findings/fsl-fast-original [--only 161,441] [--jobs 8]
"""

from __future__ import annotations

import argparse
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
    head = next((ln for ln in lines if ln.startswith("fsl-fast-original")), "")
    detail = next((ln.strip() for ln in lines if ln.startswith("  ")), "")
    if not head:
        head = f"(exit {p.returncode}) {(p.stderr.strip().splitlines() or [''])[-1][:200]}"
    return head, detail[:400]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("findings")
    ap.add_argument("--only", default="", help="case numbers, comma-separated")
    ap.add_argument("--jobs", type=int, default=8)
    a = ap.parse_args()
    only = {int(x) for x in a.only.split(",") if x}
    todo = []
    for f in sorted(Path(a.findings).glob("*.json")):
        doc = json.loads(f.read_text(encoding="utf-8"))
        if doc.get("verdict") == "memory" or "memory" in (doc.get("signature") or []):
            print(f"{doc['n']}: memory finding, skipped")
            continue
        if only and doc["n"] not in only:
            continue
        todo.append((doc["n"], f))
    with concurrent.futures.ThreadPoolExecutor(max_workers=min(8, a.jobs)) as ex:
        results = list(ex.map(lambda t: (t[0], *replay(t[1])), todo))
    for n, head, detail in results:
        print(f"{n}: {head}")
        if detail and "agree" not in head:
            print(f"    {detail}")


if __name__ == "__main__":
    main()
