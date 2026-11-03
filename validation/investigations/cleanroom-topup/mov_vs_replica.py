# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# The original against the replica (black box) on the half-resolution pair: the movement part of
# the first gradient, per configuration (which transformation makes FSL's rotation gradient differ).
import os, shutil, subprocess, sys
from pathlib import Path
import numpy as np

S = Path(sys.argv[1])
W = Path(sys.argv[2])
nc = Path(os.environ["LARMORX_NC_BIN"])
mine = W / "target/release/larmorx"
inp = Path(os.environ["LARMORX_WORKSPACE"]) / "oracles/inputs/fsl-topup-replica"
base = [
    "--imain=half.nii.gz",
    "--datain=acq.txt",
    "--out=o",
    "--warpres=14",
    "--miter=1",
    "--lambda=0.001",
    "--estmov=1",
    "--debug=1",
    "--minmet=0",
]
confs = {
    "a r0 s1 f0": ["--regrid=0", "--subsamp=1", "--fwhm=0"],
    "b r1 s1 f0": ["--regrid=1", "--subsamp=1", "--fwhm=0"],
    "c r0 s2 f0": ["--regrid=0", "--subsamp=2", "--fwhm=0"],
    "d r0 s1 f6": ["--regrid=0", "--subsamp=1", "--fwhm=6"],
    "e r1 s2 f6": ["--regrid=1", "--subsamp=2", "--fwhm=6"],
}
env = {
    "PATH": "/usr/bin:/bin",
    "FSLOUTPUTTYPE": "NIFTI_GZ",
    "HOME": os.environ["HOME"],
    "LARMORX_IMPLEMENTATION": "original",
    "OMP_NUM_THREADS": "4",
    "LARMORX_UPSTREAM_DEFECTS": "reproduce",
}
for name, extra in confs.items():
    res = {}
    for who, exe in (("replica", [str(nc)]), ("original", [str(mine)])):
        d = S / "cmp" / name.replace(" ", "_") / who
        if d.exists():
            shutil.rmtree(d)
        d.mkdir(parents=True)
        for f in ("half.nii.gz", "acq.txt"):
            shutil.copy(inp / f, d / f)
        p = subprocess.run(
            [*exe, "fsl", "topup", *base, *extra], cwd=d, env=env, capture_output=True, text=True
        )
        g = np.loadtxt(d / "TopupDebugGradient_level01_iter01_attempt00.txt")
        res[who] = g
        if p.returncode:
            print(who, p.stderr[-300:])
    a, b = res["replica"], res["original"]
    print(
        name,
        "n",
        a.shape,
        b.shape,
        "coef rel",
        np.abs(a[:-5] - b[:-5]).max() / np.abs(a[:-5]).max(),
        "\n   replica ",
        np.round(a[-5:], 4),
        "\n   original",
        np.round(b[-5:], 4),
    )
