# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx-nc-core's splinterpolator, interp_partial and warpfns with FSL 6.0.7.23's own
libraries.

Generates the cases (seeded), runs ``warpfns_probe`` (built by ``build.sh``) on each (the
resampling cases at one and at three threads), runs the replica's side (the ignored test
``warpfns_probe`` of ``crates-nc/larmorx-nc-core``) at one and at three threads, and prints, per
case, how many lines differ by quantity. See README.md. Run inside a memory box
(``scripts/orchestration/boxed.sh``).
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
ET = {"zeros": 0, "constant": 1, "mirror": 2, "periodic": 3, "softzeros": 4, "correctconstant": 5}


def write_params(d: Path, items: dict) -> None:
    d.mkdir(parents=True, exist_ok=True)
    lines = []
    for k, v in items.items():
        vals = v if isinstance(v, (list, tuple)) else [v]
        lines.append(" ".join([k] + [repr(x) if isinstance(x, float) else str(x) for x in vals]))
    (d / "params.txt").write_text("\n".join(lines) + "\n")


def mat_bytes(m) -> bytes:
    return np.asarray(m, dtype="<f8").reshape(4, 4).tobytes()


def rigid(rx: float, ry: float, rz: float, t) -> np.ndarray:
    cx, sx, cy, sy, cz, sz = np.cos(rx), np.sin(rx), np.cos(ry), np.sin(ry), np.cos(rz), np.sin(rz)
    r = (
        np.array([[1, 0, 0], [0, cx, sx], [0, -sx, cx]])
        @ np.array([[cy, 0, -sy], [0, 1, 0], [sy, 0, cy]])
        @ np.array([[cz, sz, 0], [-sz, cz, 0], [0, 0, 1]])
    )
    m = np.eye(4)
    m[:3, :3] = r
    m[:3, 3] = t
    return m


# --------------------------------------------------------------------------------------------
# spl: the Splinterpolator itself. name: (dims, order, extrapolations, NaN points)
SPL = {
    "spl-o3-mirror": ((7, 6, 5), 3, ["mirror"] * 3, False),
    "spl-o3-periodic": ((8, 7, 6), 3, ["periodic"] * 3, False),
    "spl-o3-mixed": ((7, 6, 5), 3, ["zeros", "constant", "periodic"], False),
    "spl-o3-soft": ((6, 7, 5), 3, ["softzeros", "correctconstant", "mirror"], False),
    "spl-o0": ((6, 5, 4), 0, ["mirror", "zeros", "periodic"], False),
    "spl-o1": ((6, 5, 4), 1, ["constant", "periodic", "zeros"], False),
    "spl-o2": ((7, 6, 5), 2, ["periodic", "mirror", "constant"], False),
    "spl-o4": ((9, 8, 7), 4, ["mirror", "periodic", "zeros"], False),
    "spl-o5": ((9, 8, 7), 5, ["periodic", "constant", "softzeros"], False),
    "spl-o6": ((10, 9, 8), 6, ["mirror", "correctconstant", "periodic"], False),
    "spl-o7": ((10, 9, 8), 7, ["periodic", "mirror", "zeros"], False),
    "spl-1d": ((12,), 3, ["periodic"], False),
    "spl-2d": ((9, 8), 2, ["constant", "zeros"], False),
    "spl-4d": ((5, 4, 4, 3), 3, ["mirror", "periodic", "constant", "zeros"], False),
    "spl-5d": ((4, 3, 3, 3, 2), 2, ["mirror"] * 5, False),
    "spl-thin": ((7, 1, 5), 3, ["periodic", "mirror", "constant"], False),
    "spl-nan": ((6, 5, 4), 3, ["constant", "zeros", "constant"], True),
}


def make_spl(d: Path, dims, order, ets, nan: bool, rng) -> None:
    write_params(
        d,
        {
            "mode": "spl",
            "ndim": len(dims),
            "dims": list(dims),
            "order": order,
            "et": [ET[e] for e in ets],
        },
    )
    n = int(np.prod(dims))
    data = rng.normal(50, 20, n).astype("<f4")
    data[rng.random(n) < 0.05] = 0.0
    (d / "data.bin").write_bytes(data.tobytes())
    nd = len(dims)
    pts = np.zeros((40, 5))
    for k in range(nd):
        pts[:30, k] = rng.uniform(-2.5, dims[k] + 1.5, 30)
        pts[30:, k] = rng.integers(-1, dims[k] + 1, 10)  # grid points and just outside
    pts[30, :nd] = 0.0
    pts[31, :nd] = [x - 1 for x in dims]
    pts[32, :nd] = [x - 0.5 for x in dims]
    if nan:
        pts[33, 0] = np.nan
        pts[34, nd - 1] = np.inf
    # the probe reads the coordinates as float for the vector forms: keep them floats
    pts = pts.astype("<f4").astype("<f8")
    (d / "points.bin").write_bytes(pts.tobytes())
    grid = np.zeros((10, 5), dtype="<u4")
    for k in range(nd):
        grid[:, k] = rng.integers(0, dims[k], 10)
        grid[0, k] = 0
        grid[1, k] = dims[k] - 1
    (d / "grid.bin").write_bytes(grid.tobytes())


# --------------------------------------------------------------------------------------------
# vol: newimage's interpolation. name: (interp, order, extrap, pad, validity, ep2, NaN points;
# not with spline interpolation and periodic or mirror extrapolation, where FSL's index loops take
# about 2^31 / size steps per coefficient)
VOL = {
    "vol-spline-periodic": (
        "spline",
        3,
        "periodic",
        0.0,
        (0, 1, 0),
        ("extraslice", (1, 0, 0)),
        False,
    ),
    "vol-spline-mirror": ("spline", 3, "mirror", 0.0, (0, 0, 0), ("periodic", (0, 1, 0)), False),
    "vol-spline-zeropad": ("spline", 3, "zeropad", 0.0, (0, 0, 0), ("constpad", (0, 0, 1)), True),
    "vol-spline-constpad": ("spline", 3, "constpad", 3.5, (1, 0, 0), ("zeropad", (0, 0, 0)), True),
    "vol-spline-extraslice": (
        "spline",
        3,
        "extraslice",
        -2.0,
        (0, 0, 0),
        ("mirror", (0, 0, 0)),
        False,
    ),
    "vol-spline-o2": ("spline", 2, "periodic", 0.0, (1, 0, 0), ("extraslice", (0, 1, 0)), False),
    "vol-spline-o5": ("spline", 5, "mirror", 0.0, (0, 1, 0), ("zeropad", (0, 0, 0)), False),
    "vol-tri-periodic": ("trilinear", 3, "periodic", 0.0, (0, 1, 0), ("constpad", (1, 0, 0)), True),
    "vol-tri-zeropad": ("trilinear", 3, "zeropad", 0.0, (0, 0, 0), ("periodic", (0, 1, 0)), True),
    "vol-tri-constpad": (
        "trilinear",
        3,
        "constpad",
        4.25,
        (0, 0, 0),
        ("extraslice", (0, 0, 0)),
        True,
    ),
    "vol-tri-extraslice": (
        "trilinear",
        3,
        "extraslice",
        1.5,
        (0, 0, 1),
        ("mirror", (0, 0, 0)),
        True,
    ),
    "vol-tri-mirror": ("trilinear", 3, "mirror", 0.0, (0, 0, 0), ("zeropad", (1, 1, 0)), True),
    "vol-nearest": (
        "nearestneighbour",
        3,
        "constpad",
        7.0,
        (0, 0, 0),
        ("periodic", (0, 0, 0)),
        False,
    ),
}


def make_vol(d: Path, interp, order, extrap, pad, valid, ep2, nan: bool, rng) -> None:
    nx, ny, nz = 9, 8, 7
    write_params(
        d,
        {
            "mode": "vol",
            "fsize": [nx, ny, nz],
            "fvxs": [2.0, 2.5, F33],
            "finterp": interp,
            "forder": order,
            "fextrap": extrap,
            "fpad": pad,
            "fvalid": list(valid),
            "ep2extrap": ep2[0],
            "ep2valid": list(ep2[1]),
        },
    )
    f = rng.normal(100, 30, (nz, ny, nx)).astype("<f4")
    f[0, 0, :3] = 0.0
    (d / "f.bin").write_bytes(f.tobytes())
    pts = np.column_stack([rng.uniform(-3, n + 2, 60) for n in (nx, ny, nz)])
    ints = np.column_stack([rng.integers(-1, n + 1, 15) for n in (nx, ny, nz)]).astype(float)
    edges = np.array(
        [
            [0, 0, 0],
            [nx - 1, ny - 1, nz - 1],
            [nx - 1.5, 0.5, nz - 0.25],
            [-0.5, ny - 0.5, 3.0],
            [4.0, 3.0, -1.0],
            [nx - 0.75, ny + 0.25, 2.0],
        ]
    )
    pts = np.vstack([pts, ints, edges])
    if nan:
        pts = np.vstack([pts, [[np.nan, 2.0, 3.0], [1.0, np.inf, 2.0]]])
    (d / "points.bin").write_bytes(pts.astype("<f4").tobytes())


# --------------------------------------------------------------------------------------------
# warp: fsl-warpfns' resampling. name: settings
WARP = {
    # topup's call: the field along y, a small rigid movement, periodic splines valid along y
    "warp-topup-like": dict(
        fsize=(16, 16, 10),
        fvxs=(3.0, 3.0, F33),
        osize=(16, 16, 10),
        ovxs=(3.0, 3.0, F33),
        dsize=(16, 16, 10),
        dvxs=(3.0, 3.0, F33),
        finterp="spline",
        forder=3,
        fextrap="periodic",
        fvalid=(0, 1, 0),
        pe=1,
        dir1=1,
        A=rigid(0.01, -0.02, 0.015, (0.3, -0.5, 0.2)),
        TT=np.eye(4),
        M=np.eye(4),
        fsform=0,
        dsform=0,
    ),
    # x phase encoding, trilinear (topup's --interp=linear)
    "warp-topup-x-linear": dict(
        fsize=(14, 12, 8),
        fvxs=(2.5, 2.5, 3.0),
        osize=(14, 12, 8),
        ovxs=(2.5, 2.5, 3.0),
        dsize=(14, 12, 8),
        dvxs=(2.5, 2.5, 3.0),
        finterp="trilinear",
        forder=3,
        fextrap="periodic",
        fvalid=(1, 0, 0),
        pe=0,
        dir1=0,
        A=rigid(-0.02, 0.01, 0.03, (-0.4, 0.6, -0.1)),
        TT=np.eye(4),
        M=np.eye(4),
        fsform=0,
        dsform=0,
    ),
    # the image on another grid than the output (topup's regridding): iM differs
    "warp-regrid": dict(
        fsize=(12, 12, 8),
        fvxs=(2.5, 2.5, 2.75),
        osize=(10, 10, 7),
        ovxs=(3.0, 3.0, F33),
        dsize=(10, 10, 7),
        dvxs=(3.0, 3.0, F33),
        finterp="spline",
        forder=3,
        fextrap="periodic",
        fvalid=(0, 1, 0),
        pe=1,
        dir1=2,
        A=rigid(0.0, 0.0, 0.0, (0.0, 0.0, 0.0)),
        TT=np.eye(4),
        M=np.eye(4),
        fsform=1,
        dsform=0,
    ),
    # the fields on their own grid (iT), apply_warp with TT and M, sforms from the fields
    "warp-iT": dict(
        fsize=(12, 11, 9),
        fvxs=(2.0, 2.0, 2.0),
        osize=(10, 9, 8),
        ovxs=(2.5, 2.5, 2.5),
        dsize=(9, 8, 7),
        dvxs=(3.0, 3.0, 3.0),
        finterp="trilinear",
        forder=3,
        fextrap="extraslice",
        fpad=-1.0,
        fvalid=(0, 0, 0),
        pe=-1,
        dir1=1,
        A=rigid(0.05, 0.02, -0.03, (1.0, -0.5, 0.7)),
        TT=rigid(0.0, 0.0, 0.02, (0.5, 0.0, -0.3)),
        M=rigid(0.01, 0.0, 0.0, (0.0, 0.4, 0.0)),
        fsform=2,
        dsform=4,
    ),
    # a 12-dof affine, quadratic splines, zero padding, the input's sform combined
    "warp-affine-zeropad": dict(
        fsize=(13, 12, 10),
        fvxs=(2.0, 2.2, 2.4),
        osize=(11, 10, 9),
        ovxs=(2.5, 2.5, 2.5),
        dsize=(11, 10, 9),
        dvxs=(2.5, 2.5, 2.5),
        finterp="spline",
        forder=2,
        fextrap="zeropad",
        fvalid=(0, 0, 0),
        pe=-1,
        dir1=0,
        A=np.array(
            [
                [1.05, 0.02, -0.01, 1.2],
                [0.03, 0.97, 0.04, -0.8],
                [-0.02, 0.01, 1.02, 0.5],
                [0, 0, 0, 1],
            ]
        ),
        TT=np.eye(4),
        M=rigid(0.0, 0.01, 0.0, (0.2, 0.0, 0.0)),
        fsform=1,
        dsform=0,
    ),
    "warp-mirror-constpad": dict(
        fsize=(10, 10, 6),
        fvxs=(3.0, 3.0, 3.0),
        osize=(10, 10, 6),
        ovxs=(3.0, 3.0, 3.0),
        dsize=(10, 10, 6),
        dvxs=(3.0, 3.0, 3.0),
        finterp="spline",
        forder=3,
        fextrap="mirror",
        fvalid=(0, 0, 0),
        pe=-1,
        dir1=2,
        A=rigid(0.1, 0.0, 0.0, (2.0, 1.0, 0.0)),
        TT=np.eye(4),
        M=np.eye(4),
        fsform=0,
        dsform=1,
    ),
    "warp-nearest": dict(
        fsize=(9, 9, 6),
        fvxs=(2.0, 2.0, 2.0),
        osize=(9, 9, 6),
        ovxs=(2.0, 2.0, 2.0),
        dsize=(9, 9, 6),
        dvxs=(2.0, 2.0, 2.0),
        finterp="nearestneighbour",
        forder=3,
        fextrap="constpad",
        fpad=7.0,
        fvalid=(0, 0, 0),
        pe=-1,
        dir1=1,
        A=rigid(0.0, 0.0, 0.05, (0.5, 0.5, 0.0)),
        TT=np.eye(4),
        M=np.eye(4),
        fsform=0,
        dsform=0,
    ),
}


def make_warp(d: Path, c: dict, rng) -> None:
    items = {"mode": "warp"}
    for k in ("fsize", "fvxs", "osize", "ovxs", "dsize", "dvxs", "fvalid"):
        items[k] = list(c[k])
    for k in ("finterp", "forder", "fextrap", "dir1", "fsform", "dsform"):
        items[k] = c[k]
    if "fpad" in c:
        items["fpad"] = c["fpad"]
    write_params(d, items)
    fx, fy, fz = c["fsize"]
    f = rng.normal(100, 30, (fz, fy, fx)).astype("<f4")
    f[:, : fy // 3, : fx // 3] = 0.0
    (d / "f.bin").write_bytes(f.tobytes())
    dx, dy, dz = c["dsize"]
    disp = np.zeros((3, dz, dy, dx), dtype="<f4")
    if c["pe"] >= 0:  # topup: only the phase-encoding direction (a smooth field, in mm)
        zz, yy, xx = np.meshgrid(np.arange(dz), np.arange(dy), np.arange(dx), indexing="ij")
        disp[c["pe"]] = (
            1.5 * np.sin(xx / dx * 3.0) * np.cos(yy / dy * 2.0) + 0.3 * zz / dz
        ).astype("<f4")
    else:
        disp[:] = rng.normal(0, 1.2, disp.shape)
    (d / "d.bin").write_bytes(disp.tobytes())
    (d / "A.bin").write_bytes(mat_bytes(c["A"]))
    (d / "TT.bin").write_bytes(mat_bytes(c["TT"]))
    (d / "M.bin").write_bytes(mat_bytes(c["M"]))
    s = np.diag([-2.0, 2.0, 2.0, 1.0])
    s[:3, 3] = (40.0, -50.0, -30.0)
    (d / "fsform.bin").write_bytes(mat_bytes(s))
    s2 = s.copy()
    s2[0, 3] = 37.5
    (d / "dsform.bin").write_bytes(mat_bytes(s2))


# --------------------------------------------------------------------------------------------
# jac: Jacobians. name: (field size, voxel sizes, knot spacing, order, jac size, jac extrapolation)
JAC = {
    "jac-topup-like": ((16, 16, 10), (3.0, 3.0, F33), (4, 4, 4), 3, (16, 16, 10), None),
    "jac-quad": ((12, 10, 8), (2.0, 2.5, 3.0), (2, 3, 2), 2, (12, 10, 8), None),
    "jac-smaller-periodic": ((12, 10, 8), (2.0, 2.0, 2.0), (3, 3, 3), 3, (10, 8, 6), "periodic"),
    "jac-smaller-mirror": ((12, 10, 8), (2.0, 2.0, 2.0), (3, 3, 3), 3, (11, 9, 7), "mirror"),
    "jac-smaller-extraslice": (
        (12, 10, 8),
        (2.0, 2.0, 2.0),
        (3, 3, 3),
        3,
        (11, 9, 6),
        "extraslice",
    ),
    "jac-larger": ((10, 9, 7), (2.0, 2.0, 2.0), (3, 3, 3), 3, (13, 11, 9), None),
}


def make_jac(d: Path, fsize, vxs, ksp, order, jsize, jext, rng) -> None:
    items = {
        "mode": "jac",
        "fsize": list(fsize),
        "fvxs": list(vxs),
        "ksp": list(ksp),
        "order": order,
        "jsize": list(jsize),
        "scale": [0.15, -0.165],
    }
    if jext:
        items["jextrap"] = jext
    write_params(d, items)
    (d / "coef.bin").write_bytes(rng.normal(0, 4, 3 * 20000).astype("<f8").tobytes())
    (d / "aff.bin").write_bytes(mat_bytes(rigid(0.02, -0.01, 0.03, (0.5, -0.2, 0.1))))


def compare(a: Path, b: Path) -> tuple[int, collections.Counter, int]:
    la = a.read_text().splitlines()
    lb = b.read_text().splitlines()
    bad = collections.Counter()
    for x, y in zip(la, lb, strict=False):
        if x != y:
            bad[x.split(" ", 1)[0]] += 1
    if len(la) != len(lb):
        bad["<length>"] += abs(len(la) - len(lb))
    return len(la), bad, sum(1 for x in la if " error " in x)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--probe", required=True, help="the warpfns_probe binary")
    ap.add_argument("--work", required=True, help="a scratch directory for the cases")
    args = ap.parse_args()
    work = Path(args.work)
    rng = np.random.default_rng(20261103)
    for name, (dims, order, ets, nan) in SPL.items():
        make_spl(work / name, dims, order, ets, nan, rng)
    for name, v in VOL.items():
        make_vol(work / name, *v, rng)
    for name, c in WARP.items():
        make_warp(work / name, c, rng)
    for name, v in JAC.items():
        make_jac(work / name, *v, rng)
    names = list(SPL) + list(VOL) + list(WARP) + list(JAC)
    env = dict(os.environ, OPENBLAS_NUM_THREADS="1", OPENBLAS_CORETYPE="Haswell")
    for name in names:
        for nthr in (1, 3) if name in WARP else (1,):
            with open(work / name / f"fsl{nthr}.txt", "w") as out:
                subprocess.run(
                    [args.probe, str(work / name), str(nthr)],
                    stdout=out,
                    env=env,
                    stderr=subprocess.DEVNULL,
                    check=False,
                )
    for nthr in (1, 3):
        subprocess.run(
            [
                "cargo",
                "test",
                "--release",
                "-q",
                "-p",
                "larmorx-nc-core",
                "--test",
                "warpfns_probe",
                "--",
                "--ignored",
            ],
            cwd=REPO / "crates-nc",
            env=dict(env, WF_PROBE_ROOT=str(work), WF_PROBE_NTHR=str(nthr)),
            check=True,
            stdout=subprocess.DEVNULL,
        )
    total_bad = total_lines = 0
    for name in names:
        n, bad, errs = compare(work / name / "fsl1.txt", work / name / "replica1.txt")
        total_lines += n
        total_bad += sum(bad.values())
        extra = ""
        if name in WARP:
            _, b3, _ = compare(work / name / "fsl1.txt", work / name / "fsl3.txt")
            _, r3, _ = compare(work / name / "fsl1.txt", work / name / "replica3.txt")
            extra = f"  fsl3: {sum(b3.values())}  replica3: {sum(r3.values())}"
            total_bad += sum(r3.values())
        print(
            f"{name:26s} {n:8d} lines ({errs} errors)  differ: {sum(bad.values())}"
            f" {dict(bad) if bad else ''}{extra}"
        )
    print(f"total {total_lines} lines, {total_bad} differ")
    return 0 if total_bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
