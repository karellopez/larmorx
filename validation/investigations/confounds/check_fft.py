# SPDX-License-Identifier: Apache-2.0
"""SciPy's single-precision FFTs: larmorx against this environment's ``scipy.fftpack``.

larmorx's ``_core.scipy_fftpack`` (``larmorx_confounds::scipy_fft``: SciPy 1.15's pocketfft or
SciPy 1.18.1's ducc0, told this environment's SciPy) against ``scipy.fftpack.fft`` and ``ifft``
of one-dimensional float32 and complex64 arrays of power-of-two length, bit for bit. Sections:

- ``synthetic``: every power of two from 1 to 2^14 (``--tier full``: real arrays to 2^20,
  complex to 2^16 with ducc0 and 2^20 with pocketfft), both directions, real and complex input;
  inputs of the length, shorter (padded with zeros, as nitime pads ``T`` to ``fsize``) and
  longer (cut), contiguous, every second value, reversed (nitime's ``x[::-1]``); kinds:
  Gaussian at three scales, BOLD-like (a large offset), integers, zeros, a constant, an
  impulse, alternating signs, huge values (overflow to infinity and NaN inside the transform),
  subnormals, a NaN, an infinity of each sign, NaN with infinity; and the refusals (a length
  below one: SciPy's ``ValueError``; not a power of two; ducc0's complex lengths from 131072:
  larmorx's ``NotImplementedError``).
- ``nitime``: nitime's ``fftconvolve`` calls under nipype's DVARS on synthetic BOLD-like series
  (``T`` from 2 to 9000): ``regress_poly(0, ...)`` of nipype, float32, then ``fftpack.fft(x,
  fsize)``, ``fftpack.fft(x[::-1].conj(), fsize)`` and ``fftpack.ifft`` of numpy's complex64
  product (the product itself is relay 2e-ii's); the harness checks its reconstruction against
  ``nitime.utils.fftconvolve``.
- ``real``: the same calls on voxels of fMRIPrep 25.2.6's own DVARS inputs (the ``dvars`` node's
  ``in_file`` and ``in_mask`` in the oracle runs' working and output folders; nipype's median
  normalisation, robust SD and zero-variance removal, then ``regress_poly(0, ...)``): ds005752
  (10 volumes), ds000256 (164), ds005040; ``--tier full`` adds ds005454 (280) and ds004958
  (600). Up to 2000 voxels per run (seeded choice; all of them for small masks).

NaN results are compared bit for bit too; a case whose bits differ only in a NaN's sign or
payload is counted apart (``nan-bits``: SSE keeps the first operand's NaN, and which operand a
compiler puts first is not in the source). ``--control`` tells larmorx the other library and
counts the cases whose bits change. Run it in each environment with the worktree's extension::

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<worktree>/python python -P check_fft.py \\
        [--tier quick|full] [--sections synthetic,nitime,real] [--control] [--time]

``--time`` prints the per-call time of nitime's calls instead. Exit status 1 when any case differs (``--control``: never).
"""

from __future__ import annotations

import argparse
import os
import sys
import time
import warnings
from collections import defaultdict
from pathlib import Path

import numpy as np
import scipy
import scipy.fftpack as fftpack

from larmorx import _core

SCIPY = scipy.__version__
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
COUNTS: dict[str, list[int]] = defaultdict(lambda: [0, 0, 0])  # identical, nan-bits, differ
FAILS: list[str] = []
CONTROL = False
LIB = SCIPY
# Series lengths of the nitime section: around the powers of two, the oracle runs' T, long series.
NITIME_T = (
    2,
    3,
    5,
    10,
    33,
    64,
    129,
    164,
    240,
    256,
    257,
    280,
    513,
    588,
    600,
    1200,
    2000,
    4097,
    9000,
)


def other(v: str) -> str:
    return "1.18.1" if v.startswith("1.15") else "1.15.3"


def bits(a: np.ndarray) -> np.ndarray:
    return np.ascontiguousarray(a).view(np.uint32)


def compare(key: str, want: np.ndarray, got: np.ndarray, what: str) -> None:
    c = COUNTS[key]
    if (
        want.shape == got.shape
        and want.dtype == got.dtype
        and np.array_equal(bits(want), bits(got))
    ):
        c[0] += 1
        return
    if want.shape == got.shape and want.dtype == got.dtype:
        w, g = want.view(np.float32), got.view(np.float32)
        nan_w, nan_g = np.isnan(w), np.isnan(g)
        same = bits(w) == bits(g)
        if np.array_equal(nan_w, nan_g) and np.all(same | nan_w):
            c[1] += 1
            return
    c[2] += 1
    if not CONTROL and len(FAILS) < 40:
        FAILS.append(f"{key}: {what}")


def run_case(key: str, x: np.ndarray, n: int | None, forward: bool, what: str) -> None:
    f = fftpack.fft if forward else fftpack.ifft
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        want = f(x, n) if n is not None else f(x)
        got = _core.scipy_fftpack(x, n, forward, LIB)
    compare(key, want, got, what)


def kinds(rng: np.random.Generator, m: int) -> dict[str, np.ndarray]:
    d = {
        "gauss": rng.standard_normal(m),
        "gauss-1e3": rng.standard_normal(m) * 1e3,
        "gauss-1e-3": rng.standard_normal(m) * 1e-3,
        "bold": 1000.0 + 10.0 * rng.standard_normal(m),
        "int": rng.integers(-50, 50, m).astype(np.float64),
        "zeros": np.zeros(m),
        "const": np.full(m, 3.25),
        "impulse": np.zeros(m),
        "alternating": np.where(np.arange(m) % 2 == 0, 1.0, -1.0),
        "huge": rng.standard_normal(m) * 3e37,
        "tiny": rng.standard_normal(m) * 1e-40,
        "nan": rng.standard_normal(m),
        "inf": rng.standard_normal(m),
        "neginf": rng.standard_normal(m),
        "nan-inf": rng.standard_normal(m),
    }
    j = int(rng.integers(0, m))
    d["impulse"][j] = 1.0
    d["nan"][j] = np.nan
    d["inf"][j] = np.inf
    d["neginf"][j] = -np.inf
    d["nan-inf"][j] = np.nan
    d["nan-inf"][(j + m // 2) % m] = np.inf
    return d


def synthetic(tier: str) -> None:
    rng = np.random.default_rng(20261111)
    kmax_real = 20 if tier == "full" else 14
    # ducc0's complex transform from 131072 values is Bailey's algorithm (refused below).
    complex_kmax = min(kmax_real, 16) if LIB.startswith("1.18") else kmax_real
    for k in range(kmax_real + 1):
        n = 1 << k
        kinds_here = (
            ("gauss", "bold", "huge", "nan", "inf") if k > 16 else tuple(kinds(rng, 1).keys())
        )
        for m in sorted({n, n // 2 + 1, n + 3} - {0}):
            ks = kinds(rng, m)
            for kind in kinds_here:
                base = ks[kind]
                for forward in (True, False):
                    d = "fft" if forward else "ifft"
                    xr = base.astype(np.float32)
                    run_case(f"real {d} {kind}", xr, n, forward, f"n={n} m={m}")
                    if k <= 12 and m == n:
                        run_case(
                            f"real {d} strided",
                            base.astype(np.float32)[::-1],
                            None,
                            forward,
                            f"n={n}",
                        )
                    if k <= 12 and m == n + 3:
                        run_case(f"real {d} strided", np.repeat(xr, 2)[::2], n, forward, f"n={n}")
                    if k <= complex_kmax:
                        im = rng.standard_normal(m) * (
                            np.abs(base).max() if np.isfinite(base).all() else 1.0
                        )
                        xc = (base + 1j * im).astype(np.complex64)
                        run_case(f"complex {d} {kind}", xc, n, forward, f"n={n} m={m}")
                        if k <= 12 and m == n:
                            run_case(f"complex {d} strided", xc[::-1], None, forward, f"n={n}")
    # Refusals and errors.
    c = COUNTS["refusals"]
    for x, n, exc in (
        (np.ones(4, np.float32), 0, ValueError),
        (np.ones(4, np.float32), -2, ValueError),
        (np.ones(0, np.float32), None, ValueError),
    ):
        try:
            fftpack.fft(x, n)
            want = None
        except ValueError as e:
            want = str(e)
        try:
            _core.scipy_fftpack(x, n, True, LIB)
            got = None
        except exc as e:
            got = str(e)
        if want is not None and want == got:
            c[0] += 1
        else:
            c[2] += 1
            FAILS.append(f"refusal n={n}: scipy {want!r}, larmorx {got!r}")
    for x, n in ((np.ones(3, np.float32), None), (np.ones(4, np.float32), 6)):
        try:
            _core.scipy_fftpack(x, n, True, LIB)
            c[2] += 1
            FAILS.append(f"length {n or len(x)} not refused")
        except NotImplementedError:
            c[0] += 1
    if LIB.startswith("1.18"):
        try:
            _core.scipy_fftpack(np.ones(131072, np.complex64), None, False, LIB)
            c[2] += 1
            FAILS.append("ducc0 complex 131072 not refused")
        except NotImplementedError:
            c[0] += 1


def nitime_calls(key: str, x32: np.ndarray, check_harness: bool = False) -> None:
    """nitime's crosscov(x, x) FFTs of one float32 series."""
    t = x32.shape[0]
    size = 2 * t - 1
    fsize = 2 ** int(np.ceil(np.log2(size)))
    y = x32[::-1].conj()
    a = fftpack.fft(x32, fsize)
    b = fftpack.fft(y, fsize)
    compare(f"{key} fft(x)", a, _core.scipy_fftpack(x32, fsize, True, LIB), f"T={t}")
    compare(f"{key} fft(x[::-1])", b, _core.scipy_fftpack(y, fsize, True, LIB), f"T={t}")
    a *= b
    ret = fftpack.ifft(a)
    compare(f"{key} ifft", ret, _core.scipy_fftpack(a, None, False, LIB), f"T={t}")
    if check_harness:
        from nitime.utils import fftconvolve

        ref = fftconvolve(x32, y, axis=-1, mode="full")
        mine = ret[:size].copy().real
        if not np.array_equal(ref.view(np.uint32), mine.view(np.uint32)):
            FAILS.append(f"harness: nitime's fftconvolve is not the reconstruction (T={t})")


def nitime_section() -> None:
    from nipype.algorithms.confounds import regress_poly

    rng = np.random.default_rng(7)
    for t in NITIME_T:
        nvox = 40 if t < 2000 else 6
        walk = np.cumsum(rng.standard_normal((nvox, t)), axis=1) + 800.0
        mfunc = (walk + rng.standard_normal((nvox, t)) * 5).astype(np.float32)
        rows = regress_poly(0, mfunc, remove_mean=True)[0].astype(np.float32)
        for i, row in enumerate(rows):
            nitime_calls("nitime", np.ascontiguousarray(row), check_harness=i == 0)


def real_section(tier: str) -> None:
    import nibabel as nb
    from nipype.algorithms.confounds import regress_poly

    runs = [
        ("ds005752-sub-ON01016-noreconall", "reverse"),
        ("ds000256-sub-CTS203-noreconall", "restbaseline"),
        ("ds005040-sub-002-noreconall", "clip"),
    ]
    if tier == "full":
        runs += [
            ("ds005454-sub-16-noreconall", "rest"),
            ("ds004958-sub-fsm64ps-noreconall", "rest"),
        ]
    rng = np.random.default_rng(11)
    for run, tag in runs:
        work = RUNS / run / "work"
        wfs = [w for w in sorted(work.glob("fmriprep_25_2_wf/sub_*_wf/bold_*_wf")) if tag in w.name]
        if not wfs:
            FAILS.append(f"real: {run} {tag} not found")
            continue
        native = sorted(
            (wfs[0] / "bold_native_wf" / "boldref_bold").glob("*_xformresampled.nii.gz")
        )
        out = RUNS / run / "out"
        stem = native[0].name.split("_bold")[0]
        masks = sorted(out.rglob(f"{stem}_desc-brain_mask.nii.gz"))
        if not native or not masks:
            FAILS.append(f"real: {run} inputs missing")
            continue
        func = np.float32(nb.load(native[0]).dataobj)
        mask = np.bool_(nb.load(masks[0]).dataobj)
        idx = np.where(mask > 0)
        mfunc = func[idx[0], idx[1], idx[2], :]
        del func
        mfunc = (mfunc / np.median(mfunc)) * 1000
        func_sd = (
            np.percentile(mfunc, 75, axis=1, method="lower")
            - np.percentile(mfunc, 25, axis=1, method="lower")
        ) / 1.349
        mfunc = mfunc[func_sd > 1e-7, :]
        pick = rng.choice(mfunc.shape[0], min(2000, mfunc.shape[0]), replace=False)
        rows = regress_poly(0, mfunc[np.sort(pick)], remove_mean=True)[0].astype(np.float32)
        for row in rows:
            nitime_calls(f"real {run.split('-')[0]}", np.ascontiguousarray(row))


def timing() -> None:
    """Per-call time of nitime's forward and backward FFTs: SciPy (its plan cache warm) against
    larmorx's binding (which builds the plan at every call; the confounds' own loop reuses it)."""
    rng = np.random.default_rng(3)
    print("timing: n, scipy fft, larmorx fft, scipy ifft, larmorx ifft (microseconds per call)")
    for n in (32, 512, 1024, 2048, 16384):
        x = rng.standard_normal(n // 2).astype(np.float32)
        z = fftpack.fft(x, n)
        reps = max(20, 200_000 // n)
        row = []
        for f in (
            lambda x=x, n=n: fftpack.fft(x, n),
            lambda x=x, n=n: _core.scipy_fftpack(x, n, True, LIB),
            lambda z=z: fftpack.ifft(z),
            lambda z=z: _core.scipy_fftpack(z, None, False, LIB),
        ):
            f()
            t = time.perf_counter()
            for _ in range(reps):
                f()
            row.append((time.perf_counter() - t) / reps * 1e6)
        print(f"  {n:6d} " + " ".join(f"{v:9.1f}" for v in row))


def main() -> int:
    global CONTROL, LIB
    ap = argparse.ArgumentParser()
    ap.add_argument("--tier", choices=("quick", "full"), default="quick")
    ap.add_argument("--sections", default="synthetic,nitime,real")
    ap.add_argument("--control", action="store_true")
    ap.add_argument("--time", action="store_true", help="only the timing")
    args = ap.parse_args()
    CONTROL = args.control
    LIB = other(SCIPY) if CONTROL else SCIPY
    secs = args.sections.split(",")
    if args.time:
        timing()
        return 0
    t0 = time.time()
    print(f"scipy {SCIPY} numpy {np.__version__} larmorx told {LIB} tier {args.tier}", flush=True)
    if "synthetic" in secs:
        synthetic(args.tier)
    if "nitime" in secs:
        nitime_section()
    if "real" in secs:
        real_section(args.tier)
    tot = [0, 0, 0]
    for key in sorted(COUNTS):
        c = COUNTS[key]
        tot = [a + b for a, b in zip(tot, c, strict=True)]
        print(f"  {key:34s} identical {c[0]:6d}  nan-bits {c[1]:4d}  differ {c[2]:6d}")
    print(
        f"TOTAL identical {tot[0]} nan-bits {tot[1]} differ {tot[2]} "
        f"({time.time() - t0:.1f} s){' CONTROL' if CONTROL else ''}"
    )
    for f in FAILS:
        print("FAIL", f)
    if CONTROL:
        return 0
    return 1 if (tot[2] or FAILS) else 0


if __name__ == "__main__":
    sys.exit(main())
