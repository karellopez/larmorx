# SPDX-License-Identifier: Apache-2.0
"""Relay 28: the four golden cases of the code paths relay 27 and 28 added, run once and their
``reproduce`` outputs compared with the originals in this process (SciPy, scikit-image,
sdcflows, niworkflows: the newest Python environment), and with larmorx's default where the two
must differ.

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python:<repo>/validation/src \\
        python -P check_golden_paths.py

- ``py.ndimage.binary.int8_mask`` (SCI-31): SciPy's iterated erosion and closing with an int8
  mask, the outputs and the caller's mask afterwards;
- ``py.skimage.random_walker.nan``: scikit-image's random walker of an image with NaNs (the
  conjugate gradients run to ``maxiter`` in SciPy; larmorx stops once its solution is NaN and
  cannot change), the probabilities' bits and the labels;
- ``py.sdcflows.brain_masker.no_padding`` (SDC-46): sdcflows' ``brain_masker(..., padding=0)``,
  its three files (decompressed);
- ``py.niworkflows.intra_modal_merge.ras_mask`` (PIPE-40): niworkflows' ``IntraModalMerge`` with
  ``to_ras``, grand-mean scaling and a mask, its two files (decompressed).

Prints one line per comparison and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import gzip
import logging
import os
import sys
import tempfile
import warnings
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO / "tests" / "golden"))

import registry as g  # noqa: E402

FAILS = 0


def report(name: str, ok: bool) -> None:
    global FAILS
    FAILS += not ok
    print(("OK  " if ok else "FAIL") + " " + name)


def raw(path: str | os.PathLike[str]) -> bytes:
    b = Path(path).read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b


def same(a, b) -> bool:
    a, b = np.asarray(a), np.asarray(b)
    return a.dtype == b.dtype and a.shape == b.shape and a.tobytes() == b.tobytes()


def run_case(case_id: str, d: Path) -> dict:
    sub = d / case_id
    sub.mkdir()
    return g.CASES[case_id].run(g.Context(sub, None))


def check_sci31(d: Path) -> None:
    from scipy import ndimage

    out = run_case("py.ndimage.binary.int8_mask", d)
    gr = np.indices((20, 18, 16)).sum(axis=0)
    blob = g.ints(58, (20, 18, 16), 0, 100) < 55 + (gr < 25) * 30
    m = (g.ints(62, (20, 18, 16), 0, 4) > 0).astype(np.int8)
    e = ndimage.binary_erosion(blob, iterations=3, mask=m)
    report("SCI-31 erosion = SciPy's", same(out["erosion.reproduce"], e))
    report("SCI-31 erosion's mask afterwards = SciPy's", same(out["erosion.reproduce.mask"], m))
    m = (g.ints(63, (20, 18, 16), 0, 4) > 0).astype(np.int8)
    c = ndimage.binary_closing(blob, iterations=2, mask=m)
    report("SCI-31 closing = SciPy's", same(out["closing.reproduce"], c))
    report("SCI-31 closing's mask afterwards = SciPy's", same(out["closing.reproduce.mask"], m))
    m0 = (g.ints(62, (20, 18, 16), 0, 4) > 0).astype(np.int8)
    report("SCI-31 fix leaves the mask", same(out["erosion.fix.mask"], m0))
    report(
        "SCI-31 fix differs from reproduce",
        not same(out["erosion.fix.mask"], out["erosion.reproduce.mask"])
        and not same(out["closing.fix"], out["closing.reproduce"]),
    )


def check_rw_nan(d: Path) -> None:
    from skimage.segmentation import random_walker

    out = run_case("py.skimage.random_walker.nan", d)
    shape = (12, 10, 9)
    nan = np.array([0xFFC00000], np.uint32).view(np.float32)[0]
    data = (g.ints(84, shape, 0, 1000) / 1000).astype(np.float32)
    data[2:9, 3:8, 2:7] = nan
    labels = np.zeros(shape, np.int8)
    labels[0] = 2
    labels[-1] = 2
    labels[6, 5, 4] = 1
    zooms = np.array([2.0, 2.0, 2.5], np.float32)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for mode in ("cg", "cg_j"):
            p = random_walker(data, labels, spacing=zooms, mode=mode, return_full_prob=True)
            report(
                f"random walker NaN f32 {mode} = scikit-image's (bits)", same(out[f"f32.{mode}"], p)
            )
            lab = random_walker(data, labels, spacing=zooms, mode=mode)
            report(f"random walker NaN f32 {mode} labels", same(out[f"f32.{mode}.labels"], lab))
        p = random_walker(data.astype(np.float64), labels, mode="cg_j", return_full_prob=True)
        report("random walker NaN f64 cg_j = scikit-image's (bits)", same(out["f64.cg_j"], p))
    nans = np.isnan(out["f32.cg_j"]).sum()
    print(f"INFO random walker NaN: {nans} of {out['f32.cg_j'].size} probabilities NaN")


def check_sdc46(d: Path) -> None:
    from sdcflows.utils.tools import brain_masker

    out = run_case("py.sdcflows.brain_masker.no_padding", d)
    sub = d / "py.sdcflows.brain_masker.no_padding"
    head = str(sub / "head.nii.gz")
    od = d / "sdcflows-sdc46"
    od.mkdir()
    brain, prob, mask = brain_masker(head, str(od / "orig_mask.nii.gz"), padding=0)
    larm = {
        "mask": out["reproduce.mask"],
        "probseg": out["reproduce.probseg"],
        "brain": out["reproduce.brain"],
    }
    for name, path in (("mask", mask), ("probseg", prob), ("brain", brain)):
        report(f"SDC-46 {name} = sdcflows' (decompressed)", raw(larm[name].path) == raw(path))
    report("SDC-46 fix differs", raw(out["fix.mask"].path) != raw(out["reproduce.mask"].path))


def check_pipe40(d: Path) -> None:
    from niworkflows.interfaces.images import IntraModalMerge

    for name in ("nipype", "nipype.interface", "nipype.workflow", "nipype.utils"):
        logging.getLogger(name).setLevel(logging.ERROR)
    out = run_case("py.niworkflows.intra_modal_merge.ras_mask", d)
    sub = d / "py.niworkflows.intra_modal_merge.ras_mask"
    obl = [str(sub / f"p40-sub-01_magnitude{i}.nii.gz") for i in (1, 2)]
    od = d / "niworkflows-pipe40"
    od.mkdir()
    try:
        IntraModalMerge(
            in_files=obl,
            in_mask=str(sub / "p40-mask.nii.gz"),
            hmc=False,
            to_ras=True,
            grand_mean_scaling=True,
        ).run(cwd=str(od))
        message = ""
    except IndexError as e:
        message = str(e).strip().splitlines()[-1]
    print(f"INFO PIPE-40 niworkflows: {message!r}")
    report("PIPE-40 reproduce = niworkflows' IndexError", out["reproduce.error"] == message != "")
    # the corrected run: the series reoriented as niworkflows reorients it, scaled in the mask
    # reoriented the same way (its two volumes each by the largest median over its own)
    report("PIPE-40 fix written", raw(out["fix.avg"].path) != b"")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="lx-r28-golden-") as t:
        d = Path(t)
        check_sci31(d)
        check_rw_nan(d)
        check_sdc46(d)
        check_pipe40(d)
    print("ALL OK" if FAILS == 0 else f"FAIL {FAILS}")
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
