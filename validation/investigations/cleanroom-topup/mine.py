# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Runs the clean-room original on the inputs of an FSL run directory (ARGS.txt), in a new
directory; prints stdout and a comparison of the files. Usage: mine.py <fsl-run-dir> [threads]"""

import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

import nibabel as nib
import numpy as np

W = Path(__file__).resolve().parents[3]
src = Path(sys.argv[1]).resolve()
threads = sys.argv[2] if len(sys.argv) > 2 else "4"
dst = src.parent / (src.name.replace("fsl-", "mine-"))
if dst.exists():
    shutil.rmtree(dst)
dst.mkdir()
args = [a for a in (src / "ARGS.txt").read_text().split() if not a.startswith("--debug")]
inputs = set()
for a in [x for x in args if x.split("=")[0] in ("--imain", "--datain", "--acqp", "--config")]:
    v = a.partition("=")[2]
    for part in v.split(","):
        if part and (src / part).is_file():
            inputs.add(part)
for f in ("b02b0.cnf",):
    if (src / f).is_file():
        inputs.add(f)
for f in inputs:
    shutil.copy(src / f, dst / f)
env = {
    "PATH": "/usr/bin:/bin",
    "HOME": os.environ["HOME"],
    "FSLOUTPUTTYPE": "NIFTI_GZ",
    "FSLDIR": os.environ.get("FSLDIR", os.path.expanduser("~/fsl-6.0.7.23")),
    "LARMORX_IMPLEMENTATION": "original",
    "OMP_NUM_THREADS": threads,
    "LARMORX_UPSTREAM_DEFECTS": os.environ.get("LXD", "reproduce"),
}
t0 = time.time()
p = subprocess.run(
    [str(W / "target/release/larmorx"), "fsl", "topup", *args],
    cwd=dst,
    env=env,
    capture_output=True,
    text=True,
)
dt = time.time() - t0
(dst / "STDOUT.txt").write_text(p.stdout)
(dst / "STDERR.txt").write_text(p.stderr)
print(f"exit {p.returncode} in {dt:.1f} s; stderr: {p.stderr[-500:]!r}")
fo = (src / "STDOUT.txt").read_text().splitlines()
mo = p.stdout.splitlines()
if fo or mo:
    print("stdout lines fsl/mine:", len(fo), len(mo))
    for a, b in zip(fo, mo):
        if a != b:
            print("  FSL ", a)
            print("  mine", b)
fs = {f.name for f in src.iterdir()} - {"ARGS.txt", "STDOUT.txt", "STDERR.txt"}
ms = {f.name for f in dst.iterdir()} - {"STDOUT.txt", "STDERR.txt"}
print(
    "files only FSL:",
    sorted(x for x in fs - ms if not x.startswith("TopupDebug")),
    "only mine:",
    sorted(ms - fs),
)
for name in sorted(fs & ms):
    if name.endswith(".nii.gz") and name not in inputs:
        a = np.asarray(nib.load(src / name).dataobj, np.float64)
        b = np.asarray(nib.load(dst / name).dataobj, np.float64)
        if a.shape != b.shape:
            print(f"{name}: shapes {a.shape} {b.shape}")
            continue
        d = np.abs(a - b)
        scale = np.percentile(np.abs(a), 99) or 1
        print(
            f"{name}: max|d| {d.max():.4g}  p99|d| {np.percentile(d, 99):.4g}  mean|d| {d.mean():.4g}  (p99|fsl| {scale:.4g})  corr {np.corrcoef(a.ravel(), b.ravel())[0, 1]:.6f}"
        )
    elif name.endswith((".txt", ".mat", "_log")) and name not in inputs:
        same = (src / name).read_text() == (dst / name).read_text()
        print(f"{name}: {'identical' if same else 'DIFFERS'}")
