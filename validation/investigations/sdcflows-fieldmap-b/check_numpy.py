# SPDX-License-Identifier: Apache-2.0
"""numpy's percentiles, medians, ``clip`` and float32 ``exp`` against ``larmorx_core::numpy``.

The parts of numpy the field maps' part B reaches (sdcflows' ``brain_masker``: ``percentile`` of
the positive voxels with a Python-number ``q``; niworkflows' ``IntensityClip``: the same with
``p_min``/``p_max``, ``clip``; ``RobustAverage``: ``percentile`` of a 4D series, ``median`` along
the last axis of a Fortran-ordered series; scikit-image's random walker: float32 ``exp``), through
the private ``_core.numpy_*`` functions, bit for bit. Run in each kept environment (the shared
``.venv``: numpy 2.3.5; ``oracles/python-newest``: numpy 2.5.4) or in fMRIPrep's image (numpy
2.2.6), with ``PYTHONPATH=<repo>/python``:

    OPENBLAS_CORETYPE=Haswell python -P check_numpy.py [--expf]

``--expf`` adds numpy's float32 ``exp`` on all 2^32 bit patterns (64 chunk digests) on this CPU's
default path (AVX-512F on the main machine) and with AVX-512 off (AVX2/FMA3, numpy's
``NPY_DISABLE_CPU_FEATURES``), against ``expf_numpy``, and the number of numpy's results that are
not correctly rounded (``math::expf``).

Prints one line per function (cases, identical); ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

import numpy as np

from larmorx import _core

sys.path.insert(0, str(Path(__file__).resolve().parent))
import steplines  # the originals' step: numpy's outputs by digest

NO_AVX512 = "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"
REL = ".".join(np.__version__.split(".")[:2])


def same(a, b) -> bool:
    a, b = np.asarray(a), np.asarray(b)
    return a.dtype == b.dtype and a.shape == b.shape and a.tobytes() == b.tobytes()


class Tally(dict):
    def add(self, name: str, ok: bool, what: str) -> None:
        steplines.note(name)
        tot, good, first = self.get(name, (0, 0, None))
        self[name] = (tot + 1, good + int(ok), first if (ok or first) else what)


def arrays(rng):
    out = []
    for n in (1, 2, 3, 7, 8, 9, 100, 1001, 65536, 1_000_003):
        for dtype in (np.float32, np.float64):
            a = rng.normal(100.0, 40.0, n).astype(dtype)
            out.append((f"normal n={n} {np.dtype(dtype).name}", a))
            b = rng.integers(-3, 4, n).astype(dtype)  # ties and zeros
            b[b == 0] = -0.0 if n % 2 else 0.0
            out.append((f"ties n={n} {np.dtype(dtype).name}", b))
    big = rng.random((1 << 24) + 5).astype(np.float32)  # n - 1 above 2^24: numpy 2.3 rounds it
    out.append(("n=2^24+5 float32", big))
    nan = rng.random(50).astype(np.float32)
    nan[17] = np.nan
    out.append(("with NaN", nan))
    inf = rng.random(50).astype(np.float32)
    inf[3], inf[9] = np.inf, -np.inf
    out.append(("with inf", inf))
    return out


def percentiles(t: Tally, rng) -> None:
    qs = [
        0,
        15,
        35,
        35.0,
        50,
        99.98,
        99.8,
        0.2,
        0.01,
        99.9,
        100,
        100.0,
        12.345678,
        1e-5,
        100.000001,
        -0.0,
        101,
        -1,
        float("nan"),
    ]
    for desc, a in arrays(rng):
        for q in qs:
            try:
                want = steplines.oracle(np.percentile(a, q))
                werr = None
            except Exception as e:
                want, werr = None, steplines.oracle(type(e).__name__)
            try:
                got = _core.numpy_percentile_weak(a, q, REL)
                gerr = None
            except Exception as e:
                got, gerr = None, type(e).__name__
            if werr or gerr:
                t.add("percentile (Python q)", werr == gerr, f"{desc} q={q}: {werr} / {gerr}")
                continue
            g = np.asarray(got, dtype=a.dtype)
            t.add("percentile (Python q)", same(want, g), f"{desc} q={q}: {want!r} / {g!r}")
        ql = [0.0, 15.0, 50.0, 99.98, 100.0]
        want = steplines.oracle(np.percentile(a, ql))
        got = np.asarray(_core.numpy_percentile(a, ql))
        t.add("percentile (list q)", same(want, got), f"{desc}: {want} / {got}")
    # the 4D series of RobustAverage (whole-array percentile of a Fortran-ordered array)
    s = np.asfortranarray(rng.normal(500, 200, (11, 9, 7, 5)).astype(np.float32))
    for q in (0.2, 99.8):
        t.add(
            "percentile (Python q)",
            same(
                steplines.oracle(np.percentile(s, q)),
                np.asarray(_core.numpy_percentile_weak(s, q, REL), dtype=np.float32),
            ),
            f"4D q={q}",
        )


def medians(t: Tally, rng) -> None:
    for desc, a in arrays(rng):
        t.add("median", same(steplines.oracle(np.median(a)), _core.numpy_median(a)), desc)
    for shape in ((7, 6, 5, 4), (5, 4, 3, 3), (6, 5, 4, 1), (3, 3, 3, 2), (17, 13, 11, 8)):
        for order in ("C", "F"):
            a = np.asarray(rng.normal(0, 3, shape).astype(np.float32), order=order)
            a[0, 0, 0, 0] = np.nan
            a[1, 1, 1, :] = -0.0
            for axis in range(4):
                want = steplines.oracle(np.median(a, axis=axis))
                got = _core.numpy_median(a, axis)
                t.add("median (axis)", same(want, got), f"{shape} {order} axis={axis}")


def clips(t: Tally, rng) -> None:
    a = rng.normal(0, 10, 1000).astype(np.float32)
    a[:4] = [np.nan, -0.0, np.inf, -np.inf]
    for lo, hi in ((0.0, 5.0), (-3.5, 3.5), (5.0, -5.0), (np.nan, 1.0), (0.0, np.nan), (-0.0, 0.0)):
        want = steplines.oracle(np.clip(a, np.float32(lo), np.float32(hi)))
        got = _core.numpy_clip_f32(a, np.float32(lo), np.float32(hi))
        t.add("clip", same(want, got), f"[{lo}, {hi}]")


def exps(t: Tally, rng) -> None:
    for desc, x in [
        (
            "specials",
            np.array(
                [
                    0.0,
                    -0.0,
                    np.nan,
                    -np.nan,
                    np.inf,
                    -np.inf,
                    88.72283935546875,
                    88.7228,
                    -103.97208404541015625,
                    -103.972,
                    -87.3,
                    -100.0,
                    1e-30,
                    -1e-30,
                ],
                dtype=np.float32,
            ),
        ),
        ("random", rng.uniform(-110, 90, 200_000).astype(np.float32)),
        ("small", rng.uniform(-1, 1, 200_000).astype(np.float32)),
    ]:
        with np.errstate(all="ignore"):
            t.add("exp float32", same(steplines.oracle(np.exp(x)), _core.numpy_exp_f32(x)), desc)


def expf_digests(path: Path) -> None:
    lines = []
    step = 1 << 26
    with np.errstate(all="ignore"):
        for start in range(0, 1 << 32, step):
            idx = np.arange(start, start + step, dtype=np.uint64)
            bits = np.exp(idx.astype(np.uint32).view(np.float32)).view(np.uint32).astype(np.uint64)
            d = int(np.sum(bits * (2 * idx + 1), dtype=np.uint64))
            lines.append(f"{start}\t{step}\t{d}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def expf_all(t: Tally, work: Path) -> None:
    env_off = dict(os.environ, NPY_DISABLE_CPU_FEATURES=NO_AVX512)
    step = 1 << 26
    for part, env in (("default", dict(os.environ)), ("avx512off", env_off)):
        out = work / f"expf-{part}.tsv"
        subprocess.run(
            [sys.executable, "-P", __file__, "--expf-part", str(out)], env=env, check=True
        )
        diffs = 0
        for line in out.read_text(encoding="utf-8").splitlines():
            start, n, d = (int(v) for v in line.split("\t"))
            idx = np.arange(start, start + n, dtype=np.uint64)
            bits = (
                _core.numpy_exp_f32(idx.astype(np.uint32).view(np.float32))
                .view(np.uint32)
                .astype(np.uint64)
            )
            mine = int(np.sum(bits * (2 * idx + 1), dtype=np.uint64))
            diffs += int(mine != d)
        steplines.oracle(out.read_bytes())  # numpy's digests of every chunk
        t.add(
            f"exp float32 all 2^32 ({part})",
            diffs == 0,
            f"{diffs} of {(1 << 32) // step} chunks differ",
        )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--expf", action="store_true")
    ap.add_argument("--work", type=Path, default=None)
    ap.add_argument("--expf-part", type=Path, default=None)
    args = ap.parse_args()
    if args.expf_part is not None:
        expf_digests(args.expf_part)
        return 0
    print(f"numpy {np.__version__}")
    rng = np.random.default_rng(20261108)
    t = Tally()
    percentiles(t, rng)
    medians(t, rng)
    clips(t, rng)
    exps(t, rng)
    if args.expf:
        work = args.work or Path.cwd()
        work.mkdir(parents=True, exist_ok=True)
        expf_all(t, work)
    bad = 0
    for name, (tot, good, first) in sorted(t.items()):
        bad += tot - good
        print(f"{name}: {tot} cases, identical {good}")
        if first:
            print(f"   first difference: {first}")
    steplines.lines()
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
