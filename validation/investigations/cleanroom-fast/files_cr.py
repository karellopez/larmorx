# SPDX-License-Identifier: Apache-2.0
"""FSL 6.0.7.23's ``fast`` with correctly rounded maths (the CORE-MATH preload, through the
oracle cache) against the clean-room original (``reproduce``) on command lines given: for each
image both write, the fraction of voxels with identical values and the largest relative
difference; the two directories are kept under ``<out>/fsl`` and ``<out>/original``. A
development tool of FAST's clean-room original (relay 1, 2026-11-03).

    PYTHONPATH=python:validation/src python -P validation/investigations/cleanroom-fast/files_cr.py \
        <out> "-b -I 0 -O 0 -W 1 -o seg @t1_half.nii.gz"

Run it inside scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import shlex
import shutil
import sys
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
    out = Path(sys.argv[1])
    for line in sys.argv[2:]:
        shutil.rmtree(out, ignore_errors=True)
        a, b = out / "fsl", out / "original"
        a.mkdir(parents=True)
        b.mkdir(parents=True)
        run = rep.Run(tuple(shlex.split(line)))
        args = rep._prepare(run, a)
        rep._prepare(run, b)
        never = rep.UNWRITTEN_PROB if "-p" in args else ""
        rep.run_oracle(args, cwd=a, settings={"LD_PRELOAD": preload()}, never=never)
        orig.run_original(args, cwd=b, settings={}, mode="reproduce")
        print(f"== {line}")
        for f in sorted(a.iterdir()):
            if not (b / f.name).is_file() or f.name.endswith(".img"):
                continue
            x, y = load(f).astype(np.float64), load(b / f.name).astype(np.float64)
            same = (x == y) | (np.isnan(x) & np.isnan(y))
            rel = np.abs(x - y) / np.maximum(np.abs(x), 1e-30)
            print(f"   {f.name}: identical {same.mean():.6f}, max rel {np.nanmax(rel):.3g}")


if __name__ == "__main__":
    main()
