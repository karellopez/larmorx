# SPDX-License-Identifier: Apache-2.0
"""Reproduces a FLIRT difftest finding by hand: its shrunk recipe's inputs in two directories,
FSL's program and the replica (reproduce) on each, their standard output and error kept
(``stdout.txt``, ``stderr.txt``) with the first line where they differ, the output images
compared voxel by voxel.

Usage: python -P finding_repro.py <finding.json> <out dir> [--drawn] [extra flirt args...]
"""

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest import inputs
from larmorx_validation.difftest.core import arguments
from larmorx_validation.parity import fsl_flirt_replica as ff

finding = json.loads(Path(sys.argv[1]).read_text())
out = Path(sys.argv[2])
rest = sys.argv[3:]
recipe = finding["recipe"] if "--drawn" in rest else finding["shrunk"]
extra = [a for a in rest if a != "--drawn"]
prefix = inputs.register(recipe["inputs"])
if out.exists():
    shutil.rmtree(out)
words = []
for d in ("fsl", "rep"):
    (out / d).mkdir(parents=True)
for w in arguments(recipe):
    if w.startswith("{in:"):
        src = inputs.path("dt:" + inputs.key(prefix, w[4:-1]))
        for d in ("fsl", "rep"):
            shutil.copy(src, out / d / src.name)
        words.append(src.name)
    elif w.startswith("@fslsch:"):
        words.append(str(ff.fsldir() / "etc/flirtsch" / w[8:]))
    else:
        words.append(w)
words += extra
env = dict(
    os.environ,
    FSLOUTPUTTYPE=recipe["extra"]["output_type"],
    FSLDIR=str(ff.fsldir()),
    LARMORX_UPSTREAM_DEFECTS="reproduce",
)
prog = recipe["extra"]["tool"]
print(prog, " ".join(words))
a = subprocess.run(
    [str(ff.fsldir() / "bin" / prog), *words],
    cwd=out / "fsl",
    env=env,
    capture_output=True,
    text=True,
)
b = subprocess.run(
    [str(ff.replica_binary()), "fsl", prog, *words],
    cwd=out / "rep",
    env=env,
    capture_output=True,
    text=True,
)
print("exit", a.returncode, b.returncode)
print("FSL stderr:", a.stderr[-500:])
print("rep stderr:", b.stderr[-500:])


def first_difference(x: str, y: str) -> str:
    lx, ly = x.splitlines(), y.splitlines()
    for k in range(max(len(lx), len(ly))):
        u = lx[k] if k < len(lx) else "<end>"
        v = ly[k] if k < len(ly) else "<end>"
        if u != v:
            return f"line {k + 1}: FSL {u!r}, replica {v!r} ({len(lx)} vs {len(ly)} lines)"
    return f"identical ({len(lx)} lines)"


for run, d in ((a, "fsl"), (b, "rep")):
    (out / d / "stdout.txt").write_text(run.stdout, encoding="utf-8")
    (out / d / "stderr.txt").write_text(run.stderr, encoding="utf-8")
print("stdout:", first_difference(a.stdout, b.stdout))
print("stderr:", first_difference(a.stderr, b.stderr))
for f in sorted((out / "fsl").iterdir()):
    g = out / "rep" / f.name
    if f.name.startswith("out") and f.suffix != ".hdr":
        if f.name.endswith((".nii", ".nii.gz", ".img")):
            x = np.asanyarray(nib.load(str(f)).dataobj).astype(np.float64)
            y = np.asanyarray(nib.load(str(g)).dataobj).astype(np.float64)
            diff = np.argwhere(x != y)
            print(f.name, x.shape, "differ:", len(diff))
            for idx in diff[:12]:
                print("  ", tuple(idx), x[tuple(idx)], y[tuple(idx)])
        else:
            print(f.name, "same" if f.read_bytes() == g.read_bytes() else "DIFFER")
