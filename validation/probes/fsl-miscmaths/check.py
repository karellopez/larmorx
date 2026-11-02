# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx-nc-core's sparse matrices, solvers and optimisers with FSL 6.0.7.23's own.

Generates the cases (seeded), runs ``miscmaths_probe`` (built by ``build.sh``) on each, runs the
replica's side (the ignored test ``miscmaths_probe`` of ``crates-nc/larmorx-nc-core``) once over
all cases, and prints, per case, how many lines differ by quantity. See README.md. Run inside a
memory box (``scripts/orchestration/boxed.sh``).
"""

from __future__ import annotations

import argparse
import collections
import os
import struct
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]

# name: (n, band, far pairs, D rows, LM iterations, SCG iterations)
CASES = {
    "n6": (6, 2, 1, 2, 30, 40),
    "n20": (20, 3, 4, 3, 30, 60),
    "n33": (33, 2, 6, 3, 30, 80),
    "n150": (150, 4, 30, 4, 40, 200),
}


def csc_bytes(mat: np.ndarray, keep: np.ndarray) -> bytes:
    """Compressed columns of ``mat`` at the positions ``keep`` (explicit zeros kept)."""
    m, n = mat.shape
    irp, jcp, val = [], [0], []
    for c in range(n):
        for r in range(m):
            if keep[r, c]:
                irp.append(r)
                val.append(mat[r, c])
        jcp.append(len(irp))
    out = struct.pack("<III", m, n, len(irp))
    out += np.asarray(irp, "<u4").tobytes() + np.asarray(jcp, "<u4").tobytes()
    out += np.asarray(val, "<f8").tobytes()
    return out


def make_case(d: Path, n: int, band: int, far: int, drows: int, lm: int, scg: int, seed: int):
    rng = np.random.default_rng(seed)
    d.mkdir(parents=True, exist_ok=True)
    # A: symmetric, diagonally dominant (positive definite), a band and a few far pairs, one
    # explicit zero kept.
    keep = np.zeros((n, n), bool)
    for k in range(-band, band + 1):
        keep |= np.eye(n, k=k, dtype=bool)
    for _ in range(far):
        i, j = rng.integers(0, n, 2)
        keep[i, j] = keep[j, i] = True
    a = rng.normal(0, 1, (n, n)) * keep
    a = (a + a.T) / 3.0
    a[np.diag_indices(n)] = np.abs(a).sum(1) + rng.uniform(0.5, 2.0, n)
    if n > 3:
        a[0, 2] = a[2, 0] = 0.0
        keep[0, 2] = keep[2, 0] = True
    (d / "A.csc").write_bytes(csc_bytes(a, keep))
    # B: another sparsity (not symmetric), every third column empty.
    keepb = rng.random((n, n)) < min(0.9, 4.0 / n)
    keepb[np.diag_indices(n)] |= rng.random(n) < 0.5
    keepb[:, ::3] = False
    b = rng.normal(0, 2, (n, n)) * keepb
    (d / "B.csc").write_bytes(csc_bytes(b, keepb))
    dm = rng.normal(0, 1, (drows, n))
    dm[rng.random((drows, n)) < 0.3] = 0.0
    dm[0, 1] = -0.0
    (d / "D.bin").write_bytes(
        struct.pack("<II", drows, n) + np.asarray(dm, "<f8").T.copy().tobytes()
    )
    (d / "x.bin").write_bytes(rng.normal(0, 1, n).astype("<f8").tobytes())
    (d / "b.bin").write_bytes(rng.normal(0, 3, n).astype("<f8").tobytes())
    c = rng.normal(0.5, 1.0, n)
    dd = rng.uniform(0.0, 0.3, n)
    q = 0.75
    (d / "cf.bin").write_bytes(np.concatenate([c, dd, [q]]).astype("<f8").tobytes())
    (d / "params.txt").write_text(f"{lm} {scg}\n")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe", required=True, help="the miscmaths_probe binary")
    ap.add_argument("--work", required=True, help="a scratch directory for the cases")
    args = ap.parse_args()
    work = Path(args.work)
    for i, (name, spec) in enumerate(CASES.items()):
        make_case(work / name, *spec, seed=3000 + i)
    env = dict(os.environ, OPENBLAS_NUM_THREADS="1", OPENBLAS_CORETYPE="Haswell")
    env.pop("FSL_NUM_THREADS", None)
    env.pop("FSL_SKIP_GLOBAL", None)
    for name in CASES:
        with open(work / name / "fsl.txt", "w") as out:
            subprocess.run([args.probe, str(work / name)], stdout=out, env=env, check=False)
    subprocess.run(
        [
            "cargo",
            "test",
            "--release",
            "-q",
            "-p",
            "larmorx-nc-core",
            "--test",
            "miscmaths_probe",
            "--",
            "--ignored",
        ],
        cwd=REPO / "crates-nc",
        env=dict(env, MM_PROBE_ROOT=str(work)),
        check=True,
        stdout=subprocess.DEVNULL,
    )
    total_bad = 0
    for name in CASES:
        fsl = (work / name / "fsl.txt").read_text().splitlines()
        rep = (work / name / "replica.txt").read_text().splitlines()
        bad = collections.Counter()
        first = {}
        for k, (a, b) in enumerate(zip(fsl, rep, strict=False)):
            if a != b:
                t = a.split(" ", 1)[0]
                bad[t] += 1
                first.setdefault(t, k + 1)
        if len(fsl) != len(rep):
            bad["<length>"] += abs(len(fsl) - len(rep))
        total_bad += sum(bad.values())
        print(
            f"{name}: {len(fsl)} lines, replica differs {sum(bad.values())} "
            f"{dict(bad) if bad else ''} {('first at ' + str(first)) if first else ''}"
        )
        if fsl and fsl[-1].startswith("exception"):
            print(f"  FSL: {fsl[-1]}")
    print(f"total differing lines: {total_bad}")
    return 1 if total_bad else 0


if __name__ == "__main__":
    sys.exit(main())
