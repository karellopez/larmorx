# SPDX-License-Identifier: Apache-2.0
# ruff: noqa: B023 (every lambda runs at once, inside the loop that defines it)
"""``lx.ndimage``'s filters, morphology and labelling against ``scipy.ndimage``, bit for bit.

Run in each kept environment (the shared ``.venv``: SciPy 1.15.3; ``oracles/python-newest``:
SciPy 1.18.1) with ``PYTHONPATH=<repo>/python``:

    OPENBLAS_CORETYPE=Haswell python -P check_ndimage.py [--quick] [--threads N]

Synthetic cases (seeded, plain arithmetic): every element type, 1- to 4-D arrays, C, Fortran
and non-contiguous layouts, axes of length 1 to 2, every boundary mode, ``cval``, origins
(the valid range of each axis), footprints (balls, random ones, a full box: the separable
path), non-flat structures, the ``axes`` argument, NaN, infinities, signed zeros and ties in
float data; the minimum, maximum, rank, median and percentile filters, the 1-D minimum and
maximum, grey erosion, dilation, opening and closing, binary erosion, dilation, opening,
closing (iterations 1 to 3 and until nothing changes, both strategies, masks of every kind,
both border values, origins), propagation, hole filling, hit-or-miss,
``generate_binary_structure``, ``iterate_structure``, ``label`` (structures, element types,
Fortran order) and ``sum_labels`` (sequence, scalar and no index, label types); and the
exceptions SciPy raises (type and message). Each result is compared with SciPy's: the dtype,
the shape and the bytes.

Prints one line per function (cases, identical) and the first differences; ``ALL OK`` at the
end, or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import sys
import warnings
from collections import defaultdict
from pathlib import Path

import numpy as np
import scipy
from scipy import ndimage as sp

import larmorx.ndimage as lx

sys.path.insert(0, str(Path(__file__).resolve().parent))
import steplines  # the originals' step: SciPy's outputs by digest

MODES = [
    "reflect",
    "grid-mirror",
    "constant",
    "grid-constant",
    "nearest",
    "mirror",
    "wrap",
    "grid-wrap",
]
TYPES = [
    np.float32,
    np.float64,
    np.uint8,
    np.int8,
    np.uint16,
    np.int16,
    np.int32,
    np.uint32,
    np.int64,
    np.uint64,
    np.bool_,
]


class Tally:
    def __init__(self) -> None:
        self.total = defaultdict(int)
        self.same = defaultdict(int)
        self.first: dict[str, str] = {}

    def add(self, name: str, ok: bool, what: str) -> None:
        steplines.note(name)
        self.total[name] += 1
        if ok:
            self.same[name] += 1
        elif name not in self.first:
            self.first[name] = what

    def report(self) -> int:
        bad = 0
        for name in sorted(self.total):
            t, s = self.total[name], self.same[name]
            bad += t - s
            print(f"{name}: {t} cases, identical {s}")
            if name in self.first:
                print(f"   first difference: {self.first[name]}")
        return bad


def ball(r: int) -> np.ndarray:
    """scikit-image's ``ball(r)`` (strict radius), as plain integer arithmetic."""
    g = np.arange(-r, r + 1)
    z, y, x = np.meshgrid(g, g, g, indexing="ij")
    return (x * x + y * y + z * z <= r * r).astype(np.uint8)


def same(a, b) -> bool:
    if isinstance(a, tuple) or isinstance(b, tuple):
        return (
            isinstance(a, tuple)
            and isinstance(b, tuple)
            and len(a) == len(b)
            and all(same(x, y) for x, y in zip(a, b, strict=True))
        )
    if np.isscalar(a) or np.isscalar(b) or isinstance(a, int | float):
        a2, b2 = np.asarray(a), np.asarray(b)
        return a2.dtype == b2.dtype and a2.tobytes() == b2.tobytes()
    a = np.asarray(a)
    b = np.asarray(b)
    return (
        a.dtype == b.dtype
        and a.shape == b.shape
        and np.ascontiguousarray(a).tobytes() == np.ascontiguousarray(b).tobytes()
    )


VERBOSE = False


def compare(t: Tally, name: str, desc: str, f_sp, f_lx) -> None:
    if VERBOSE:
        print(name, desc, flush=True)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            want = steplines.oracle(f_sp())
            werr = None
        except Exception as e:
            want, werr = None, steplines.oracle((type(e).__name__, str(e)))
        try:
            got = f_lx()
            gerr = None
        except Exception as e:
            got, gerr = None, (type(e).__name__, str(e))
    if werr or gerr:
        t.add(name, werr == gerr, f"{desc}: scipy {werr}, larmorx {gerr}")
        return
    ok = same(want, got)
    detail = ""
    if not ok:
        try:
            w, g = np.asarray(want), np.asarray(got)
            if w.shape == g.shape and w.dtype == g.dtype:
                n = int(
                    (
                        np.ascontiguousarray(w).view(np.uint8)
                        != np.ascontiguousarray(g).view(np.uint8)
                    ).sum()
                )
                detail = f" ({n} bytes differ)"
            else:
                detail = f" (scipy {w.dtype}{w.shape}, larmorx {g.dtype}{g.shape})"
        except Exception:
            detail = ""
    t.add(name, ok, desc + detail)


def values(rng, shape, dtype, special: bool) -> np.ndarray:
    if dtype == np.bool_:
        return rng.random(shape) < 0.6
    if np.issubdtype(dtype, np.integer):
        info = np.iinfo(dtype)
        lo = max(info.min, -1000)
        hi = min(info.max, 1000)
        a = rng.integers(lo, hi + 1, size=shape).astype(dtype)
        if special and a.size > 3:
            a.flat[0] = info.max
            a.flat[1] = info.min
        return a
    a = (rng.integers(-60, 61, size=shape) * 0.25).astype(dtype)
    if special and a.size > 6:
        flat = a.reshape(-1)
        flat[rng.integers(0, a.size)] = np.nan
        flat[rng.integers(0, a.size)] = -0.0
        flat[rng.integers(0, a.size)] = 0.0
        flat[rng.integers(0, a.size)] = np.inf
        flat[rng.integers(0, a.size)] = -np.inf
    return a


def layouts(a: np.ndarray, rng) -> list[tuple[str, np.ndarray]]:
    out = [("C", np.ascontiguousarray(a)), ("F", np.asfortranarray(a))]
    if a.ndim >= 2 and a.shape[0] >= 2:
        big = np.zeros((a.shape[0] * 2, *a.shape[1:]), a.dtype)
        big[::2] = a
        out.append(("strided", big[::2]))
    return out


def random_footprint(rng, shape) -> np.ndarray:
    fp = rng.random(shape) < 0.55
    fp.flat[rng.integers(0, fp.size)] = True
    return fp


def reads_outside(n: int, win: int, mode: str, origin: int) -> bool:
    """Whether ``_rank_filter_1d.cpp`` (SciPy 1.18.1) indexes its input outside ``[0, n)``."""
    lim = (win - 1) // 2 - origin
    idx: list[int] = []
    if mode == "reflect":
        idx += list(range(win - lim - 1, -1, -1))
    elif mode == "mirror":
        idx += list(range(win - lim, 0, -1))
    elif mode == "wrap":
        offset = 2 if win % 2 == 0 else 0
        idx += list(range(n - lim - offset - 2 * origin, n))
    idx += list(range(0, lim))
    for i in range(lim):
        idx.append({"reflect": n - 1 - i, "mirror": n - 2 - i, "wrap": i}.get(mode, 0))
    return any(i < 0 or i >= n for i in idx)


def filters(t: Tally, rng, quick: bool, nt: int) -> None:
    shapes = [(17,), (9, 7), (6, 5, 4), (5, 1, 6), (2, 3, 4, 3), (1, 5), (3,)]
    for shape in shapes:
        nd_ = len(shape)
        for dtype in TYPES:
            special = rng.random() < 0.7
            base = values(rng, shape, dtype, special)
            for lname, a in layouts(base, rng):
                if quick and lname == "strided":
                    continue
                for mode in MODES if not quick else ["reflect", "constant", "wrap"]:
                    cval = float(rng.choice([0.0, -2.5, 7.0]))
                    fshape = tuple(int(rng.integers(1, 4)) for _ in range(nd_))
                    fp = random_footprint(rng, fshape)
                    org = [int(rng.integers(-(s // 2), (s - 1) // 2 + 1)) for s in fshape]
                    desc = f"{dtype.__name__}{shape} {lname} {mode} cval={cval} fp={fp.astype(int).tolist()} origin={org}"
                    for fn in ("minimum_filter", "maximum_filter"):
                        compare(
                            t,
                            fn,
                            desc,
                            lambda fn=fn: getattr(sp, fn)(
                                a, footprint=fp, mode=mode, cval=cval, origin=org
                            ),
                            lambda fn=fn: getattr(lx, fn)(
                                a, footprint=fp, mode=mode, cval=cval, origin=org, n_threads=nt
                            ),
                        )
                    size = [int(rng.integers(1, 5)) for _ in range(nd_)]
                    compare(
                        t,
                        "minimum_filter(size)",
                        desc + f" size={size}",
                        lambda: sp.minimum_filter(a, size=size, mode=mode, cval=cval),
                        lambda: lx.minimum_filter(a, size=size, mode=mode, cval=cval, n_threads=nt),
                    )
                    compare(
                        t,
                        "maximum_filter(size)",
                        desc + f" size={size}",
                        lambda: sp.maximum_filter(a, size=size, mode=mode, cval=cval),
                        lambda: lx.maximum_filter(a, size=size, mode=mode, cval=cval, n_threads=nt),
                    )
                    ax = int(rng.integers(-nd_, nd_))
                    sz = int(rng.integers(1, 6))
                    o1 = int(rng.integers(-(sz // 2), (sz - 1) // 2 + 1))
                    for fn in ("minimum_filter1d", "maximum_filter1d"):
                        compare(
                            t,
                            fn,
                            desc + f" size={sz} axis={ax} origin={o1}",
                            lambda fn=fn: getattr(sp, fn)(
                                a, sz, axis=ax, mode=mode, cval=cval, origin=o1
                            ),
                            lambda fn=fn: getattr(lx, fn)(
                                a, sz, axis=ax, mode=mode, cval=cval, origin=o1, n_threads=nt
                            ),
                        )
                    n = int(fp.sum())
                    rank = int(rng.integers(-n, n))
                    # SciPy's own results with upstream_defects="reproduce"; a 1-D footprint with
                    # holes (SCI-09) corrected by default: SciPy's n-dimensional filter of the
                    # same footprint along a second axis of length 2
                    compare(
                        t,
                        "rank_filter",
                        desc + f" rank={rank}",
                        lambda: sp.rank_filter(
                            a, rank, footprint=fp, mode=mode, cval=cval, origin=org
                        ),
                        lambda: lx.rank_filter(
                            a,
                            rank,
                            footprint=fp,
                            mode=mode,
                            cval=cval,
                            origin=org,
                            n_threads=nt,
                            upstream_defects="reproduce",
                        ),
                    )
                    compare(
                        t,
                        "median_filter",
                        desc,
                        lambda: sp.median_filter(a, footprint=fp, mode=mode, cval=cval, origin=org),
                        lambda: lx.median_filter(
                            a,
                            footprint=fp,
                            mode=mode,
                            cval=cval,
                            origin=org,
                            n_threads=nt,
                            upstream_defects="reproduce",
                        ),
                    )
                    if nd_ == 1 and not fp.all():
                        compare(
                            t,
                            "rank_filter SCI-09 corrected",
                            desc + f" rank={rank}",
                            lambda: sp.rank_filter(
                                np.stack([a, a]),
                                rank,
                                footprint=fp[None, :],
                                mode=mode,
                                cval=cval,
                                origin=[0, *org],
                            )[0],
                            lambda: lx.rank_filter(
                                a,
                                rank,
                                footprint=fp,
                                mode=mode,
                                cval=cval,
                                origin=org,
                                n_threads=nt,
                            ),
                        )
                    pct = float(rng.choice([0.0, 12.5, 50.0, 75.0, 100.0, -30.0]))
                    compare(
                        t,
                        "percentile_filter",
                        desc + f" pct={pct}",
                        lambda: sp.percentile_filter(a, pct, size=size, mode=mode, cval=cval),
                        lambda: lx.percentile_filter(
                            a, pct, size=size, mode=mode, cval=cval, n_threads=nt
                        ),
                    )
                    st = rng.integers(-3, 4, size=fshape) * 0.5
                    for fn in ("grey_erosion", "grey_dilation", "grey_opening", "grey_closing"):
                        compare(
                            t,
                            fn + "(structure)",
                            desc + f" structure={st.tolist()}",
                            lambda fn=fn: getattr(sp, fn)(
                                a, structure=st, mode=mode, cval=cval, origin=org
                            ),
                            lambda fn=fn: getattr(lx, fn)(
                                a, structure=st, mode=mode, cval=cval, origin=org, n_threads=nt
                            ),
                        )
                        compare(
                            t,
                            fn + "(footprint)",
                            desc,
                            lambda fn=fn: getattr(sp, fn)(
                                a, footprint=fp, mode=mode, cval=cval, origin=org
                            ),
                            lambda fn=fn: getattr(lx, fn)(
                                a, footprint=fp, mode=mode, cval=cval, origin=org, n_threads=nt
                            ),
                        )
                        compare(
                            t,
                            fn + "(size)",
                            desc + f" size={size}",
                            lambda fn=fn: getattr(sp, fn)(a, size=size, mode=mode, cval=cval),
                            lambda fn=fn: getattr(lx, fn)(
                                a, size=size, mode=mode, cval=cval, n_threads=nt
                            ),
                        )
            # axes, output dtypes, sequences of modes
            if nd_ >= 2:
                axes = (0, nd_ - 1)
                fp2 = random_footprint(rng, (3, 2))
                compare(
                    t,
                    "axes",
                    f"{dtype.__name__}{shape} axes={axes}",
                    lambda: sp.median_filter(base, footprint=fp2, axes=axes),
                    lambda: lx.median_filter(base, footprint=fp2, axes=axes, n_threads=nt),
                )
                compare(
                    t,
                    "axes",
                    f"{dtype.__name__}{shape} axes={axes} size",
                    lambda: sp.maximum_filter(base, size=3, axes=axes, mode=["wrap", "nearest"]),
                    lambda: lx.maximum_filter(
                        base, size=3, axes=axes, mode=["wrap", "nearest"], n_threads=nt
                    ),
                )
            if dtype != np.bool_:
                compare(
                    t,
                    "output",
                    f"{dtype.__name__}{shape} output=float64",
                    lambda: sp.minimum_filter(base, size=2, output=np.float64),
                    lambda: lx.minimum_filter(base, size=2, output=np.float64, n_threads=nt),
                )
    # the brain masker's balls on a 3D volume and every thread count
    a = (rng.integers(0, 2000, size=(23, 19, 17)) / 1999.0).astype(np.float32)
    for nt2 in (1, 3, 8):
        compare(
            t,
            "ball filters",
            f"grey_closing ball(1) threads={nt2}",
            lambda: sp.grey_closing(a, structure=ball(1)),
            lambda: lx.grey_closing(a, structure=ball(1), n_threads=nt2),
        )
        compare(
            t,
            "ball filters",
            f"median ball(3) threads={nt2}",
            lambda: sp.median_filter(a, footprint=ball(3)),
            lambda: lx.median_filter(a, footprint=ball(3), n_threads=nt2),
        )
    # 1-D inputs shorter than the footprint's half: SciPy 1.15 returned zeros (its running-rank
    # code returned before writing, gh-23293), SciPy 1.18 runs the n-dimensional filter (and
    # larmorx with it); a single element takes the running-rank code's own branch
    old = tuple(int(v) for v in scipy.__version__.split(".")[:2]) < (1, 18)
    for n in (1, 2, 3):
        x = (rng.integers(-9, 10, n) * 0.5).astype(np.float64)
        for size in (5, 7, 9):
            for mode in ("reflect", "constant", "wrap", "mirror", "nearest"):
                want = sp.median_filter(x, size=size, mode=mode, cval=1.5)
                got = lx.median_filter(x, size=size, mode=mode, cval=1.5)
                lim2 = n - ((size - 1) // 2)
                if old and lim2 < 0:
                    t.add(
                        "1-D rank, SciPy 1.15 step",
                        np.all(want == 0),
                        f"n={n} size={size} {mode}: {want}",
                    )
                    continue
                if (lim2 >= 0 or n == 1) and n > 1 and reads_outside(n, size, mode, 0):
                    # SciPy's running-rank code reads before or after the input here (its value
                    # is memory's); larmorx reads the mode's extension
                    t.add("1-D rank, SciPy reads outside (not compared)", True, "")
                    continue
                steplines.oracle(want)
                t.add("1-D rank, short input", same(want, got), f"n={n} size={size} {mode}")
    # errors
    a = values(rng, (6, 5), np.float32, False)
    for desc, f_sp, f_lx in [
        ("no footprint", lambda: sp.minimum_filter(a), lambda: lx.minimum_filter(a)),
        (
            "all-zero footprint",
            lambda: sp.minimum_filter(a, footprint=np.zeros((2, 2))),
            lambda: lx.minimum_filter(a, footprint=np.zeros((2, 2))),
        ),
        (
            "bad origin",
            lambda: sp.median_filter(a, size=3, origin=2),
            lambda: lx.median_filter(a, size=3, origin=2),
        ),
        (
            "bad origin 1d",
            lambda: sp.minimum_filter1d(a, 3, origin=-2),
            lambda: lx.minimum_filter1d(a, 3, origin=-2),
        ),
        (
            "bad mode",
            lambda: sp.median_filter(a, size=3, mode="foo"),
            lambda: lx.median_filter(a, size=3, mode="foo"),
        ),
        ("bad rank", lambda: sp.rank_filter(a, 9, size=3), lambda: lx.rank_filter(a, 9, size=3)),
        (
            "bad percentile",
            lambda: sp.percentile_filter(a, 101, size=3),
            lambda: lx.percentile_filter(a, 101, size=3),
        ),
        (
            "footprint rank",
            lambda: sp.median_filter(a, footprint=np.ones(3, bool)),
            lambda: lx.median_filter(a, footprint=np.ones(3, bool)),
        ),
        (
            "mode sequence",
            lambda: sp.median_filter(a, size=3, mode=["wrap", "wrap"]),
            lambda: lx.median_filter(a, size=3, mode=["wrap", "wrap"]),
        ),
        ("grey no size", lambda: sp.grey_erosion(a), lambda: lx.grey_erosion(a)),
        (
            "complex",
            lambda: sp.median_filter(a + 1j, size=3),
            lambda: lx.median_filter(a + 1j, size=3),
        ),
        ("size 0 1d", lambda: sp.minimum_filter1d(a, 0), lambda: lx.minimum_filter1d(a, 0)),
    ]:
        compare(t, "errors", desc, f_sp, f_lx)


def centred(st: np.ndarray, org: list) -> bool:
    """Whether the structure's centre is true for the erosion and for the dilation (whose
    structure SciPy reverses and whose origins it negates, minus one on even axes)."""
    for invert in (False, True):
        s2 = st[(slice(None, None, -1),) * st.ndim] if invert else st
        o2 = [
            (-o - (0 if d & 1 else 1)) if invert else o for o, d in zip(org, st.shape, strict=True)
        ]
        c = tuple(o + d // 2 for o, d in zip(o2, st.shape, strict=True))
        if any(i < 0 or i >= d for i, d in zip(c, st.shape, strict=True)) or not s2[c]:
            return False
    return True


def binary(t: Tally, rng, quick: bool, nt: int) -> None:
    shapes = [(15,), (9, 8), (7, 6, 5), (4, 1, 6), (3, 4, 3, 3)]
    for shape in shapes:
        nd_ = len(shape)
        structs = [None, sp.generate_binary_structure(nd_, nd_), random_footprint(rng, (3,) * nd_)]
        if nd_ <= 3:
            structs.append(
                random_footprint(rng, tuple(int(rng.integers(1, 5)) for _ in range(nd_)))
            )
        for dtype in (np.bool_, np.uint8, np.float32, np.int16, np.float64):
            base = rng.random(shape) < 0.55
            if dtype == np.float32:
                a0 = base.astype(np.float32) * rng.choice(
                    [1.0, -0.0, np.nan, 2.5], size=shape
                ).astype(np.float32)
            else:
                a0 = base.astype(dtype)
            for lname, a in layouts(a0, rng):
                if quick and lname == "strided":
                    continue
                for st in structs:
                    its = (1, 2, 3, 0, -1) if not quick else (1, 2, 0)
                    for iterations in its:
                        bv = int(rng.integers(0, 2))
                        if st is None:
                            org = 0
                        else:
                            org = [
                                int(rng.integers(-(s // 2), (s - 1) // 2 + 1)) for s in np.shape(st)
                            ]
                        # SciPy repeats whole passes until nothing changes for a structure whose
                        # centre (after the origin) is false, which can alternate for ever: such
                        # structures only with a fixed number of iterations
                        if iterations < 1 and st is not None and not centred(np.asarray(st), org):
                            continue
                        mk = rng.integers(0, 4)
                        if mk == 0:
                            mask = None
                        elif mk == 1:
                            mask = rng.random(shape) < 0.8
                        elif mk == 2:
                            mask = rng.choice([0.0, 0.5, 1.0, 2.0], size=shape)
                        else:
                            mask = rng.choice([0, 1, 256], size=shape).astype(np.int16)
                        bf = bool(rng.integers(0, 2))
                        desc = f"{np.dtype(dtype).name}{shape} {lname} st={None if st is None else np.asarray(st).astype(int).tolist()} it={iterations} bv={bv} origin={org} mask={None if mask is None else mask.dtype} bf={bf}"
                        # SciPy's own results with upstream_defects="reproduce"; a float or int16
                        # mask over several passes (SCI-10) corrected by default: SciPy's result
                        # with the mask's truth as booleans
                        truth = None if mask is None else mask != 0
                        for fn in ("binary_erosion", "binary_dilation"):
                            compare(
                                t,
                                fn,
                                desc,
                                lambda fn=fn: getattr(sp, fn)(
                                    a,
                                    structure=st,
                                    iterations=iterations,
                                    mask=mask,
                                    border_value=bv,
                                    origin=org,
                                    brute_force=bf,
                                ),
                                lambda fn=fn: getattr(lx, fn)(
                                    a,
                                    structure=st,
                                    iterations=iterations,
                                    mask=mask,
                                    border_value=bv,
                                    origin=org,
                                    brute_force=bf,
                                    n_threads=nt,
                                    upstream_defects="reproduce",
                                ),
                            )
                            if mask is not None and mask.dtype != bool and iterations != 1:
                                compare(
                                    t,
                                    fn + " SCI-10 corrected",
                                    desc,
                                    lambda fn=fn: getattr(sp, fn)(
                                        a,
                                        structure=st,
                                        iterations=iterations,
                                        mask=truth,
                                        border_value=bv,
                                        origin=org,
                                        brute_force=bf,
                                    ),
                                    lambda fn=fn: getattr(lx, fn)(
                                        a,
                                        structure=st,
                                        iterations=iterations,
                                        mask=mask,
                                        border_value=bv,
                                        origin=org,
                                        brute_force=bf,
                                        n_threads=nt,
                                    ),
                                )
                        if iterations >= 1:
                            for fn in ("binary_opening", "binary_closing"):
                                compare(
                                    t,
                                    fn,
                                    desc,
                                    lambda fn=fn: getattr(sp, fn)(
                                        a,
                                        structure=st,
                                        iterations=iterations,
                                        origin=org,
                                        border_value=bv,
                                    ),
                                    lambda fn=fn: getattr(lx, fn)(
                                        a,
                                        structure=st,
                                        iterations=iterations,
                                        origin=org,
                                        border_value=bv,
                                        n_threads=nt,
                                    ),
                                )
                compare(
                    t,
                    "binary_propagation",
                    f"{np.dtype(dtype).name}{shape} {lname}",
                    lambda: sp.binary_propagation(a, mask=base),
                    lambda: lx.binary_propagation(a, mask=base, n_threads=nt),
                )
                compare(
                    t,
                    "binary_fill_holes",
                    f"{np.dtype(dtype).name}{shape} {lname}",
                    lambda: sp.binary_fill_holes(a),
                    lambda: lx.binary_fill_holes(a, n_threads=nt),
                )
                compare(
                    t,
                    "binary_hit_or_miss",
                    f"{np.dtype(dtype).name}{shape} {lname}",
                    lambda: sp.binary_hit_or_miss(a, structure1=structs[1]),
                    lambda: lx.binary_hit_or_miss(a, structure1=structs[1], n_threads=nt),
                )
    # the brain masker's calls on a 3D blob
    g = np.indices((30, 28, 26)).astype(np.float64)
    blob = (((g[0] - 15) / 11) ** 2 + ((g[1] - 13) / 10) ** 2 + ((g[2] - 12) / 9) ** 2) < 1.0
    blob ^= rng.random(blob.shape) < 0.05
    for nt2 in (1, 4):
        compare(
            t,
            "ball morphology",
            f"opening ball(3) threads={nt2}",
            lambda: sp.binary_opening(blob.astype(np.float32), ball(3)),
            lambda: lx.binary_opening(blob.astype(np.float32), ball(3), n_threads=nt2),
        )
        compare(
            t,
            "ball morphology",
            f"closing ball(5) threads={nt2}",
            lambda: sp.binary_closing(blob, ball(5)),
            lambda: lx.binary_closing(blob, ball(5), n_threads=nt2),
        )
        compare(
            t,
            "ball morphology",
            f"dilation ball(5) threads={nt2}",
            lambda: sp.binary_dilation(blob, ball(5)),
            lambda: lx.binary_dilation(blob, ball(5), n_threads=nt2),
        )
        compare(
            t,
            "ball morphology",
            f"erosion ball(5) threads={nt2}",
            lambda: sp.binary_erosion(blob, structure=ball(5)),
            lambda: lx.binary_erosion(blob, structure=ball(5), n_threads=nt2),
        )
    for rank in range(0, 5):
        for conn in range(-1, rank + 2):
            compare(
                t,
                "generate_binary_structure",
                f"rank={rank} conn={conn}",
                lambda: sp.generate_binary_structure(rank, conn),
                lambda: lx.generate_binary_structure(rank, conn),
            )
    for st in (
        sp.generate_binary_structure(2, 1),
        sp.generate_binary_structure(3, 2),
        np.array([1, 0, 1, 1]),
        ball(1),
    ):
        for it in (0, 1, 2, 3):
            compare(
                t,
                "iterate_structure",
                f"{np.asarray(st).shape} it={it}",
                lambda: sp.iterate_structure(st, it),
                lambda: lx.iterate_structure(st, it),
            )
            compare(
                t,
                "iterate_structure",
                f"{np.asarray(st).shape} it={it} origin",
                lambda: sp.iterate_structure(st, it, origin=1),
                lambda: lx.iterate_structure(st, it, origin=1),
            )
    a = rng.random((6, 5)) < 0.5
    for desc, f_sp, f_lx in [
        (
            "iterations float",
            lambda: sp.binary_erosion(a, iterations=1.5),
            lambda: lx.binary_erosion(a, iterations=1.5),
        ),
        (
            "structure ndim",
            lambda: sp.binary_erosion(a, structure=np.ones(3)),
            lambda: lx.binary_erosion(a, structure=np.ones(3)),
        ),
        (
            "empty structure",
            lambda: sp.binary_erosion(a, structure=np.ones((0, 3))),
            lambda: lx.binary_erosion(a, structure=np.ones((0, 3))),
        ),
        (
            "mask shape",
            lambda: sp.binary_erosion(a, mask=np.ones((2, 2))),
            lambda: lx.binary_erosion(a, mask=np.ones((2, 2))),
        ),
        (
            "origin beyond",
            lambda: sp.binary_erosion(a, origin=2),
            lambda: lx.binary_erosion(a, origin=2),
        ),
        (
            "origin -2",
            lambda: sp.binary_dilation(a, origin=-2),
            lambda: lx.binary_dilation(a, origin=-2),
        ),
    ]:
        compare(t, "binary errors", desc, f_sp, f_lx)
    # where SciPy's C code reads outside its arrays (a crash, probe_origin_crash.py), larmorx
    # raises ValueError('invalid origin')
    for desc, f_lx in [
        ("erosion origin -2", lambda: lx.binary_erosion(a, origin=-2)),
        ("erosion origin -3", lambda: lx.binary_erosion(a, origin=-3)),
        ("dilation origin 2", lambda: lx.binary_dilation(a, origin=2)),
        ("dilation origin 3", lambda: lx.binary_dilation(a, origin=3)),
    ]:
        try:
            f_lx()
            ok = False
        except ValueError as e:
            ok = str(e) == "invalid origin"
        t.add("binary errors (SciPy crashes)", ok, desc)


def labels(t: Tally, rng, nt: int) -> None:
    shapes = [(40,), (13, 11), (9, 8, 7), (5, 1, 7), (4, 3, 5, 4), (1,), (1, 1)]
    for shape in shapes:
        nd_ = len(shape)
        for dtype in (np.bool_, np.uint8, np.float32, np.int64, np.float64):
            a0 = (rng.random(shape) < 0.45).astype(dtype)
            for lname, a in layouts(a0, rng):
                for conn in range(1, nd_ + 1):
                    st = sp.generate_binary_structure(nd_, conn)
                    compare(
                        t,
                        "label",
                        f"{np.dtype(dtype).name}{shape} {lname} conn={conn}",
                        lambda: sp.label(a, st),
                        lambda: lx.label(a, st),
                    )
                lab, n = sp.label(a)
                idx = [*range(n + 2), -1]
                for ldt in (np.int32, np.int64, np.uint8, np.int16):
                    labx = lab.astype(ldt)
                    w = rng.integers(-5, 6, size=shape).astype(np.float32) * 0.25
                    compare(
                        t,
                        "sum_labels",
                        f"{shape} {lname} labels {np.dtype(ldt).name}",
                        lambda: sp.sum_labels(w, labx, idx),
                        lambda: lx.sum_labels(w, labx, idx),
                    )
                    compare(
                        t,
                        "sum_labels",
                        f"{shape} {lname} uint8 input",
                        lambda: sp.sum(a0.astype(np.uint8), labx, np.arange(n + 1)),
                        lambda: lx.sum(a0.astype(np.uint8), labx, np.arange(n + 1)),
                    )
                    compare(
                        t,
                        "sum_labels",
                        f"{shape} scalar index",
                        lambda: sp.sum_labels(w, labx, 1),
                        lambda: lx.sum_labels(w, labx, 1),
                    )
                    compare(
                        t,
                        "sum_labels",
                        f"{shape} no index",
                        lambda: sp.sum_labels(w, labx),
                        lambda: lx.sum_labels(w, labx),
                    )
    # the largest-component step of brain_masker
    blob = rng.random((20, 18, 16)) < 0.4
    lab, n = sp.label(blob)
    compare(
        t,
        "label",
        "brain_masker largest component",
        lambda: sp.sum(blob.astype(np.uint8), lab, range(n + 1)),
        lambda: lx.sum(blob.astype(np.uint8), lx.label(blob)[0], range(n + 1)),
    )
    a = rng.random((6, 5)) < 0.5
    for desc, f_sp, f_lx in [
        ("structure 5", lambda: sp.label(a, np.ones((5, 5))), lambda: lx.label(a, np.ones((5, 5)))),
        ("structure rank", lambda: sp.label(a, np.ones(3)), lambda: lx.label(a, np.ones(3))),
        (
            "asymmetric",
            lambda: sp.label(a, np.array([[1, 1, 0], [1, 1, 0], [0, 0, 0]])),
            lambda: lx.label(a, np.array([[1, 1, 0], [1, 1, 0], [0, 0, 0]])),
        ),
        ("scalar", lambda: sp.label(np.array(3)), lambda: lx.label(np.array(3))),
        ("empty", lambda: sp.label(np.zeros((0, 3))), lambda: lx.label(np.zeros((0, 3)))),
        (
            "output int16",
            lambda: sp.label(a, output=np.int16),
            lambda: lx.label(a, output=np.int16),
        ),
    ]:
        compare(t, "label errors", desc, f_sp, f_lx)


# --- the filters' argument handling (relay 21) -------------------------------------------------


class _Outcome(tuple):
    """An outcome (:func:`_outcome`); ``step`` is its form for the originals' step digest when
    that differs from the outcome itself (an object array: its elements' reprs, as its bytes are
    the elements' addresses, which change from process to process)."""

    step: tuple | None = None


def _outcome(f) -> _Outcome:
    """``f()``'s result (type, shape, bytes) or exception (type, message), its warnings
    (category and text) and the upstream-defect entries larmorx's notices name."""
    from larmorx._defects import UpstreamDefectWarning

    step = None
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        try:
            r = np.asarray(f())
            res: tuple = ("ok", r.dtype.str, r.shape, np.ascontiguousarray(r).tobytes())
            if r.dtype == object:
                step = ("ok", r.dtype.str, r.shape, [repr(v) for v in r.ravel()])
        except Exception as e:
            res = (type(e).__name__, str(e))
    msgs = [
        f"{x.category.__name__}: {x.message}" for x in w if x.category is not UpstreamDefectWarning
    ]
    ids = [x.message.entry for x in w if x.category is UpstreamDefectWarning]
    out = _Outcome((res, msgs, ids))
    if step is not None:
        out.step = (step, msgs, ids)
    return out


def _oracle(o: _Outcome) -> _Outcome:
    """SciPy's outcome ``o``, unchanged, its digest gathered for the originals' step
    (``steplines.oracle``) in its step form: never what the comparisons see."""
    steplines.oracle(tuple(o) if o.step is None else o.step)
    return o


def _show(o: tuple) -> str:
    return str(o if o[0] != "ok" else (*o[:3], o[3].hex()[:48]))


_ARG_BASE = np.arange(12.0).reshape(3, 4) % 5
_ARG_LINE = np.array([3.0, 1, 4, 1, 5, 9, 2, 6, 5])


def _arg_filters() -> dict:
    """name -> (call(module, input, **kw), 1-D input?)"""
    fp = [[1, 0, 1], [0, 1, 0]]
    return {
        "minimum_filter1d": (lambda m, x, **k: m.minimum_filter1d(x, 3, **k), False),
        "maximum_filter1d": (lambda m, x, **k: m.maximum_filter1d(x, 3, **k), False),
        "minimum_filter size": (lambda m, x, **k: m.minimum_filter(x, 3, **k), False),
        "maximum_filter footprint": (
            lambda m, x, **k: m.maximum_filter(x, footprint=fp, **k),
            False,
        ),
        "grey_erosion structure": (
            lambda m, x, **k: m.grey_erosion(x, structure=np.ones((2, 2)), **k),
            False,
        ),
        "grey_dilation size": (lambda m, x, **k: m.grey_dilation(x, size=2, **k), False),
        "grey_opening": (lambda m, x, **k: m.grey_opening(x, size=2, **k), False),
        "grey_closing": (lambda m, x, **k: m.grey_closing(x, size=2, **k), False),
        "rank_filter": (lambda m, x, **k: m.rank_filter(x, 1, 3, **k), False),
        "median_filter": (lambda m, x, **k: m.median_filter(x, 3, **k), False),
        "percentile_filter": (lambda m, x, **k: m.percentile_filter(x, 40, 3, **k), False),
        "median_filter 1-D": (lambda m, x, **k: m.median_filter(x, 3, **k), True),
        "rank_filter 1-D": (lambda m, x, **k: m.rank_filter(x, 1, 3, **k), True),
        "percentile_filter 1-D": (lambda m, x, **k: m.percentile_filter(x, 50, 3, **k), True),
        "minimum_filter1d 1-D": (lambda m, x, **k: m.minimum_filter1d(x, 3, **k), True),
    }


def _arg_cases(line: bool) -> list[tuple[str, np.ndarray, dict]]:
    """(label, input, keyword arguments): inputs of unsupported and odd types, outputs as
    types, strings and arrays, ``cval``, origins, modes, axes, sizes and ranks of every Python
    and numpy kind. Values that would make SciPy write outside its arrays are not among them
    (SCI-21: :func:`_sci21`)."""
    b = _ARG_LINE if line else _ARG_BASE
    shape = b.shape
    ro = np.zeros(shape)
    ro.flags.writeable = False
    cases: list[tuple[str, np.ndarray, dict]] = []
    for k, dt in {
        "float16": np.float16,
        "longdouble": np.longdouble,
        "object": object,
        "complex64": np.complex64,
        "bool": np.bool_,
        "str": "U3",
        "int8": np.int8,
        "uint64": np.uint64,
        ">i8": ">i8",
        ">f8 (n-D)": ">f8",
    }.items():
        if line and k == ">f8 (n-D)":
            continue
        cases.append((f"input {k}", b.astype(dt), {}))
    cases.append(("input empty float16", np.zeros((0,) if line else (0, 3), np.float16), {}))
    outputs = {
        "type float16": np.float16,
        "str 'bool'": "bool",
        "str 'object'": "object",
        "str 'U3'": "U3",
        "str 'float16'": "float16",
        "type bool": np.bool_,
        "type complex64": np.complex64,
        "array of another shape": lambda: np.zeros((*shape[:-1], shape[-1] + 1)),
        "read-only array": lambda: ro,
        "list": lambda: np.zeros(shape).tolist(),
        "list of another shape": lambda: [0.0],
    }
    if line:
        # SciPy's running rank writes a float64 input's values into any output of another type
        # (SCI-21, past its end when its items are smaller): those are SCI-21's own cases
        for k in ("type float16", "str 'float16'", "type bool", "type complex64"):
            del outputs[k]
        outputs.update(
            {
                "type float64": np.float64,
                "array float64": lambda: np.zeros(shape),
                "array int8 (int16 input)": lambda: np.zeros(shape, np.int8),
            }
        )
    else:
        outputs.update(
            {
                "array float16": lambda: np.zeros(shape, np.float16),
                "array int8": lambda: np.zeros(shape, np.int8),
                "array complex64": lambda: np.zeros(shape, np.complex64),
                "dtype float32": np.dtype(np.float32),
                "str 'f4'": "f4",
                "array strided": lambda: np.zeros((*shape, 2))[..., 0],
                "array >f8": lambda: np.zeros(shape, ">f8"),
            }
        )
    for k, v in outputs.items():
        x = b.astype(np.int16) if "int16 input" in k else b
        cases.append((f"output {k}", x, {"output": v}))
    for k, v in {
        "complex": 1j,
        "numpy complex": np.complex128(1 + 0j),
        "str": "x",
        "None": None,
        "numpy int": np.int64(3),
        "numpy str": np.str_("1"),
        "bool": True,
        "NaN": float("nan"),
        "0-d array": np.array(2.0),
        "1-element array": np.array([2.0]),
        "float32 7.1": np.float32(7.1),
    }.items():
        cases.append((f"cval {k}", b, {"mode": "constant", "cval": v}))
        if line:
            cases.append((f"cval {k} (int8)", b.astype(np.int8), {"mode": "constant", "cval": v}))
    for k, v in {
        "float 0.0": 0.0,
        "float 1.0": 1.0,
        "float -1.0": -1.0,
        "numpy int": np.int64(1),
        "True": True,
        "numpy True": np.True_,
        "-1": -1,
        "beyond": 5,
    }.items():
        cases.append((f"origin {k}", b, {"origin": v}))
    for k, v in {
        "unknown": "bogus",
        "grid-wrap": "grid-wrap",
        "grid-constant": "grid-constant",
    }.items():
        cases.append((f"mode {k}", b, {"mode": v}))
    if not line:
        cases.append(("mode sequence", b, {"mode": ["reflect", "nearest"]}))
        for k, v in {"float": (1.0,), "repeated": (0, 0), "beyond": (5,), "int": 1}.items():
            cases.append((f"axes {k}", b, {"axes": v}))
    return cases


def _arg_axis_cases() -> list[tuple[str, dict]]:
    return [(f"axis {k}", {"axis": v}) for k, v in
            {"float": 1.0, "numpy int": np.int64(0), "beyond": 5, "-3": -3, "True": True}.items()]  # fmt: skip


def _arg_extra() -> list[tuple[str, object, object]]:
    """Sizes, ranks and percentiles of every kind, the separable path with nothing to filter,
    input arrays as output arrays."""
    b, x = _ARG_BASE, _ARG_LINE
    f16 = b.astype(np.float16)
    out: list[tuple[str, object, object]] = []
    for k, v in {"float 3.0": 3.0, "True": True, "numpy int": np.int64(3), "0": 0}.items():
        out.append((f"minimum_filter1d size {k}", lambda m, v=v: m.minimum_filter1d(x, v), None))
        out.append((f"minimum_filter size {k}", lambda m, v=v: m.minimum_filter(b, v), None))
    for k, v in {"float 1.0": 1.0, "numpy int": np.int64(1), "True": True, "-1": -1}.items():
        out.append((f"rank_filter rank {k}", lambda m, v=v: m.rank_filter(x, v, 3), None))
        out.append((f"rank_filter n-D rank {k}", lambda m, v=v: m.rank_filter(b, v, 3), None))
    for k, v in {
        "str": "50",
        "50.5": 50.5,
        "float32": np.float32(50),
        "-30": -30,
        "101": 101,
    }.items():
        out.append((f"percentile_filter {k}", lambda m, v=v: m.percentile_filter(x, v, 3), None))
    for k, v in {"float16": f16, "object": b.astype(object), "str cval": None}.items():
        kw = {"cval": "x"} if v is None else {}
        arr = b if v is None else v
        out.append(
            (f"separable size 1 {k}", lambda m, a=arr, kw=kw: m.minimum_filter(a, 1, **kw), None)
        )
    for name, fn in {
        "minimum_filter": lambda m, a: m.minimum_filter(a, 3, output=a),
        "maximum_filter footprint": lambda m, a: m.maximum_filter(
            a, footprint=[[1, 0, 1]], output=a
        ),
        "median_filter": lambda m, a: m.median_filter(a, 3, output=a),
        "median_filter 1-D": lambda m, a: m.median_filter(a.reshape(-1), 3, output=a.reshape(-1)),
    }.items():
        out.append((f"{name} into its input", None, fn))
    return out


def arguments(t: Tally) -> None:
    """The filters' argument handling against SciPy's: the default and ``reproduce`` both
    (outside SCI-21 to SCI-23 they agree), every result, error and warning."""
    for fname, (call, line) in _arg_filters().items():
        cases = _arg_cases(line)
        if "1d" in fname and not line:
            cases += [(lab, _ARG_BASE, kw) for lab, kw in _arg_axis_cases()]
        for label, x, kw in cases:
            if line and fname.startswith("minimum_filter1d") and label.startswith("output"):
                continue

            def run(m, mode=None, x=x, kw=kw, call=call):
                kk = {k: (v() if callable(v) else v) for k, v in kw.items()}
                if mode is not None:
                    kk["upstream_defects"] = mode
                return call(m, x, **kk)

            want = _oracle(_outcome(lambda: run(sp)))
            fixed = _outcome(lambda: run(lx))
            if "rank" in fname or "median" in fname or "percentile" in fname:
                repro = _outcome(lambda: run(lx, "reproduce"))
            else:
                repro = fixed
            if _scipy115_system_error(want[0]):
                # SCI-24 (SciPy 1.15.3; fixed in 1.18.1): larmorx gives 1.18.1's error
                t.add(
                    "arguments: SCI-24 (SciPy 1.15.3's SystemError; 1.18.1's error given)",
                    fixed[0][0] == repro[0][0] == "TypeError" and not fixed[2],
                    f"{fname} {label}: {_show(fixed[0])}",
                )
                continue
            t.add(
                f"arguments: {fname}",
                want[:2] == fixed[:2] == repro[:2] and not fixed[2],
                f"{label}: scipy {_show(want[0])} {want[1]}; larmorx {_show(fixed[0])} "
                f"{fixed[1]} {fixed[2]}; reproduce {_show(repro[0])}",
            )
    for label, call, into in _arg_extra():
        if into is None:
            want, got = _oracle(_outcome(lambda: call(sp))), _outcome(lambda: call(lx))
        else:
            a1, a2 = _ARG_BASE.reshape(3, 4).copy(), _ARG_BASE.reshape(3, 4).copy()
            want, got = (
                _oracle(_outcome(lambda: (into(sp, a1), a1)[1])),
                _outcome(lambda: (into(lx, a2), a2)[1]),
            )
        t.add("arguments: sizes, ranks, outputs", want[:2] == got[:2] and not got[2],
              f"{label}: scipy {_show(want[0])} {want[1]}; larmorx {_show(got[0])} {got[1]} {got[2]}")  # fmt: skip
    _sci21(t)
    _sci22(t)
    _sci23(t)
    _morph_arguments(t)


def _morph_arguments(t: Tally) -> None:
    """Binary morphology's, ``label``'s, ``sum_labels``' (and ``sum``'s), ``center_of_mass``'
    and ``watershed_ift``'s argument handling (relay 22, ``relay22/morph_cases.py``): every case
    with ``reproduce`` = SciPy's, the default = SciPy's or a registered correction; the SCI-25
    family and ``label``'s numbering into outputs of every layout."""
    import importlib.util
    from pathlib import Path

    path = Path(__file__).resolve().parent / "relay22" / "morph_cases.py"
    spec = importlib.util.spec_from_file_location("larmorx_relay22_morph_cases", path)
    mc = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mc)
    cats: dict[str, int] = defaultdict(int)
    for name, rep, ok, cat, detail in mc.cases():
        fn = name.split(" ")[0]
        t.add(f"arguments: {fn} (reproduce = SciPy)", rep, f"{name}: {detail}")
        t.add(
            f"arguments: {fn} (the default = SciPy or a registered correction)",
            ok,
            f"{name}: {detail}",
        )
        cats[cat] += 1
    print(
        "INFO the binary morphology's, labels' and sums' defaults: "
        + ", ".join(f"{k} {v}" for k, v in sorted(cats.items()))
    )
    for name, ok, detail in (*mc.sci25_family(), *mc.label_order_family()):
        t.add(f"arguments: {name}", ok, detail)


def _scipy115_system_error(o: tuple) -> bool:
    """SCI-24: SciPy 1.15.3's running rank returns with a Python error set (an unknown type
    number, a ``cval`` its type cannot take), which CPython turns into a ``SystemError``."""
    return (
        scipy.__version__.startswith("1.15.")
        and o[0] == "SystemError"
        and "returned a result with an exception set" in o[1]
    )


#: SCI-21's SciPy calls: how many processes one call may take, and each one's time limit (s).
SCI21_ATTEMPTS = 10
SCI21_TIMEOUT = 30
#: The end of the line a SCI-21 process prints (a line without it was cut short by a crash).
SCI21_END = " END"


def _scipy_in_process(code: str) -> tuple[str, str]:
    """SciPy's result of ``code`` (which prints it, ending in :data:`SCI21_END`): SCI-21's calls
    write past their output's end, which corrupts the heap, so each runs in a process of its own
    (through the oracle-output cache, never cached: memory SciPy never defines). Such a process
    may crash (a signal) or hang (its time limit) before it prints, at random, as the heap's
    layout changes from process to process (address-space and hash randomisation); one that
    printed gives the same result in every process (the bytes within the output's extent and
    the warnings: ``relay24/probe_sci21.py``, 360 processes per environment and way of starting
    them). So a call whose process printed nothing runs again, in a new process, up to
    :data:`SCI21_ATTEMPTS` times. Returns (the printed result without its end, or ``""`` when no
    process printed one; the last process's outcome)."""
    from larmorx_validation import oracle_cache

    policy = oracle_cache.Policy(
        threads=1,
        never="SCI-21: SciPy's running rank writes past its output's end, into memory it never "
        "defines (D13); its process may crash or hang at random",
    )
    status = ""
    for _ in range(SCI21_ATTEMPTS):
        done = oracle_cache.run(
            [sys.executable, "-I", "-c", code],
            policy=policy,
            timeout=SCI21_TIMEOUT,
            runner="check-ndimage-sci21",
        )
        line = done.text().strip()
        if line.endswith(SCI21_END):
            return line.removesuffix(SCI21_END), f"exit {done.returncode}"
        status = "hung (its time limit)" if done.timed_out else f"exit {done.returncode}"
    return "", status


def _sci21(t: Tally) -> None:
    """SCI-21: SciPy's running rank of a float32, float64 or int64 input writes the input
    type's values into an output of another type (another type or byte order), past its end
    when its items are smaller. ``reproduce`` = SciPy's bytes where they are its output's (each
    call's SciPy in a process of its own: :func:`_scipy_in_process`); the default = the values
    cast into the output, with the notice.

    Every call is one case of both comparisons, whatever SciPy's processes did, so the counts
    are the same in every run: a call none of whose processes printed a result (none in the
    runs so far) is counted as such, its ``reproduce`` checked for its notice and the output's
    type and size alone (SciPy's bytes unknown), and the INFO line counts those calls (0 when
    every call gave SciPy's result)."""
    x = _ARG_LINE
    cases = {
        "float64 into float32": ("float64", "np.float32"),
        "float64 into int16": ("float64", "np.int16"),
        "float64 into uint8": ("float64", "np.uint8"),
        "float32 into float64": ("float32", "np.float64"),
        "float32 into int64": ("float32", "np.int64"),
        "int64 into float64": ("int64", "np.float64"),
        "int64 into int32": ("int64", "np.int32"),
        "float64 into >f8": ("float64", "np.dtype('>f8')"),
        "float32 into >f4": ("float32", "np.dtype('>f4')"),
        "float64 into a float32 array": ("float64", "np.full(9, 7, np.float32)"),
        "float32 into a float64 array": ("float32", "np.full(9, 7.0)"),
        # (into a strided float32 array SciPy's copy overflows and its process hangs)
        "float32 into a strided float64 array": ("float32", "np.full((9, 2), 7.0)[:, 0]"),
    }
    crashed = 0
    for label, (src, out) in cases.items():
        for fn in ("median_filter(x, 3", "rank_filter(x, 1, 3", "percentile_filter(x, 50, 3"):
            code = (
                "import warnings, numpy as np\nfrom scipy import ndimage as sp\n"
                f"x = np.array({x.tolist()}, dtype='{src}')\n"
                "with warnings.catch_warnings(record=True) as w:\n"
                "    warnings.simplefilter('always')\n"
                f"    r = sp.{fn}, output={out})\n"
                "print(r.dtype.str, np.ascontiguousarray(r).tobytes().hex(), "
                "repr([f'{type(m.message).__name__}: {m.message}' for m in w]) "
                f"+ {SCI21_END!r}, flush=True)\n"
            )
            sp_out, status = _scipy_in_process(code)
            steplines.oracle(sp_out or "SciPy's processes gave no result")
            xs = x.astype(src)
            repro = _outcome(
                lambda: eval(
                    f"lx.{fn}, output={out}, upstream_defects='reproduce')",
                    {"lx": lx, "np": np, "x": xs},
                )
            )
            fixed = _outcome(lambda: eval(f"lx.{fn}, output={out})", {"lx": lx, "np": np, "x": xs}))
            mine = ""
            if repro[0][0] == "ok":
                mine = f"{repro[0][1]} {repro[0][3].hex()} {[m for m in repro[1]]!r}"
            if sp_out:
                ok = sp_out == mine and repro[2] == ["SCI-21"]
                what = f"scipy {sp_out[:120]}"
            else:
                # no process printed SciPy's result: only what does not depend on its bytes
                crashed += 1
                target = eval(out, {"np": np})
                tdt = np.dtype(target.dtype if isinstance(target, np.ndarray) else target)
                ok = (
                    repro[0][0] == "ok"
                    and repro[0][1] == tdt.str
                    and len(repro[0][3]) == x.size * tdt.itemsize
                    and repro[2] == ["SCI-21"]
                )
                what = f"SciPy's processes gave no result ({status})"
            t.add("arguments: SCI-21 reproduced (SciPy's own bytes)", ok,
                  f"{label} {fn}: {what}; larmorx {mine[:120]} {repro[2]}")  # fmt: skip
            # corrected: SciPy's own result of the input's type, cast into the output
            plain = getattr(sp, fn.split("(")[0])(xs, *eval("[" + fn.split(", ", 1)[1] + "]"))
            steplines.oracle(plain)
            target = eval(out, {"np": np})
            if isinstance(target, np.ndarray):
                target = target.copy()
                np.copyto(target, plain, casting="unsafe")
                want = target
            else:
                want = plain.astype(target)
            ok = (
                fixed[0][0] == "ok"
                and fixed[0][1] == want.dtype.str
                and fixed[0][3] == np.ascontiguousarray(want).tobytes()
            )
            t.add("arguments: SCI-21 corrected (the values cast into the output)", ok and fixed[2] == ["SCI-21"] and not fixed[1],
                  f"{label} {fn}: larmorx {_show(fixed[0])} {fixed[1]} {fixed[2]}")  # fmt: skip
    print(
        "INFO SCI-21 calls whose SciPy processes all crashed or hung "
        f"({SCI21_ATTEMPTS} each): {crashed}"
    )


def _sci22(t: Tally) -> None:
    """SCI-22: SciPy's running rank refuses big-endian float32 and float64 inputs (by dtype
    equality) and numpy's ``longlong`` (its C++ switch knows only the type number of
    ``int64``; SciPy 1.15.3's returns with the error set: a ``SystemError``). ``reproduce`` =
    SciPy's error (1.18.1's); the default = SciPy's result of the native copy, with the notice."""
    x = _ARG_LINE
    for label, dt in {">f8": ">f8", ">f4": ">f4", "longlong": np.longlong}.items():
        xs = x.astype(dt)
        native = xs.astype(np.dtype(xs.dtype.kind + str(xs.dtype.itemsize)))
        for fn in ("median_filter", "rank_filter", "percentile_filter"):
            args = {"median_filter": (3,), "rank_filter": (1, 3), "percentile_filter": (50, 3)}[fn]
            want = _oracle(_outcome(lambda: getattr(sp, fn)(xs, *args)))
            repro = _outcome(lambda: getattr(lx, fn)(xs, *args, upstream_defects="reproduce"))
            if label == "longlong" and _scipy115_system_error(want[0]):
                t.add("arguments: SCI-24 (SciPy 1.15.3's SystemError; 1.18.1's error given)",
                      repro[0] == ("TypeError", "Unsupported array type"), f"{label} {fn}: {_show(repro[0])}")  # fmt: skip
            else:
                t.add("arguments: SCI-22 reproduced (SciPy's error)", want[:2] == repro[:2],
                      f"{label} {fn}: scipy {_show(want[0])}; larmorx {_show(repro[0])}")  # fmt: skip
            fixed = _outcome(lambda: getattr(lx, fn)(xs, *args))
            ref = _oracle(_outcome(lambda: getattr(sp, fn)(native, *args)))
            t.add("arguments: SCI-22 corrected (SciPy's result of the native copy)",
                  fixed[0] == ref[0] and fixed[2] == ["SCI-22"], f"{label} {fn}: {_show(fixed[0])} {fixed[2]}")  # fmt: skip


def _sci23(t: Tally) -> None:
    """SCI-23: SciPy's running rank into a non-contiguous output of its input's type never
    resolves numpy's write-back copy: numpy warns when the copy is freed (its text per
    release) and writes it back. ``reproduce`` = the values and numpy's warning; the default =
    the values without it."""
    x = _ARG_LINE
    for fn, args in {
        "median_filter": (3,),
        "rank_filter": (1, 3),
        "percentile_filter": (50, 3),
    }.items():
        for dt in (np.float64, np.float32, np.int64):
            xs = x.astype(dt)
            o1, o2, o3 = (np.zeros((9, 2), dt)[:, 0] for _ in range(3))
            want = _oracle(_outcome(lambda: getattr(sp, fn)(xs, *args, output=o1)))
            repro = _outcome(
                lambda: getattr(lx, fn)(xs, *args, output=o2, upstream_defects="reproduce")
            )
            fixed = _outcome(lambda: getattr(lx, fn)(xs, *args, output=o3))
            t.add("arguments: SCI-23 reproduced (numpy's warning)", want[:2] == repro[:2] and o1.tobytes() == o2.tobytes(),
                  f"{fn} {np.dtype(dt).name}: scipy {want[1]}; larmorx {repro[1]}")  # fmt: skip
            t.add("arguments: SCI-23 corrected (no warning)", want[0] == fixed[0] and not fixed[1] and o1.tobytes() == o3.tobytes(),
                  f"{fn} {np.dtype(dt).name}: {fixed[1]}")  # fmt: skip


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--quick", action="store_true")
    ap.add_argument("--threads", type=int, default=2)
    ap.add_argument("--verbose", action="store_true")
    ap.add_argument(
        "--sections",
        default="filters,binary,labels,arguments",
        help="comma-separated: filters, binary, labels, arguments (the filters' argument "
        "handling, SCI-21 to SCI-23, relay 21; binary morphology's, label's, the sums' and "
        "watershed_ift's, SCI-10, SCI-25 to SCI-30, relay 22)",
    )
    args = ap.parse_args()
    sections = set(args.sections.split(","))
    global VERBOSE
    VERBOSE = args.verbose
    print(f"numpy {np.__version__}, SciPy {scipy.__version__}")
    rng = np.random.default_rng(20261108)
    t = Tally()
    if "filters" in sections:
        filters(t, rng, args.quick, args.threads)
        print("filters done", flush=True)
    if "binary" in sections:
        binary(t, rng, args.quick, args.threads)
        print("binary morphology done", flush=True)
    if "labels" in sections:
        labels(t, rng, args.threads)
    if "arguments" in sections:
        arguments(t)
    bad = t.report()
    steplines.lines()
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
