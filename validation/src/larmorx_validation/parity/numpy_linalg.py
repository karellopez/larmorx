# SPDX-License-Identifier: Apache-2.0
"""Parity of larmorx's 4×4 matrix arithmetic with numpy's (``larmorx_transform::openblas``).

``lx.transforms`` computes the 4×4 matrices of nitransforms and fMRIPrep (inverses, products,
the ITK-to-RAS conversion, head motion in voxel space, ``ensure_positive_cosines``) in Rust,
in the operation order of OpenBLAS 0.3.30's Haswell kernels, which numpy uses on x86-64 CPUs
with AVX2 and FMA. On such a machine every result must be bit-identical to numpy's:

- ``products``: ``a @ b`` (``dgemm``) of random, affine, badly scaled, sparse (signed zeros)
  and non-finite matrices, the ITK-to-RAS chain and fMRIPrep's head-motion expression
  ``inv(vox2ras) @ M @ vox2ras``;
- ``inverses``: ``np.linalg.inv`` (``dgesv``) of random, affine, rigid, badly scaled,
  near-singular, permuted, tied (equal pivot candidates), triangular, subnormal-pivot, singular
  and non-finite matrices;
- ``points``: ``Affine.map`` and ``DenseField.map`` against nitransforms for 1 to 64 points
  (a single point goes through ``dgemv``, more through ``dgemm``);
- ``orientation``: ``ensure_positive_cosines`` against sdcflows (nibabel's ``io_orientation``
  and ``as_reoriented``) on all 48 axis orders and flips, oblique and exactly 45° grids;
- ``real``: every transform file, source affine and head-motion series of the
  ``resample-series`` suite, loaded by larmorx and by fMRIPrep's ``load_transforms``.

Inputs are built with elementwise numpy and larmorx's own products only, so they are the same
whichever OpenBLAS kernel numpy runs. The suite skips itself when numpy's OpenBLAS core is not
one whose kernels larmorx reproduces (``KERNELS``).

``python -m larmorx_validation.parity.numpy_linalg kernels`` runs the random families under
each OpenBLAS kernel this CPU can execute (``OPENBLAS_CORETYPE``, one process each) and
tabulates how often numpy agrees with larmorx and with numpy's Haswell kernel.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.parity.harness import (
    EXPECTED,
    SKIPPED,
    Case,
    CaseResult,
    CheckList,
    Outcome,
    Suite,
)

#: numpy's OpenBLAS cores whose 4×4 arithmetic larmorx reproduces (Zen runs the same dgemm,
#: dgemv, ddot, dscal and triangular-solve kernels as Haswell; checked with ``kernels``).
KERNELS = ("Haswell", "Zen")
#: Matrices per family and tier.
N = {"smoke": 2_000, "standard": 1_000_000, "full": 10_000_000}
#: Point-mapping and orientation cases per tier.
N_SMALL = {"smoke": 200, "standard": 20_000, "full": 100_000}
CHUNK = 250_000
_TIERS = ("smoke", "standard", "full")


def openblas_core() -> str:
    """The OpenBLAS core numpy runs (threadpoolctl), or "unknown"."""
    try:
        import threadpoolctl
    except ImportError:
        return "unknown"
    for info in threadpoolctl.threadpool_info():
        if info.get("internal_api") == "openblas" and "numpy" in info.get("filepath", ""):
            return str(info.get("architecture", "unknown"))
    return "unknown"


# ------------------------------------------------------------------------------------------------
# Inputs: elementwise numpy and larmorx's products only (the same under every BLAS kernel)


def _matmul(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    from larmorx.transforms._linalg import matmul

    return matmul(a, b)


def _rotations(rng: np.random.Generator, n: int, max_angle: float | None = None) -> np.ndarray:
    """Random 3×3 rotations from unit quaternions (elementwise arithmetic only); with
    ``max_angle``, angles uniform in [0, max_angle] about random axes."""
    if max_angle is None:
        q = rng.normal(size=(n, 4))
        q /= np.sqrt(np.sum(q * q, axis=1))[:, None]
    else:
        axis = rng.normal(size=(n, 3))
        axis /= np.sqrt(np.sum(axis * axis, axis=1))[:, None]
        half = rng.uniform(0, max_angle, n) / 2
        q = np.concatenate([np.cos(half)[:, None], np.sin(half)[:, None] * axis], axis=1)
    w, x, y, z = q.T
    return np.stack(
        [
            np.stack([1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)], -1),
            np.stack([2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)], -1),
            np.stack([2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)], -1),
        ],
        1,
    )


def _affines(
    rng: np.random.Generator,
    n: int,
    zooms: tuple[float, float] = (0.5, 4.0),
    shift: float = 100.0,
    max_angle: float | None = None,
) -> np.ndarray:
    a = np.zeros((n, 4, 4))
    a[:, :3, :3] = _rotations(rng, n, max_angle) * rng.uniform(*zooms, size=(n, 1, 3))
    a[:, :3, 3] = rng.normal(size=(n, 3)) * shift
    a[:, 3, 3] = 1.0
    return a


def _sparse(rng: np.random.Generator, n: int) -> np.ndarray:
    vals = np.array([-2.0, -1.0, -0.5, -0.0, 0.0, 0.0, 0.5, 1.0, 2.0])
    return vals[rng.integers(0, len(vals), size=(n, 4, 4))]


def _nonfinite(rng: np.random.Generator, n: int) -> np.ndarray:
    a = rng.normal(size=(n, 4, 4))
    k = rng.integers(1, 3, size=n)
    for j in range(2):
        rows = np.flatnonzero(k > j)
        a[rows, rng.integers(0, 4, len(rows)), rng.integers(0, 4, len(rows))] = rng.choice(
            [np.nan, np.inf, -np.inf], size=len(rows)
        )
    return a


def _product_inputs(kind: str, rng: np.random.Generator, n: int) -> tuple[np.ndarray, np.ndarray]:
    if kind == "general":
        return rng.normal(size=(n, 4, 4)), rng.normal(size=(n, 4, 4))
    if kind == "affine":
        return _affines(rng, n), _affines(rng, n)
    if kind == "exponents":
        # Products from 2^-1040 to 2^1040: overflow, subnormal and underflowing results.
        def scaled() -> np.ndarray:
            return rng.normal(size=(n, 4, 4)) * 2.0 ** rng.integers(-520, 521, size=(n, 4, 4))

        return scaled(), scaled()
    if kind == "sparse":
        return _sparse(rng, n), _sparse(rng, n)
    if kind == "nonfinite":
        return _nonfinite(rng, n), rng.normal(size=(n, 4, 4))
    raise ValueError(kind)


def _orthogonal_scaled(rng: np.random.Generator, n: int, s: np.ndarray) -> np.ndarray:
    """``U · diag(s) · Vᵀ`` with random rotations U and V (larmorx's products)."""
    u = np.zeros((n, 4, 4))
    v = np.zeros((n, 4, 4))
    u[:, :3, :3], v[:, :3, :3] = _rotations(rng, n), _rotations(rng, n)
    u[:, 3, 3] = v[:, 3, 3] = 1.0
    perm = rng.permuted(np.tile(np.arange(4), (n, 1)), axis=1)
    u = u[np.arange(n)[:, None], perm]  # mix the fourth axis in
    d = np.zeros((n, 4, 4))
    d[:, np.arange(4), np.arange(4)] = s
    return _matmul(_matmul(u, d), np.swapaxes(v, 1, 2))


def _inverse_inputs(kind: str, rng: np.random.Generator, n: int) -> np.ndarray:
    if kind == "general":
        return rng.normal(size=(n, 4, 4))
    if kind == "affine":
        return _affines(rng, n)
    if kind == "rigid":
        return _affines(rng, n, zooms=(1.0, 1.0), shift=5.0, max_angle=0.2)
    if kind == "exponents":
        # Rows and columns scaled by 2^-300 to 2^300.
        a = rng.normal(size=(n, 4, 4))
        return (
            a
            * 2.0 ** rng.integers(-300, 301, (n, 4, 1))
            * 2.0 ** rng.integers(-300, 301, (n, 1, 4))
        )
    if kind == "near-singular":
        # Condition numbers from 1e3 to 1e16.
        s = np.stack([np.ones(n), *(10.0 ** -rng.uniform(0, 16, (3, n)))], 1)
        return _orthogonal_scaled(rng, n, s)
    if kind == "permutation":
        # Signed permutations times a diagonal, some with small off-diagonal terms.
        perm = rng.permuted(np.tile(np.arange(4), (n, 1)), axis=1)
        a = np.zeros((n, 4, 4))
        a[np.arange(n)[:, None], np.arange(4), perm] = rng.choice(
            [-1.0, 1.0], (n, 4)
        ) * rng.uniform(0.25, 4, (n, 4))
        a += (rng.random((n, 4, 4)) < 0.2) * rng.normal(size=(n, 4, 4)) * 1e-3
        return a
    if kind == "ties":
        return np.array([-2.0, -1.0, 1.0, 2.0])[rng.integers(0, 4, size=(n, 4, 4))]
    if kind == "triangular":
        a = rng.normal(size=(n, 4, 4))
        upper = rng.random(n) < 0.5
        return np.where(upper[:, None, None], np.triu(a), np.tril(a))
    if kind == "subnormal":
        # One column scaled into the subnormal range: pivots below DBL_MIN.
        a = rng.normal(size=(n, 4, 4))
        a[np.arange(n), :, rng.integers(0, 4, n)] *= 1e-310
        return a
    if kind == "singular":
        # A row repeated or a column zeroed: mostly exactly singular.
        a = rng.normal(size=(n, 4, 4))
        dup = rng.random(n) < 0.5
        i, j = rng.integers(0, 4, n), rng.integers(1, 4, n)
        rows = np.arange(n)
        a[rows[dup], (i[dup] + j[dup]) % 4] = a[rows[dup], i[dup]]
        a[rows[~dup], :, i[~dup]] = 0.0
        return a
    if kind == "nonfinite":
        return _nonfinite(rng, n)
    raise ValueError(kind)


PRODUCTS = ("general", "affine", "exponents", "sparse", "nonfinite")
INVERSES = (
    "general",
    "affine",
    "rigid",
    "exponents",
    "near-singular",
    "permutation",
    "ties",
    "triangular",
    "subnormal",
    "singular",
    "nonfinite",
)
NAN_NOTE = "; NaN compared as NaN, not by its bits"
_SEEDS = {k: 100 + i for i, k in enumerate((*PRODUCTS, *(f"inv-{k}" for k in INVERSES)))}


# ------------------------------------------------------------------------------------------------
# Comparisons


def _same(a: np.ndarray, b: np.ndarray, nan_bits: bool = True) -> np.ndarray:
    """Per matrix: every value has the same bits. With ``nan_bits=False`` any NaN equals any
    NaN: which NaN an operation on two NaNs returns, and the sign of a NaN it creates, depend
    on operand order inside the CPU (x86 creates -NaN, ARM +NaN)."""
    a, b = np.ascontiguousarray(a, np.float64), np.ascontiguousarray(b, np.float64)
    if not nan_bits:
        a = np.where(np.isnan(a), np.nan, a)
        b = np.where(np.isnan(b), np.nan, b)
    return np.all(a.view(np.uint64) == b.view(np.uint64), axis=(-2, -1))


def _numpy_inv(a: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """``np.linalg.inv`` per matrix, and which matrices numpy calls singular (NaN there)."""
    with np.errstate(all="ignore"):
        try:
            return np.linalg.inv(a), np.zeros(len(a), bool)
        except np.linalg.LinAlgError:
            pass
        out = np.full_like(a, np.nan)
        singular = np.zeros(len(a), bool)
        for i in range(len(a)):
            try:
                out[i] = np.linalg.inv(a[i])
            except np.linalg.LinAlgError:
                singular[i] = True
        return out, singular


def _larmorx_inv(a: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    from larmorx import _core

    out, singular = _core.linalg_inv4(np.ascontiguousarray(a))
    return out, singular


def _chunks(n: int):
    for start in range(0, n, CHUNK):
        yield start, min(n, start + CHUNK)


def _count(checks: CheckList, name: str, same: int, total: int, extra: str = "") -> None:
    checks(
        name,
        same == total,
        metric="bit-identical",
        value=f"{same} of {total}",
        threshold="all",
        detail="" if same == total else f"{total - same} differ{extra}",
    )


def _run_products(kind: str, n: int, checks: CheckList) -> None:
    from larmorx import _core

    rng = np.random.default_rng(_SEEDS[kind])
    same = 0
    for s, e in _chunks(n):
        a, b = _product_inputs(kind, rng, e - s)
        with np.errstate(all="ignore"):
            ref = a @ b
        got = _core.linalg_matmul4(np.ascontiguousarray(a), np.ascontiguousarray(b))
        same += int(_same(ref, got, nan_bits=kind != "nonfinite").sum())
    _count(checks, f"a @ b ({kind}{NAN_NOTE if kind == 'nonfinite' else ''})", same, n)


def _run_inverses(kind: str, n: int, checks: CheckList) -> None:
    rng = np.random.default_rng(_SEEDS[f"inv-{kind}"])
    same = both_singular = disagree_singular = 0
    for s, e in _chunks(n):
        a = _inverse_inputs(kind, rng, e - s)
        ref, ref_sing = _numpy_inv(a)
        got, got_sing = _larmorx_inv(a)
        both_singular += int((ref_sing & got_sing).sum())
        disagree_singular += int((ref_sing != got_sing).sum())
        ok = _same(ref, got, nan_bits=kind != "nonfinite") & ~ref_sing & ~got_sing
        same += int((ok | (ref_sing & got_sing)).sum())
    extra = f"; {disagree_singular} singular in only one" if disagree_singular else ""
    note = NAN_NOTE if kind == "nonfinite" else ""
    _count(checks, f"np.linalg.inv ({kind}{note})", same, n, extra)
    if both_singular:
        checks("singular in both (numpy raises)", True, metric="count", value=both_singular)


def _run_chains(kind: str, n: int, checks: CheckList) -> None:
    """The ITK-to-RAS conversion and fMRIPrep's head motion in voxel space."""
    from larmorx.transforms._linalg import inv, matmul

    rng = np.random.default_rng(_SEEDS.get(kind, 7))
    lps = np.diag([-1, -1, 1, 1])
    same = 0
    for s, e in _chunks(n):
        m = e - s
        if kind == "itk-to-ras":
            # nitransforms: float32 parameters and centre, LPS @ C+ @ M @ C- @ LPS.
            mat = (
                _affines(rng, m, zooms=(0.8, 1.25), shift=20.0)
                .astype(np.float32)
                .astype(np.float64)
            )
            centre = (rng.normal(size=(m, 3)) * 30).astype(np.float32).astype(np.float64)
            c_neg = np.tile(np.eye(4), (m, 1, 1))
            c_pos = c_neg.copy()
            c_neg[:, :3, 3] = centre * -1.0
            c_pos[:, :3, 3] = centre
            ref = lps @ (c_pos @ (mat @ (c_neg @ lps)))
            got = matmul(lps, matmul(c_pos, matmul(mat, matmul(c_neg, lps))))
        else:  # head-motion: ras2vox @ M @ vox2ras for each volume
            vox2ras = _affines(rng, m, zooms=(1.5, 4.0), shift=100.0, max_angle=0.6)
            motion = _affines(rng, m, zooms=(1.0, 1.0), shift=2.0, max_angle=0.05)
            ref = np.linalg.inv(vox2ras) @ motion @ vox2ras
            got = matmul(matmul(inv(vox2ras), motion), vox2ras)
        same += int(_same(ref, got).sum())
    _count(checks, kind, same, n)


def _run_points(n_points: int, n: int, checks: CheckList) -> None:
    import nitransforms as nt

    import larmorx as lx

    rng = np.random.default_rng(1000 + n_points)
    same = 0
    for _ in range(n):
        a = _affines(rng, 1)[0]
        p = rng.normal(size=(n_points, 3)) * 60
        same += _same(nt.linear.Affine(a).map(p)[None], lx.transforms.Affine(a).map(p)[None])[0]
    _count(checks, f"Affine.map, {n_points} point(s)", int(same), n)


def _run_field(n: int, checks: CheckList) -> None:
    """DenseField.map (its index lookup is a product with the grid's inverse) vs nitransforms."""
    import nibabel as nib
    import nitransforms as nt

    import larmorx as lx

    rng = np.random.default_rng(77)
    same = total = 0
    for i in range(max(1, n // 50)):
        shape = (7, 6, 5)
        aff = _affines(rng, 1, zooms=(2.0, 4.0), shift=10.0)[0]
        deltas = rng.normal(size=(*shape, 3))
        field = nt.nonlinear.DenseFieldTransform(nib.Nifti1Image(deltas, aff), is_deltas=True)
        mine = lx.transforms.DenseField(deltas, aff)
        for n_points in (1, 2, 9):
            if i % 2:  # on the grid: looked up
                idx = np.stack([rng.integers(0, s, n_points) for s in shape], 1).astype(float)
                p = idx @ aff[:3, :3].T + aff[:3, 3]
            else:  # off the grid: interpolated
                p = aff[:3, 3] + rng.uniform(0, 4, (n_points, 3)) @ aff[:3, :3].T
            same += bool(_same(field.map(p)[None], mine.map(p)[None])[0])
            total += 1
    _count(checks, "DenseField.map (1, 2 and 9 points, on and off the grid)", same, total)


# ------------------------------------------------------------------------------------------------
# Orientation (sdcflows' ensure_positive_cosines)


def _signed_permutations() -> list[np.ndarray]:
    import itertools

    out = []
    for perm in itertools.permutations(range(3)):
        for signs in itertools.product((-1.0, 1.0), repeat=3):
            m = np.zeros((3, 3))
            m[list(perm), range(3)] = signs
            out.append(m)
    return out


def _run_orientation(kind: str, n: int, checks: CheckList) -> None:
    import nibabel as nib
    from sdcflows.utils.tools import ensure_positive_cosines as sdc_epc

    import larmorx as lx

    rng = np.random.default_rng(55 if kind == "oblique" else 56)
    perms = _signed_permutations()
    data = np.arange(4 * 3 * 2, dtype=np.float32).reshape(4, 3, 2)
    decisions = affines = total = 0
    for i in range(n):
        base = perms[i % len(perms)]
        aff = np.eye(4)
        if kind == "oblique":
            rot = _rotations(rng, 1, max_angle=0.7)[0]  # up to 40°
        else:  # exactly 45° about one axis (cosines tie)
            c = np.sqrt(0.5)
            k = i % 3
            rot = np.eye(3)
            u, v = [x for x in range(3) if x != k]
            rot[[u, u, v, v], [u, v, u, v]] = (c, -c, c, c)
        aff[:3, :3] = rot @ base * rng.uniform(1.0, 3.0, 3) if kind == "oblique" else rot @ base
        aff[:3, 3] = rng.normal(size=3) * 50
        img, codes = sdc_epc(nib.Nifti1Image(data, aff))
        d, a, mine = lx.transforms.ensure_positive_cosines(data, aff)
        same_dec = tuple(codes) == tuple(mine) and np.array_equal(np.asarray(img.dataobj), d)
        decisions += same_dec
        affines += bool(same_dec and _same(img.affine[None], np.asarray(a)[None])[0])
        total += 1
    if kind == "45-degrees" and decisions < total:
        raise Outcome(
            EXPECTED,
            f"axis decisions agree for {decisions} of {total} grids rotated by exactly 45°: two "
            "cosines tie, and nibabel decides with numpy's LAPACK SVD, larmorx with vnl's",
        )
    _count(checks, "axis codes and reoriented data", decisions, total)
    _count(checks, "new affine", affines, total)


# ------------------------------------------------------------------------------------------------
# Real transforms (the resample-series suite's)


def _run_real(checks: CheckList) -> None:
    import nibabel as nib
    from fmriprep.utils.transforms import load_transforms as fmriprep_load
    from nitransforms.linear import LinearTransformsMapping
    from nitransforms.nonlinear import DenseFieldTransform
    from sdcflows.utils.tools import ensure_positive_cosines as sdc_epc

    import larmorx as lx
    from larmorx.transforms._linalg import inv, matmul
    from larmorx_validation.parity import resample_series as rs

    seen: set = set()
    counts = {"steps": [0, 0], "ras2vox": [0, 0], "hmc": [0, 0]}
    with tempfile.TemporaryDirectory() as tmp:
        for _cid, _cat, _desc, s in rs._scenarios():
            key = (s.source, s.transforms, s.inverse, bool(s.pe_dir and s.ro_time))
            if key in seen or s.expect_error:
                continue
            seen.add(key)
            xfms = [rs._resolve(p, Path(tmp)) for p in s.transforms]
            inverse = list(s.inverse) if len(s.inverse) == len(xfms) else [s.inverse[0]] * len(xfms)
            ref_chain = fmriprep_load(xfms, inverse) if xfms else None
            mine = lx.transforms.load_transforms(xfms, inverse) if xfms else None
            ref_steps = (
                list(ref_chain.transforms) if hasattr(ref_chain, "transforms") else [ref_chain]
            )
            my_steps = mine.steps() if isinstance(mine, lx.transforms.TransformChain) else [mine]
            hmc_ref = hmc_mine = None
            for r, m in zip(ref_steps, my_steps, strict=True):
                if r is None:
                    continue
                if isinstance(r, LinearTransformsMapping):
                    pairs = [(r.matrix, m.matrices)]
                    hmc_ref, hmc_mine = r, m
                elif isinstance(r, DenseFieldTransform):
                    pairs = [(r.reference.affine, m.affine), (r.reference.inverse, m.inverse)]
                else:
                    pairs = [(r.matrix, m.matrix)]
                for x, y in pairs:
                    x = np.asarray(x, np.float64).reshape(-1, 4, 4)
                    y = np.asarray(y, np.float64).reshape(-1, 4, 4)
                    counts["steps"][0] += int(_same(x, y).sum())
                    counts["steps"][1] += len(x)
            src = nib.load(rs._resolve(s.source, Path(tmp)))
            img = src
            if s.pe_dir and s.ro_time:
                # fMRIPrep reorients the source first; compare the new affine too.
                img, _ = sdc_epc(src)
                zeros = np.zeros(src.shape[:3], np.float32)
                _, mine_affine, _ = lx.transforms.ensure_positive_cosines(zeros, src.affine)
                counts["ras2vox"][0] += int(_same(img.affine[None], mine_affine[None])[0])
                counts["ras2vox"][1] += 1
            vox2ras = img.affine
            ras2vox = np.linalg.inv(vox2ras)
            counts["ras2vox"][0] += int(_same(ras2vox[None], inv(vox2ras)[None])[0])
            counts["ras2vox"][1] += 1
            if hmc_ref is not None:
                ref = np.stack([ras2vox @ x.matrix @ vox2ras for x in hmc_ref])
                got = matmul(matmul(inv(vox2ras), hmc_mine.matrices), vox2ras)
                counts["hmc"][0] += int(_same(ref, got).sum())
                counts["hmc"][1] += len(ref)
    _count(
        checks,
        "loaded matrices (affines, head-motion series, field grids and inverses)",
        *counts["steps"],
    )
    _count(
        checks,
        "source affines after ensure_positive_cosines, and their inverses",
        *counts["ras2vox"],
    )
    _count(checks, "head motion in voxel space (ras2vox @ M @ vox2ras)", *counts["hmc"])


# ------------------------------------------------------------------------------------------------
# Suite


def cases(tier: str) -> list[Case]:
    n, small = N[tier], N_SMALL[tier]
    out = [
        Case(f"products/{k}", "products", f"a @ b, {n:,} {k} 4×4 pairs", ("products", k, n))
        for k in PRODUCTS
    ]
    out += [
        Case(
            f"products/{k}",
            "products",
            {
                "itk-to-ras": f"nitransforms' ITK-to-RAS conversion LPS·C⁺·M·C⁻·LPS, {n:,} float32 transforms",
                "head-motion": f"fMRIPrep's inv(vox2ras) @ M @ vox2ras, {n:,} oblique grids and rigid motions",
            }[k],
            ("chains", k, n),
        )
        for k in ("itk-to-ras", "head-motion")
    ]
    out += [
        Case(
            f"inverses/{k}",
            "inverses",
            f"np.linalg.inv, {n:,} {k} 4×4 matrices",
            ("inverses", k, n),
        )
        for k in INVERSES
    ]
    out += [
        Case(
            f"points/affine-{p}",
            "points",
            f"Affine.map vs nitransforms, {small:,} affines, {p} point(s)",
            ("points", p, small),
        )
        for p in (1, 2, 3, 5, 64)
    ]
    out.append(
        Case("points/field", "points", "DenseField.map vs nitransforms", ("field", 0, small))
    )
    out += [
        Case(
            f"orientation/{k}",
            "orientation",
            {
                "oblique": f"ensure_positive_cosines vs sdcflows, {small:,} grids: 48 axis orders and flips, rotated up to 40°",
                "45-degrees": f"ensure_positive_cosines vs sdcflows, {min(small, 4800):,} grids rotated by exactly 45° (tied cosines)",
            }[k],
            ("orientation", k, small if k == "oblique" else min(small, 4800)),
        )
        for k in ("oblique", "45-degrees")
    ]
    out.append(
        Case(
            "real/resample-series",
            "real",
            "every transform file, source grid and head-motion series of the resample-series suite",
            ("real", "", 0),
        )
    )
    return out


def run_case(case: Case, checks: CheckList) -> None:
    core = openblas_core()
    if core not in KERNELS:
        raise Outcome(
            SKIPPED,
            f"numpy's OpenBLAS runs the {core} kernels; larmorx reproduces {', '.join(KERNELS)}",
        )
    what, kind, n = case.payload
    if what == "products":
        _run_products(kind, n, checks)
    elif what == "chains":
        _run_chains(kind, n, checks)
    elif what == "inverses":
        _run_inverses(kind, n, checks)
    elif what == "points":
        _run_points(kind, n, checks)
    elif what == "field":
        _run_field(n, checks)
    elif what == "orientation":
        _run_orientation(kind, n, checks)
    else:
        _run_real(checks)


def _highlights(results: list[CaseResult]) -> list[str]:
    from larmorx_validation.report import table

    rows = []
    same = total = 0
    for r in results:
        for c in r.checks:
            if c.metric == "bit-identical" and isinstance(c.value, str):
                k, n = (int(v) for v in c.value.split(" of "))
                same, total = same + k, total + n
                rows.append((f"`{r.case}`", c.name, f"{k:,} of {n:,}"))
    return [
        f"**{same:,} of {total:,} compared results are bit-identical** to numpy's on the "
        f"{openblas_core()} kernels (OpenBLAS core reported by threadpoolctl).",
        "",
        table(("Case", "Comparison", "Bit-identical"), rows),
    ]


def suite() -> Suite:
    import numpy

    return Suite(
        name="numpy-linalg",
        title="4×4 matrix products and inverses (numpy with OpenBLAS's Haswell kernels)",
        tool=(
            "`larmorx_transform::openblas` (`matmul4`, `inv4`), the point products of "
            "`larmorx_transform::nitransforms`, `nitransforms::closest_orthogonal`, through "
            "`lx.transforms`"
        ),
        reference=(
            f"numpy {numpy.__version__} with its bundled OpenBLAS, in-process, on the "
            f"{openblas_core()} kernels; nitransforms, sdcflows and fMRIPrep for the mapped "
            "points, orientations and loaded transforms"
        ),
        thresholds=[
            (
                "Every matrix, point and affine",
                "bit-identical; with NaN or infinite inputs, NaN is compared as NaN",
            ),
            ("Singular matrices", "numpy raises `LinAlgError` exactly when larmorx does"),
            (
                "Exactly 45° grids",
                "axis decisions may differ (tied cosines; documented divergence)",
            ),
        ],
        cases=cases,
        run_case=run_case,
        packages=("numpy", "threadpoolctl", "nitransforms", "nibabel", "sdcflows", "fmriprep"),
        highlights=_highlights,
        notes=(
            "Inputs are seeded and built with elementwise numpy and larmorx's own products, so "
            "they do not depend on the BLAS kernel. Other kernels: `python -m "
            "larmorx_validation.parity.numpy_linalg kernels` (docs/findings/numpy-blas.md)."
        ),
    )


# ------------------------------------------------------------------------------------------------
# numpy's other kernels: one process per OPENBLAS_CORETYPE


def _worker(args: argparse.Namespace) -> int:
    """Under the kernel set by OPENBLAS_CORETYPE: numpy's results per family, saved, and how
    many equal larmorx's."""
    from larmorx import _core

    out = Path(args.dir)
    out.mkdir(parents=True, exist_ok=True)
    report: dict = {"core": openblas_core(), "families": {}}
    for kind in PRODUCTS:
        a, b = _product_inputs(kind, np.random.default_rng(_SEEDS[kind]), args.n)
        with np.errstate(all="ignore"):
            ref = a @ b
        got = _core.linalg_matmul4(np.ascontiguousarray(a), np.ascontiguousarray(b))
        np.save(out / f"products-{kind}.npy", ref)
        report["families"][f"products/{kind}"] = int(_same(ref, got, kind != "nonfinite").sum())
    for kind in INVERSES:
        a = _inverse_inputs(kind, np.random.default_rng(_SEEDS[f"inv-{kind}"]), args.n)
        ref, ref_sing = _numpy_inv(a)
        got, got_sing = _larmorx_inv(a)
        np.save(out / f"inverses-{kind}.npy", ref)
        ok = _same(ref, got, kind != "nonfinite") & ~ref_sing & ~got_sing | (ref_sing & got_sing)
        report["families"][f"inverses/{kind}"] = int(ok.sum())
    (out / "report.json").write_text(json.dumps(report), encoding="utf-8")
    return 0


def _kernels(args: argparse.Namespace) -> int:
    """Run the families under each kernel and tabulate the agreement."""
    results: dict[str, dict] = {}
    with tempfile.TemporaryDirectory() as tmp:
        for kernel in args.kernels:
            d = Path(tmp) / kernel
            env = {**os.environ, "OPENBLAS_CORETYPE": kernel}
            cmd = [sys.executable, "-m", "larmorx_validation.parity.numpy_linalg", "worker"]
            proc = subprocess.run(
                [*cmd, "--n", str(args.n), "--dir", str(d)], env=env, capture_output=True, text=True
            )
            if proc.returncode != 0:
                results[kernel] = {
                    "error": proc.stderr.strip().splitlines()[-1:] or [str(proc.returncode)]
                }
                continue
            results[kernel] = json.loads((d / "report.json").read_text(encoding="utf-8"))
        base = args.kernels[0]
        for kernel, rep in results.items():
            if "error" in rep:
                continue
            rep["same_as_" + base] = {}
            for fam in rep["families"]:
                f = fam.replace("/", "-") + ".npy"
                x, y = np.load(Path(tmp) / kernel / f), np.load(Path(tmp) / base / f)
                rep["same_as_" + base][fam] = int(_same(x, y, "nonfinite" not in fam).sum())
    families = [f"products/{k}" for k in PRODUCTS] + [f"inverses/{k}" for k in INVERSES]
    lines = [
        f"{args.n:,} matrices per family. Each cell: numpy identical to larmorx / to numpy's {base} kernel.",
        "",
        "| Family | "
        + " | ".join(f"{k} ({results[k].get('core', 'error')})" for k in results)
        + " |",
        "|---|" + "---|" * len(results),
    ]
    for fam in families:
        cells = []
        for rep in results.values():
            if "error" in rep:
                cells.append("–")
                continue
            a = rep["families"][fam] / args.n
            b = rep["same_as_" + base][fam] / args.n
            cells.append(f"{100 * a:.2f} % / {100 * b:.2f} %")
        lines.append(f"| {fam} | " + " | ".join(cells) + " |")
    text = "\n".join(lines)
    print(text)
    if args.out:
        Path(args.out).write_text(
            json.dumps({"n": args.n, "kernels": results}, indent=1) + "\n", encoding="utf-8"
        )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m larmorx_validation.parity.numpy_linalg")
    sub = parser.add_subparsers(dest="command", required=True)
    k = sub.add_parser("kernels", help="compare numpy under several OpenBLAS kernels")
    k.add_argument(
        "--kernels",
        nargs="+",
        default=["Haswell", "Zen", "SandyBridge", "Nehalem", "Core2", "Prescott"],
        help="OPENBLAS_CORETYPE values; the first is the reference (default: this CPU's)",
    )
    k.add_argument("--n", type=int, default=100_000)
    k.add_argument("--out", help="write the counts as JSON")
    k.set_defaults(func=_kernels)
    w = sub.add_parser("worker", help=argparse.SUPPRESS)
    w.add_argument("--n", type=int, required=True)
    w.add_argument("--dir", required=True)
    w.set_defaults(func=_worker)
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
