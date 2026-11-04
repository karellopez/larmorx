# SPDX-License-Identifier: Apache-2.0
"""A random-testing finding of fslmaths' replica, value by value: FSL's output against the
replica's (reproduce), the input's value at each differing voxel, with every value's bits.

    FSLDIR=<FSL 6.0.7.23> python -P validation/investigations/replica-glibc-dbl/finding_diff.py \\
        <larmorx-nc> <finding.json>

The finding's shrunk recipe is materialized (``larmorx_validation.difftest.inputs``), its
arguments run by both programs in fresh directories. Prints the headers' equality and at most 12
differing voxels.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest import inputs
from larmorx_validation.difftest.core import arguments


def main(nc: str, finding: str) -> int:
    nc = str(Path(nc).resolve())
    doc = json.loads(Path(finding).read_text(encoding="utf-8"))
    recipe = doc.get("shrunk") or doc["recipe"]
    prefix = inputs.register(recipe["inputs"])
    words, srcs = [], {}
    for w in arguments(recipe):
        if w.startswith("{in:") and w.endswith("}"):
            p = inputs.path(inputs.key(prefix, w[4:-1]))
            srcs[p.name] = p
            words.append(p.name)
        else:
            words.append(w)
    env = {**os.environ, "FSLOUTPUTTYPE": recipe["extra"].get("output_type", "NIFTI_GZ")}
    out = {}
    for who, cmd, extra in (
        ("fsl", [str(Path(os.environ["FSLDIR"]) / "bin/fslmaths")], {}),
        ("replica", [nc, "fsl", "fslmaths"], {"LARMORX_UPSTREAM_DEFECTS": "reproduce"}),
    ):
        d = Path(tempfile.mkdtemp(prefix="lx-finding-"))
        for name, p in srcs.items():
            shutil.copy(p, d / name)
        r = subprocess.run([*cmd, *words], cwd=d, env={**env, **extra}, capture_output=True)
        print(who, "exit", r.returncode)
        out[who] = nib.load(str(next(d.glob("out*"))))
    print("arguments:", " ".join(words))
    a, b = out["fsl"], out["replica"]
    print("headers identical:", a.header.binaryblock == b.header.binaryblock)
    x, y = np.asarray(a.dataobj).ravel(), np.asarray(b.dataobj).ravel()
    view = {4: np.uint32, 8: np.uint64}[x.dtype.itemsize]
    bad = np.flatnonzero(x.view(view) != y.view(view))
    src = np.asarray(nib.load(str(next(iter(srcs.values())))).dataobj).ravel()
    print(f"{len(bad)} of {x.size} values differ in their bits ({x.dtype})")
    for i in bad[:12]:
        print(f"  voxel {i}: input {src[i]!r}, FSL {x[i]!r} {x[i : i + 1].view(view)[0]:x}, "
              f"replica {y[i]!r} {y[i : i + 1].view(view)[0]:x}")  # fmt: skip
    return 0 if len(bad) == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
