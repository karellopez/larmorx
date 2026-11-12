# SPDX-License-Identifier: Apache-2.0
"""The replay proof's tables (docs/validation/freesurfer-replay.md) from the reports that
``python -m larmorx_validation freesurfer-replay run ... --json <file>`` writes.

    python -P validation/investigations/freesurfer-oracle/proof_table.py <report.json>...

Prints, in Markdown: the counts (steps, programs, files by kind and verdict), one row per step
(run, stream, step, threads, verdict, its files by verdict, recorded and replayed seconds) and
every file that was not identical or provenance-only, with its detail.
"""

from __future__ import annotations

import getpass
import json
import re
import socket
import sys
from collections import Counter
from pathlib import Path


def public(text: str) -> str:
    """No private names in a public page: the machine's and the user's names, home paths."""
    text = re.sub(r"/home/[^/\s]+/", "<home>/", text)
    for name, mask in ((socket.gethostname(), "<host>"), (getpass.getuser(), "<user>")):
        if len(name) >= 3:
            text = text.replace(name, mask)
    return text


def main(paths: list[str]) -> int:
    reports = [r for p in paths for r in json.loads(Path(p).read_text(encoding="utf-8"))]
    files = [(r, f) for r in reports for f in r["files"]]
    programs = sorted({r["step"].split("-", 1)[1] for r in reports})
    by_verdict = Counter(r["verdict"] for r in reports)
    print(f"- **Steps replayed:** {len(reports)} ({len(programs)} programs, "
          f"{len({(r['run']) for r in reports})} runs): "
          + ", ".join(f"{n} {v}" for v, n in by_verdict.most_common()))  # fmt: skip
    kinds = Counter((f["kind"], f["verdict"]) for _r, f in files)
    table = {}
    for (k, v), n in kinds.items():
        table.setdefault(k, Counter())[v] = n
    verdicts = ["identical", "provenance", "log", "varies", "differs", "missing", "extra"]
    print(f"- **Files compared:** {len(files)}\n")
    print("| Kind | " + " | ".join(verdicts) + " |")
    print("|---|" + "---:|" * len(verdicts))
    for k in sorted(table):
        print(f"| {k} | " + " | ".join(str(table[k].get(v, "")) for v in verdicts) + " |")
    print("\n| Run | Step | Threads | Verdict | Files | Seconds (recorded, replayed) |")
    print("|---|---|---:|---|---|---|")
    for r in reports:
        c = Counter(f["verdict"] for f in r["files"])
        counts = ", ".join(f"{n} {v}" for v, n in sorted(c.items()))
        served = " (cache)" if r.get("served_from_cache") else ""
        print(f"| {r['run']} | {r['step']} | {r['threads']} | {r['verdict']} | {counts} | "
              f"{r['seconds']['recorded']:.1f}, {r['seconds']['replayed']:.1f}{served} |")  # fmt: skip
    odd = [(r, f) for r, f in files if f["verdict"] not in ("identical", "provenance")]
    if odd:
        print("\n| Step | File | Verdict | Detail |")
        print("|---|---|---|---|")
        for r, f in odd:
            detail = public(f["detail"]).replace("|", "\\|")[:300]
            path = public(f["path"] or f["replay_path"])
            print(f"| {r['run']} {r['step']} | `{path}` | {f['verdict']} | {detail} |")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
