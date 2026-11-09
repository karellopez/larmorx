# SPDX-License-Identifier: Apache-2.0
"""niworkflows' ``RegridToZooms`` (``resample_by_spacing``) and SciPy's ``correlate1d``,
``gaussian_filter1d`` and ``gaussian_filter`` against larmorx's ports
(``lx.niworkflows.regrid_to_zooms``, ``lx.ndimage``).

Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``; boxed):

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_regrid.py \\
        [--sections filters,regrid,runs] [--threads N]

numpy's float64 ``exp`` (the Gaussian kernel) takes Intel's SVML kernels on a CPU with
AVX-512 and glibc's elsewhere; larmorx follows the second path, so the script re-runs itself
with numpy's AVX-512 dispatch off (``NPY_DISABLE_CPU_FEATURES``).

- ``filters``: ``correlate1d`` (1-D to 3-D, every input type, symmetric, anti-symmetric,
  asymmetric and even kernels, origins, the eight modes, ``cval``, output types) and the
  Gaussian filters (sigmas per axis, zeros, ``truncate``, ``radius``, modes per axis, ``axes``,
  output types), seeded, bit for bit; numpy's matrix-vector product against ``a.dot(x)``
  (shapes to 33 x 21 and blocks of 2048 rows), SciPy's ``_gaussian_kernel1d`` for orders 0 to 9,
  the Gaussian derivatives in both filters; complex inputs and weights (complex64 and 128, real
  inputs with complex weights, complex ``cval``, the output rules and their warnings) and
  ``correlate1d``'s argument handling (types, ``cval``, ``origin``, ``axis``, read-only and
  big-endian outputs), the warnings compared too.
- ``regrid``: ``RegridToZooms`` on synthetic files (int16, uint8, float32 and a scaled int16;
  plumb, oblique and LPS affines; every qform/sform code pair; finer, coarser and mixed zooms;
  orders 0, 1, 3 and 5; ``clip``; ``smooth`` False, True and 0.8), the written files compared
  (data bytes and the 348 header bytes); and edge voxel sizes (zero, subnormal, NaN), the
  errors compared.
- ``runs``: fMRIPrep 25.2.6's own ``RegridToZooms`` nodes (``zooms_epi`` of the SyN runs,
  ``res_tmpl`` and ``res_target`` of the brain extraction of every run), fed the nodes' inputs,
  against their files.

Prints one line per check and ``ALL OK`` or ``FAIL <n>``.
"""

# The lambdas below run at once (inside `both` and `run`), so their loop variables are bound.
# ruff: noqa: B023

from __future__ import annotations

import os
import sys

NO_AVX512 = "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"
if os.environ.get("NPY_DISABLE_CPU_FEATURES") != NO_AVX512:  # re-run once with AVX-512 off
    os.environ["NPY_DISABLE_CPU_FEATURES"] = NO_AVX512
    os.execv(sys.executable, [sys.executable, "-P", *sys.argv])

import argparse  # noqa: E402
import tempfile  # noqa: E402
import warnings  # noqa: E402
from pathlib import Path  # noqa: E402

import nibabel as nb  # noqa: E402
import numpy as np  # noqa: E402
import scipy  # noqa: E402
from scipy import ndimage as snd  # noqa: E402

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_syn_preprocessing as csp  # noqa: E402
import check_workflows as cw  # noqa: E402
import steplines  # noqa: E402  (the originals' step: SciPy's and niworkflows' outputs)

from larmorx import ndimage as lnd  # noqa: E402
from larmorx import niworkflows as lnw  # noqa: E402

MODES = (
    "reflect",
    "grid-mirror",
    "constant",
    "grid-constant",
    "nearest",
    "mirror",
    "wrap",
    "grid-wrap",
)
TYPES = (np.float64, np.float32, np.int16, np.uint8, np.int32, np.bool_)


def same(a, b) -> bool:
    a, b = np.asarray(a), np.asarray(b)
    return a.dtype == b.dtype and a.shape == b.shape and a.tobytes() == b.tobytes()


def run(fn):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            return "ok", fn()
        except Exception as e:
            return type(e).__name__, str(e)


def both(t: cw.Tally, what: str, label: str, ref_fn, mine_fn) -> None:
    k1, v1 = steplines.oracle(run(ref_fn))
    k2, v2 = run(mine_fn)
    if k1 != k2:
        t.add(what, False, f"{label}: SciPy {k1} {v1!r:.120} / larmorx {k2} {v2!r:.120}")
    elif k1 == "ok":
        t.add(what, same(v1, v2), f"{label}: values differ")
    else:
        t.add(what, v1 == v2, f"{label}: {v1!r} / {v2!r}")


def sec_filters(t: cw.Tally, nt: int) -> None:
    rng = np.random.default_rng(20261109)
    shapes = [(17,), (9, 11), (6, 7, 5), (1, 8, 3)]
    kernels = {
        "symmetric": np.array([0.1, 0.25, 0.3, 0.25, 0.1]),
        "anti": np.array([-0.5, -0.2, 0.0, 0.2, 0.5]),
        "asymmetric": np.array([0.3, -0.1, 0.7]),
        "even": np.array([0.2, 0.5, 0.25, 0.05]),
        "one": np.array([1.5]),
        "long": rng.normal(size=21),
    }
    for shape in shapes:
        for dt in TYPES:
            base = rng.normal(40, 30, shape)
            a = (base > 40) if dt is np.bool_ else base.astype(dt)
            for kname, w in kernels.items():
                for mode in MODES:
                    for axis in range(len(shape)):
                        lw = len(w)
                        for origin in sorted({0, -(lw // 2), (lw - 1) // 2}):
                            label = (
                                f"{shape} {np.dtype(dt).name} {kname} {mode} axis {axis} o {origin}"
                            )
                            both(
                                t,
                                "correlate1d",
                                label,
                                lambda: snd.correlate1d(a, w, axis, None, mode, 3.5, origin),
                                lambda: lnd.correlate1d(
                                    a, w, axis, None, mode, 3.5, origin, n_threads=nt
                                ),
                            )
            for out in (np.float32, np.int16, np.uint8):
                both(
                    t,
                    "correlate1d output types",
                    f"{shape} {np.dtype(dt).name} -> {np.dtype(out).name}",
                    lambda: snd.correlate1d(a, kernels["symmetric"], -1, out, "reflect"),
                    lambda: lnd.correlate1d(
                        a, kernels["symmetric"], -1, out, "reflect", n_threads=nt
                    ),
                )
    a = rng.normal(size=(5, 6))
    for label, args in [
        ("no weights", (a, [])),
        ("2-D weights", (a, [[1.0]])),
        ("bad origin", (a, [1.0, 2.0, 3.0], -1, None, "reflect", 0.0, 2)),
        ("bad mode", (a, [1.0], -1, None, "bogus")),
        ("bad axis", (a, [1.0], 4)),
    ]:
        both(
            t,
            "correlate1d errors",
            label,
            lambda: snd.correlate1d(*args),
            lambda: lnd.correlate1d(*args),
        )
    sigmas = [
        0.5,
        1.0,
        1.7,
        3.0,
        (0.8, 0.0, 1.3),
        (0.0, 0.0, 0.0),
        np.array([0.25, 0.25, 0.25]),
        1e-16,
    ]
    for dt in (np.float64, np.float32, np.int16, np.uint8):
        vol = rng.normal(100, 30, (11, 9, 8)).astype(dt)
        for sg in sigmas:
            for extra in (
                {},
                {"truncate": 2.5},
                {"mode": "nearest"},
                {"mode": ("reflect", "constant", "wrap")},
                {"output": np.float32},
            ):
                label = f"{np.dtype(dt).name} sigma {sg} {extra}"
                both(
                    t,
                    "gaussian_filter",
                    label,
                    lambda: snd.gaussian_filter(vol, sg, **extra),
                    lambda: lnd.gaussian_filter(vol, sg, n_threads=nt, **extra),
                )
        for sg in (0.6, 2.2):
            for axis in (0, 1, 2):
                both(
                    t,
                    "gaussian_filter1d",
                    f"{np.dtype(dt).name} sigma {sg} axis {axis}",
                    lambda: snd.gaussian_filter1d(vol, sg, axis),
                    lambda: lnd.gaussian_filter1d(vol, sg, axis, n_threads=nt),
                )
        both(
            t,
            "gaussian_filter axes/radius",
            f"{np.dtype(dt).name}",
            lambda: snd.gaussian_filter(vol, (1.0, 2.0), axes=(0, 2), radius=(2, 3)),
            lambda: lnd.gaussian_filter(vol, (1.0, 2.0), axes=(0, 2), radius=(2, 3), n_threads=nt),
        )
    both(
        t,
        "gaussian_filter1d errors",
        "negative radius",
        lambda: snd.gaussian_filter1d(a, 1.0, radius=-1),
        lambda: lnd.gaussian_filter1d(a, 1.0, radius=-1),
    )
    sec_derivatives(t, nt)
    sec_complex(t, nt)
    sec_longdouble(t, nt)


def run_w(fn):
    """As ``run``, with the warnings' categories and messages."""
    with warnings.catch_warnings(record=True) as w:
        warnings.simplefilter("always")
        try:
            r = ("ok", fn())
        except Exception as e:
            r = (type(e).__name__, str(e))
    return r, [(x.category.__name__, str(x.message)) for x in w]


def both_w(t: cw.Tally, what: str, label: str, ref_fn, mine_fn) -> None:
    """As ``both``, the warnings compared too, and an output array's identity: SciPy returns the
    array it was given."""
    (k1, v1), w1 = steplines.oracle(run_w(ref_fn))
    (k2, v2), w2 = run_w(mine_fn)
    if k1 != k2:
        t.add(what, False, f"{label}: SciPy {k1} {v1!r:.120} / larmorx {k2} {v2!r:.120}")
    elif w1 != w2:
        t.add(what, False, f"{label}: warnings {w1} / {w2}")
    elif k1 == "ok":
        t.add(what, same(v1, v2), f"{label}: values differ")
    else:
        t.add(what, v1 == v2, f"{label}: {v1!r} / {v2!r}")


def sec_longdouble(t: cw.Tally, nt: int) -> None:
    """A ``numpy.longdouble`` sigma (relay 21): larmorx takes it as float64 (its kernel is
    SciPy's of ``float(sigma)``, bit for bit, with one warning where the long double is wider
    than float64); SciPy computes that kernel in numpy's long double (x86-64's 80-bit
    extended precision here), so the difference from SciPy's own is measured and reported."""
    from scipy.ndimage._filters import _gaussian_kernel1d as sp_kernel

    from larmorx._ndimage_filters import _gaussian_kernel1d as lx_kernel

    rng = np.random.default_rng(20261121)
    wide = np.dtype(np.longdouble).itemsize > 8
    sigmas = [0.5, 1.0, 1.5, 2.0, 0.3, 3.7, 10.0, 0.05, *rng.uniform(0.1, 12.0, 40)]
    kernels = differ = 0
    largest = 0.0
    largest_ulp = 0.0
    for sg in sigmas:
        ld = np.longdouble(sg)
        for order in range(4):
            radius = int(4.0 * float(ld) + 0.5)
            ref = steplines.oracle(sp_kernel(float(ld), order, radius))
            with warnings.catch_warnings(record=True) as w:
                warnings.simplefilter("always")
                mine = lx_kernel(ld, order, radius, np.__version__)
            t.add(
                "longdouble sigma: kernel = SciPy's of float(sigma)",
                same(ref, mine),
                f"{sg} {order}",
            )
            t.add(
                "longdouble sigma: one warning where it is wider than float64",
                len(w) == (1 if wide else 0)
                and all(x.category is UserWarning and "as float64" in str(x.message) for x in w),
                f"{sg} {order}: {[str(x.message) for x in w]}",
            )
            theirs = np.asarray(sp_kernel(ld, order, radius), dtype=np.float64)
            kernels += 1
            d = np.abs(theirs - mine)
            if (theirs != mine).any():
                differ += 1
                peak = float(np.abs(theirs).max())
                largest = max(largest, float(d.max()) / peak)
                if order == 0:  # the Gaussian's weights, in float64 units in the last place
                    ulps = np.abs(theirs - mine) / np.spacing(np.abs(theirs))
                    largest_ulp = max(largest_ulp, float(ulps.max()))
    # the filters themselves: SciPy's with float(sigma) against larmorx's with the long double
    a = rng.normal(size=(9, 11, 7))
    for sg in (np.longdouble(1.5), np.array([0.7, 2.2, 1.0], dtype=np.longdouble)):
        for order in (0, 1, 2):
            both(
                t,
                "longdouble sigma: gaussian_filter = SciPy's of float64 sigmas",
                f"{sg} {order}",
                lambda: snd.gaussian_filter(a, np.asarray(sg, dtype=np.float64), order),
                lambda: lnd.gaussian_filter(a, sg, order, n_threads=nt),
            )
        both(
            t,
            "longdouble sigma: gaussian_filter1d = SciPy's of float64 sigmas",
            f"{sg}",
            lambda: snd.gaussian_filter1d(a, float(np.ravel(sg)[0]), 1, 1),
            lambda: lnd.gaussian_filter1d(a, np.ravel(sg)[0], 1, 1, n_threads=nt),
        )
    print(
        f"INFO longdouble sigma: SciPy's kernels in {np.dtype(np.longdouble).name} against "
        f"larmorx's float64 kernels: {differ} of {kernels} differ, the largest difference "
        f"{largest:.3e} of the kernel's largest weight; the order-0 kernels' weights at most "
        f"{largest_ulp:.1f} units in the last place apart"
    )


def sec_derivatives(t: cw.Tally, nt: int) -> None:
    """numpy's matrix-vector product (``_core.numpy_matvec_f64``) against ``a.dot(x)``; SciPy's
    ``_gaussian_kernel1d`` for orders 0 to 9; the Gaussian derivatives in the filters."""
    from scipy.ndimage._filters import _gaussian_kernel1d as sp_kernel

    from larmorx import _core
    from larmorx._ndimage_filters import _gaussian_kernel1d as lx_kernel

    rng = np.random.default_rng(20261110)
    shapes = [(r, c) for r in range(1, 34) for c in range(1, 22)]
    shapes += [(5, 2047), (6, 2048), (7, 2049), (9, 2052), (3, 4099), (11, 4100), (13, 6147)]
    for r, c in shapes:
        for k in range(4):
            a = rng.normal(size=(r, c)) * np.exp2(rng.integers(-40, 40, size=(r, c)))
            x = rng.normal(size=c) * np.exp2(rng.integers(-40, 40, size=c))
            if k == 3:  # integers: exact zeros and signed zeros
                a, x = np.round(a * 4) % 7 - 3, np.round(x) % 5 - 2
                a[a == 0] = -0.0
            want = steplines.oracle(a.dot(x))
            t.add("matvec", same(want, _core.numpy_matvec_f64(a, x)), f"({r}, {c}) {k}")
    sigmas = [0.3, 0.5, 1.0, 1.7, 2.5, 4.0, 7.3, 15.0, 1, np.float32(1.3), np.float64(0.9), 2]
    for sg in sigmas:
        for order in range(7):
            for radius in (0, 1, 2, 3, 5, 8, 13, 31):
                ref, mine = (
                    steplines.oracle(run(lambda: sp_kernel(sg, order, radius))),
                    run(lambda: lx_kernel(sg, order, radius, np.__version__)),
                )
                ok = ref[0] == mine[0] and (
                    same(ref[1], mine[1]) if ref[0] == "ok" else ref[1] == mine[1]
                )
                t.add("_gaussian_kernel1d", ok, f"sigma {sg!r} order {order} radius {radius}")
    for order, radius in ((1, 3000), (2, 1500), (3, 700), (4, 250), (9, 40)):
        t.add(
            "_gaussian_kernel1d",
            same(
                steplines.oracle(sp_kernel(37.0, order, radius)),
                lx_kernel(37.0, order, radius, np.__version__),
            ),
            f"sigma 37 order {order} radius {radius}",
        )
    vol = rng.normal(100, 30, (11, 9, 8))
    line = rng.normal(size=23)
    for dt in (np.float64, np.float32, np.int16, np.uint8):
        v = vol.astype(dt)
        for order in (1, 2, 3, 4):
            for sg in (0.7, 1.6, 3.0):
                for mode in ("reflect", "constant", "nearest", "wrap"):
                    for axis in (0, 2):
                        both(
                            t,
                            "gaussian_filter1d order > 0",
                            f"{np.dtype(dt).name} order {order} sigma {sg} {mode} axis {axis}",
                            lambda: snd.gaussian_filter1d(v, sg, axis, order, mode=mode, cval=2.0),
                            lambda: lnd.gaussian_filter1d(
                                v, sg, axis, order, mode=mode, cval=2.0, n_threads=nt
                            ),
                        )
        for order in (1, 2, (0, 1, 2), (2, 0, 1), (1, 1, 1), (3, 0, 0)):
            for extra in ({}, {"output": np.float32}, {"truncate": 2.0}, {"radius": 2}):
                both(
                    t,
                    "gaussian_filter order > 0",
                    f"{np.dtype(dt).name} order {order} {extra}",
                    lambda: snd.gaussian_filter(v, (1.2, 0.8, 2.0), order, **extra),
                    lambda: lnd.gaussian_filter(v, (1.2, 0.8, 2.0), order, n_threads=nt, **extra),
                )
    for label, kw in [
        ("order 2 radius 0", {"order": 2, "radius": 0}),
        ("order 5", {"order": 5}),
        ("order 1 sigma float32", {"order": 1, "sigma": np.float32(1.3)}),
        ("order 1 sigma int", {"order": 1, "sigma": 2}),
        ("order -1", {"order": -1}),
        ("order 1.0", {"order": 1.0}),
        ("order 0.0", {"order": 0.0}),
        ("order True", {"order": True}),
        ("order 1 sigma 0", {"order": 1, "sigma": 0.0}),
        ("order 1 sigma 1e-300", {"order": 1, "sigma": 1e-300}),
    ]:
        kw = dict(kw)
        sg = kw.pop("sigma", 1.1)
        both_w(
            t,
            "gaussian_filter1d order edges",
            label,
            lambda: snd.gaussian_filter1d(line, sg, **kw),
            lambda: lnd.gaussian_filter1d(line, sg, n_threads=nt, **kw),
        )


def sec_complex(t: cw.Tally, nt: int) -> None:
    """Complex inputs and weights in ``correlate1d`` and the Gaussian filters
    (``_complex_via_real_components``, the conjugated weights, ``_get_output``'s complex
    rules), and ``correlate1d``'s argument handling (``_get_output``, the C parser's ``"d"``
    and ``"n"``, ``NI_InitLineBuffer``'s types)."""
    rng = np.random.default_rng(20261111)
    re = rng.normal(10, 20, (7, 9, 5))
    im = rng.normal(-3, 15, (7, 9, 5))
    c128 = re + 1j * im
    inputs = {
        "complex128": c128,
        "complex64": c128.astype(np.complex64),
        "float64": re,
        "float32": re.astype(np.float32),
        "int16": re.astype(np.int16),
        "bool": re > 10,
    }
    kernels = {
        "real sym": np.array([0.25, 0.5, 0.25]),
        "real asym": np.array([0.3, -0.1, 0.7, 0.2]),
        "complex128": np.array([0.2 + 0.1j, 0.5 - 0.3j, 0.25 + 0.0j, -0.1 + 0.4j, 0.05j]),
        "complex64 sym": np.array([0.1 + 0.2j, 0.6 - 0.1j, 0.1 + 0.2j], dtype=np.complex64),
        "complex anti": np.array([-0.5 + 0.5j, 0.0j, 0.5 - 0.5j]),
    }
    for iname, a in inputs.items():
        for kname, w in kernels.items():
            if not (np.iscomplexobj(a) or np.iscomplexobj(w)):
                continue
            for mode in MODES:
                for axis in (0, 1, 2):
                    for cval in (0.0, 2.5, 1.5 - 2j):
                        lw = len(w)
                        for origin in sorted({0, -(lw // 2), (lw - 1) // 2}):
                            both_w(
                                t,
                                "correlate1d complex",
                                f"{iname} {kname} {mode} axis {axis} cval {cval} o {origin}",
                                lambda: snd.correlate1d(a, w, axis, None, mode, cval, origin),
                                lambda: lnd.correlate1d(
                                    a, w, axis, None, mode, cval, origin, n_threads=nt
                                ),
                            )
    shape = c128.shape
    outs = {
        "type complex64": lambda: np.complex64,
        "type float64": lambda: np.float64,
        "type int16": lambda: np.int16,
        "str complex64": lambda: "complex64",
        "str float64": lambda: "float64",
        "array complex64": lambda: np.zeros(shape, np.complex64),
        "array complex128": lambda: np.zeros(shape, np.complex128),
        "array big-endian complex128": lambda: np.zeros(shape, ">c16"),
        "array float64": lambda: np.zeros(shape),
        "array wrong shape": lambda: np.zeros((3, 3), np.complex128),
        "read-only array": lambda: np.zeros(shape, np.complex128).copy().view(),
    }
    for iname in ("complex128", "complex64", "float32"):
        a = inputs[iname]
        w = kernels["complex128"] if iname == "float32" else kernels["real asym"]
        for oname, mk in outs.items():

            def call(f, mk=mk):
                o = mk()
                if oname == "read-only array":
                    o.flags.writeable = False
                return f(a, w, 1, o, "reflect", 0.5)

            both_w(
                t,
                "correlate1d complex output",
                f"{iname} {oname}",
                lambda: call(snd.correlate1d),
                lambda: call(lnd.correlate1d),
            )
    for iname in ("complex128", "complex64"):
        a = inputs[iname]
        for order in (0, 1, (0, 2, 1)):
            for extra in (
                {},
                {"output": np.complex64},
                {"mode": "constant", "cval": 1 + 2j},
                {"output": np.complex128, "truncate": 2.5},
            ):
                both_w(
                    t,
                    "gaussian_filter complex",
                    f"{iname} order {order} {extra}",
                    lambda: snd.gaussian_filter(a, (1.1, 0.0, 0.7), order, **extra),
                    lambda: lnd.gaussian_filter(a, (1.1, 0.0, 0.7), order, n_threads=nt, **extra),
                )
        for order in (0, 2):
            both_w(
                t,
                "gaussian_filter1d complex",
                f"{iname} order {order}",
                lambda: snd.gaussian_filter1d(a, 1.4, 1, order, output=np.complex64),
                lambda: lnd.gaussian_filter1d(a, 1.4, 1, order, output=np.complex64, n_threads=nt),
            )
    r = inputs["float64"]
    gcases = {
        "complex in, float64 out": (c128, 1.0, {"output": np.float64}),
        "complex in, float64 out, no pass": (c128, 0.0, {"output": np.float64}),
        "complex in, float64 array out": (c128, 1.0, {"output": np.zeros(shape)}),
        "real in, complex out": (r, 1.0, {"output": np.complex128}),
        "real in, complex out, no pass": (r, 0.0, {"output": np.complex64}),
        "real in, complex cval": (r, 1.0, {"cval": 1j}),
        "real in, bad mode, complex out": (r, 1.0, {"output": np.complex128, "mode": "bogus"}),
        "real in, str bool out": (r, 1.0, {"output": "bool"}),
        "real in, float16 out": (r, 1.0, {"output": np.float16}),
    }
    for label, (a, sg, kw) in gcases.items():
        both_w(
            t,
            "gaussian_filter complex errors",
            label,
            lambda: snd.gaussian_filter(a, sg, **kw),
            lambda: lnd.gaussian_filter(a, sg, **kw),
        )
    a = re[0]
    w3 = [1.0, 2.0, 1.0]
    ccases = {
        "float16 input": (a.astype(np.float16), w3, {}),
        "longdouble input": (a.astype(np.longdouble), w3, {}),
        "object input": (a.astype(object), w3, {}),
        "clongdouble input": (a.astype(np.clongdouble), w3, {}),
        "longlong input": (a.astype(np.longlong), w3, {}),
        "big-endian input": (a.astype(">f8"), w3, {}),
        "float16 output type": (a, w3, {"output": np.float16}),
        "big-endian output type": (a, w3, {"output": np.dtype(">f4")}),
        "str bool output": (a, w3, {"output": "bool"}),
        "type bool output": (a, w3, {"output": np.bool_}),
        "str object output": (a, w3, {"output": "object"}),
        "str U3 output": (a, w3, {"output": "U3"}),
        "float16 output array": (a, w3, {"output": np.zeros(a.shape, np.float16)}),
        "complex output type, real": (a, w3, {"output": np.complex128}),
        "cval complex": (a, w3, {"cval": 1j}),
        "cval numpy complex": (a, w3, {"cval": np.complex128(1.5 + 0j)}),
        "cval str": (a, w3, {"cval": "x"}),
        "cval None": (a, w3, {"cval": None}),
        "cval numpy int": (a, w3, {"cval": np.int64(3)}),
        "cval numpy str": (a, w3, {"cval": np.str_("3")}),
        "origin float": (a, w3, {"origin": 1.0}),
        "axis float": (a, w3, {"axis": 1.0}),
        "axis out of range": (a, w3, {"axis": 2}),
        "complex weights, complex cval, real input": (a, [1.0, 2j, 1.0], {"cval": 1j}),
        "complex input, no weights": (c128, [], {}),
        "complex input, bad origin": (c128, [1.0, 2.0, 3.0], {"origin": 2}),
        "complex input, bad mode": (c128, w3, {"mode": "bogus"}),
        "complex weights, 2-D": (a, [[1j]], {}),
    }
    for label, (a2, w, kw) in ccases.items():
        both_w(
            t,
            "correlate1d arguments",
            label,
            lambda: snd.correlate1d(a2, w, **kw),
            lambda: lnd.correlate1d(a2, w, **kw),
        )


def oblique(rot, zooms, t) -> np.ndarray:
    ax, ay, az = rot
    rx = np.array([[1, 0, 0], [0, np.cos(ax), -np.sin(ax)], [0, np.sin(ax), np.cos(ax)]])
    ry = np.array([[np.cos(ay), 0, np.sin(ay)], [0, 1, 0], [-np.sin(ay), 0, np.cos(ay)]])
    rz = np.array([[np.cos(az), -np.sin(az), 0], [np.sin(az), np.cos(az), 0], [0, 0, 1]])
    a = np.eye(4)
    a[:3, :3] = rx @ ry @ rz @ np.diag(zooms)
    a[:3, 3] = t
    return a


def regrid_ref(path: str, zooms, out_dir: Path, **kw) -> str:
    from niworkflows.interfaces.images import RegridToZooms

    cwd = os.getcwd()
    out_dir.mkdir(parents=True, exist_ok=True)
    os.chdir(out_dir)
    try:
        res = RegridToZooms(in_file=path, zooms=tuple(zooms), **kw).run()
    finally:
        os.chdir(cwd)
    return res.outputs.out_file


def sec_regrid(t: cw.Tally, nt: int, tmp: Path) -> None:
    rng = np.random.default_rng(7)
    affines = {
        "plumb": np.diag([1.2, 1.0, 1.5, 1.0])
        + np.array([[0, 0, 0, -30.0], [0, 0, 0, 20], [0, 0, 0, 5], [0, 0, 0, 0]]),
        "oblique": oblique([0.1, -0.05, 0.2], [1.0, 1.1, 0.9], [-40.0, 30.0, 12.0]),
        "lps": np.diag([-2.0, -2.0, 2.5, 1.0]),
    }
    data = {
        "int16": (rng.normal(500, 200, (13, 11, 9))).astype(np.int16),
        "uint8": rng.integers(0, 255, (12, 10, 9)).astype(np.uint8),
        "float32": rng.normal(100, 50, (12, 13, 8)).astype(np.float32),
        "scaled": (rng.normal(500, 200, (11, 10, 9))).astype(np.int16),
    }
    codes = [(1, 1), (0, 2), (1, 0), (0, 0)]
    zoom_sets = [(2.0, 2.0, 2.0), (0.7, 0.8, 0.9), (1.5, 0.75, 3.0)]
    i = 0
    for dname, arr in data.items():
        for aname, aff in affines.items():
            for qc, sc in codes:
                img = nb.Nifti1Image(arr, aff)
                img.header.set_qform(aff, qc)
                img.header.set_sform(aff, sc)
                if dname == "scaled":
                    img.header.set_slope_inter(0.5, 10.0)
                path = tmp / f"in_{dname}_{aname}_{qc}{sc}.nii.gz"
                img.to_filename(path)
                for zooms in zoom_sets:
                    for kw in (
                        {},
                        {"order": 1},
                        {"order": 0, "clip": False},
                        {"smooth": True},
                        {"smooth": 0.8},
                        {"order": 5},
                    ):
                        i += 1
                        if i % 3 and kw not in ({}, {"smooth": True}):
                            continue  # one in three of the rarer options, every combination of the rest
                        label = f"{dname} {aname} q{qc}s{sc} {zooms} {kw}"
                        ok_ref, ref = steplines.oracle(
                            run(lambda: regrid_ref(str(path), zooms, tmp / f"ref{i}", **kw))
                        )
                        ok_mine, mine = run(
                            lambda: (
                                lnw.regrid_to_zooms(
                                    path, zooms, out_dir=tmp / f"mine{i}", n_threads=nt, **kw
                                ).out_file
                            )
                        )
                        if ok_ref != "ok" or ok_mine != "ok":
                            t.add(
                                "RegridToZooms: errors",
                                ok_ref == ok_mine,
                                f"{label}: {ok_ref} {ref} | {ok_mine} {mine}",
                            )
                            continue
                        csp.tally(t, "RegridToZooms synthetic", label, ref, mine)
    # edge voxel sizes
    for name, pd in [("zero", 0.0), ("subnormal", 1e-40), ("nan", np.nan), ("negative", -1.0)]:
        img = nb.Nifti1Image(data["float32"], np.eye(4))
        img.header["pixdim"][2] = pd
        path = tmp / f"edge_{name}.nii.gz"
        img.to_filename(path)
        k1, v1 = steplines.oracle(
            run(lambda: regrid_ref(str(path), (2.0, 2.0, 2.0), tmp / f"ref_{name}"))
        )
        k2, v2 = run(
            lambda: (
                lnw.regrid_to_zooms(path, (2.0, 2.0, 2.0), out_dir=tmp / f"mine_{name}").out_file
            )
        )
        if k1 == k2 == "ok":
            csp.tally(t, "RegridToZooms edge voxel sizes", name, v1, v2)
        else:
            ok = k1 == k2 and (str(v1).split("\n")[0] == str(v2).split("\n")[0])
            t.add(
                "RegridToZooms edge voxel sizes",
                ok,
                f"{name}: {k1} {str(v1)[:80]!r} | {k2} {str(v2)[:80]!r}",
            )


def regrid_nodes(root: Path) -> list[Path]:
    out = []
    for run_dir in sorted(root.iterdir()):
        work = run_dir / "work"
        if not work.is_dir():
            continue
        for name in ("zooms_epi", "res_tmpl", "res_target"):
            out += sorted(p for p in work.rglob(name) if (p / f"result_{name}.pklz").exists())
    return out


def sec_runs(t: cw.Tally, nt: int, tmp: Path, root: Path) -> None:
    for k, node in enumerate(regrid_nodes(root)):
        inp = csp.inputs(node)
        if "in_file" not in inp:
            continue
        src = inp["in_file"]
        if src.startswith("/templateflow/"):  # the runs' TemplateFlow folder
            in_file = str(cw.W / "oracles" / "templateflow" / src[len("/templateflow/") :])
        else:
            in_file = csp.host(node, src)
        zooms = csp.value(node, "zooms")
        smooth = csp.value(node, "smooth")
        order = int(csp.value(node, "order"))
        clip = bool(csp.value(node, "clip"))
        ref = next(node.glob("*_regrid.nii*"), None)
        if ref is None:
            continue
        label = f"{node.relative_to(root).parts[0]}/{node.name} ({Path(in_file).name})"
        res = lnw.regrid_to_zooms(
            in_file,
            zooms,
            order=order,
            clip=clip,
            smooth=smooth,
            out_dir=tmp / f"run{k}",
            numpy_version=csp.IMAGE["numpy_version"],
            n_threads=nt,
        )
        csp.tally(t, f"fMRIPrep's {node.name}", label, ref, res.out_file)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="filters,regrid,runs")
    ap.add_argument("--threads", type=int, default=1)
    ap.add_argument("--root", type=Path, default=cw.W / "oracles" / "fmriprep-25.2.6")
    args = ap.parse_args()
    sections = set(args.sections.split(","))
    print(f"SciPy {scipy.__version__}, numpy {np.__version__}, NPY_DISABLE_CPU_FEATURES set")
    t = steplines.Noted(cw.Tally())
    with tempfile.TemporaryDirectory(prefix="lx-regrid-") as d:
        tmp = Path(d)
        if "filters" in sections:
            sec_filters(t, args.threads)
        if "regrid" in sections:
            sec_regrid(t, args.threads, tmp)
        if "runs" in sections:
            sec_runs(t, args.threads, tmp, args.root)
    bad = t.report()
    steplines.lines()
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return int(bad > 0)


if __name__ == "__main__":
    sys.exit(main())
