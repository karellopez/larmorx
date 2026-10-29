# SPDX-License-Identifier: Apache-2.0
"""Black-box experiments on AFNI 25.2.09's NIfTI geometry, on headers built here: the quaternion
AFNI writes (read in full double precision from a NIfTI-2 output, `AFNI_NIFTI_WRITE_TYPE=2`), the
orientation letters 3dAutomask prints for each axis, and the obliquity angle. Each experiment
compares AFNI's answer with candidate computations written in this file (from the spec and the
public-domain NIfTI reference library's method), and prints how many inputs each one matches.
Nothing here reads AFNI's source.

    python -P geometry_probe.py quat <work> [N]      # quaternion: AFNI vs the candidates
    python -P geometry_probe.py orient <work> [N]    # orientation letters: AFNI vs candidates
    python -P geometry_probe.py angle <work>         # the obliquity angle: which matrix, precision
    python -P geometry_probe.py qform-units <work> [N]  # AFNI-07's default on qform-only metre headers

AFNI in $AFNI_BIN (default <workspace>/oracles/afni-25.2.09/bin); <workspace> is
$LARMORX_WORKSPACE.
"""

from __future__ import annotations

import os
import re
import shutil
import struct
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
AFNI = Path(os.environ.get("AFNI_BIN", WORKSPACE / "oracles/afni-25.2.09/bin"))
LD = np.longdouble  # x87 extended precision on x86-64 Linux
assert np.finfo(LD).nmant == 63, "needs x86-64's 80-bit long double"


# ------------------------------------------------------------------------------------------------
# Inputs


def write_input(path: Path, srow: np.ndarray | None, quat=None, qoffset=(0, 0, 0), pixdim=(1, 1, 1),
                qfac=1.0, shape=(2, 2, 2), units=2) -> None:  # fmt: skip
    """A float32 NIfTI-1 file of ones with the given sform rows (float32) and/or qform."""
    import nibabel as nib

    h = nib.Nifti1Header()
    h.set_data_shape(shape)
    h.set_data_dtype(np.float32)
    h["vox_offset"] = 352
    h.set_xyzt_units(units, 8)
    h["pixdim"][1:4] = pixdim
    h["pixdim"][0] = qfac
    if quat is not None:
        h["qform_code"] = 1
        h["quatern_b"], h["quatern_c"], h["quatern_d"] = quat
        h["qoffset_x"], h["qoffset_y"], h["qoffset_z"] = qoffset
    else:
        h["qform_code"] = 0
    if srow is not None:
        h["sform_code"] = 1
        h["srow_x"], h["srow_y"], h["srow_z"] = srow.astype(np.float32)
    else:
        h["sform_code"] = 0
    # nibabel would rewrite the qform from the affine; write the header bytes as set.
    raw = h.binaryblock + b"\x00" * 4 + np.ones(shape, np.float32).tobytes(order="F")
    path.write_bytes(raw)


def afni(work: Path, name: str, inp: Path, extra_env=None, args=()) -> tuple[str, Path]:
    d = work / name
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    env = {"HOME": str(work), "PATH": "/usr/bin:/bin", "AFNI_DONT_LOGFILE": "YES",
           "AFNI_NIFTI_NOEXT": "YES", **(extra_env or {})}  # fmt: skip
    p = subprocess.run([str(AFNI / "3dAutomask"), *args, "-prefix", "m.nii", str(inp)], cwd=d,
                       env=env, capture_output=True)  # fmt: skip
    return p.stderr.decode("utf-8", "replace"), d / "m.nii"


def nifti2_quatern(path: Path) -> tuple[float, float, float, float]:
    raw = path.read_bytes()
    assert struct.unpack("<i", raw[:4])[0] == 540, "not NIfTI-2"
    # NIfTI-2: pixdim at 104 (8 doubles), quatern_b/c/d at 352, 360, 368
    qfac = struct.unpack("<d", raw[104:112])[0]
    b, c, d = struct.unpack("<3d", raw[352:376])
    return b, c, d, qfac


# ------------------------------------------------------------------------------------------------
# Candidate computations of the quaternion (nifti_clib's method in double; variants)


def _det(r):
    return (r[0][0] * r[1][1] * r[2][2] - r[0][0] * r[2][1] * r[1][2] - r[1][0] * r[0][1] * r[2][2]
            + r[1][0] * r[2][1] * r[0][2] + r[2][0] * r[0][1] * r[1][2]
            - r[2][0] * r[1][1] * r[0][2])  # fmt: skip


def _inverse(r, recip_x87: bool):
    deti = _det(r)
    if deti != 0.0:
        deti = float(LD(1) / LD(deti)) if recip_x87 else 1.0 / deti
    (r11, r12, r13), (r21, r22, r23), (r31, r32, r33) = r
    return [
        [deti * (r22 * r33 - r32 * r23), deti * (-r12 * r33 + r32 * r13), deti * (r12 * r23 - r22 * r13)],
        [deti * (-r21 * r33 + r31 * r23), deti * (r11 * r33 - r31 * r13), deti * (-r11 * r23 + r21 * r13)],
        [deti * (r21 * r32 - r31 * r22), deti * (-r11 * r32 + r31 * r12), deti * (r11 * r22 - r21 * r12)],
    ]  # fmt: skip


def _rownorm(a):
    r = [abs(a[i][0]) + abs(a[i][1]) + abs(a[i][2]) for i in range(3)]
    m = r[0]
    m = r[1] if m < r[1] else m
    return r[2] if m < r[2] else m


def _colnorm(a):
    r = [abs(a[0][j]) + abs(a[1][j]) + abs(a[2][j]) for j in range(3)]
    m = r[0]
    m = r[1] if m < r[1] else m
    return r[2] if m < r[2] else m


def _polar(a, v):
    f32 = (lambda x: float(np.float32(x))) if v.get("float_scal") else (lambda x: x)
    x = [row[:] for row in a]
    dif, k = 1.0, 0
    gam = _det(x)
    while gam == 0.0:
        gam = 0.00001 * (0.001 + _rownorm(x))
        for i in range(3):
            x[i][i] += gam
        gam = _det(x)
    while True:
        y = _inverse(x, v.get("recip_x87", False))
        if dif > 0.3:
            alp = f32(np.sqrt(f32(_rownorm(x)) * f32(_colnorm(x))))
            bet = f32(np.sqrt(f32(_rownorm(y)) * f32(_colnorm(y))))
            gam = f32(np.sqrt(bet / alp))
            gmi = f32(1.0 / gam)
        else:
            gam = gmi = 1.0
        z = [[0.5 * (gam * x[i][j] + gmi * y[j][i]) for j in range(3)] for i in range(3)]
        dif = f32(sum(abs(z[i][j] - x[i][j]) for i in range(3) for j in range(3)))
        k += 1
        if k > 100 or dif < 3.0e-6:
            return z
        x = z


def _x87div(a, b):
    return float(LD(a) / LD(b))


def quaternion(m33: np.ndarray, v: dict) -> tuple[float, float, float, float]:
    r = [[float(m33[i][j]) for j in range(3)] for i in range(3)]
    xd = np.sqrt(r[0][0] ** 2 + r[1][0] ** 2 + r[2][0] ** 2) if not v.get("sq") else None
    cols = []
    for j in range(3):
        s = (r[0][j] * r[0][j] + r[1][j] * r[1][j]) + r[2][j] * r[2][j]
        cols.append(float(np.sqrt(s)))
    del xd
    for j in range(3):
        if cols[j] == 0.0:
            for i in range(3):
                r[i][j] = 1.0 if i == j else 0.0
            cols[j] = 1.0
    if v.get("mul_recip"):
        r = [[r[i][j] * (1.0 / cols[j]) for j in range(3)] for i in range(3)]
    else:
        r = [[r[i][j] / cols[j] for j in range(3)] for i in range(3)]
    if v.get("q_float"):
        r = [[float(np.float32(r[i][j])) for j in range(3)] for i in range(3)]
    p = _polar(r, v)
    (r11, r12, r13), (r21, r22, r23), (r31, r32, r33) = p
    if _det(p) > 0:
        qfac = 1.0
    else:
        qfac = -1.0
        r13, r23, r33 = -r13, -r23, -r33
    a = float(LD(r11 + r22 + r33) + LD(1))
    if a > 0.5:
        a = 0.5 * np.sqrt(a)
        b = _x87div(0.25 * (r32 - r23), a)
        c = _x87div(0.25 * (r13 - r31), a)
        d = _x87div(0.25 * (r21 - r12), a)
    else:
        xd = 1.0 + r11 - (r22 + r33)
        yd = 1.0 + r22 - (r11 + r33)
        zd = 1.0 + r33 - (r11 + r22)
        if xd > 1.0:
            b = 0.5 * np.sqrt(xd)
            c = _x87div(0.25 * (r12 + r21), b)
            d = _x87div(0.25 * (r13 + r31), b)
            a = _x87div(0.25 * (r32 - r23), b)
        elif yd > 1.0:
            c = 0.5 * np.sqrt(yd)
            b = _x87div(0.25 * (r12 + r21), c)
            d = _x87div(0.25 * (r23 + r32), c)
            a = _x87div(0.25 * (r13 - r31), c)
        else:
            d = 0.5 * np.sqrt(zd)
            b = _x87div(0.25 * (r13 + r31), d)
            c = _x87div(0.25 * (r23 + r32), d)
            a = _x87div(0.25 * (r21 - r12), d)
        if a < 0.0:
            b, c, d = -b, -c, -d
    return float(b), float(c), float(d), qfac


VARIANTS = {
    "spec (double)": {},
    "1/det in x87": {"recip_x87": True},
    "scalars float": {"float_scal": True},
    "1/det x87 + scalars float": {"recip_x87": True, "float_scal": True},
    "normalised to float": {"q_float": True},
    "multiply by 1/length": {"mul_recip": True},
}


# ------------------------------------------------------------------------------------------------
# Random matrices of the kinds the random tests draw


def random_cases(n: int, seed: int = 1):
    rng = np.random.default_rng(seed)
    out = []
    for t in range(n):
        kind = t % 4
        if kind in (0, 1):
            # A rational quaternion with a zero component or two (exactly representable rotations
            # with zeros, where the method's rounding shows), and unit or odd spacing.
            q = rng.integers(-30, 31, 4)
            zero = rng.choice(4, size=int(rng.integers(0, 3)), replace=False)
            q[zero] = 0
            if not q.any():
                q[0] = 1
            q = q / np.sqrt((q.astype(float) ** 2).sum())
            a, b, c, d = q
            rot = np.array([
                [a * a + b * b - c * c - d * d, 2 * (b * c - a * d), 2 * (b * d + a * c)],
                [2 * (b * c + a * d), a * a + c * c - b * b - d * d, 2 * (c * d - a * b)],
                [2 * (b * d - a * c), 2 * (c * d + a * b), a * a + d * d - c * c - b * b],
            ])  # fmt: skip
            sp = np.array([1.0, 1.0, 1.0]) if kind == 0 else rng.choice([0.8, 1.5, 2.0, 3.1], 3)
            if rng.random() < 0.3:
                sp[2] = -sp[2]
            m = rot * sp
        elif kind == 2:
            m = rng.normal(size=(3, 3)) * rng.choice([1.0, 2.5], 3)
        else:
            m = np.eye(3)[rng.permutation(3)] * rng.choice([-1, 1], 3) + rng.normal(0, 0.2, (3, 3))
        srow = np.zeros((3, 4))
        srow[:, :3] = m
        srow[:, 3] = rng.integers(-50, 50, 3)
        out.append(srow.astype(np.float32))
    return out


def quat(work: Path, n: int) -> None:
    cases = random_cases(n)

    def one(t):
        srow = cases[t]
        inp = work / f"q{t}.nii"
        write_input(inp, srow)
        _, out = afni(work, f"q{t}", inp, {"AFNI_NIFTI_WRITE_TYPE": "2"}, ["-q"])
        return nifti2_quatern(out)

    with ThreadPoolExecutor(12) as ex:
        got = list(ex.map(one, range(n)))
    score = dict.fromkeys(VARIANTS, 0)
    bad = {k: [] for k in VARIANTS}
    for t, g in enumerate(got):
        m33 = cases[t][:, :3].astype(np.float64)
        for name, v in VARIANTS.items():
            if quaternion(m33, v) == tuple(g):
                score[name] += 1
            else:
                bad[name].append(t)
    for name in VARIANTS:
        print(f"{name:28s} {score[name]:4d} of {n}   first misses {bad[name][:8]}")
    t = bad["spec (double)"][:3]
    for i in t:
        print(i, cases[i].tolist(), got[i], quaternion(cases[i][:, :3].astype(np.float64), {}))


# ------------------------------------------------------------------------------------------------
# Orientation letters


LETTERS = {1: "LR", -1: "RL", 2: "PA", -2: "AP", 3: "IS", -3: "SI"}


def orientation_nifti(m33: np.ndarray, precision=np.float32) -> list[str]:
    """The public-domain NIfTI reference library's method (`nifti_mat44_to_orientation`): unit
    columns, j and k orthogonalised (Gram-Schmidt, when a dot product exceeds 1e-4), then the
    signed permutation whose product with the matrix has the largest trace (loop order i, j, k
    over columns, p, q, r over signs; strict `>`), in `precision`."""
    f = precision
    xi, xj, xk = (f(v) for v in m33[0])
    yi, yj, yk = (f(v) for v in m33[1])
    zi, zj, zk = (f(v) for v in m33[2])

    def nrm(a, b, c):
        return f(np.sqrt(np.float64(a * a + b * b + c * c)))

    val = nrm(xi, yi, zi)
    if val == 0:
        return ["??"] * 3
    xi, yi, zi = xi / val, yi / val, zi / val
    val = nrm(xj, yj, zj)
    if val == 0:
        return ["??"] * 3
    xj, yj, zj = xj / val, yj / val, zj / val
    val = xi * xj + yi * yj + zi * zj
    if abs(val) > 1.0e-4:
        xj, yj, zj = xj - val * xi, yj - val * yi, zj - val * zi
        val = nrm(xj, yj, zj)
        if val == 0:
            return ["??"] * 3
        xj, yj, zj = xj / val, yj / val, zj / val
    val = nrm(xk, yk, zk)
    if val == 0:
        xk, yk, zk = yi * zj - zi * yj, zi * xj - zj * xi, xi * yj - yi * xj
    else:
        xk, yk, zk = xk / val, yk / val, zk / val
    for u, v_, w in ((xi, yi, zi), (xj, yj, zj)):
        val = u * xk + v_ * yk + w * zk
        if abs(val) > 1.0e-4:
            xk, yk, zk = xk - val * u, yk - val * v_, zk - val * w
            val = nrm(xk, yk, zk)
            if val == 0:
                return ["??"] * 3
            xk, yk, zk = xk / val, yk / val, zk / val
    q = np.array([[xi, xj, xk], [yi, yj, yk], [zi, zj, zk]], dtype=np.float64)
    q = [[f(v) for v in row] for row in q]
    detq = f(_det(q))
    if detq == 0:
        return ["??"] * 3
    vbest, best = f(-666.0), (1, 2, 3, 1, 1, 1)
    for i in (1, 2, 3):
        for j in (1, 2, 3):
            if i == j:
                continue
            for k in (1, 2, 3):
                if k in (i, j):
                    continue
                for p in (-1, 1):
                    for qq in (-1, 1):
                        for r in (-1, 1):
                            pm = [[0.0] * 3 for _ in range(3)]
                            pm[0][i - 1], pm[1][j - 1], pm[2][k - 1] = p, qq, r
                            if _det(pm) * detq <= 0:
                                continue
                            tr = f(0)
                            for row in range(3):
                                s = f(f(pm[row][0]) * q[0][row] + f(pm[row][1]) * q[1][row]
                                      + f(pm[row][2]) * q[2][row])  # fmt: skip
                                tr = f(tr + s)
                            if tr > vbest:
                                vbest, best = tr, (i, j, k, p, qq, r)
    i, j, k, p, qq, r = best
    return [LETTERS[i * p], LETTERS[j * qq], LETTERS[k * r]]


def orientation_rows(m33: np.ndarray) -> list[str]:
    """larmorx's original before 2026-10-29: each column's largest entry, `??` on a clash."""
    rows = [int(np.argmax(np.abs(m33[:, j]))) for j in range(3)]
    out = []
    for j in range(3):
        r = rows[j]
        if rows.count(r) > 1 or m33[r, j] == 0:
            out.append("??")
        else:
            out.append(LETTERS[(r + 1) * (1 if m33[r, j] > 0 else -1)])
    return out


PLANES = re.compile(r"first +\d+ ([xyz])-planes are zero \[from (..?)\]")


def orient(work: Path, n: int) -> None:
    cases = random_cases(n, seed=2)
    # Add matrices at 45 degrees, where two permutations tie.
    s = np.float32(np.sqrt(0.5))
    extra = [
        np.array([[s, -s, 0, 0], [s, s, 0, 0], [0, 0, 1, 0]], np.float32),
        np.array([[s, 0, s, 0], [0, 1, 0, 0], [-s, 0, s, 0]], np.float32),
        np.array([[1, 0, 0, 0], [0, s, -s, 0], [0, s, s, 0]], np.float32),
        np.array([[s, s, 0, 0], [-s, s, 0, 0], [0, 0, 1, 0]], np.float32),
    ]
    cases = extra + cases

    def one(t):
        inp = work / f"o{t}.nii"
        write_input(inp, cases[t])
        err, _ = afni(work, f"o{t}", inp)
        got = dict(PLANES.findall(err))
        return [got.get(a, "--")[:2] for a in "xyz"]

    with ThreadPoolExecutor(12) as ex:
        got = list(ex.map(one, range(len(cases))))
    for name, fn in (
        ("nifti method, float", lambda m: orientation_nifti(m, np.float32)),
        ("nifti method, double", lambda m: orientation_nifti(m, np.float64)),
        ("largest entry per column", orientation_rows),
    ):
        miss = [
            t
            for t, g in enumerate(got)
            if [x[0] for x in fn(cases[t][:, :3].astype(np.float64))] != [x[0] for x in g]
        ]
        print(f"{name:28s} {len(cases) - len(miss):4d} of {len(cases)}   misses {miss[:10]}")
    print("AFNI on the 45-degree matrices:", got[:4])


# ------------------------------------------------------------------------------------------------
# AFNI-07's default (the whole matrix converted) on qform-only headers in metres: the replica
# program's default, the shared default of both implementations, against candidate rules


def _x87mul(a, b):
    return float(LD(a) * LD(b))


def qform_matrix(quat, pixdim, qoffset, qfac, mm=1.0, steps_first=True):
    """The qform's matrix (spec §6.1 step 1, with §7.1's extended-precision products), with the
    voxel sizes multiplied by `mm` first (`steps_first`: as float32 steps, spec §6.1 step 2) or
    the whole float32 matrix converted after (`steps_first=False`)."""
    b, c, d = (float(np.float32(v)) for v in quat)
    s = (b * b + c * c) + d * d
    a = float(LD(1.0) - LD(s))
    if a < 1.0e-7:
        r = float(LD(1.0) / LD(np.sqrt(s)))
        b, c, d, a = b * r, c * r, d * r, 0.0
    else:
        a = float(np.sqrt(a))
    sizes = []
    for p in pixdim:
        p = float(np.float32(p))
        p = p if p > 0 else 1.0
        if steps_first and mm != 1.0:
            p = float(np.float32(p * mm))
        sizes.append(p)
    xd, yd, zd = sizes
    if qfac < 0:
        zd = -zd
    m = np.array([
        [(a * a + b * b - c * c - d * d) * xd, _x87mul(2.0 * (b * c - a * d), yd), _x87mul(2.0 * (b * d + a * c), zd)],
        [_x87mul(2.0 * (b * c + a * d), xd), (a * a + c * c - b * b - d * d) * yd, _x87mul(2.0 * (c * d - a * b), zd)],
        [_x87mul(2.0 * (b * d - a * c), xd), _x87mul(2.0 * (c * d + a * b), yd), (a * a + d * d - c * c - b * b) * zd],
    ])  # fmt: skip
    m = m.astype(np.float32)
    off = np.array([np.float32(v) for v in qoffset], np.float32)
    if steps_first and mm != 1.0:
        off = np.array([np.float32(float(v) * mm) for v in off], np.float32)
    if not steps_first and mm != 1.0:
        m = (m.astype(np.float64) * mm).astype(np.float32)
        off = (off.astype(np.float64) * mm).astype(np.float32)
    return np.concatenate([m, off[:, None]], axis=1)


def qform_units(work: Path, n: int) -> None:
    gpl = os.environ.get("LARMORX_GPL_BIN", str(REPO / "crates-gpl/target/release/larmorx-gpl"))
    rng = np.random.default_rng(11)
    cases = []
    for t in range(n):
        q = rng.integers(-30, 31, 4).astype(float)
        if not q.any():
            q[0] = 1
        q /= np.sqrt((q**2).sum())
        if q[0] < 0:
            q = -q
        units, mm = (1, 1000.0) if t % 2 == 0 else (3, 0.001)
        sp = rng.choice([1.0, 0.0031, 0.0024, 2.5, 0.8, 1.7]) * np.array(rng.choice([1, 1.3], 3))
        off = rng.integers(-100, 100, 3) / rng.choice([1, 7, 1000])
        cases.append((q[1:], sp, off, float(rng.choice([1, -1])), units, mm))

    def one(t):
        quat, sp, off, qfac, units, _ = cases[t]
        inp = work / f"u{t}.nii"
        write_input(inp, None, quat=quat, qoffset=off, pixdim=sp, qfac=qfac, units=units)
        d = work / f"u{t}"
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        env = {"HOME": str(work), "PATH": "/usr/bin:/bin", "AFNI_DONT_LOGFILE": "YES",
               "AFNI_NIFTI_NOEXT": "YES", "LARMORX_UPSTREAM_DEFECTS": "fix"}  # fmt: skip
        subprocess.run([gpl, "afni", "3dAutomask", "-q", "-prefix", "m.nii", str(inp)], cwd=d,
                       env=env, capture_output=True)  # fmt: skip
        raw = (d / "m.nii").read_bytes()
        return np.frombuffer(raw[280:328], "<f4").reshape(3, 4)

    with ThreadPoolExecutor(12) as ex:
        got = list(ex.map(one, range(n)))
    for label, kw in (("steps converted first (float32)", {"steps_first": True}),
                      ("float32 matrix converted after", {"steps_first": False})):  # fmt: skip
        ok = 0
        for t, g in enumerate(got):
            quat, sp, off, qfac, _, mm = cases[t]
            m = qform_matrix(quat, sp, off, qfac, mm, **kw)
            ok += bool(np.array_equal(m.view(np.uint32), g.view(np.uint32)))
        print(f"{label:36s} {ok} of {n} srow bit for bit (the replica's default)")


# ------------------------------------------------------------------------------------------------
# The obliquity angle


ANGLE = re.compile(r"is ([0-9.]+) degrees from plumb")


def angle(work: Path) -> None:
    """Which matrix the angle comes from (sform or qform when both are present and differ), and
    the arithmetic: the angle printed against candidate computations."""
    from math import acos

    def plumb_cos(m):
        out = []
        for j in range(3):
            x, y, z = (np.float32(m[i][j]) for i in range(3))
            s = np.float32(np.float32(np.float32(x * x) + np.float32(y * y)) + np.float32(z * z))
            ln = np.float32(np.sqrt(np.float64(s)))
            mx = max(abs(x), abs(y), abs(z))
            out.append(np.float32(np.float64(mx) / np.float64(ln)))
        return min(out)

    def cand(m):
        c = plumb_cos(m)
        return {
            "acos(float cos), float": f"{np.float32(acos(np.float64(c)) * 180 / 3.141592653):f}",
        }

    rng = np.random.default_rng(3)
    for t in range(12):
        th = rng.uniform(0.05, 0.6)
        ph = th + rng.uniform(-0.01, 0.01)

        def rot(a):
            return np.array([[1, 0, 0], [0, np.cos(a), -np.sin(a)], [0, np.sin(a), np.cos(a)]])

        ms = np.zeros((3, 4), np.float32)
        ms[:, :3] = rot(th) * [2.0, 2.0, 3.0]
        # qform: a rotation by ph about x
        quat = (np.sin(ph / 2), 0.0, 0.0)
        inp = work / f"a{t}.nii"
        write_input(inp, ms, quat=quat, pixdim=(2.0, 2.0, 3.0))
        err, _ = afni(work, f"a{t}", inp)
        got = ANGLE.findall(err)
        mq = rot(ph) * [2.0, 2.0, 3.0]
        print(t, "AFNI", got, "sform", cand(ms), "qform", cand(mq),
              "deg sform", np.degrees(th), "qform", np.degrees(ph))  # fmt: skip


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    work = Path(argv[1]).resolve()
    work.mkdir(parents=True, exist_ok=True)
    n = int(argv[2]) if len(argv) > 2 else 200
    {"quat": lambda: quat(work, n), "orient": lambda: orient(work, n),
     "angle": lambda: angle(work), "qform-units": lambda: qform_units(work, n)}[argv[0]]()  # fmt: skip
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
