# SPDX-License-Identifier: Apache-2.0
"""Random testing's cases of the confounds' part (``difftest/tools/confounds.py``) and their
permanent cases, for the suites ``confounds-linalg``, ``confounds-nipype`` and
``confounds-fmriprep`` (whose script cases ``parity.confounds_scripts`` runs).

One drawn call per case: the arrays and files are built from the recipe, the original runs in
this process (numpy, SciPy, nitime, nipype, niworkflows, pandas, fMRIPrep 25.2.7's and 25.2.6's
interfaces from their tags) and larmorx with ``upstream_defects="reproduce"`` and this
environment's releases (``numpy_version``, ``scipy_version``, ``pandas_version``); one check
compares every output bit for bit (arrays by type, shape and bytes, numbers by their bits, NIfTI
files by header and stored data, other files by their decompressed bytes, errors by type and last
line, traits' ``TraitError`` as larmorx's ``ValueError``). A NaN whose payload differs is counted
apart (x86-64's default NaN; the suites' ``nan-bits``). Then the D13 fix pass: larmorx's default
against ``reproduce``, a defect named exactly when they differ
(:func:`larmorx_validation.parity.defects.add_fix_check`).

Both sides run in a work folder of the same name (made afresh for each run), so the paths an
original writes into its messages are larmorx's too. The comparisons follow the investigation's
scripts (``validation/investigations/confounds``), whose helpers are imported where they hold the
originals' rules: ``check_motion``'s model of SciPy's Euler angles (D4: the ``rot_*`` cells
glibc's ``atan2``/``hypot`` change are compared with the model's correctly rounded text) and
fMRIPrep's interfaces from their tags, ``check_masks``' fMRIPrep functions from the tag,
``check_tables``' table forms, ``check_model``'s columns by name (PIPE-43). Powers other than 0,
1 and 2 (``ExpandModel``) are compared only where numpy's ``pow`` is the C library's (no AVX-512
dispatch: ``NPY_DISABLE_CPU_FEATURES``), as ``check_model.py`` compares them.
"""

from __future__ import annotations

import contextlib
import contextvars
import dataclasses
import functools
import gzip
import importlib.util
import logging
import math
import os
import re
import shutil
import sys
import tempfile
import warnings
from collections.abc import Callable
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity.fieldmaps_b_random import (
    _arrays,
    _describe,
    _Err,
    _key,
    _need_kernels,
    _numpy_lapack,
    _strip,
)
from larmorx_validation.parity.harness import SKIPPED, Case, CheckList, Outcome

SCRIPTS = Path(__file__).resolve().parents[3] / "investigations" / "confounds"
_IDS = re.compile(r"\b((?:PIPE|SCI|NP|PD|GLIBC|NIB|SKI|SDC)-\d\d)\b")
for _name in ("nipype.interface", "nipype.workflow", "nipype.utils", "larmorx.confounds"):
    logging.getLogger(_name).setLevel(logging.ERROR)


# --------------------------------------------------------------------------------------------
# helpers


@functools.cache
def _load(name: str):
    """One of the investigation's scripts as a module (its folder on ``sys.path``)."""
    if str(SCRIPTS) not in sys.path:
        sys.path.insert(0, str(SCRIPTS))
    spec = importlib.util.spec_from_file_location(f"_lx_conf_{name}", SCRIPTS / f"{name}.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@functools.cache
def _fmriprep(release: str):
    """fMRIPrep ``release``'s ``interfaces/confounds.py`` from its tag (``check_motion``)."""
    return _load("check_motion").fmriprep_module(release)


@functools.cache
def _pow_is_libm() -> bool:
    """Whether numpy's float64 ``x ** 3`` is the C library's ``pow`` here (no SVML dispatch)."""
    xs = np.random.default_rng(11).uniform(-30.0, 30.0, 4000)
    return all(float(v) == math.pow(float(x), 3.0) for v, x in zip(xs**3.0, xs, strict=True))


def _versions() -> dict[str, str]:
    import pandas
    import scipy

    return {
        "numpy_version": np.__version__,
        "scipy_version": scipy.__version__,
        "pandas_version": pandas.__version__,
    }


def _kw(fn, **extra) -> dict:
    """The release arguments ``fn`` takes, and ``extra``."""
    import inspect

    params = inspect.signature(fn).parameters
    out = {k: v for k, v in _versions().items() if k in params}
    out.update(extra)
    return out


class _Line(_Err):
    """An exception as a result: its type (traits' ``TraitError`` as ``ValueError``, the type
    larmorx raises for the same refused input) and its message's last line."""

    def __init__(self, exc: BaseException, *, kind_only: bool = False) -> None:
        kind = type(exc).__name__
        self.kind = "ValueError" if kind == "TraitError" else kind
        lines = str(exc).strip().splitlines()
        self.text = "" if kind_only else (lines[-1] if lines else "")
        #: the whole message (the fix pass reads the defects it names)
        self.full = str(exc)


def _guard(fn: Callable[[], Any], *, kind_only: bool = False) -> Any:
    """``fn()``, or its error as a result (a :class:`_Line`)."""
    try:
        return fn()
    except Exception as exc:  # the error is the result compared
        return _Line(exc, kind_only=kind_only)


def _canon(v: Any) -> Any:
    """``v`` with every NaN made one NaN (the suites count a NaN's payload apart)."""
    if isinstance(v, np.ndarray):
        if v.dtype.kind in "fc" and v.size and np.isnan(v).any():
            v = v.copy()
            parts = (v,) if v.dtype.kind == "f" else (v.real, v.imag)
            for p in parts:
                p[np.isnan(p)] = np.nan
        return v
    if isinstance(v, np.floating) and np.isnan(v):
        return type(v)(np.nan)
    if isinstance(v, float) and math.isnan(v):
        return math.nan
    if isinstance(v, dict):
        return {k: _canon(x) for k, x in v.items()}
    if isinstance(v, list | tuple):
        return type(v)(_canon(x) for x in v)
    return v


def _content(p: Path) -> Any:
    """A file's content: NIfTI files by header and stored data, others by their decompressed
    bytes."""
    if p.name.endswith((".nii", ".nii.gz")):
        import nibabel as nb

        try:
            img = nb.load(p)
            data = np.asarray(img.dataobj)
            return ("nifti", img.header.binaryblock, data)
        except Exception:  # a table named after a NIfTI reference (FSLMotionParams' .tsv.nii.gz)
            pass
    data = p.read_bytes()
    if p.name.endswith(".gz"):
        data = gzip.decompress(data)
    return data


def _files(d: Path) -> dict[str, Any]:
    """Every file ``d`` holds (nipype's own ``_*`` and ``result*`` files apart)."""
    if not d.exists():
        return {}
    return {
        str(p.relative_to(d)): _content(p)
        for p in sorted(d.rglob("*"))
        if p.is_file() and not p.name.startswith(("_", "result"))
    }


def _rel(v: Any, d: Path) -> Any:
    """Paths below ``d`` as relative names (outputs), recursively."""
    if isinstance(v, str | os.PathLike) and os.path.isabs(os.fspath(v)):
        s = os.fspath(v)
        return os.path.relpath(s, d) if s.startswith(str(d)) else s
    if isinstance(v, list | tuple):
        return [_rel(x, d) for x in v]
    if isinstance(v, dict):
        return {k: _rel(x, d) for k, x in v.items()}
    return v


@contextlib.contextmanager
def _work(tmp: Path):
    """The run's work folder, ``tmp/work``, made afresh (each side runs in a folder of the same
    name, so the paths in its messages are the same)."""
    d = tmp / "work"
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    old = os.getcwd()
    os.chdir(d)
    try:
        yield d  # warnings reach the caller's record (larmorx's notices: the fix pass)
    finally:
        os.chdir(old)


def _iface(klass, tmp: Path, outputs: tuple[str, ...] = (), *, kind_only: bool = False, **inputs):
    """A nipype interface run in the work folder: its files and the named outputs, or its
    error."""
    with _work(tmp) as d:

        def run():
            from nipype.interfaces.base import isdefined

            res = klass(**{k: v for k, v in inputs.items() if v is not None}).run()
            out = {}
            for k in outputs:
                v = getattr(res.outputs, k)
                out[k] = _rel(v, d) if isdefined(v) else None
            return {"files": _files(d), "outputs": out}

        return _guard(run, kind_only=kind_only)


def _lx(fn, tmp: Path, outputs: tuple[str, ...] = (), *args, kind_only: bool = False, **kw):
    """A larmorx function writing into the work folder (``out_dir``): its files and the named
    fields of its result, or its error."""
    with _work(tmp) as d:

        def run():
            r = fn(*args, out_dir=d, **kw)
            one = isinstance(r, str | os.PathLike)
            out = {k: _rel(r if one else getattr(r, k), d) for k in outputs}
            return {"files": _files(d), "outputs": out}

        return _guard(run, kind_only=kind_only)


def _plain(fn: Callable[[], Any]) -> Any:
    return _guard(fn)


@dataclasses.dataclass
class _Call:
    """One drawn call: the original, larmorx in a mode, and the fix pass's rules."""

    original: Callable[[], Any]
    larmorx: Callable[[str], Any]
    switch: bool = True
    #: defects whose notice is about a condition the outputs may not show
    silent: tuple[str, ...] = ()
    #: the original's and larmorx's (reproduce) results made comparable, with a note
    normalize: Callable[[Any, Any], tuple[Any, Any, str]] | None = None


def _ih(rng: np.random.Generator, shape) -> np.ndarray:
    """Noise of mean 0 (the sum of three uniform draws, centred: plain arithmetic)."""
    return rng.random(shape) + rng.random(shape) + rng.random(shape) - 1.5


def _f(fmt: str, v: float) -> str:
    return repr(float(v)) if fmt == "repr" else fmt % v


# --------------------------------------------------------------------------------------------
# confounds-linalg

_TINY = np.array([1.0, 1e-3, 1e-6, 1e-9, 1e-12])


def _matrix(s: dict) -> np.ndarray:
    rng = np.random.default_rng(s["seed"])
    m, n, kind = s["m"], s["n"], s["kind"]
    if kind == "integers":
        a = rng.integers(-9, 10, (m, n)).astype(np.float64)
    elif kind == "low rank":
        r = max(1, min(m, n) // 3)
        a = _ih(rng, (m, r)) @ _ih(rng, (r, n))
    elif kind == "repeated columns":
        a = _ih(rng, (m, n))
        a[:, 1::2] = a[:, :1]
    elif kind == "zero rows":
        a = _ih(rng, (m, n))
        a[::2] = 0.0
    elif kind == "nearly dependent":
        a = _ih(rng, (m, n))
        if n > 1:
            a[:, -1] = a[:, 0] + 1e-9 * a[:, -1]
    elif kind == "tiny pivots":
        a = _ih(rng, (m, n)) * _TINY[rng.integers(0, len(_TINY), n)]
    else:
        a = _ih(rng, (m, n))
    a = a * s["scale"]
    if s.get("nan") and a.size:
        a.flat[int(rng.integers(0, a.size))] = np.nan
    return np.asfortranarray(a) if s["layout"] == "F" else np.ascontiguousarray(a)


def _vector(s: dict) -> np.ndarray:
    rng = np.random.default_rng(s["seed"])
    t, kind = s["t"], s["kind"]
    base = 100.0 + 2.0 * _ih(rng, t)
    if kind == "smooth":
        x = 50.0 + np.arange(t) * 0.01
    elif kind == "steps":
        x = (np.arange(t) // 7).astype(np.float64) * 3.0
    elif kind == "zeros":
        x = np.zeros(t)
    elif kind == "constant":
        x = np.full(t, 5.0)
    elif kind == "subnormal":
        x = base * 1e-42
    else:
        x = base
    if kind in ("nan", "inf") and t:
        x[int(rng.integers(0, t))] = np.nan if kind == "nan" else np.inf
    if s.get("dtype") == "complex64":
        return (x + 1j * _ih(rng, t)).astype(np.complex64)
    return x.astype(np.float32)


def _ndarray(s: dict) -> np.ndarray:
    rng = np.random.default_rng(s["seed"])
    shape = tuple(s["shape"])
    kind = s["kind"]
    if kind == "integers":
        a = rng.integers(-50, 51, shape).astype(np.float64)
    elif kind == "bold":
        a = 100.0 + 2.0 * _ih(rng, shape)
    else:
        a = rng.random(shape) * 100.0
    if kind == "special" and a.size:
        for v in (np.nan, -0.0, np.inf):
            a.flat[int(rng.integers(0, a.size))] = v
    a = a.astype(s["dtype"])
    return np.asfortranarray(a) if s["layout"] == "F" else np.ascontiguousarray(a)


def _la_svd(e, files, tmp, nt):
    from larmorx import _core

    lapack = _numpy_lapack()
    return _Call(
        lambda: _plain(lambda: tuple(np.linalg.svd(_matrix(e["a"]), full_matrices=False))),
        lambda mode: _plain(
            lambda: tuple(_core.numpy_svd(_matrix(e["a"]), lapack=lapack, n_threads=nt))
        ),
        switch=False,
    )


def _rhs(e: dict, m: int) -> np.ndarray:
    rng = np.random.default_rng(e["b_seed"])
    return _ih(rng, (m,) if e["nrhs"] is None else (m, e["nrhs"]))


def _la_lstsq(e, files, tmp, nt):
    from larmorx import _core

    lapack = _numpy_lapack()

    def orig():
        a = _matrix(e["a"])
        x, res, rank, s = np.linalg.lstsq(a, _rhs(e, a.shape[0]), rcond=e["rcond"])
        return (x, res, int(rank), s)

    def mine(mode):
        a = _matrix(e["a"])
        x, res, rank, s = _core.numpy_lstsq(a, _rhs(e, a.shape[0]), e["rcond"], lapack, nt)
        return (x, res, int(rank), s)

    return _Call(lambda: _plain(orig), lambda mode: _plain(lambda: mine(mode)), switch=False)


def _la_pinv(e, files, tmp, nt):
    from larmorx import _core

    lapack = _numpy_lapack()
    return _Call(
        lambda: _plain(lambda: np.linalg.pinv(_matrix(e["a"]), rcond=e["rcond"])),
        lambda mode: _plain(
            lambda: _core.numpy_pinv(_matrix(e["a"]), e["rcond"], lapack=lapack, n_threads=nt)
        ),
        switch=False,
    )


def _la_gesvd(e, files, tmp, nt):
    import scipy
    import scipy.linalg

    from larmorx import _core

    return _Call(
        lambda: _plain(
            lambda: tuple(
                scipy.linalg.svd(_matrix(e["a"]), full_matrices=False, lapack_driver="gesvd")
            )
        ),
        lambda mode: _plain(
            lambda: tuple(
                _core.scipy_svd(
                    _matrix(e["a"]), False, True, "gesvd", scipy=scipy.__version__, n_threads=nt
                )
            )
        ),
        switch=False,
    )


def _la_fallback(e, files, tmp, nt):
    from nipype.algorithms.confounds import fallback_svd

    import larmorx.confounds as lc

    return _Call(
        lambda: _plain(lambda: tuple(fallback_svd(_matrix(e["a"]), full_matrices=False))),
        lambda mode: _plain(
            lambda: tuple(
                lc.fallback_svd(
                    _matrix(e["a"]), False, **_kw(lc.fallback_svd, n_threads=nt)
                )
            )
        ),
        switch=False,
    )


def _la_full_rank(e, files, tmp, nt):
    from nipype.algorithms.confounds import _full_rank

    import larmorx.confounds as lc

    def orig():
        x, c = _full_rank(_matrix(e["a"]), cmax=e["cmax"])
        return (x, np.float64(c))

    def mine(mode):
        x, c = lc.full_rank(_matrix(e["a"]), e["cmax"], **_kw(lc.full_rank, n_threads=nt))
        return (x, np.float64(c))

    return _Call(lambda: _plain(orig), lambda mode: _plain(lambda: mine(mode)), switch=False)


def _operands(e: dict) -> tuple[np.ndarray, np.ndarray]:
    rng = np.random.default_rng(e["seed"])
    a, b = _ih(rng, (e["m"], e["k"])), _ih(rng, (e["k"], e["n"]))
    lay = e["layouts"]
    return (
        np.asfortranarray(a) if lay[0] == "F" else a,
        np.asfortranarray(b) if lay[1] == "F" else b,
    )


def _la_matmul(e, files, tmp, nt):
    from larmorx import _core

    def orig():
        a, b = _operands(e)
        return a @ b

    def mine(mode):
        a, b = _operands(e)
        return _core.numpy_matmul(a, b, nt)

    return _Call(lambda: _plain(orig), lambda mode: _plain(lambda: mine(mode)), switch=False)


def _la_reduce(e, files, tmp, nt):
    from larmorx import _core

    op, axis = e["op"], e["axis"]
    extra = {"ddof": e["ddof"]} if op in ("var", "std") else {}
    return _Call(
        lambda: _plain(lambda: np.asarray(getattr(np, op)(_ndarray(e["array"]), axis=axis, **extra))),
        lambda mode: _plain(
            lambda: np.asarray(
                _core.numpy_reduce(_ndarray(e["array"]), op, axis, np.__version__, e["ddof"])
            )
        ),
        switch=False,
    )


def _la_percentile(e, files, tmp, nt):
    from larmorx import _core

    axis = e["axis"]
    if e["fn"] == "median":
        return _Call(
            lambda: _plain(lambda: np.asarray(np.median(_ndarray(e["array"]), axis=axis))),
            lambda mode: _plain(lambda: np.asarray(_core.numpy_median(_ndarray(e["array"]), axis))),
            switch=False,
        )
    return _Call(
        lambda: _plain(
            lambda: np.asarray(
                np.percentile(_ndarray(e["array"]), e["q"], axis=axis, method=e["method"])
            )
        ),
        lambda mode: _plain(
            lambda: np.asarray(
                _core.numpy_percentile_rank(
                    _ndarray(e["array"]), e["q"], axis, e["method"], np.__version__
                )
            )
        ),
        switch=False,
    )


def _la_fft(e, files, tmp, nt):
    import scipy
    from scipy import fftpack

    from larmorx import _core

    spec = dict(e["x"], dtype=e["dtype"])
    fn = fftpack.fft if e["forward"] else fftpack.ifft
    return _Call(
        lambda: _plain(lambda: fn(_vector(spec), e["n"])),
        lambda mode: _plain(
            lambda: _core.scipy_fftpack(_vector(spec), e["n"], e["forward"], scipy.__version__)
        ),
        switch=False,
    )


def _la_crosscov(e, files, tmp, nt):
    import scipy
    from nitime.utils import crosscov

    from larmorx import _core

    def xy():
        x = np.ascontiguousarray(_vector(e["x"]))
        if e["same"]:
            return x, x
        y = np.ascontiguousarray(_vector(dict(e["x"], seed=e["x"]["seed"] + 1, kind="bold")))
        return x, y

    def orig():
        x, y = xy()
        return crosscov(x, y, debias=False, all_lags=e["all_lags"])

    def mine(mode):
        x, y = xy()
        return _core.nitime_crosscov(x, y, e["all_lags"], scipy.__version__)

    return _Call(lambda: _plain(orig), lambda mode: _plain(lambda: mine(mode)), switch=False)


def _la_ar(e, files, tmp, nt):
    import scipy
    from nitime.algorithms import AR_est_YW

    from larmorx import _core

    def orig():
        ak, sigma = AR_est_YW(np.ascontiguousarray(_vector(e["x"])), 1)
        return (np.asarray(ak), np.float32(sigma))

    def mine(mode):
        ak, sigma = _core.nitime_ar_est_yw(np.ascontiguousarray(_vector(e["x"])), scipy.__version__)
        return (np.asarray(ak), np.float32(sigma))

    return _Call(lambda: _plain(orig), lambda mode: _plain(lambda: mine(mode)), switch=False)


# --------------------------------------------------------------------------------------------
# confounds-nipype


def _with_bright(path: str, e: dict, tmp: Path) -> str:
    """The series with its first volumes brighter (the detector's case), written beside it."""
    b = e.get("bright")
    if not b:
        return path
    import nibabel as nb

    img = nb.load(path)
    data = np.asarray(img.dataobj).copy()
    if data.ndim < 4:
        return path
    k = b["volumes"]
    if data.dtype.kind == "f":
        data[..., :k] = data[..., :k] * data.dtype.type(b["factor"])
    else:
        info = np.iinfo(data.dtype)
        x = np.clip(data[..., :k].astype(np.float64) * b["factor"], info.min, info.max)
        data[..., :k] = np.floor(x).astype(data.dtype)
    out = tmp / "in" / ("bright" + "".join(Path(path).suffixes))
    out.parent.mkdir(parents=True, exist_ok=True)
    nb.Nifti1Image(data, img.affine, img.header).to_filename(out)
    return str(out)


def _np_dvars(e, files, tmp, nt):
    from nipype.algorithms.confounds import compute_dvars

    import larmorx.confounds as lc

    f, m = files["series"], files["mask"]
    kw = e["kw"]
    return _Call(
        lambda: _plain(lambda: tuple(compute_dvars(f, m, **kw))),
        lambda mode: _plain(
            lambda: lc.compute_dvars(
                f, m, **kw, **_kw(lc.compute_dvars, upstream_defects=mode, n_threads=nt)
            ).as_tuple()
        ),
    )


AVG = ("avg_std", "avg_nstd", "avg_vxstd")


def _avg(out: Any) -> Any:
    if isinstance(out, dict):
        for k in AVG:
            v = out["outputs"].get(k)
            out["outputs"][k] = None if v is None else np.float64(v)
    return out


def _np_dvars_node(e, files, tmp, nt):
    from nipype.algorithms.confounds import ComputeDVARS

    import larmorx.confounds as lc

    f, m = files["series"], files["mask"]
    kw = e["kw"]
    return _Call(
        lambda: _avg(_iface(ComputeDVARS, tmp, AVG, in_file=f, in_mask=m, **kw)),
        lambda mode: _avg(
            _lx(
                lc.compute_dvars_node,
                tmp,
                AVG,
                f,
                m,
                **kw,
                **_kw(lc.compute_dvars_node, upstream_defects=mode, n_threads=nt),
            )
        ),
    )


def _masks(files: dict) -> list[str]:
    return [files[k] for k in sorted(files) if k.startswith("mask")]


def _np_compcor(e, files, tmp, nt):
    from nipype.algorithms import confounds as nac

    import larmorx.confounds as lc

    klass = getattr(nac, e["kind"])
    fn = {"ACompCor": lc.acompcor, "TCompCor": lc.tcompcor, "CompCor": lc.compcor}[e["kind"]]
    f = files["series"]
    masks = _masks(files)
    kw = dict(e["kw"])
    mask_kw = {"mask_files": masks} if masks else {}
    def normalize(want, got):
        # the result's pre-filter basis (PIPE-52, PIPE-55) has no interface output to compare
        # with: compared in the fix pass only
        if isinstance(got, dict):
            got = dict(got, outputs={})
        return want, got, ""

    return _Call(
        lambda: _iface(klass, tmp, realigned_file=f, **mask_kw, **kw),
        lambda mode: _lx(
            fn,
            tmp,
            ("basis",),
            f,
            masks or None,
            **kw,
            **_kw(fn, upstream_defects=mode, n_threads=nt),
        ),
        normalize=normalize,
    )


def _data(s: dict) -> np.ndarray:
    rng = np.random.default_rng(s["seed"])
    n, t = s["voxels"], s["t"]
    kind = s["kind"]
    if kind == "smooth":
        x = 50.0 + np.arange(t)[None, :] * 0.03 + _ih(rng, (n, 1)) * 10.0 + 0.0 * _ih(rng, (n, t))
    elif kind == "constant":
        x = np.full((n, t), 7.0)
    else:
        x = 100.0 + 2.0 * _ih(rng, (n, t)) + _ih(rng, (n, 1)) * 20.0
    if kind == "nan" and x.size:
        x.flat[int(rng.integers(0, x.size))] = np.nan
    if s["dtype"] == "int16":
        x = np.floor(np.nan_to_num(x) + 0.5)
    x = x.astype(s["dtype"])
    return np.asfortranarray(x) if s["layout"] == "F" else np.ascontiguousarray(x)


def _np_regress(e, files, tmp, nt):
    from nipype.algorithms.confounds import regress_poly

    import larmorx.confounds as lc

    args = dict(remove_mean=e["remove_mean"], failure_mode=e["failure_mode"])
    return _Call(
        lambda: _plain(lambda: tuple(regress_poly(e["degree"], _data(e["data"]), **args))),
        lambda mode: _plain(
            lambda: tuple(
                lc.regress_poly(
                    e["degree"],
                    _data(e["data"]),
                    **args,
                    **_kw(lc.regress_poly, upstream_defects=mode, n_threads=nt),
                )
            )
        ),
    )


def _np_cosine(e, files, tmp, nt):
    from nipype.algorithms.confounds import cosine_filter

    import larmorx.confounds as lc

    args = dict(remove_mean=e["remove_mean"], failure_mode=e["failure_mode"])
    ts, pc = e["timestep"], e["period_cut"]
    return _Call(
        lambda: _plain(lambda: tuple(cosine_filter(_data(e["data"]), ts, pc, **args))),
        lambda mode: _plain(
            lambda: tuple(
                lc.cosine_filter(
                    _data(e["data"]),
                    ts,
                    pc,
                    **args,
                    **_kw(lc.cosine_filter, upstream_defects=mode, n_threads=nt),
                )
            )
        ),
    )


def _np_tsnr(e, files, tmp, nt):
    from nipype.algorithms.confounds import TSNR

    import larmorx.confounds as lc

    srcs = [files[k] for k in sorted(files) if k.startswith("in")]
    src: Any = srcs if len(srcs) > 1 else srcs[0]
    rp = e["regress_poly"]
    extra = {"regress_poly": rp} if rp is not None else {}
    return _Call(
        lambda: _iface(TSNR, tmp, in_file=src, **extra),
        lambda mode: _lx(
            lc.tsnr, tmp, (), src, rp, **_kw(lc.tsnr, upstream_defects=mode, n_threads=nt)
        ),
    )


def _np_nssd(e, files, tmp, nt):
    import nibabel as nb
    from nipype.algorithms.confounds import NonSteadyStateDetector

    import larmorx.confounds as lc

    f = _with_bright(files["series"], e, tmp)

    def orig():
        with _work(tmp):
            res = NonSteadyStateDetector(in_file=f).run()
            gs = nb.load(f).dataobj[:, :, :, :50].mean(axis=0).mean(axis=0).mean(axis=0)
            return {"n": int(res.outputs.n_volumes_to_discard), "signal": np.asarray(gs)}

    def mine(mode):
        r = lc.nonsteady_state_detector(
            f, **_kw(lc.nonsteady_state_detector, upstream_defects=mode, n_threads=nt)
        )
        return {"n": int(r.n_volumes_to_discard), "signal": np.asarray(r.global_signal)}

    return _Call(lambda: _guard(orig), lambda mode: _plain(lambda: mine(mode)))


def _motion_rows(m: dict, columns: int) -> np.ndarray:
    rng = np.random.default_rng(m["seed"])
    t = m["t"]
    x = _ih(rng, (t, columns))
    half = min(3, columns)
    x[:, :half] *= m["rot"]
    x[:, half:] *= m["trans"]
    if m["special"] and x.size:
        x.flat[int(rng.integers(0, x.size))] = {"nan": np.nan, "inf": np.inf, "-0": -0.0}[
            m["special"]
        ]
    return x


def _motion_text(m: dict, columns: int) -> str:
    rows = _motion_rows(m, columns)
    fmt = m["format"]
    return "".join("  ".join(_f(fmt, v) for v in row) + "\n" for row in rows)


def _write(tmp: Path, name: str, text: str | bytes) -> str:
    p = tmp / "in" / name
    p.parent.mkdir(parents=True, exist_ok=True)
    data = text.encode("utf-8") if isinstance(text, str) else text
    if name.endswith(".gz"):
        data = gzip.compress(data, mtime=0)
    p.write_bytes(data)
    return str(p)


def _np_fd(e, files, tmp, nt):
    from nipype.algorithms.confounds import FramewiseDisplacement

    import larmorx.confounds as lc

    f = _write(tmp, "motion.par", _motion_text(e["motion"], e["columns"]))
    src, radius, norm = e["source"], e["radius"], e["normalize"]

    def fd_avg(out):
        if isinstance(out, dict):
            out["outputs"]["fd_average"] = np.float64(out["outputs"]["fd_average"])
        return out

    return _Call(
        lambda: fd_avg(
            _iface(
                FramewiseDisplacement,
                tmp,
                ("fd_average",),
                in_file=f,
                parameter_source=src,
                radius=radius,
                normalize=norm,
            )
        ),
        lambda mode: fd_avg(
            _lx(
                lc.framewise_displacement,
                tmp,
                ("fd_average",),
                f,
                src,
                radius,
                normalize=norm,
                **_kw(lc.framewise_displacement, upstream_defects=mode),
            )
        ),
    )


def _np_normalize_mc(e, files, tmp, nt):
    from nipype.utils.misc import normalize_mc_params

    import larmorx.confounds as lc

    def row():
        rng = np.random.default_rng(e["seed"])
        return _ih(rng, e["length"]) * e["scale"]

    return _Call(
        lambda: _plain(lambda: np.asarray(normalize_mc_params(row(), e["source"]))),
        lambda mode: _plain(
            lambda: np.asarray(
                lc.normalize_mc_params(
                    row(), e["source"], **_kw(lc.normalize_mc_params, upstream_defects=mode)
                )
            )
        ),
    )


# --------------------------------------------------------------------------------------------
# confounds-fmriprep: tables


def _table_frame(s: dict):
    """A pandas table of drawn columns (the kinds the chain writes, and odd ones)."""
    import pandas as pd

    rng = np.random.default_rng(s["seed"])
    rows, cols = s["rows"], s["cols"]
    data: dict[str, Any] = {}
    for j in range(cols):
        name = f"c{j}"
        pick = rng.random()
        if s["kinds"] == "floats" or pick < 0.6:
            mag = np.array([1e-300, 1e-20, 1e-6, 1e-3, 1.0, 1e3, 1e16, 1e300])[
                rng.integers(0, 8, rows)
            ]
            v = _ih(rng, rows) * mag
            if s["missing"] and rows:
                v[rng.random(rows) < 0.15] = np.nan
            data[name] = v
        elif pick < 0.75:
            v = rng.integers(-(2**40), 2**40, rows)
            if s["kinds"] == "odd" and rows:
                v[0] = np.iinfo(np.int64).min if rng.random() < 0.5 else np.iinfo(np.int64).max
            data[name] = v
        elif pick < 0.85:
            data[name] = rng.random(rows) < 0.5
        else:
            words = np.array(["a", "CSF", "n/a", "1.5e3", "x y", "", "nan", "True", "inf"])
            data[name if s["kinds"] != "odd" or j % 2 else "c0"] = list(
                words[rng.integers(0, len(words), rows)]
            )
    return pd.DataFrame(data)


def _table_text(s: dict) -> str:
    """The table's text as the chain writes it: pandas' ``to_csv`` with its float formats."""
    df = _table_frame(s)
    fmt = s["formats"][0]
    return df.to_csv(sep="\t", index=False, na_rep="n/a", float_format=None if fmt == "repr" else fmt)


def _frame_form(df) -> Any:
    """A pandas table's names, kinds and values (``check_tables``' columns)."""
    ct = _load("check_tables")
    cols = []
    for j in range(df.shape[1]):
        s = df.iloc[:, j]
        kind = ct.kind_of(s)
        if kind == "float64":
            cols.append((str(df.columns[j]), kind, s.to_numpy().copy()))
        elif kind == "object":
            cols.append((str(df.columns[j]), kind, [_obj(x) for x in s.to_numpy(dtype=object)]))
        else:
            cols.append((str(df.columns[j]), kind, s.to_numpy().copy()))
    return cols


def _obj(x: Any) -> Any:
    if isinstance(x, float | np.floating):
        return ("float", float(x))
    if isinstance(x, bool | np.bool_):
        return ("bool", bool(x))
    if isinstance(x, int | np.integer):
        return ("int", int(x))
    return ("str", str(x))


def _table_form(t) -> Any:
    cols = []
    for name, c in zip(t.names, t.columns, strict=True):
        if c.kind == "object":
            cols.append((str(name), c.kind, [_obj(x) for x in c.values]))
        else:
            cols.append((str(name), c.kind, np.asarray(c.values).copy()))
    return cols


def _fp_read(e, files, tmp, nt):
    import pandas as pd

    from larmorx.confounds import _tables as tb

    path = _write(tmp, "table" + e["suffix"], _table_text(e["table"]))
    return _Call(
        lambda: _plain(lambda: _frame_form(pd.read_csv(path, sep="\t"))),
        lambda mode: _plain(
            lambda: _table_form(
                tb.read_tsv(path, pandas_version=pd.__version__, upstream_defects=mode)
            )
        ),
    )


def _fp_write(e, files, tmp, nt):
    from larmorx.confounds import _tables as tb

    ct = _load("check_tables")
    name = "out" + e["suffix"]

    def orig():
        with _work(tmp) as d:
            _table_frame(e["table"]).to_csv(d / name, sep="\t", index=False, na_rep="n/a")
            return _files(d)

    def mine(mode):
        with _work(tmp) as d:
            tb.write_tsv(d / name, ct.to_table(_table_frame(e["table"])))
            return _files(d)

    return _Call(lambda: _guard(orig), lambda mode: _guard(lambda: mine(mode)), switch=False)


def _ujson_values(e: dict) -> np.ndarray:
    rng = np.random.default_rng(e["seed"])
    n = e["n"]
    kind = e["kind"]
    if kind == "ties":
        return rng.integers(-(2**30), 2**30, n) / np.array([2.0**k for k in range(1, 45)])[
            rng.integers(0, 44, n)
        ]
    if kind == "rounded":
        return np.round(_ih(rng, n) * 40.0, 10) + rng.choice([-1.0, 1.0], n) * 5e-11
    if kind == "special":
        vals = np.array(
            [0.0, -0.0, 0.99999999995, 9.99999999995, 1e16 - 2, 1e16, 1e-15, 9.999999999999999e-16,
             2.0**53, 5e-324, 4.5e-11, 1e300, np.nan, np.inf, -np.inf]
        )  # fmt: skip
        return vals[rng.integers(0, len(vals), n)]
    mag = np.array([1e-300, 1e-16, 1e-11, 1e-6, 1.0, 1e6, 1e15, 1e17, 1e300])
    return _ih(rng, n) * mag[rng.integers(0, len(mag), n)]


def _fp_ujson(e, files, tmp, nt):
    import pandas as pd

    from larmorx.confounds import _tables as tb

    def orig():
        return pd.Series(_ujson_values(e)).to_json(orient="values")

    def mine(mode):
        vals = _ujson_values(e).tolist()
        return "[" + ",".join("null" if not math.isfinite(v) else tb.ujson_double(v) for v in vals) + "]"

    return _Call(lambda: _plain(orig), lambda mode: _plain(lambda: mine(mode)), switch=False)


MOTION_COLUMNS = ("trans_x", "trans_y", "trans_z", "rot_x", "rot_y", "rot_z")


def _write_frame(tmp: Path, name: str, df, fmt: str = "repr") -> str:
    p = tmp / "in" / name
    p.parent.mkdir(parents=True, exist_ok=True)
    df.to_csv(p, sep="\t", index=False, na_rep="n/a", float_format=None if fmt == "repr" else fmt)
    return str(p)


def _motion_frame(m: dict, missing: bool = False):
    import pandas as pd

    rows = _motion_rows(m, 6)[:, [3, 4, 5, 0, 1, 2]]
    df = pd.DataFrame(rows, columns=list(MOTION_COLUMNS))
    if missing and len(df):
        df.iloc[0, 1] = np.nan
    return df


def _fp_fd(e, files, tmp, nt):
    import larmorx.confounds as lc

    fc = _fmriprep("25.2.7")
    f = _write_frame(tmp, "motion.tsv", _motion_frame(e["motion"], e["missing"]), e["motion"]["format"])
    return _Call(
        lambda: _iface(fc.FramewiseDisplacement, tmp, ("out_file",), in_file=f, radius=e["radius"]),
        lambda mode: _lx(
            lc.fmriprep_framewise_displacement,
            tmp,
            ("out_file",),
            f,
            radius=e["radius"],
            **_kw(lc.fmriprep_framewise_displacement, upstream_defects=mode),
        ),
    )


def _metadata_frame(md: dict):
    import pandas as pd

    rng = np.random.default_rng(md["seed"])
    rows = []
    for mask in md["masks"]:
        sv = np.sort(np.abs(100.0 + 50.0 * _ih(rng, md["n"])))[::-1]
        ve = sv * sv / np.sum(sv * sv)
        cve = np.cumsum(ve)
        for k in range(md["n"]):
            if md["retained"] == "all":
                keep: Any = True
            elif md["retained"] == "none":
                keep = False
            elif md["retained"] == "text":
                keep = "yes" if k % 2 else "no"
            else:
                keep = bool(cve[k] <= 0.5 or k == 0)
            name = md["names"] if md["names"] != "dropped" else "dropped"
            rows.append(
                {
                    "component": f"{name}_{len(rows):02d}",
                    "mask": mask,
                    "singular_value": float(f"{sv[k]:.10f}"),
                    "variance_explained": float(f"{ve[k]:.10f}"),
                    "cumulative_variance_explained": float(f"{cve[k]:.10f}"),
                    "retained": keep,
                }
            )
    return pd.DataFrame(rows)


def _fp_filter(e, files, tmp, nt):
    import larmorx.confounds as lc

    fc = _fmriprep("25.2.7")
    f = _write_frame(tmp, "metadata.tsv", _metadata_frame(e["metadata"]))
    return _Call(
        lambda: _iface(fc.FilterDropped, tmp, ("out_file",), in_file=f),
        lambda mode: _lx(
            lc.filter_dropped, tmp, ("out_file",), f, **_kw(lc.filter_dropped, upstream_defects=mode)
        ),
    )


def _fp_rename(e, files, tmp, nt):
    import pandas as pd

    import larmorx.confounds as lc

    fc = _fmriprep("25.2.7")
    md = _metadata_frame(e["metadata"])
    retained = md[md["retained"].astype(str) == "True"]["component"].tolist()
    if e["mismatch"]:
        retained = retained[:-1] if len(retained) > 1 else [*retained, "extra_00"]
    rng = np.random.default_rng(e["metadata"]["seed"] + 7)
    comps = pd.DataFrame({c: _ih(rng, e["components_t"]) for c in retained})
    cf = _write_frame(tmp, "components.tsv", comps, "%.10f")
    mf = _write_frame(tmp, "metadata.tsv", md)
    out = ("components_file", "metadata_file")
    return _Call(
        lambda: _iface(fc.RenameACompCor, tmp, out, components_file=cf, metadata_file=mf),
        lambda mode: _lx(
            lc.rename_acompcor, tmp, out, cf, mf, **_kw(lc.rename_acompcor, upstream_defects=mode)
        ),
    )


GATHER_COLUMNS = {
    "signals": ("global_signal", "csf", "white_matter", "csf_wm"),
    "dvars": ("dvars",),
    "std_dvars": ("std_dvars",),
    "fd": ("framewise_displacement",),
    "rmsd": ("rmsd",),
    "tcompcor": ("t_comp_cor_00", "t_comp_cor_01"),
    "acompcor": ("c_comp_cor_00", "w_comp_cor_00", "a_comp_cor_00"),
    "crowncompcor": ("edge_comp_00", "edge_comp_01"),
    "cos_basis": ("cosine00", "cosine01", "cosine02"),
    "motion": MOTION_COLUMNS,
}


def _fp_gather(e, files, tmp, nt):
    import pandas as pd

    import larmorx.confounds as lc

    fc = _fmriprep("25.2.7")
    rng = np.random.default_rng(e["seed"])
    paths: dict[str, str] = {}
    for k, part in enumerate(e["parts"]):
        t = e["t"] - (1 if part in ("dvars", "std_dvars") else 0)
        if e["mismatch"] and k == 0:
            t += 2
        t = max(t, 0)
        cols = GATHER_COLUMNS[part]
        df = pd.DataFrame({c: 10.0 * _ih(rng, t) for c in cols})
        if part == "fd" and t:
            df.iloc[0, 0] = np.nan
        paths[part] = _write_frame(tmp, f"{part}.tsv", df, e["format"])
    out = ("confounds_file", "confounds_list")
    return _Call(
        lambda: _iface(fc.GatherConfounds, tmp, out, **paths),
        lambda mode: _lx(
            lc.gather_confounds, tmp, out, **paths, **_kw(lc.gather_confounds, upstream_defects=mode)
        ),
    )


def _fp_add_header(e, files, tmp, nt):
    from niworkflows.interfaces.utility import AddTSVHeader

    import larmorx.niworkflows as lnw

    rng = np.random.default_rng(e["seed"])
    x = _ih(rng, (e["t"], e["k"])) * 3.0
    text = "".join("\t".join(e["format"] % v for v in row) + "\n" for row in x)
    f = _write(tmp, "values.tsv", text)
    k = e["k"] + {"right": 0, "too few": -1, "too many": 1}[e["names"]]
    cols = [f"col_{j}" for j in range(max(k, 0))]
    return _Call(
        lambda: _iface(AddTSVHeader, tmp, ("out_file",), in_file=f, columns=cols),
        lambda mode: _lx(
            lnw.add_tsv_header, tmp, ("out_file",), f, cols, **_kw(lnw.add_tsv_header, upstream_defects=mode)
        ),
    )


def _fp_tsv2json(e, files, tmp, nt):
    from niworkflows.interfaces.utility import TSV2JSON

    import larmorx.niworkflows as lnw

    f = _write_frame(tmp, "metadata.tsv", _metadata_frame(e["metadata"]))
    idx = e["index_column"] if e["index_column"] != "missing" else "nothing"
    kw: dict = {"enforce_case": e["enforce_case"]}
    if e["drop_columns"] is not None:
        kw["drop_columns"] = list(e["drop_columns"])
    if e["additional_metadata"] is not None:
        kw["additional_metadata"] = dict(e["additional_metadata"])
    if e["output"] is None:
        kw["output"] = None

    def orig():
        with _work(tmp) as d:

            def run():
                res = TSV2JSON(in_file=f, index_column=idx, **kw).run()
                return {"files": _files(d), "output": _rel(res.outputs.output, d)}

            return _guard(run)

    def mine(mode):
        with _work(tmp) as d:

            def run():
                out = lnw.tsv2json(
                    f, idx, **kw, out_dir=d, **_kw(lnw.tsv2json, upstream_defects=mode)
                )
                return {"files": _files(d), "output": _rel(out, d)}

            return _guard(run)

    return _Call(orig, mine)


def _confounds_frame(c: dict):
    """An fMRIPrep-like confounds table (the expansion's and the spike regressors' input)."""
    import pandas as pd

    rng = np.random.default_rng(c["seed"])
    t = c["t"]
    cols: dict[str, Any] = {}
    for name in ("global_signal", "csf", "white_matter", "csf_wm"):
        cols[name] = 100.0 + 5.0 * _ih(rng, t)
    for k in range(c["tcc"]):
        cols[f"t_comp_cor_{k:02d}"] = 0.1 * _ih(rng, t)
    for k in range(c["acc"]):
        cols[f"a_comp_cor_{k:02d}"] = 0.1 * _ih(rng, t)
    for k in range(c["cos"]):
        cols[f"cosine{k:02d}"] = 0.1 * _ih(rng, t)
    for k in range(c["nss"]):
        cols[f"non_steady_state_outlier{k:02d}"] = (np.arange(t) == k).astype(np.float64)
    for name, scale in (("dvars", 20.0), ("std_dvars", 1.0), ("framewise_displacement", c["fd_scale"])):
        v = np.abs(scale + scale * _ih(rng, t))
        if t:
            v[0] = np.nan
        cols[name] = v
    cols["rmsd"] = np.abs(0.1 * _ih(rng, t))
    rows = _motion_rows({"seed": c["seed"] + 3, "t": t, "rot": 0.01, "trans": 0.5, "special": None}, 6)
    for j, name in enumerate(("rot_x", "rot_y", "rot_z", "trans_x", "trans_y", "trans_z")):
        cols[name] = rows[:, j]
    if c["extra"] == "integers":
        cols["volume"] = np.arange(t)
    elif c["extra"] == "booleans":
        cols["flag"] = np.arange(t) % 3 == 0
    elif c["extra"] == "text":
        cols["label"] = ["x"] * t
    return pd.DataFrame(cols)


def _by_name(out: Any) -> Any:
    """``ExpandModel``'s tables by column name (PIPE-43: niworkflows' order follows Python's
    per-process string hashes; ``check_model.tsv_by_name``)."""
    if not isinstance(out, dict):
        return out
    cm = _load("check_model")
    out["files"] = {
        k: cm.tsv_by_name(v) if k.endswith(".tsv") and isinstance(v, bytes) else v
        for k, v in out["files"].items()
    }
    return out


def _fp_expand(e, files, tmp, nt):
    from niworkflows.interfaces.confounds import ExpandModel

    import larmorx.niworkflows as lnw

    if re.search(r"\^\^?(?:\d-)?[3-9]", e["formula"]) and not _pow_is_libm():
        raise Outcome(
            SKIPPED,
            "a power above 2: numpy's float64 pow is not the C library's here (AVX-512's SVML); "
            "compared with NPY_DISABLE_CPU_FEATURES set, as check_model.py",
        )
    f = _write_frame(tmp, "confounds.tsv", _confounds_frame(e["confounds"]), e["confounds"]["format"])
    formula = e["formula"]
    return _Call(
        lambda: _by_name(
            _iface(ExpandModel, tmp, kind_only=True, confounds_file=f, model_formula=formula)
        ),
        lambda mode: _by_name(
            _lx(
                lnw.expand_model,
                tmp,
                (),
                f,
                formula,
                kind_only=True,
                **_kw(lnw.expand_model, upstream_defects=mode),
            )
        ),
        silent=("PIPE-43",),
    )


def _fp_spikes(e, files, tmp, nt):
    from niworkflows.interfaces.confounds import SpikeRegressors

    import larmorx.niworkflows as lnw

    f = _write_frame(tmp, "confounds.tsv", _confounds_frame(e["confounds"]), e["confounds"]["format"])
    kw = dict(e["kw"])
    return _Call(
        lambda: _iface(SpikeRegressors, tmp, ("confounds_file",), confounds_file=f, **kw),
        lambda mode: _lx(
            lnw.spike_regressors_node,
            tmp,
            ("confounds_file",),
            f,
            **kw,
            **_kw(lnw.spike_regressors_node, upstream_defects=mode),
        ),
        # PD-01's notice is the table's read (its values change); without ``concatenate`` only
        # the thresholded flags are written, which the read's last bits rarely move
        silent=() if kw.get("concatenate", True) else ("PD-01",),
    )


def _fp_normalize(e, files, tmp, nt):
    from niworkflows.interfaces.confounds import NormalizeMotionParams

    import larmorx.niworkflows as lnw

    f = _write(tmp, "motion.par", _motion_text(e["motion"], e["columns"]))
    return _Call(
        lambda: _iface(NormalizeMotionParams, tmp, ("out_file",), in_file=f, format=e["format"]),
        lambda mode: _lx(
            lnw.normalize_motion_params,
            tmp,
            ("out_file",),
            f,
            format=e["format"],
            **_kw(lnw.normalize_motion_params, upstream_defects=mode),
        ),
    )


# --------------------------------------------------------------------------------------------
# confounds-fmriprep: motion, masks, signals


def _xfm(e: dict, tmp: Path) -> str:
    """The head-motion transforms (ITK text, as fMRIPrep's head-motion correction writes them;
    ``check_motion``'s writer and rotations)."""
    from scipy.spatial.transform import Rotation

    cm = _load("check_motion")
    rng = np.random.default_rng(e["seed"])
    n = e["volumes"]
    scale = {"small": 1.0, "moderate": 5.0, "large": 30.0, "gimbal": 1.0}[e["degrees"]]
    params = cm.motion_series(rng, n, scale)
    if e["degrees"] == "gimbal" and n:
        p = Rotation.from_euler("XYZ", [0.1, np.pi / 2, 0.0]).as_matrix()
        params[-1] = np.concatenate([p.ravel(), [1.0, 2.0, 3.0]])
    kind = "float"
    if e["double"]:
        params = params + _ih(rng, params.shape) * 1e-9
        kind = "double"
    p = tmp / "in" / "hmc_xfm.txt"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(cm.itk_text(params, np.zeros((n, 3)), e["text"], kind), encoding="utf-8")
    return str(p)


def _cells(text: Any) -> Any:
    return [line.split("\t") for line in text.decode().splitlines()] if isinstance(text, bytes) else text


def _fp_motion(e, files, tmp, nt):
    import larmorx.confounds as lc

    release = e["release"]
    fc = _fmriprep(release)
    xfm = _xfm(e, tmp)
    ref = files["ref"]

    def table(out):
        if isinstance(out, dict):
            return _cells(next(iter(out["files"].values()), b""))
        return out

    def orig():
        return {
            "motion": table(_iface(fc.FSLMotionParams, tmp, xfm_file=xfm, boldref_file=ref)),
            "rmsd": _iface(fc.FSLRMSDeviation, tmp, xfm_file=xfm, boldref_file=ref),
        }

    def mine(mode):
        return {
            "motion": table(
                _lx(
                    lc.fsl_motion_params,
                    tmp,
                    (),
                    xfm,
                    ref,
                    fmriprep_version=release,
                    **_kw(lc.fsl_motion_params, upstream_defects=mode, n_threads=nt),
                )
            ),
            "rmsd": _lx(
                lc.fsl_rms_deviation, tmp, (), xfm, ref, upstream_defects=mode
            ),
        }

    def normalize(want, got):
        if not (isinstance(want, dict) and isinstance(got, dict)):
            return want, got, ""
        a, b = want.get("motion"), got.get("motion")
        if not (isinstance(a, list) and isinstance(b, list)):
            return want, got, ""
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            try:
                model = _load("check_motion").motion_model(xfm, ref, release)
            except Exception as exc:  # the model needs what the original read
                return want, got, f"no model: {exc!r:.80}"
        a2 = [list(r) for r in a]
        b2 = [list(r) for r in b]
        d4 = 0
        for r in range(1, min(len(a2), len(b2))):
            for k in range(3, min(len(a2[r]), len(b2[r]), 6)):
                g, c, _ = model.get((r - 1, k), (None, None, None))
                if g is None or g == c:
                    continue
                if a2[r][k] == g:
                    a2[r][k] = "<D4>"
                if b2[r][k] == c:
                    b2[r][k] = "<D4>"
                    d4 += 1
        note = f"{d4} rot_* cells glibc's atan2/hypot change (D4), the model's" if d4 else ""
        return dict(want, motion=a2), dict(got, motion=b2), note

    return _Call(orig, mine, normalize=normalize)


def _fp_masks(e, files, tmp, nt):
    import nibabel as nb

    import larmorx.confounds as lc

    cm = _load("check_masks")
    vfs = [files["gm"], files["wm"], files["csf"]]
    z = tuple(float(x) for x in nb.load(vfs[0]).header.get_zooms()[:3])
    zooms = {
        "same": z,
        "coarse": tuple(v * 1.5 for v in z),
        "anisotropic": (2.0, 2.5, 3.0),
    }[e["zooms"]]

    def contents(out):
        if isinstance(out, dict):
            masks = out["outputs"]["out_masks"]
            out = {"masks": [_content(Path(p)) if os.path.isabs(p) else p for p in masks],
                   "files": out["files"]}  # fmt: skip
        return out

    def orig():
        with _work(tmp) as d:

            def run():
                res = cm.IFACES.aCompCorMasks(
                    in_vfs=vfs, is_aseg=e["is_aseg"], bold_zooms=zooms
                ).run()
                return contents({"files": _files(d), "outputs": {"out_masks": res.outputs.out_masks}})

            return _guard(run)

    def mine(mode):
        with _work(tmp) as d:

            def run():
                r = lc.acompcor_masks_node(
                    vfs,
                    zooms,
                    is_aseg=e["is_aseg"],
                    out_dir=d,
                    **_kw(lc.acompcor_masks_node, upstream_defects=mode, n_threads=nt),
                )
                return contents({"files": _files(d), "outputs": {"out_masks": r.out_masks}})

            return _guard(run)

    return _Call(orig, mine)


def _fp_union(e, files, tmp, nt):
    import larmorx.confounds as lc

    cm = _load("check_masks")
    a, b = files["m0"], files["m1"]

    def orig():
        with _work(tmp) as d:
            return _guard(lambda: {"out": _rel(cm.UNION["_binary_union"](a, b), d), "files": _files(d)})

    def mine(mode):
        with _work(tmp) as d:
            return _guard(
                lambda: {
                    "out": _rel(
                        lc.binary_union(a, b, out_dir=d, n_threads=nt, upstream_defects=mode), d
                    ),
                    "files": _files(d),
                }
            )

    return _Call(orig, mine)


def _fp_morph(e, files, tmp, nt):
    from niworkflows.interfaces.morphology import BinaryDilation, BinarySubtraction

    from larmorx.niworkflows import morphology as lm

    if e["call"] == "dilate":
        f, r = files["m0"], e["radius"]
        return _Call(
            lambda: _iface(BinaryDilation, tmp, ("out_mask",), in_mask=f, radius=r),
            lambda mode: _lx(
                lm.binary_dilation,
                tmp,
                ("out_mask",),
                f,
                radius=r,
                n_threads=nt,
                upstream_defects=mode,
            ),
        )
    a, b = files["m0"], files["m1"]
    return _Call(
        lambda: _iface(BinarySubtraction, tmp, ("out_mask",), in_base=a, in_subtract=b),
        lambda mode: _lx(lm.binary_subtraction, tmp, ("out_mask",), a, b, n_threads=nt),
        switch=False,
    )


CLASS_NAMES = ("global_signal", "csf", "white_matter", "csf_wm", "extra")


def _fp_signals(e, files, tmp, nt):
    import nibabel as nb
    from niworkflows.interfaces.images import SignalExtraction

    from larmorx.niworkflows import images as li

    series = files["series"]
    n = e["classes"]
    if e["labels_kind"] == "label image":
        labels: Any = [files["labels"]]
    else:
        maps = [files[f"map{k}"] for k in range(n)]
        if e["labels_kind"] == "4-D maps":
            imgs = [nb.load(m) for m in maps]
            data = np.stack([np.asarray(i.dataobj) for i in imgs], axis=-1)
            p = tmp / "in" / "maps4d.nii.gz"
            p.parent.mkdir(parents=True, exist_ok=True)
            nb.Nifti1Image(data, imgs[0].affine, imgs[0].header).to_filename(p)
            labels = [str(p)]
        else:
            labels = maps
    kind = e["class_labels"]
    if kind == "short":
        names = [f"c{k}" for k in range(n)]
    elif kind == "too many":
        names = list(CLASS_NAMES[: n + 1])
    elif kind == "unicode":
        names = ["globé", "中"][:n] + [f"c{k}" for k in range(max(0, n - 2))]
    else:
        names = list(CLASS_NAMES[:n])
    thr = e["prob_thres"]
    return _Call(
        lambda: _iface(
            SignalExtraction,
            tmp,
            ("out_file",),
            in_file=series,
            label_files=labels,
            class_labels=names,
            prob_thres=thr,
        ),
        lambda mode: _lx(
            li.signal_extraction,
            tmp,
            ("out_file",),
            series,
            labels,
            names,
            prob_thres=thr,
            **_kw(li.signal_extraction, upstream_defects=mode, n_threads=nt),
        ),
    )


# --------------------------------------------------------------------------------------------

CALLS: dict[str, Callable[[dict, dict, Path, int], _Call]] = {
    # confounds-linalg
    "svd": _la_svd,
    "lstsq": _la_lstsq,
    "pinv": _la_pinv,
    "gesvd": _la_gesvd,
    "fallback_svd": _la_fallback,
    "full_rank": _la_full_rank,
    "matmul": _la_matmul,
    "reduce": _la_reduce,
    "percentile": _la_percentile,
    "fft": _la_fft,
    "crosscov": _la_crosscov,
    "ar": _la_ar,
    # confounds-nipype
    "dvars": _np_dvars,
    "dvars_node": _np_dvars_node,
    "compcor": _np_compcor,
    "regress_poly": _np_regress,
    "cosine_filter": _np_cosine,
    "tsnr": _np_tsnr,
    "nssd": _np_nssd,
    "fd": _np_fd,
    "normalize_mc": _np_normalize_mc,
    # confounds-fmriprep
    "read": _fp_read,
    "write": _fp_write,
    "ujson": _fp_ujson,
    "fmriprep_fd": _fp_fd,
    "filter_dropped": _fp_filter,
    "rename_acompcor": _fp_rename,
    "gather": _fp_gather,
    "add_header": _fp_add_header,
    "tsv2json": _fp_tsv2json,
    "expand": _fp_expand,
    "spikes": _fp_spikes,
    "normalize_motion": _fp_normalize,
    "motion": _fp_motion,
    "acompcor_masks": _fp_masks,
    "union": _fp_union,
    "signals": _fp_signals,
    "dilate": _fp_morph,
    "subtract": _fp_morph,
}


def _errors(v: Any) -> list[_Err]:
    """The errors a result holds (a call's, or its parts')."""
    if isinstance(v, _Err):
        return [v]
    if isinstance(v, dict):
        return [e for x in v.values() for e in _errors(x)]
    if isinstance(v, list | tuple):
        return [e for x in v for e in _errors(x)]
    return []


def _entries(record) -> list[str]:
    from larmorx_validation.parity import defects

    return defects.warning_entries(record)


def run_case(case: Case, checks: CheckList) -> None:
    """A random or permanent case of the three tools (payload ``("random", spec)``), larmorx's
    nibabel arithmetic following this environment's LAPACK release (as
    ``fieldmaps_b_random.run_case``), for this case only."""
    contextvars.copy_context().run(_run_case, case, checks)


def _run_case(case: Case, checks: CheckList) -> None:
    from larmorx.io.nibabel_header import use_lapack
    from larmorx_validation.difftest import inputs
    from larmorx_validation.parity import defects

    _need_kernels()
    use_lapack(_numpy_lapack())
    spec = case.payload[1]
    recipe = spec["recipe"]
    e = recipe["extra"]
    call = e["call"]
    nt = int(e.get("threads", 1))
    files = {name: str(inputs.path(k)) for name, k in spec["files"].items()}
    with tempfile.TemporaryDirectory(prefix="lx-conf-dt_") as t:
        tmp = Path(t)
        # each side its own arguments, built afresh from the recipe, in a work folder of one name
        c = CALLS[call](e, files, tmp, nt)
        with warnings.catch_warnings(record=True):
            warnings.simplefilter("always")
            want = c.original()
        with warnings.catch_warnings(record=True):
            warnings.simplefilter("always")
            got = CALLS[call](e, files, tmp, nt).larmorx("reproduce")
        note = ""
        a, b = want, got
        if c.normalize is not None:
            a, b, note = c.normalize(a, b)
        exact = _key(a) == _key(b)
        ok = exact or _key(_canon(a)) == _key(_canon(b))
        detail = ""
        if not ok:
            detail = _describe(_canon(a), _canon(b))
        else:
            if isinstance(want, _Err):
                detail = f"both refuse: {want!r}"
            if not exact:
                detail = (detail + "; " if detail else "") + "NaN payloads differ (x86-64's default NaN)"
            if note:
                detail = (detail + "; " if detail else "") + note
        checks(f"{call}: every output bit-identical", ok, value=0 if ok else 1, detail=detail[:400])
        if not c.switch:
            return
        with warnings.catch_warnings(record=True) as w3:
            warnings.simplefilter("always")
            fixed = CALLS[call](e, files, tmp, nt).larmorx("fix")
        entries = _entries(w3)
        entries = list(dict.fromkeys([*entries, *(i for x in _errors(fixed) for i in _IDS.findall(getattr(x, "full", x.text)))]))
        fixed, got = _canon(fixed), _canon(got)
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
                    other = (other + "; " if other else "") + _describe(
                        _strip(fixed), _strip(got)
                    )[:200]
        defects.add_fix_check(checks, entries, change, other_changes=other, silent_ok=c.silent)


__all__ = ["CALLS", "run_case"]
