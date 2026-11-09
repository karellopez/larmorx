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

import numpy as np
import scipy
from scipy import ndimage as sp

import larmorx.ndimage as lx

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
            want = f_sp()
            werr = None
        except Exception as e:
            want, werr = None, (type(e).__name__, str(e))
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--quick", action="store_true")
    ap.add_argument("--threads", type=int, default=2)
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()
    global VERBOSE
    VERBOSE = args.verbose
    print(f"numpy {np.__version__}, SciPy {scipy.__version__}")
    rng = np.random.default_rng(20261108)
    t = Tally()
    filters(t, rng, args.quick, args.threads)
    print("filters done", flush=True)
    binary(t, rng, args.quick, args.threads)
    print("binary morphology done", flush=True)
    labels(t, rng, args.threads)
    bad = t.report()
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
