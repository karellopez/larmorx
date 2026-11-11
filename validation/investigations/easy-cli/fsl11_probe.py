# SPDX-License-Identifier: Apache-2.0
"""FSL-11 as a user meets it: FSL's programs (black box) and larmorx's clean-room originals with
``FSLOUTPUTTYPE`` unset, on a small synthetic series.

For each call: FSL's exit status, the first line of its standard error and the files it wrote;
larmorx's (the standalone program given) with ``--upstream-defects fix`` (the default: ``.nii.gz``
and one FSL-11 line) and ``reproduce`` (FSL's message and exit 1 where FSL stops). The calls:
mcflirt (``-mats -plots``; ``-meanvol -mats``: where it stops when an image comes before the
matrices), flirt with ``-omat`` only (no image: nothing to stop at) and with ``-out``, fslmaths.

    python -P validation/investigations/easy-cli/fsl11_probe.py \\
        --fsl ~/fsl-6.0.7.23 --larmorx target/release/larmorx --work <scratch>/fsl11

Small inputs (16 x 16 x 12, 4 volumes): no memory box needed. Results in README.md.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
from pathlib import Path

import nibabel as nib
import numpy as np

CALLS: dict[str, list[str]] = {
    "mcflirt -mats -plots": ["mcflirt", "-in", "bold.nii", "-out", "o", "-mats", "-plots"],
    "mcflirt -meanvol -mats": ["mcflirt", "-in", "bold.nii", "-out", "o", "-meanvol", "-mats"],
    "flirt -omat": ["flirt", "-in", "t1.nii", "-ref", "t1.nii", "-omat", "o.mat"],
    "flirt -out": ["flirt", "-in", "t1.nii", "-ref", "t1.nii", "-out", "o", "-omat", "o.mat"],
    "fslmaths": ["fslmaths", "t1.nii", "-add", "1", "o"],
}


def inputs(d: Path) -> None:
    rng = np.random.default_rng(11)
    shape = (16, 16, 12)
    base = np.zeros(shape, np.float32)
    base[4:12, 4:12, 3:9] = 100
    vols = [base + rng.normal(0, 1, shape).astype(np.float32) for _ in range(4)]
    affine = np.diag([2.0, 2.0, 2.0, 1.0])
    nib.save(nib.Nifti1Image(np.stack(vols, -1), affine), d / "bold.nii")
    nib.save(nib.Nifti1Image(vols[0], affine), d / "t1.nii")


def run(cmd: list[str], d: Path, env: dict[str, str]) -> tuple[int, str, list[str]]:
    before = {p.name for p in d.iterdir()}
    r = subprocess.run(cmd, cwd=d, env=env, capture_output=True, text=True, timeout=600)
    after = sorted(p.name for p in d.iterdir() if p.name not in before)
    lines = [x for x in r.stderr.splitlines() if x.strip()]
    first = next(
        (x for x in lines if "FSL-11" in x or "FSLOUTPUTTYPE" in x), lines[0] if lines else ""
    )
    for name in after:
        p = d / name
        shutil.rmtree(p) if p.is_dir() else p.unlink()
    return r.returncode, first, after


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fsl", type=Path, required=True)
    ap.add_argument("--larmorx", type=Path, required=True)
    ap.add_argument("--work", type=Path, required=True)
    a = ap.parse_args()
    d = a.work
    d.mkdir(parents=True, exist_ok=True)
    inputs(d)
    base = {k: v for k, v in os.environ.items() if k in ("HOME", "LANG", "PATH", "TMPDIR")}
    fsl_env = {**base, "FSLDIR": str(a.fsl), "PATH": f"{a.fsl}/bin:/usr/bin:/bin"}
    for name, argv in CALLS.items():
        code, first, files = run([str(a.fsl / "bin" / argv[0]), *argv[1:]], d, fsl_env)
        print(f"FSL     {name}: exit {code}; {first!r}; wrote {files}")
        for mode in ("fix", "reproduce"):
            cmd = [str(a.larmorx.resolve()), "--upstream-defects", mode, "fsl", *argv]
            code, first, files = run(cmd, d, base)
            print(f"larmorx {mode:9s} {name}: exit {code}; {first!r}; wrote {files}")


if __name__ == "__main__":
    main()
