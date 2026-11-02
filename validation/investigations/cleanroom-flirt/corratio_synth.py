# SPDX-License-Identifier: Apache-2.0
"""The correlation ratio on synthetic images: FSL 6.0.7.23's printed cost of the identity
(`-noresample`, smoothing 0, one scale) against the clean-room original's, for reference images
whose bins hold known numbers of points; which bin counts and populations agree. A development
tool of the clean-room original (part 1, 2026-11-02): the findings' "The costs".

    PYTHONPATH=python:validation/src LARMORX_FLIRT_BIN=target/release/larmorx \\
        python -P validation/investigations/cleanroom-flirt/corratio_synth.py <work dir>
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

SCHEDULE = """setscale 2 force
setoption smoothing 0
setrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
clear U
measurecost 6 UA 0 0 0 0 0 0 rel
print U
"""


def save(path: Path, data: np.ndarray) -> None:
    aff = np.diag([-2.0, 2.0, 2.0, 1.0])
    img = nib.Nifti1Image(data.astype(np.float32), aff)
    img.set_qform(aff, 1)
    img.set_sform(aff, 1)
    nib.save(img, str(path))


def cost(work: Path, ref: np.ndarray, inp: np.ndarray, bins: int, extra=()) -> tuple[str, str]:
    with tempfile.TemporaryDirectory(dir=work) as fd_, tempfile.TemporaryDirectory(dir=work) as od_:
        out = []
        for d in (Path(fd_), Path(od_)):
            save(d / "r.nii", ref)
            save(d / "i.nii", inp)
            (d / "s.sch").write_text(SCHEDULE)
        args = ["-in", "i.nii", "-ref", "r.nii", "-noresample", "-noclamp", "-schedule", "s.sch",
                "-bins", str(bins), "-omat", "o.mat", *extra]  # fmt: skip
        f = rep.run_oracle("flirt", args, cwd=Path(fd_), settings={})
        o = orig.run_original("flirt", args, cwd=Path(od_), settings={}, mode=defects.REPRODUCE)
        for p in (f, o):
            out.append(p.stdout.split()[0] if p.stdout.split() else p.stderr[-80:])
        return out[0], out[1]


def main() -> None:
    work = Path(sys.argv[1])
    work.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(1)
    shape = (24, 24, 24)
    # a reference of smooth values over [0, 100] and an input of a nonlinear function plus noise
    x = np.indices(shape).sum(0).astype(np.float64)
    ref = 100 * x / x.max()
    inp = np.sqrt(ref + 1) * 10 + rng.normal(0, 3, shape)
    for bins in (4, 8, 16, 32, 64, 128, 256):
        print("smooth ramp", bins, *cost(work, ref, inp, bins))
    # a reference with a heavy bin 0 (half the voxels 0) as a masked head has
    ref2 = ref.copy()
    ref2[:, :, :12] = 0
    for bins in (8, 16, 32, 64):
        print("ramp + zeros", bins, *cost(work, ref2, inp, bins))
    # random integer levels: few distinct values
    ref3 = rng.integers(0, 10, shape).astype(np.float64)
    inp3 = ref3 * 3 + rng.normal(0, 2, shape)
    for bins in (8, 16, 32, 64):
        print("10 levels", bins, *cost(work, ref3, inp3, bins))


if __name__ == "__main__":
    main()
