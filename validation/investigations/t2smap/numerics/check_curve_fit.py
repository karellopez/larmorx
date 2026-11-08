# SPDX-License-Identifier: Apache-2.0
"""SciPy's ``curve_fit`` as tedana's ``t2smap --fittype curvefit`` calls it, against larmorx's.

Writes seeded cases with SciPy's and numpy's own results into ``--work``, runs the Rust program
``curve_fit_check`` (``cargo build --release -p larmorx-optim --example curve_fit_check``) on
them and compares bit for bit:

- ``dot``, ``matvec``: ``numpy.dot`` of vectors (lengths around the Haswell kernel's 16) and
  of C- and F-contiguous matrices on the shapes ``least_squares`` gives it (``m × 2``, ``2 × m``,
  ``2 × 2``);
- ``svd``: ``scipy.linalg.svd(a, full_matrices=False)`` of ``m × 2`` matrices (2 to 4099 rows,
  scaled below SMLNUM and above BIGNUM, rank one, zero columns, signed zeros) and
  ``numpy.linalg.svd`` of the same (numpy's own LAPACK);
- ``fit``: ``curve_fit(monoexponential, te, y, p0=p0, bounds=((min(y), 0), (inf, inf)))``
  on tedana-like voxels (2 to 5 echoes, 1 to 600 volumes, float32 signal, the log-linear
  start), and the edge cases of each exit (a start outside the bounds or on them, a value
  that is not finite, a flat or rising signal, tiny and huge T2*, two residuals, a perfect
  fit, negative and zero signal, the evaluation budget exhausted);
- ``real``: the calls tedana's ``t2smap`` itself made (``--real``: folders written by
  ``dump_curve_fit_calls.py``), with its own results.

Run it with numpy's AVX-512 kernels off and OpenBLAS's Haswell kernels (D2)::

    NPY_DISABLE_CPU_FEATURES="X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL \\
        AVX512_ICL AVX512_SPR" OPENBLAS_CORETYPE=Haswell <env>/bin/python -P check_curve_fit.py \\
        --checker <target>/release/examples/curve_fit_check --work <empty dir> [--real <dir>]

The output is one line per group (records, identical) and the first differing records.
"""

from __future__ import annotations

import argparse
import struct
import subprocess
import sys
import warnings
from pathlib import Path

import numpy as np
import scipy
import scipy.linalg
import scipy.optimize
from threadpoolctl import threadpool_limits

TES = {
    2: [12.0, 28.0],
    3: [14.0, 29.96, 45.92],
    4: [12.0, 28.0, 44.0, 60.0],
    5: [10.0, 20.0, 30.0, 40.0, 50.0],
}


def monoexponential(tes, s0, t2star):
    return s0 * np.exp(-tes / t2star)


def scipy_code() -> int:
    major, minor = (int(v) for v in scipy.__version__.split(".")[:2])
    return 0 if (major, minor) >= (1, 18) else 1


def numpy_release_code() -> int:
    major, minor = (int(v) for v in np.__version__.split(".")[:2])
    return 1 if (major, minor) >= (2, 5) else 0


def f64(v) -> bytes:
    return np.ascontiguousarray(v, dtype="<f8").tobytes()


def fit_result(x, y, p0, lb, ub) -> bytes:
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            popt, pcov, info, _msg, ier = scipy.optimize.curve_fit(
                monoexponential, x, y, p0=p0, bounds=(lb, ub), full_output=True
            )
        except (RuntimeError, ValueError) as e:
            return b"\x00" + f"{type(e).__name__}: {e}".encode()
    warn = any(issubclass(w.category, scipy.optimize.OptimizeWarning) for w in caught)
    return (
        b"\x01"
        + f64(popt)
        + f64(np.asarray(pcov).reshape(4))
        + bytes([warn])
        + struct.pack("<Ii", int(info["nfev"]), int(ier))
    )


def svd_result(a, which) -> bytes:
    try:
        if which == "scipy":
            u, s, vt = scipy.linalg.svd(a, full_matrices=False)
        else:
            u, s, vt = np.linalg.svd(a, full_matrices=False)
    except (ValueError, np.linalg.LinAlgError) as e:
        return b"\x00" + f"{type(e).__name__}: {e}".encode()
    return b"\x01" + f64(np.asarray(u).T.reshape(-1)) + f64(s) + f64(np.asarray(vt).T.reshape(-1))


class Cases:
    def __init__(self) -> None:
        self.records: list[bytes] = []
        self.expected: list[bytes] = []
        self.groups: list[str] = []

    def add(self, group: str, record: bytes, expected: bytes) -> None:
        self.records.append(record)
        self.expected.append(expected)
        self.groups.append(group)

    def fit(self, group, x, y, p0, lb, ub) -> None:
        x = np.asarray(x, dtype=np.float64)
        rec = (
            b"\x00"
            + struct.pack("<I", len(x))
            + f64(x)
            + f64(np.asarray(y, dtype=np.float64))
            + f64(p0)
            + f64(lb)
            + f64(ub)
        )
        self.add(group, rec, fit_result(x, y, p0, lb, ub))

    def svd(self, group, a, numpy=True) -> None:
        a = np.asarray(a, dtype=np.float64)
        body = struct.pack("<I", a.shape[0]) + f64(a.T.reshape(-1))
        self.add(group, b"\x01" + body, svd_result(a, "scipy"))
        if numpy:
            self.add(group + "-numpy", b"\x02" + body, svd_result(a, "numpy"))

    def dot(self, group, x, y) -> None:
        rec = b"\x03" + struct.pack("<I", len(x)) + f64(x) + f64(y)
        self.add(group, rec, f64([np.dot(x, y)]))

    def matvec(self, group, a, x) -> None:
        layout = 0 if a.flags["C_CONTIGUOUS"] else 1
        buf = a.reshape(-1, order="C" if layout == 0 else "F")
        rec = b"\x04" + struct.pack("<IIB", a.shape[0], a.shape[1], layout) + f64(buf) + f64(x)
        self.add(group, rec, f64(a.dot(x)))

    def raw(self, group, record: bytes, expected: bytes) -> None:
        self.add(group, record, expected)

    def write(self, path: Path) -> None:
        head = struct.pack("<IBB", len(self.records), scipy_code(), numpy_release_code())
        path.write_bytes(head + b"".join(self.records))


def loglin_start(x, y):
    """The log-linear start of one voxel (tedana's design and transform)."""
    design = np.column_stack([np.ones_like(x), -x])
    beta = np.linalg.lstsq(design, np.log(np.abs(y) + 1), rcond=None)[0]
    return np.exp(beta[0]), 1 / beta[1]


def voxel(rng, tes, vols, s0, t2, noise):
    x = np.repeat(np.asarray(tes, dtype=np.float64), vols)
    y = s0 * np.exp(-x / t2) * (1 + noise * rng.standard_normal(len(x)))
    return x, y.astype(np.float32)


def blas_cases(c: Cases, rng) -> None:
    for n in (1, 2, 3, 15, 16, 17, 31, 32, 33, 47, 600, 602, 3002):
        c.dot("dot", rng.standard_normal(n) * 100, rng.standard_normal(n))
    # (numpy treats an m x 1 or 1 x n operand as a vector: not a shape least_squares makes.)
    for m in (2, 3, 4, 5, 7, 8, 9, 600, 602, 2049, 3002):
        a = rng.standard_normal((m, 2)) * 10
        c.matvec("matvec", np.ascontiguousarray(a), rng.standard_normal(2))
        c.matvec("matvec", np.ascontiguousarray(a.T), rng.standard_normal(m))
        c.matvec("matvec", np.asfortranarray(a.T), rng.standard_normal(m))
    for _ in range(4):
        a = rng.standard_normal((2, 2))
        c.matvec("matvec", np.ascontiguousarray(a), rng.standard_normal(2))
        c.matvec("matvec", np.asfortranarray(a), rng.standard_normal(2))


def svd_cases(c: Cases, rng) -> None:
    for m in (2, 3, 4, 5, 6, 7, 15, 16, 17, 18, 33, 202, 602, 2048, 2049, 2050, 3002, 4099):
        for scale in (1.0, 1e-3, 1e5):
            c.svd("svd", rng.standard_normal((m, 2)) * scale * rng.uniform(0.1, 10, 2))
    a = rng.standard_normal((40, 2))
    c.svd("svd", a * 1e-150)
    c.svd("svd", a * 1e150)
    c.svd("svd", np.column_stack([a[:, 0], 2 * a[:, 0]]))
    c.svd("svd", np.column_stack([a[:, 0], np.zeros(40)]))
    c.svd("svd", np.column_stack([np.zeros(40), a[:, 0]]))
    c.svd("svd", np.zeros((40, 2)))
    b = a.copy()
    b[1:, 0] = 0
    c.svd("svd", b)
    b = a.copy()
    b[2:, 1] = 0
    c.svd("svd", b)
    c.svd("svd", -np.abs(a))
    c.svd("svd", np.array([[1.0, 0.0], [0.0, 1.0]]))
    c.svd("svd", np.array([[0.0, -0.0], [-0.0, 3.0], [0.0, 0.0]]))
    # tedana's augmented Jacobian shape: [J * d; diag(sqrt(diag_h))].
    x = np.repeat(np.asarray(TES[3]), 200)
    j = np.column_stack([np.exp(-x / 30), 800 * x / 900 * np.exp(-x / 30)])
    c.svd("svd", np.vstack([j * [0.3, 2.0], np.diag([0.0, 0.7])]))
    c.svd("svd", np.vstack([j, np.diag([0.0, 0.0])]))
    bad = a.copy()
    bad[3, 1] = np.inf
    c.svd("svd", bad, numpy=False)
    bad[3, 1] = np.nan
    c.svd("svd", bad, numpy=False)


def fit_cases(c: Cases, rng) -> None:
    inf2 = (np.inf, np.inf)
    for tes in TES.values():
        for vols in (1, 2, 5, 50, 200, 600):
            for noise in (0.005, 0.05, 0.3):
                for _ in range(2):
                    s0 = rng.uniform(50, 5000)
                    t2 = rng.uniform(5, 150)
                    x, y = voxel(rng, tes, vols, s0, t2, noise)
                    p0 = loglin_start(x, y.astype(np.float64))
                    c.fit("fit", x, y, p0, (np.min(y), 0), inf2)
    # Exits and odd voxels (3 echoes, 50 volumes unless said).
    x, y = voxel(rng, TES[3], 50, 1000.0, 30.0, 0.02)
    lo = float(np.min(y))
    c.fit("edge", x, y, (lo - 1, 30.0), (lo, 0), inf2)  # s0 below the bound
    c.fit("edge", x, y, (1000.0, -5.0), (lo, 0), inf2)  # negative T2*
    c.fit("edge", x, y, (lo, 30.0), (lo, 0), inf2)  # on the lower bound
    c.fit("edge", x, y, (1000.0, 0.0), (lo, 0), inf2)  # T2* on its bound
    c.fit("edge", x, y, (1000.0, np.inf), (lo, 0), inf2)  # infinite start
    c.fit("edge", x, y, (np.inf, 30.0), (lo, 0), inf2)  # residuals not finite at the start
    tiny = (np.float32(3e-18) * np.exp(-x / 30)).astype(np.float32)
    c.fit("edge", x, tiny, loglin_start(x, tiny.astype(float)), (float(tiny.min()), 0), inf2)
    c.fit("edge", x, y, (np.nan, 30.0), (lo, 0), inf2)
    c.fit("edge", x, y, (1e30, 30.0), (lo, 0), inf2)  # huge start
    c.fit("edge", x, y, (1000.0, 1e-300), (lo, 0), inf2)  # tiny start
    for bad in (np.nan, np.inf, -np.inf):
        yb = y.copy()
        yb[7] = bad
        c.fit("edge", x, yb, (1000.0, 30.0), (lo, 0), inf2)
    flat = np.full_like(y, 700.0)
    c.fit("edge", x, flat, (700.0, 1e6), (700.0, 0), inf2)
    c.fit("edge", x, flat, loglin_start(x, flat.astype(float)), (700.0, 0), inf2)
    rising = (100.0 * np.exp(x / 40)).astype(np.float32)
    p0 = loglin_start(x, rising.astype(float))
    c.fit("edge", x, rising, p0, (float(rising.min()), 0), inf2)
    c.fit("edge", x, rising, (float(rising.max()), 30.0), (float(rising.min()), 0), inf2)
    exact = (900.0 * np.exp(-x / 25.0)).astype(np.float32)
    c.fit("edge", x, exact, (900.0, 25.0), (float(exact.min()), 0), inf2)  # perfect fit?
    c.fit("edge", x, exact, (800.0, 20.0), (float(exact.min()), 0), inf2)
    for t2 in (0.05, 0.5, 2.0, 1e4, 1e7):
        xt, yt = voxel(rng, TES[3], 20, 800.0, t2, 0.02)
        lo_t = float(np.min(yt))
        c.fit("edge", xt, yt, loglin_start(xt, yt.astype(float)), (lo_t, 0), inf2)
        c.fit("edge", xt, yt, (900.0, 30.0), (lo_t, 0), inf2)
    neg = y - np.float32(1200.0)
    c.fit("edge", x, neg, (abs(float(neg.min())) + 10, 30.0), (float(neg.min()), 0), inf2)
    zeros = np.zeros_like(y)
    c.fit("edge", x, zeros, (1.0, 30.0), (0.0, 0), inf2)
    c.fit("edge", x, zeros, (0.0, 30.0), (0.0, 0), inf2)
    sub = (np.float32(1e-38) * np.exp(-x / 30)).astype(np.float32)
    c.fit("edge", x, sub, (2e-38, 30.0), (float(sub.min()), 0), inf2)
    c.fit("edge", x, sub, (2e-38, 1e-3), (float(sub.min()), 0), inf2)
    big = (np.float32(3e38) * np.exp(-x / 30)).astype(np.float32)
    c.fit("edge", x, big, (3.2e38, 30.0), (float(big.min()), 0), inf2)
    for tes in (TES[2], TES[3]):
        x2, y2 = voxel(rng, tes[:2], 1, 900.0, 30.0, 0.05)  # two residuals
        c.fit("edge", x2, y2, loglin_start(x2, y2.astype(float)), (float(y2.min()), 0), inf2)
    x3, y3 = voxel(rng, TES[3], 1, 900.0, 30.0, 0.05)  # three residuals
    c.fit("edge", x3, y3, loglin_start(x3, y3.astype(float)), (float(y3.min()), 0), inf2)
    # Noisy voxels with far starts (many iterations, reflections, the budget).
    for _ in range(60):
        xs, ys = voxel(rng, TES[4], 30, rng.uniform(100, 3000), rng.uniform(3, 300), 0.5)
        lo_s = float(np.min(ys))
        start = (lo_s + abs(lo_s) * rng.uniform(0, 3) + 1, rng.uniform(1e-3, 1e4))
        c.fit("edge", xs, ys, start, (lo_s, 0), inf2)


def read_real(c: Cases, folder: Path) -> None:
    name = folder.name
    rec = (folder / "records.bin").read_bytes()
    exp = (folder / "expected.bin").read_bytes()
    kinds = (folder / "kinds.txt").read_text(encoding="utf-8").split()
    lens = [int(v) for v in (folder / "lengths.txt").read_text(encoding="utf-8").split()]
    at = 0
    eat = 0
    for kind, n in zip(kinds, lens, strict=True):
        record = rec[at : at + n]
        at += n
        (m,) = struct.unpack("<I", exp[eat : eat + 4])
        expected = exp[eat + 4 : eat + 4 + m]
        eat += 4 + m
        c.raw(f"real-{name}" if kind == "fit" else f"real-{name}-x", record, expected)


def describe(blob: bytes) -> str:
    if not blob:
        return "-"
    if blob[0] == 1 and len(blob) == 1 + 48 + 1 + 8:
        popt = np.frombuffer(blob[1:17], "<f8")
        pcov = np.frombuffer(blob[17:49], "<f8")
        nfev, ier = struct.unpack("<Ii", blob[50:58])
        return f"popt {popt.tolist()} pcov {pcov.tolist()} warn {blob[49]} nfev {nfev} ier {ier}"
    if blob[0] == 0:
        return blob[1:].decode(errors="replace")
    return f"{np.frombuffer(blob[(len(blob) % 8) :], '<f8')[:8].tolist()} ..."


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--checker", required=True)
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--real", type=Path, action="append", default=[])
    ap.add_argument("--threads", type=int, default=8)
    ap.add_argument("--only-real", action="store_true")
    ap.add_argument("--show", type=int, default=8)
    args = ap.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(20261108)
    c = Cases()
    with threadpool_limits(limits=1, user_api="blas"):
        if not args.only_real:
            blas_cases(c, rng)
            svd_cases(c, rng)
            fit_cases(c, rng)
    for folder in args.real:
        read_real(c, folder)
    cases = args.work / "cases.bin"
    out = args.work / "out.bin"
    c.write(cases)
    print(
        f"numpy {np.__version__}, SciPy {scipy.__version__}: {len(c.records)} records",
        flush=True,
    )
    subprocess.run([args.checker, str(cases), str(out), str(args.threads)], check=True)
    blob = out.read_bytes()
    got = []
    at = 0
    while at < len(blob):
        (n,) = struct.unpack("<I", blob[at : at + 4])
        got.append(blob[at + 4 : at + 4 + n])
        at += 4 + n
    assert len(got) == len(c.expected), (len(got), len(c.expected))
    stats: dict[str, list[int]] = {}
    shown = 0
    failures = 0
    for k, (group, want, have) in enumerate(zip(c.groups, c.expected, got, strict=True)):
        s = stats.setdefault(group, [0, 0])
        s[0] += 1
        if want == have:
            s[1] += 1
            continue
        failures += 1
        if shown < args.show:
            shown += 1
            print(
                f"  DIFF {group} #{k}\n    SciPy:   {describe(want)}\n    larmorx: {describe(have)}"
            )
    for group, (n, same) in stats.items():
        print(f"{group}: {same}/{n} identical")
    # Which exits the fits reached (SciPy's results).
    outcomes: dict[str, int] = {}
    for group, want in zip(c.groups, c.expected, strict=True):
        if not want or (group.startswith(("dot", "matvec", "svd")) or group.endswith("-x")):
            continue
        if want[0] == 0:
            key = want[1:].decode(errors="replace")
        else:
            nfev, ier = struct.unpack("<Ii", want[50:58])
            key = f"ok, status {ier}" + (", covariance warning" if want[49] else "")
            if nfev == 200:
                key += ", 200 evaluations"
        outcomes[key] = outcomes.get(key, 0) + 1
    for key, n in sorted(outcomes.items(), key=lambda kv: -kv[1]):
        print(f"  outcome: {n:6d}  {key}")
    print(f"FAILURES {failures}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
