# SPDX-License-Identifier: Apache-2.0
"""Runs one FAST command line with FSL 6.0.7.23 (through the oracle cache, under the
fsl-fast-replica suite's keys) and, unless ``--fsl-only``, the clean-room original
(``LARMORX_FAST_BIN``, else ``python -m larmorx``) into two kept directories, and prints the
streams and each image's agreement (per-class Dice and identical voxels for the label images,
mean and 99.9th-percentile absolute differences for the others, inside FSL's mask). A
development tool of FAST's clean-room original (relay 1, 2026-11-02).

    PYTHONPATH=python:validation/src python -P validation/investigations/cleanroom-fast/probe.py \
        <out-dir> --case opt/type2-verbose [--mode reproduce|fix] [--fsl-only]
    ... probe.py <out-dir> --args "-v -o seg @t1_half.nii.gz"
    ... probe.py <out-dir> --fmriprep ds000005-sub-01-noreconall

Run it inside scripts/orchestration/boxed.sh (it runs FSL and the original on real images).
"""

from __future__ import annotations

import argparse
import shlex
import shutil
import time
from pathlib import Path

import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_fast_replica as rep


def _run_original(args: list[str], cwd: Path, settings: dict, mode: str):
    from larmorx_validation.parity import fsl_fast as orig

    return orig.run_original(args, cwd=cwd, settings=settings, mode=mode)


def _load(p: Path):
    import nibabel as nib

    return np.asarray(nib.load(str(p)).dataobj)


def agreement(a: Path, b: Path) -> None:
    seg = next(a.glob("*_seg.nii*"), None)
    mask = _load(seg) > 0 if seg is not None else None
    for fa in sorted(a.glob("*.nii*")):
        fb = b / fa.name
        if not fb.is_file():
            print(f"  {fa.name}: missing from the original")
            continue
        x, y = _load(fa), _load(fb)
        if x.shape != y.shape:
            print(f"  {fa.name}: shapes {x.shape} {y.shape}")
            continue
        m = mask if mask is not None and mask.shape == x.shape else np.ones(x.shape, bool)
        if x.dtype.kind in "iu":
            same = float(np.mean(x[m] == y[m])) if m.any() else 1.0
            dice = []
            for c in sorted(set(np.unique(x[m])) | set(np.unique(y[m]))):
                xa, ya = x[m] == c, y[m] == c
                s = xa.sum() + ya.sum()
                dice.append(f"{c}:{2 * (xa & ya).sum() / s:.5f}" if s else f"{c}:-")
            print(f"  {fa.name}: identical {same:.6f}; Dice {' '.join(dice)}")
        else:
            xs, ys = x[m].astype(np.float64), y[m].astype(np.float64)
            d = np.abs(xs - ys)
            ok = ~np.isnan(d)
            nan = int(np.sum(~ok))
            if not ok.any():
                print(f"  {fa.name}: no comparable voxels (NaN {nan})")
                continue
            rel = d[ok] / np.maximum(np.abs(xs[ok]), 1e-30)
            print(
                f"  {fa.name}: mean |d| {d[ok].mean():.3g}, p99.9 {np.percentile(d[ok], 99.9):.3g}, "
                f"max {d[ok].max():.3g}, rel p99.9 {np.percentile(rel, 99.9):.3g}, NaN {nan}, "
                f"bit-identical {float(np.mean(xs == ys)):.4f}"
            )
    extra = sorted(p.name for p in b.iterdir() if not (a / p.name).exists())
    if extra:
        print(f"  only the original's: {extra}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--case")
    ap.add_argument("--args")
    ap.add_argument("--fmriprep")
    ap.add_argument("--mode", default=defects.REPRODUCE)
    ap.add_argument("--fsl-only", action="store_true")
    ap.add_argument("--quiet", action="store_true", help="streams' first and last lines only")
    a = ap.parse_args()
    out = Path(a.out)
    shutil.rmtree(out, ignore_errors=True)
    fa, fb = out / "fsl", out / "original"
    fa.mkdir(parents=True)
    fb.mkdir(parents=True)
    settings: dict = {}
    if a.fmriprep:
        node = next(n for r, n in rep.fmriprep_calls() if r == a.fmriprep)
        src = rep.fmriprep_input(a.fmriprep, str(node))
        args = rep.fmriprep_args(node)
        for d in (fa, fb):
            shutil.copy(src, d / args[-1])
    else:
        run = rep._all_cases()[a.case][2] if a.case else rep.Run(tuple(shlex.split(a.args)))
        settings = dict(run.settings)
        args = rep._prepare(run, fa)
        rep._prepare(run, fb)
    print("args:", " ".join(args))
    t0 = time.time()
    p = rep.run_oracle(args, cwd=fa, settings=settings)
    t1 = time.time()
    print(f"FSL: exit {p.code} ({t1 - t0:.1f} s, cache-served if fast)")

    def show(name: str, text: str) -> None:
        lines = text.splitlines()
        if a.quiet and len(lines) > 12:
            lines = [*lines[:6], f"... ({len(lines) - 12} lines)", *lines[-6:]]
        print(f"--- {name} ({len(text.splitlines())} lines)")
        for ln in lines:
            print("  " + ln)

    show("FSL stdout", p.stdout)
    show("FSL stderr", p.stderr)
    if a.fsl_only:
        return
    t0 = time.time()
    q = _run_original(args, fb, settings, a.mode)
    t1 = time.time()
    print(f"original: exit {q.code} ({t1 - t0:.1f} s)")
    show("original stdout", q.stdout)
    show("original stderr", q.stderr)
    agreement(fa, fb)


if __name__ == "__main__":
    main()
