# SPDX-License-Identifier: Apache-2.0
"""ANTS-40 on phantoms with a known bias field: does leaving N4's zero voxels out of the fit
recover the field better?

A head-like phantom (WM, GM, CSF in ellipsoids, 1.5 mm voxels) times a known smooth bias field
(±25 %), with Rician noise, in three versions: (a) its background exactly 0 (a skull-stripped
or zero-filled image, as N4 meets in fMRIPrep's BOLD references and field-map magnitudes);
(b) the same with 2 % of the head's voxels also 0 (signal dropout); (c) a noisy background
(no zero voxel: the control, where the two runs must agree). N4 runs with fMRIPrep's first
anatomical call (``-s 4 -b [200] -c [50x50x50x50,1e-7]``), as ANTs does
(``LARMORX_UPSTREAM_DEFECTS=reproduce``) and with the zero voxels left out (a mask of the
positive voxels). Reported, inside the head: the field's shape error (the standard deviation
of log(estimated / true), its mean removed: N4's field has an arbitrary scale) and the white
matter's coefficient of variation after correction (lower is better for both).

Usage (from the larmorx checkout, larmorx built into ``python/``)::

    PYTHONPATH=python:validation/src python -P validation/investigations/ants-40-n4-nonpositive/phantom.py --out <dir>
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import os
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

SHAPE = (96, 112, 96)
SPACING = 1.5


def phantom(kind: str, seed: int = 7) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """The image, the true bias field and the white-matter mask."""
    rng = np.random.default_rng(seed)
    i, j, k = np.indices(SHAPE, dtype=np.float64)
    c = (np.array(SHAPE) - 1) / 2
    r = np.sqrt(((i - c[0]) / 38) ** 2 + ((j - c[1]) / 46) ** 2 + ((k - c[2]) / 36) ** 2)
    tissue = np.where(r < 1.0, 40.0, 0.0)  # CSF
    tissue = np.where(r < 0.93, 80.0, tissue)  # GM
    wm = r < 0.7 + 0.06 * np.sin(i / 5) * np.cos(j / 7)
    tissue = np.where(wm, 120.0, tissue)
    x, y, z = (i - c[0]) / SHAPE[0], (j - c[1]) / SHAPE[1], (k - c[2]) / SHAPE[2]
    log_bias = 0.35 * x - 0.25 * y + 0.2 * z + 0.3 * x * y - 0.25 * z * z
    bias = np.exp(log_bias - log_bias[r < 1.0].mean())
    sigma = 3.0
    signal = tissue * bias
    noisy = np.abs(signal + rng.normal(0, sigma, SHAPE) + 1j * rng.normal(0, sigma, SHAPE))
    head = r < 1.0
    if kind in ("zero-background", "zero-dropout"):
        noisy = np.where(head, noisy, 0.0)
    if kind == "zero-dropout":
        drop = head & (rng.random(SHAPE) < 0.02)
        noisy = np.where(drop, 0.0, noisy)
    return noisy.astype(np.float32), bias, wm & head


def run_n4(argv: list[str]) -> None:
    from larmorx.cli import run

    os.environ["LARMORX_UPSTREAM_DEFECTS"] = "reproduce"
    err = io.StringIO()
    with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
        code = run(["larmorx", "ants", "N4BiasFieldCorrection", *argv])
    if code != 0:
        raise RuntimeError(err.getvalue())


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    affine = np.diag([SPACING, SPACING, SPACING, 1.0])
    results = []
    for kind in ("zero-background", "zero-dropout", "noisy-background"):
        img, bias, wm = phantom(kind)
        f = args.out / f"{kind}.nii.gz"
        nib.Nifti1Image(img, affine).to_filename(f)
        m = args.out / f"{kind}-positive.nii.gz"
        nib.Nifti1Image((img > 0).astype(np.uint8), affine).to_filename(m)
        head = img > 0
        rec: dict = {
            "phantom": kind,
            "voxels": int(img.size),
            "zero_voxels": int((img == 0).sum()),
        }
        base = ["-d", "3", "-i", str(f), "-s", "4", "-b", "[200]", "-c", "[50x50x50x50,1e-7]"]
        for label, extra in (("ants", []), ("excluded", ["-x", str(m)])):
            c, b = args.out / f"{kind}-{label}-c.nii.gz", args.out / f"{kind}-{label}-b.nii.gz"
            run_n4([*base, *extra, "-o", f"[{c},{b}]"])
            est = np.asarray(nib.load(b).dataobj, np.float64)
            corrected = img / est
            e = np.log(est[head]) - np.log(bias[head])
            rec[label] = {
                "field_shape_error": float(np.std(e - e.mean())),
                "wm_cv": float(corrected[wm].std() / corrected[wm].mean()),
                "uncorrected_wm_cv": float(img[wm].std() / img[wm].mean()),
            }
        print(json.dumps(rec), flush=True)
        results.append(rec)
    (args.out / "phantom.json").write_text(json.dumps(results, indent=1), encoding="utf-8")
    print(
        "| Phantom | zero voxels | field shape error: ANTs, excluded | WM CV: input, ANTs, excluded |"
    )
    print("|---|---|---|---|")
    for r in results:
        a, x = r["ants"], r["excluded"]
        print(
            f"| {r['phantom']} | {r['zero_voxels']} | {a['field_shape_error']:.4f}, "
            f"{x['field_shape_error']:.4f} | {a['uncorrected_wm_cv']:.4f}, {a['wm_cv']:.4f}, "
            f"{x['wm_cv']:.4f} |"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
