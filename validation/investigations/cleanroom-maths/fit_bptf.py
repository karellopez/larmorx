# SPDX-License-Identifier: Apache-2.0
"""`-bptf`'s arithmetic as FSL evaluates it (findings 15): FSL's outputs of the double series of
the replica suite (``@tsd``, 9 volumes) against two evaluations of §13.2's formulas, the spec's
(weights ``exp(−Δ² / (2 s²))``, ``C = Σ w Δ²``) and FSL's (``exp(−0.5 Δ² / (s s))``,
``C = Σ (w Δ) Δ``, ``D = Σ (w Δ) x``), counting the values each gives bit for bit. Python's
``math.exp`` is correctly rounded at these arguments, as larmorx's ``exp``.

```
boxed.sh 4 python -P fsl_outputs.py <dir> "args:bptf_d_hp:@tsd -bptf 2.5 -1 out" \\
  "args:bptf_d_hp33:@tsd -bptf 3.3 -1 out" "args:bptf_d_hp07:@tsd -bptf 0.7 -1 out" \\
  "args:bptf_d_lp:@tsd -bptf -1 1 out" "args:bptf_d_lp15:@tsd -bptf -1 1.5 out" \\
  "args:bptf_d_lp07:@tsd -bptf -1 0.7 out" bptf/double
python -P fit_bptf.py <dir>
```
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np

PROBES = (
    ("bptf_d_hp", 2.5, -1.0),
    ("bptf_d_hp33", 3.3, -1.0),
    ("bptf_d_hp07", 0.7, -1.0),
    ("bptf_d_lp", -1.0, 1.0),
    ("bptf_d_lp15", -1.0, 1.5),
    ("bptf_d_lp07", -1.0, 0.7),
    ("bptf_double", 2.5, 1.0),
)


def plain_sum(values) -> float:
    """A left-to-right sum (Python's ``sum`` of floats is compensated since 3.12)."""
    total = 0.0
    for v in values:
        total += v
    return total


def weight(d: int, s: float, fsl: bool) -> float:
    return math.exp(-0.5 * (d * d) / (s * s)) if fsl else math.exp(-(d * d) / (2.0 * s * s))


def high_pass(v: list[float], s: float, fsl: bool) -> list[float]:
    m, n = int(3 * s), len(v)
    out, c0 = [0.0] * n, None
    for t in range(n):
        a = b = c = d = nn = 0.0
        for dd in range(-m, m + 1):
            if 0 <= t + dd < n:
                w, x = weight(dd, s, fsl), v[t + dd]
                a += w * dd
                b += w * x
                c += (w * dd) * dd if fsl else w * (dd * dd)
                d += (w * dd) * x
                nn += w
        den = c * nn - a * a
        if den == 0:
            out[t] = v[t]
            continue
        ct = (b * c - a * d) / den
        c0 = ct if c0 is None else c0
        out[t] = c0 + v[t] - ct
    mean = plain_sum(out) / n
    return [o - mean for o in out]


def low_pass(v: list[float], s: float, fsl: bool) -> list[float]:
    m, n = int(20 * s) + 2, len(v)
    total = plain_sum(weight(dd, s, fsl) for dd in range(-m, m + 1))
    out = []
    for t in range(n):
        sw = swx = 0.0
        for dd in range(-m, m + 1):
            if 0 <= t + dd < n:
                w = weight(dd, s, fsl) / total
                sw += w
                swx += w * v[t + dd]
        out.append(swx / sw)
    return out


def main(argv: list[str]) -> int:
    import nibabel as nib

    base = Path(argv[0])
    for case, hp, lp in PROBES:
        d = base / case
        if not (d / "out.nii.gz").exists():
            print(f"{case}: not made (run fsl_outputs.py first)")
            continue
        x = np.asarray(nib.load(str(d / "tsd.nii.gz")).dataobj).astype(np.float64)
        f = np.asarray(nib.load(str(d / "out.nii.gz")).dataobj)
        nt = x.shape[3]
        xs, fs = x.reshape(-1, nt, order="F"), f.reshape(-1, nt, order="F")
        counts = {}
        for fsl in (False, True):
            same = 0
            for k in range(xs.shape[0]):
                v = list(xs[k])
                if hp > 0:
                    v = high_pass(v, hp, fsl)
                if lp > 0:
                    v = low_pass(v, lp, fsl)
                same += int(np.count_nonzero(np.array(v).view(np.int64) == fs[k].view(np.int64)))
            counts["FSL's order" if fsl else "the spec's form"] = same
        print(f"{case} (-bptf {hp:g} {lp:g}): of {xs.size} values, bit for bit: {counts}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
