# SPDX-License-Identifier: Apache-2.0
"""SciPy's ``linalg.inv`` of 4 x 4 matrices against ``larmorx.transforms._linalg.scipy_inv``.

nilearn's ``resample_img`` inverts the image's affine with ``scipy.linalg.inv`` (niworkflows'
``GenerateSamplingReference``, sdcflows' SyN preprocessing). larmorx follows SciPy 1.18.1 (the
newest: structure detection, then ``trtri``, ``1 / a`` or ``getrf``/``gecon``/``getri``) and
1.15 (``getrf``/``getri``) with their wheels' OpenBLAS on the Haswell kernels
(``larmorx_transform::scipy_linalg``). Run in each kept environment (the shared ``.venv``:
SciPy 1.15.3 with OpenBLAS 0.3.28; ``oracles/python-newest``: SciPy 1.18.1 with OpenBLAS
0.3.31.dev), with ``PYTHONPATH=<repo>/python``:

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_scipy_inv.py [--n 20000]
        [--images <file.nii.gz> ...]

Families (seeded, ``--n`` each): random, oblique affines, plumb affines (upper triangular for
1.18), axis-permuted affines, diagonal, lower and upper triangular, rows and columns scaled by
2^+-300, condition 10^3 to 10^12, entries in {+-1, +-2} (tied pivots), near-permutations,
integers; and one by one: singular matrices, a symmetric one (1.18: not ported), an
ill-conditioned diagonal (the warning's text), an overflowing norm (SCI-18 both modes),
non-finite and non-square input (the messages), ``--images``' affines (nibabel's). Every value
is compared bit for bit (signed zeros included); a case larmorx refuses
(``NotImplementedError``) is counted apart. Prints one line per family; ``ALL OK`` or
``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import sys
import warnings

import numpy as np
import scipy
from scipy import linalg

from larmorx.transforms._linalg import LinAlgWarning, scipy_inv

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
        except Exception as e:  # noqa: BLE001 - compared by type and text
            return type(e).__name__, str(e), [str(x.message) for x in w]
    msgs = [f"{x.category.__name__}: {x.message}" for x in w]
    return kind, value, msgs


def ours(m, mode="fix"):
    return scipy_inv(m, scipy_version=VERSION, upstream_defects=mode)


def refused_values(m, scipy_value) -> bool:
    """For a matrix larmorx refuses because 1.18's warning needs LAPACK's condition estimate:
    whether its values (``_core.linalg_scipy_inv4``) are SciPy's."""
    from larmorx import _core

    a = np.ascontiguousarray(np.asarray(m, dtype=np.float64))
    _, status, inverse, _, _ = _core.linalg_scipy_inv4(a, VERSION)
    return status == "estimate_needed" and same(inverse, scipy_value)


def compare(m) -> str:
    """'same', 'refused', 'refused, values same' or a description of the difference."""
    k1, v1, w1 = outcome(linalg.inv, m)
    k2, v2, w2 = outcome(ours, m)
    if k2 == "refused":
        if k1 == "ok" and refused_values(m, v1):
            return "refused, values same"
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
        "lower triangular": lambda: np.tril(rng.normal(size=(4, 4))) + np.diag(rng.choice([-3, 3], 4)),
        "upper triangular": lambda: np.triu(rng.normal(size=(4, 4))) + np.diag(rng.choice([-3, 3], 4)),
        "scaled 2^+-300": lambda: (
            rng.normal(size=(4, 4))
            * (2.0 ** rng.integers(-300, 301, 4))[:, None]
            * (2.0 ** rng.integers(-300, 301, 4))[None, :]
        ),
        "condition 1e3..1e12": lambda: conditioned(rng),
        "entries +-1 +-2": lambda: rng.choice([-2.0, -1.0, 1.0, 2.0], (4, 4)),
        "near-permutation": lambda: np.eye(4)[rng.permutation(4)] + rng.normal(0, 1e-9, (4, 4)),
        "integers": lambda: rng.integers(-5, 6, (4, 4)),
    }
    for name, make in fam.items():
        yield name, (make() for _ in range(n))


def conditioned(rng):
    u, _ = np.linalg.qr(rng.normal(size=(4, 4)))
    v, _ = np.linalg.qr(rng.normal(size=(4, 4)))
    s = np.logspace(0, -rng.uniform(3, 12), 4)
    return (u * s) @ v.T


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
    out.append(("ill-conditioned diagonal", np.diag([1.0, 2.0, 1e-17, 1.0])))
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
        tot = good = refused = rvals = 0
        first = None
        for m in gen:
            tot += 1
            r = compare(m)
            if r == "same":
                good += 1
            elif r.startswith("refused"):
                refused += 1
                rvals += r == "refused, values same"
            elif first is None:
                first = r
        bad = tot - good - refused
        fails += bad
        extra = f" (their values: {rvals} identical)" if refused else ""
        print(
            f"{name}: {tot} cases, {good} identical, {refused} refused{extra}, {bad} differ"
            + (f" (first: {first})" if first else "")
        )
    for name, m in singles(rng):
        r = compare(m)
        if not (r == "same" or r.startswith("refused")):
            fails += 1
        print(f"{name}: {r}")
    # 1.18's symmetric path is not ported.
    sym = np.array([[0, 1, 0, 0], [1, 0, 0, 0], [0, 0, -1, 0], [0, 0, 0, 1.0]])
    print(f"symmetric: {compare(sym)}")
    # SCI-18: an overflowing norm (1.18 returns the LU factors; 1.15 the inverse).
    big = np.array(
        [[1.7e308, 1.7e308, 0, 1], [0.5, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1.0]]
    )
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        ref = linalg.inv(big)
        rep = scipy_inv(big, scipy_version=VERSION, upstream_defects="reproduce")
        fix = scipy_inv(big, scipy_version=VERSION, upstream_defects="fix")
    ok = same(ref, rep)
    fails += not ok
    print(f"norm overflow (SCI-18): reproduce = SciPy {ok}; fix = the inverse "
          f"{np.allclose(fix @ big, np.eye(4), atol=1e-6) if np.isfinite(fix).all() else 'non-finite'}")
    if args.images:
        import nibabel as nb

        tot = good = 0
        for path in args.images:
            m = nb.load(path).affine
            tot += 1
            r = compare(m)
            good += r == "same"
            if r != "same":
                print(f"  {path}: {r}")
        fails += tot - good
        print(f"image affines: {tot} cases, {good} identical")
    print("ALL OK" if fails == 0 else f"FAIL {fails}")
    return 0 if fails == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
