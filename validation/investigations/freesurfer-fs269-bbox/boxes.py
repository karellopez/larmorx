# SPDX-License-Identifier: Apache-2.0
"""FS-269 measured on the recorded heads: FreeSurfer 8.2.0's bounding box against the box that
holds every voxel from ``min - npad`` to ``max + npad``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it models FreeSurfer's box from its source).

For each recorded head (``<workspace>/oracles/freesurfer-8.2.0/runs/<head>-<stream>/``):

- ``mri_mask -bb 3 orig.mgz orig.mgz invol.crop.nii.gz`` (the v8 stream's SynthMorph step,
  ``fs-synthmorph-reg``): the box of ``orig.mgz`` with a pad of 3 on every side, FreeSurfer's size
  and the full one, and whether the crop would change;
- ``mri_binarize --bb <npad>`` of three masks (``aseg.mgz --match 2 41``, ``brainmask.mgz --min 1``,
  ``aseg.mgz --match 17``) with pads 0 to 3: the mask voxels FreeSurfer's box leaves out and the far
  side's pad on each axis.

The model: the voxels of frame 0 whose magnitude is not below 1e-5 (as ``float``); start
``max(min - npad, 0)``, FreeSurfer's size ``min(max - min + 2 npad, extent - start)``, the full size
``min(max - min + 1 + 2 npad, extent - start)``.

    scripts/orchestration/boxed.sh 8 <workspace>/.venv/bin/python -P \\
        validation/investigations/freesurfer-fs269-bbox/boxes.py [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

HEADS = (
    "ds000005-sub-01",
    "ds000228-sub-pixar008",
    "ds002422-sub-02",
    "ds004173-sub-603309",
    "ds005003-sub-RS001",
    "ds005454-sub-16",
)
MASKS = (
    ("aseg --match 2 41", "aseg.mgz", lambda v: np.isin(v, (2, 41))),
    ("brainmask --min 1", "brainmask.mgz", lambda v: v >= 1),
    ("aseg --match 17", "aseg.mgz", lambda v: v == 17),
)


def extremes(nonzero: np.ndarray) -> tuple[list[int], list[int]] | None:
    idx = np.nonzero(nonzero)
    if idx[0].size == 0:
        return None
    return [int(i.min()) for i in idx], [int(i.max()) for i in idx]


def box(lo: list[int], hi: list[int], shape: tuple[int, ...], npad: int, one: int):
    start = [max(lo[a] - npad, 0) for a in range(3)]
    size = [min(hi[a] - lo[a] + one + 2 * npad, shape[a] - start[a]) for a in range(3)]
    return start, size


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", type=Path, help="the recorded runs' folder")
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    if a.runs is None:
        sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "src"))
        from larmorx_validation.oracle_cache.store import workspace

        ws = workspace()
        if ws is None:
            raise SystemExit("no workspace with oracles/")
        runs = ws / "oracles" / "freesurfer-8.2.0" / "runs"
    else:
        runs = a.runs
    out: dict[str, list] = {"mri_mask": [], "mri_binarize": []}
    for head in HEADS:
        sub = head.split("-", 1)[1]
        # mri_mask -bb 3 orig.mgz (v8)
        orig = runs / f"{head}-v8" / "subjects" / sub / "mri" / "orig.mgz"
        v = np.asarray(nib.load(orig).dataobj)[..., 0] if nib.load(orig).ndim == 4 else None
        if v is None:
            v = np.asarray(nib.load(orig).dataobj)
        e = extremes(np.abs(v.astype(np.float32)) >= np.float32(1e-5))
        assert e is not None
        lo, hi = e
        fs = box(lo, hi, v.shape, 3, 0)
        full = box(lo, hi, v.shape, 3, 1)
        row = {
            "head": head,
            "shape": list(v.shape),
            "min": lo,
            "max": hi,
            "freesurfer": fs,
            "full": full,
            "crop_changes": fs != full,
        }
        out["mri_mask"].append(row)
        print(
            f"mri_mask -bb 3 {head}-v8: shape {v.shape} min {lo} max {hi} "
            f"FreeSurfer {fs[0]} {fs[1]} full {full[1]} changes {fs != full}"
        )
        for stream in ("classic", "v8"):
            mri = runs / f"{head}-{stream}" / "subjects" / sub / "mri"
            for name, file, sel in MASKS:
                vol = np.asarray(nib.load(mri / file).dataobj)
                m = sel(vol)
                n = int(m.sum())
                e = extremes(m)
                if e is None:
                    continue
                lo, hi = e
                for npad in range(4):
                    fs = box(lo, hi, vol.shape, npad, 0)
                    full = box(lo, hi, vol.shape, npad, 1)
                    sl = tuple(slice(fs[0][k], fs[0][k] + max(fs[1][k], 0)) for k in range(3))
                    inside = int(m[sl].sum())
                    far = [fs[0][k] + fs[1][k] - 1 - hi[k] for k in range(3)]
                    out["mri_binarize"].append(
                        {
                            "head": head,
                            "stream": stream,
                            "mask": name,
                            "npad": npad,
                            "voxels": n,
                            "left_out": n - inside,
                            "far_pad": far,
                            "freesurfer": fs,
                            "full": full,
                        }
                    )
                    print(
                        f"mri_binarize {head}-{stream} {name} --bb {npad}: {n} voxels, "
                        f"{n - inside} left out, far pad {far}, FreeSurfer size {fs[1]}, "
                        f"full {full[1]}"
                    )
    nm = sum(r["crop_changes"] for r in out["mri_mask"])
    lost = [r for r in out["mri_binarize"] if r["npad"] == 0]
    print(
        f"SUMMARY mri_mask -bb 3: the crop changes on {nm} of {len(out['mri_mask'])} heads; "
        f"mri_binarize --bb 0: {sum(r['left_out'] for r in lost)} of "
        f"{sum(r['voxels'] for r in lost)} mask voxels left out over {len(lost)} masks "
        f"({min(r['left_out'] for r in lost)} to {max(r['left_out'] for r in lost)} each)"
    )
    if a.json:
        a.json.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
