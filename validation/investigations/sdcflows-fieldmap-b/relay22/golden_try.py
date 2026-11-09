# SPDX-License-Identifier: Apache-2.0
"""Relay 22: the new entries of the golden cases ``py.ndimage.binary`` and ``py.ndimage.label``
twice (repeatable) and against SciPy (its corrected forms where larmorx corrects it). Run in
each environment with ``PYTHONPATH=<repo>/python``."""

import sys
import tempfile
import warnings
from pathlib import Path

import numpy as np
from scipy import ndimage as snd

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tests" / "golden"))
import registry as r


def run(name):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return r.CASES[name].run(r.Context(Path(tempfile.mkdtemp()), None))


for name in ("py.ndimage.binary", "py.ndimage.label"):
    a, b = run(name), run(name)
    print(
        name,
        "repeatable",
        all(np.asarray(a[k]).tobytes() == np.asarray(b[k]).tobytes() for k in a),
        len(a),
        "entries",
    )
a = run("py.ndimage.binary")
g = np.indices((20, 18, 16)).sum(axis=0)
blob = r.ints(58, (20, 18, 16), 0, 100) < 55 + (g < 25) * 30
mask = (r.ints(59, (20, 18, 16), 0, 4) / 2) > 0.5
o = np.zeros(blob.shape, np.float32)
snd.binary_erosion(blob, iterations=3, mask=mask, output=o)
fix = snd.binary_erosion(blob, iterations=3, mask=mask).astype(np.float32)
ref = {
    "erosion.mask.float32.reproduce": o,
    "erosion.mask.float32.fix": fix,
    "erosion.longlong": snd.binary_erosion(blob.astype(np.longlong), iterations=2),
    "dilation.0d": snd.binary_dilation(np.array(True), iterations=2),
}
for k, v in ref.items():
    print(k, a[k].dtype == v.dtype and a[k].tobytes() == v.tobytes())
print(
    "SCI-25 visible:",
    a["erosion.mask.float32.fix"].tobytes() != a["erosion.mask.float32.reproduce"].tobytes(),
)
b = run("py.ndimage.label")
blob = r.ints(60, (17, 15, 13), 0, 100) < 45
of = np.zeros(blob.shape, np.int32, order="F")
n = snd.label(blob, output=of)
lab, _ = snd.label(blob)
w = (r.ints(61, (17, 15, 13), -50, 50) / 4).astype(np.float32)
ob = np.zeros(blob.shape, ">i4")
ob[...] = lab
ref = {
    "label.F": of,
    "label.F.count": np.asarray([n]),
    "label.>i4": ob,
    "sums.uint64": snd.sum_labels(
        w, lab.astype(np.uint64), np.array([0.0, 1.0, 1.5, 3.0, 2.0**40])
    ),
}
for k, v in ref.items():
    print(k, b[k].dtype == v.dtype and b[k].tobytes() == v.tobytes())
print(
    "F numbering differs from C:",
    b["label.F"].tobytes() != np.ascontiguousarray(b["label"].astype(np.int32)).tobytes(),
)
