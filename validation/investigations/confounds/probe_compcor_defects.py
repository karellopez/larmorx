# SPDX-License-Identifier: Apache-2.0
"""What nipype 1.12.0's CompCor functions do on the inputs behind the confounds' part's defect
candidates (relay 2f), measured with this environment's nipype, numpy and SciPy:

1. ``_full_rank`` of singular designs: the sign of the zero singular value numpy's ``svd``
   returns, and whether the design is regularized (``c = smax / smin`` is ``-inf`` for a
   ``-0.0``).
2. ``regress_poly``'s ``non_constant_regressors`` (``X[:, :-1]``): which columns they hold.
3. ``compute_noise_components`` with an empty mask: ``'all'`` (the ``break``) and the basis an
   empty last mask returns with ``failure_mode='NaN'``.
4. A voxel with an infinity (no NaN) in each pre-filter: the components, or the error.
5. An unknown ``filter_type``; ``components_criterion`` 0 and below; ``regress_poly`` with
   ``axis=0``.
6. nipype's ``compute_dvars`` with a masked median of 0.

Run with ``python -P probe_compcor_defects.py`` in each environment (Haswell, one BLAS thread).
"""

from __future__ import annotations

import os
import tempfile
import warnings

import nibabel as nb
import numpy as np
import scipy
from nipype.algorithms.confounds import (
    _full_rank,
    compute_dvars,
    compute_noise_components,
    regress_poly,
)

try:  # larmorx's DVARS for part 6 (PYTHONPATH=<worktree>/python), when importable
    import larmorx as lx
except ImportError:
    lx = None


def outcome(f, *a, **k):
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        try:
            r = f(*a, **k)
        except Exception as e:
            return f"{type(e).__name__}: {e}", [str(x.message) for x in w]
    return r, [str(x.message) for x in w]


def img(mask):
    return nb.Nifti1Image(mask.astype(np.uint8), np.eye(4))


def main() -> None:
    print(f"numpy {np.__version__}")
    rng = np.random.default_rng(0)
    # 1. _full_rank of singular designs.
    print("1. _full_rank of singular designs (min singular value bits, c, regularized)")
    designs = {
        "zero column": np.c_[np.ones(6), np.zeros(6)],
        "repeated column": np.c_[np.arange(6.0), np.arange(6.0)],
        "zero row block": np.c_[np.ones(6), np.r_[np.ones(3), np.zeros(3)], np.ones(6)],
        "rank 1 (3 cols)": np.outer(np.arange(1.0, 9.0), [1.0, 2.0, 3.0]),
        "all zeros": np.zeros((5, 2)),
        "cosine T=10 TR=2": None,
    }
    for name, x in designs.items():
        if x is None:
            continue
        s = np.linalg.svd(x, compute_uv=False)
        (xr, c), msgs = outcome(_full_rank, x)
        print(
            f"   {name}: s={s.tolist()} min bits {np.float64(s.min()).tobytes().hex()} "
            f"c={c} regularized={not np.array_equal(xr, x)} {msgs}"
        )
    # random rank-deficient designs: how often does the zero come out as -0.0?
    neg = pos = tiny = 0
    for i in range(2000):
        m, n = int(rng.integers(3, 40)), int(rng.integers(2, 8))
        x = rng.standard_normal((m, n))
        x[:, -1] = 0.0 if i % 2 else x[:, 0]
        s = np.linalg.svd(x, compute_uv=False)
        if s.min() == 0:
            if np.signbit(s.min()):
                neg += 1
            else:
                pos += 1
        else:
            tiny += 1
    print(f"   2000 random designs with a zero/repeated column: -0.0 {neg}, +0.0 {pos}, tiny {tiny}")
    # every kind of exact zero singular value: zero or negative-zero columns and rows at any
    # place, signs flipped, both orientations, numpy's svd and SciPy's gesvd
    from scipy.linalg import svd as scipy_svd

    found = {"numpy -0.0": 0, "numpy +0.0": 0, "gesvd -0.0": 0, "gesvd +0.0": 0}
    for i in range(6000):
        m, n = int(rng.integers(2, 60)), int(rng.integers(2, 30))
        x = rng.standard_normal((m, n)) * rng.choice([1e-3, 1.0, 1e3])
        kind = i % 6
        j = int(rng.integers(0, n))
        if kind == 0:
            x[:, j] = 0.0
        elif kind == 1:
            x[:, j] = -0.0
        elif kind == 2:
            x[:, j] = -x[:, (j + 1) % n]
        elif kind == 3:
            x[int(rng.integers(0, m)), :] = 0.0
            x[:, j] = 0.0
        elif kind == 4:
            x = np.outer(rng.standard_normal(m), rng.standard_normal(n))
        else:
            x[:, j] = 0.0
            x = x.T.copy()
        for lib, f in (
            ("numpy", lambda a: np.linalg.svd(a, compute_uv=False)),
            ("gesvd", lambda a: scipy_svd(a, compute_uv=False, lapack_driver="gesvd")),
        ):
            s = f(x)
            z = s[s == 0]
            for v in z:
                found[f"{lib} {'-0.0' if np.signbit(v) else '+0.0'}"] += 1
    print(f"   6000 designs with exact zeros (counts of zero singular values): {found}")
    # numpy's s.min() between +0.0 and -0.0 is the CPU's; nipype's c with a -0.0 minimum
    s = np.array([2.0, -0.0])
    print(f"   c of s=[2, -0.0]: {s.max() / s.min()} (regularized: {not s.max() / s.min() < 1e15})")
    # 2. regress_poly's basis.
    data = rng.standard_normal((4, 9)).astype(np.float32)
    for deg in (0, 1, 2, 3):
        _, basis = regress_poly(deg, data)
        cols = basis.shape[1] if basis.size else 0
        first = basis[:, 0].tolist() if cols else []
        print(f"2. regress_poly degree {deg}: basis {basis.shape}, first column {first[:3]}...")
    # 3. empty masks.
    t, shape = 30, (3, 3, 2)
    series = rng.standard_normal(shape + (t,)).astype(np.float32) + 100
    full = np.ones(shape, bool)
    empty = np.zeros(shape, bool)
    for crit in ("all", 0.5, 3):
        for fm in ("error", "NaN"):
            r, msgs = outcome(
                compute_noise_components,
                series,
                [img(empty), img(full)],
                crit,
                "cosine",
                0,
                40,
                2.0,
                fm,
                ["empty", "full"],
            )
            if isinstance(r, str):
                print(f"3. [empty, full] crit {crit} {fm}: {r}")
            else:
                comp, basis, md = r
                print(
                    f"3. [empty, full] crit {crit} {fm}: components {comp.shape} {comp.dtype}, "
                    f"basis {basis.shape}, masks in metadata {sorted(set(md['mask']))}"
                )
            r, msgs = outcome(
                compute_noise_components,
                series,
                [img(full), img(empty)],
                crit,
                "cosine",
                0,
                40,
                2.0,
                fm,
                ["full", "empty"],
            )
            if isinstance(r, str):
                print(f"3. [full, empty] crit {crit} {fm}: {r}")
            else:
                comp, basis, md = r
                print(
                    f"3. [full, empty] crit {crit} {fm}: components {comp.shape}, basis "
                    f"{basis.shape}, masks in metadata {sorted(set(md['mask']))}"
                )
    # 4. an infinite voxel.
    for filt, deg in (("cosine", 0), ("polynomial", 1), (False, 0)):
        s2 = series.copy()
        s2[0, 0, 0, 5] = np.inf
        for fm in ("error", "NaN"):
            r, msgs = outcome(
                compute_noise_components, s2, [img(full)], 3, filt, deg, 40, 2.0, fm, ["full"]
            )
            if isinstance(r, str):
                print(f"4. inf voxel {filt} {fm}: {r}")
            else:
                comp = r[0]
                print(
                    f"4. inf voxel {filt} {fm}: components {comp.shape}, NaN "
                    f"{int(np.isnan(comp).sum())} of {comp.size}"
                )
    # 5. unknown filter, criterion 0 and below, axis=0.
    r, _ = outcome(compute_noise_components, series, [img(full)], 3, "foo", 0, 40, 2.0, "error")
    print(f"5. filter 'foo': {r if isinstance(r, str) else (r[0].shape, r[0].dtype, r[2]['singular_value'][:2])}")
    for crit in (0, -2, 0.0):
        r, _ = outcome(compute_noise_components, series, [img(full)], crit, "cosine", 0, 40, 2.0, "NaN")
        print(f"5. criterion {crit!r}: {r if isinstance(r, str) else (r[0].shape, sum(r[2]['retained']))}")
    d2 = rng.standard_normal((9, 4)).astype(np.float32)
    a = regress_poly(1, d2, axis=0)[0]
    b = regress_poly(1, d2.T)[0].T
    print(f"5. regress_poly axis=0 equals the transpose's: {np.array_equal(a, b)}")
    # 6. DVARS with a masked median of 0.
    with tempfile.TemporaryDirectory() as tmp:
        for kind in ("three quarters zero", "symmetric about 0"):
            d = rng.standard_normal((4, 4, 4, 20)).astype(np.float32)
            if kind == "three quarters zero":
                d[:3] = 0.0
                d[3:] += 50.0
            else:
                d[2:] = -d[:2]
            m = np.ones((4, 4, 4), np.uint8)
            fi, fm = os.path.join(tmp, "f.nii.gz"), os.path.join(tmp, "m.nii.gz")
            nb.Nifti1Image(d, np.eye(4)).to_filename(fi)
            nb.Nifti1Image(m, np.eye(4)).to_filename(fm)
            print(f"6. median {np.median(d[m.astype(bool)])}")
            for rz in (True, False):
                r, msgs = outcome(compute_dvars, fi, fm, rz)
                text = r if isinstance(r, str) else [int(np.isfinite(x).sum()) for x in r]
                same = ""
                if lx is not None:
                    for mode in ("reproduce", "fix"):
                        g, _ = outcome(
                            lx.confounds.compute_dvars,
                            fi,
                            fm,
                            rz,
                            numpy_version=np.__version__,
                            scipy_version=scipy.__version__,
                            upstream_defects=mode,
                        )
                        g = g if isinstance(g, str) else [int(np.isfinite(x).sum()) for x in g.as_tuple()]
                        same += f", larmorx {mode} {'the same' if g == text else g}"
                print(f"6. DVARS median 0 ({kind}, remove_zerovariance={rz}): {text}{same}")


if __name__ == "__main__":
    main()
