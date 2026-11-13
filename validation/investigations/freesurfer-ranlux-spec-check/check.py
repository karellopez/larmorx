# SPDX-License-Identifier: Apache-2.0
"""Checks specs/ranlux.md's model (model.py) against FreeSurfer's own functions (the probe of
validation/investigations/freesurfer-ranlux-probe, a black box) and prints one line per group:
cases, identical, differing (with the first difference). CLOSED to the spec's clean-room
implementer (README.md).

Usage: python -P check.py <fs_rng_probe> [--quick]
"""

from __future__ import annotations

import hashlib
import math
import random
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import model

print = __import__("functools").partial(print, flush=True)

PROBE = sys.argv[1]
QUICK = "--quick" in sys.argv

TABLE = {  # §2.5: seed -> SHA-256 of 100,000 values
    53: "b40812f1162500bca3938e3148b47358a9300d4c7be994cd7e03163e9115541d",
    0: "12d4f2cdf218304041625d02416dc679eba7d14bf3ca30ff282fadce55a1e3b7",
    1: "391c821d14bb056db5e2302e22202721e758206eb7ebe90f8a74ca38c0c20ead",
    2: "87c0b8ce736a3e5633677ec80d807728a0fa62f9fed7d39bc7f2378be831979d",
    1234: "edb3efc9bd60f606ead133110dc82c15ead6deb1360cb9c113b9c5fa6fd08338",
    2147483562: "fdde7d836524bd89f00fde457d8039ac5012b5ebbba15e295ababb6db9e2a2d5",
    4294967295: "cd8cf49f25873643013cc8df819c2571be182e31330a7e99c6c41c5ec6f3ece5",
    4294967296: "22483b9dba48de1a1fa4d18e2325de2ec38abc68e0f1551a53d915c6ffa99f46",
    2**63 - 1: "6a65e3a99aed14c45fd3ff53bf5c59c3f0b8609eb81f0408447c659723ce3621",
    2**63: "17809d02bbe72bcdf755d5315c47475de9cfb6c14080ab911cbbd9c7b64e8749",
    2**64 - 53: "bae173cae0f216f7e3af02218d6c93aa33f7078757e2646ba1fb99101af16f67",
}


def probe(*args: str) -> list[str]:
    out = subprocess.run([PROBE, *args], capture_output=True, text=True, check=True, timeout=600)
    return out.stdout.split()


def seed_arg(s: int) -> str:
    return str(s % (1 << 64))


def report(name: str, n: int, bad: list[str]) -> None:
    print(
        f"{name}: {n} cases, {n - len(bad)} identical, {len(bad)} differ"
        + (f"; first: {bad[0]}" if bad else "")
    )


# 1. The generator
bad = []
n = 0
for s, sha in TABLE.items():
    g = model.Generator(s)
    text = "".join(f"{g.next()}\n" for _ in range(100000 if not QUICK else 1000))
    n += 1
    if not QUICK and hashlib.sha256(text.encode()).hexdigest() != sha:
        bad.append(f"seed {s}: the model's hash differs from §2.5's")
    want = probe("raw", seed_arg(s), "1000")
    got = text.split()[:1000]
    if want != got:
        bad.append(f"seed {s}: first 1000 values differ from the probe")
report("generator, §2.5's seeds", n, bad)

rng = random.Random(20261112)
seeds = (
    [rng.randrange(1 << 64) for _ in range(150 if not QUICK else 10)]
    + [rng.randrange(-(1 << 31), 1 << 31) for _ in range(150 if not QUICK else 10)]
    + [
        2**31 - 1,
        -(2**31),
        2**53 - 1,
        2**53 + 1,
        2**63 - 2,
        2**63 + 1,
        2147483564,
        9438321819,
        9438321820,
        -9438321819,
        -9438321820,
        2147483563,
        -2147483563,
        4294967126,
        10737417815,
    ]
)
bad = []
for s in seeds:
    g = model.Generator(s)
    got = [str(g.next()) for _ in range(3000)]
    if probe("raw", seed_arg(s), "3000") != got:
        bad.append(f"seed {s}")
report("generator, random and boundary seeds (3000 values each)", len(seeds), bad)

# FS-101's default (the model's own; no oracle): the first values of the corrected stream
g = model.Generator(2147483563, reproduce=False)
first = [g.next() for _ in range(12)]
print(f"FS-101 default stream for 2147483563, first 12: {' '.join(map(str, first))}")

# 2. The distributions
NAN2 = model.from_bits(0x7FF8000000001234)
CASES = [
    ("uni", 0, 0),
    ("uni_pos", 0, 0),
    ("flat", 0.0, 1.0),
    ("flat", -3.5, 7.25),
    ("flat", 5.0, 5.0),
    ("flat", 2.0, -1.0),
    ("flat", -0.0, -0.0),
    ("flat", math.nan, 1.0),
    ("flat", 1.0, math.nan),
    ("flat", math.inf, 1.0),
    ("flat", 0.0, math.inf),
    ("gaussian", 1.0, 0),
    ("gaussian", 2.5, 0),
    ("gaussian", -1.0, 0),
    ("gaussian", 0.0, 0),
    ("gaussian", 1e-300, 0),
    ("gaussian", 1e300, 0),
    ("gaussian", math.nan, 0),
    ("gaussian", math.inf, 0),
    ("ugauss", 0, 0),
    ("exponential", 1.0, 0),
    ("exponential", 0.75, 0),
    ("exponential", 0.0, 0),
    ("exponential", -2.0, 0),
    ("exponential", math.nan, 0),
    ("chisq", 0.0, 0),
    ("chisq", 1.0, 0),
    ("chisq", 3.0, 0),
    ("chisq", 5.0, 0),
    ("chisq", 24.0, 0),
    ("chisq", 25.0, 0),
    ("chisq", 30.0, 0),
    ("chisq", 1000.0, 0),
    ("fdist", 4.0, 9.0),
    ("fdist", 3.0, 25.0),
    ("fdist", 1.0, 1.0),
    ("fdist", 30.0, 50.0),
    ("fdist", 0.0, 3.0),
    ("tdist", 0.0, 0),
    ("tdist", 0.5, 0),
    ("tdist", 1.0, 0),
    ("tdist", 1.5, 0),
    ("tdist", 2.0, 0),
    ("tdist", 2.25, 0),
    ("tdist", 3.0, 0),
    ("tdist", 7.0, 0),
    ("tdist", 30.0, 0),
    ("tdist", 1000.0, 0),
    ("tdist", math.nan, 0),
    ("gamma_int", 0, 0),
    ("gamma_int", 1, 0),
    ("gamma_int", 11, 0),
    ("gamma_int", 12, 0),
    ("gamma_large", 12.0, 0),
    ("gamma_large", 40.0, 0),
    ("gamma_frac", 0.001, 0),
    ("gamma_frac", 0.5, 0),
    ("gamma_frac", 0.999, 0),
]
for a in (
    0.0,
    1e-300,
    0.001,
    0.4,
    0.5,
    0.999,
    1.0,
    2.0,
    2.5,
    7.3,
    11.0,
    11.5,
    12.0,
    12.5,
    25.0,
    31.6,
    1000.25,
    1e6,
    2**32 - 0.5,
):
    for b in (1.0, 2.0, 0.5, -1.0):
        CASES.append(("gamma", a, b))
# FS-100 under reproduce: the finite results
for a in (2**32 + 0.5, 2**32 + 12.25, 2.0**33 + 8, 1e300, math.nan, math.inf):
    CASES.append(("gamma", a, 1.0))

ndraw = 2000 if not QUICK else 100
bad = []
n = 0
for seed in (53, 1, 4294967296) if not QUICK else (53,):
    for name, p1, p2 in CASES:
        n += 1
        g = model.Generator(seed)
        f = model.DRAWS[name]
        try:
            got = [
                f"{model.bits(f(g, float(p1), float(p2)) if name != 'gamma_int' else f(g, p1, p2)):016x}"
                for _ in range(ndraw)
            ]
        except model.NeverReturns as e:
            bad.append(f"{name}({p1}, {p2}) seed {seed}: the model says it never returns ({e})")
            continue
        want = probe("dist", name, str(seed), str(ndraw), repr(float(p1)), repr(float(p2)))
        if want != got:
            k = next(i for i, (w, h) in enumerate(zip(want, got, strict=False)) if w != h)
            bad.append(f"{name}({p1}, {p2}) seed {seed}: draw {k}: probe {want[k]} model {got[k]}")
report("distributions (draws x seeds)", n, bad)

bad = []
for seed in (53, 1, 4294967296):
    g = model.Generator(seed)
    got = [f"{model.bits(v):016x}" for v in model.mixed(g, 14000 if not QUICK else 1400)]
    want = probe("mixed", str(seed), str(len(got)))
    if want != got:
        k = next(i for i, (w, h) in enumerate(zip(want, got, strict=False)) if w != h)
        bad.append(f"seed {seed}: draw {k}")
report("mixed rotation", 3, bad)

# FS-101 under reproduce: the zero stream and the flat draw
bad = []
for s in (2147483563, -2147483563):
    g = model.Generator(s)
    got = [f"{model.bits(model.flat(g, 1.5, -2.0)):016x}" for _ in range(50)]
    if probe("dist", "flat", seed_arg(s), "50", "1.5", "-2.0") != got:
        bad.append(f"seed {s}")
report("FS-101 reproduce: flat on the zero stream", 2, bad)
