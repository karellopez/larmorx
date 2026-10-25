# SPDX-License-Identifier: Apache-2.0
"""ResampleImage's Gaussian interpolator with too few, zero or negative sigmas (ANTS-160): ANTs
2.6.5 and larmorx in both D13 modes on one small image, each run's exit status, its messages
(counted) and the output's type and values.

    python -P validation/investigations/difftest/probe_resample_sigma.py <work dir>

ANTs is ``<workspace>/oracles/ants-2.6.5/bin/ResampleImage``; larmorx is ``python -m larmorx``
of the current environment.
"""

from __future__ import annotations

import collections
import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import ants_programs

SIGMAS = ("1", "1x1", "0x0x0", "-1x-1x-1", "1x1x1")


def summary(path: Path) -> str:
    if not path.is_file():
        return "no output"
    img = nib.load(path)
    d = np.asarray(img.dataobj)
    u = np.unique(d)
    return f"{img.get_data_dtype()} {d.shape} values {u[:4].tolist()}{' ...' if u.size > 4 else ''}"


def lines(text: str) -> str:
    c = collections.Counter(ln.strip() for ln in text.splitlines() if ln.strip())
    return "; ".join(f"{n} x {ln[:90]}" for ln, n in c.most_common(3))


def main() -> int:
    work = Path(sys.argv[1])
    work.mkdir(parents=True, exist_ok=True)
    a = work / "a.nii.gz"
    img = nib.Nifti1Image(np.full((2, 2, 2), 3.0, np.float32), np.eye(4))
    img.header.set_qform(np.eye(4), 1)
    img.header.set_sform(np.eye(4), 1)
    nib.save(img, a)
    ants = ants_programs.ants_bin() / "ResampleImage"
    for sigma in SIGMAS:
        # Pixel type 7 (double) after the sigma and alpha (argument 9): larmorx's reading;
        # ANTs reads argument 7 (the sigma) as the pixel type (ANTS-30).
        args = ["3", str(a), "{out}", "3x3x3", "1", "2", sigma, "1"]
        out = work / f"ants-{sigma}.nii.gz"
        p = subprocess.run(
            [str(ants), *[str(out) if x == "{out}" else x for x in args]],
            capture_output=True,
            text=True,
            check=False,
        )
        print(
            f"sigma {sigma}: ANTs exit {p.returncode}: {summary(out)} | {lines(p.stdout + p.stderr)}"
        )
        for mode in ("fix", "reproduce"):
            out = work / f"lx-{mode}-{sigma}.nii.gz"
            env = {**os.environ, "LARMORX_UPSTREAM_DEFECTS": mode}
            cmd = [
                sys.executable,
                "-m",
                "larmorx",
                "ants",
                "ResampleImage",
                *[str(out) if x == "{out}" else x for x in args],
            ]
            p = subprocess.run(cmd, capture_output=True, text=True, env=env, check=False)
            print(
                f"  larmorx {mode}: exit {p.returncode}: {summary(out)} | {lines(p.stdout + p.stderr)}"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
