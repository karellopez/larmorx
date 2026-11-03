# SPDX-License-Identifier: Apache-2.0
"""FSL's output files for cases of the replica suite (``fsl-maths-replica``) or for ad-hoc
command lines, kept for analysis: black-box runs of FSL 6.0.7.23's fslmaths through the
oracle-output cache (served when they ran before), as the clean-room original may use them.

Run (in a memory box): ``python -P fsl_outputs.py <outdir> <case or 'args:...'> ...``

- ``<case>``: a replica-suite case name (``conv/fft-even``); its run directory is copied to
  ``<outdir>/<case with / as _>/``;
- ``args:<name>:<arg> <arg> ...``: fslmaths' arguments, ``@name`` naming a replica-suite input
  (copied into the run directory) or ``@probe:<recipe>`` one of this script's own inputs
  (``impulse``: a 15 × 15 × 15 float image of zeros with 1 at its centre, voxel sizes 2, 2.5,
  3; ``impulse<n>``: the same of ``n`` voxels per axis; ``ramp``: a 16 × 13 × 11 float image
  ``x + 20 y + 400 z`` with the same voxel sizes
  (``rampnan``, ``rampinf``: its first voxel NaN or +inf);
  ``asym``: a 13 × 11 × 9 asymmetric kernel file of values in [0, 1); ``asym<nx>x<ny>x<nz>``: an
  asymmetric kernel of those sizes, values ``1 + x + 3 y + 7 z``), each written once with fixed
  bytes.
"""

from __future__ import annotations

import shutil
import sys
import tempfile
from pathlib import Path

import numpy as np
from larmorx_validation.parity import fsl_maths_replica as rep


def _probe_input(name: str, d: Path) -> str:
    import nibabel as nib

    aff = np.diag([-2.0, 2.5, 3.0, 1.0])
    if name == "impulse":
        a = np.zeros((15, 15, 15), dtype=np.float32)
        a[7, 7, 7] = 1.0
    elif name.startswith("impulse") and name[7:].isdigit():
        # impulse<n>: an n x n x n impulse (n odd), the 1 at its centre
        n = int(name[7:])
        a = np.zeros((n, n, n), dtype=np.float32)
        a[n // 2, n // 2, n // 2] = 1.0
    elif name in ("ramp", "rampnan", "rampinf"):
        x, y, z = np.meshgrid(np.arange(16), np.arange(13), np.arange(11), indexing="ij")
        a = (x + 20 * y + 400 * z).astype(np.float32)
        if name != "ramp":
            a[0, 0, 0] = np.nan if name == "rampnan" else np.inf
    elif name == "asym":
        rng = np.random.default_rng(7)
        a = rng.random((13, 11, 9)).astype(np.float32)
    elif name.startswith("asym") and "x" in name:
        # asym<nx>x<ny>x<nz>: an asymmetric kernel of those sizes, values 1 + x + 3 y + 7 z
        sx, sy, sz = (int(v) for v in name[4:].split("x"))
        x, y, z = np.meshgrid(np.arange(sx), np.arange(sy), np.arange(sz), indexing="ij")
        a = (1 + x + 3 * y + 7 * z).astype(np.float32)
    else:
        raise SystemExit(f"unknown probe input {name}")
    img = nib.Nifti1Image(a, aff)
    img.header.set_sform(aff, 1)
    img.header.set_qform(aff, 1)
    p = d / f"probe_{name}.nii.gz"
    nib.save(img, str(p))
    return p.name


def main(argv: list[str]) -> int:
    out = Path(argv[0])
    out.mkdir(parents=True, exist_ok=True)
    cases = rep._cases()
    for item in argv[1:]:
        with tempfile.TemporaryDirectory() as t:
            d = Path(t)
            if item.startswith("args:"):
                _, label, rest = item.split(":", 2)
                args = []
                for a in rest.split():
                    if a.startswith("@probe:"):
                        args.append(_probe_input(a[len("@probe:") :], d))
                    elif a.startswith("@"):
                        args.extend(rep._prepare(rep._r(a), d))
                    else:
                        args.append(a)
                settings: dict[str, str | None] = {}
            else:
                label = item.replace("/", "_")
                run = cases[item][2]
                args = rep._prepare(run, d)
                settings = dict(run.settings)
            p = rep.oracle_run(args, cwd=d, settings=settings)
            dest = out / label
            if dest.exists():
                shutil.rmtree(dest)
            shutil.copytree(d, dest)
            (dest / "RUN.txt").write_text(
                f"fslmaths {' '.join(args)}\nexit {p.returncode}\n--stdout\n{p.stdout}\n"
                f"--stderr\n{p.stderr}\n",
                encoding="utf-8",
            )
            print(f"{label}: exit {p.returncode} -> {dest}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
