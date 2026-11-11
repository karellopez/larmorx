# SPDX-License-Identifier: Apache-2.0
"""How close nitime's AR(1) estimate under nipype's DVARS comes to 1, and when it reaches or
passes it (PIPE-46, PIPE-47).

nipype computes ``AR_est_YW(regress_poly(0, x)[0].astype(float32), 1)``: ``r1 / r0`` of the
float32 FFT autocorrelation. The Yule-Walker estimate of the biased autocorrelation is below 1
in exact arithmetic (Cauchy-Schwarz), so a value of 1 or more is the FFT's rounding. Sections:

1. ``search``: 4,000 seeded structured series (ramps, steps, double random walks, quadratics,
   slow sines, tiny and huge amplitudes; ``T`` 2 to 3000): the largest estimate (measured
   2026-11-12 with numpy 2.3.5 and SciPy 1.15.3: 0.99986, a double random walk of 3000
   volumes; none reaches 1).
2. ``sines``: one full period of a sine (the smoothest zero-mean series that vanishes at both
   ends) of 600 to 40,000 volumes: the estimate reaches 1.0 exactly from 20,000 volumes (an
   AR(1) of 1 makes the predicted SD 0: PIPE-46's division by zero) and 1.0000001 at 30,000 and
   40,000 (the square root of a negative number: PIPE-47's NaN); measured: of the 64 cases of
   20,000 volumes and more, 22 at 1.0 and 2 above.

Run: ``python -P probe_ar1_above_one.py [search|sines]`` (both by default; a few minutes).
"""

import sys
import warnings

import numpy as np
from nipype.algorithms.confounds import regress_poly
from nitime.algorithms import AR_est_YW


def ar1(series: np.ndarray) -> np.float32:
    m = series.astype(np.float32)[None, :]
    r = regress_poly(0, m, remove_mean=True)[0].astype(np.float32)[0]
    return AR_est_YW(r, 1)[0][0]


def search() -> None:
    rng = np.random.default_rng(1)

    def gen(t: int, kind: str) -> np.ndarray:
        x = np.arange(t, dtype=np.float64)
        return {
            "ramp": lambda: 1000 + x * rng.uniform(1e-3, 10),
            "step": lambda: 1000 + (x >= rng.integers(1, t)) * rng.uniform(1e-3, 100),
            "smooth": lambda: 1000 + np.cumsum(np.cumsum(rng.standard_normal(t))),
            "quad": lambda: 1000 + (x - t / 2) ** 2 * rng.uniform(1e-4, 1),
            "sine": lambda: 1000 + 50 * np.sin(2 * np.pi * x / (t * rng.uniform(2, 20))),
            "tiny": lambda: 1000 + np.cumsum(rng.standard_normal(t)) * 1e-4,
            "big": lambda: 1e30 * (1 + np.cumsum(rng.standard_normal(t)) * 1e-3),
            "sub": lambda: 1e-30 * np.cumsum(rng.standard_normal(t)),
        }[kind]()

    best, where, hits = -2.0, None, 0
    for _ in range(4000):
        t = int(rng.choice([2, 3, 4, 5, 8, 10, 33, 64, 129, 164, 280, 600, 1200, 3000]))
        kind = str(rng.choice(["ramp", "step", "smooth", "quad", "sine", "tiny", "big", "sub"]))
        try:
            a = float(ar1(gen(t, kind)))
        except ValueError:  # a non-finite autocorrelation (the huge series)
            continue
        if a > best:
            best, where = a, (t, kind)
        hits += a >= 1
    print(f"search: largest AR(1) {best!r} at {where}; {hits} of 4000 at 1 or above")


def sines() -> None:
    counts = {"below 1": 0, "1": 0, "above 1": 0}
    for t in (600, 3000, 10000, 20000, 25000, 30000, 40000):
        for amp in (1.0, 10.0, 100.0, 1000.0):
            for phase in (0.0, 0.01, 0.05, 0.1):
                x = np.arange(t)
                a = ar1(1000 + amp * np.sin(2 * np.pi * x / t + phase))
                k = "above 1" if a > 1 else ("1" if a == 1 else "below 1")
                counts[k] += 1
                if a >= 1:
                    print(f"  T={t} amplitude {amp} phase {phase}: {a!r}")
    print("sines:", counts)


if __name__ == "__main__":
    warnings.simplefilter("ignore")
    for name in sys.argv[1:] or ["search", "sines"]:
        {"search": search, "sines": sines}[name]()
