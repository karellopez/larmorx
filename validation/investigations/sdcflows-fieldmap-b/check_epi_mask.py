# SPDX-License-Identifier: Apache-2.0
"""sdcflows' ``epi_mask`` (``utils/epimanip.py``) and SciPy's ``watershed_ift`` and
``center_of_mass`` against ``lx.sdcflows.epi_mask``, ``lx.ndimage.watershed_ift`` and
``lx.ndimage.center_of_mass``.

No workflow calls ``epi_mask``; the references are sdcflows 2.17.1's own function and the
environment's SciPy (1.15.3 or 1.18.1: the same ``ni_measure.c`` and wrappers), on:

- ``watershed``: seeded random cases, 1-D to 4-D (axes of length 1), ``uint8`` and ``uint16``
  inputs (many levels, few levels for ties, flat), ``int8`` to ``int64`` markers (objects,
  background, values beyond 32 bits), the default, full and random structures (centre false
  too), ``int8`` to ``int64`` output arrays, Fortran-ordered, strided and big-endian arrays,
  0-d and empty arrays, and the wrapper's errors. ``upstream_defects='reproduce'`` must give
  SciPy's labels and type bit for bit; the corrected labels (the default) are checked against an
  independent Python implementation of the corrected transform (per-level queues, a neighbour
  inside the array, an element always taken out of its queue, 64-bit markers), and the
  differences from SciPy are counted by their cause (1-D arrays have no neighbour across an
  edge: their differences are the queue's alone);
- ``com``: ``center_of_mass`` of float32, float64 (C and Fortran order, strided), integer and
  boolean arrays, with no labels, labels with no index, a scalar index and a sequence of
  indexes, an empty selection (NaN), with ``numpy_version`` = this environment's numpy;
- ``real``: the BOLD references (``desc-hmc_boldref``) of four fMRIPrep oracle runs (single
  band, a child, multiband 8, multi-echo);
- ``synthetic``: a scaled ``int16`` image, a ``float64`` one, one with a length-1 axis, one
  whose object touches the edges, a 4-D image and a flat one (errors), the default output name.

Files compared by their uncompressed bytes (nibabel's writer against larmorx's), errors by type
and message. Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``; boxed):

    OPENBLAS_CORETYPE=Haswell python -P check_epi_mask.py --scratch <folder> [--sections ...]

Prints one line per check and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
import time
import warnings
from collections import Counter, deque
from pathlib import Path
from typing import Any

import nibabel as nb
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_derivatives as cd  # noqa: E402
import check_workflows as cw  # noqa: E402
import steplines  # noqa: E402  (the originals' step: SciPy's and sdcflows' outputs)

REAL_RUNS = (
    "ds005040-sub-002-noreconall",
    "ds000228-sub-pixar008-noreconall",
    "ds004958-sub-fsm64ps-noreconall",
    "ds003592-sub-280-noreconall",
)


def _err(fn: Any) -> tuple[Any, str]:
    try:
        return fn(), ""
    except Exception as e:  # the error is the result
        return None, f"{type(e).__name__}: {e}"


# --- watershed_ift ---------------------------------------------------------------------------


def ref_ift(inp: np.ndarray, markers: np.ndarray, structure: np.ndarray, bits: int) -> np.ndarray:
    """The corrected image foresting transform, written independently: SciPy's order (levels
    in increasing order, each queue from its front; a non-negative label to the front, a
    negative one to the end; the markers in C order), neighbours inside the array only, an
    element always leaving its queue, 64-bit markers; labels stored in a ``bits``-bit type."""
    shape = inp.shape
    n = inp.size
    vals = [int(v) for v in inp.ravel()]
    mk = [int(v) for v in markers.ravel()]

    def store(v: int) -> int:
        m = 1 << bits
        v %= m
        return v - m if v >= m >> 1 else v

    maxval = max(vals) if n else 0
    cost = [maxval + 1] * n
    done = [False] * n
    lab = [store(m) for m in mk]
    queues: list[deque[int]] = [deque() for _ in range(maxval + 1)]
    for j, m in enumerate(mk):
        if m != 0:
            cost[j] = 0
            if m > 0:
                queues[0].appendleft(j)
            else:
                queues[0].append(j)
    ndim = len(shape)
    centre = structure.size // 2
    deltas = [
        np.array(np.unravel_index(k, (3,) * ndim)) - 1
        for k in range(structure.size)
        if structure.ravel()[k] and k != centre
    ]
    for level in range(maxval + 1):
        q = queues[level]
        while q:
            v = q.popleft()
            done[v] = True
            cv = np.array(np.unravel_index(v, shape)) if ndim else np.zeros(0, int)
            for d in deltas:
                c = cv + d
                if any(c[i] < 0 or c[i] >= shape[i] for i in range(ndim)):
                    continue
                p = int(np.ravel_multi_index(tuple(c), shape))
                if done[p]:
                    continue
                mx = max(cost[v], abs(vals[p] - vals[v]))
                if mx < cost[p]:
                    if cost[p] <= maxval:
                        queues[cost[p]].remove(p)
                    cost[p] = mx
                    label = lab[v]
                    lab[p] = store(label)
                    if label < 0:
                        queues[mx].append(p)
                    else:
                        queues[mx].appendleft(p)
    return np.array(lab, dtype=np.int64).reshape(shape)


def _structure(rng: np.random.Generator, ndim: int) -> np.ndarray | None:
    from scipy import ndimage

    kind = rng.integers(0, 4)
    if kind == 0 or ndim == 0:
        return None
    if kind == 1:
        return ndimage.generate_binary_structure(ndim, ndim)
    if kind == 2:
        return ndimage.generate_binary_structure(ndim, int(rng.integers(1, ndim + 1)))
    s = rng.random((3,) * ndim) < 0.5
    if rng.random() < 0.5:
        s = s | s[(slice(None, None, -1),) * ndim]  # symmetric
    return s


def watershed_cases(n: int, seed: int) -> list[dict[str, Any]]:
    rng = np.random.default_rng(seed)
    cases: list[dict[str, Any]] = []
    shapes = [
        (12,), (40,), (1,), (5, 7), (9, 1), (1, 9), (6, 6), (4, 5, 6), (3, 1, 8), (7, 7, 7),
        (2, 3, 4, 5), (1, 1, 1), (10, 12), (16, 3), (5, 5, 5),
    ]  # fmt: skip
    for i in range(n):
        shape = shapes[i % len(shapes)]
        dtype = np.uint8 if rng.random() < 0.6 else np.uint16
        levels = [2, 4, 16, 256, 1000][int(rng.integers(0, 5))]
        if dtype == np.uint8:
            levels = min(levels, 256)
        inp = rng.integers(0, levels, size=shape).astype(dtype)
        if rng.random() < 0.1:
            inp[...] = inp.flat[0]
        mdt = [np.int8, np.int16, np.int32, np.int64][int(rng.integers(0, 4))]
        markers = np.zeros(shape, mdt)
        k = int(rng.integers(1, 5))
        for _ in range(k):
            pos = tuple(int(rng.integers(0, s)) for s in shape)
            lim = int(np.iinfo(mdt).max)
            v = int(rng.integers(1, min(lim, 6) + 1))
            if rng.random() < 0.35:
                v = -v
            if mdt == np.int64 and rng.random() < 0.08:
                v = int(rng.choice([2**31, 2**32 + 3, -(2**31) - 5, 2**40]))
            markers[pos] = v
        out = None
        if rng.random() < 0.2:
            out = np.zeros(shape, [np.int8, np.int16, np.int32, np.int64][int(rng.integers(0, 4))])
        layout = int(rng.integers(0, 6))
        cases.append(
            {
                "label": f"{i} {shape} {np.dtype(dtype).name} {np.dtype(mdt).name}",
                "input": inp,
                "markers": markers,
                "structure": _structure(rng, len(shape)),
                "output": out,
                "layout": layout,
            }
        )
    return cases


def _layout(a: np.ndarray, layout: int) -> np.ndarray:
    if a.ndim < 2:
        return a
    if layout == 1:
        return np.asfortranarray(a)
    if layout == 2:
        big = np.zeros(tuple(2 * s for s in a.shape), a.dtype)
        big[tuple(slice(None, None, 2) for _ in a.shape)] = a
        return big[tuple(slice(None, None, 2) for _ in a.shape)]
    if layout == 3:
        return a.astype(a.dtype.newbyteorder(">"))
    return a


def sec_watershed(t: cw.Tally, n: int) -> None:
    from scipy import ndimage

    from larmorx import ndimage as lnd

    causes: Counter[str] = Counter()
    for c in watershed_cases(n, 20261109):
        lab = c["label"]
        inp = _layout(c["input"], c["layout"])
        mk = _layout(c["markers"], c["layout"])
        st = c["structure"]
        out_ref = None if c["output"] is None else c["output"].copy()
        out_mine = None if c["output"] is None else c["output"].copy()
        want, e1 = steplines.oracle(_err(lambda: ndimage.watershed_ift(inp, mk, st, out_ref)))  # noqa: B023
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            got, e2 = _err(
                lambda: lnd.watershed_ift(inp, mk, st, out_mine, upstream_defects="reproduce")  # noqa: B023
            )
        if e1 or e2:
            t.add("watershed: the same error", e1 == e2, f"{lab}: {e1!r} / {e2!r}")
            continue
        same = (
            want.dtype == got.dtype
            and want.shape == got.shape
            and np.ascontiguousarray(want).tobytes() == np.ascontiguousarray(got).tobytes()
        )
        if out_ref is not None:
            same = same and got is out_mine and out_ref.tobytes() == out_mine.tobytes()
        t.add("watershed: reproduce = SciPy's labels and type", same, lab)
        # the corrected labels
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            fixed = lnd.watershed_ift(
                inp, mk, st, None if c["output"] is None else c["output"].copy()
            )
        warned = sorted({w.message.entry for w in caught if hasattr(w.message, "entry")})
        bits = 8 * (c["output"].dtype.itemsize if c["output"] is not None else mk.dtype.itemsize)
        struct = (
            ndimage.generate_binary_structure(inp.ndim, 1) if st is None else np.asarray(st, bool)
        )
        ref = ref_ift(np.asarray(inp), np.asarray(mk), struct, bits)
        t.add(
            "watershed: fix = the independent corrected transform",
            np.array_equal(np.asarray(fixed, dtype=np.int64), ref),
            lab,
        )
        differ = not np.array_equal(np.asarray(fixed), np.asarray(want))
        t.add("watershed: a warning exactly when the labels differ", bool(warned) == differ, lab)
        if differ:
            alone = inp.ndim == 1 and warned == ["SCI-14"]
            causes[" ".join(warned) + (" (1-D: the queue alone)" if alone else "")] += 1
    for cause, k in sorted(causes.items()):
        print(f"INFO watershed: corrected labels differ from SciPy's in {k} cases: {cause}")
    # the wrapper's errors and the edge cases
    z8 = np.zeros((3, 3), np.uint8)
    zi = np.zeros((3, 3), int)
    special = [
        ("0-d", lambda f: f(np.array(3, np.uint8), np.array(-4))),
        ("empty", lambda f: f(np.zeros((0, 3), np.uint8), np.zeros((0, 3), int))),
        ("list inputs", lambda f: f([[1, 2], [3, 4]], [[0, 1], [0, -1]])),
        ("output dtype ignored", lambda f: f(z8, zi.astype(np.int16), output=np.int8)),
        ("int16 input", lambda f: f(z8.astype(np.int16), zi)),
        ("float input", lambda f: f(z8.astype(np.float32), zi)),
        ("uint8 markers", lambda f: f(z8, zi.astype(np.uint8))),
        ("bool markers", lambda f: f(z8, zi.astype(bool))),
        ("float markers", lambda f: f(z8, zi.astype(np.float64))),
        ("markers' shape", lambda f: f(z8, np.zeros((3, 4), int))),
        ("structure rank", lambda f: f(z8, zi, structure=np.ones((3,), bool))),
        ("structure size", lambda f: f(z8, zi, structure=np.ones((3, 5), bool))),
        ("uint16 output", lambda f: f(z8, zi, output=np.zeros((3, 3), np.uint16))),
        ("output shape", lambda f: f(z8, zi, output=np.zeros((3, 4), np.int64))),
        ("8-D", lambda f: f(np.zeros((2,) * 8, np.uint8), np.zeros((2,) * 8, int))),
        ("longlong markers", lambda f: f(z8, zi.astype(np.longlong))),
    ]
    for lab, call in special:
        want, e1 = steplines.oracle(_err(lambda call=call: call(ndimage.watershed_ift)))
        got, e2 = _err(
            lambda call=call: call(
                lambda *a, **k: lnd.watershed_ift(*a, upstream_defects="reproduce", **k)
            )
        )
        if e1 or e2:
            t.add("watershed: the same error", e1 == e2, f"{lab}: {e1!r} / {e2!r}")
            continue
        t.add(
            "watershed: reproduce = SciPy's labels and type (edge cases)",
            want.dtype == got.dtype and want.shape == got.shape and want.tobytes() == got.tobytes(),
            lab,
        )


# --- center_of_mass --------------------------------------------------------------------------


def sec_com(t: cw.Tally) -> None:
    from scipy import ndimage

    from larmorx import ndimage as lnd

    rng = np.random.default_rng(7)
    g = (rng.random((23, 19, 17)) * 1000).astype(np.float32)
    lab3 = (rng.integers(0, 4, g.shape)).astype(np.int32)
    big = (rng.random((61, 67, 43)) * 50 - 5).astype(np.float32)  # pairwise blocks and chunks
    arrays = [
        ("float32 C", g),
        ("float32 F", np.asfortranarray(g)),
        ("float32 strided", g[::2, :, 1::3]),
        ("float32 large", big),
        ("float64 C", g.astype(np.float64) / 7),
        ("float64 F", np.asfortranarray(g.astype(np.float64) / 7)),
        ("int16", (g - 500).astype(np.int16)),
        ("uint8", (g / 4).astype(np.uint8)),
        ("bool", g > 500),
        ("1-D", g[:, 0, 0]),
        ("zeros (NaN)", np.zeros((4, 5), np.float32)),
        ("signed masses (inf)", np.array([-1.0, 1.0])),
    ]
    ver = np.__version__
    for name, a in arrays:
        lab_a = lab3[tuple(slice(0, s) for s in a.shape)] if a.ndim == 3 else None
        calls = [("no labels", {})]
        if lab_a is not None and lab_a.shape == a.shape:
            calls += [
                ("labels", {"labels": lab_a}),
                ("labels, index 2", {"labels": lab_a, "index": 2}),
                ("labels, index [0, 1, 3, 9]", {"labels": lab_a, "index": [0, 1, 3, 9]}),
                ("labels, index 9 (empty)", {"labels": lab_a, "index": 9}),
            ]
        for cname, kw in calls:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                want, e1 = steplines.oracle(
                    _err(lambda a=a, kw=kw: ndimage.center_of_mass(a, **kw))
                )
                got, e2 = _err(lambda a=a, kw=kw: lnd.center_of_mass(a, numpy_version=ver, **kw))
            if e1 or e2:
                t.add("com: the same error", e1 == e2, f"{name} {cname}: {e1!r} / {e2!r}")
                continue
            same = (
                type(want) is type(got)
                and np.asarray(want, dtype=np.float64).tobytes()
                == np.asarray(got, dtype=np.float64).tobytes()
                and [type(x) for x in np.ravel(np.asarray(want, dtype=object))]
                == [type(x) for x in np.ravel(np.asarray(got, dtype=object))]
            )
            t.add("com: SciPy's centres, bit for bit", same, f"{name} {cname}: {want} / {got}")


# --- epi_mask --------------------------------------------------------------------------------


def _save(path: Path, data: np.ndarray, aff: np.ndarray, slope: float | None = None) -> str:
    img = nb.Nifti1Image(data, aff)
    if slope is not None:
        img.header.set_slope_inter(slope, 3.0)
    img.to_filename(path)
    return str(path)


def real_cases() -> list[dict[str, Any]]:
    out = []
    for name in REAL_RUNS:
        run = cw.RUNS / name
        refs = sorted((run / "out").rglob("*_desc-hmc_boldref.nii.gz"))
        if refs:
            out.append({"label": name.split("-noreconall")[0], "in": str(refs[0])})
    return out


def synthetic_cases(root: Path) -> list[dict[str, Any]]:
    root.mkdir(parents=True, exist_ok=True)
    aff = np.diag([-3.0, 3.0, 3.5, 1.0])
    shape = (30, 34, 22)
    i, j, k = np.indices(shape)
    r2 = ((i - 15) / 11.0) ** 2 + ((j - 17) / 13.0) ** 2 + ((k - 11) / 8.0) ** 2
    head = np.where(r2 < 1, 600 + 40 * ((i * 7 + j * 3 + k * 5) % 13), 0) + 25 * (
        (i + 2 * j + k) % 3
    )
    edge = np.where(((i - 2) / 14.0) ** 2 + ((j - 17) / 20.0) ** 2 < 1, 800.0, 30.0) + (k % 4) * 9
    flat = np.full(shape, 7.0, np.float32)
    return [
        {"label": "scaled int16", "in": _save(root / "scaled.nii.gz", (head // 2).astype(np.int16), aff, 2.0)},
        {"label": "float64", "in": _save(root / "f64.nii.gz", head.astype(np.float64) / 3.0, aff)},
        {"label": "float32 touching the edges", "in": _save(root / "edge.nii.gz", edge.astype(np.float32), aff)},
        {"label": "one slice (length-1 axis)", "in": _save(root / "slice.nii.gz", head[:, :, 11:12].astype(np.float32), aff)},
        {"label": "error: 4-D", "in": _save(root / "four.nii.gz", np.stack([head, head], -1).astype(np.float32), aff)},
        {"label": "error: flat image", "in": _save(root / "flat.nii.gz", flat, aff)},
    ]  # fmt: skip


def sec_files(t: cw.Tally, scratch: Path, cases: list[dict[str, Any]], what: str) -> None:
    from sdcflows.utils.epimanip import epi_mask as sdc_epi_mask

    import larmorx.sdcflows as ls

    ver = np.__version__
    for c in cases:
        lab = c["label"]
        base = (
            scratch
            / what
            / lab.replace(" ", "_").replace(":", "").replace("(", "").replace(")", "")
        )
        if base.exists():
            shutil.rmtree(base)
        orig, mine = base / "sdcflows", base / "larmorx"
        orig.mkdir(parents=True)
        mine.mkdir(parents=True)
        cwd = os.getcwd()
        os.chdir(orig)
        try:
            t0 = time.perf_counter()
            ref_out, ref_err = steplines.oracle(_err(lambda: sdc_epi_mask(c["in"])))  # noqa: B023
            t_ref = time.perf_counter() - t0
        finally:
            os.chdir(cwd)
        results = {}
        for mode in ("reproduce", "fix"):
            os.chdir(mine)
            try:
                with warnings.catch_warnings(record=True) as caught:
                    warnings.simplefilter("always")
                    t0 = time.perf_counter()
                    got, err = _err(
                        lambda m=mode, c=c: ls.epi_mask(
                            c["in"],
                            None if m == "reproduce" else f"mask_{m}.nii.gz",
                            numpy_version=ver,
                            upstream_defects=m,
                        )
                    )
                    t_mine = time.perf_counter() - t0
            finally:
                os.chdir(cwd)
            warned = sorted({w.message.entry for w in caught if hasattr(w.message, "entry")})
            results[mode] = (got, err, warned, t_mine)
        got, err, warned, t_mine = results["reproduce"]
        if ref_err or err:
            t.add(f"{what}: the same error", ref_err == err, f"{lab}: {ref_err!r} / {err!r}")
            t.add(f"{what}: the same error (fix)", results["fix"][1] == ref_err, lab)
            print(f"INFO {lab}: {ref_err[:160]!r}")
            continue
        t.add(f"{what}: file name = sdcflows'", Path(got.out_file).name == Path(ref_out).name, lab)
        t.add(
            f"{what}: uncompressed bytes = sdcflows' (reproduce)",
            cd.raw(Path(ref_out)) == cd.raw(Path(got.out_file)),
            lab,
        )
        fgot, ferr, fwarned, _ = results["fix"]
        a = np.asanyarray(nb.load(ref_out).dataobj)
        b = np.asanyarray(nb.load(fgot.out_file).dataobj) if not ferr else None
        ndiff = -1 if b is None else int((a != b).sum())
        t.add(f"{what}: fix runs", not ferr, f"{lab}: {ferr!r}")
        t.add(
            f"{what}: fix warns exactly when the mask differs",
            bool(fwarned) == (ndiff > 0),
            f"{lab}: {fwarned} {ndiff}",
        )
        print(
            f"INFO {lab}: {a.shape}, {int(a.sum())} voxels; centre {got.centre}; fix: {ndiff} voxels "
            f"differ {fwarned}; reproduce warned {warned}; sdcflows {t_ref:.2f} s, larmorx {t_mine:.2f} s"
        )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="watershed,com,synthetic,real")
    ap.add_argument("--scratch", required=True)
    ap.add_argument("--cases", type=int, default=600)
    a = ap.parse_args()
    scratch = Path(a.scratch).absolute()
    scratch.mkdir(parents=True, exist_ok=True)
    import scipy

    print(f"numpy {np.__version__}, SciPy {scipy.__version__}")
    t = steplines.Noted(cw.Tally())
    sections = a.sections.split(",")
    if "watershed" in sections:
        sec_watershed(t, a.cases)
    if "com" in sections:
        sec_com(t)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", RuntimeWarning)
        if "synthetic" in sections:
            sec_files(t, scratch, synthetic_cases(scratch / "inputs"), "synthetic")
        if "real" in sections:
            sec_files(t, scratch, real_cases(), "real")
    bad = t.report()
    steplines.lines()
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
