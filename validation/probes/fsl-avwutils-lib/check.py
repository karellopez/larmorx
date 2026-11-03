# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx-nc-core's Armadillo extrema and fsl-cprob functions with FSL 6.0.7.23's own.

Makes two inputs (seeded): ``arma.txt``, vectors of 1 to 101 elements holding signed zeros, NaN,
infinities, ties and random values (Armadillo 14.2.1's ``max``/``min`` keep two partial extrema,
which decide the sign of a zero result); ``cprob.txt``, the arguments of ``ndtr``, ``ndtri``,
``erf`` and ``erfc`` fslmaths meets (``-ztop`` of statistics stored as floats, ``-ptoz`` of
p-values, ``-ranknorm``'s ``(k - 0.5) / n``, ``-randn``'s ``drand() - 1``), each function's
branch limits and the special values. Runs ``probe`` (built by ``build.sh``) on each and the
replica's side (the ignored test ``avwutils_probe`` of ``crates-nc/larmorx-nc-core``), and counts
the differing lines. Run inside a memory box (``scripts/orchestration/boxed.sh``). See README.md.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]


def bits(x: float) -> str:
    return f"{np.float64(x).view(np.uint64):016x}"


def arma_vectors(rng) -> list[list[float]]:
    out: list[list[float]] = [
        [0.0, -0.0],
        [-0.0, 0.0],
        [0.0, -0.0, -1.0],
        [-0.0, 0.0, -1.0],
        [-1.0, -0.0, -2.0, 0.0],
        [np.nan],
        [np.nan, np.nan],
        [np.nan, 1.0, 2.0],
        [1.0, np.nan, 2.0, np.nan, 0.5],
        [np.inf, -np.inf],
        [-np.inf, np.nan],
        [5e-324, -5e-324, 0.0],
        [3.0],
        [3.0, 3.0, 3.0],
    ]
    special = np.array([0.0, -0.0, np.nan, np.inf, -np.inf, 1.0, -1.0, 2.0])
    for n in (*range(1, 10), 16, 33, 64, 101):
        for _ in range(40):
            kind = rng.integers(0, 3)
            if kind == 0:
                v = rng.choice(special, n)
            elif kind == 1:
                v = rng.integers(-2, 3, n).astype(float)
                v[rng.random(n) < 0.3] *= -0.0 if rng.random() < 0.5 else 0.0
            else:
                v = rng.standard_normal(n).astype(np.float32).astype(float)
                v[rng.random(n) < 0.1] = np.nan
            out.append(list(v))
    return out


def cprob_values(rng) -> np.ndarray:
    sqrth = 7.07106781186547524401e-1
    edges = []
    for b in (1.0 / sqrth, 8.0, 8.0 / sqrth, 1.0, 26.5, 37.5, 37.6, 38.6, 0.135335283236612691):
        x = np.float64(b)
        edges += [x, np.nextafter(x, 0), np.nextafter(x, np.inf), -x]
    special = [0.0, -0.0, 1.0, -1.0, 0.5, np.nan, np.inf, -np.inf, 5e-324, 2.2250738585072014e-308]
    special += [1 - 2.0**-53, 2.0**-53, 1e-300, 1 - 1e-16, 2.0, -0.5]
    z_float = (rng.standard_normal(60000) * 3).astype(np.float32).astype(float)
    wide = rng.uniform(-45, 45, 30000).astype(np.float32).astype(float)
    p_float = rng.random(40000).astype(np.float32).astype(float)
    tails = 10.0 ** rng.uniform(-320, -1, 20000)
    upper = 1 - 10.0 ** rng.uniform(-16, -1, 10000)
    ranks = np.concatenate(
        [(np.arange(1, n + 1) - 0.5) / n for n in (1, 2, 3, 7, 10, 64, 150, 601)]
    )
    drand = rng.random(20000)
    return np.concatenate(
        [edges, special, z_float, wide, p_float, tails, upper, ranks, drand]
    ).astype(float)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--probe", required=True, type=Path)
    ap.add_argument("--work", required=True, type=Path)
    args = ap.parse_args()
    work: Path = args.work
    work.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(20261103)
    vectors = arma_vectors(rng)
    (work / "arma.txt").write_text(
        "".join(" ".join(bits(x) for x in v) + "\n" for v in vectors), encoding="utf-8"
    )
    values = cprob_values(rng)
    (work / "cprob.txt").write_text("".join(bits(x) + "\n" for x in values), encoding="utf-8")
    for kind in ("arma", "cprob"):
        with open(work / f"{kind}.fsl", "w", encoding="utf-8") as out:
            subprocess.run([args.probe, kind, str(work / f"{kind}.txt")], stdout=out, check=True)
    subprocess.run(
        [
            "cargo",
            "test",
            "--release",
            "-q",
            "-p",
            "larmorx-nc-core",
            "--test",
            "avwutils_probe",
            "--",
            "--ignored",
        ],
        cwd=REPO / "crates-nc",
        env=dict(os.environ, AVW_PROBE_ROOT=str(work)),
        check=True,
        stdout=subprocess.DEVNULL,
    )
    status = 0
    for kind, n in (("arma", len(vectors)), ("cprob", len(values))):
        a = (work / f"{kind}.fsl").read_text(encoding="utf-8").splitlines()
        b = (work / f"{kind}.replica").read_text(encoding="utf-8").splitlines()
        diff = [i for i, (x, y) in enumerate(zip(a, b, strict=False)) if x != y]
        nd = len(diff) + abs(len(a) - len(b))
        tags: dict[str, int] = {}
        for i in diff:
            t = a[i].split(" ")[0] or "<message>"
            tags[t] = tags.get(t, 0) + 1
        print(f"{kind}: {n} inputs, {len(a)} lines, {nd} differ {tags or ''}")
        for i in diff[:5]:
            print(f"  line {i + 1}: FSL {a[i]!r} replica {b[i]!r}")
        status |= nd != 0
    return status


if __name__ == "__main__":
    sys.exit(main())
