# SPDX-License-Identifier: Apache-2.0
"""mcflirt's and larmorx's matrices for few histogram bins, side by side, on the §14.1 phantom
of ``larmorx_validation.parity.mri_hmc_failures`` (black-box runs only).

    PYTHONPATH=python:validation/src python -P hmc_bins_matrices.py <work> --larmorx <program> \\
        -- -cost mutualinfo -bins 8 -stages 1

Runs mcflirt (FSL 6.0.7.23) and ``larmorx mri hmc`` (``reproduce``) with the arguments given
(``-in``, ``-out`` and ``-mats`` added) and prints per volume the RMS deviation (mm, radius 80 mm)
and whether the matrix files are identical as text.
"""

from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path

from larmorx_validation.parity import mri_hmc, mri_hmc_failures


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("work", type=Path)
    ap.add_argument("--larmorx", required=True)
    ap.add_argument("rest", nargs=argparse.REMAINDER)
    a = ap.parse_args()
    extra = [w for w in a.rest if w != "--"]
    program, env = mri_hmc_failures._environment()
    series = mri_hmc_failures.inputs()["in"]
    out = {}
    for who, cmd in (("mcflirt", [program]), ("larmorx", [a.larmorx, "mri", "hmc"])):
        d = a.work / who
        d.mkdir(parents=True, exist_ok=True)
        e = dict(env, LARMORX_UPSTREAM_DEFECTS="reproduce", OMP_NUM_THREADS="1")
        p = subprocess.run(
            [*cmd, "-in", str(series), "-out", str(d / "o"), "-mats", *extra],
            env=e,
            capture_output=True,
            text=True,
        )
        print(who, "exit", p.returncode, " ".join(p.stderr.split())[:200])
        out[who] = mri_hmc._read_mats(d / "o.mat")
    centre = mri_hmc._fov_centre(series)
    (ma, ta), (mb, tb) = out["mcflirt"], out["larmorx"]
    for t, (x, y) in enumerate(zip(ma, mb, strict=True)):
        dev = mri_hmc.rms_deviation(x, y, centre)
        print(f"volume {t}: RMS deviation {dev:.4f} mm, identical as text: {ta[t] == tb[t]}")
    os.sync()


if __name__ == "__main__":
    main()
