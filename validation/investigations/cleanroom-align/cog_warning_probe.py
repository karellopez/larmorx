# SPDX-License-Identifier: Apache-2.0
"""cog_warning_probe.py <dir>: how often mcflirt (FSL 6.0.7.23, through the oracle-output cache,
black box) prints ``WARNING::in calculating COG, total = 0.0`` (`specs/mcflirt.md` §6.1: a centre
whose total is below 1e-5) on small series holding constant volumes, and what the clean-room
original prints on the same calls. Each line: the case, mcflirt's status and its count of the
warning, the original's status and count. The clean-room catch-up, relay 7 (2026-11-05)."""

from __future__ import annotations

import contextlib
import io
import os
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

WARNING = "WARNING::in calculating COG, total = 0.0"


def series(constant: list[int], n: int = 4) -> np.ndarray:
    """A 16 x 16 x 8 series of ``n`` volumes: a blob moving by a voxel per volume, the volumes in
    ``constant`` all 100."""
    x, y, z = np.meshgrid(np.arange(16), np.arange(16), np.arange(8), indexing="ij")
    out = np.zeros((16, 16, 8, n), np.float32)
    for t in range(n):
        if t in constant:
            out[..., t] = 100.0
        else:
            r2 = (x - 7.5 - 0.3 * t) ** 2 + (y - 8.0) ** 2 + ((z - 3.5) * 2) ** 2
            out[..., t] = 100.0 + 400.0 * np.exp(-r2 / 18.0)
    return out


CASES = {
    # one constant test volume, the middle volume (2 of 4) the reference
    "constant-test-volume": ([3], []),
    # the reference itself constant (the .par's centre, and every test volume's own)
    "constant-reference": ([2], []),
    "constant-reference-plots": ([2], ["-plots"]),
    "two-constant": ([0, 3], ["-plots"]),
    "every-volume-constant": ([0, 1, 2, 3], ["-plots"]),
    "constant-test-volume-stages4": ([3], ["-stages", "4"]),
    "constant-test-volume-meanvol": ([3], ["-meanvol", "-plots"]),
    "constant-test-volume-fudge": ([3], ["-fudge"]),
}


def main(argv: list[str]) -> int:
    from larmorx_validation import oracle_cache
    from larmorx_validation.parity.mri_hmc_failures import _environment

    from larmorx.cli import run as lx_run

    root = Path(argv[0])
    program, env = _environment()
    for name, (constant, extra) in CASES.items():
        d = root / name
        (d / "fsl").mkdir(parents=True, exist_ok=True)
        (d / "lx").mkdir(parents=True, exist_ok=True)
        img = nib.Nifti1Image(series(constant), np.diag([3.0, 3.0, 4.0, 1.0]))
        img.header.set_xyzt_units("mm", "sec")
        nib.save(img, d / "in.nii.gz")
        args = ["-in", str(d / "in.nii.gz"), "-out", "out", *extra]
        p = oracle_cache.run(
            [program, *args],
            env=env,
            cwd=d / "fsl",
            outputs=[d / "fsl"],
            policy=oracle_cache.Policy(threads=1, family="fsl", never=""),
            runner="mri-hmc-cog-probe",
        )
        out, err = io.StringIO(), io.StringIO()
        saved = os.getcwd()
        os.chdir(d / "lx")
        os.environ["FSLOUTPUTTYPE"] = "NIFTI_GZ"
        try:
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = lx_run(["larmorx", "mri", "hmc", *args])
        finally:
            os.chdir(saved)
        print(
            f"{name}: mcflirt {p.returncode} {p.stderr.count(WARNING)}x "
            f"(stdout {p.stdout.count(WARNING)}x), original {code} {err.getvalue().count(WARNING)}x"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
