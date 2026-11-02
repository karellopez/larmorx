# SPDX-License-Identifier: Apache-2.0
"""A model of FLIRT's boundary-based cost written only from ``specs/flirt.md`` §6.5 (the spec
writer's text of 2026-11-02: the boundary points, the Gaussian of sigma = 2 mm, the 3 × 3 × 3 gradient
masks, the normals, the cost and ``-bbrtype``) and §4.1's clamping (``specs/bet.md`` §4.2's robust
range). It computes the cost of the initial matrix of the suite's ``bbr/default`` images with
``bbrstep`` 1, 2, 200 and 201 (``bbrcost.sch`` at ``setscale 1 force``) and compares it with what
FSL 6.0.7.23 printed (``fsl_runs.py``'s ``bbrcost*`` runs).

    PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 8 \
        python -P validation/investigations/flirt-spec-answers/bbr_model.py <runs dir>

numpy's float32 arithmetic is IEEE single precision (each operation rounded); double values are
float64; ``math.exp`` and ``math.tanh`` are the platform's (glibc's on Linux).
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

F32, F64 = np.float32, np.float64
#: the replica suite's derived inputs, ``<workspace>/oracles/inputs/fsl-flirt-replica`` (the workspace
#: is the parent of this checkout that holds ``oracles/``)
INPUTS = next(
    p / "oracles" / "inputs" / "fsl-flirt-replica"
    for p in Path(__file__).resolve().parents
    if (p / "oracles").is_dir()
)


def read(name: str) -> tuple[np.ndarray, np.ndarray]:
    """An image as FLIRT reads it: single precision, in radiological order (x reversed when the
    header's determinant is positive), with its voxel sizes (single precision)."""
    img = nib.load(str(INPUTS / name))
    a = np.asanyarray(img.dataobj).astype(F32)
    if np.linalg.det(img.affine[:3, :3]) > 0:
        a = a[::-1].copy()
    return a, np.array(img.header["pixdim"][1:4], dtype=F32)


def robust_range(v: np.ndarray) -> tuple[np.float32, np.float32]:
    """``specs/bet.md`` §4.2 with its exact arithmetic (the 2 % and 98 % limits)."""
    data = v.ravel()
    lo0, hi0 = F32(data.min()), F32(data.max())
    if hi0 == lo0:
        return lo0, hi0
    lo, hi = lo0, hi0
    t2 = t98 = lo
    for npass in range(1, 11):
        full = npass == 10
        if full:
            lo, hi = lo0, hi0
        w = F32(hi - lo)
        a = 1000.0 / F64(w)
        b = (1000.0 * -F64(lo)) / F64(w)
        bins = np.trunc(a * data.astype(F64) + b)
        bins = np.clip(bins, 0, 999).astype(np.int64)
        hist = np.bincount(bins, minlength=1000)
        n = int(data.size)
        start, end = 0, 999
        if full:
            n -= int(hist[0] + hist[999])
            start, end = 1, 998
        lim = n // 50
        c, bl = 0, start
        while c < lim:
            c += int(hist[bl])
            bl += 1
        bl -= 1
        c, bu = 0, end
        while c < lim:
            c += int(hist[bu])
            bu -= 1
        bu += 1
        delta = F64(w) / 1000.0
        t2 = F32(lo + F32(bl * delta))
        t98 = F32(lo + F32((bu + 1) * delta))
        if full or not (F64(F32(t98 - t2)) < F64(w) / 10.0):
            break
        nlo = F32(F64(lo) + (max(bl - 1, 0) / 1000.0) * F64(w))
        nhi = F32(F64(lo) + ((min(bu + 1, 999) + 1) / 1000.0) * F64(w))
        lo, hi = nlo, nhi
    return t2, t98


def conv_axis(v: np.ndarray, taps: np.ndarray, axis: int) -> np.ndarray:
    """One pass of the separable convolution: each value the single-precision running sum, in tap
    order (offset −r first), of the double products of value and tap; zero outside."""
    r = (len(taps) - 1) // 2
    n = v.shape[axis]
    acc = np.zeros(v.shape, dtype=F32)
    for m, t in enumerate(taps):
        off = m - r
        sh = np.zeros(v.shape, dtype=F32)
        src = [slice(None)] * 3
        dst = [slice(None)] * 3
        if off >= 0:
            src[axis], dst[axis] = slice(off, n), slice(0, n - off)
        else:
            src[axis], dst[axis] = slice(0, n + off), slice(-off, n)
        sh[tuple(dst)] = v[tuple(src)]
        inside = np.zeros(v.shape, dtype=bool)
        inside[tuple(dst)] = True
        new = (acc.astype(F64) + sh.astype(F64) * F64(t)).astype(F32)
        acc = np.where(inside, new, acc)
    return acc


def gaussian_taps(sigma: np.float32) -> np.ndarray:
    """The Gaussian of sigma voxels: radius ``2·trunc(sigma − 0.001) + 3``, ``2·radius + 1`` taps."""
    radius = math.trunc(F64(sigma) - 0.001) * 2 + 3
    vals, s = [], F32(0.0)
    for j in range(-radius, radius + 1):
        val = F32(math.exp(F64(-(j * j)) / ((2.0 * F64(sigma)) * F64(sigma))))
        vals.append(val)
        s = F32(s + val)
    f = 1.0 / F64(s)
    return np.array([F64(x) * f for x in vals], dtype=F64)


def grad_masks() -> np.ndarray:
    """``m[c][dz+1][dy+1][dx+1]``: the offset along component ``c``'s axis times 3, 1 or 1/3."""
    m = np.zeros((3, 3, 3, 3), dtype=F32)
    for dz in (-1, 0, 1):
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                for c, (along, o1, o2) in enumerate(((dx, dy, dz), (dy, dx, dz), (dz, dx, dy))):
                    e = 1 - abs(o1) - abs(o2)
                    m[c, dz + 1, dy + 1, dx + 1] = F32(along * (3.0**e))
    return m


def gradient(v: np.ndarray) -> list[np.ndarray]:
    m = grad_masks()
    nx, ny, nz = v.shape
    pad = np.zeros((nx + 2, ny + 2, nz + 2), dtype=F32)
    pad[1:-1, 1:-1, 1:-1] = v
    g = [np.zeros(v.shape, dtype=F32) for _ in range(3)]
    for dz in (-1, 0, 1):
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                nb = pad[1 + dx : 1 + dx + nx, 1 + dy : 1 + dy + ny, 1 + dz : 1 + dz + nz]
                for c in range(3):
                    g[c] = (g[c] + (nb * m[c, dz + 1, dy + 1, dx + 1]).astype(F32)).astype(F32)
    return g


def boundary_points(wm: np.ndarray, d: np.ndarray, dref: np.ndarray):
    box = np.ones(3, dtype=F64)
    s = wm
    for ax in range(3):
        s = conv_axis(s, box, ax)
    sm = wm
    for ax in range(3):
        sm = conv_axis(sm, gaussian_taps(F32(F32(2.0) / d[ax])), ax)
    g = gradient(sm)
    sel = (wm.astype(F64) > 0.5) & (s.astype(F64) < 26.5)
    # z, y, x order: transpose so that C order is z outer
    idx = np.argwhere(sel.transpose(2, 1, 0))[:, ::-1]  # columns x, y, z
    x, y, z = idx[:, 0], idx[:, 1], idx[:, 2]
    n = [(-g[c][x, y, z]).astype(F32) / dref[c] for c in range(3)]
    n = [a.astype(F32) for a in n]
    nn = np.sqrt(((n[0] * n[0] + n[1] * n[1]).astype(F32) + n[2] * n[2]).astype(F32)).astype(F32)
    n = [(a / nn).astype(F32) for a in n]
    coord = [idx[:, c].astype(F64) * F64(d[c]) for c in range(3)]
    two = [(a * F32(2.0)).astype(F32).astype(F64) for a in n]
    gm = np.stack([(coord[c] + two[c]).astype(F32) for c in range(3)], axis=1)
    wmp = np.stack([(coord[c] - two[c]).astype(F32) for c in range(3)], axis=1)
    return gm, wmp


def trilinear(v: np.ndarray, p: np.ndarray) -> np.ndarray:
    """Trilinear interpolation at single-precision voxel coordinates, zero outside (each
    neighbour outside the grid counts 0), x first, then y, then z, in single precision."""
    nx, ny, nz = v.shape
    p = p.astype(F32)
    i = np.floor(p).astype(np.int64)
    f = (p - i.astype(F32)).astype(F32)
    pad = np.zeros((nx + 2, ny + 2, nz + 2), dtype=F32)
    pad[1:-1, 1:-1, 1:-1] = v
    out = np.zeros(len(p), dtype=F32)
    ok = np.all((i >= -1) & (i <= np.array([nx - 1, ny - 1, nz - 1])), axis=1)

    def at(a, b, c):
        return pad[i[ok, 0] + 1 + a, i[ok, 1] + 1 + b, i[ok, 2] + 1 + c]

    fx, fy, fz = f[ok, 0], f[ok, 1], f[ok, 2]
    e00 = ((at(1, 0, 0) - at(0, 0, 0)) * fx + at(0, 0, 0)).astype(F32)
    e01 = ((at(1, 0, 1) - at(0, 0, 1)) * fx + at(0, 0, 1)).astype(F32)
    e10 = ((at(1, 1, 0) - at(0, 1, 0)) * fx + at(0, 1, 0)).astype(F32)
    e11 = ((at(1, 1, 1) - at(0, 1, 1)) * fx + at(0, 1, 1)).astype(F32)
    h0 = ((e10 - e00) * fy + e00).astype(F32)
    h1 = ((e11 - e01) * fy + e01).astype(F32)
    out[ok] = ((h1 - h0) * fz + h0).astype(F32)
    return out


def bbr_cost(test, dtest, aff, gm, wmp, step, kind="signed", slope=-0.5):
    t = np.diag([1 / F64(dtest[0]), 1 / F64(dtest[1]), 1 / F64(dtest[2]), 1.0]) @ np.linalg.inv(aff)
    rows = np.arange(0, len(gm), step)

    def vox(p):
        h = np.concatenate([p[rows].astype(F64), np.ones((len(rows), 1))], axis=1)
        return (h @ t.T)[:, :3].astype(F32)

    g = trilinear(test, vox(gm)).astype(F64)
    w = trilinear(test, vox(wmp)).astype(F64)
    s = g + w
    q = np.where(np.abs(s) > 1e-6, 200.0 * (g - w) / np.where(s == 0, 1, s), 0.0)
    u = np.array([1.0 + math.tanh(F64(F32(slope)) * (qq - 0.0)) for qq in q])
    if kind == "local_abs":
        u = np.minimum(u, 2.0 - u)
    tot = 0.0
    for x in u:  # double sums in pair order
        tot += x
    c = tot / len(u)
    if kind == "global_abs":
        c = min(c, 2.0 - c)
    return F32(c)


def main() -> None:
    runs = Path(sys.argv[1])
    wm, dwm = read("wm.nii")
    epi, depi = read("epi.nii")
    aff = np.loadtxt(INPUTS / "init.mat")
    # the reference level at `setscale 1 force`: the reference resampled to s_min = 3 mm
    dref = np.array([3.0, 3.0, 3.0], dtype=F32)
    gm, wmp = boundary_points(wm, dwm, dref)
    lo, hi = robust_range(epi)
    print(f"boundary points: {len(gm)}; input clamped to [{lo:.6f}, {hi:.6f}]")
    clamped = np.where(epi > hi, hi, np.where(epi < lo, lo, epi)).astype(F32)
    bad = 0
    for run, test, kind in (
        ("bbrcost", clamped, "signed"),
        ("bbrcost-noclamp", epi, "signed"),
        ("bbrcost-local_abs", clamped, "local_abs"),
        ("bbrcost-global_abs", clamped, "global_abs"),
    ):
        fsl = [ln.split()[0] for ln in (runs / run / "stdout.txt").read_text().splitlines()]
        mine = [f"{bbr_cost(test, depi, aff, gm, wmp, s, kind):.6f}" for s in (1, 2, 200, 201)]
        ok = mine == fsl
        bad += not ok
        print(f"{run}: FSL {' '.join(fsl)}  model {' '.join(mine)}  {'same' if ok else 'DIFFER'}")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
