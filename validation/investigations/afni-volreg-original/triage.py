# SPDX-License-Identifier: Apache-2.0
"""Triage of a random-testing night of 3dvolreg's clean-room original (``afni-volreg-original``):
for each case beyond the record's thresholds, whether the shear decomposition's exact ties
(``specs/3dvolreg.md`` §5.3, §16) move the original's own result as far.

Written by the clean-room implementer (black boxes only). Where two axes have the same voxel
size, pairs of axis orders have equal norms in exact arithmetic and the evaluation's last bits
decide between them at every resampling; AFNI's evaluation is not specified, so the original
cannot follow its choices, and §14.3's thresholds allow for either side of every tie on the
spec writer's 15 cases. The original takes either side on request (``LARMORX_VOLREG_TIES=first``
or ``last``, a validation switch, ``crates/larmorx-afni/src/volreg/rotate.rs``). Each differing
case's drawn command line runs again: the original with ``reproduce`` and each tie side, and,
with ``-verbose -verbose`` prepended, AFNI (through the oracle cache) and the original, whose
final-pass ``Iteration complete at <k> steps`` lines give each volume's iteration count. With
``bound`` the parameters' per-volume threshold of §14.3 and ``r`` the largest ``|Δ| / bound``:

- ``r_original``: the original (its default tie rule) against AFNI;
- ``r_ties``: the largest of the three tie sides against each other (the original's own spread
  from the ties alone);
- verdicts: ``runaway`` (AFNI's run failed, or a parameter beyond 15 degrees or 20 mm: far outside
  the small motions 3dvolreg is made for); ``beyond the parameters`` (the parameters within their
  bounds, another check failed); ``within the ties' spread`` (``r_ties >= r_original / 2``, or a
  tie side within the bounds of AFNI); ``not converged`` (else, every volume beyond its bounds
  stopped at the iteration limit M in AFNI or in the original: 23, ``-maxite``, ``-params``, 66
  with ``-twopass`` for an M below 10); ``converged and beyond`` (else: examined case by case).

    python -P triage.py <night directory> [--jobs N] [--limit N] [--out FILE]

Run inside ``scripts/orchestration/boxed.sh``, with ``LARMORX_WORKSPACE`` and ``PYTHONPATH`` set as
the suite's are. Prints the counts and the ``converged and beyond`` cases; ``--out`` writes a JSON
summary with one row per case.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import sys
import tempfile
from concurrent.futures import ProcessPoolExecutor
from multiprocessing import get_context
from pathlib import Path

import numpy as np

_DONE = re.compile(r"Iteration complete at (\d+) steps")


def _iterations(stderr: str) -> list[int]:
    """The final pass's iteration counts, in the order of its fits (after its first line)."""
    tail = stderr.split("Starting final pass", 1)[-1] if "Starting final pass" in stderr else stderr
    return [int(m) for m in _DONE.findall(tail)]


def _limit(argv: list[str]) -> int:
    m, twopass = 23, "-twopass" in argv
    for i, w in enumerate(argv):
        if (w.startswith("-max") and w not in ("-maxdisp", "-maxdisp1D")) and i + 1 < len(argv):
            v = int(float(argv[i + 1]))
            m = v if v > 0 else m
        if w.startswith("-par") and i + 1 < len(argv):
            m = int(float(argv[i + 1]))
    return 66 if twopass and m < 10 else m


def _case(finding: str) -> dict:
    from larmorx_validation.difftest import inputs
    from larmorx_validation.difftest.tools import afni_volreg_original as tool
    from larmorx_validation.parity import afni_volreg_original as vo

    f = json.loads(Path(finding).read_text(encoding="utf-8"))
    recipe = f["recipe"]
    prefix = inputs.register(recipe["inputs"])
    argv = ["-verbose", "-verbose", *tool._argv(recipe, prefix)]
    src = Path(argv[-1])
    work = Path(tempfile.mkdtemp(prefix="volreg-triage_", dir=vo._scratch()))
    try:
        runs = {}
        plain = argv[2:]
        for name in ("afni", "original", "first", "last"):
            d = work / name
            d.mkdir()
            if name == "afni":
                r = vo.run_afni(argv, d, {}, [Path(w) for w in argv if Path(w).is_file()])
            elif name == "original":
                r = vo.run_lx("original", "reproduce", argv, d, {}, 1)
            else:
                r = vo.run_lx("original", "reproduce", plain, d, {"LARMORX_VOLREG_TIES": name}, 1)  # fmt: skip
            p1 = d / "p.1D"
            runs[name] = (r.code, vo._table(p1, 6) if p1.is_file() else None, _iterations(r.stderr))  # fmt: skip
        s = vo.settings_of(argv)
        g = vo.geometry_of(src)
        (ca, pa, ia), (co, po, io) = runs["afni"], runs["original"]
        row = {
            "case": f"{f['tool']}:{f['seed']}:{f['n']}",
            "failed": sorted({c.split(": ", 1)[-1] for c in f.get("signature", [])}),
            "exits": [ca, co],
            "method": s.method,
            "limit": _limit(argv),
            "iterations": [ia, io],
        }
        if pa is None or po is None or pa.shape != po.shape or ca != 0:
            row["verdict"] = "runaway"
            row["why"] = "a run failed or wrote no parameters"
            return row
        b = vo.parameter_bounds(pa, po, s, g).param_bound
        ratio = (np.abs(po - pa) / b).max(axis=1)
        row["r_original"] = float(ratio.max())
        big = float(np.abs(pa[:, :3]).max()), float(np.abs(pa[:, 3:]).max())
        row["afni_largest"] = big
        # the fitted volumes, in order: all but those with no fit block (the base, empty ones)
        fitted = [v for v in range(pa.shape[0]) if np.any(pa[v] != 0) or np.any(po[v] != 0)]
        beyond = [v for v in range(pa.shape[0]) if ratio[v] > 1.0]
        row["beyond"] = beyond
        sides = [po] + [runs[k][1] for k in ("first", "last") if runs[k][1] is not None and runs[k][1].shape == po.shape]  # fmt: skip
        r_ties = max((float((np.abs(x - y) / b).max()) for x in sides for y in sides), default=0.0)  # fmt: skip
        r_side = min(float((np.abs(x - pa) / b).max()) for x in sides)
        row["r_ties"], row["r_best_side"] = r_ties, r_side
        if big[0] > 15 or big[1] > 20:
            row["verdict"] = "runaway"
        elif not beyond:
            row["verdict"] = "beyond the parameters"
        elif r_ties >= row["r_original"] / 2 or r_side <= 1.0:
            row["verdict"] = "within the ties' spread"
        else:
            m = row["limit"]
            stuck = set()
            for counts in (ia, io):
                if len(counts) == len(fitted):
                    stuck |= {v for v, k in zip(fitted, counts, strict=True) if k >= m}
            row["verdict"] = "not converged" if set(beyond) <= stuck else "converged and beyond"
        return row
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("night")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--out", default="")
    a = ap.parse_args(argv)
    found = sorted((Path(a.night) / "findings" / "afni-volreg-original").glob("*.json"))
    if a.limit:
        found = found[: a.limit]
    with ProcessPoolExecutor(a.jobs, mp_context=get_context("spawn")) as pool:
        rows = list(pool.map(_case, [str(f) for f in found]))
    counts: dict[str, int] = {}
    for r in rows:
        counts[r["verdict"]] = counts.get(r["verdict"], 0) + 1
    print(json.dumps(counts))
    for r in rows:
        if r["verdict"] == "converged and beyond":
            print(json.dumps(r))
    if a.out:
        Path(a.out).write_text(json.dumps({"counts": counts, "rows": rows}, indent=1), encoding="utf-8")  # fmt: skip
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
