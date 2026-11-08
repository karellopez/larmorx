# SPDX-License-Identifier: Apache-2.0
"""The numpy and SciPy steps of sdcflows' ``brain_masker`` and niworkflows' ``IntensityClip`` on
fMRIPrep 25.2.6's own inputs (the oracle runs' nine ``brainextraction_wf`` nodes: seven
phase-difference magnitudes, two PEPOLAR references), larmorx against SciPy and numpy.

Run in each kept environment with ``PYTHONPATH=<repo>/python`` and ``LARMORX_WORKSPACE``:

    OPENBLAS_CORETYPE=Haswell python -P check_masker_steps.py [--threads N]

- ``IntensityClip`` (niworkflows' ``_advanced_clip``) built from ``lx.ndimage.median_filter`` and
  ``larmorx_core::numpy``'s percentile and ``clip`` (the rest is numpy's own elementwise
  arithmetic), against niworkflows' own function in this environment, and (with numpy 2.3's
  percentile arithmetic) against fMRIPrep's own ``clipper_pre/clipped.nii.gz`` and
  ``clipper_post/clipped.nii.gz`` (data and type; fMRIPrep's image: numpy 2.2.6, SciPy 1.15.2);
  how many of fMRIPrep's files numpy 2.5's percentile changes (the step);
- ``brain_masker`` up to the random walker (the 15th percentile of the positive voxels, the
  inverted image, ``grey_closing`` with ``ball(1)``, ``median_filter`` with ``ball(3)``, scikit-
  image's Otsu threshold (not ported here: the same function for both chains), the rough mask's
  ``binary_opening`` with ``ball(3)``, ``label``, ``sum``, the largest component,
  ``binary_closing`` with ``ball(5)``, the markers' ``binary_dilation`` and ``binary_erosion``
  with ``ball(5)``): every intermediate of the larmorx chain against the SciPy chain's, byte for
  byte.

Prints one line per step (cases, identical), the times, and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
import tempfile
import time
from pathlib import Path

import nibabel as nb
import numpy as np
import scipy
from niworkflows.interfaces.nibabel import _advanced_clip
from scipy import ndimage as sp
from skimage.filters import threshold_otsu

import larmorx.ndimage as lx
from larmorx import _core

REL = ".".join(np.__version__.split(".")[:2])
RUNS = Path(os.environ["LARMORX_WORKSPACE"]) / "oracles" / "fmriprep-25.2.6"


def ball(r: int) -> np.ndarray:
    g = np.arange(-r, r + 1)
    z, y, x = np.meshgrid(g, g, g, indexing="ij")
    return (x * x + y * y + z * z <= r * r).astype(np.uint8)


def same(a, b) -> bool:
    a, b = np.asarray(a), np.asarray(b)
    return (
        a.dtype == b.dtype
        and a.shape == b.shape
        and np.ascontiguousarray(a).tobytes() == np.ascontiguousarray(b).tobytes()
    )


def nodes() -> list[Path]:
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


def report_inputs(node: Path) -> dict[str, str]:
    text = (node / "_report" / "report.rst").read_text(encoding="utf-8")
    part = text.split("Original Inputs")[-1]
    return dict(re.findall(r"^\* (\w+) : (.*)$", part, flags=re.M))


def run_path(node: Path, path: str) -> Path:
    work = node.parents[[p.name for p in node.parents].index("work")]
    return work.parent / path.lstrip("/")


def advanced_clip_lx(
    in_file: Path, p_min: float, p_max: float, nt: int, rel: str = REL
) -> np.ndarray:
    img = nb.squeeze_image(nb.load(in_file))
    data = img.get_fdata(dtype="float32")
    denoised = lx.median_filter(data, footprint=ball(3), n_threads=nt)
    sel = denoised[denoised > 0]
    a_min = np.float32(_core.numpy_percentile_weak(sel, p_min, rel))
    a_max = np.float32(_core.numpy_percentile_weak(sel, p_max, rel))
    data = _core.numpy_clip_f32(data, a_min, a_max)
    data -= data.min()
    data /= data.max()
    return np.round(255 * data).astype("int16")


def masker_steps(data0: np.ndarray, use_lx: bool, nt: int) -> dict[str, np.ndarray]:
    nd_ = lx if use_lx else sp
    kw = {"n_threads": nt} if use_lx else {}
    out = {}
    data = np.pad(data0, 5)
    pos = data[data > 0]
    p15 = (
        np.float32(_core.numpy_percentile_weak(pos, 15, REL)) if use_lx else np.percentile(pos, 15)
    )
    out["p15"] = np.asarray(p15)
    data[data < p15] = 0
    data[data > 0] -= data[data > 0].min()
    datainv = -data.copy()
    datainv -= datainv.min()
    datainv /= datainv.max()
    closed = nd_.grey_closing(datainv, structure=ball(1), **kw)
    out["grey_closing ball(1)"] = closed
    denoised = nd_.median_filter(closed, footprint=ball(3), **kw)
    out["median_filter ball(3)"] = denoised
    th = threshold_otsu(denoised)
    closedbin = np.zeros_like(closed)
    closedbin[closed < th] = 1
    closedbin = nd_.binary_opening(closedbin, ball(3), **kw).astype("uint8")
    out["binary_opening ball(3)"] = closedbin
    label_im, nb_labels = nd_.label(closedbin)
    out["label"] = label_im
    sizes = nd_.sum(closedbin, label_im, range(nb_labels + 1))
    out["sum"] = sizes
    mask = sizes == sizes.max()
    closedbin = mask[label_im]
    closedbin = nd_.binary_closing(closedbin, ball(5), **kw).astype("uint8")
    out["binary_closing ball(5)"] = closedbin
    out["binary_dilation ball(5)"] = nd_.binary_dilation(closedbin, ball(5), **kw)
    out["binary_erosion ball(5)"] = nd_.binary_erosion(closedbin, structure=ball(5), **kw)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--threads", type=int, default=4)
    args = ap.parse_args()
    print(f"numpy {np.__version__}, SciPy {scipy.__version__}")
    tally: dict[str, list[int]] = {}
    first: dict[str, str] = {}
    times = {"scipy": 0.0, "larmorx": 0.0}
    steps = {"clipper_pre": 0, "clipper_post": 0}

    def add(name: str, ok: bool, what: str) -> None:
        t = tally.setdefault(name, [0, 0])
        t[0] += 1
        t[1] += int(ok)
        if not ok and name not in first:
            first[name] = what

    for node in nodes():
        label = "/".join(node.parts[-6:-1])
        for clip in ("clipper_pre", "clipper_post"):
            inputs = report_inputs(node / clip)
            src = run_path(node, inputs["in_file"])
            p_min, p_max = float(inputs["p_min"]), float(inputs["p_max"])
            # against niworkflows' own _advanced_clip in this environment
            with tempfile.TemporaryDirectory() as tmp:
                ref = _advanced_clip(str(src), p_min=p_min, p_max=p_max, newpath=tmp)
                want = np.asanyarray(nb.load(ref).dataobj)
            got = advanced_clip_lx(src, p_min, p_max, args.threads)
            add(
                f"IntensityClip ({clip}) = niworkflows' in this environment", same(want, got), label
            )
            # against fMRIPrep's own file (numpy 2.2.6: the 2.3 arithmetic of a Python-number q)
            want = np.asanyarray(nb.load(node / clip / "clipped.nii.gz").dataobj)
            got = advanced_clip_lx(src, p_min, p_max, args.threads, "2.3")
            add(
                f"IntensityClip ({clip}, numpy 2.3's percentile) = fMRIPrep's file",
                same(want, got),
                label,
            )
            steps[clip] += int(
                not same(want, advanced_clip_lx(src, p_min, p_max, args.threads, "2.5"))
            )
        masker_in = node / "clipper_post" / "clipped.nii.gz"
        data0 = nb.load(masker_in).get_fdata(dtype="float32")
        t0 = time.perf_counter()
        ref = masker_steps(data0, False, args.threads)
        t1 = time.perf_counter()
        mine = masker_steps(data0, True, args.threads)
        t2 = time.perf_counter()
        times["scipy"] += t1 - t0
        times["larmorx"] += t2 - t1
        for k in ref:
            add(f"brain_masker: {k}", same(ref[k], mine[k]), label)
    bad = 0
    for name in tally:
        n, ok = tally[name]
        bad += n - ok
        print(f"{name}: {n} cases, identical {ok}")
        if name in first:
            print(f"   first difference: {first[name]}")
    print(f"IntensityClip with numpy 2.5's percentile differs from fMRIPrep's file: {steps}")
    print(
        f"brain_masker steps: SciPy {times['scipy']:.1f} s, larmorx {times['larmorx']:.1f} s ({args.threads} threads)"
    )
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
