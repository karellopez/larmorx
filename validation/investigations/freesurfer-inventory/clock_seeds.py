# SPDX-License-Identifier: Apache-2.0
"""Which recorded programs drew random numbers from a clock seed (variation.md §3).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it names what FreeSurfer's source prints and why.

    python -P validation/investigations/freesurfer-inventory/clock_seeds.py \\
        <workspace>/oracles/freesurfer-8.2.0/runs

FreeSurfer's ``randomNumber`` (``utils/utils.cpp``) seeds itself from ``time(NULL)`` when no seed
was set and ``FREESURFER_SEED`` is empty, printing once per process ``<comm> supposed to be
reproducible but seed not set`` to standard error (``<comm>``: ``/proc/self/comm``, at most 15
characters). recon-all's log holds every program's standard error, so each such line is one process
of a default path that drew from a clock seed. For each run (the folders ending in ``-v8`` or
``-classic`` by default) this prints, per program, how many processes did and the command line that
preceded the warning (the last line of the log starting with a program name of that ``comm``).

Only ``randomNumber``'s fallback prints the warning: the time-of-day seeds of ``drand48``
(``PDFtodSeed``) print nothing and are not seen here.
"""

from __future__ import annotations

import argparse
import re
from collections import Counter, defaultdict
from pathlib import Path

WARNING = re.compile(r"^(?P<comm>\S{1,15}) supposed to be reproducible but seed not set$")


def scan(log: Path) -> list[tuple[str, str]]:
    """(comm, the command line before it) for every warning of one recon-all.log."""
    out: list[tuple[str, str]] = []
    last: dict[str, str] = {}
    for line in log.read_text(errors="replace").splitlines():
        s = line.strip()
        m = WARNING.match(s)
        if m:
            comm = m["comm"]
            cmd = next((c for p, c in last.items() if p[:15] == comm), "")
            out.append((comm, cmd))
            continue
        prog = s.split(" ", 1)[0] if s else ""
        if re.fullmatch(r"[A-Za-z][\w.+-]*", prog) and " " in s:
            last[prog] = s
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("runs", type=Path)
    ap.add_argument("--run-pattern", default=r"-(classic|v8)$")
    a = ap.parse_args()
    per_prog: dict[str, Counter] = defaultdict(Counter)
    calls: dict[str, Counter] = defaultdict(Counter)
    nruns = 0
    for run in sorted(p for p in a.runs.iterdir() if re.search(a.run_pattern, p.name)):
        # the subject's own log (fsaverage, linked or copied beside it, has FreeSurfer's)
        logs = sorted(
            p
            for p in run.glob("subjects/*/scripts/recon-all.log")
            if not p.parts[-3].startswith("fsaverage")
        )
        if not logs:
            continue
        nruns += 1
        for comm, cmd in scan(logs[0]):
            per_prog[comm][run.name] += 1
            # the program and its first options only (no paths: they hold the machine's folders)
            words = cmd.split()
            calls[comm][" ".join(words[:1] + [w for w in words[1:] if w.startswith("-")][:2])] += 1
    print(f"runs: {nruns}")
    for comm in sorted(per_prog):
        counts = sorted(set(per_prog[comm].values()))
        print(
            f"{comm}: in {len(per_prog[comm])} runs, {'/'.join(map(str, counts))} processes per run"
        )
        for c, n in calls[comm].most_common():
            print(f"    {n:4d}  {c}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
