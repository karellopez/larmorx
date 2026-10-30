# SPDX-License-Identifier: Apache-2.0
"""How FSL rounds the robust range's thresholds (black-box): for each image, FSL's printed
``thresh2`` and ``thresh98`` (``bet2 <image> out -n -v``) against candidate roundings of
``lo + k (hi - lo) / 1000``. ``python -P range_probe.py <images...>``."""

from __future__ import annotations

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import fslrun

f = np.float32


def candidates(lo, hi, k):
    w64 = float(hi) - float(lo)
    w32 = f(hi - lo)
    return {
        "A f(lo)+f(k*w64/1000)": float(f(lo + f(k * w64 / 1000))),
        "B float: lo+(k*w32)/1000": float(lo + (f(k) * w32) / f(1000)),
        "C lo+f(k*w32/1000)": float(f(lo + f(k * float(w32) / 1000))),
        "D lo+k*f(w32/1000)": float(f(lo + f(k) * f(w32 / f(1000)))),
        "E f(lo+k*w64/1000)": float(f(float(lo) + k * w64 / 1000)),
        "F double": float(lo) + k * w64 / 1000,
        "G f(lo+k*(w64/1000))": float(f(float(lo) + k * (w64 / 1000))),
        "H lo+f(k*f(w32/1000))": float(f(lo + f(k * float(f(w32 / f(1000)))))),
    }


def main(argv):
    for path in argv:
        p = fslrun.run("bet2", [path, "out", "-n", "-v"], Path("/tmp") / "lx-range-probe_")
        words = p.stdout.split()
        t2s, t98s = words[3], words[7]
        a = np.asanyarray(nib.load(path).dataobj, dtype=np.float32)
        if a.ndim == 4:
            a = a[..., 0]
        lo, hi = f(a.min()), f(a.max())
        w = float(hi) - float(lo)
        b = round((float(t2s) - float(lo)) / w * 1000)
        u1 = round((float(t98s) - float(lo)) / w * 1000)
        print(Path(path).name, "FSL", t2s, t98s, "bins", b, u1)
        for name in candidates(lo, hi, b):
            v2 = candidates(lo, hi, b)[name]
            v98 = candidates(lo, hi, u1)[name]
            ok = ("%g" % v2) == t2s and ("%g" % v98) == t98s
            print(f"   {'==' if ok else '  '} {name}: {v2!r} {v98!r}")


if __name__ == "__main__":
    main(sys.argv[1:])
