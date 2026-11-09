# SPDX-License-Identifier: Apache-2.0
"""Rule D's measurement for SciPy's watershed defects (SCI-13 and SCI-14, relay 15's candidates
8 and 9) on sdcflows' ``epi_mask``: are the corrected masks better?

For the BOLD references (``desc-hmc_boldref``) of the four oracle runs ``check_epi_mask.py``
uses, both masks (``upstream_defects="reproduce"``: sdcflows' and SciPy's, bit for bit;
``"fix"``: the corrected watershed) are compared with two independent masks on the same grid:

- fMRIPrep's own BOLD brain mask of the run (``desc-brain_mask``, the reference's grid; made by
  niworkflows' skull stripping, ``init_skullstrip_bold_wf``, on the coregistration reference,
  which for a PEPOLAR run (ds005040) is the corrected one): the Dice coefficient, the fraction of
  fMRIPrep's mask covered, and the mask's voxels outside it;
- a head mask of the reference (its voxels above 5 % of its 98th percentile, holes filled slice
  by slice and in 3-D, the largest connected component): the mask's voxels outside the head.

Usage (the newest environment, ``OPENBLAS_CORETYPE=Haswell``; about a minute)::

    python -P measure_watershed_masks.py [--json out.json]

Prints one line per run and mode and a verdict line (better or worse: a higher or lower Dice
coefficient with fMRIPrep's mask; the voxels outside the head are printed beside it); the
numbers are in
``docs/findings/sdcflows-estimation.md`` and the register's SCI-13 and SCI-14.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import warnings
from pathlib import Path

import numpy as np

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
REAL_RUNS = (
    "ds005040-sub-002-noreconall",
    "ds000228-sub-pixar008-noreconall",
    "ds004958-sub-fsm64ps-noreconall",
    "ds003592-sub-280-noreconall",
)


def head_mask(ref: np.ndarray) -> np.ndarray:
    """The reference's voxels above 5 % of its 98th percentile, holes filled, largest component."""
    from scipy import ndimage

    m = ref > 0.05 * np.percentile(ref, 98)
    for k in range(m.shape[2]):
        m[..., k] = ndimage.binary_fill_holes(m[..., k])
    m = ndimage.binary_fill_holes(m)
    lab, n = ndimage.label(m)
    if n > 1:
        sizes = np.bincount(lab.ravel())
        sizes[0] = 0
        m = lab == int(np.argmax(sizes))
    return m


def compare(mask: np.ndarray, brain: np.ndarray, head: np.ndarray) -> dict:
    inter = int(np.count_nonzero(mask & brain))
    n, nb = int(np.count_nonzero(mask)), int(np.count_nonzero(brain))
    return {
        "voxels": n,
        "dice": round(2 * inter / (n + nb), 4) if n + nb else None,
        "covered": round(inter / nb, 4) if nb else None,
        "outside_brain": n - inter,
        "outside_head": int(np.count_nonzero(mask & ~head)),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--json", type=Path, default=None)
    args = ap.parse_args()
    import nibabel as nib

    import larmorx.sdcflows as ls

    rows = []
    for name in REAL_RUNS:
        func = RUNS / name / "out"
        refs = sorted(func.rglob("*_desc-hmc_boldref.nii.gz"))
        if not refs:
            print(f"SKIP {name}: no desc-hmc_boldref")
            continue
        ref_path = refs[0]
        brain_path = Path(str(ref_path).replace("_desc-hmc_boldref", "_desc-brain_mask"))
        ref_img, brain_img = nib.load(ref_path), nib.load(brain_path)
        assert ref_img.shape == brain_img.shape, (ref_img.shape, brain_img.shape)
        assert np.allclose(ref_img.affine, brain_img.affine, atol=1e-4), name
        ref = np.asanyarray(ref_img.dataobj, dtype=np.float32)
        brain = np.asanyarray(brain_img.dataobj) > 0
        head = head_mask(ref)
        row = {
            "run": name.split("-noreconall")[0],
            "shape": list(ref.shape),
            "fmriprep_brain": int(brain.sum()),
            "head": int(head.sum()),
            "fmriprep_brain_outside_head": int(np.count_nonzero(brain & ~head)),
        }
        for mode in ("reproduce", "fix"):
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                r = ls.epi_mask(str(ref_path), write=False, upstream_defects=mode)
            row[mode] = compare(np.asanyarray(r.mask.data) > 0, brain, head)
            print(
                f"{row['run']:22s} {mode:9s} " + " ".join(f"{k}={v}" for k, v in row[mode].items())
            )
        rows.append(row)
    better = [r["run"] for r in rows if (r["fix"]["dice"] or 0) > (r["reproduce"]["dice"] or 0)]
    worse = [r["run"] for r in rows if (r["fix"]["dice"] or 0) < (r["reproduce"]["dice"] or 0)]
    same = [r["run"] for r in rows if r["fix"] == r["reproduce"]]
    print(f"VERDICT better={better} worse={worse} same={same}")
    if args.json:
        args.json.write_text(json.dumps(rows, indent=1) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
