# SPDX-License-Identifier: Apache-2.0
"""Runs the official mri_volsynth (FreeSurfer 8.2.0, one thread) for six random fields and compares
its float32 voxels with the probe's RFdrawVal draws, taken in RFsynth's order (column outermost,
then row, slice, frame innermost) and rounded to float32 (specs/ranlux.md §10.4).

Usage: python -P volsynth_compare.py <FREESURFER_HOME> <FS_LICENSE> <fs_rng_probe> <scratch folder>
A one-off observation of the spec writer (outside the oracle-output cache); suites use oracle_cache.run.
"""

from __future__ import annotations

import os
import struct
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

FS_HOME, LICENSE, PROBE, OUT = sys.argv[1:5]
OUT_DIR = Path(OUT)
OUT_DIR.mkdir(parents=True, exist_ok=True)
ENV = dict(os.environ, FREESURFER_HOME=FS_HOME, FS_LICENSE=LICENSE, OMP_NUM_THREADS="1")
DIM = ["5", "4", "3", "2"]
CASES = [  # mri_volsynth options, probe distribution, seed, params
    (["--pdf", "z"], "gaussian", 53, (0.0, 1.0)),
    (["--pdf", "chi2", "--dof", "30"], "chi2", 7, (30.0, 0.0)),
    (["--pdf", "chi2", "--dof", "3"], "chi2", 7, (3.0, 0.0)),
    (["--pdf", "t", "--dof", "9"], "t", 7, (9.0, 0.0)),
    (["--pdf", "t", "--dof", "2"], "t", 13, (2.0, 0.0)),
    (["--pdf", "F", "--dof-num", "3", "--dof-den", "25"], "F", 11, (3.0, 25.0)),
]
for k, (opts, name, seed, (p1, p2)) in enumerate(CASES):
    vol = OUT_DIR / f"case{k}.mgz"
    subprocess.run(
        [
            f"{FS_HOME}/bin/mri_volsynth",
            *opts,
            "--dim",
            *DIM,
            "--seed",
            str(seed),
            "--vol",
            str(vol),
        ],
        env=ENV,
        check=True,
        capture_output=True,
    )
    img = np.asarray(nib.load(vol).dataobj)
    w, h, d, f = img.shape
    out = subprocess.run(
        [PROBE, "rf", name, str(seed), str(w * h * d * f), repr(p1), repr(p2)],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.split()
    draws = iter(struct.unpack(">d", bytes.fromhex(x))[0] for x in out)
    same = sum(
        np.float32(next(draws)) == img[c, r, s, fr]
        for c in range(w)
        for r in range(h)
        for s in range(d)
        for fr in range(f)
    )
    print(f"{' '.join(opts)} --seed {seed}: {same} of {w * h * d * f} voxels equal (float32)")
