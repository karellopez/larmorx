# SPDX-License-Identifier: Apache-2.0
"""probe.py's calls of avscale (random matrices)."""

import os
from pathlib import Path

import numpy as np

#: the parity suite's derived inputs (``oracles/inputs/fsl-flirt-replica`` in the workspace)
INP = Path(
    os.environ.get(
        "LARMORX_FLIRT_INPUTS",
        Path(__file__).resolve().parents[4] / "oracles/inputs/fsl-flirt-replica",
    )
)
FSL = Path(os.environ.get("LARMORX_FSL_623", Path.home() / "fsl-6.0.7.23"))

rng = np.random.default_rng(20261101)
PROGRAM = "avscale"
FILES = {}
CASES = []


def rot(a, b, c):
    ca, sa, cb, sb, cc, sc = np.cos(a), np.sin(a), np.cos(b), np.sin(b), np.cos(c), np.sin(c)
    rx = np.array([[1, 0, 0], [0, ca, -sa], [0, sa, ca]])
    ry = np.array([[cb, 0, sb], [0, 1, 0], [-sb, 0, cb]])
    rz = np.array([[cc, -sc, 0], [sc, cc, 0], [0, 0, 1]])
    return rx @ ry @ rz


for i in range(400):
    kind = i % 5
    m = np.eye(4)
    ang = rng.uniform(-np.pi, np.pi, 3) * (0.05 if kind == 0 else 1.0)
    r = rot(*ang)
    s = np.diag(rng.uniform(0.5, 2.0, 3)) if kind != 1 else np.eye(3)
    k = np.eye(3)
    if kind >= 2:
        k[0, 1], k[0, 2], k[1, 2] = rng.uniform(-0.3, 0.3, 3)
    a = r @ k @ s
    if kind == 3:
        a[:, 0] = -a[:, 0]  # a reflection
    if kind == 4:
        a = rng.normal(0, 1, (3, 3))  # any matrix
    m[:3, :3] = a
    m[:3, 3] = rng.uniform(-100, 100, 3)
    name = f"m{i}.mat"
    FILES[name] = "\n".join("  ".join(f"{v:.10g}" for v in row) for row in m) + "\n"
    opt = ["--allparams", "--inverteddies", None][i % 3]
    CASES.append([opt, name] if opt else [name])
