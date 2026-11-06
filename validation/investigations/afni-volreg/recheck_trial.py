# SPDX-License-Identifier: Apache-2.0
"""Every finding of a random-testing trial of 3dvolreg's replica replayed with the current
build (``difftest --replay``: the shrunk recipe, AFNI's runs from the oracle cache), ``--jobs``
at a time; one JSON line per finding in ``<out.jsonl>`` and the counts by verdict and by the
trial's signatures (the triage of ``validation/difftest/triage.toml``'s ``afni-volreg``
entries).

Run inside a memory box (each replay holds well under 1 GiB), from the repository, with
``PYTHONPATH`` naming its ``python`` and ``validation/src`` and ``LARMORX_DIFFTEST_CACHE_MODE=use``:
``python -P recheck_trial.py <night> <out.jsonl> [--jobs 8]``.
"""

import collections
import concurrent.futures as cf
import json
import os
import re
import subprocess
import sys
from pathlib import Path

_LINE = re.compile(r"^(afni-volreg) \((replica)\): (\w+)\s*(.*)$")


def _replay(f: Path) -> dict:
    p = subprocess.run(
        [sys.executable, "-m", "larmorx_validation", "difftest", "--replay", str(f)],
        capture_output=True,
        text=True,
        env=os.environ.copy(),
        timeout=900,
    )
    text = p.stdout + p.stderr
    verdict, detail = "unclear", text[-400:]
    for line in text.splitlines():
        m = _LINE.match(line.strip())
        if m:
            verdict, detail = m.group(3), m.group(4)[:600]
    d = json.loads(f.read_text(encoding="utf-8"))
    rec = d.get("shrunk") or d["recipe"]
    return {
        "finding": f.name,
        "case": f"{d['tool']}:{d['seed']}:{d['n']}",
        "trial_signature": d.get("signature"),
        "verdict": verdict,
        "detail": detail,
        "args": " ".join(" ".join(g["args"]) for g in rec["groups"]),
        "content": rec["inputs"].get("input", {}).get("content", {}).get("type"),
        "dtype": rec["inputs"].get("input", {}).get("dtype"),
    }


def main(argv: list[str]) -> int:
    night, out = Path(argv[0]), Path(argv[1])
    jobs = int(argv[argv.index("--jobs") + 1]) if "--jobs" in argv else 8
    files = sorted((night / "findings").rglob("*.json"))
    rows = []
    with cf.ThreadPoolExecutor(jobs) as ex, out.open("w", encoding="utf-8") as fh:
        for row in ex.map(_replay, files):
            rows.append(row)
            fh.write(json.dumps(row) + "\n")
    by_verdict = collections.Counter(r["verdict"] for r in rows)
    print(f"{len(rows)} findings replayed: {dict(by_verdict)}")
    by_sig = collections.Counter((tuple(r["trial_signature"] or ()), r["verdict"]) for r in rows)
    for (sig, verdict), n in by_sig.most_common():
        print(f"  {n:5d}  {verdict:9s}  trial: {', '.join(sig)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
