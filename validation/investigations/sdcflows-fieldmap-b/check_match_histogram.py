# SPDX-License-Identifier: Apache-2.0
"""sdcflows' ``match_histogram`` (``workflows/fit/syn.py``) and scikit-image's
``match_histograms`` against ``lx.sdcflows.match_histogram`` and
``larmorx.skimage.match_histograms``.

No workflow calls ``match_histogram``; the reference is sdcflows 2.17.1's own function (and
scikit-image 0.26.0's, the same file as 0.25.2's) in this environment, on:

- ``real``: the three fMRIPrep ``--use-syn-sdc`` runs' anatomy in the EPI's space
  (``anat2epi``) matched onto their clipped EPI reference (``clip_epi``) with both masks
  (``mask2epi``, ``epi_umask``) and without; ds000228's raw T1w (int16, its own grid) onto the
  EPI reference (no masks: numpy's assignment truncates the matched values);
- ``synthetic``: a scaled int16 image (float64 values), uint8 and uint16 images with unsigned
  references (``bincount``), a uint16 image with a float reference (SKI-02: scikit-image's
  ``TypeError``; corrected, and reproduced with ``reproduce``), an int16 image with a reference
  beyond its range and one with NaN (SDC-27: numpy's undefined casts; clamped, and numpy's
  values with ``reproduce``), float32 onto float64, an empty and a mis-shaped mask (both
  refuse);
- ``arrays``: ``match_histograms`` itself on arrays: channels on the first, last and middle
  axis, float16, int8, a complex image, different dimensions and channel counts (errors).

Files compared by their uncompressed bytes (nibabel's writer against larmorx's), errors by type
and message. Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``; boxed):

    OPENBLAS_CORETYPE=Haswell python -P check_match_histogram.py --scratch <folder>

Prints one line per check and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
import warnings
from pathlib import Path
from typing import Any

import nibabel as nb
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_derivatives as cd  # noqa: E402
import check_workflows as cw  # noqa: E402
import steplines  # noqa: E402  (the originals' step: scikit-image's and sdcflows' outputs)

SYN_RUNS = (
    "ds000228-sub-pixar008-noreconall-syn",
    "ds004884-sub-M2079-noreconall-syn",
    "ds005003-sub-RS001-noreconall-syn",
)


def _err(fn: Any) -> tuple[Any, str]:
    try:
        return fn(), ""
    except Exception as e:  # the error is the result
        return None, f"{type(e).__name__}: {e}"


def _save(path: Path, data: np.ndarray, aff: np.ndarray, slope: float | None = None) -> str:
    img = nb.Nifti1Image(data, aff)
    if slope is not None:
        img.header.set_slope_inter(slope, 3.0)
    img.to_filename(path)
    return str(path)


def real_cases() -> list[dict[str, Any]]:
    out = []
    for name in SYN_RUNS:
        run = cw.RUNS / name
        subwf = sorted((run / "work" / "fmriprep_25_2_wf").glob("sub_*_wf"))[0]
        pre = sorted(subwf.glob("syn_preprocessing_*"))[0]
        wf = subwf / "fmap_preproc_wf" / ("wf_" + pre.name.split("syn_preprocessing_")[1])
        ref = sorted((wf / "clip_epi").glob("*.nii.gz"))[0]
        img = sorted((pre / "anat2epi").glob("*.nii.gz"))[0]
        ref_mask = sorted((wf / "epi_umask").glob("*.nii.gz"))[0]
        img_mask = sorted((pre / "mask2epi").glob("*_uint8.nii.gz"))[0]
        label = name.split("-")[0]
        out.append({"label": f"{label} masked", "args": (ref, img, ref_mask, img_mask)})
        out.append({"label": f"{label} unmasked", "args": (ref, img)})
        if name.startswith("ds000228"):
            t1w = sorted(cw.bids_root(pre).rglob("sub-pixar008*_T1w.nii.gz"))
            out.append({"label": f"{label} raw T1w (int16) unmasked", "args": (ref, t1w[0])})
    return out


def synthetic_cases(root: Path) -> list[dict[str, Any]]:
    root.mkdir(parents=True, exist_ok=True)
    aff = np.diag([-2.0, 2.0, 2.5, 1.0])
    shape = (14, 12, 10)
    i, j, k = np.indices(shape)
    base = ((i * 37 + j * 11 + k * 5) % 211).astype(np.float64)
    ref_f = _save(root / "ref_float.nii.gz", (base * 1.75 - 20).astype(np.float32), aff)
    ref_wide = _save(root / "ref_wide.nii.gz", (base * 400 - 9000).astype(np.float32), aff)
    nan = (base * 1.5).astype(np.float32)
    nan[0, 0, :3] = np.nan
    ref_nan = _save(root / "ref_nan.nii.gz", nan, aff)
    ref_u8 = _save(root / "ref_u8.nii.gz", ((base * 3) % 251).astype(np.uint8), aff)
    ref_u16 = _save(root / "ref_u16.nii.gz", (base * 97).astype(np.uint16), aff)
    ref_f64 = _save(root / "ref_f64.nii.gz", base * 0.125 + 1e-3, aff)
    img_i16 = _save(root / "img_i16.nii.gz", ((base * 13) % 997 - 300).astype(np.int16), aff)
    img_sc = _save(root / "img_scaled.nii.gz", ((base * 7) % 401).astype(np.int16), aff, 0.5)
    img_u8 = _save(root / "img_u8.nii.gz", ((base * 5) % 199).astype(np.uint8), aff)
    img_u16 = _save(root / "img_u16.nii.gz", ((base * 31) % 4001).astype(np.uint16), aff)
    img_f32 = _save(root / "img_f32.nii.gz", (base / 3).astype(np.float32), aff)
    mask = _save(root / "mask.nii.gz", ((i + j + k) % 3 != 0).astype(np.uint8), aff)
    empty = _save(root / "empty.nii.gz", np.zeros(shape, np.uint8), aff)
    small = _save(root / "small.nii.gz", np.ones((4, 4, 4), np.uint8), aff)
    return [
        {"label": "scaled int16 onto float32", "args": (ref_f, img_sc, None, mask)},
        {"label": "uint8 onto uint8 (bincount)", "args": (ref_u8, img_u8)},
        {"label": "uint16 onto uint16 masked (bincount)", "args": (ref_u16, img_u16, mask, mask)},
        {"label": "uint16 onto float32 (SKI-02)", "args": (ref_f, img_u16), "defect": "SKI-02"},
        {"label": "uint8 onto int16 with negatives (SKI-02)", "args": (img_i16, img_u8), "defect": "SKI-02"},
        {"label": "int16 onto a wider range (SDC-27)", "args": (ref_wide, img_i16), "defect": "SDC-27"},
        {"label": "uint8 onto float32 with negatives (SKI-02)", "args": (ref_f, img_u8), "defect": "SKI-02"},
        {"label": "int16 onto float32 with NaN (SDC-27)", "args": (ref_nan, img_i16), "defect": "SDC-27"},
        {"label": "float32 onto float64", "args": (ref_f64, img_f32, mask, None)},
        {"label": "int16 onto float32", "args": (ref_f, img_i16, mask, mask)},
        {"label": "error: empty reference mask", "args": (ref_f, img_f32, empty, None)},
        {"label": "error: mask of another shape", "args": (ref_f, img_f32, None, small)},
    ]  # fmt: skip


def sec_files(t: cw.Tally, scratch: Path, cases: list[dict[str, Any]], what: str) -> None:
    from sdcflows.workflows.fit.syn import match_histogram as sdc_match

    import larmorx.sdcflows as ls

    for c in cases:
        lab = c["label"]
        base = scratch / what / lab.replace(" ", "_").replace("(", "").replace(")", "")
        if base.exists():
            shutil.rmtree(base)
        orig, mine = base / "sdcflows", base / "larmorx"
        orig.mkdir(parents=True)
        args = [None if a is None else str(a) for a in c["args"]]
        cwd = os.getcwd()
        os.chdir(orig)
        try:
            ref_out, ref_err = steplines.oracle(_err(lambda: sdc_match(*args)))  # noqa: B023
        finally:
            os.chdir(cwd)
        modes = ("reproduce", "fix") if c.get("defect") else ("reproduce",)
        for mode in modes:
            with warnings.catch_warnings(record=True) as caught:
                warnings.simplefilter("always")
                got, err = _err(
                    lambda m=mode, a=args, o=mine: ls.match_histogram(
                        *a, out_dir=o / m, upstream_defects=m
                    )
                )
            warned = sorted(
                {
                    str(w.message).split(" ")[0]
                    for w in caught
                    if "SDC-" in str(w.message) or "SKI-" in str(w.message)
                }
            )
            if mode == "reproduce":
                if ref_err or err:
                    same = (
                        ref_err.split(":")[0] == err.split(":")[0] and bool(ref_err) and bool(err)
                    )
                    t.add(f"{what}: the same error", same, f"{lab}: {ref_err!r} / {err!r}")
                    print(f"INFO {lab}: sdcflows {ref_err[:150]!r}; larmorx {err[:150]!r}")
                    continue
                t.add(f"{what}: file name = sdcflows'", Path(got).name == Path(ref_out).name, lab)
                t.add(
                    f"{what}: uncompressed bytes = sdcflows'",
                    cd.raw(Path(ref_out)) == cd.raw(Path(got)),
                    lab,
                )
                a = np.asanyarray(nb.load(ref_out).dataobj)
                print(f"INFO {lab}: {a.dtype} {a.shape}, identical")
            else:
                t.add(
                    f"{what}: {c['defect']} corrected with its warning",
                    not err and c["defect"] in " ".join(warned),
                    f"{lab}: {err!r} {warned}",
                )
                if not err:
                    a = np.asanyarray(nb.load(got).dataobj)
                    print(
                        f"INFO {lab} fix: {a.dtype}, {int(np.isnan(a).sum()) if a.dtype.kind == 'f' else 0} NaN, range [{a.min()}, {a.max()}]"
                    )


def sec_arrays(t: cw.Tally) -> None:
    from skimage.exposure import match_histograms as sk

    from larmorx.skimage import match_histograms as lx_mh

    g = np.indices((6, 5, 4)).sum(0)
    rgb = np.stack([g * 3 % 17, g * 5 % 23, g * 7 % 29], 0).astype(np.float32)
    ref = np.stack([g * 2 % 19, g * 3 % 13, g % 7], 0).astype(np.float64) * 1.5
    cases = [
        ("channels first", rgb, ref, {"channel_axis": 0}),
        ("channels last", np.moveaxis(rgb, 0, -1), np.moveaxis(ref, 0, -1), {"channel_axis": -1}),
        ("channels middle", np.moveaxis(rgb, 0, 1), np.moveaxis(ref, 0, 1), {"channel_axis": 1}),
        ("float16", (g / 3).astype(np.float16), ref[0], {}),
        ("int8", (g * 9 % 101 - 50).astype(np.int8), ref[1], {}),
        ("uint8 channels", (np.moveaxis(rgb, 0, -1) * 3).astype(np.uint8), (np.moveaxis(ref, 0, -1)).astype(np.uint8), {"channel_axis": -1}),
        ("complex", (g + 1j * g).astype(np.complex64), ref[0], {}),
        ("error: dimensions", rgb, ref[0], {}),
        ("error: channels", rgb, ref[:2], {"channel_axis": 0}),
        ("error: two channel axes", rgb, ref, {"channel_axis": (0, 1)}),
    ]  # fmt: skip
    for lab, img, r, kw in cases:
        want, e1 = steplines.oracle(_err(lambda img=img, r=r, kw=kw: sk(img, r, **kw)))
        got, e2 = _err(lambda img=img, r=r, kw=kw: lx_mh(img, r, **kw))
        if e1 or e2:
            t.add("arrays: the same error", e1 == e2, f"{lab}: {e1!r} / {e2!r}")
            continue
        t.add(
            "arrays: values and type = scikit-image's",
            want.dtype == got.dtype and want.shape == got.shape and want.tobytes() == got.tobytes(),
            f"{lab}: {want.dtype} {got.dtype}",
        )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="arrays,synthetic,real")
    ap.add_argument("--scratch", required=True)
    a = ap.parse_args()
    scratch = Path(a.scratch).absolute()
    scratch.mkdir(parents=True, exist_ok=True)
    print(f"numpy {np.__version__}")
    t = steplines.Noted(cw.Tally())
    sections = a.sections.split(",")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        if "arrays" in sections:
            sec_arrays(t)
        if "synthetic" in sections:
            sec_files(t, scratch, synthetic_cases(scratch / "inputs"), "synthetic")
        if "real" in sections:
            sec_files(t, scratch, real_cases(), "real")
    bad = t.report()
    steplines.lines()
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
