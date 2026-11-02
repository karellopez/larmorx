# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx-nc-core's translation of fsl-basisfield with FSL 6.0.7.23's own library.

Generates the cases (seeded), runs ``basisfield_probe`` (built by ``build.sh``) on each at one
and at four threads, runs the replica's side (the ignored test ``basisfield_probe`` of
``crates-nc/larmorx-nc-core``) once over all cases, and prints, per case, how many lines differ
by quantity. See README.md. Run inside a memory box (``scripts/orchestration/boxed.sh``).
"""

from __future__ import annotations

import argparse
import collections
import os
import subprocess
import sys
from pathlib import Path

import numpy as np

F33 = float(np.float32(3.3))
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]

# name: (size, voxel size, knot spacing, order)
CASES = {
    "cubic-k1": ((9, 8, 7), (2.0, 2.0, 2.0), (1, 1, 1), 3),
    "cubic-k232": ((14, 12, 10), (2.5, 2.0, 3.3), (2, 3, 2), 3),
    # 3.3 mm is not a float: AsVolume refuses the field (FSL compares the double voxel size
    # with the volume's float one); the voxel sizes of the other cases are floats.
    "cubic-k324": ((13, 11, 12), (3.0, 3.0, F33), (3, 2, 4), 3),
    "cubic-k5-thin": ((16, 15, 3), (2.0, 2.0, 4.0), (5, 5, 4), 3),
    "quad-k1": ((8, 7, 6), (2.0, 2.0, 2.0), (1, 1, 1), 2),
    "quad-k234": ((14, 13, 12), (2.0, 2.5, 3.0), (2, 3, 4), 2),
    "quad-k5": ((17, 12, 9), (1.5, 3.0, 2.0), (5, 3, 3), 2),
    "cubic-topup-like": ((24, 24, 16), (3.0, 3.0, F33), (5, 5, 4), 3),
}
# Energies only, on more than 10000 coefficients: OpenBLAS's threaded ddot.
BIG = {"cubic-k1-big": ((24, 22, 20), (2.0, 2.0, F33), (1, 1, 1), 3)}


def make_case(d: Path, size, vxs, ksp, order, seed: int) -> None:
    rng = np.random.default_rng(seed)
    nx, ny, nz = size
    d.mkdir(parents=True, exist_ok=True)
    (d / "params.txt").write_text(
        f"{nx} {ny} {nz} {vxs[0]} {vxs[1]} {vxs[2]} {ksp[0]} {ksp[1]} {ksp[2]} {order}\n"
    )
    # More coefficients than any case needs: both sides read the first CoefSz.
    (d / "coef.bin").write_bytes(rng.normal(0, 1, 200000).astype("<f8").tobytes())
    shape = (nz, ny, nx)
    ima1 = rng.normal(100, 30, shape).astype("<f4")
    ima2 = rng.normal(0, 5, shape).astype("<f4")
    # A block of zeros wider than a spline's support (the zero-spline maps), a negative zero and
    # a few exact zeros scattered.
    ima1[:, : ny // 2, : nx // 2] = 0.0
    ima1[0, -1, -1] = -0.0
    ima2[rng.random(shape) < 0.05] = 0.0
    mask = (rng.random(shape) < 0.85).astype("i1")
    mask[nz // 2 :, ny // 2 :, :] = 0
    (d / "ima1.bin").write_bytes(ima1.tobytes())
    (d / "ima2.bin").write_bytes(ima2.tobytes())
    (d / "mask.bin").write_bytes(mask.tobytes())
    pts = rng.uniform([-1.5, -1.5, -1.5], [nx + 0.5, ny + 0.5, nz + 0.5], (12, 3))
    pts = np.vstack([pts, [[0.0, 0.0, 0.0], [nx - 1, ny - 1, nz - 1], [2.5, 1.0, 0.5]]])
    (d / "points.txt").write_text(
        "".join(f"{float(x)!r} {float(y)!r} {float(z)!r}\n" for x, y, z in pts)
    )
    wide = [(-1, 0, 0), (nx, ny // 2, 0), (-3, -2, nz + 1), (1, 1, 1), (nx + 2, ny + 3, -2)]
    (d / "wide.txt").write_text("".join(f"{i} {j} {k}\n" for i, j, k in wide))


def tags(lines: list[str]) -> list[str]:
    return [ln.split(" ", 1)[0] for ln in lines]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe", required=True, help="the basisfield_probe binary")
    ap.add_argument("--work", required=True, help="a scratch directory for the cases")
    args = ap.parse_args()
    work = Path(args.work)
    for i, (name, (size, vxs, ksp, order)) in enumerate(CASES.items()):
        make_case(work / name, size, vxs, ksp, order, seed=1000 + i)
    env = dict(os.environ, OPENBLAS_NUM_THREADS="1", OPENBLAS_CORETYPE="Haswell")
    for name in CASES:
        for nthr in (1, 4):
            with open(work / name / f"fsl{nthr}.txt", "w") as out:
                subprocess.run(
                    [args.probe, str(work / name), str(nthr)], stdout=out, env=env, check=False
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
            "basisfield_probe",
            "--",
            "--ignored",
        ],
        cwd=REPO / "crates-nc",
        env=dict(env, BF_PROBE_ROOT=str(work)),
        check=True,
        stdout=subprocess.DEVNULL,
    )
    total_bad = 0
    for name in CASES:
        fsl1 = (work / name / "fsl1.txt").read_text().splitlines()
        fsl4 = (work / name / "fsl4.txt").read_text().splitlines()
        rep = (work / name / "replica.txt").read_text().splitlines()
        thr = sum(a != b for a, b in zip(fsl1, fsl4, strict=False)) + abs(len(fsl1) - len(fsl4))
        bad = collections.Counter()
        for a, b in zip(fsl1, rep, strict=False):
            if a != b:
                bad[a.split(" ", 1)[0]] += 1
        if len(fsl1) != len(rep):
            bad["<length>"] += abs(len(fsl1) - len(rep))
        total_bad += sum(bad.values())
        print(
            f"{name}: {len(fsl1)} lines, FSL 1 vs 4 threads differ {thr}, replica differs "
            f"{sum(bad.values())} {dict(bad) if bad else ''}"
        )
        if fsl1 and fsl1[-1].startswith("exception"):
            print(f"  FSL: {fsl1[-1]}")
    # Energies on > 10000 coefficients, OpenBLAS at 1 and 4 threads, the replica alike. FSL's
    # libfsl-utils sets OpenBLAS to one thread when it loads unless FSL_SKIP_GLOBAL is set.
    for i, (name, (size, vxs, ksp, order)) in enumerate(BIG.items()):
        make_case(work / name, size, vxs, ksp, order, seed=2000 + i)
        for bt in (1, 4):
            with open(work / name / f"fsl_blas{bt}.txt", "w") as out:
                subprocess.run(
                    [args.probe, str(work / name), "1", "energy"],
                    stdout=out,
                    env=dict(env, OPENBLAS_NUM_THREADS=str(bt), FSL_SKIP_GLOBAL=str(int(bt > 1))),
                    check=False,
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
                    "basisfield_probe",
                    "--",
                    "--ignored",
                ],
                cwd=REPO / "crates-nc",
                env=dict(
                    env,
                    BF_PROBE_ROOT=str(work),
                    BF_PROBE_ONLY=name,
                    BF_PROBE_ENERGY="1",
                    BF_PROBE_BLAS_THREADS=str(bt),
                    BF_PROBE_OUTNAME=f"replica_blas{bt}.txt",
                ),
                check=True,
                stdout=subprocess.DEVNULL,
            )
            a = (work / name / f"fsl_blas{bt}.txt").read_text().splitlines()
            b = (work / name / f"replica_blas{bt}.txt").read_text().splitlines()
            nd = sum(x != y for x, y in zip(a, b, strict=False)) + abs(len(a) - len(b))
            total_bad += nd
            print(f"{name} (energies, OpenBLAS {bt} threads): {len(a)} lines, replica differs {nd}")
        e1 = (work / name / "fsl_blas1.txt").read_text().splitlines()[:3]
        e4 = (work / name / "fsl_blas4.txt").read_text().splitlines()[:3]
        print(f"  FSL's energies at 1 and 4 OpenBLAS threads: {'same' if e1 == e4 else 'differ'}")
    print(f"total differing lines: {total_bad}")
    return 1 if total_bad else 0


if __name__ == "__main__":
    sys.exit(main())
