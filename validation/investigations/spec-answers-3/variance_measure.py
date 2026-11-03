# SPDX-License-Identifier: Apache-2.0
"""Rule D's measurement for FAST's single-channel class variance (``specs/fast.md`` §4.4, §9.2):
how far a more accurate variance moves FAST's outputs, against how far a last-bit change moves
them (FSL's own band). Spec answers for topup, FAST and FLIRT, relay 2 (2026-11-03).

Per case, four runs: FSL 6.0.7.23's ``fast`` as built and with correctly rounded ``exp``,
``log``, ``expf``, ``logf`` (the CORE-MATH preload: the band), both through the oracle cache;
the FAST replica (bit-identical to FSL) built with the measurement hook below and run with
``LXNC_FAST_VARIANCE=double`` (the mean's square from the double-precision mean, the rest as
FSL) and ``=twopass`` (the weighted variance about the double-precision mean, summed in
double), in ``reproduce`` mode. Each run's images against FSL's: the label images' smallest
per-class Dice and identical voxels inside FSL's mask, the partial volumes' mean, 99.9th
percentile and largest absolute difference; with ``-v`` the printed class statistics.

    LARMORX_NC_BIN=<the patched larmorx-nc> PYTHONPATH=python:validation/src \\
        boxed.sh 12 python -P validation/investigations/spec-answers-3/variance_measure.py \\
        <out.json> [case ...]

The patched binary (the patch is FSL-licensed replica code, so it lives in the replica crate:
``crates-nc/larmorx-nc-fast/dev-patches/variance_variant.patch``; a clean-room implementer does
not open it): apply it in a checkout, build ``larmorx-nc-cli``
(release), copy the binary out, revert the patch and rebuild (the hook never enters the
crate's source).
"""

from __future__ import annotations

import json
import os
import re
import shutil
import sys
import tempfile
from pathlib import Path

import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_fast_replica as rep

CASES = [
    "fmriprep/ds000005-sub-01-noreconall",
    "fmriprep/ds000228-sub-pixar008-noreconall",
    "fmriprep/ds005003-sub-RS001-noreconall",
    "real/fmriprep-call-half",
    "opt/bias-outputs",
    "opt/2d",
    "opt/big",
    "verbose/half",
]
VARIANTS = ("double", "twopass")


def _preload() -> str:
    from larmorx_validation.parity.fsl_mcflirt_replica import workspace

    ws = workspace()
    assert ws is not None
    return str(ws / "oracles" / "core-math-preload" / "libcrmath.so")


def _load(p: Path) -> np.ndarray:
    import nibabel as nib

    return np.asarray(nib.load(str(p)).dataobj)


def _prepare_all(case, dirs: list[Path]) -> list[str]:
    """The case's inputs copied into every run directory; its arguments."""
    args = rep._prepare(case, dirs[0])
    for d in dirs[1:]:
        rep._prepare(case, d)
    return args


def _setup(name: str, dirs: list[Path]) -> tuple[list[str], dict, str]:
    if name.startswith("fmriprep/"):
        run = name.split("/", 1)[1]
        node = next(n for r, n in rep.fmriprep_calls() if r == run)
        src = rep.fmriprep_input(run, str(node))
        args = rep.fmriprep_args(node)
        for d in dirs:
            shutil.copy(src, d / args[-1])
        return args, {}, ""
    if name == "verbose/half":
        # fMRIPrep's call on the half-resolution T1w, with -v: the printed class statistics
        case = rep._all_cases()["real/fmriprep-call-half"][2]
        args = _prepare_all(case, dirs)
        return ["-v", *args], dict(case.settings), case.never
    case = rep._all_cases()[name][2]
    args = _prepare_all(case, dirs)
    return args, dict(case.settings), case.never


def compare(a: Path, b: Path) -> dict:
    """``b``'s images against ``a``'s (FSL's) inside ``a``'s mask."""
    seg = next(a.glob("*_seg.nii*"), None)
    mask = (_load(seg) > 0) if seg is not None else None
    out: dict = {}
    for fa in sorted(a.glob("*.nii*")):
        fb = b / fa.name
        if not fb.is_file():
            out[fa.name] = "missing"
            continue
        x, y = _load(fa), _load(fb)
        m = mask if mask is not None and mask.shape == x.shape else np.ones(x.shape, bool)
        key = re.sub(r"\.nii(\.gz)?$", "", fa.name)
        if x.dtype.kind in "iu":
            dice = []
            for c in sorted(set(np.unique(x[m])) | set(np.unique(y[m]))):
                xa, ya = x[m] == c, y[m] == c
                s = int(xa.sum() + ya.sum())
                if s:
                    dice.append(2 * int((xa & ya).sum()) / s)
            out[key] = {
                "identical": float(np.mean(x[m] == y[m])),
                "voxels_differ": int(np.sum(x[m] != y[m])),
                "min_dice": min(dice) if dice else 1.0,
            }
        elif "_pve_" in fa.name or "_prob_" in fa.name:
            d = np.abs(x[m].astype(np.float64) - y[m].astype(np.float64))
            out[key] = {
                "mean": float(d.mean()),
                "p999": float(np.percentile(d, 99.9)),
                "max": float(d.max()),
                "voxels_differ": int(np.sum(d > 0)),
            }
        else:
            xs, ys = x[m].astype(np.float64), y[m].astype(np.float64)
            rel = np.abs(xs - ys) / np.maximum(np.abs(xs), 1e-30)
            out[key] = {"rel_p999": float(np.percentile(rel, 99.9)), "rel_max": float(rel.max())}
    return out


def _class_lines(text: str) -> list[list[float]]:
    """The last ``-v`` class line's (mean, stddev) pairs."""
    rows = [ln for ln in text.splitlines() if ln.startswith(" CLASS ")]
    if not rows:
        return []
    nums = re.findall(r"MEAN (\S+) STDDEV (\S+)", rows[-1])
    return [[float(m), float(s)] for m, s in nums]


def main() -> None:
    out_json = Path(sys.argv[1])
    names = sys.argv[2:] or CASES
    assert os.environ.get("LARMORX_NC_BIN"), "LARMORX_NC_BIN: the patched replica"
    result: dict = {}
    for name in names:
        with tempfile.TemporaryDirectory(prefix="lx-fast-var_") as t_:
            t = Path(t_)
            dirs = [t / k for k in ("fsl", "fsl-cr", *VARIANTS)]
            for d in dirs:
                d.mkdir()
            args, settings, never = _setup(name, dirs)
            p = rep.run_oracle(args, cwd=dirs[0], settings=settings, never=never)
            q = rep.run_oracle(
                args, cwd=dirs[1], settings={**settings, "LD_PRELOAD": _preload()}, never=never
            )
            row: dict = {
                "args": " ".join(args),
                "exit": [p.code, q.code],
                "band": compare(dirs[0], dirs[1]),
            }
            stats = {"fsl": _class_lines(p.stdout), "fsl-cr": _class_lines(q.stdout)}
            for v, d in zip(VARIANTS, dirs[2:], strict=True):
                r = rep.run_replica(
                    rep.PROGRAM,
                    args,
                    cwd=d,
                    settings={**settings, "LXNC_FAST_VARIANCE": v},
                    mode=defects.REPRODUCE,
                    threads=1,
                )
                row["exit"].append(r.code)
                row[v] = compare(dirs[0], d)
                stats[v] = _class_lines(r.stdout)
            if any(stats.values()):
                row["class_stats"] = stats
            result[name] = row
            print(f"== {name}: exit {row['exit']}", flush=True)
            for k in ("band", *VARIANTS):
                labels = {
                    f: v for f, v in row[k].items() if isinstance(v, dict) and "min_dice" in v
                }
                print(
                    f"   {k:8s} "
                    + "; ".join(
                        f"{f.split('_', 1)[-1]} Dice {v['min_dice']:.5f} ({v['voxels_differ']} vox)"
                        for f, v in labels.items()
                    ),
                    flush=True,
                )
    out_json.write_text(json.dumps(result, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
