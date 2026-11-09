# SPDX-License-Identifier: Apache-2.0
"""SciPy's ``linalg.inv`` of 4 x 4 matrices against ``larmorx.transforms._linalg.scipy_inv``.

nilearn's ``resample_img`` inverts the image's affine with ``scipy.linalg.inv`` (niworkflows'
``GenerateSamplingReference``, sdcflows' SyN preprocessing). larmorx follows SciPy 1.18.1 (the
newest: structure detection, then ``1 / a``, ``trtri``/``trcon``, ``getrf``/``gecon``/``getri``,
or for a symmetric matrix Cholesky's ``potrf``/``pocon``/``potri`` and Bunch-Kaufman's
``sytrf``/``sycon``/``sytri``) and 1.15 (``getrf``/``getri``) with their wheels' OpenBLAS on the
Haswell kernels (``larmorx_transform::scipy_linalg``, ``scipy_lapack``). Run in each kept
environment (the shared ``.venv``: SciPy 1.15.3 with OpenBLAS 0.3.28; ``oracles/python-newest``:
SciPy 1.18.1 with OpenBLAS 0.3.31.dev), with ``PYTHONPATH=<repo>/python``:

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_scipy_inv.py [--n 20000]
        [--images <file.nii.gz> ...]

Families (seeded, ``--n`` each): random, oblique affines, plumb affines (upper triangular for
1.18), axis-permuted affines, diagonal, lower and upper triangular, rows and columns scaled by
2^+-300 and 2^+-500, condition 10^3 to 10^12 and 10^8 to 10^20 (the ill-conditioned warning),
entries in {+-1, +-2} (tied pivots), near-permutations, integers; symmetric: definite and
indefinite, each well and ill conditioned (10^8 to 10^20), small integers, a zero diagonal
(Bunch-Kaufman's 2 x 2 pivots), scaled by 2^+-1000, affines with a zero origin (axis swaps);
triangular with a tiny diagonal (10^-18 to 1), scaled by 2^+-500, and with column sums that
overflow (``dlatrs``' rescaled paths); and one by one: singular matrices (general, diagonal,
triangular, symmetric), an ill-conditioned diagonal (the warning's text), an overflowing norm
(SCI-18 both modes), non-finite and non-square input (the messages), ``--images``' affines
(nibabel's). Every value is compared bit for bit (signed zeros included), the warnings' texts
(which print 1.18's ``rcond`` in full) and the errors' messages; with SciPy 1.18 every
non-diagonal ``rcond`` is also compared with SciPy's own LAPACK calls on the same factors
(``scipy.linalg.lapack``: ``dgecon``, ``dtrcon``, ``dpocon``, ``dsycon``), warning or not. A
case larmorx refuses (``NotImplementedError``) is counted apart. Prints one line per family;
``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import sys
import warnings

import numpy as np
import scipy
from scipy import linalg

from larmorx.transforms._linalg import scipy_inv

VERSION = scipy.__version__


def same(a, b) -> bool:
    a, b = np.asarray(a), np.asarray(b)
    return a.dtype == b.dtype and a.shape == b.shape and a.tobytes() == b.tobytes()


def outcome(fn, m):
    """(kind, value, messages): the result or the exception, and the warnings' texts."""
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        try:
            value = fn(m)
            kind = "ok"
        except NotImplementedError as e:
            return "refused", str(e), []
        except Exception as e:
            return type(e).__name__, str(e), [str(x.message) for x in w]
    msgs = [f"{x.category.__name__}: {x.message}" for x in w]
    return kind, value, msgs


def ours(m, mode="fix"):
    return scipy_inv(m, scipy_version=VERSION, upstream_defects=mode)


def _norm_rows(a) -> float:
    """SciPy 1.18's ``norm1_`` (the row sums of the column-major copy, in column order)."""
    best = 0.0
    for r in range(4):
        t = abs(a[r, 0])
        for c in range(1, 4):
            t += abs(a[r, c])
        if t > best:
            best = t
    return best


def _norm_symmetric(a) -> float:
    """SciPy 1.18's ``norm1_sym_herm('U')`` (the upper triangle, in its order)."""
    rw = [0.0] * 4
    for i in range(4):
        rw[i] += abs(a[i, i])
        for j in range(i):
            t = abs(a[j, i])
            rw[j] += t
            rw[i] += t
    best = 0.0
    for w in rw:
        if w > best:
            best = w
    return best


def scipy_rcond(m, structure):
    """The ``rcond`` SciPy 1.18's ``inv`` computes, from SciPy's own LAPACK wrappers (the same
    library) on the same factors; None where there is none (singular, overflowing norm)."""
    from scipy.linalg import lapack

    a = np.asarray(m, dtype=np.float64)
    if structure == "general":
        lu, _, info = lapack.dgetrf(a)
        if info != 0:
            return None
        rc, info = lapack.dgecon(lu, _norm_rows(a), norm="1")
        return rc if info >= 0 else None
    if structure.endswith("triangular"):
        low = structure.startswith("lower")
        inv, info = lapack.dtrtri(a, lower=low)
        if info != 0:
            return None
        return lapack.dtrcon(inv, norm="1", uplo="L" if low else "U", diag="N")[0]
    if structure == "symmetric":
        anorm = _norm_symmetric(a)
        c, info = lapack.dpotrf(a, lower=0, clean=0)
        if info == 0:
            return lapack.dpocon(c, anorm, uplo="U")[0]
        ldu, ipiv, info = lapack.dsytrf(a, lower=0)
        if info != 0:
            return None
        return lapack.dsycon(ldu, ipiv, anorm, lower=0)[0]
    return None


def rcond_same(m) -> bool | None:
    """With SciPy 1.18: whether larmorx's ``rcond`` is SciPy's LAPACK estimate (None: not
    applicable)."""
    from larmorx import _core

    if not VERSION.startswith("1.18"):
        return None
    a = np.ascontiguousarray(np.asarray(m, dtype=np.float64))
    structure, status, _, _, rcond = _core.linalg_scipy_inv4(a, VERSION)
    if structure == "diagonal" or status in ("singular", "norm_overflow"):
        return None
    ref = scipy_rcond(a, structure)
    if ref is None:
        return None
    return np.float64(ref).tobytes() == np.float64(rcond).tobytes()


WARNED: list[int] = [0]


def compare(m) -> str:
    """'same', 'refused' or a description of the difference (SciPy's warnings counted in
    ``WARNED``)."""
    k1, v1, w1 = outcome(linalg.inv, m)
    WARNED[0] += bool(w1)
    k2, v2, w2 = outcome(ours, m)
    if k2 == "refused":
        return "refused"
    if k1 != k2:
        return f"scipy {k1} {v1!r} / larmorx {k2} {v2!r}"
    if k1 == "ok":
        if not same(v1, v2):
            d = np.max(np.abs(np.asarray(v1) - np.asarray(v2)))
            return f"values differ (max {d:.3g})"
        w1 = [x.replace("LinAlgWarning", "W") for x in w1]
        w2 = [x.replace("LinAlgWarning", "W") for x in w2]
        if w1 != w2:
            return f"warnings {w1} / {w2}"
        return "same"
    return "same" if v1 == v2 else f"messages {v1!r} / {v2!r}"


def rotation(rng):
    q = rng.normal(size=4)
    q /= np.linalg.norm(q)
    w, x, y, z = q
    return np.array(
        [
            [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
        ]
    )


def affine(rng, rs):
    a = np.eye(4)
    a[:3, :3] = rs
    a[:3, 3] = rng.uniform(-150, 150, 3)
    return a


def families(rng, n):
    zooms = lambda: rng.choice([0.8, 1.0, 1.2, 2.0, 2.5, 3.0, 3.5], 3) * rng.uniform(0.9, 1.1, 3)  # noqa: E731
    fam = {
        "random": lambda: rng.normal(size=(4, 4)),
        "affine oblique": lambda: affine(rng, rotation(rng) * zooms()),
        "affine oblique float32": lambda: affine(
            rng, (rotation(rng) * zooms()).astype(np.float32).astype(np.float64)
        ),
        "affine plumb": lambda: affine(rng, np.diag(zooms() * rng.choice([-1, 1], 3))),
        "affine permuted": lambda: affine(
            rng, (np.eye(3)[rng.permutation(3)] * rng.choice([-1, 1], 3)) * zooms()
        ),
        "diagonal": lambda: np.diag(rng.uniform(0.1, 5, 4) * rng.choice([-1, 1], 4)),
        "lower triangular": lambda: (
            np.tril(rng.normal(size=(4, 4))) + np.diag(rng.choice([-3, 3], 4))
        ),
        "upper triangular": lambda: (
            np.triu(rng.normal(size=(4, 4))) + np.diag(rng.choice([-3, 3], 4))
        ),
        "scaled 2^+-300": lambda: (
            rng.normal(size=(4, 4))
            * (2.0 ** rng.integers(-300, 301, 4))[:, None]
            * (2.0 ** rng.integers(-300, 301, 4))[None, :]
        ),
        "condition 1e3..1e12": lambda: conditioned(rng),
        "entries +-1 +-2": lambda: rng.choice([-2.0, -1.0, 1.0, 2.0], (4, 4)),
        "near-permutation": lambda: np.eye(4)[rng.permutation(4)] + rng.normal(0, 1e-9, (4, 4)),
        "integers": lambda: rng.integers(-5, 6, (4, 4)),
        "condition 1e8..1e20": lambda: conditioned(rng, 8, 20),
        "scaled 2^+-500": lambda: (
            rng.normal(size=(4, 4))
            * (2.0 ** rng.integers(-500, 501, 4))[:, None]
            * (2.0 ** rng.integers(-500, 501, 4))[None, :]
        ),
        "symmetric definite": lambda: (lambda b: b @ b.T + 0.1 * np.eye(4))(
            rng.normal(size=(4, 4))
        ),
        "symmetric definite 1e8..1e20": lambda: symmetric_conditioned(rng, False),
        "symmetric indefinite": lambda: sym(rng.normal(size=(4, 4))),
        "symmetric indefinite 1e8..1e20": lambda: symmetric_conditioned(rng, True),
        "symmetric integers": lambda: sym(rng.integers(-3, 4, (4, 4)).astype(np.float64)),
        "symmetric zero diagonal": lambda: (lambda m: m - np.diag(np.diag(m)))(
            sym(rng.normal(size=(4, 4)))
        ),
        "symmetric 2^+-1000": lambda: sym(
            rng.normal(size=(4, 4)) * 2.0 ** rng.integers(-1000, 1000)
        ),
        "affine symmetric (zero origin)": lambda: symmetric_affine(rng),
        "upper tiny diagonal": lambda: tiny_diagonal(np.triu(rng.normal(size=(4, 4))), rng),
        "lower tiny diagonal": lambda: tiny_diagonal(np.tril(rng.normal(size=(4, 4))), rng),
        "upper 2^+-500": lambda: (
            np.triu(rng.normal(size=(4, 4)))
            * (2.0 ** rng.integers(-500, 501, 4))[:, None]
            * (2.0 ** rng.integers(-500, 501, 4))[None, :]
        ),
        "lower column sums overflow": lambda: (
            np.tril(rng.normal(size=(4, 4)) * 2.0**1020, -1) + np.diag(rng.uniform(0.5, 2, 4))
        ),
    }
    for name, make in fam.items():
        yield name, (make() for _ in range(n))


def conditioned(rng, lo=3, hi=12):
    u, _ = np.linalg.qr(rng.normal(size=(4, 4)))
    v, _ = np.linalg.qr(rng.normal(size=(4, 4)))
    s = np.logspace(0, -rng.uniform(lo, hi), 4)
    return (u * s) @ v.T


def sym(m):
    """The upper triangle mirrored (exactly symmetric)."""
    return np.triu(m) + np.triu(m, 1).T


def symmetric_conditioned(rng, indefinite):
    q, _ = np.linalg.qr(rng.normal(size=(4, 4)))
    s = np.logspace(0, -rng.uniform(8, 20), 4)
    if indefinite:
        s = s * rng.choice([-1, 1], 4)
    return sym((q * s) @ q.T)


def symmetric_affine(rng):
    """An affine whose 3 x 3 part is a symmetric permutation with zooms and a zero origin."""
    a = np.eye(4)
    a[:3, :3] = sym(np.eye(3)[rng.permutation(3)] * rng.choice([0.8, 1.0, 2.0, 2.5], 3))
    return a


def tiny_diagonal(t, rng):
    """A triangle whose diagonal is replaced by +-10^-18 .. +-1."""
    t = t.copy()
    np.fill_diagonal(t, 10.0 ** rng.uniform(-18, 0, 4) * rng.choice([-1, 1], 4))
    return t


def singles(rng):
    out = []
    s = rng.normal(size=(4, 4))
    s[2] = s[0]
    out.append(("singular rows", s))
    out.append(("singular zeros", np.zeros((4, 4))))
    d = np.diag([1.0, 2.0, 0.0, 1.0])
    out.append(("singular diagonal", d))
    t = np.triu(rng.normal(size=(4, 4)))
    t[1, 1] = 0.0
    out.append(("singular triangular", t))
    out.append(
        ("singular symmetric", np.array([[0, 1, 0, 0], [1, 0, 0, 0], [0, 0, 0, 0.0], [0, 0, 0, 1]]))
    )
    out.append(("ill-conditioned diagonal", np.diag([1.0, 2.0, 1e-17, 1.0])))
    out.append(
        (
            "ill-conditioned general",
            np.array([[1, 1, 0, 0], [1, 1 + 1e-17, 0, 0], [0, 0, 1, 0], [0, 0, 1, 1.0]]),
        )
    )
    neg = np.diag([2.0, -4.0, 0.5, 1.0])
    neg[0, 1] = -0.0
    out.append(("diagonal with -0", neg))
    lz = np.tril(rng.normal(size=(4, 4))) + 3 * np.eye(4)
    lz[0, 2] = -0.0
    out.append(("lower with -0 above", lz))
    out.append(("identity", np.eye(4)))
    out.append(("non-finite nan", np.diag([1.0, np.nan, 1.0, 1.0])))
    out.append(("non-finite inf", np.diag([1.0, np.inf, 1.0, 1.0])))
    out.append(("non-square", np.ones((3, 4))))
    out.append(("1-D", np.ones(4)))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=20000)
    ap.add_argument("--seed", type=int, default=20261109)
    ap.add_argument("--images", nargs="*", default=[])
    args = ap.parse_args()
    rng = np.random.default_rng(args.seed)
    print(f"SciPy {VERSION}, numpy {np.__version__}")
    fails = 0
    for name, gen in families(rng, args.n):
        tot = good = refused = rvals = rtot = rgood = 0
        WARNED[0] = 0
        first = None
        for m in gen:
            tot += 1
            r = compare(m)
            rs = rcond_same(m)
            if rs is not None:
                rtot += 1
                rgood += rs
                if not rs and first is None:
                    first = "rcond differs"
            if r == "same":
                good += 1
            elif r.startswith("refused"):
                refused += 1
                rvals += r == "refused, values same"
            elif first is None:
                first = r
        bad = tot - good - refused
        fails += bad + (rtot - rgood)
        extra = f" (their values: {rvals} identical)" if refused else ""
        rc = f", rcond {rgood}/{rtot} identical" if rtot else ""
        rc += f", {WARNED[0]} warned" if WARNED[0] else ""
        print(
            f"{name}: {tot} cases, {good} identical{rc}, {refused} refused{extra}, {bad} differ"
            + (f" (first: {first})" if first else "")
        )
    for name, m in singles(rng):
        r = compare(m)
        if not (r == "same" or r.startswith("refused")):
            fails += 1
        print(f"{name}: {r}")
    # An axis swap with a zero origin: symmetric for 1.18 (Bunch-Kaufman's 2 x 2 pivot).
    swap = np.array([[0, 1, 0, 0], [1, 0, 0, 0], [0, 0, -1, 0], [0, 0, 0, 1.0]])
    r = compare(swap)
    fails += r != "same"
    print(f"symmetric axis swap: {r}")
    # SCI-18: an overflowing norm (1.18 returns the LU factors; 1.15 the inverse).
    big = np.array([[1.7e308, 1.7e308, 0, 1], [0.5, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1.0]])
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        ref = linalg.inv(big)
        rep = scipy_inv(big, scipy_version=VERSION, upstream_defects="reproduce")
        fix = scipy_inv(big, scipy_version=VERSION, upstream_defects="fix")
    ok = same(ref, rep)
    fails += not ok
    print(
        f"norm overflow (SCI-18): reproduce = SciPy {ok}; fix = the inverse "
        f"{np.allclose(fix @ big, np.eye(4), atol=1e-6) if np.isfinite(fix).all() else 'non-finite'}"
    )
    if args.images:
        import nibabel as nb

        tot = good = 0
        for path in args.images:
            m = nb.load(path).affine
            tot += 1
            r = compare(m)
            if rcond_same(m) is False:
                r = "rcond differs"
            good += r == "same"
            if r != "same":
                print(f"  {path}: {r}")
        fails += tot - good
        print(f"image affines: {tot} cases, {good} identical")
    print("ALL OK" if fails == 0 else f"FAIL {fails}")
    return 0 if fails == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
