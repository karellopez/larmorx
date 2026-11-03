# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx-nc-core's blocked LU solve and ``AᵀA`` with FSL 6.0.7.23's own OpenBLAS.

Generates the cases (seeded): ``dgesv`` of general, nearly singular-pivot, exactly singular and
spline-normal-equation systems of 9 to 120 unknowns with 2 to 70 right-hand sides, and
``dsyrk('U', 'T')`` with inner dimensions from 49 to 700 (one, a halved and several blocks);
runs ``lapack_probe`` (built by ``build.sh``) on each and the replica's side (the ignored test
``lapack_probe`` of ``crates-nc/larmorx-nc-core``), and counts the differing lines. See
README.md. Run inside a memory box (``scripts/orchestration/boxed.sh``).
"""

from __future__ import annotations

import argparse
import os
import struct
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]


def gesv_case(path: Path, a: np.ndarray, b: np.ndarray) -> None:
    n, nrhs = b.shape
    path.write_bytes(
        struct.pack("<III", 0, n, nrhs)
        + np.asarray(a, "<f8").T.copy().tobytes()
        + np.asarray(b, "<f8").T.copy().tobytes()
    )


def syrk_case(path: Path, a: np.ndarray) -> None:
    k, n = a.shape
    path.write_bytes(struct.pack("<III", 1, k, n) + np.asarray(a, "<f8").T.copy().tobytes())


def spline_normal(isz: int, csz: int, rng) -> np.ndarray:
    """A banded normal-equation matrix like splinefield::Set's (ASᵀAS)."""
    a = np.zeros((isz, csz))
    for i in range(csz):
        c = (i - 1) * isz / max(csz - 3, 1)
        for j in range(isz):
            t = abs(j - c) / max(isz / max(csz - 3, 1), 1.0)
            if t < 2:
                a[j, i] = (2 - t) ** 3 / 6 if t >= 1 else 2 / 3 - t * t + t**3 / 2
    s = np.zeros((isz, csz))
    s[0, 0], s[0, 1], s[0, -1] = 2, -1, -1
    s[-1, 0], s[-1, -2], s[-1, -1] = -1, -1, 2
    as_ = np.vstack([a, 0.005 * s])
    return as_.T @ as_ + 1e-3 * rng.random((csz, csz)) * 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe", required=True, help="the lapack_probe binary")
    ap.add_argument("--work", required=True, help="a scratch directory for the cases")
    args = ap.parse_args()
    work = Path(args.work)
    work.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(4242)
    names = []
    sizes = [9, 16, 17, 20, 33, 34, 40, 41, 49, 56, 57, 64, 67, 99, 120]
    for n in sizes:
        for nrhs in (2, 9, 25, 70):
            a = rng.normal(0, 1, (n, n))
            b = rng.normal(0, 1, (n, nrhs))
            name = f"gesv_n{n}_r{nrhs}"
            gesv_case(work / f"{name}.case", a, b)
            names.append(name)
        # A diagonally dominant one (no interchanges), a spline normal matrix, a matrix with a
        # tiny first pivot and an exactly singular one.
        a = rng.normal(0, 1, (n, n)) + n * np.eye(n)
        gesv_case(work / f"gesv_n{n}_dd.case", a, rng.normal(0, 1, (n, 5)))
        names.append(f"gesv_n{n}_dd")
        gesv_case(
            work / f"gesv_n{n}_spline.case", spline_normal(2 * n, n, rng), rng.normal(0, 1, (n, 64))
        )
        names.append(f"gesv_n{n}_spline")
        a = rng.normal(0, 1, (n, n))
        a[:, 0] *= 1e-310
        gesv_case(work / f"gesv_n{n}_tiny.case", a, rng.normal(0, 1, (n, 3)))
        names.append(f"gesv_n{n}_tiny")
        a = rng.normal(0, 1, (n, n))
        a[:, n // 2] = a[:, 1] * 2.0
        a[n // 2, :] = 0.0
        gesv_case(work / f"gesv_n{n}_sing.case", a, rng.normal(0, 1, (n, 2)))
        names.append(f"gesv_n{n}_sing")
    for k, n in [
        (49, 7),
        (64, 30),
        (128, 66),
        (256, 70),
        (300, 40),
        (511, 9),
        (512, 10),
        (700, 12),
    ]:
        syrk_case(work / f"syrk_k{k}_n{n}.case", rng.normal(0, 1, (k, n)))
        names.append(f"syrk_k{k}_n{n}")
    env = dict(os.environ, OPENBLAS_NUM_THREADS="1", OPENBLAS_CORETYPE="Haswell")
    for name in names:
        with open(work / f"{name}.fsl", "w") as out:
            subprocess.run(
                [args.probe, str(work / f"{name}.case")], stdout=out, env=env, check=True
            )
    subprocess.run(
        [
            "cargo",
            "test",
            "--release",
            "-q",
            "-p",
            "larmorx-nc-core",
            "--test",
            "lapack_probe",
            "--",
            "--ignored",
        ],
        cwd=REPO / "crates-nc",
        env=dict(env, LP_PROBE_ROOT=str(work)),
        check=True,
        stdout=subprocess.DEVNULL,
    )
    total = 0
    bad_cases = []
    nlines = 0
    for name in names:
        a = (work / f"{name}.fsl").read_text().splitlines()
        b = (work / f"{name}.replica").read_text().splitlines()
        nd = sum(x != y for x, y in zip(a, b, strict=False)) + abs(len(a) - len(b))
        nlines += len(a)
        if nd:
            first = next(
                (i for i, (x, y) in enumerate(zip(a, b, strict=False)) if x != y),
                min(len(a), len(b)),
            )
            bad_cases.append(
                f"{name}: {nd} of {len(a)} (first at line {first + 1}: {a[first] if first < len(a) else '<end>'})"
            )
        total += nd
    print(f"{len(names)} cases, {nlines} lines, {total} differ")
    for line in bad_cases:
        print("  " + line)
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())
