# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Black-box probes of the clean-room catch-up (2026-11-04): FSL's runs through the oracle cache
(use mode) on inputs derived from the replica suite's half-resolution pair. Usage:

  catchup_probes.py final-single <dir>   topup --splineorder=2 on 6 slices whose last level is
                                         subsampled: the outputs' refit has one coefficient along z
  catchup_probes.py general-count <dir> <coef dir>
                                         applytopup --method=jac on a coefficient file of the
                                         general quadratic count on a short axis (the original's
                                         default output, <coef dir>/o_fieldcoef.nii.gz and
                                         o_movpar.txt), as FSL reads it

Each writes ARGS.txt, STDOUT.txt and STDERR.txt next to the outputs (mine_debug.py or mine.py
then run the original on the same inputs).
"""

import shutil
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.parity import fsl_topup_replica as T


def half_pair(d: Path) -> None:
    """The suite's half-resolution pair and its acquisition file, in `d`."""
    run = T.Run(("--imain=@half.nii.gz", "--datain=@acq.txt"))
    T._prepare(run, d, T._source_node())


def final_single(d: Path) -> None:
    d.mkdir(parents=True, exist_ok=True)
    half_pair(d)
    img = nib.load(d / "half.nii.gz")
    data = np.asarray(img.dataobj)[:, :, 7:13, :]
    out = nib.Nifti1Image(data, img.affine, img.header)
    nib.save(out, d / "crop6.nii.gz")
    args = [
        "--imain=crop6.nii.gz",
        "--datain=acq.txt",
        "--out=o",
        "--warpres=44",
        "--subsamp=2",
        "--fwhm=8",
        "--miter=2",
        "--lambda=0.005",
        "--estmov=0",
        "--splineorder=2",
        "--fout=f",
        "-v",
    ]
    finish(d, T.Run(tuple(args)), args)


def general_count(d: Path, coef: Path) -> None:
    d.mkdir(parents=True, exist_ok=True)
    half_pair(d)
    shutil.copy(coef / "o_fieldcoef.nii.gz", d / "tu_fieldcoef.nii.gz")
    shutil.copy(coef / "o_movpar.txt", d / "tu_movpar.txt")
    args = [
        "--imain=half.nii.gz",
        "--inindex=1",
        "--datain=acq.txt",
        "--topup=tu",
        "--out=hifi",
        "--method=jac",
    ]
    finish(d, T.Run(tuple(args), program="applytopup"), args)


def finish(d: Path, run, args) -> None:
    p = T._oracle(run, list(args), d, {})
    (d / "ARGS.txt").write_text(" ".join(args) + "\n")
    (d / "STDOUT.txt").write_text(p.stdout)
    (d / "STDERR.txt").write_text(p.stderr)
    print("exit", p.code)
    print(p.stdout[-800:])
    print(p.stderr[-800:])


if __name__ == "__main__":
    kind = sys.argv[1]
    if kind == "final-single":
        final_single(Path(sys.argv[2]))
    elif kind == "general-count":
        general_count(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        raise SystemExit(__doc__)
