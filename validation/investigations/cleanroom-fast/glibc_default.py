# SPDX-License-Identifier: Apache-2.0
"""The clean-room original's default against the replica's default with the C library's
functions in both: the original built with ``glibc_default.patch`` (``LXM_GLIBC=1`` makes its
default call glibc 2.35's ``exp``, ``log``, ``expf``, ``logf`` as ``reproduce`` does) and the
replica's program (``larmorx-nc``, ``LARMORX_NC_BIN``), both with ``LARMORX_UPSTREAM_DEFECTS=fix``,
on suite cases (the original's suite, ``parity.fsl_fast``): exit status, standard output and
every image's voxels, bit for bit. Where they agree, the default's corrections (FSL-202's
variance, FSL-203's extrema, the others) are the replica's exactly, and what remains between
the two defaults is the correctly rounded functions' last bits. A development tool of the
clean-room catch-up (relay 2, 2026-11-05); no FSL run.

    LARMORX_FAST_BIN=<larmorx built with glibc_default.patch> LARMORX_NC_BIN=<larmorx-nc> \\
        scripts/orchestration/boxed.sh 16 python -P \\
        validation/investigations/cleanroom-fast/glibc_default.py "fmriprep/|real/|opt/" [--jobs 8]
"""

from __future__ import annotations

import argparse
import concurrent.futures
import os
import re
import subprocess
import tempfile
from pathlib import Path

import numpy as np


def _data(path: Path) -> bytes:
    from larmorx_validation.parity.fsl_fast import _load

    return np.ascontiguousarray(_load(path)[1]).tobytes()


def _run(exe: str, args: list[str], cwd: Path, settings: dict, extra: dict) -> tuple[int, str]:
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    env = {**_environment(settings), **extra, "LARMORX_UPSTREAM_DEFECTS": "fix"}
    env["OMP_NUM_THREADS"] = "4"
    p = subprocess.run(
        [exe, "fsl", "fast", *args], cwd=cwd, env=env, capture_output=True, text=True, check=False
    )
    return p.returncode, p.stdout.replace(str(cwd), "{cwd}")


def one(case) -> str:
    from larmorx_validation.parity import fsl_fast as orig
    from larmorx_validation.parity import fsl_fast_replica as rep

    kind, payload = case.payload
    if kind == "fmriprep":
        run_name, node = payload
        src = rep.fmriprep_input(run_name, node)
        if src is None:
            return f"{case.id}: skipped (no input)"
        args = rep.fmriprep_args(Path(node))
        settings: dict = {}

        def prepare(t: Path) -> list[str]:
            import shutil

            shutil.copy(src, t / args[-1])
            return args
    else:
        run = payload if kind == "direct" else rep._all_cases()[payload][2]
        settings = dict(run.settings)

        def prepare(t: Path) -> list[str]:
            return orig._prepare(run, t)

    with tempfile.TemporaryDirectory() as a_, tempfile.TemporaryDirectory() as b_:
        a, b = Path(a_), Path(b_)
        args = prepare(a)
        prepare(b)
        before = {p.name for p in a.iterdir()}
        ca, oa = _run(os.environ["LARMORX_NC_BIN"], args, a, settings, {})
        cb, ob = _run(
            os.environ["LARMORX_FAST_BIN"],
            args,
            b,
            settings,
            {"LXM_GLIBC": "1", "LARMORX_IMPLEMENTATION": "original"},
        )
        if ca != cb:
            return f"{case.id}: exit {ca} / {cb}"
        bad = [] if oa == ob else ["stdout"]
        files = sorted(p.name for p in a.iterdir() if p.name not in before)
        for f in files:
            if not (b / f).is_file():
                bad.append(f"{f} missing")
            elif re.search(r"\.(nii|hdr)(\.(gz|zst|bz2))?$", f) and _data(a / f) != _data(b / f):
                bad.append(f)
        return f"{case.id}: {'identical' if not bad else 'differ ' + ', '.join(bad)} ({len(files)} files)"


def main() -> None:
    from larmorx_validation.parity import fsl_fast as orig

    ap = argparse.ArgumentParser()
    ap.add_argument("pattern")
    ap.add_argument("--jobs", type=int, default=8)
    a = ap.parse_args()
    cases = [c for c in orig.cases("standard") if re.search(a.pattern, c.id)]
    with concurrent.futures.ThreadPoolExecutor(a.jobs) as ex:
        for line in ex.map(one, cases):
            print(line, flush=True)


if __name__ == "__main__":
    main()
