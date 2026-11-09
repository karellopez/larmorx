# SPDX-License-Identifier: Apache-2.0
"""scikit-image's ``ball``, ``threshold_otsu`` and ``random_walker``, numpy's ``dot`` and
``histogram``, and sdcflows' ``brain_masker``: larmorx against the originals in this
environment, byte for byte.

Run in each kept environment with ``PYTHONPATH=<repo>/python`` and ``LARMORX_WORKSPACE``:

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_skimage.py [--threads N]
        [--sections ball,dot,hist,otsu,rw,masker,clip]

- ``dot``: ``np.dot`` of float32 and float64 vectors of 0 to 300 elements and a few long ones;
- ``hist``: ``np.histogram`` with equal bins of float32/float64 arrays (constant, two-valued,
  random, wide ranges; 1 to 300 bins);
- ``otsu``: ``threshold_otsu`` of float, integer images and histograms;
- ``rw``: ``random_walker`` on seeded 2-D and 3-D images: every mode ported (``cg_j``, ``cg``,
  ``None``, ``cg_mg`` without pyamg), float32 and float64 data and spacings, multichannel data,
  integer data, negative labels (the mask path), isolated seeds, non-consecutive labels, both
  outputs, the early returns; the warnings compared too. Cases whose system runs in float64 use
  numpy's float64 ``exp``: compared only where numpy calls the C library's (no AVX-512 dispatch:
  ``NPY_DISABLE_CPU_FEATURES`` honoured by numpy 2.3; numpy 2.5.4 ignores it), counted otherwise;
- ``masker``: ``brain_masker`` on the nine ``brainextraction_wf`` inputs of fMRIPrep's oracle runs
  (``clipper_post/clipped.nii.gz``) against sdcflows' own ``brain_masker`` in this environment
  (the three files' data and header), and, with numpy 2.3's percentile and SciPy 1.15's order,
  against fMRIPrep's own ``masker`` files (fMRIPrep's image: numpy 2.2.6, SciPy 1.15.2);
- ``clip``: niworkflows' ``IntensityClip`` (``lx.niworkflows.intensity_clip``) on the same nodes'
  ``clipper_pre`` and ``clipper_post`` inputs, the written file against niworkflows' own and, with
  numpy 2.2's percentile, against fMRIPrep's own ``clipped.nii.gz``.

Prints one line per section (cases, identical), the times, and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import math
import os
import sys
import tempfile
import time
import warnings
from pathlib import Path

import numpy as np
import scipy

import larmorx.skimage as lsk
from larmorx import _core

sys.path.insert(0, str(Path(__file__).resolve().parent))
import steplines  # the originals' step: scikit-image's and numpy's outputs

NP = np.__version__
SP = scipy.__version__
REL = ".".join(NP.split(".")[:2])
RUNS = Path(os.environ.get("LARMORX_WORKSPACE", ".")) / "oracles" / "fmriprep-25.2.6"


def same(a, b) -> bool:
    a, b = np.asarray(a), np.asarray(b)
    return (
        a.dtype == b.dtype
        and a.shape == b.shape
        and np.ascontiguousarray(a).tobytes() == np.ascontiguousarray(b).tobytes()
    )


class Tally:
    def __init__(self) -> None:
        self.t: dict[str, list] = {}

    def add(self, name: str, ok: bool, what: str = "") -> None:
        steplines.note(name)
        e = self.t.setdefault(name, [0, 0, ""])
        e[0] += 1
        e[1] += int(ok)
        if not ok and not e[2]:
            e[2] = what

    def report(self) -> int:
        bad = 0
        for name, (n, ok, first) in self.t.items():
            bad += n - ok
            print(f"{name}: {n} cases, identical {ok}" + (f"  (first: {first})" if first else ""))
        return bad


def exp64_is_libm() -> bool:
    xs = np.random.default_rng(7).uniform(-700, 700, 4000)
    return all(float(v) == math.exp(float(x)) for v, x in zip(np.exp(xs), xs, strict=True))


def sec_ball(t: Tally) -> None:
    from skimage.morphology import ball

    for r in [0, 1, 2, 3, 4, 5, 6, 1.5, 2.5, 1.7, 3.2, 0.4]:
        for strict in (True, False):
            want = steplines.oracle(ball(r, strict_radius=strict))
            got = lsk.ball(r, strict_radius=strict)
            t.add("ball", same(want, got), f"r={r} strict={strict}")


def sec_dot(t: Tally) -> None:
    rng = np.random.default_rng(1)
    lengths = [*list(range(0, 300)), 1000, 4095, 10001, 100003, 1000000]
    for dt in (np.float32, np.float64):
        for n in lengths:
            x = (rng.standard_normal(n) * rng.uniform(0.1, 100)).astype(dt)
            y = (rng.standard_normal(n) * rng.uniform(0.1, 100)).astype(dt)
            want = steplines.oracle(np.dot(x, y))
            got = dt(_core.numpy_dot(x, y))
            t.add(f"np.dot {np.dtype(dt).name}", same(want, got), f"n={n}")
            if n > 0:
                t.add(
                    f"np.linalg.norm {np.dtype(dt).name}",
                    same(
                        steplines.oracle(np.linalg.norm(x)), dt(np.sqrt(dt(_core.numpy_dot(x, x))))
                    ),
                    f"n={n}",
                )


def sec_hist(t: Tally) -> None:
    rng = np.random.default_rng(2)
    arrays = []
    for dt in (np.float32, np.float64):
        arrays += [
            np.full(10, 3.5, dt),
            np.array([0, 1], dt),
            np.array([-0.0, 0.0, 1e-30], dt),
            rng.standard_normal(1000).astype(dt),
            (rng.random(70000) * 1e6).astype(dt),
            rng.uniform(-3e5, 7, 5000).astype(dt),
            np.array([1.0, 1.0 + np.finfo(dt).eps * 4], dt),
            rng.integers(0, 5, 300).astype(dt),
        ]
    for a in arrays:
        for nb in (1, 2, 3, 7, 64, 255, 256, 300):
            try:
                want = steplines.oracle(np.histogram(a, bins=nb))
            except ValueError as e:
                steplines.oracle(e)
                try:
                    _core.numpy_histogram(a, nb)
                    t.add("np.histogram", False, f"numpy raised {e}")
                except ValueError:
                    t.add("np.histogram", True)
                continue
            c, e = _core.numpy_histogram(a, nb)
            t.add(
                "np.histogram",
                same(want[0], c) and same(want[1], e),
                f"{a.dtype} n={a.size} bins={nb}",
            )


def sec_otsu(t: Tally) -> None:
    from skimage.filters import threshold_otsu

    rng = np.random.default_rng(3)
    imgs = []
    for dt in (np.float32, np.float64):
        imgs += [
            rng.random((30, 31)).astype(dt),
            np.concatenate([rng.normal(0, 1, 500), rng.normal(5, 2, 700)]).astype(dt),
            (rng.random((8, 9, 10)) ** 3).astype(dt),
            np.full((4, 4), 2.25, dt),
            np.array([0, 0, 1, 1, 1], dt),
            rng.uniform(-1e4, 1e4, 3000).astype(dt),
        ]
    for it in (np.uint8, np.int16, np.int32, np.int64):
        imgs += [rng.integers(0 if it == np.uint8 else -500, 200, 2000).astype(it)]
    for img in imgs:
        for nb in (256, 64, 2):
            want = steplines.oracle(threshold_otsu(img, nbins=nb))
            got = lsk.threshold_otsu(img, nbins=nb)
            t.add(
                "threshold_otsu",
                same(want, got) and type(want) is type(got),
                f"{img.dtype} {img.shape} nbins={nb}",
            )
    for counts, centers in [
        (np.array([0, 3, 5, 0, 2, 9, 0]), None),
        (np.array([4, 1, 1, 7]), np.array([0.5, 1.5, 2.5, 3.5], np.float32)),
        (np.array([1, 2, 3, 4, 5]), np.array([10, 20, 30, 40, 50], np.int64)),
        (rng.integers(0, 50, 100), np.linspace(0, 1, 100)),
    ]:
        hist = counts if centers is None else (counts, centers)
        want = steplines.oracle(threshold_otsu(hist=hist))
        got = lsk.threshold_otsu(hist=hist)
        t.add("threshold_otsu(hist=...)", same(want, got), f"{counts.size}")


def rw_cases(rng):
    """(name, data, labels, kwargs)."""
    cases = []

    def seeds2(shape, k=2):
        lab = np.zeros(shape, np.int32)
        flat = lab.reshape(-1)
        idx = rng.choice(flat.size, size=max(2, flat.size // 40), replace=False)
        flat[idx] = rng.integers(1, k + 1, idx.size)
        return lab

    a2 = (rng.random((30, 32)) * 0.2).astype(np.float32)
    a2[10:20, 12:22] += 1
    l2 = np.zeros((30, 32), np.int32)
    l2[3, 3] = 1
    l2[15, 16] = 2
    cases.append(("2d f32 default spacing", a2, l2, {}))
    cases.append(("2d f32 f32 spacing", a2, l2, {"spacing": np.array([1.0, 2.0], np.float32)}))
    cases.append(("2d f32 full prob", a2, l2, {"return_full_prob": True}))
    cases.append(("2d f32 cg", a2, l2, {"mode": "cg"}))
    cases.append(("2d f32 None mode", a2, l2, {"mode": None, "tol": 1e-5}))
    cases.append(("2d f32 cg_mg", a2, l2, {"mode": "cg_mg"}))
    a3 = (rng.random((20, 24, 28)) * 0.3).astype(np.float32)
    a3[5:15, 6:18, 7:20] += 1
    l3 = np.zeros(a3.shape, np.int8)
    l3[0, :, :] = 2
    l3[-1, :, :] = 2
    l3[9:11, 11:13, 12:14] = 1
    zooms = np.array([2.5, 2.5, 3.0], np.float32)
    cases.append(("3d f32 zooms", a3, l3, {"spacing": zooms, "return_full_prob": True}))
    cases.append(("3d f32 zooms labels", a3, l3, {"spacing": zooms}))
    cases.append(
        ("3d f32 zooms tol 1e-6", a3, l3, {"spacing": zooms, "tol": 1e-6, "return_full_prob": True})
    )
    cases.append(
        ("3d f32 beta 2000", a3, l3, {"spacing": zooms, "beta": 2000, "return_full_prob": True})
    )
    cases.append(("3d f32 tuple spacing", a3, l3, {"spacing": (2.5, 2.5, 3.0)}))
    cases.append(
        ("3d f64 data", a3.astype(np.float64), l3, {"spacing": zooms, "return_full_prob": True})
    )
    cases.append(
        (
            "3d f32 three labels",
            a3,
            seeds2(a3.shape, 3),
            {"spacing": zooms, "return_full_prob": True},
        )
    )
    l3n = l3.astype(np.int16) * 3
    l3n[l3n == 6] = 7
    cases.append(("3d non-consecutive labels", a3, l3n, {"spacing": zooms}))
    cases.append(
        ("3d non-consecutive labels reproduce", a3, l3n, {"spacing": zooms, "_reproduce": True})
    )
    l3m = l3.astype(np.int32).copy()
    l3m[:, :3, :] = -1
    l3m[4, 1, 5] = 1  # a seed inside the pruned region: isolated
    cases.append(
        ("3d negative labels (mask)", a3, l3m, {"spacing": zooms, "return_full_prob": True})
    )
    cases.append(
        (
            "3d negative labels (mask) reproduce",
            a3,
            l3m,
            {"spacing": zooms, "return_full_prob": True, "_reproduce": True},
        )
    )
    l3p = l3.astype(np.int32).copy()
    l3p[:, :2, :] = -1
    cases.append(
        (
            "3d negative labels, no isolated seed",
            a3,
            l3p,
            {"spacing": zooms, "return_full_prob": True},
        )
    )
    l3i = l3.astype(np.int32).copy()
    l3i[:, 0:2, :] = -1
    l3i[3, 0, 3] = 1  # an isolated seed whose neighbours are all pruned
    cases.append(("3d isolated seed", a3, l3i, {"spacing": zooms, "return_full_prob": True}))
    cases.append(
        (
            "3d isolated seed reproduce",
            a3,
            l3i,
            {"spacing": zooms, "return_full_prob": True, "_reproduce": True},
        )
    )
    u8 = (rng.random((25, 26)) * 255).astype(np.uint8)
    cases.append(("2d uint8 data", u8, l2[:25, :26], {}))
    i16 = (rng.standard_normal((25, 26)) * 3000).astype(np.int16)
    cases.append(("2d int16 data", i16, l2[:25, :26], {"return_full_prob": True}))
    mc = (rng.random((30, 32, 3)) * 0.5).astype(np.float32)
    cases.append(("2d multichannel", mc, l2, {"channel_axis": -1, "return_full_prob": True}))
    mc3 = (rng.random((3, 12, 14, 10)) * 0.5).astype(np.float32)
    cases.append(
        ("3d multichannel axis 0", mc3, seeds2((12, 14, 10)), {"channel_axis": 0, "spacing": zooms})
    )
    cases.append(("no zero labels", a2, np.ones((30, 32), np.int32), {}))
    cases.append(
        ("no zero labels full", a2, np.ones((30, 32), np.int32) * 2, {"return_full_prob": True})
    )
    allis = np.full((30, 32), -1, np.int32)
    allis[0, 0] = 1
    allis[5, 5] = 0
    cases.append(("all isolated", a2, allis, {}))
    return cases


def sec_rw(t: Tally, nt: int) -> None:
    from skimage.segmentation import random_walker

    libm = exp64_is_libm()
    rng = np.random.default_rng(4)
    skipped = 0
    for name, data, labels, kw in rw_cases(rng):
        kw = dict(kw)
        reproduce = kw.pop("_reproduce", False)
        sp = kw.get("spacing")
        f64 = (
            data.dtype not in (np.float32, np.float16)
            or kw.get("channel_axis") is not None
            or sp is None
            or np.asarray(sp).dtype != np.float32
        )
        with warnings.catch_warnings(record=True) as w1:
            warnings.simplefilter("always")
            try:
                want = random_walker(data, labels.copy(), **kw)
            except Exception as e:
                want = e
        with warnings.catch_warnings(record=True) as w2:
            warnings.simplefilter("always")
            try:
                got = lsk.random_walker(
                    data,
                    labels.copy(),
                    scipy_version=SP,
                    upstream_defects="reproduce" if reproduce else "fix",
                    n_threads=nt,
                    **kw,
                )
            except Exception as e:
                got = e
        if not isinstance(want, Exception):
            # copy=False: scikit-image marks the isolated unlabeled pixels in the caller's labels
            l1, l2 = labels.copy(), labels.copy()
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                try:
                    random_walker(data, l1, copy=False, **kw)
                    lsk.random_walker(data, l2, copy=False, scipy_version=SP,
                                      upstream_defects="reproduce" if reproduce else "fix",
                                      n_threads=nt, **kw)  # fmt: skip
                except Exception as e:
                    l1 = l2 = e
            steplines.oracle(l1)
            t.add("random_walker copy=False: the caller's labels afterwards",
                  not isinstance(l1, Exception) and same(l1, l2), name)  # fmt: skip
        msgs1 = [str(x.message) for x in w1 if "random_walker" not in str(x.filename) or True]
        msgs1 = [
            m
            for m in msgs1
            if "pyamg" in m
            or "UMFPACK" in m
            or "Conjugate" in m
            or "probability" in m
            or "Random walker" in m
            or "isolated" in m
        ]
        msgs2 = [
            str(x.message)
            for x in w2
            if not any(k in str(x.message) for k in ("SKI-01", "SKI-03", "SKI-04"))
        ]
        ski01 = any("SKI-01" in str(x.message) for x in w2)
        ski03 = any("SKI-03" in str(x.message) for x in w2)
        ski04 = any("SKI-04" in str(x.message) for x in w2)
        if (ski03 or ski04) and not reproduce and not isinstance(want, Exception):
            # corrected (rule A): scikit-image's labels mapped back to the labels given (SKI-03),
            # the seeds as (nlabels, ...) float probabilities (SKI-04)
            values = np.unique(labels)
            if ski04:
                expected = np.stack([labels == v for v in values if v > 0]).astype(np.float64)
            else:
                expected = values[np.asarray(want) + int(np.searchsorted(values, 0))]
                expected = expected.astype(labels.dtype)
            steplines.oracle(want)
            t.add(
                f"random_walker {'SKI-04' if ski04 else 'SKI-03'} corrected",
                same(expected, got),
                name,
            )
            continue
        if ski01 and not reproduce:
            # the defect changes the result: the default must differ, reproduce must agree
            t.add(
                "random_walker SKI-01 corrected (differs from scikit-image)",
                not same(want, got),
                name,
            )
            continue
        if f64 and not libm:
            skipped += 1
            continue
        if isinstance(want, Exception) or isinstance(got, Exception):
            ok = type(want) is type(got) and str(want) == str(got)
        else:
            ok = same(want, got)
        steplines.oracle(want)
        t.add("random_walker", ok, name)
        steplines.oracle(msgs1)
        t.add("random_walker warnings", msgs1 == msgs2, f"{name}: {msgs1} vs {msgs2}")
    print(
        f"random_walker: {skipped} float64 cases not compared (numpy's float64 exp is not the C library's here)"
    )


def masker_nodes() -> list[Path]:
    out = []
    for run in sorted(RUNS.iterdir()):
        out += sorted(
            run.glob("work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/*/brainextraction_wf")
        )
        out += sorted(
            run.glob(
                "work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/*/magnitude_wf/brainextraction_wf"
            )
        )
    return out


def nii_same(p1: str, p2: str) -> bool:
    import nibabel as nb

    a, b = nb.load(p1), nb.load(p2)
    return (
        same(np.asanyarray(a.dataobj), np.asanyarray(b.dataobj))
        and a.header.binaryblock == b.header.binaryblock
    )


def sec_masker(t: Tally, nt: int) -> None:
    from sdcflows.utils.tools import brain_masker as sdc_masker

    from larmorx.sdcflows import brain_masker

    times = [0.0, 0.0]
    for node in masker_nodes():
        label = "/".join(node.parts[-6:-1])
        src = node / "clipper_post" / "clipped.nii.gz"
        with tempfile.TemporaryDirectory() as tmp:
            o1 = os.path.join(tmp, "a", "clipped_mask.nii.gz")
            o2 = os.path.join(tmp, "b", "clipped_mask.nii.gz")
            os.makedirs(os.path.dirname(o1))
            os.makedirs(os.path.dirname(o2))
            t0 = time.perf_counter()
            ref = sdc_masker(str(src), o1)
            t1 = time.perf_counter()
            mine = brain_masker(
                src,
                o2,
                numpy_version=NP,
                scipy_version=SP,
                upstream_defects="reproduce",
                n_threads=nt,
            )
            t2 = time.perf_counter()
            times[0] += t1 - t0
            times[1] += t2 - t1
            for kind, a, b in zip(
                ("brain", "probseg", "mask"),
                ref,
                (mine.out_brain, mine.out_probseg, mine.out_mask),
                strict=True,
            ):
                t.add(f"brain_masker {kind} = sdcflows' in this environment", nii_same(a, b), label)
            if REL == "2.3":
                # fMRIPrep's own files (numpy 2.2.6 = 2.3's arithmetic, SciPy 1.15.2)
                o3 = os.path.join(tmp, "c", "clipped_mask.nii.gz")
                os.makedirs(os.path.dirname(o3))
                m3 = brain_masker(
                    src, o3, numpy_version="2.2.6", scipy_version="1.15.2", n_threads=nt
                )
                masker = node / "masker"
                for kind, b in (
                    ("brain", m3.out_brain),
                    ("probseg", m3.out_probseg),
                    ("mask", m3.out_mask),
                ):
                    name = os.path.basename(b)
                    want = masker / name
                    t.add(
                        f"brain_masker {kind} = fMRIPrep's file",
                        want.exists() and nii_same(str(want), b),
                        label,
                    )
    print(f"brain_masker: sdcflows {times[0]:.1f} s, larmorx {times[1]:.1f} s ({nt} threads)")


def sec_clip(t: Tally, nt: int) -> None:
    """niworkflows' IntensityClip through lx.niworkflows.intensity_clip on the nine nodes'
    clipper_pre and clipper_post inputs: the written file against niworkflows' own in this
    environment and (numpy 2.2's percentile) against fMRIPrep's own clipped.nii.gz."""
    import re

    from niworkflows.interfaces.nibabel import _advanced_clip

    from larmorx.niworkflows import intensity_clip

    for node in masker_nodes():
        label = "/".join(node.parts[-6:-1])
        for clip in ("clipper_pre", "clipper_post"):
            text = (node / clip / "_report" / "report.rst").read_text(encoding="utf-8")
            inputs = dict(
                re.findall(r"^\* (\w+) : (.*)$", text.split("Original Inputs")[-1], flags=re.M)
            )
            work = node.parents[[p.name for p in node.parents].index("work")]
            src = work.parent / inputs["in_file"].lstrip("/")
            kw = {"p_min": float(inputs["p_min"]), "p_max": float(inputs["p_max"])}
            with tempfile.TemporaryDirectory() as tmp:
                os.makedirs(os.path.join(tmp, "a"))
                ref = _advanced_clip(str(src), newpath=os.path.join(tmp, "a"), **kw)
                mine = intensity_clip(
                    src, out_dir=os.path.join(tmp, "b"), numpy_version=NP, n_threads=nt, **kw
                )
                t.add(
                    f"IntensityClip ({clip}) = niworkflows' in this environment",
                    nii_same(ref, mine.out_file),
                    label,
                )
                old = intensity_clip(
                    src, out_dir=os.path.join(tmp, "c"), numpy_version="2.2.6", n_threads=nt, **kw
                )
                t.add(
                    f"IntensityClip ({clip}) = fMRIPrep's file",
                    nii_same(str(node / clip / "clipped.nii.gz"), old.out_file),
                    label,
                )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--sections", default="ball,dot,hist,otsu,rw,masker,clip")
    args = ap.parse_args()
    print(f"numpy {NP}, SciPy {SP}")
    t = Tally()
    secs = args.sections.split(",")
    t0 = time.perf_counter()
    if "ball" in secs:
        sec_ball(t)
    if "dot" in secs:
        sec_dot(t)
    if "hist" in secs:
        sec_hist(t)
    if "otsu" in secs:
        sec_otsu(t)
    if "rw" in secs:
        sec_rw(t, args.threads)
    if "masker" in secs:
        sec_masker(t, args.threads)
    if "clip" in secs:
        sec_clip(t, args.threads)
    bad = t.report()
    steplines.lines()
    print(f"time {time.perf_counter() - t0:.1f} s")
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
