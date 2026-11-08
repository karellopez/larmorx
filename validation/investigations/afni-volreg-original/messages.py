# SPDX-License-Identifier: Apache-2.0
"""3dvolreg's command line, messages and failures: the clean-room original against AFNI 26.3.05
(``reproduce``) on command lines of ``specs/3dvolreg.md`` §9 (the option scan, the checks after
it, the notices of obsolete options), as black boxes.

Each case runs AFNI (through the oracle-output cache) and the original in fresh directories, and
compares the exit status and standard error, AFNI's banner and compile date and larmorx's own
lines set aside (``blackbox._lines``), and standard output. With ``REF=replica`` (and
``LARMORX_UPSTREAM_DEFECTS`` fix or reproduce) the reference is the replica, a black box, and
larmorx's own lines are compared too.

    python -P messages.py [case ...]
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import blackbox as bb  # noqa: E402

IN = str(bb.INPUTS / "lps_f32.nii.gz")
V0 = str(bb.INPUTS / "lps_f32_vol0.nii.gz")
OUT = ["-prefix", "out.nii"]

#: name -> (arguments, extra environment)
CASES: dict[str, tuple[list[str], dict[str, str]]] = {
    "too-few": ([*OUT], {}),
    "too-many": ([*OUT, IN, IN], {}),
    "unknown": ([*OUT, "-bogus", IN], {}),
    "final-bad": ([*OUT, "-final", "spline", IN], {}),
    "twoblur-small": ([*OUT, "-twoblur", "1.5", IN], {}),
    "zpad-negative": ([*OUT, "-zpad", "-1", IN], {}),
    "zpad-twice": ([*OUT, "-zpad", "2", "-zpad", "3", IN], {}),
    "edging-negative": ([*OUT, "-edging", "-1", IN], {}),
    "edging-percent": ([*OUT, "-edging", "30%", IN], {}),
    "edging-env": ([*OUT, IN], {"AFNI_VOLREG_EDGING": "40%"}),
    "tshift-negative": ([*OUT, "-tshift", "-1", IN], {}),
    "base-negative": ([*OUT, "-base", "-1", IN], {}),
    "base-too-large": ([*OUT, "-base", "9", IN], {}),
    "base-missing": ([*OUT, "-base", "nothere.nii", IN], {}),
    "base-twice": ([*OUT, "-base", "1", "-base", "2", IN], {}),
    "input-missing": ([*OUT, "-input", "nothere.nii"], {}),
    "dataset-missing": ([*OUT, "nothere.nii"], {}),
    "weight-missing": ([*OUT, "-weight", "nothere.nii[0]", IN], {}),
    "base-not-nifti": ([*OUT, "-base", "a.HEAD", IN], {}),
    "weight-no-bracket": ([*OUT, "-weight", V0, IN], {}),
    "weight-no-index": ([*OUT, "-weight", V0 + "[x]", IN], {}),
    "weight-bad-index": ([*OUT, "-weight", V0 + "[3]", IN], {}),
    "matrix-twice": ([*OUT, "-1Dmatrix_save", "a", "-1Dmatrix_save", "b", IN], {}),
    "matrix-last": ([*OUT, "-1Dmatrix_save"], {}),
    "input-twice": ([*OUT, "-input", IN, "-input", IN], {}),
    "input-extra": ([*OUT, "-input", IN, "extra"], {}),
    "input-last": ([*OUT, "-input"], {}),
    "notices": ([*OUT, "-clipit", "-coarserot", "-wtrim", IN], {}),
    "one-volume": ([*OUT, V0], {}),
    "prefix-null": (["-prefix", "NULL", "-1Dfile", "p.1D", IN], {}),
    "rotcom": ([*OUT, "-rotcom", IN], {}),
    "dfile": ([*OUT, "-dfile", "d.txt", IN], {}),
    "maxdisp1d-stdout": ([*OUT, "-maxdisp1D", "-", IN], {}),
    "base-dset": ([*OUT, "-verbose", "-base", V0, IN], {}),
    "weight": ([*OUT, "-weight", V0 + "[0]", IN], {}),
    "wtinp": ([*OUT, "-wtinp", IN], {}),
    "maxite-prefix": ([*OUT, "-maxd", "3", IN], {}),
    "params": ([*OUT, "-params", "5", "0.02", "0.05", "0.9", IN], {}),
    "gridparent-alone": ([*OUT, "-gridparent", V0, IN], {}),
}


def run(who: str, args: list[str], extra: dict[str, str], d: Path):
    from larmorx_validation import oracle_cache

    d.mkdir(parents=True)
    env = {**bb.clean_env(d), **extra}
    ref = os.environ.get("REF", "afni")
    mode = os.environ.get("LARMORX_UPSTREAM_DEFECTS", "reproduce")
    if who == "afni" and ref == "afni":
        return oracle_cache.run(
            [str(bb.AFNI), *args],
            env=env,
            cwd=d,
            outputs=[d],
            policy=oracle_cache.Policy(
                threads=1,
                family="afni",
                nifti_extensions="AFNI's history: the date and the output's path",
            ),
            runner="afni-volreg-original-messages",
            timeout=600,
        )
    impl = "replica" if who == "afni" else "original"
    env.update(LARMORX_IMPLEMENTATION=impl, LARMORX_UPSTREAM_DEFECTS=mode)
    return subprocess.run(
        [bb.LX, "afni", "3dvolreg", *args], cwd=d, env=env, capture_output=True, timeout=600
    )


def main(argv: list[str]) -> int:
    root = Path(tempfile.mkdtemp(prefix="volreg-messages_", dir=os.environ.get("VOLREG_WORK")))
    differ = 0
    for name in argv or list(CASES):
        args, extra = CASES[name]
        ra = run("afni", args, extra, root / name / "afni")
        rb = run("original", args, extra, root / name / "original")
        if os.environ.get("REF", "afni") == "afni":
            la, lb = bb._lines(bb._err(ra)), bb._lines(bb._err(rb))
        else:  # the replica: larmorx's own lines compared too
            la, lb = (bb._err(r).decode(errors="replace").splitlines()[1:] for r in (ra, rb))
            la = [x for x in la if not x.startswith("++ implementation:")]
            lb = [x for x in lb if not x.startswith("++ implementation:")]
        oa, ob = bb._out(ra), bb._out(rb)
        same = ra.returncode == rb.returncode and la == lb and oa == ob
        differ += not same
        print(f"{name:20} exit {ra.returncode}/{rb.returncode}  {'same' if same else 'DIFFERENT'}")
        if not same:
            import difflib

            for line in difflib.unified_diff(la, lb, "afni", "original", lineterm="", n=0):
                print("    " + line[:240])
            if oa != ob:
                print("    stdout differs:", oa[:200], "|", ob[:200])
    print(f"{differ} of {len(argv or CASES)} differ")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
