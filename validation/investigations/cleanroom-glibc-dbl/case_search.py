# SPDX-License-Identifier: Apache-2.0
"""Black-box search for inputs where glibc 2.35's functions (``reproduce``) and the correctly
rounded ones (larmorx's default) give a clean-room original other outputs (GLIBC-07).

The suites' cases ``orig/glibc-*`` (``fsl_maths.py``) and ``methods/glibc-sinf-*``
(``afni_tshift.py``) and the golden cases of the reproduce paths take their parameters from
this search. It runs larmorx's command line in both modes and compares the outputs; the
``bptf-weights`` search compares the system's glibc ``exp`` (CPython's ``math.exp``, a black
box) with the correctly rounded value (mpmath) on ``-bptf``'s weight arguments.

Usage (with the worktree's package on ``PYTHONPATH``, inside ``scripts/orchestration/boxed.sh``):

    python -P case_search.py fslmaths <dir>     # -s, -kernel gauss, -bptf, -tfce parameters
    python -P case_search.py bptf-weights       # sigmas whose -bptf weights differ
    python -P case_search.py tshift <dir> [n]   # slice times for -wsinc5 / -wsinc9
"""

from __future__ import annotations

import math
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np


def _larmorx(argv: list[str], mode: str, cwd: Path) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env["LARMORX_UPSTREAM_DEFECTS"] = mode
    env["LARMORX_IMPLEMENTATION"] = "original"
    env["FSLOUTPUTTYPE"] = "NIFTI_GZ"
    env["OMP_NUM_THREADS"] = "1"
    return subprocess.run(
        [sys.executable, "-m", "larmorx", *argv], cwd=cwd, env=env, capture_output=True, text=True
    )


def _differing(a: np.ndarray, b: np.ndarray) -> int:
    return int(np.count_nonzero((a != b) & ~(np.isnan(a) & np.isnan(b))))


def _both_modes(argv: list[str], out: str, work: Path) -> tuple[int | str, bool, bool]:
    """The values that differ between the modes' outputs ``out`` (each run in a directory of
    its own, where ``out`` is written), and whether each mode named GLIBC-07."""
    import nibabel as nib

    data, named = [], []
    for mode in ("fix", "reproduce"):
        d = Path(tempfile.mkdtemp(dir=work))
        p = _larmorx(argv, mode, d)
        if p.returncode:
            return f"exit {p.returncode}: {p.stderr[-160:]}", False, False
        data.append(np.asanyarray(nib.load(d / out).dataobj))
        named.append("GLIBC-07" in p.stderr)
    return _differing(*data), named[0], named[1]


def fslmaths(work: Path) -> None:
    """The fslmaths suite's own inputs (``fsl_maths.own_inputs``): parameters of the filters
    whose default and reproduce outputs differ."""
    from larmorx_validation.parity import fsl_maths

    base = fsl_maths.own_inputs()
    work.mkdir(parents=True, exist_ok=True)
    f = lambda n: str(base / f"{n}.nii.gz")  # noqa: E731
    jobs = [
        ["fsl", "fslmaths", f("flt24"), "-s", f"{s:.2f}", "out"] for s in np.arange(0.5, 6.0, 0.05)
    ]
    jobs += [
        ["fsl", "fslmaths", f("flt24"), "-kernel", "gauss", f"{s:.2f}", "-fmean", "out"]
        for s in np.arange(0.5, 6.0, 0.05)
    ]
    jobs += [
        ["fsl", "fslmaths", "-dt", "double", f("dbl4d"), "-bptf", s, "-1", "out", "-odt", "double"]
        for s in ("1.7", "1.73", "5.72")
    ]
    jobs += [
        ["fsl", "fslmaths", f("tstat"), "-tfce_delta", d, "-tfce", h, e, "6", "out"]
        for d in ("0.01", "0.02")
        for h in ("1.3", "1.7", "2")
        for e in ("0.5", "0.6", "0.7")
    ]
    with ThreadPoolExecutor(16) as ex:
        found = list(ex.map(lambda a: (a, _both_modes(a, "out.nii.gz", work)), jobs))
    for argv, (n, nf, nr) in found:
        if n:
            print(" ".join(Path(x).name for x in argv[2:]), "->", n, "named", (nf, nr))
    print(f"fslmaths: {len(jobs)} runs, {sum(1 for _, (n, _, _) in found if n)} differ")


def bptf_weights() -> None:
    """Sigmas (two decimals, 0.6 to 20) for which a weight exp(-0.5 d² / (s s)) of -bptf (s the sigma)
    differs between glibc's exp (the system's, through CPython) and the correctly rounded one."""
    import mpmath

    mpmath.mp.prec = 200

    def cr_exp(q: float) -> float:
        v = mpmath.exp(mpmath.mpf(q))
        f = float(v)
        return min(
            (f, math.nextafter(f, 0), math.nextafter(f, 2)), key=lambda c: abs(mpmath.mpf(c) - v)
        )

    hits = []
    for k in range(60, 2000):
        s = k / 100
        for d in range(1, int(4 * s) + 3):
            q = -0.5 * float(d * d) / (s * s)
            if math.exp(q) != cr_exp(q):
                hits.append((s, d))
                break
    print(f"bptf-weights: {len(hits)} of 1940 sigmas; the first: {hits[:20]}")


def tshift(work: Path, n: int) -> None:
    """Slice times (4 slices, the first 0) for which 3dTshift's weighted sinc on the suite's
    float32 series differs between the modes."""
    import larmorx_testdata as td

    work.mkdir(parents=True, exist_ok=True)
    img = str(td.get("synthetic/tshift/dtypes/float32.nii.gz"))
    rng = np.random.default_rng(5)
    jobs = []
    for method in ("-wsinc5", "-wsinc9"):
        for _ in range(n):
            times = "0 " + " ".join(f"{v:.3f}" for v in rng.uniform(0, 1.9, 3))
            jobs.append(["afni", "3dTshift", method, "-tpattern", f"@1D: {times}", "-prefix", "o.nii", img])  # fmt: skip
    with ThreadPoolExecutor(16) as ex:
        found = list(ex.map(lambda a: (a, _both_modes(a, "o.nii", work)), jobs))
    for argv, (k, nf, nr) in found:
        if k:
            print(argv[2], argv[4], "->", k, "named", (nf, nr))
    print(f"tshift: {len(jobs)} runs, {sum(1 for _, (k, _, _) in found if k)} differ")


if __name__ == "__main__":
    what = sys.argv[1] if len(sys.argv) > 1 else ""
    if what == "fslmaths":
        fslmaths(Path(sys.argv[2]))
    elif what == "bptf-weights":
        bptf_weights()
    elif what == "tshift":
        tshift(Path(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 15)
    else:
        print(__doc__)
        sys.exit(2)
