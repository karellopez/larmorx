# SPDX-License-Identifier: Apache-2.0
"""Models of FLIRT's final resampling, written from ``specs/flirt.md``'s text alone (the spec
writer's answers of 2026-11-02 to the clean-room question 13: §9.2's windowed sinc, its table,
its lookup and its sums), compared voxel by voxel with what FSL 6.0.7.23 wrote in
``fsl_runs.py``'s ``sinc`` run (``-applyxfm -interp sinc`` of the BOLD reference onto the T1w
by ``init.mat``: the default Hanning window of full width 7, no blur).

    PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 8 \\
        python -P validation/investigations/flirt-spec-answers/resample_model.py <runs> \\
        [--variant double-sums|kernel-order|divide-table]

It models every voxel whose pre-image lies inside the input (FSL's padding mask leaves the
rest at the background value). ``--variant`` shows where another reading parts from FSL:
``double-sums`` (the two sums in double), ``kernel-order`` (the weight as ``x·(y·z)``),
``divide-table`` (the table's lookup position computed in double).
"""

from __future__ import annotations

import ctypes
import ctypes.util
import importlib.util
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("matrices_model", HERE / "matrices_model.py")
assert _spec is not None and _spec.loader is not None
mm = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(mm)

F32 = np.float32
_libm = ctypes.CDLL(ctypes.util.find_library("m"))
_libm.sinf.restype = ctypes.c_float
_libm.sinf.argtypes = [ctypes.c_float]
_libm.cos.restype = ctypes.c_double
_libm.cos.argtypes = [ctypes.c_double]
VARIANT = ""


def sinc_table(w: int = 7, nstore: int = 1201, window: str = "hanning") -> np.ndarray:
    """The tabulated windowed sinc: ``nstore`` samples over ``[-hw, hw]`` (``hw = (w - 1)/2``),
    each a single-precision product of the sinc and the window."""
    hw = (w - 1) // 2
    halfnk = F32((nstore - 1.0) / 2.0)
    ker = np.zeros(nstore)
    for k in range(1, nstore + 1):
        x = F32(F32(F32(F32(k) - halfnk) - F32(1.0)) / halfnk) * F32(hw)
        if abs(float(x)) < 1e-7:
            s = F32(1.0 - abs(float(x)))
        else:
            y = F32(math.pi * float(x))
            s = F32(F32(_libm.sinf(float(y))) / y)
        if abs(float(x)) > hw:
            win = F32(0.0)
        elif window == "hanning":
            win = F32(0.5 + 0.5 * _libm.cos(math.pi * float(x) / hw))
        else:
            raise NotImplementedError(window)
        ker[k - 1] = float(F32(s * win))
    return ker


def kernel_lookup(x: np.ndarray, hw: int, ker: np.ndarray) -> np.ndarray:
    """The table read at ``x`` (single precision): position ``dn = x/hw·halfnk + halfnk + 1``
    (single, the last addition in double then rounded), ``n = ⌊dn⌋``, ``f = dn − n``, the value
    ``k(n)·(1 − f) + k(n + 1)·f`` in double, rounded to single; 0 beyond the half-width or the
    table."""
    nrows = len(ker)
    halfnk = F32((nrows - 1.0) / 2.0)
    x = x.astype(F32)
    if VARIANT == "divide-table":
        dn = (x.astype(np.float64) / hw * float(halfnk) + float(halfnk) + 1.0).astype(F32)
    else:
        dn = ((x / F32(hw)) * halfnk + halfnk).astype(np.float64) + 1.0
        dn = dn.astype(F32)
    n = np.floor(dn).astype(np.int64)
    f = (dn - n.astype(F32)).astype(F32)
    ok = (np.abs(x) <= F32(hw)) & (n <= nrows - 1) & (n >= 1)
    nn = np.clip(n, 1, nrows - 1)
    a, b = ker[nn - 1], ker[nn]
    val = (a * (1.0 - f.astype(np.float64)) + b * f.astype(np.float64)).astype(F32)
    return np.where(ok, val, F32(0.0))


def positions(aff: np.ndarray, d_in, d_out, shape_out) -> tuple[np.ndarray, ...]:
    """The pre-images of every output voxel in input voxels (single precision, accumulated along
    y as the spec says), indexed ``[x, y, z]``."""
    iaff = mm.inv(aff)
    b = mm.mul(iaff, np.diag([*d_out, 1.0]))
    a = np.zeros((4, 4))
    for r in range(4):
        rec = 1.0 / ([*d_in, 1.0][r])
        for c in range(4):
            a[r, c] = rec * b[r, c]
    a = a.astype(F32)
    nx, ny, nz = shape_out
    xs = np.arange(nx, dtype=F32)[:, None]
    zs = np.arange(nz, dtype=F32)[None, :]
    out = []
    for r in range(3):
        o = (xs * a[r, 0] + zs * a[r, 2]).astype(F32) + a[r, 3]
        o = o.astype(F32)
        full = np.zeros((nx, ny, nz), dtype=F32)
        for y in range(ny):
            full[:, y, :] = o
            o = (o + a[r, 1]).astype(F32)
        out.append(full)
    return tuple(out)


def sinc_values(img: np.ndarray, px, py, pz, ker: np.ndarray, hw: int) -> np.ndarray:
    """The sinc at the given positions: per axis the table read at the 2·hw + 1 distances, the
    weight of each neighbour inside the grid the single product ``(w_x·w_y)·w_z``, the value
    times weight and the weight summed in single precision (z outer, then y, then x), the value
    their quotient."""
    nx, ny, nz = img.shape
    ix0, iy0, iz0 = (np.floor(p).astype(np.int64) for p in (px, py, pz))
    acc_t = np.float64 if VARIANT == "double-sums" else F32
    conv = np.zeros(px.shape, dtype=acc_t)
    ksum = np.zeros(px.shape, dtype=acc_t)

    def store(p, i0, d):
        return kernel_lookup(((p - i0.astype(F32)).astype(F32) + F32(d)).astype(F32), hw, ker)

    sx = {d: store(px, ix0, d) for d in range(-hw, hw + 1)}
    sy = {d: store(py, iy0, d) for d in range(-hw, hw + 1)}
    sz = {d: store(pz, iz0, d) for d in range(-hw, hw + 1)}
    for dz in range(-hw, hw + 1):
        z1 = iz0 + dz
        for dy in range(-hw, hw + 1):
            y1 = iy0 + dy
            for dx in range(-hw, hw + 1):
                x1 = ix0 + dx
                inside = (x1 >= 0) & (x1 < nx) & (y1 >= 0) & (y1 < ny) & (z1 >= 0) & (z1 < nz)
                wx, wy, wz = sx[-dx], sy[-dy], sz[-dz]
                if VARIANT == "kernel-order":
                    kf = (wx * (wy * wz).astype(F32)).astype(F32)
                else:
                    kf = ((wx * wy).astype(F32) * wz).astype(F32)
                v = img[np.clip(x1, 0, nx - 1), np.clip(y1, 0, ny - 1), np.clip(z1, 0, nz - 1)]
                term = (v * kf).astype(F32).astype(acc_t)
                conv = np.where(inside, (conv + term).astype(acc_t), conv)
                ksum = np.where(inside, (ksum + kf.astype(acc_t)).astype(acc_t), ksum)
    return (conv / ksum).astype(F32)


def blur_kernel(r: float, d: float) -> np.ndarray:
    """§5's blur to the resolution ``r`` along an axis of voxel size ``d``."""
    ratio = F32(F32(r) / F32(d))
    sigma = F32(F32(F32(0.85) * ratio) / F32(2.0))
    if abs(d) < 1e-8 or ratio < F32(1.1) or sigma < F32(0.5):
        return np.array([1.0])
    n = 2 * int(float(sigma) - 0.001) + 3
    c = n // 2 + 1
    den = float(F32(F32(sigma * sigma) * F32(4.0)))
    taps = [math.exp(-float(F32((k - c) * (k - c))) / den) for k in range(1, n + 1)]
    s_even = sum(taps[0::2][1:], taps[0])
    s_odd = 0.0
    for t in taps[1::2]:
        s_odd += t
    total = s_even + s_odd
    return np.array([t / total for t in taps])


def convolve_axis(v: np.ndarray, ker: np.ndarray, axis: int, pad: float) -> np.ndarray:
    """One pass: each value the single-precision running sum, in kernel order, of the double
    products of the neighbour (``pad`` beyond the image: the constant edge rule) and the tap."""
    n = len(ker)
    mid = (n - 1) // 2
    size = v.shape[axis]
    acc = np.zeros(v.shape, dtype=F32)
    for m in range(n):
        off = m - mid
        idx = np.arange(size) + off
        ok = (idx >= 0) & (idx < size)
        nb = np.take(v, np.clip(idx, 0, size - 1), axis=axis).astype(np.float64)
        shape = [1, 1, 1]
        shape[axis] = size
        nb = np.where(ok.reshape(shape), nb, pad)
        acc = (acc.astype(np.float64) + nb * ker[m]).astype(F32)
    return acc


def spline_coefficients(v: np.ndarray) -> np.ndarray:
    """Cubic B-spline coefficients: along x, then y, then z, each column in double (pole
    ``z = √3 − 2``; the causal start ``c₀ + Σ_{i<n} zⁱ cᵢ`` with ``n = ⌊ln(1e-8)/ln|z| + 1.5⌋``
    capped at the column's length, the powers by repeated products; the anti-causal start
    ``−z/(1 − z²)·(2c_last − l)`` with ``l`` the column's last input value; times 6), stored in
    single precision after each axis."""
    zp = math.sqrt(3.0) - 2.0
    out = v.astype(F32)
    for axis in range(3):
        size = out.shape[axis]
        if size <= 1:
            continue
        col = np.moveaxis(out, axis, 0).astype(np.float64).copy()
        nterm = min(int(math.log(1e-8) / math.log(abs(zp)) + 1.5), size)
        lv = col[size - 1].copy()
        iv = col[0].copy()
        z2i = zp
        for i in range(1, nterm):
            iv = iv + z2i * col[i]
            z2i *= zp
        col[0] = iv
        for i in range(1, size):
            col[i] = col[i] + zp * col[i - 1]
        col[size - 1] = (-zp / (1.0 - zp * zp)) * (2.0 * col[size - 1] - lv)
        for i in range(size - 2, -1, -1):
            col[i] = zp * (col[i + 1] - col[i])
        col = col * 6.0
        out = np.moveaxis(col.astype(F32), 0, axis)
    return out


def cubic_weight(x: np.ndarray) -> np.ndarray:
    ax = np.abs(x)
    a2 = 2.0 - ax
    inner = 2.0 / 3.0 + ((0.5 * ax) * ax) * (ax - 2.0)
    outer = (1.0 / 6.0) * ((a2 * a2) * a2)
    return np.where(ax < 1.0, inner, np.where(ax < 2.0, outer, 0.0))


def spline_values(coef: np.ndarray, px, py, pz, zeros: bool) -> np.ndarray:
    """The spline at the positions: start indices ``⌈p⌉ − 2``, four weights per axis, the sum
    over z (outer), y, x (inner) of ``(c·w_x)·(w_z·w_y)`` in double; a coefficient outside the
    grid counts 0 (``zeros``) or repeats the edge one; rounded to single."""
    nx, ny, nz = coef.shape
    p = [px.astype(np.float64), py.astype(np.float64), pz.astype(np.float64)]
    s = [np.ceil(q).astype(np.int64) - 2 for q in p]
    w = [[cubic_weight(q - (si + i)) for i in range(4)] for q, si in zip(p, s, strict=True)]
    val = np.zeros(px.shape)
    for k in range(4):
        for j in range(4):
            wgt2 = w[2][k] * w[1][j]
            for i in range(4):
                ix, iy, iz = s[0] + i, s[1] + j, s[2] + k
                ok = (ix >= 0) & (ix < nx) & (iy >= 0) & (iy < ny) & (iz >= 0) & (iz < nz)
                c = coef[np.clip(ix, 0, nx - 1), np.clip(iy, 0, ny - 1), np.clip(iz, 0, nz - 1)]
                c = c.astype(np.float64)
                if zeros:
                    c = np.where(ok, c, 0.0)
                val = val + (c * w[0][i]) * wgt2
    return val.astype(F32)


def check_spline(runs: Path) -> None:
    """Question 14: ``-applyisoxfm 6 -setbackground 40 -interp spline`` of four volumes."""
    import nibabel as nib

    d = runs / "isoxfm-setbg"
    bold = nib.load(str(d / "bold4.nii"))
    raw = np.asanyarray(bold.dataobj.get_unscaled())
    slope, inter = F32(bold.dataobj.slope), F32(bold.dataobj.inter)
    vols = ((raw.astype(F32) * slope).astype(F32) + inter).astype(F32)  # a radiological file
    t1 = nib.load(str(d / "t1.nii"))
    d_in = [float(v) for v in bold.header.get_zooms()[:3]]
    d_ref = [float(v) for v in t1.header.get_zooms()[:3]]
    step = [F32(F32(6.0) / F32(v)) for v in d_ref]
    shape = tuple(
        int(max(1.0, float(F32(n) / st))) for n, st in zip(t1.shape[:3], step, strict=True)
    )
    aff = mm.read_matrix(d / "init.mat")
    px, py, pz = positions(aff, d_in, [6.0, 6.0, 6.0], shape)
    fsl = np.asanyarray(nib.load(str(d / "out.nii.gz")).dataobj).astype(F32)[::-1]
    n = vols.shape[:3]
    inside = (
        (px >= 0) & (px <= n[0] - 1) & (py >= 0) & (py <= n[1] - 1) & (pz >= 0) & (pz <= n[2] - 1)
    )
    pad = 40.0
    readings = {
        "the spec (the blur reads 40 outside; zero coefficients outside)": (pad, True),
        "the blur reading zeros outside": (0.0, True),
        "the edge coefficients repeated outside": (pad, False),
    }
    for name, (blur_pad, zeros) in readings.items():
        same = 0
        for t in range(vols.shape[3]):
            v = vols[..., t]
            for axis, dd in enumerate(d_in):
                v = convolve_axis(v, blur_kernel(6.0, dd), axis, blur_pad)
            coef = spline_coefficients(v)
            got = np.full(shape, F32(pad), dtype=F32)
            got[inside] = spline_values(coef, px[inside], py[inside], pz[inside], zeros)
            same += int(np.sum(got.view(np.uint32) == fsl[..., t].view(np.uint32)))
        total = fsl.size
        print(f"spline (question 14), {name}: {same} of {total} values bit-identical to FSL's")


def main() -> None:
    global VARIANT
    import nibabel as nib

    args = sys.argv[1:]
    if "--spline" in args:
        args.remove("--spline")
        check_spline(Path(args[0]))
        return
    if "--variant" in args:
        i = args.index("--variant")
        VARIANT = args[i + 1]
        del args[i : i + 2]
    d = Path(args[0]) / "sinc"
    epi = nib.load(str(d / "epi.nii"))
    t1 = nib.load(str(d / "t1.nii"))
    out = np.asanyarray(nib.load(str(d / "out.nii.gz")).dataobj).astype(F32)
    img = np.asanyarray(epi.dataobj).astype(F32)  # a radiological file: as stored
    d_in = [float(v) for v in epi.header.get_zooms()[:3]]
    d_out = [float(v) for v in t1.header.get_zooms()[:3]]
    shape = t1.shape[:3]
    aff = mm.read_matrix(d / "init.mat")
    px, py, pz = positions(aff, d_in, d_out, shape)
    # the T1w is a neurological file: FLIRT's output grid is it reversed along x
    fsl = out[::-1, :, :]
    n = img.shape
    inside = (
        (px >= 0) & (px <= n[0] - 1) & (py >= 0) & (py <= n[1] - 1) & (pz >= 0) & (pz <= n[2] - 1)
    )
    idx = np.nonzero(inside)
    rng = np.random.default_rng(13)
    pick = rng.choice(len(idx[0]), size=min(40000, len(idx[0])), replace=False)
    sel = tuple(a[pick] for a in idx)
    ker = sinc_table()
    got = sinc_values(img, px[sel], py[sel], pz[sel], ker, 3)
    want = fsl[sel]
    same = int(np.sum(got.view(np.uint32) == want.view(np.uint32)))
    print(
        f"sinc (question 13, variant {VARIANT or 'the spec'}): {same} of {len(pick)} voxels "
        f"bit-identical to FSL's (of {len(idx[0])} inside the input); "
        f"max |difference| {float(np.max(np.abs(got - want))):.3g}"
    )


if __name__ == "__main__":
    main()
