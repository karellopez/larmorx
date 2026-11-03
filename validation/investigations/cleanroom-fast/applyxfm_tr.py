# SPDX-License-Identifier: Apache-2.0
"""Which TR (``pixdim[4]``) FLIRT's ``-applyxfm`` writes: FSL 6.0.7.23's ``flirt`` (through the
oracle cache, every input file keyed by content) and larmorx's (``LARMORX_FAST_BIN`` or
``LARMORX_FLIRT_BIN``, a ``larmorx`` binary, else this Python's package) on small inputs that
differ in format (NIfTI-1, ANALYZE 7.5), ``dim[0]`` (3, or 4 with one volume) and ``pixdim[4]``
(0, 2.5; and a 4D input of two volumes with TR 0, TR -2 with each ``dim[0]``, NIfTI), against a
NIfTI reference with ``pixdim[4..7] = 1.7, 1.3, 1.3, 1.3``. Prints each
output's ``pixdim[4..7]``. Behind ``docs/findings/fsl-flirt-original.md``, "`-applyxfm`'s TR".

    PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 8 \\
        python -P validation/investigations/cleanroom-fast/applyxfm_tr.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity.fsl_flirt_replica import _environment, fsldir

VARIANTS = [
    (fmt, ndim, tr) for fmt in ("nifti", "analyze") for ndim in (3, 4) for tr in (0.0, 2.5)
] + [("nifti", 5, 0.0), ("nifti", 4, -2.0), ("nifti", 3, -2.0)]


def write_input(d: Path, fmt: str, ndim: int, tr: float) -> str:
    rng = np.random.default_rng(11)
    a = (rng.random((12, 11, 10)) * 100).astype(np.float32)
    if ndim == 4:
        a = a[..., None]
    elif ndim == 5:  # a 4D image of two volumes (dim[0] 4)
        a = np.stack([a, a * 0.5], -1)
    aff = np.diag([2.0, 2.0, 2.0, 1.0])
    if fmt == "nifti":
        im = nib.Nifti1Image(a, aff)
        im.header["pixdim"][4:8] = [tr, 0, 0, 0]
        nib.save(im, d / "in.nii.gz")
        return "in.nii.gz"
    im = nib.AnalyzeImage(a, aff)
    im.header["pixdim"][4:8] = [tr, 0, 0, 0]
    nib.save(im, d / "in.hdr")
    return "in.hdr"


def main() -> int:
    exe = os.environ.get("LARMORX_FLIRT_BIN") or os.environ.get("LARMORX_FAST_BIN")
    lx = [exe] if exe else [sys.executable, "-m", "larmorx"]
    for fmt, ndim, tr in VARIANTS:
        with tempfile.TemporaryDirectory(prefix="lx-flirt-tr-") as t:
            d = Path(t)
            name = write_input(d, fmt, ndim, tr)
            r = nib.Nifti1Image(
                (np.arange(14 * 13 * 12, dtype=np.float32) % 17).reshape(14, 13, 12),
                np.diag([1.7, 1.7, 1.7, 1.0]),
            )
            r.header["pixdim"][4:8] = [1.7, 1.3, 1.3, 1.3]
            nib.save(r, d / "ref.nii.gz")
            (d / "ident.mat").write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n")
            args = ["-in", name, "-ref", "ref.nii.gz", "-applyxfm", "-init", "ident.mat", "-out"]
            env = _environment({"FSLOUTPUTTYPE": "NIFTI_GZ"})
            p = oracle_cache.run(
                [str(fsldir() / "bin" / "flirt"), *args, "fsl"],
                env=env,
                cwd=d,
                outputs=[d],
                policy=oracle_cache.Policy(threads=1, family="fsl"),
                runner="cleanroom-fast",
                inputs=[q for q in sorted(d.iterdir()) if q.is_file()],
                timeout=600,
            )
            q = subprocess.run(
                [*lx, "fsl", "flirt", *args, "orig"],
                cwd=d,
                env={**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"},
                capture_output=True,
                text=True,
                check=False,
            )
            out = []
            for who, code in (("fsl", p.returncode), ("orig", q.returncode)):
                f = d / f"{who}.nii.gz"
                if f.is_file():
                    out.append(f"{who} {nib.load(str(f)).header['pixdim'][4:8].tolist()}")
                else:
                    out.append(f"{who} exit {code}, no output")
            what = "dim[0]=4, 2 volumes" if ndim == 5 else f"dim[0]={ndim}"
            print(f"{fmt:7s} {what} pixdim[4]={tr}: " + "; ".join(out))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
