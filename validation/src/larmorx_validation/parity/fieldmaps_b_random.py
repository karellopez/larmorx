# SPDX-License-Identifier: Apache-2.0
"""Random testing's cases of the field maps' part B (``difftest/tools/fieldmaps_b.py``) and
their permanent cases, for the suites ``ndimage-skimage``, ``niworkflows-helpers`` and
``sdcflows-estimation`` (whose script cases ``parity.sdcflows_scripts`` runs).

One drawn call per case: the arrays and files are built from the recipe, the original runs in
this process (SciPy, numpy, scikit-image, niworkflows, nipype, sdcflows, pybids) and larmorx
with ``upstream_defects="reproduce"``; one check compares every output bit for bit (arrays by
type, shape and bytes, numbers by their bits, files by name and decompressed bytes, command
lines character for character, errors by type and message). Then the D13 fix pass: larmorx's
default against ``reproduce``, a defect named exactly when they differ
(:func:`larmorx_validation.parity.defects.add_fix_check`).

The comparisons are the investigation's (``validation/investigations/sdcflows-fieldmap-b``),
whose helpers are imported where they hold the originals' rules: ``check_ndimage``'s value
generator and footprints, ``check_scipy_inv``'s comparison, ``relay11/estimators_dump``'s
serializer of the layouts and estimators. Calls whose original runs numpy's float64 ``exp``
(the Gaussian kernels, ``RegridToZooms``, ``random_walker`` in float64) are compared only
where numpy calls the C library's ``exp`` (no AVX-512 dispatch: ``NPY_DISABLE_CPU_FEATURES``),
as ``check_regrid.py`` and ``check_skimage.py`` compare them; elsewhere they are skipped.
"""

from __future__ import annotations

import contextlib
import contextvars
import dataclasses
import functools
import gzip
import importlib.util
import inspect
import math
import os
import re
import struct
import sys
import tempfile
import warnings
from collections.abc import Callable
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity.harness import SKIPPED, Case, CheckList, Outcome

SCRIPTS = Path(__file__).resolve().parents[3] / "investigations" / "sdcflows-fieldmap-b"
_IDS = re.compile(r"\b((?:SDC|PIPE|SCI|SKI|NP|NIB)-\d\d)\b")


# --------------------------------------------------------------------------------------------
# helpers


@functools.cache
def _load(name: str):
    """One of the investigation's scripts as a module (its folder on ``sys.path``)."""
    for p in (SCRIPTS, SCRIPTS / "relay11"):
        if str(p) not in sys.path:
            sys.path.insert(0, str(p))
    path = SCRIPTS / f"{name}.py"
    if not path.exists():
        path = SCRIPTS / "relay11" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(f"_lx_fmapb_{name}", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@functools.cache
def _exp64_is_libm() -> bool:
    """Whether numpy's float64 ``exp`` is the C library's here (``check_skimage.exp64_is_libm``)."""
    xs = np.random.default_rng(7).uniform(-700, 700, 4000)
    return all(float(v) == math.exp(float(x)) for v, x in zip(np.exp(xs), xs, strict=True))


def _need_libm_exp(what: str) -> None:
    if not _exp64_is_libm():
        raise Outcome(
            SKIPPED,
            f"{what}: numpy's float64 exp is not the C library's here (AVX-512 dispatch); "
            "compared with NPY_DISABLE_CPU_FEATURES set, as check_regrid.py",
        )


def _need_kernels() -> None:
    from larmorx_validation.parity.numpy_linalg import KERNELS, openblas_core

    core = openblas_core()
    if core not in KERNELS:
        raise Outcome(SKIPPED, f"numpy's OpenBLAS runs the {core} kernels (Haswell's compared)")


class _Err:
    """An exception as a result: its type and message."""

    def __init__(self, exc: BaseException) -> None:
        self.kind = type(exc).__name__
        self.text = str(exc)

    def __repr__(self) -> str:
        return f"{self.kind}: {self.text}"


def _files(paths, root: Path) -> tuple:
    """Files as (name below ``root``, decompressed bytes)."""
    out = []
    for p in paths:
        p = Path(p)
        name = os.path.relpath(p, root) if p.is_absolute() else str(p)
        if not p.exists():
            out.append((name, None))
            continue
        data = p.read_bytes()
        if p.name.endswith(".gz"):
            data = gzip.decompress(data)
        out.append((name, data))
    return tuple(out)


def _folder(root: Path) -> tuple:
    """Every file below ``root`` (names and decompressed bytes)."""
    if not root.exists():
        return ()
    return _files(sorted(p for p in root.rglob("*") if p.is_file()), root)


def _key(v: Any) -> Any:
    """A comparable form of a result: arrays by type, shape and bytes, floats by their bits."""
    if isinstance(v, _Err):
        return ("error", v.kind, v.text)
    if isinstance(v, np.ndarray):
        return ("array", v.dtype.str, v.shape, np.ascontiguousarray(v).tobytes())
    if isinstance(v, np.generic):
        return ("scalar", v.dtype.str, v.tobytes())
    if isinstance(v, bool | int | str | bytes) or v is None:
        return (type(v).__name__, v)
    if isinstance(v, float):
        return ("float", struct.pack("<d", v))
    if isinstance(v, os.PathLike):
        return ("path", os.fspath(v))
    if isinstance(v, dict):
        return (
            "dict",
            tuple((str(k), _key(x)) for k, x in sorted(v.items(), key=lambda i: str(i[0]))),
        )
    if isinstance(v, list | tuple):
        return (type(v).__name__, tuple(_key(x) for x in v))
    if dataclasses.is_dataclass(v):
        return (
            "dataclass",
            type(v).__name__,
            tuple((f.name, _key(getattr(v, f.name))) for f in dataclasses.fields(v)),
        )
    return ("other", type(v).__name__, repr(v))


def _arrays(v: Any) -> list[np.ndarray]:
    if isinstance(v, np.ndarray):
        return [v]
    if isinstance(v, np.generic):
        return [np.asarray(v)]
    if isinstance(v, dict):
        return [a for x in v.values() for a in _arrays(x)]
    if isinstance(v, list | tuple):
        return [a for x in v for a in _arrays(x)]
    return []


def _strip(v: Any) -> Any:
    """The result without its arrays (the fix pass counts the arrays' changes apart)."""
    if isinstance(v, np.ndarray | np.generic):
        return "<array>"
    if isinstance(v, dict):
        return {k: _strip(x) for k, x in v.items()}
    if isinstance(v, list | tuple):
        return type(v)(_strip(x) for x in v)
    return v


def _describe(a: Any, b: Any) -> str:
    """The first difference of two results, in a line."""
    if isinstance(a, _Err) or isinstance(b, _Err):
        return f"original {a!r:.200} / larmorx {b!r:.200}"
    if isinstance(a, np.ndarray) and isinstance(b, np.ndarray):
        if a.dtype != b.dtype or a.shape != b.shape:
            return f"original {a.dtype}{a.shape}, larmorx {b.dtype}{b.shape}"
        size = max(a.dtype.itemsize, 1)
        x = np.ascontiguousarray(a).view(np.uint8).reshape(-1, size)
        y = np.ascontiguousarray(b).view(np.uint8).reshape(-1, size)
        bad = np.flatnonzero((x != y).any(axis=1))
        k = int(bad[0]) if bad.size else 0
        fa, fb = np.ascontiguousarray(a).reshape(-1), np.ascontiguousarray(b).reshape(-1)
        return f"{bad.size} of {fa.size} elements differ (first at {k}: {fa[k]!r} / {fb[k]!r})"
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b), key=str):
            if _key(a.get(k)) != _key(b.get(k)):
                return f"{k}: {_describe(a.get(k), b.get(k))}"
    if isinstance(a, list | tuple) and isinstance(b, list | tuple):
        if len(a) != len(b):
            return f"{len(a)} outputs / {len(b)}: {str(a)[:150]} / {str(b)[:150]}"
        for i, (x, y) in enumerate(zip(a, b, strict=True)):
            if _key(x) != _key(y):
                return f"[{i}] {_describe(x, y)}"
    if isinstance(a, bytes) and isinstance(b, bytes):
        k = next((i for i in range(min(len(a), len(b))) if a[i] != b[i]), min(len(a), len(b)))
        return f"bytes differ at {k} ({len(a)} / {len(b)} bytes)"
    return f"original {a!r:.200} / larmorx {b!r:.200}"


def _run(fn: Callable[[], Any]) -> tuple[Any, list]:
    """The result (or the error) of ``fn`` and the warnings it gave."""
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        try:
            value = fn()
        except Exception as exc:  # the error is the result
            value = _Err(exc)
    return value, list(w)


@contextlib.contextmanager
def _cwd(path: Path):
    path.mkdir(parents=True, exist_ok=True)
    old = os.getcwd()
    os.chdir(path)
    try:
        yield path
    finally:
        os.chdir(old)


def _versions(fn) -> dict:
    """The running releases for larmorx's version arguments, and one thread."""
    import scipy

    params = inspect.signature(fn).parameters
    kw: dict = {}
    if "numpy_version" in params:
        kw["numpy_version"] = np.__version__
    if "scipy_version" in params:
        kw["scipy_version"] = scipy.__version__
    return kw


def _switch(fn) -> bool:
    return "upstream_defects" in inspect.signature(fn).parameters


@dataclasses.dataclass
class _Call:
    """One drawn call: the original, larmorx in a mode, and the fix pass's rules."""

    original: Callable[[], Any]
    larmorx: Callable[[str], Any]
    switch: bool = True
    #: defects whose notice is about a condition the outputs may not show
    silent: tuple[str, ...] = ()
    #: warnings' texts compared too (a filter of the original's and larmorx's)
    warnings: Callable[[list, list], tuple[Any, Any]] | None = None


# --------------------------------------------------------------------------------------------
# arrays (ndimage-skimage)


def _layout(a: np.ndarray, layout: str) -> np.ndarray:
    if layout == "F":
        return np.asfortranarray(a)
    if layout == "strided" and a.ndim >= 2 and a.shape[0] >= 2:
        big = np.zeros((a.shape[0] * 2, *a.shape[1:]), a.dtype)
        big[::2] = a
        return big[::2]
    return np.ascontiguousarray(a)


def _build(spec: dict, *, binary: bool = False) -> np.ndarray:
    cn = _load("check_ndimage")
    rng = np.random.default_rng(spec["seed"])
    shape = tuple(spec["shape"])
    dt = np.dtype(spec["dtype"])
    if binary:
        a = (rng.random(shape) < 0.55).astype(dt)
        if dt.kind == "f":
            a = (a * dt.type(rng.choice([1.0, 0.5, 2.0]))).astype(dt)
    elif dt.kind == "c":
        re_ = cn.values(rng, shape, np.float64, spec["special"])
        im = cn.values(rng, shape, np.float64, False)
        a = (re_ + 1j * im).astype(dt)
    else:
        a = cn.values(rng, shape, dt.type, spec["special"])
    if spec.get("scale") and dt.kind == "f":
        a = (a * spec["scale"]).astype(dt)
    if spec.get("sparse"):
        a = np.where(rng.random(shape) < 0.5, np.zeros((), dt), a).astype(dt)
    if spec.get("empty"):
        a = a[:0]
    return _layout(a, spec.get("layout", "C"))


def _footprint(f: dict) -> np.ndarray:
    cn = _load("check_ndimage")
    return cn.random_footprint(np.random.default_rng(f["seed"]), tuple(f["shape"]))


def _out(name: str | None):
    return None if name is None else np.dtype(name).type


def _nd(fn_name: str):
    import scipy.ndimage as sp

    import larmorx.ndimage as lnd

    return getattr(sp, fn_name), getattr(lnd, fn_name)


def _with_inputs(fn, args: tuple, kw: dict):
    """The call's result and its array arguments afterwards (an original that writes into its
    caller's arrays: SCI-31). Each side builds its own arguments (:func:`run_case`)."""
    arrays = [x for x in (*args, *kw.values()) if isinstance(x, np.ndarray)]

    def run():
        result = fn()
        return {"result": result, "arguments afterwards": [a.copy() for a in arrays]}

    return run


def _plain(fsp, flx, args: tuple, kw: dict, lx_kw: dict | None = None) -> _Call:
    lx_kw = {**_versions(flx), **(lx_kw or {})}
    sw = _switch(flx)
    return _Call(
        _with_inputs(lambda: fsp(*args, **kw), args, kw),
        lambda mode: _with_inputs(
            lambda: flx(*args, **kw, **lx_kw, **({"upstream_defects": mode} if sw else {})),
            args,
            kw,
        )(),
        switch=sw,
    )


def _c_gaussian(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    _need_libm_exp("the Gaussian kernel")
    a = _build(e["array"])
    kw = dict(e["kw"])
    kw["output"] = _out(kw["output"])
    fsp, flx = _nd(e["fn"])
    sigma = kw.pop("sigma")
    return _plain(fsp, flx, (a, sigma), kw, {"n_threads": nt})


def _c_correlate1d(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    a = _build(e["array"])
    w = np.random.default_rng(e["weights_seed"]).integers(-8, 9, e["n_weights"]) * 0.25
    kw = dict(e["kw"])
    kw["output"] = _out(kw["output"])
    fsp, flx = _nd("correlate1d")
    return _plain(fsp, flx, (a, w), kw, {"n_threads": nt})


def _c_rank(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    a = _build(e["array"])
    kw = dict(e["kw"])
    fn = e["fn"]
    if "footprint" in kw:
        fp = _footprint(kw["footprint"])
        kw["footprint"] = fp
        n = int(fp.sum())
    else:
        n = e.get("window") or 1
    if "output" in kw:
        kw["output"] = _out(kw["output"])
    args: tuple = (a,)
    if fn.endswith("1d"):
        args = (a, kw.pop("size"))
    if fn == "rank_filter":
        r = kw.pop("rank")
        rank = r["draw"] % n
        args = (a, rank - n if r["negative"] else rank)
    if fn == "percentile_filter":
        args = (a, kw.pop("percentile"))
    if a.ndim == 1 and not fn.endswith("1d"):
        cn = _load("check_ndimage")
        win = n if "footprint" not in kw else kw["footprint"].size
        org = kw.get("origin", 0)
        org = org[0] if isinstance(org, list) else org
        if cn.reads_outside(a.shape[0], win, kw["mode"], org):
            raise Outcome(SKIPPED, "SciPy's 1-D rank filter reads outside the input here (SCI-08)")
    fsp, flx = _nd(fn)
    return _plain(fsp, flx, args, kw, {"n_threads": nt})


def _c_grey(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    a = _build(e["array"])
    kw = dict(e["kw"])
    if "footprint" in kw:
        kw["footprint"] = _footprint(kw["footprint"])
    if "structure" in kw:
        s = kw["structure"]
        kw["structure"] = np.random.default_rng(s["seed"]).integers(-3, 4, tuple(s["shape"])) * 0.5
    fsp, flx = _nd(e["fn"])
    return _plain(fsp, flx, (a,), kw, {"n_threads": nt})


def _structure(s: dict | None, nd: int):
    import scipy.ndimage as sp

    if s is None:
        return None
    if "connectivity" in s:
        return sp.generate_binary_structure(nd, s["connectivity"])
    st = _footprint(s)
    st[tuple(k // 2 for k in st.shape)] = True
    return st


def _c_binary(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    a = _build(e["array"], binary=True)
    nd = a.ndim
    kw = dict(e["kw"])
    fn = e["fn"]
    st = _structure(kw.pop("structure", None), nd)
    if fn == "binary_hit_or_miss":
        s2 = kw.pop("structure2")
        s1 = st
        if s2 == "complement":
            base = s1 if s1 is not None else _structure({"connectivity": 1}, nd)
            s2v = ~np.asarray(base, bool)
        else:
            s2v = None
        kw.update(structure1=s1, structure2=s2v)
    else:
        kw["structure"] = st
    if "mask" in kw:
        m = kw["mask"]
        rng = np.random.default_rng(m["seed"])
        truth = rng.random(a.shape) < 0.7
        dt = np.dtype(m["dtype"])
        if dt == np.float32:
            mask = np.where(truth, np.float32(rng.choice([1.0, 0.5])), np.float32(0))
        elif dt == np.uint16:
            mask = np.where(truth, np.uint16(rng.choice([1, 256])), np.uint16(0))
        elif dt == np.int8:
            mask = np.where(truth, np.int8(rng.choice([1, -1])), np.int8(0))
        else:
            mask = truth.astype(dt)
        kw["mask"] = mask.astype(dt)
    if "output" in kw:
        kw["output"] = _out(kw["output"])
    fsp, flx = _nd(fn)
    return _plain(fsp, flx, (a,), kw, {"n_threads": nt})


def _c_label(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    a = _build(e["array"])
    kw = dict(e["kw"])
    if "structure" in kw:
        kw["structure"] = _structure(kw["structure"], a.ndim)
    out = kw.pop("output", None)
    fsp, flx = _nd("label")
    if out is not None and out.startswith("array "):
        dt = np.dtype(out.split(" ", 1)[1])

        def run(f, extra: dict):
            o = np.zeros(a.shape, dt)
            n = f(a, output=o, **kw, **extra)
            return (n, o)

        sw = _switch(flx)
        return _Call(
            lambda: run(fsp, {}),
            lambda mode: run(flx, {"upstream_defects": mode} if sw else {}),
            switch=sw,
        )
    if out is not None:
        kw["output"] = np.dtype(out) if out.startswith(">") else np.dtype(out).type
    return _plain(fsp, flx, (a,), kw)


def _c_measure(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    a = _build(e["array"])
    labels = None
    k = 1
    if (lab := e["labels"]) is not None:
        rng = np.random.default_rng(lab["seed"])
        k = lab["k"]
        dt = np.dtype(lab["dtype"])
        labels = (
            (rng.random(a.shape) < 0.5)
            if dt == np.bool_
            else rng.integers(0, k + 1, a.shape).astype(dt)
        )
    rng = np.random.default_rng(e["index"]["seed"])
    form = e["index"]["form"]
    index: Any = None
    if form == "scalar":
        index = int(rng.integers(0, k + 1))
    elif form == "list":
        index = sorted({int(x) for x in rng.integers(0, k + 2, int(rng.integers(1, 4)))})
    elif form == "array":
        index = np.array(sorted({int(x) for x in rng.integers(0, k + 2, 3)}), np.int64)
    elif form == "0-d array":
        index = np.array(1)
    elif form == "missing":
        index = [k + 3, 1]
    fsp, flx = _nd(e["fn"])
    return _plain(fsp, flx, (a, labels, index), {})


def _c_watershed(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    import scipy.ndimage as sp

    spec = dict(e["array"])
    rng0 = np.random.default_rng(spec["seed"])
    top = 256 if spec["dtype"] == "uint8" else 4000
    a = _layout(rng0.integers(0, top, tuple(spec["shape"])).astype(spec["dtype"]), spec["layout"])
    m = e["markers"]
    rng = np.random.default_rng(m["seed"])
    big = m["dtype"] == "int64 big"
    dt = np.dtype("int64" if big else m["dtype"])
    markers = np.zeros(a.shape, dt)
    flat = markers.reshape(-1)
    pos = rng.choice(flat.size, size=min(m["n"], flat.size), replace=False)
    for i, p in enumerate(pos):
        v = i + 1
        if m["negative"] and dt.kind == "i" and i % 2:
            v = -v
        flat[p] = v + (2**32 if big else 0)
    st = None if e["structure"] is None else sp.generate_binary_structure(a.ndim, e["structure"])
    fsp, flx = _nd("watershed_ift")
    return _plain(fsp, flx, (a, markers), {"structure": st})


def _rw_labels(kind: str, shape: tuple, seed: int) -> np.ndarray:
    rng = np.random.default_rng(seed)
    lab = np.zeros(shape, np.int32)
    flat = lab.reshape(-1)
    n = max(2, flat.size // 20)
    idx = rng.choice(flat.size, size=n, replace=False)
    values = {"two": (1, 2), "three": (1, 2, 3), "non-consecutive": (3, 7)}.get(kind, (1, 2))
    for i, p in enumerate(idx):
        flat[p] = values[i % len(values)]
    if kind == "masked":
        free = np.flatnonzero(flat == 0)
        flat[rng.choice(free, size=max(1, free.size // 6), replace=False)] = -1
    elif kind == "isolated":
        # an unlabeled pixel whose neighbours are all masked
        c = tuple(s // 2 for s in shape)
        sl = tuple(slice(max(0, x - 1), x + 2) for x in c)
        lab[sl] = -1
        lab[c] = 0
    elif kind == "no zeros":
        lab[...] = 1
    return lab


def _c_skimage(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    import larmorx.skimage as lsk

    fn = e["fn"]
    if fn == "ball":
        from skimage.morphology import ball

        kw = dict(e["kw"])
        kw["dtype"] = np.dtype(kw["dtype"]).type
        r = kw.pop("radius")
        return _Call(lambda: ball(r, **kw), lambda mode: lsk.ball(r, **kw), switch=False)
    if fn == "threshold_otsu":
        from skimage.filters import threshold_otsu

        kw = dict(e["kw"])
        if "hist" in e:
            h = e["hist"]
            rng = np.random.default_rng(h["seed"])
            counts = rng.integers(0, 50, h["n"])
            centers = {
                "none": None,
                "float32": (np.arange(h["n"]) * 0.5 + 0.25).astype(np.float32),
                "int64": np.arange(h["n"], dtype=np.int64) * 10,
                "float64": np.arange(h["n"]) * 0.125 - 1.0,
            }[h["centers"]]
            kw["hist"] = counts if centers is None else (counts, centers)

            def ref():
                return threshold_otsu(**kw)

            def mine(mode):
                return lsk.threshold_otsu(**kw)
        else:
            img = _build(e["array"])

            def ref():
                return threshold_otsu(img, **kw)

            def mine(mode):
                return lsk.threshold_otsu(img, **kw)

        def typed(f):
            def g(*x):
                v = f(*x)
                return (type(v).__name__, v)

            return g

        return _Call(typed(ref), typed(mine), switch=False)
    if fn == "random_walker":
        from skimage.segmentation import random_walker

        data = _build(e["array"])
        labels = _rw_labels(e["labels"]["kind"], data.shape, e["labels"]["seed"])
        kw = dict(e["kw"])
        if "spacing" in kw:
            s = kw["spacing"]
            kw["spacing"] = (
                np.asarray(s["values"], np.float32)
                if s["kind"] == "float32"
                else tuple(s["values"])
            )
        sp = kw.get("spacing")
        f64 = (
            data.dtype not in (np.float32, np.float16)
            or sp is None
            or np.asarray(sp).dtype != np.float32
        )
        if f64:
            _need_libm_exp("random_walker in float64")
        lx_kw = {**_versions(lsk.random_walker), "n_threads": nt}

        def keep(ws: list) -> list[str]:
            return [
                str(w.message)
                for w in ws
                if any(
                    k in str(w.message)
                    for k in (
                        "pyamg",
                        "UMFPACK",
                        "Conjugate",
                        "probability",
                        "Random walker",
                        "isolated",
                    )
                )
                and not _IDS.search(str(w.message))
            ]

        return _Call(
            lambda: random_walker(data, labels.copy(), **kw),
            lambda mode: lsk.random_walker(
                data, labels.copy(), **kw, **lx_kw, upstream_defects=mode
            ),
            warnings=lambda w1, w2: (keep(w1), keep(w2)),
        )
    from skimage.exposure import match_histograms

    img, ref = _build(e["array"]), _build(e["reference"])
    return _Call(
        lambda: match_histograms(img, ref),
        lambda mode: lsk.match_histograms(img, ref, upstream_defects=mode),
    )


def _c_numpy(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from larmorx import _core

    fn = e["fn"]
    spec = dict(e["array"])
    rng = np.random.default_rng(spec["seed"])
    if fn == "median axis":
        a = _build(spec)
        ax = e["axis"]
        return _Call(
            lambda: np.median(a, axis=ax), lambda mode: _core.numpy_median(a, ax), switch=False
        )
    dt = np.dtype(spec["dtype"])
    if fn == "exp":
        x = (rng.integers(-440, 361, spec["shape"]) * 0.25).astype(np.float32)
        if spec["special"] and x.size > 4:
            x[:4] = [np.nan, -0.0, np.inf, -np.inf]

        def ref():
            with np.errstate(all="ignore"):
                return np.exp(x)

        return _Call(ref, lambda mode: _core.numpy_exp_f32(x), switch=False)
    a = (rng.integers(-400, 401, spec["shape"]) * spec["scale"]).astype(dt)
    if spec["special"] and a.size > 3:
        a[int(rng.integers(0, a.size))] = rng.choice([np.nan, np.inf, -np.inf, -0.0])
    rel = ".".join(np.__version__.split(".")[:2])
    if fn == "percentile":
        q = e["q"]

        def ref():
            try:
                return np.percentile(a, q)
            except Exception as exc:
                return ("error", type(exc).__name__)

        def mine(mode):
            try:
                return np.asarray(_core.numpy_percentile_weak(a, q, rel), dtype=a.dtype)
            except Exception as exc:
                return ("error", type(exc).__name__)

        # numpy's 0-d result and larmorx's, compared as arrays (check_numpy)
        return _Call(lambda: _as0d(ref()), lambda mode: _as0d(mine(mode)), switch=False)
    if fn == "percentile list":
        ql = [0.0, 15.0, 50.0, 99.98, 100.0]
        return _Call(
            lambda: np.percentile(a, ql),
            lambda mode: np.asarray(_core.numpy_percentile(a, ql)),
            switch=False,
        )
    if fn == "median":
        return _Call(
            lambda: np.asarray(np.median(a)),
            lambda mode: np.asarray(_core.numpy_median(a)),
            switch=False,
        )
    lo, hi = (np.float32(v) for v in e["bounds"])
    return _Call(
        lambda: np.clip(a, lo, hi), lambda mode: _core.numpy_clip_f32(a, lo, hi), switch=False
    )


def _as0d(v):
    return np.asarray(v) if not isinstance(v, tuple) else v


def _c_inv(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from larmorx_validation.difftest import gen

    _need_kernels()
    rng = np.random.default_rng(e["seed"])
    fam = e["family"]
    ints = rng.integers(-5, 6, (4, 4)).astype(np.float64)
    if fam == "affine":
        m = np.eye(4)
        r = gen.rotation(e["quaternion"])
        if e["oblique"]:
            q = [int(x) for x in rng.integers(-3, 4, 4)]
            q[0] = 10
            r = gen.rotation(q) @ r
        m[:3, :3] = r @ np.diag(e["zooms"])
        m[:3, 3] = rng.integers(-600, 601, 3) * 0.25
    elif fam == "integers":
        m = ints
    elif fam == "symmetric":
        m = ints + ints.T
    elif fam == "diagonal":
        m = np.diag(rng.integers(1, 9, 4) * 0.5 * rng.choice([-1.0, 1.0], 4))
    elif fam == "triangular":
        m = np.triu(ints) if rng.random() < 0.5 else np.tril(ints)
        np.fill_diagonal(m, rng.integers(1, 6, 4) * 1.0)
    elif fam == "singular":
        m = ints.copy()
        m[3] = m[0] + m[1]
    elif fam == "ill-conditioned":
        m = np.diag([1.0, 1.0, 1.0, 1e-17]) @ (ints + 6 * np.eye(4))
    else:
        with np.errstate(over="ignore"):
            m = ints * 2.0 ** e["exponent"]
    m = np.ascontiguousarray(m)
    ci = _load("check_scipy_inv")
    from scipy import linalg

    def norm(o):
        kind, value, msgs = o
        if kind == "refused":
            return ("refused",)
        return (
            kind,
            value if kind == "ok" else str(value),
            [x.replace("LinAlgWarning", "W") for x in msgs],
        )

    return _Call(
        lambda: norm(ci.outcome(linalg.inv, m)),
        lambda mode: norm(ci.outcome(lambda x: ci.ours(x, mode), m)),
    )


# --------------------------------------------------------------------------------------------
# files (niworkflows-helpers, sdcflows-estimation)


def _ni_clip(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from niworkflows.interfaces.nibabel import _advanced_clip

    import larmorx.niworkflows as lnw

    src = files["in"]
    kw = e["kw"]

    def ref():
        d = tmp / "a"
        d.mkdir(exist_ok=True)
        return _files([_advanced_clip(src, newpath=str(d), **kw)], d)

    def mine(mode):
        d = tmp / f"b-{mode}"
        r = lnw.intensity_clip(
            src,
            **kw,
            out_dir=d,
            **_versions(lnw.intensity_clip),
            upstream_defects=mode,
            n_threads=nt,
        )
        return _files([r.out_file], d)

    return _Call(ref, mine)


def _ni_ravg(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from niworkflows.interfaces.images import RobustAverage

    import larmorx.niworkflows as lnw

    src = files["in"]
    kw = dict(e["kw"])

    def ref():
        d = tmp / "a"
        d.mkdir(exist_ok=True)
        o = RobustAverage(in_file=src, mc_method=None, **kw).run(cwd=str(d)).outputs
        drift = o.out_drift if isinstance(o.out_drift, list) else []
        return {
            "out_file": Path(o.out_file).name,
            "drift": [float(x) for x in drift],
            "files": _folder(d),
        }

    def mine(mode):
        d = tmp / f"b-{mode}"
        r = lnw.robust_average(
            src,
            mc_method=None,
            **kw,
            out_dir=d,
            **_versions(lnw.robust_average),
            upstream_defects=mode,
            n_threads=nt,
        )
        return {
            "out_file": Path(r.out_file).name,
            "drift": [float(x) for x in (r.out_drift or [])],
            "files": _folder(d),
        }

    return _Call(ref, mine)


def _in_files(files: dict) -> list[str]:
    return [files[k] for k in sorted(files) if k.startswith("in")]


def _ni_merge(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from niworkflows.interfaces.nibabel import MergeSeries

    import larmorx.niworkflows as lnw

    srcs = _in_files(files)
    kw = dict(e["kw"])

    def ref():
        d = tmp / "a"
        d.mkdir(exist_ok=True)
        o = MergeSeries(in_files=srcs, **kw).run(cwd=str(d)).outputs
        return {"out_file": Path(o.out_file).name, "files": _folder(d)}

    def mine(mode):
        d = tmp / f"b-{mode}"
        out = lnw.merge_series(srcs, **kw, out_dir=d, n_threads=nt)
        return {"out_file": Path(out).name if out else None, "files": _folder(d)}

    return _Call(ref, mine, switch=False)


def _ni_imm(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from niworkflows.interfaces.images import IntraModalMerge

    import larmorx.niworkflows as lnw

    srcs = _in_files(files)
    kw = dict(e["kw"])
    mask = files.get("mask")

    def ref():
        d = tmp / "a"
        d.mkdir(exist_ok=True)
        extra = {"in_mask": mask} if mask else {}
        o = IntraModalMerge(in_files=srcs, **kw, **extra).run(cwd=str(d)).outputs
        return {
            "out_file": Path(o.out_file).name,
            "out_avg": Path(o.out_avg).name,
            "files": _folder(d),
        }

    def mine(mode):
        d = tmp / f"b-{mode}"
        r = lnw.intra_modal_merge(
            srcs,
            mask,
            **kw,
            out_dir=d,
            **_versions(lnw.intra_modal_merge),
            upstream_defects=mode,
            n_threads=nt,
        )
        return {
            "out_file": Path(r.out_file).name,
            "out_avg": Path(r.out_avg).name,
            "files": _folder(d),
        }

    return _Call(ref, mine)


def _ni_regrid(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from niworkflows.interfaces.images import RegridToZooms

    import larmorx.niworkflows as lnw

    _need_libm_exp("RegridToZooms' smoothing and spline kernels")
    src = files["in"]
    kw = dict(e["kw"])
    zooms = tuple(e["zooms"])

    def ref():
        with _cwd(tmp / "a") as d:
            o = RegridToZooms(in_file=src, zooms=zooms, **kw).run().outputs
            return {"out_file": Path(o.out_file).name, "files": _folder(d)}

    def mine(mode):
        d = tmp / f"b-{mode}"
        r = lnw.regrid_to_zooms(
            src, zooms, **kw, out_dir=d, **_versions(lnw.regrid_to_zooms), n_threads=nt
        )
        return {"out_file": Path(r.out_file).name, "files": _folder(d)}

    return _Call(ref, mine, switch=False)


#: nipype's interfaces of the command lines larmorx formats (check_nipype.py's).
NIPYPE = {
    "N4BiasFieldCorrection": (
        "nipype.interfaces.ants.N4BiasFieldCorrection",
        "n4_bias_field_correction",
    ),
    "DenoiseImage": ("nipype.interfaces.ants.DenoiseImage", "denoise_image"),
    "ImageMath": ("nipype.interfaces.ants.ImageMath", "image_math"),
    "antsApplyTransforms": ("nipype.interfaces.ants.ApplyTransforms", "apply_transforms"),
    "bet": ("nipype.interfaces.fsl.BET", "bet"),
    "fslmaths": ("nipype.interfaces.fsl.BinaryMaths", "binary_maths"),
    "3dAutomask": ("nipype.interfaces.afni.Automask", "automask"),
    "3dvolreg": ("nipype.interfaces.afni.Volreg", "volreg"),
}


def _nipype_inputs(prog: str, rng: np.random.Generator, f: dict[str, str]) -> dict:
    """A random valid set of the interface's inputs (the values of check_nipype's cases)."""

    def pick(options):
        return options[int(rng.integers(0, len(options)))]

    def maybe(p=0.5):
        return bool(rng.random() < p)

    kw: dict = {}
    if prog == "N4BiasFieldCorrection":
        kw["input_image"] = f["in.nii.gz"]
        if maybe(0.3):
            kw["dimension"] = pick([3, 2, 4])
        if maybe():
            kw["mask_image"] = f["mask.nii.gz"]
        if maybe(0.2):
            kw["weight_image"] = f["w.nii.gz"]
        if maybe():
            kw["save_bias"] = pick([True, False])
        if maybe(0.2):
            kw["bias_image"] = "bias.nii.gz"
        if maybe():
            kw["bspline_fitting_distance"] = pick([200, 300, 150.5])
            if maybe(0.3):
                kw["bspline_order"] = pick([3, 5])
        if maybe():
            kw["n_iterations"] = pick([[50] * 4, [50, 40], [20]])
            if maybe():
                kw["convergence_threshold"] = pick([1e-7, 1e-6, 0.0])
        if maybe():
            kw["shrink_factor"] = pick([2, 4, 1])
        if maybe(0.3):
            kw["histogram_sharpening"] = pick([(0.2, 0.01, 100), (0.15, 0.01, 200)])
        for k in ("rescale_intensities", "copy_header"):
            if maybe(0.3):
                kw[k] = pick([True, False])
        if maybe(0.3):
            kw["output_image"] = "corrected.nii.gz"
    elif prog == "DenoiseImage":
        kw["input_image"] = f["in.nii.gz"]
        if maybe():
            kw["dimension"] = pick([3, 2, 4])
        if maybe():
            kw["noise_model"] = pick(["Rician", "Gaussian"])
        if maybe():
            kw["shrink_factor"] = pick([1, 2])
        if maybe():
            kw["save_noise"] = pick([True, False])
        if maybe(0.3):
            kw["output_image"] = "den.nii.gz"
        if maybe(0.3):
            kw["verbose"] = True
    elif prog == "ImageMath":
        kw["op1"] = f["in.nii.gz"]
        op = pick(
            [
                "PadImage",
                "MD",
                "ME",
                "Laplacian",
                "Normalize",
                "TruncateImageIntensity",
                "GetLargestComponent",
                "FillHoles",
                "m",
                "+",
                "Byte",
            ]
        )
        kw["operation"] = op
        if op in ("PadImage", "MD", "ME"):
            kw["op2"] = pick(["5", "2", "-3"])
        elif op in ("m", "+"):
            kw["op2"] = pick([f["ref.nii.gz"], "2"])
        elif op == "Laplacian":
            kw["op2"] = pick(["1.5 1", "1"])
        elif op == "TruncateImageIntensity":
            kw["op2"] = "0.01 0.999 256"
        if maybe(0.3):
            kw["dimension"] = pick([3, 2, 4])
        if maybe(0.3):
            kw["copy_header"] = pick([True, False])
        if maybe(0.3):
            kw["output_image"] = "math.nii.gz"
    elif prog == "antsApplyTransforms":
        kw["input_image"] = f["in.nii.gz"]
        kw["reference_image"] = f["ref.nii.gz"]
        n = int(rng.integers(1, 4))
        ts = [pick([f["t.mat"], f["w.nii.gz"], "identity"]) for _ in range(n)]
        kw["transforms"] = ts if n > 1 or maybe() else ts[0]
        if maybe(0.4) and isinstance(kw["transforms"], list):
            kw["invert_transform_flags"] = [maybe() for _ in ts]
        interp = pick(
            [
                "Linear",
                "NearestNeighbor",
                "LanczosWindowedSinc",
                "BSpline",
                "Gaussian",
                "MultiLabel",
                "GenericLabel",
            ]
        )
        kw["interpolation"] = interp
        if interp == "BSpline" and maybe():
            kw["interpolation_parameters"] = (pick([5, 3]),)
        if interp == "Gaussian" and maybe():
            kw["interpolation_parameters"] = (1.0, 1.0)
        if maybe(0.3):
            kw["default_value"] = pick([0, 0.5, -1])
        if maybe(0.3):
            kw["float"] = pick([True, False])
        if maybe(0.3):
            kw["dimension"] = pick([3, 2])
        if maybe(0.2):
            kw["input_image_type"] = pick([0, 1, 2, 3])
        if maybe(0.3):
            kw["output_image"] = "warped.nii.gz"
            # (nipype requires output_image for it: its requires checks are not ported, as
            # nothing larmorx runs reaches them: larmorx._nipype's documentation)
            if maybe(0.3):
                kw["print_out_composite_warp_file"] = True
    elif prog == "bet":
        kw["in_file"] = f["in.nii.gz"]
        if maybe():
            kw["frac"] = pick([0.3, 0.5, 0.65])
        if maybe(0.3):
            kw["vertical_gradient"] = pick([-0.1, 0.2])
        for k in ("mask", "outline", "skull", "mesh", "threshold", "no_output"):
            if maybe(0.3):
                kw[k] = True
        if maybe(0.2):
            kw["radius"] = 70
        if maybe(0.2):
            kw["center"] = [10, 20, 30]
        if maybe(0.4):
            kw[
                pick(["robust", "padding", "remove_eyes", "surfaces", "functional", "reduce_bias"])
            ] = True
        if maybe(0.3):
            kw["out_file"] = "brain.nii.gz"
    elif prog == "fslmaths":
        kw["in_file"] = f["in.nii.gz"]
        kw["operation"] = pick(["add", "sub", "mul", "div", "rem", "max", "min"])
        if maybe():
            kw["operand_value"] = pick([2.5, 0, -1, 1e-3])
        else:
            kw["operand_file"] = f["ref.nii.gz"]
        if maybe(0.3):
            kw["internal_datatype"] = pick(["float", "double", "int"])
        if maybe(0.3):
            kw["output_datatype"] = pick(["short", "float", "char", "input"])
        if maybe(0.2):
            kw["nan2zeros"] = True
        if maybe(0.3):
            kw["out_file"] = "maths.nii.gz"
    elif prog == "3dAutomask":
        kw["in_file"] = f["in.nii.gz"]
        if maybe():
            kw["outputtype"] = pick(["NIFTI_GZ", "NIFTI", "AFNI"])
        if maybe():
            kw["dilate"] = pick([1, 2])
        if maybe():
            kw["erode"] = pick([1, 2])
        if maybe():
            kw["clfrac"] = pick([0.4, 0.1])
        if maybe(0.3):
            kw["out_file"] = "mask_auto.nii.gz"
        if maybe(0.3):
            kw["brain_file"] = "brain_auto.nii.gz"
    else:  # 3dvolreg
        kw["in_file"] = f["in.nii.gz"]
        if maybe():
            kw["args"] = "-twopass"
        if maybe():
            kw["zpad"] = pick([4, 2])
        if maybe():
            kw["interp"] = pick(["Fourier", "cubic", "heptic", "quintic", "linear"])
        if maybe():
            kw["outputtype"] = pick(["NIFTI_GZ", "NIFTI"])
        if maybe(0.3):
            kw["basefile"] = f["ref.nii.gz"]
        for k in ("timeshift", "copyorigin", "verbose"):
            if maybe(0.3):
                kw[k] = True
    return kw


def _ni_nipype(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    import nibabel as nb

    from larmorx import _nipype

    prog = e["program"]
    path, formatter = NIPYPE[prog]
    work = tmp / "nipype"
    work.mkdir(exist_ok=True)
    f = {}
    for name in ("in.nii.gz", "ref.nii.gz", "mask.nii.gz", "t.mat", "w.nii.gz"):
        (work / name).write_bytes(b"")
        f[name] = str(work / name)
    nb.save(nb.Nifti1Image(np.zeros((2, 2, 2, 3), np.float32), np.eye(4)), work / "in.nii.gz")
    kw = _nipype_inputs(prog, np.random.default_rng(e["seed"]), f)
    mod, cls = path.rsplit(".", 1)

    def outputs(o: dict) -> dict:
        return {k: v for k, v in o.items() if v not in (None, []) and not str(v).startswith("<")}

    def ref():
        import importlib

        with _cwd(work):
            iface = getattr(importlib.import_module(mod), cls)(**kw)
            return {"cmdline": iface.cmdline, "outputs": outputs(iface._list_outputs())}

    def mine(mode):
        with _cwd(work):
            cmd = getattr(_nipype, formatter)(cwd=work, **kw)
            ours = outputs(dict(cmd.outputs))
        try:
            theirs = ref()["outputs"]
        except Exception:  # nipype refuses the inputs: every output larmorx gives
            theirs = ours
        # nipype's outputs, each as larmorx gives it (check_nipype's rule: the keys nipype lists)
        return {"cmdline": cmd.cmdline, "outputs": {k: ours.get(k) for k in theirs}}

    return _Call(ref, mine, switch=False)


def _es_trt(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from sdcflows.utils.epimanip import get_trt

    import larmorx.sdcflows as lxs

    meta, src = e["meta"], files.get("in")
    kw = {"use_estimate": e["use_estimate"], "fallback": e["fallback"]}
    return _Call(
        lambda: get_trt(meta, src, **kw), lambda mode: lxs.get_trt(meta, src, **kw), switch=False
    )


def _es_readout(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from sdcflows.interfaces.epi import GetReadoutTime

    import larmorx.sdcflows as lxs

    meta, src, fb = e["meta"], files["in"], e["fallback"]

    def ref():
        from sdcflows.interfaces.epi import _GetReadoutTimeOutputSpec
        from traits.api import TraitError

        gi = GetReadoutTime(metadata=meta, in_file=src)
        if fb is not None:
            gi.inputs.fallback = fb
        try:
            o = gi.run(cwd=str(tmp)).outputs
        except TraitError:
            # nipype checks the outputs in the order of a set of their names, which follows the
            # process's string hashes (PIPE-39): with several invalid, its error names any;
            # the outputs its _run_interface computed, checked by traits in declared order
            spec = _GetReadoutTimeOutputSpec()
            for k in ("readout_time", "pe_direction", "pe_dir_fsl"):
                setattr(spec, k, gi._results[k])
            raise
        return (o.readout_time, o.pe_direction, o.pe_dir_fsl)

    def mine(mode):
        r = lxs.get_readout_time(meta, src, fallback=fb)
        return (r.readout_time, r.pe_direction, r.pe_dir_fsl)

    return _Call(ref, mine, switch=False)


def _multi(v) -> list:
    """nipype's ``OutputMultiObject``: a list of one comes out as its element."""
    return list(v) if isinstance(v, list | tuple) else [v]


def _paths_out(v, d: Path):
    vals = v if isinstance(v, list | tuple) else [v]
    return [
        os.path.relpath(x, d) if os.path.isabs(str(x)) and str(x).startswith(str(d)) else str(x)
        for x in vals
    ]


def _es_select(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from sdcflows.interfaces.epi import SelectPEVolumes, SortPEBlips

    import larmorx.sdcflows as lxs

    srcs = _in_files(files)
    pe, ro = e["pe"], e["ro"]
    sel = "max_vols" in e

    def ref():
        d = tmp / "a"
        d.mkdir(exist_ok=True)
        if sel:
            o = (
                SelectPEVolumes(
                    in_data=srcs, pe_dirs_fsl=pe, readout_times=ro, max_vols_per_pe=e["max_vols"]
                )
                .run(cwd=str(d))
                .outputs
            )
            return {
                "out_data": _paths_out(o.out_data, d),
                "pe_dirs_fsl": _multi(o.pe_dirs_fsl),
                "readout_times": _multi(o.readout_times),
                "files": _folder(d),
            }
        o = SortPEBlips(in_data=srcs, pe_dirs_fsl=pe, readout_times=ro).run(cwd=str(d)).outputs
        return {
            "out_data": _paths_out(o.out_data, d),
            "pe_dirs": _multi(o.pe_dirs),
            "pe_dirs_fsl": _multi(o.pe_dirs_fsl),
            "readout_times": _multi(o.readout_times),
        }

    def mine(mode):
        d = tmp / f"b-{mode}"
        if sel:
            m = lxs.select_pe_volumes(srcs, pe, ro, e["max_vols"], out_dir=d, n_threads=nt)
            return {
                "out_data": _paths_out(list(m.out_data), d),
                "pe_dirs_fsl": list(m.pe_dirs_fsl),
                "readout_times": list(m.readout_times),
                "files": _folder(d),
            }
        m = lxs.sort_pe_blips(srcs, pe, ro)
        return {
            "out_data": _paths_out(list(m.out_data), d),
            "pe_dirs": list(m.pe_dirs),
            "pe_dirs_fsl": list(m.pe_dirs_fsl),
            "readout_times": list(m.readout_times),
        }

    return _Call(ref, mine, switch=False)


def _es_uniform(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from sdcflows.interfaces.utils import UniformGrid

    import larmorx.sdcflows as lxs

    srcs = _in_files(files)
    ref_i = e["reference"]

    def ref():
        d = tmp / "a"
        d.mkdir(exist_ok=True)
        o = UniformGrid(in_data=srcs, reference=ref_i).run(cwd=str(d)).outputs
        return {
            "out_data": _paths_out(o.out_data, d),
            "reference": _paths_out(o.reference, d),
            "files": _folder(d),
        }

    def mine(mode):
        d = tmp / f"b-{mode}"
        m = lxs.uniform_grid(srcs, ref_i, out_dir=d, n_threads=nt)
        return {
            "out_data": _paths_out(list(m.out_data), d),
            "reference": _paths_out(m.reference, d),
            "files": _folder(d),
        }

    return _Call(ref, mine, switch=False)


def _es_reorient(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from sdcflows.interfaces.utils import ReorientImageAndMetadata

    import larmorx.sdcflows as lxs

    src, target, pe = files["in"], e["target"], e["pe"]

    def ref():
        d = tmp / "a"
        d.mkdir(exist_ok=True)
        o = (
            ReorientImageAndMetadata(in_file=src, target_orientation=target, pe_dir=pe)
            .run(cwd=str(d))
            .outputs
        )
        return {
            "out_file": _paths_out(o.out_file, d),
            "pe_dir": _multi(o.pe_dir),
            "files": _folder(d),
        }

    def mine(mode):
        d = tmp / f"b-{mode}"
        f, p = lxs.reorient_image_and_metadata(
            src, target, pe, out_dir=d, upstream_defects=mode, n_threads=nt
        )
        return {"out_file": _paths_out(f, d), "pe_dir": list(p), "files": _folder(d)}

    return _Call(ref, mine)


def _es_masker(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from sdcflows.utils.tools import brain_masker

    import larmorx.sdcflows as lxs

    src, pad = files["in"], e["padding"]

    def ref():
        d = tmp / "a"
        d.mkdir(exist_ok=True)
        out = brain_masker(src, str(d / "clipped_mask.nii.gz"), padding=pad)
        return {"names": _paths_out(list(out), d), "files": _folder(d)}

    def mine(mode):
        d = tmp / f"b-{mode}"
        d.mkdir(exist_ok=True)
        r = lxs.brain_masker(
            src,
            d / "clipped_mask.nii.gz",
            pad,
            **_versions(lxs.brain_masker),
            upstream_defects=mode,
            n_threads=nt,
        )
        return {
            "names": _paths_out([r.out_brain, r.out_probseg, r.out_mask], d),
            "files": _folder(d),
        }

    return _Call(ref, mine)


def _es_epi_mask(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from sdcflows.utils.epimanip import epi_mask

    import larmorx.sdcflows as lxs

    src = files["in"]

    def ref():
        with _cwd(tmp / "a") as d:
            out = epi_mask(src)
            return {"out_file": _paths_out(out, d), "files": _folder(d)}

    def mine(mode):
        with _cwd(tmp / f"b-{mode}") as d:
            r = lxs.epi_mask(src, **_versions(lxs.epi_mask), upstream_defects=mode, n_threads=nt)
            return {"out_file": _paths_out(r.out_file, d), "files": _folder(d)}

    return _Call(ref, mine)


def _es_match(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    from sdcflows.workflows.fit.syn import match_histogram

    import larmorx.sdcflows as lxs

    args = [files["ref"], files["img"], files.get("ref_mask"), files.get("img_mask")]

    def ref():
        with _cwd(tmp / "a") as d:
            out = match_histogram(*args)
            return {"out_file": _paths_out(out, d), "files": _folder(d)}

    def mine(mode):
        d = tmp / f"b-{mode}"
        out = lxs.match_histogram(*args, out_dir=d, upstream_defects=mode, n_threads=nt)
        return {"out_file": _paths_out(out, d), "files": _folder(d)}

    return _Call(ref, mine)


def _es_estimators(e: dict, files: dict, tmp: Path, nt: int) -> _Call:
    """pybids' index and sdcflows 2.17.1's search against ``lx.bids`` and
    ``lx.sdcflows.find_estimators``, through ``check_estimators``' serializer: both modes
    reproduce = the originals on the dataset where it is (below the drawn parent folder)."""
    import sdcflows.utils.wrangler as w2171
    from bids.layout import BIDSLayout as PLayout
    from bids.layout import Query as PQ
    from bids.layout import parse_file_entities as pparse
    from niworkflows.utils.testing import generate_bids_skeleton
    from sdcflows import fieldmaps as sfm

    import larmorx.bids as lb
    import larmorx.sdcflows as ls

    dump = _load("estimators_dump")
    w2171.reduce = dump.natural_reduce  # SDC-19 (check_estimators' rule)
    # each side its own copy of the dataset, below the drawn folder (mkdtemp's name holds no
    # dash, so no entity of pybids' patterns)
    root = Path(tempfile.mkdtemp(prefix="s", dir=tmp)) / e["parent"] / "ds"
    root.parent.mkdir(parents=True)
    generate_bids_skeleton(str(root), e["spec"])
    ser = dump.Ser({str(root): "<root>"})
    fl = e["fmapless"]
    cases = [{"subject": "01", **({"force_fmapless": True} if fl == "force" else {"fmapless": fl})}]

    def ref():
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            lay = PLayout(str(root), validate=False)
        idx = dump.dump_index(lay, ser, PQ, pparse)
        found = dump.dump_wrangler(
            w2171.find_estimators,
            sfm.clear_registry,
            lambda: dump.registry_state(sfm._estimators, sfm._intents, ser),
            lay,
            ser,
            cases,
        )
        return {"index": idx, "estimators": found}

    def mine(mode):
        lay = lb.BIDSLayout(str(root), validate=False, upstream_defects=mode)
        idx = dump.dump_index(lay, ser, lb.Query, lb.parse_file_entities)

        def find(**kw):
            return ls.find_estimators(upstream_defects=mode, **kw)

        found = dump.dump_wrangler(
            find,
            ls.clear_registry,
            lambda: dump.registry_state(ls.REGISTRY.estimators, ls.REGISTRY.intents, ser),
            lay,
            ser,
            cases,
        )
        return {"index": idx, "estimators": found}

    # SDC-45's notice is about the entities parsed from the folders above a participant's, which
    # need not change the estimators found
    return _Call(ref, mine, silent=("SDC-45",))


CALLS: dict[str, Callable[[dict, dict, Path, int], _Call]] = {
    "gaussian": _c_gaussian,
    "correlate1d": _c_correlate1d,
    "rank": _c_rank,
    "grey": _c_grey,
    "binary": _c_binary,
    "label": _c_label,
    "measure": _c_measure,
    "watershed": _c_watershed,
    "skimage": _c_skimage,
    "numpy": _c_numpy,
    "inv": _c_inv,
    "clip": _ni_clip,
    "robust_average": _ni_ravg,
    "merge_series": _ni_merge,
    "intra_modal_merge": _ni_imm,
    "regrid": _ni_regrid,
    "nipype": _ni_nipype,
    "get_trt": _es_trt,
    "readout_time": _es_readout,
    "select_pe": _es_select,
    "sort_pe": _es_select,
    "uniform_grid": _es_uniform,
    "reorient": _es_reorient,
    "brain_masker": _es_masker,
    "epi_mask": _es_epi_mask,
    "match_histogram": _es_match,
    "estimators": _es_estimators,
}


# --------------------------------------------------------------------------------------------


def _numpy_lapack() -> str:
    """The LAPACK release this environment's numpy bundles: "3.12.1" from OpenBLAS 0.3.31 on
    (numpy 2.5.4's 0.3.34), else "3.12.0" (numpy 2.3.5's 0.3.30); as
    ``validation/investigations/sdcflows-fieldmap/stepdigest.py``'s ``numpy_lapack``."""
    version = np.show_config(mode="dicts")["Build Dependencies"]["blas"]["version"]
    return "3.12.1" if tuple(int(x) for x in version.split(".")[:3]) >= (0, 3, 31) else "3.12.0"


def run_case(case: Case, checks: CheckList) -> None:
    """A random or permanent case of the three tools (payload ``("random", spec)``), larmorx's
    nibabel arithmetic (``io_orientation``'s and ``set_qform``'s SVD) following this
    environment's LAPACK release, as the original's nibabel does (relay 28: the permanent case
    ``reorient-rotation-with-two-candidates`` in the shared environment), for this case only."""
    contextvars.copy_context().run(_run_case, case, checks)


def _run_case(case: Case, checks: CheckList) -> None:
    from larmorx.io.nibabel_header import use_lapack
    from larmorx_validation.difftest import inputs
    from larmorx_validation.parity import defects

    use_lapack(_numpy_lapack())

    spec = case.payload[1]
    recipe = spec["recipe"]
    e = recipe["extra"]
    call = e["call"]
    nt = int(e.get("threads", 1))
    files = {name: str(inputs.path(k)) for name, k in spec["files"].items()}
    with tempfile.TemporaryDirectory(prefix="lx-fmapb-dt_") as t:
        tmp = Path(t)
        # each side its own arguments, built afresh from the recipe (an original may write into
        # its caller's arrays: SCI-31), and its own output folder
        c = CALLS[call](e, files, tmp, nt)
        want, w1 = _run(c.original)
        got, w2 = _run(lambda: CALLS[call](e, files, tmp, nt).larmorx("reproduce"))
        ok = _key(want) == _key(got)
        detail = ""
        if not ok:
            detail = _describe(want, got)
        elif isinstance(want, _Err):
            detail = f"both refuse: {want!r}"[:300]
        checks(f"{call}: every output bit-identical", ok, value=0 if ok else 1, detail=detail[:400])
        if c.warnings is not None:
            a, b = c.warnings(w1, w2)
            checks(f"{call}: the warnings", a == b, detail="" if a == b else f"{a} / {b}"[:400])
        if not c.switch:
            return
        fixed, w3 = _run(lambda: CALLS[call](e, files, tmp, nt).larmorx("fix"))
        entries = defects.warning_entries(w3)
        if isinstance(fixed, _Err):
            entries = list(dict.fromkeys([*entries, *_IDS.findall(fixed.text)]))
        change = defects.Change(0, 0, 0.0)
        other = ""
        if _key(fixed) != _key(got):
            if isinstance(fixed, _Err) or isinstance(got, _Err):
                other = f"fix: {fixed!r:.120}; reproduce: {got!r:.120}"
            else:
                fa, ra = _arrays(fixed), _arrays(got)
                if len(fa) == len(ra) and all(
                    x.shape == y.shape for x, y in zip(fa, ra, strict=True)
                ):
                    for x, y in zip(fa, ra, strict=True):
                        change = change + defects.Change.of(x, y)
                else:
                    other = f"{len(fa)} arrays in fix, {len(ra)} in reproduce"
                if _key(_strip(fixed)) != _key(_strip(got)):
                    other = (other + "; " if other else "") + _describe(_strip(fixed), _strip(got))[
                        :200
                    ]
        defects.add_fix_check(checks, entries, change, other_changes=other, silent_ok=c.silent)


__all__ = ["CALLS", "run_case"]
