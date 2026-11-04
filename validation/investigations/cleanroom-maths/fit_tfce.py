# SPDX-License-Identifier: Apache-2.0
"""TFCE's arithmetic and `-tfceS`'s output (findings 17): numpy models of §14 against FSL's
outputs of the replica suite's `tfce/` and `tfceS/` cases, kept by
``fsl_outputs.py <dir> tfce/basic tfce/conn18 tfce/minT tfce/delta tfce/negative-minT
d13/tfce-conn7 tfceS/basic tfceS/conn26 tfceS/not-reached tfceS/delta tfceS/low-voxel``.

Run: ``python -P fit_tfce.py <dir>``. Prints, per case, the voxels that differ with ``E`` held
in double and in single precision (``-tfce``), and the voxels of FSL's and the model's output
(``-tfceS``: the voxel's cluster at the threshold reached, or at ``h0`` when never reached; the
voxels outside every cluster when no threshold is taken).
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np

F32 = np.float32
TFCE = {
    "tfce_basic": (2, 0.5, 6, 0.0, 0.0),
    "tfce_conn18": (2, 0.5, 18, 0.0, 0.0),
    "tfce_minT": (2, 0.5, 6, 1.0, 0.0),
    "tfce_delta": (1.5, 0.66, 26, 0.0, 0.3),
    "tfce_negative-minT": (0.5, 0.5, 6, -1.0, 0.0),
    "d13_tfce-conn7": (2, 0.5, 7, 0.0, 0.0),
}
SUPPORT = {
    "tfceS_basic": (2, 0.5, 6, (3, 3, 3), 30, 0.0),
    "tfceS_conn26": (2, 0.5, 26, (7, 6, 5), 20, 0.0),
    "tfceS_not-reached": (2, 0.5, 6, (3, 3, 3), 1e9, 0.0),
    "tfceS_delta": (2, 0.5, 6, (3, 3, 3), 15, 0.35),
    "tfceS_low-voxel": (2, 0.5, 6, (0, 8, 0), 5, 0.0),
}


def structure(c: int):
    from scipy import ndimage

    return ndimage.generate_binary_structure(3, 1 if c < 18 else (2 if c < 26 else 3))


def tfce(v, h_exp, e_exp, c, min_t, delta, single_e):
    from scipy import ndimage

    m = v.ravel()[0]
    for x in v.ravel()[1:]:
        m = x if x > m else m
    d = F32(delta) if delta != 0 else F32(np.float64(m) / 100.0)
    lim, h = F32(m + d), F32(min_t)
    enh = np.zeros(v.shape, F32)
    interior = np.zeros(v.shape, bool)
    interior[1:-1, 1:-1, 1:-1] = True
    e = float(F32(e_exp)) if single_e else e_exp
    while h < lim:
        lab, n = ndimage.label((v > h) & interior, structure=structure(c))
        sizes = np.bincount(lab.ravel())
        with np.errstate(invalid="ignore"):
            hp = F32(np.power(np.float64(h), np.float64(F32(h_exp))))
        for k in range(1, n + 1):
            sel = lab == k
            enh[sel] = (
                enh[sel].astype(np.float64) + math.pow(float(sizes[k]), e) * float(hp)
            ).astype(F32)
        h = F32(h + d)
    return enh


def support(v, h_exp, e_exp, c, at, target, delta):
    from scipy import ndimage

    m = v[at]
    d = F32(delta) if delta != 0 else F32(np.float64(m) / 100.0)
    h, hs, masks = F32(0.0), [], []
    while h < m:
        lab, _ = ndimage.label((v >= h) & (v <= np.finfo(F32).max), structure=structure(c))
        hs.append(h)
        masks.append(lab == lab[at])
        h = F32(h + d)
    acc = F32(0)
    for k in range(len(hs) - 1, -1, -1):
        acc = F32(
            acc + F32(np.power(F32(masks[k].sum()), F32(e_exp))) * F32(np.power(hs[k], F32(h_exp)))
        )
        if acc >= target:
            return np.where(masks[k], v, 0).astype(F32)
    if masks:
        return np.where(masks[0], v, 0).astype(F32)
    return np.where(~(v >= 0), v, 0).astype(F32)


def main(argv: list[str]) -> int:
    import nibabel as nib

    base = Path(argv[0])
    for case, (h_exp, e_exp, c, min_t, delta) in TFCE.items():
        if not (base / case).exists():
            print(f"{case}: not made")
            continue
        v = np.asarray(nib.load(str(base / case / "tfz.nii.gz")).dataobj).astype(F32)
        out = np.asarray(nib.load(str(base / case / "out.nii.gz")).dataobj)
        diff = {}
        for single_e in (False, True):
            r = tfce(v, h_exp, e_exp, c, min_t, delta, single_e)
            diff["E single" if single_e else "E double"] = int(
                np.count_nonzero(r.view(np.int32) != out.view(np.int32))
            )
        print(f"{case}: voxels that differ of {v.size}: {diff}")
    for case, (h_exp, e_exp, c, at, target, delta) in SUPPORT.items():
        if not (base / case).exists():
            print(f"{case}: not made")
            continue
        v = np.asarray(nib.load(str(base / case / "tfz.nii.gz")).dataobj).astype(F32)
        out = np.asarray(nib.load(str(base / case / "out.nii.gz")).dataobj)
        r = support(v, h_exp, e_exp, c, at, target, delta)
        same = bool(np.array_equal(r.view(np.int32), out.view(np.int32)))
        print(
            f"{case}: FSL keeps {np.count_nonzero(out)} voxels, the model {np.count_nonzero(r)}; identical: {same}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
