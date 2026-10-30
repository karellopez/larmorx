# SPDX-License-Identifier: Apache-2.0
"""SCI-05: does ``scipy.ndimage.map_coordinates`` (mode ``nearest``, ``prefilter=False``) give
other values in repeated calls on the same inputs, and at which coordinates?

Usage: ``<python> -P sci05_probe.py [--repeats N] [--order K ...]`` with the interpreter of the
environment to probe (the shared ``.venv``: SciPy 1.15.3; ``oracles/python-newest``: SciPy
1.18.1). The inputs are the resample-series suite's ``ndimage/nearest-order<K>`` inputs
(``parity.resample_series._ndimage_inputs`` with its seed), regenerated here with the same
generator so that the probe needs only numpy and SciPy. Each call is repeated ``N`` times in
this process, a fresh copy of the inputs each time; the output values that differ between the
repetitions are listed with their coordinates, the distinct bit patterns of each and whether
every repetition gave a NaN there. Prints one JSON object.
"""

from __future__ import annotations

import argparse
import json
import sys

import numpy as np
import scipy
from scipy import ndimage as ndi

#: ``resample_series.MODES``, in its order (the case's seed uses the mode's index).
MODES = (
    "grid-constant",
    "constant",
    "nearest",
    "mirror",
    "reflect",
    "grid-mirror",
    "wrap",
    "grid-wrap",
)


def inputs(rng: np.random.Generator):
    """``resample_series._ndimage_inputs``, the same draws in the same order."""
    for shape in [(9,), (7, 5), (8, 6, 5), (1, 4, 3), (2, 3, 2), (13, 11, 9)]:
        for dtype in (np.float32, np.float64, np.int16):
            data = (rng.normal(size=shape) * 100).astype(dtype)
            n = 600
            hi = np.array(shape, float)[:, None]
            coords = rng.uniform(-7, hi + 6, size=(len(shape), n))
            coords[:, :60] = np.round(coords[:, :60])
            coords[:, 60:120] = np.round(coords[:, 60:120]) + 0.5
            coords[:, 120] = np.nan
            coords[:, 121] = -1e-17
            coords[:, 122] = hi[:, 0] - 1 + 1e-12
            coords[:, 123] = -0.0
            yield shape, data, coords


def probe(order: int, repeats: int, modes_index) -> list[dict]:
    rng = np.random.default_rng(1000 + order * 17 + modes_index)
    found = []
    for shape, data, coords in inputs(rng):
        for prefilter in (True, False):
            for cval in (0.0, -3.5):
                for out in (None, np.float32, np.float64):
                    kw = dict(order=order, mode="nearest", cval=cval, prefilter=prefilter)
                    runs = [
                        ndi.map_coordinates(data.copy(), coords.copy(), output=out, **kw)
                        for _ in range(repeats)
                    ]
                    bits = [np.ascontiguousarray(r).view(f"u{r.dtype.itemsize}") for r in runs]
                    differ = np.zeros(bits[0].shape, bool)
                    for b in bits[1:]:
                        differ |= b != bits[0]
                    if differ.any():
                        idx = np.flatnonzero(differ)
                        found.append(
                            {
                                "shape": shape,
                                "dtype": np.dtype(data.dtype).name,
                                "output": np.dtype(out or data.dtype).name,
                                "prefilter": prefilter,
                                "cval": cval,
                                "differing": int(idx.size),
                                "points": [
                                    {
                                        "index": int(i),
                                        "coords": [float(c) for c in coords[:, i]],
                                        "bits": sorted({f"{int(b.ravel()[i]):x}" for b in bits}),
                                        "all_nan": bool(all(np.isnan(r.ravel()[i]) for r in runs)),
                                    }
                                    for i in idx[:8]
                                ],
                            }
                        )
    return found


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("--repeats", type=int, default=20)
    p.add_argument("--order", type=int, nargs="+", default=[0, 1, 2, 3, 4, 5])
    a = p.parse_args(argv)
    result = {
        "scipy": scipy.__version__,
        "numpy": np.__version__,
        "repeats": a.repeats,
        "orders": {str(k): probe(k, a.repeats, MODES.index("nearest")) for k in a.order},
    }
    print(json.dumps(result, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
