# SPDX-License-Identifier: Apache-2.0
"""numpy's (and SciPy's) results against `larmorx_core::numpy` and `larmorx_core::math`.

Run with the Python environment to check (the shared `.venv`: numpy 2.3.5 / SciPy 1.15.3;
`oracles/python-newest`: numpy 2.5.4 / SciPy 1.18.1), on Linux x86-64:

    python -P check_numerics.py --checker <numpy_check> --work <empty dir> [--logf]

It writes seeded cases (inputs, numpy's outputs) into ``--work`` and runs the Rust program
``numpy_check`` (``cargo build --release -p larmorx-core --example numpy_check``) on them:

- the reductions tedana's ``t2smap`` reaches, over the shapes and axes it uses and edge cases
  (lengths around numpy's pairwise blocks, axes of length 1, NaN, zeros, infinities):
  ``add.reduce``, ``mean``, ``var``, ``std``, ``nansum``, ``nanstd``, ``average`` with weights,
  ``percentile(method="higher")``, ``nanpercentile`` (linear), SciPy's ``scoreatpercentile``
  (``"lower"``);
- float64 ``exp`` and ``log`` with numpy's AVX-512 kernels off (``NPY_DISABLE_CPU_FEATURES``,
  the names of both numpy versions): the C library's (glibc 2.35), against ``exp_glibc`` and
  ``log_glibc``; with them on (this machine's default), the same arguments against
  ``exp_glibc``/``exp`` and ``log_glibc``/``log`` as a measurement (numpy's SVML kernels);
- with ``--logf``: numpy's float32 ``log`` on all 2^32 bit patterns (64 chunk digests), on the
  default path (AVX-512) and with AVX-512 off (AVX2/FMA3), against ``logf_numpy``; and the counts
  of differences among numpy's kernel, glibc's ``logf`` and the correctly rounded one.

The output is one line per group and function: cases, identical.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

import numpy as np

# Both numpy versions' names for the AVX-512 dispatch targets (2.5.4: X86_V4 and the two above
# it; 2.3.5: the AVX512* features). Unknown names are ignored by each version.
NO_AVX512 = "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"


class Cases:
    def __init__(self, work: Path) -> None:
        self.work = work
        self.lines: list[str] = []
        self.n = 0

    def add(self, group, func, dtype, shape, param, inputs, expected) -> None:
        self.n += 1
        cid = f"{group}/{func}/{self.n}"
        names = []
        for k, arr in enumerate(inputs):
            name = f"c{self.n}_in{k}.bin"
            np.ascontiguousarray(arr).astype(arr.dtype.newbyteorder("<")).tofile(self.work / name)
            names.append(name)
        while len(names) < 2:
            names.append("-")
        out = f"c{self.n}_out.bin"
        if isinstance(expected, bytes):
            (self.work / out).write_bytes(expected)
        else:
            (self.work / out).write_bytes(np.ascontiguousarray(expected).tobytes())
        shape_s = ",".join(str(s) for s in shape)
        self.lines.append("\t".join([cid, func, dtype, shape_s, str(param), *names, out]))

    def write(self) -> None:
        (self.work / "manifest.tsv").write_text("\n".join(self.lines) + "\n", encoding="utf-8")


def onx(shape, axis):
    """numpy's array shape and reduced axis as (outer, n, inner)."""
    axis %= len(shape)
    outer = int(np.prod(shape[:axis], dtype=np.int64))
    inner = int(np.prod(shape[axis + 1 :], dtype=np.int64))
    return (outer, shape[axis], inner)


def values(rng, shape, dtype, kind):
    n = int(np.prod(shape, dtype=np.int64))
    if kind == "mri":
        a = rng.gamma(2.0, 400.0, n)
    elif kind == "cancel":
        a = rng.standard_normal(n) * 10.0 ** rng.integers(-3, 9, n)
    elif kind == "special":
        a = rng.standard_normal(n) * 100
        pick = rng.random(n)
        a[pick < 0.05] = 0.0
        a[(pick >= 0.05) & (pick < 0.08)] = -0.0
        a[(pick >= 0.08) & (pick < 0.09)] = np.inf
        a[(pick >= 0.09) & (pick < 0.1)] = -np.inf
    else:
        a = rng.standard_normal(n) * 1000 + 50
    return a.astype(dtype).reshape(shape)


def with_nans(rng, a, frac):
    a = a.copy()
    a[rng.random(a.shape) < frac] = np.nan
    return a


def reduction_cases(c: Cases, rng) -> None:
    shapes = []
    for n in [0, 1, 2, 7, 8, 9, 15, 16, 17, 127, 128, 129, 130, 255, 256, 257, 1000, 8193, 50000]:
        shapes.append(((n,), 0))
    for v in [1, 3, 1000]:
        for t in [1, 2, 3, 9, 200]:
            shapes += [((v, t), 0), ((v, t), 1)]
    for v in [1, 50]:
        for e in [2, 3, 5, 9, 12]:
            for t in [1, 2, 200]:
                shapes += [((v, e, t), 1), ((v, e, t), 2)]
    kinds = ["normal", "mri", "cancel", "special"]
    with np.errstate(all="ignore"):
        for k, (shape, axis) in enumerate(shapes):
            kind = kinds[k % len(kinds)]
            s3 = onx(shape, axis)
            for dt, dts in ((np.float32, "f32"), (np.float64, "f64")):
                a = values(rng, shape, dt, kind)
                c.add("reduce", "sum_axis", dts, s3, 0, [a], np.add.reduce(a, axis=axis))
                c.add("reduce", "mean_axis", dts, s3, 0, [a], np.asarray(a.mean(axis=axis)))
                c.add("reduce", "std_axis", dts, s3, 0, [a], np.asarray(a.std(axis=axis)))
                if dt is np.float32:
                    c.add(
                        "reduce", "var_axis", dts, s3, 1, [a], np.asarray(a.var(axis=axis, ddof=1))
                    )
                    an = with_nans(rng, a, [0.0, 0.1, 0.6, 1.0][k % 4])
                    c.add(
                        "reduce",
                        "nansum_axis",
                        dts,
                        s3,
                        0,
                        [an],
                        np.asarray(np.nansum(an, axis=axis)),
                    )
                    c.add(
                        "reduce",
                        "nanstd_axis",
                        dts,
                        s3,
                        0,
                        [an],
                        np.asarray(np.nanstd(an, axis=axis)),
                    )


def average_cases(c: Cases, rng) -> None:
    with np.errstate(all="ignore"):
        for v, e, t in [(1, 2, 1), (5, 2, 1), (40, 3, 7), (40, 5, 200), (3, 9, 1), (3, 12, 4)]:
            a = values(rng, (v, e, t), np.float32, "mri")
            w = rng.random((v, e, t)) * 50
            if v == 40:
                w[0, :, :] = np.exp(-rng.random((e, t)) * 800)  # tiny weights, underflows
            c.add(
                "average",
                "average_axis",
                "f32",
                (v, e, t),
                0,
                [a, w],
                np.average(a, axis=1, weights=w),
            )
        a = values(rng, (4, 3, 5), np.float32, "mri")
        w = rng.random((4, 3, 5))
        w[2, :, 3] = 0.0
        try:
            np.average(a, axis=1, weights=w)
            raise AssertionError("numpy did not raise")
        except ZeroDivisionError:
            c.add("average", "average_axis", "f32", (4, 3, 5), 0, [a, w], b"ZeroDivisionError")


def percentile_cases(c: Cases, rng) -> None:
    with np.errstate(all="ignore"):
        for n in [1, 2, 3, 4, 100, 101, 5000, 52826]:
            for q in [33, 0, 50, 100, 2.5, 99.5]:
                a = values(rng, (n,), np.float32, "mri")
                if n > 3 and q == 50:
                    a[: n // 2] = a[0]  # ties
                c.add(
                    "pct",
                    "pct_higher",
                    "f32",
                    (n, 1, 1),
                    q,
                    [a],
                    np.float32(np.percentile(a, q, method="higher")),
                )
        a = values(rng, (100,), np.float32, "mri")
        a[17] = np.nan
        c.add(
            "pct",
            "pct_higher",
            "f32",
            (100, 1, 1),
            33,
            [a],
            np.float32(np.percentile(a, 33, method="higher")),
        )
        for bad_q in [101, -1]:
            try:
                np.percentile(a, bad_q, method="higher")
            except ValueError:
                c.add("pct", "pct_higher", "f32", (100, 1, 1), bad_q, [a], b"error")
        q = [0, 2, 25, 50, 75, 98, 100]
        qs = ",".join(str(x) for x in q)
        for v, t, frac, first_all_nan in [
            (50, 3, 0.0, False),
            (52826, 4, 0.3, False),
            (7, 20, 0.5, False),
            (1, 5, 0.0, False),
            (6, 5, 0.6, True),
            (300, 9, 0.95, False),
        ]:
            a = values(rng, (v, t), np.float32, "mri")
            a = with_nans(rng, a, frac)
            if first_all_nan:
                a[:, 0] = np.nan
            a[:, -1] = np.nan  # an all-NaN column that is not the first
            r = np.nanpercentile(a, q, axis=0)
            tag = b"f32:" if r.dtype == np.float32 else b"f64:"
            c.add("pct", "nanpct_linear", "f32", (v, t, 1), qs, [a], tag + r.tobytes())
        from scipy import stats

        for n in [1, 2, 3, 200, 52826, 176128]:
            for per in [99.5, 0, 50, 100]:
                a = values(rng, (n,), np.float64, "mri")
                a[rng.random(n) < 0.2] = 0.0
                if n == 200 and per == 50:
                    a[:150] = np.nan
                c.add(
                    "pct",
                    "score_lower",
                    "f64",
                    (n, 1, 1),
                    per,
                    [a],
                    np.float64(stats.scoreatpercentile(a, per, interpolation_method="lower")),
                )
        c.add(
            "pct",
            "score_lower",
            "f64",
            (0, 1, 1),
            99.5,
            [np.zeros(0)],
            np.float64(stats.scoreatpercentile(np.zeros(0), 99.5, interpolation_method="lower")),
        )
        try:
            stats.scoreatpercentile(np.ones(3), 100.5, interpolation_method="lower")
        except ValueError:
            c.add("pct", "score_lower", "f64", (3, 1, 1), 100.5, [np.ones(3)], b"error")


def exp_log_arguments(rng) -> dict[str, dict[str, np.ndarray]]:
    n = 2_000_000
    return {
        # tedana: exp(-TE/T2*) (arguments <= 0, down to underflow), exp(beta0) (S0), log(eps)
        "exp": {
            "neg800": -rng.random(n) * 800,
            "pm20": rng.uniform(-20, 20, n),
            "full": rng.uniform(-745.2, 709.8, n),
            "special": np.array(
                [0.0, -0.0, np.inf, -np.inf, np.nan, 709.782712893384, -745.1332191019411, 1e-300]
            ),
        },
        "log": {
            "0to10": rng.random(n) * 10,
            "wide": np.exp(rng.uniform(-700, 700, n)),
            "special": np.array(
                [0.0, -0.0, 1.0, np.inf, np.nan, -1.0, -np.inf, 2.220446049250313e-16, 5e-324]
            ),
        },
    }


def exp_log_part(work: Path, group: str, seed: int) -> None:
    """Writes ``<group>.tsv`` lines for numpy's float64 exp/log in this process's dispatch."""
    rng = np.random.default_rng(seed)
    c = Cases(work)
    c.n = 100_000 if group == "avx512off" else 200_000
    with np.errstate(all="ignore"):
        for name, families in exp_log_arguments(rng).items():
            for fam, x in families.items():
                y = np.exp(x) if name == "exp" else np.log(x)
                g = f"{group}-{name}-{fam}"
                if group == "avx512off":
                    c.add(
                        g,
                        "exp_glibc" if name == "exp" else "log_numpy",
                        "f64",
                        (len(x), 1, 1),
                        0,
                        [x],
                        y,
                    )
                else:
                    c.add(g, f"{name}_glibc", "f64", (len(x), 1, 1), 0, [x], y)
                    c.add(g, f"{name}_cr", "f64", (len(x), 1, 1), 0, [x], y)
    (work / f"{group}.tsv").write_text("\n".join(c.lines) + "\n", encoding="utf-8")


def logf_digests(path: Path) -> None:
    lines = []
    step = 1 << 26
    with np.errstate(all="ignore"):
        for start in range(0, 1 << 32, step):
            idx = np.arange(start, start + step, dtype=np.uint64)
            bits = np.log(idx.astype(np.uint32).view(np.float32)).view(np.uint32).astype(np.uint64)
            d = int(np.sum(bits * (2 * idx + 1), dtype=np.uint64))
            lines.append(f"{start}\t{step}\t{d}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--checker", type=Path)
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--logf", action="store_true")
    ap.add_argument("--threads", type=int, default=8)
    ap.add_argument("--seed", type=int, default=20261108)
    ap.add_argument(
        "--part", choices=["exp-default", "exp-avx512off", "logf-default", "logf-avx512off"]
    )
    args = ap.parse_args()
    work = args.work
    if args.part:  # a child process: one dispatch path
        if args.part.startswith("exp"):
            exp_log_part(work, "default" if args.part == "exp-default" else "avx512off", args.seed)
        else:
            logf_digests(work / f"{args.part}.tsv")
        return 0
    work.mkdir(parents=True, exist_ok=True)
    if any(work.iterdir()):
        sys.exit(f"{work} is not empty")
    import scipy

    print(f"numpy {np.__version__}, SciPy {scipy.__version__}, {sys.executable}")
    rng = np.random.default_rng(args.seed)
    c = Cases(work)
    reduction_cases(c, rng)
    average_cases(c, rng)
    percentile_cases(c, rng)
    c.write()
    env_off = dict(os.environ, NPY_DISABLE_CPU_FEATURES=NO_AVX512)
    me = [sys.executable, "-P", str(Path(__file__).resolve())]
    for part, env in (("exp-avx512off", env_off), ("exp-default", dict(os.environ))):
        subprocess.run(
            [*me, "--work", str(work), "--part", part, "--seed", str(args.seed)],
            env=env,
            check=True,
        )
    with (work / "manifest.tsv").open("a", encoding="utf-8") as m:
        for g in ("avx512off", "default"):
            m.write((work / f"{g}.tsv").read_text(encoding="utf-8"))
    subprocess.run([str(args.checker), "cases", str(work)], check=True)
    if args.logf:
        for part, env in (("logf-default", dict(os.environ)), ("logf-avx512off", env_off)):
            subprocess.run([*me, "--work", str(work), "--part", part], env=env, check=True)
            print(f"[{part}]", flush=True)
            subprocess.run(
                [str(args.checker), "logf", str(work / f"{part}.tsv"), str(args.threads)],
                check=True,
            )
        subprocess.run([str(args.checker), "logf-stats", str(args.threads)], check=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
