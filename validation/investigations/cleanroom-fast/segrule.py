# SPDX-License-Identifier: Apache-2.0
"""Which rule gives FSL's hard segmentation (``_seg``) from its posteriors: FSL 6.0.7.23's
``fast`` with correctly rounded maths (the CORE-MATH preload, through the oracle cache) and the
clean-room original (``reproduce``) on one command line with ``-p``; prints which outputs are
bit-identical, and at the voxels where ``_seg`` differs, FSL's and the original's labels and
probabilities. A development tool of FAST's clean-room original (relay 1, 2026-11-03).

    PYTHONPATH=python:validation/src python -P validation/investigations/cleanroom-fast/segrule.py \
        "-p -o seg @t1_2d.nii.gz"

Run it inside scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import shlex
import sys
import tempfile
from pathlib import Path

import numpy as np
from larmorx_validation.parity import fsl_fast as orig
from larmorx_validation.parity import fsl_fast_replica as rep

sys.path.insert(0, str(Path(__file__).parent))
from band import preload


def load(p: Path) -> np.ndarray:
    import nibabel as nib

    return np.asarray(nib.load(str(p)).dataobj)


def main() -> None:
    run = rep.Run(tuple(shlex.split(sys.argv[1])))
    with (
        tempfile.TemporaryDirectory(prefix="lx-fast-seg_") as a_,
        tempfile.TemporaryDirectory(prefix="lx-fast-seg_") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = rep._prepare(run, a)
        rep._prepare(run, b)
        rep.run_oracle(args, cwd=a, settings={"LD_PRELOAD": preload()}, never=rep.UNWRITTEN_PROB)
        orig.run_original(args, cwd=b, settings={}, mode="reproduce")
        for f in sorted(a.glob("seg_*")):
            x, y = load(f), load(b / f.name)
            print(f"{f.name}: identical {np.array_equal(x, y)} ({np.mean(x == y):.6f})")
        sa, sb = load(a / "seg_seg.nii.gz"), load(b / "seg_seg.nii.gz")
        n = len(sorted(a.glob("seg_prob_*")))
        pa = np.stack([load(a / f"seg_prob_{i}.nii.gz") for i in range(n)], -1)
        pb = np.stack([load(b / f"seg_prob_{i}.nii.gz") for i in range(n)], -1)
        for idx in np.argwhere(sa != sb)[:12]:
            t = tuple(idx)
            print(f"  {t}: FSL {sa[t]} original {sb[t]}  FSL p {pa[t]}  original p {pb[t]}")


if __name__ == "__main__":
    main()
