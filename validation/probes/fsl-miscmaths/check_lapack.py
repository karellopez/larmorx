# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx-nc-core's blocked LU solve and ``AᵀA`` with FSL 6.0.7.23's own OpenBLAS.

Generates the cases (seeded): ``dgesv`` of general, nearly singular-pivot, exactly singular and
spline-normal-equation systems of 9 to 120 unknowns with 2 to 70 right-hand sides, and
``dsyrk('U', 'T')`` with inner dimensions from 49 to 700 (one, a halved and several blocks) and
``dgemm('N', 'N')`` with odd and even column counts and more than 512 rows; the symmetric
indefinite solve as Armadillo's ``solve_sym_fast`` calls it (``dsytrf('L')``, ``dsytrs('L')``) on
spline normal equations, indefinite matrices (zero diagonals: 2 × 2 pivots), matrices with zero
columns (exact zeros skipped by ``dsyr``; a singular ``D``) and well-conditioned ones, of 5 to 700
rows with 1 to 110 right-hand sides; and the BLAS routines that solve calls, alone, at the
shapes it calls them (``daxpy`` of any length, ``dger`` with a strided ``y``, ``dsyr('L')``,
``dgemv('T')`` with a strided ``y``, ``dgemv('N')`` with a strided ``x``, ``dgemm('N', 'T')``);
runs ``lapack_probe`` (built by ``build.sh``) on each and the replica's side (the ignored test
``lapack_probe`` of ``crates-nc/larmorx-nc-core``), and counts the differing lines. See
README.md. Run inside a memory box (``scripts/orchestration/boxed.sh``). ``--only sym,blas``
runs only the cases whose names start with those prefixes.
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


def sym_case(path: Path, a: np.ndarray, b: np.ndarray) -> None:
    n, nrhs = b.shape
    path.write_bytes(
        struct.pack("<III", 3, n, nrhs)
        + np.asarray(a, "<f8").T.copy().tobytes()
        + np.asarray(b, "<f8").T.copy().tobytes()
    )


def blas_case(path: Path, op: int, m: int, n: int, *arrays, k: int | None = None) -> None:
    head = struct.pack("<IIII", 4, op, m, n) + (struct.pack("<I", k) if k is not None else b"")
    path.write_bytes(head + b"".join(np.asarray(x, "<f8").T.copy().tobytes() for x in arrays))


def sym_cases(work: Path, rng) -> list[str]:
    """Symmetric systems: spline normal equations (``splinefield::Set``'s), indefinite ones with
    a zero diagonal (2 × 2 pivots), ones with exact zeros (``dsyr`` skips them) or a zero column
    (``D`` singular: ``dsytrf``'s info), diagonally dominant ones (no interchange)."""
    names = []
    for n in [5, 17, 40, 64, 65, 99, 100, 101, 112, 127, 128, 129, 150, 191, 200, 256, 300, 700]:
        nr = [1, 2, 7, 110] if n in (100, 112, 129, 256) else [3]
        for nrhs in nr:
            name = f"sym_n{n}_spline_r{nrhs}"
            sym_case(
                work / f"{name}.case", spline_normal(n + 8, n, rng), rng.normal(0, 1, (n, nrhs))
            )
            names.append(name)
        x = rng.normal(0, 1, (n, n))
        a = x + x.T
        np.fill_diagonal(a, 0.0)
        sym_case(work / f"sym_n{n}_indef.case", a, rng.normal(0, 1, (n, 4)))
        names.append(f"sym_n{n}_indef")
        x = rng.normal(0, 1, (n, n))
        a = x + x.T
        a[rng.random((n, n)) < 0.3] = 0.0
        a = np.tril(a) + np.tril(a, -1).T
        sym_case(work / f"sym_n{n}_zeros.case", a, rng.normal(0, 1, (n, 2)))
        names.append(f"sym_n{n}_zeros")
        a = rng.normal(0, 1, (n, n))
        a = a + a.T + 2 * n * np.eye(n)
        sym_case(work / f"sym_n{n}_dd.case", a, rng.normal(0, 1, (n, 5)))
        names.append(f"sym_n{n}_dd")
        if n >= 64:
            x = rng.normal(0, 1, (n, n))
            a = x + x.T
            a[:, n // 3] = 0.0
            a[n // 3, :] = 0.0
            sym_case(work / f"sym_n{n}_zerocol.case", a, rng.normal(0, 1, (n, 2)))
            names.append(f"sym_n{n}_zerocol")
    return names


def blas_cases(work: Path, rng) -> list[str]:
    names = []
    for m in [1, 5, 15, 16, 17, 31, 32, 33, 63, 64, 100, 255]:
        name = f"blas_axpy_m{m}"
        blas_case(
            work / f"{name}.case", 0, m, 1, [rng.normal()], rng.normal(0, 1, m), rng.normal(0, 1, m)
        )
        names.append(name)
        for n in [1, 3, 9]:
            name = f"blas_ger_m{m}_n{n}"
            blas_case(
                work / f"{name}.case", 1, m, n, rng.normal(0, 1, m), rng.normal(0, 1, (m + 1, n))
            )
            names.append(name)
            name = f"blas_gemvt_m{m}_n{n}"
            blas_case(
                work / f"{name}.case", 3, m, n, rng.normal(0, 1, m), rng.normal(0, 1, (m + 1, n))
            )
            names.append(name)
        if m < 100:
            x = rng.normal(0, 1, m)
            x[rng.random(m) < 0.25] = 0.0
            name = f"blas_syr_m{m}"
            blas_case(
                work / f"{name}.case", 2, m, m, [-rng.random()], x, rng.normal(0, 1, (m + 3, m))
            )
            names.append(name)
    for m, n in [(1, 3), (2, 5), (3, 4), (5, 7), (8, 1), (17, 63), (64, 63), (130, 2), (255, 64)]:
        name = f"blas_gemvn_m{m}_n{n}"
        blas_case(
            work / f"{name}.case",
            4,
            m,
            n,
            rng.normal(0, 1, (m, n)),
            rng.normal(0, 1, n * 5),
            rng.normal(0, 1, m),
        )
        names.append(name)
    for m, n, k in [
        (1, 64, 63),
        (3, 64, 64),
        (6, 13, 63),
        (36, 64, 63),
        (191, 64, 64),
        (636, 64, 64),
        (7, 9, 5),
    ]:
        name = f"blas_gemmnt_m{m}_n{n}_k{k}"
        blas_case(
            work / f"{name}.case",
            5,
            m,
            n,
            rng.normal(0, 1, (m, k)),
            rng.normal(0, 1, (n + 2, k)),
            rng.normal(0, 1, (m, n)),
            k=k,
        )
        names.append(name)
    return names


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe", required=True, help="the lapack_probe binary")
    ap.add_argument("--work", required=True, help="a scratch directory for the cases")
    ap.add_argument("--only", default="", help="comma-separated name prefixes to run")
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
    # dgemm('N', 'N'): odd and even column counts, row blocks beyond GEMM_P (512).
    for m, n, k in [
        (7, 5, 9),
        (12, 9, 30),
        (5, 1, 20),
        (17, 13, 300),
        (600, 3, 10),
        (4, 7, 8),
        (9, 25, 16),
        (1100, 5, 12),
    ]:
        name = f"gemm_m{m}_n{n}_k{k}"
        (work / f"{name}.case").write_bytes(
            struct.pack("<IIII", 2, m, n, k)
            + np.asarray(rng.normal(0, 1, (m, k)), "<f8").T.copy().tobytes()
            + np.asarray(rng.normal(0, 1, (k, n)), "<f8").T.copy().tobytes()
        )
        names.append(name)
    names += sym_cases(work, np.random.default_rng(4343))
    names += blas_cases(work, np.random.default_rng(4444))
    if args.only:
        prefixes = tuple(args.only.split(","))
        names = [n for n in names if n.startswith(prefixes)]
        for c in work.glob("*.case"):
            if not c.stem.startswith(prefixes):
                c.unlink()
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
