# SPDX-License-Identifier: Apache-2.0
"""ANTS-116: antsAI's blob alignment compares two blobs' patches after turning the moving one
with Wahba's ``A = V·Uᵀ`` of the patches' principal axes, whose signs are arbitrary, without
checking ``det A``: identical patches can be compared reflected or turned by 180°. larmorx's
default tries each sign combination of the moving axes with Kabsch's proper rotation and keeps
the best correlation; ``upstream_defects="reproduce"`` is ANTs' method, bit for bit.

For fMRIPrep's preprocessed T1w of ds000005 against (a) its copy with the origin moved (the
same voxels: every blob has an identical partner) and (b) its copy rotated by 8° and shifted,
and for the ``ants-ai`` suite's seeded blob phantoms (3D and 2D; a translation, a rigid
transform), this reports for each mode (larmorx's default and ANTs' method):

- the identical blob pairs (same voxel, case a) and how many correlate at 1 (≥ 0.999);
- the pairs matched, how many are true (the moving blob within 2 voxels of where the known
  transform sends the fixed one), the landmarks and how many of them are true;
- the transform's distance from the known one (mean and largest, at the suite's test points).

Usage (from the larmorx checkout, larmorx built into ``python/``; the oracle run
``ds000005-sub-01-noreconall`` for the T1w cases)::

    PYTHONPATH=python:validation/src python -P validation/investigations/ants-116-blob-orientation/measure.py \\
        --out <scratch dir> [--threads 6]
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
import warnings
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.parity import ants_ai_blobs as aib
from larmorx_validation.parity import ants_inputs, blob_phantoms

CASES = [
    # (name, fixed input, moving input, truth key, blobs)
    ("T1w vs its origin-shifted copy", None, "ai-blobs-t1w-shift", "t1w:shift", 30),
    ("T1w vs its rotated (8°) and shifted copy", None, "ai-blobs-t1w-rotated", "t1w:rotated", 30),
]


def _phantom_cases() -> list[tuple]:
    out = []
    for move, (name, kind, *_rest) in blob_phantoms.MOVES.items():
        if kind in ("translation", "rigid") or move.endswith("shift") or move.endswith("rigid"):
            out.append(
                (
                    f"phantom {name}, {kind} ({move})",
                    f"ai-blobs-fixed-{name}",
                    f"ai-blobs-moving-{move}",
                    f"phantom:{move}",
                    None,
                )
            )
    return out


def lps_to_ras(p: np.ndarray) -> np.ndarray:
    q = np.array(p, np.float64).copy()
    q[:, :2] *= -1
    return q


def analyse(name, fixed, moving, truth, blobs, threads) -> dict:
    import larmorx as lx

    rec: dict = {"case": name}
    fimg = nib.load(fixed)
    for mode in ("fix", "reproduce"):
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            try:
                kw = {} if blobs is None else {"blobs": blobs}
                a = lx.ants.blob_alignment(
                    str(fixed), str(moving), n_threads=threads, upstream_defects=mode, **kw
                )
            except Exception as e:  # noqa: BLE001: the outcome is the finding
                rec[mode] = {"error": f"{type(e).__name__}: {str(e)[:200]}"}
                continue
        named = sorted({getattr(x.message, "entry", "") for x in w} - {""})
        r: dict = {"named": named, "pairs": len(a.pairs), "landmarks": len(a.landmarks)}
        c = np.asarray(a.correspondence)
        # Identical pairs: the same voxel in both images (the origin-shifted copy).
        if truth in ("t1w:shift", "phantom:shift", "phantom:2d-shift"):
            same = [
                (i, j)
                for i, fb in enumerate(a.fixed_blobs)
                for j, mb in enumerate(a.moving_blobs)
                if tuple(fb.index) == tuple(mb.index)
            ]
            r["identical_pairs"] = len(same)
            r["identical_at_1"] = int(sum(c[i, j] >= 0.999 for i, j in same))
            r["identical_correlations"] = [round(float(c[i, j]), 4) for i, j in same]
        # True pairs: the moving blob where the known transform sends the fixed one.
        pts = aib.truth_points(truth, Path(fixed))
        if pts is not None and a.pairs:
            fpos = np.array([lx_point(fimg, a.fixed_blobs[i].index) for i, _ in a.pairs])
            mimg = nib.load(moving)
            mpos = np.array([lx_point(mimg, a.moving_blobs[j].index) for _, j in a.pairs])
            sent = true_map(truth, fpos, Path(fixed))
            tol = 2.0 * float(np.max(fimg.header.get_zooms()[: fpos.shape[1]]))
            ok = np.linalg.norm(sent - mpos, axis=1) <= tol
            r["true_pairs"] = int(ok.sum())
            r["true_landmarks"] = int(sum(ok[k] for k in a.landmarks))
        # The transform's distance from the known one.
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "t.mat"
            if len(a.landmarks) and np.all(np.isfinite(a.transform.parameters)):
                lx.transforms.write(f, [a.transform])
                err = aib.transform_error(f, truth, Path(fixed))
                r["error_mm"] = None if err is None else [round(err[0], 3), round(err[1], 3)]
            else:
                r["error_mm"] = None
        rec[mode] = r
    return rec


def lx_point(img: nib.Nifti1Image, index) -> np.ndarray:
    """A voxel's centre in RAS+ mm."""
    idx = np.zeros(3)
    idx[: len(index)] = index
    p = nib.affines.apply_affine(img.affine, idx)
    return p[: len(index)]


def true_map(truth: str, pts: np.ndarray, fixed: Path) -> np.ndarray:
    """Where the known transform sends fixed points (RAS+ mm)."""
    if truth.startswith("phantom:"):
        return blob_phantoms.true_map(truth[8:], pts)
    # The T1w cases: interpolate the suite's 27 test points' map (exact for these transforms,
    # which are affine) by a least-squares affine fit.
    fx, mv = aib.truth_points(truth, fixed)
    a = np.c_[fx, np.ones(len(fx))]
    m, *_ = np.linalg.lstsq(a, mv, rcond=None)
    return np.c_[pts, np.ones(len(pts))] @ m


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--threads", type=int, default=6)
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    results = []
    t1w = aib._oracle_t1w()
    for name, fixed, moving, truth, blobs in CASES + _phantom_cases():
        f = t1w if fixed is None else ants_inputs.path(fixed)
        m = ants_inputs.path(moving)
        rec = analyse(name, f, m, truth, blobs, args.threads)
        print(json.dumps(rec), flush=True)
        results.append(rec)
    (args.out / "results.json").write_text(json.dumps(results, indent=1), encoding="utf-8")
    print("| Case | mode | identical pairs at 1 | true pairs / pairs | true landmarks / landmarks | error (mean, max mm) |")
    print("|---|---|---|---|---|---|")
    for r in results:
        for mode in ("reproduce", "fix"):
            x = r.get(mode, {})
            if "error" in x:
                print(f"| {r['case']} | {mode} | {x['error']} | | | |")
                continue
            ident = (
                f"{x['identical_at_1']} of {x['identical_pairs']}" if "identical_pairs" in x else "–"
            )
            print(
                f"| {r['case']} | {mode} | {ident} | {x.get('true_pairs', '–')} / {x['pairs']} | "
                f"{x.get('true_landmarks', '–')} / {x['landmarks']} | {x.get('error_mm')} |"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
