# SPDX-License-Identifier: Apache-2.0
"""Checks specs/burkardt-spline.md's model (model.py) against FreeSurfer's own functions (the probe
fs_spline_probe of validation/investigations/freesurfer-ranlux-probe, a black box). Prints one line
per group: cases, identical, differing with the first difference. CLOSED to the spec's clean-room
implementer (README.md).

Usage: python -P check.py <fs_spline_probe> [--cases N]
"""

from __future__ import annotations

import itertools
import random
import struct
import subprocess
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import model

PROBE = sys.argv[1]
NCASES = int(sys.argv[sys.argv.index("--cases") + 1]) if "--cases" in sys.argv else 4000
rng = random.Random(20261112)
F = np.float32


def hx(v) -> str:
    return f"{model.b32(F(v)):08x}"


def bits_of(v: float) -> int:
    return struct.unpack("<I", struct.pack("<f", v))[0]


def knots(n: int, kind: str) -> list:
    if kind == "rows":
        return [F(v) for v in sorted(rng.sample(range(0, 256), n))]
    if kind == "ulp":
        start = F(rng.uniform(-100, 100))
        out = [start]
        for _ in range(n - 1):
            out.append(
                np.nextafter(out[-1], F(np.inf))
                if rng.random() < 0.5
                else F(out[-1] + F(rng.uniform(0.01, 3)))
            )
        return out
    if kind == "spread":
        vals = sorted({F(rng.choice([-1, 1]) * 10 ** rng.uniform(-6, 6)) for _ in range(n * 2)})
        return vals[:n] if len(vals) >= n else knots(n, "random")
    vals = sorted({F(rng.uniform(-50, 300)) for _ in range(n * 2)})
    return vals[:n] if len(vals) >= n else knots(n, "rows")


def points(t: list) -> list:
    pts = [F(-np.inf), F(np.inf), F(np.nan), F(-0.0), F(0.0), F(t[0] - 1), F(t[-1] + 1)]
    for v in t:
        pts.append(v)
    for a, b in itertools.pairwise(t):
        pts.append(F(a + (b - a) * F(rng.random())))
    return pts


cases = []
for k in range(NCASES):
    n = rng.choice([2, 2, 3, 4, 5, 8, 13, 20, 40, 80])
    t = knots(n, rng.choice(["rows", "random", "ulp", "spread"]))
    n = len(t)
    yk = rng.choice(["random", "const", "big"])
    if yk == "const":
        y = [F(rng.uniform(0.5, 2))] * n
    elif yk == "big":
        y = [F(rng.uniform(-1e30, 1e30)) for _ in range(n)]
    else:
        y = [F(rng.uniform(0.5, 1.6)) for _ in range(n)]
    ib, ie = rng.randrange(3), rng.randrange(3)
    yb = F(0) if rng.random() < 0.3 else F(rng.uniform(-2, 2))
    ye = F(0) if rng.random() < 0.3 else F(rng.uniform(-2, 2))
    if k % 50 == 0:  # NaN values and end values with distinct payloads, finite knots
        y = list(y)
        y[rng.randrange(n)] = model.f32(0x7FC00000 | rng.randrange(1, 1 << 22))
        y[rng.randrange(n)] = model.f32(0xFFC00000 | rng.randrange(1, 1 << 22))
        yb = model.f32(0x7FC12345)
    if k % 77 == 0:
        y = list(y)
        y[rng.randrange(n)] = F(np.inf)
    cases.append((n, t, y, ib, yb, ie, ye, points(t)))
cases.append((2, [F(0), F(1)], [F(1), F(2)], 0, F(0), 0, F(0), [F(0.5)]))

lines = []
for n, t, y, ib, yb, ie, ye, pts in cases:
    lines.append(
        " ".join(
            [str(n), str(ib), hx(yb), str(ie), hx(ye), str(len(pts))] + [hx(v) for v in t + y + pts]
        )
    )
out = subprocess.run(
    [PROBE, "cases"], input="\n".join(lines) + "\n", capture_output=True, text=True, check=True
).stdout.splitlines()

bad = []
nan_cases = 0
pos = 0
for idx, (n, t, y, ib, yb, ie, ye, pts) in enumerate(cases):
    ypp_line = out[pos].split()
    pos += 1
    val_lines = out[pos : pos + len(pts)]
    pos += len(pts)
    wrap_line = out[pos].split()
    pos += 1
    M = model.spline_cubic_set(n, t, y, ib, yb, ie, ye)
    want = ypp_line[1:]
    got = [hx(v) for v in M]
    if any(v != v for v in y):
        nan_cases += 1
    if want != got:
        bad.append(f"case {idx} ypp: probe {want[:4]} model {got[:4]}")
        continue
    for x, line in zip(pts, val_lines, strict=True):
        v, d1, d2 = model.spline_cubic_value(n, t, x, y, M)
        g = [hx(v), hx(d1), hx(d2)]
        if line.split()[1:] != g:
            bad.append(f"case {idx} x={x!r}: probe {line.split()[1:]} model {g}")
            break
    Mw = model.spline_cubic_set(n, t, y, 1, yb, 1, ye)
    gw = [hx(v) for v in Mw] + [hx(model.spline_cubic_value(n, t, x, y, Mw)[0]) for x in pts]
    if wrap_line[1:] != gw:
        bad.append(f"case {idx} wrappers differ")
print(
    f"spline cases: {len(cases)} cases ({nan_cases} with NaN values), {len(cases) - len(bad)} identical, {len(bad)} differ"
    + (f"; first: {bad[0]}" if bad else "")
)

# d3: refused systems and general ones
systems = []
for k in range(1000):
    n = rng.choice([1, 2, 3, 5, 9, 30])
    a = [F(rng.uniform(-3, 3)) for _ in range(3 * n)]
    b = [F(rng.uniform(-5, 5)) for _ in range(n)]
    if k % 10 == 0:
        a[1 + 3 * rng.randrange(n)] = F(-0.0) if k % 20 == 0 else F(0.0)
    systems.append((n, a, b))
lines = [" ".join([str(n)] + [hx(v) for v in a + b]) for n, a, b in systems]
out = subprocess.run(
    [PROBE, "d3"], input="\n".join(lines) + "\n", capture_output=True, text=True, check=True
).stdout.splitlines()
bad = []
for (n, a, b), line in zip(systems, out, strict=True):
    L = [F(0)] + [a[2 + (i - 1) * 3] for i in range(1, n)]
    D = [a[1 + i * 3] for i in range(n)]
    U = [a[0 + (i + 1) * 3] for i in range(n - 1)] + [F(0)]
    x = model.d3_np_fs(n, L, D, U, b)
    g = "null" if x is None else "x " + " ".join(hx(v) for v in x)
    if line != g:
        bad.append(f"n={n}: probe {line[:40]} model {g[:40]}")
print(
    f"d3 systems: {len(systems)}, {len(systems) - len(bad)} identical, {len(bad)} differ"
    + (f"; first: {bad[0]}" if bad else "")
)

# the messages (E1 to E5), standard output byte for byte and the exit status
MSG = [
    ("1", "1", "1", ["0"], ["1"]),
    ("3", "1", "1", ["0", "1", "1"], ["1", "2", "3"]),
    ("3", "1", "1", ["0", "2", "1.5"], ["1", "2", "3"]),
    ("3", "1", "1", ["0", "nan", "2"], ["1", "2", "3"]),
    ("3", "1", "1", ["1e-7", "0.1", "inf"], ["1", "2", "3"]),
    ("3", "7", "1", ["0", "1", "2"], ["1", "2", "3"]),
    ("3", "1", "-2", ["0", "1", "2"], ["1", "2", "3"]),
    ("3", "5", "9", ["0", "1", "2"], ["1", "2", "3"]),
    ("2", "1", "1", ["0", "1.4012984643e-45"], ["1", "2"]),
]
bad = []
for n, ib, ie, t, y in MSG:
    r = subprocess.run([PROBE, "set1", n, ib, ie, *t, *y], capture_output=True, text=True)
    try:
        M = model.spline_cubic_set(
            int(n), [F(float(v)) for v in t], [F(float(v)) for v in y], int(ib), 0, int(ie), 0
        )
        g_out, g_code = "ypp" + "".join(f" {hx(v)}" for v in M) + "\n", 0
    except model.SplineError as e:
        g_out, g_code = e.text, 1
    if (r.stdout, r.returncode) != (g_out, g_code):
        bad.append(
            f"set1 {n} {ib} {ie} {t}: probe {r.stdout!r} {r.returncode} model {g_out!r} {g_code}"
        )
print(
    f"messages: {len(MSG)}, {len(MSG) - len(bad)} identical, {len(bad)} differ"
    + (f"; first: {bad[0]}" if bad else "")
)
