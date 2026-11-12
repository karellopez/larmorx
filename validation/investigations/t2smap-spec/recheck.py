# SPDX-License-Identifier: Apache-2.0
"""``check.py`` again on output folders that already exist, without running any program: per case
the verdict and the rows that fail; with ``--baseline`` another ``check.py`` (e.g. an earlier
version) judges the same folders beside it, and the report counts what changed. Written from the
spec only (a clean-room implementer may read and run it)::

    python -I recheck.py [--probe WORK REF OTHER]... [--fmriprep WORK REF OTHER]...
                         [--baseline OLD_CHECK_PY] [--jobs N] [--json FILE]

``--probe``: the folders ``compare.py`` (``../t2smap-original/``) leaves, ``WORK/<run>/<case>/out``
(each case run in ``WORK/<run>/<case>``): every case whose reference wrote the combined series and
whose other run wrote its call file. ``--fmriprep``: the folders ``fmriprep_calls.py`` leaves,
``WORK/<call>/<run>/out``. Exit status 1 when a case that the baseline passes fails (or, without
a baseline, when any case fails).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import multiprocessing as mp
import os
import sys
import traceback
import warnings
from pathlib import Path

HERE = Path(__file__).resolve().parent


def probe_pairs(work: Path, ref: str, other: str) -> list[tuple[str, Path, Path, Path]]:
    out = []
    for case in sorted(p for p in (work / ref).iterdir() if p.is_dir()):
        r, o = case / "out", work / other / case.name / "out"
        combined = (r / "desc-optcom_bold.nii.gz").exists() or (r / "ts_OC.nii.gz").exists()
        if combined and (o / "t2smap_call.sh").exists():
            out.append((f"{work.name}/{case.name}: {other} vs {ref}", r, o, case))
    return out


def fmriprep_pairs(work: Path, ref: str, other: str) -> list[tuple[str, Path, Path, Path]]:
    out = []
    for call in sorted(p for p in work.iterdir() if p.is_dir()):
        r, o = call / ref / "out", call / other / "out"
        if (r / "t2smap_call.sh").exists() and (o / "t2smap_call.sh").exists():
            out.append((f"{call.name}: {other} vs {ref}", r, o, call / ref))
    return out


def judge(task: tuple[str, str, str, str]) -> tuple[str, list[str]]:
    """One case with one checker, in a process of its own (each checker imports its own model)."""
    checker, ref, other, cwd = task
    warnings.simplefilter("ignore", RuntimeWarning)
    try:
        spec = importlib.util.spec_from_file_location("check_under_test", checker)
        mod = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = mod  # its dataclasses look their module up
        spec.loader.exec_module(mod)
        results = mod.check(Path(ref), Path(other), Path(cwd))
    except Exception:  # a case outside the checker's model: its error, as check.py exits on it
        return "ERROR", traceback.format_exc().strip().splitlines()[-1:]
    lines = [r.line() for r in results]
    return ("PASS" if all(r.ok for r in results) else "FAIL"), lines


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument(
        "--probe", nargs=3, action="append", default=[], metavar=("WORK", "REF", "OTHER")
    )
    p.add_argument(
        "--fmriprep", nargs=3, action="append", default=[], metavar=("WORK", "REF", "OTHER")
    )
    p.add_argument("--checker", type=Path, default=HERE / "check.py")
    p.add_argument("--baseline", type=Path, default=None)
    p.add_argument("--jobs", type=int, default=4)
    p.add_argument("--json", type=Path, default=None)
    a = p.parse_args(argv)
    pairs = []
    for work, ref, other in a.probe:
        pairs += probe_pairs(Path(work).resolve(), ref, other)
    for work, ref, other in a.fmriprep:
        pairs += fmriprep_pairs(Path(work).resolve(), ref, other)
    checkers = [a.checker.resolve()] + ([a.baseline.resolve()] if a.baseline else [])
    tasks = [(str(c), str(r), str(o), str(w)) for _, r, o, w in pairs for c in checkers]
    os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")  # the reference setup's one BLAS thread
    with mp.get_context("spawn").Pool(a.jobs, maxtasksperchild=1) as pool:
        done = pool.map(judge, tasks, chunksize=1)
    k = len(checkers)
    report, changes, worse = [], {}, 0
    for i, (name, *_rest) in enumerate(pairs):
        new = done[i * k]
        old = done[i * k + 1] if a.baseline else None
        key = f"{old[0]}->{new[0]}" if old else new[0]
        changes[key] = changes.get(key, 0) + 1
        bad = (old[0] == "PASS" and new[0] != "PASS") if old else new[0] != "PASS"
        worse += bad
        report.append({"case": name, "new": new, "old": old})
        print(f"{key:12s} {name}")
        if new[0] != "PASS" or (old and old[0] != new[0]):
            for line in new[1]:
                if not line.startswith("PASS"):
                    print(f"    new: {line}")
            if old:
                for line in old[1]:
                    if not line.startswith("PASS"):
                        print(f"    old: {line}")
    print("RECHECK-END " + ", ".join(f"{v} {k}" for k, v in sorted(changes.items())))
    if a.json:
        a.json.write_text(json.dumps(report, indent=1), encoding="utf-8")
    return 1 if worse else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
