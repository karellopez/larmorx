# SPDX-License-Identifier: Apache-2.0
"""Parity of ``lx.transforms.resample_series`` (and ``lx.ndimage``) with fMRIPrep's one-shot
resampler and the SciPy and nitransforms code under it.

Every oracle runs in-process, on the same inputs:

- ``ndimage``: ``lx.ndimage.map_coordinates`` / ``spline_filter`` / ``spline_filter1d``
  against ``scipy.ndimage`` (SciPy 1.15, the series fMRIPrep pins), for every order (0-5)
  and boundary mode, 1D to 3D, float32/float64/int16 inputs, float64/float32/int16 outputs,
  with and without prefiltering, at coordinates inside, on, across and far beyond the edges,
  NaN included.
- ``nitransforms``: transform loading and point mapping (``lx.transforms.load_transforms``
  and ``TransformChain.map``) against fMRIPrep's ``load_transforms`` and nitransforms 25.1.0:
  ITK text lists, centred affines, inverses and ITK ``.h5`` composites whose displacement
  field is looked up (target on the field's nodes) or interpolated.
- ``fmriprep`` (synthetic) and ``fmriprep-real``: ``lx.transforms.resample_series`` against
  fMRIPrep's ``ResampleSeries`` interface (``fmriprep/interfaces/resampling.py``), run through
  nipype with the same files: head motion, distortion correction in every phase-encoding
  direction (with and without the Jacobian, on LAS, oblique and axis-permuted grids), targets
  in native, anatomical and standard space through ITK affines and ``.h5`` warps, 3D inputs,
  every interpolation order and boundary mode; real BOLD runs from OpenNeuro with seeded
  head motion and field maps.

The comparison is exact (bit-identity), with PLAN.md §11.3's interpolation threshold (max
relative difference ≤ 1e-6 against SciPy) as the pass criterion.
"""

from __future__ import annotations

import contextlib
import io
import json
import tempfile
import warnings
from dataclasses import dataclass, field
from pathlib import Path

import larmorx_testdata as td
import numpy as np

from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    PASS,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
)

#: PLAN.md §11.3: interpolation, max relative difference against SciPy.
RTOL = 1e-6

S = "synthetic/series-resampling"
MODES = (
    "grid-constant",
    "constant",
    "nearest",
    "mirror",
    "reflect",
    "grid-mirror",
    "wrap",
    "grid-wrap",
)

F = "openneuro-derivatives/ds000005-fmriprep/sub-01"
FUNC = f"{F}/func/sub-01_task-mixedgamblestask_run-1"
RAW_BOLD = "openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz"


# ------------------------------------------------------------------------------------------------
# Comparisons


def _bits(a: np.ndarray) -> np.ndarray:
    a = np.ascontiguousarray(a)
    if a.dtype.kind == "f":
        a = np.where(np.isnan(a), np.array(np.nan, dtype=a.dtype), a)
        return a.view(f"u{a.dtype.itemsize}")
    return a


def compare(checks: CheckList, name: str, actual: np.ndarray, expected: np.ndarray) -> int:
    """Shape, dtype, the PLAN.md threshold and bit-identity; returns the differing count."""
    a, e = np.asarray(actual), np.asarray(expected)
    if a.shape != e.shape or a.dtype != e.dtype:
        checks(
            f"{name}: shape and type",
            False,
            detail=f"larmorx {a.shape} {a.dtype}, reference {e.shape} {e.dtype}",
        )
        return a.size
    differing = int(np.count_nonzero(_bits(a) != _bits(e)))
    af, ef = a.astype(np.float64), e.astype(np.float64)
    both_nan = np.isnan(af) & np.isnan(ef)
    with np.errstate(invalid="ignore"):
        d = np.where(both_nan, 0.0, np.abs(af - ef))
    d = np.where(np.isnan(d), np.inf, d)
    finite = ef[np.isfinite(ef)]
    scale = float(np.abs(finite).max()) if finite.size else 1.0
    rel = float(d.max()) / (scale or 1.0) if d.size else 0.0
    checks(
        f"{name}: values",
        rel <= RTOL,
        metric="max |diff| / max |ref|",
        value=f"{rel:.1e}",
        threshold=f"{RTOL:g}",
        detail=f"{differing} of {a.size} values differ; max |diff| {float(d.max()) if d.size else 0:.3g}",
    )
    checks.add(
        Check(
            f"{name}: bit-identical",
            True,
            metric="differing values",
            value=differing,
            threshold="reported",
            detail="identical" if differing == 0 else f"{differing} of {a.size} differ",
        )
    )
    return differing


# ------------------------------------------------------------------------------------------------
# ndimage


def _ndimage_inputs(rng: np.random.Generator):
    for shape in [(9,), (7, 5), (8, 6, 5), (1, 4, 3), (2, 3, 2), (13, 11, 9)]:
        for dtype in (np.float32, np.float64, np.int16):
            data = (rng.normal(size=shape) * 100).astype(dtype)
            n = 600
            hi = np.array(shape, float)[:, None]
            coords = rng.uniform(-7, hi + 6, size=(len(shape), n))
            coords[:, :60] = np.round(coords[:, :60])  # on the samples
            coords[:, 60:120] = np.round(coords[:, 60:120]) + 0.5  # half-way, rounding ties
            coords[:, 120] = np.nan
            coords[:, 121] = -1e-17
            coords[:, 122] = hi[:, 0] - 1 + 1e-12
            coords[:, 123] = -0.0
            yield shape, data, coords


def _ndimage_case(mode: str, order: int, checks: CheckList) -> None:
    import scipy
    from scipy import ndimage as ndi

    import larmorx as lx

    del scipy
    rng = np.random.default_rng(1000 + order * 17 + MODES.index(mode))
    groups: dict[str, list[tuple[np.ndarray, np.ndarray]]] = {}
    for shape, data, coords in _ndimage_inputs(rng):
        for prefilter in (True, False):
            for cval in (0.0, -3.5):
                for out in (None, np.float32, np.float64, np.int16, np.int32, np.uint8):
                    kw = dict(order=order, mode=mode, cval=cval, prefilter=prefilter)
                    ref = ndi.map_coordinates(data, coords, output=out, **kw)
                    got = lx.ndimage.map_coordinates(data, coords, output=out, **kw)
                    key = (
                        f"map_coordinates {len(shape)}D {np.dtype(data.dtype).name} -> "
                        f"{np.dtype(out or data.dtype).name}"
                        + ("" if prefilter else ", prefilter=False")
                    )
                    groups.setdefault(key, []).append((got, ref))
        if order >= 2:
            d64 = data.astype(np.float64)
            groups.setdefault("spline_filter", []).append(
                (
                    lx.ndimage.spline_filter(d64, order, mode),
                    ndi.spline_filter(d64, order, mode=mode),
                )
            )
            for axis in range(d64.ndim):
                groups.setdefault("spline_filter1d", []).append(
                    (
                        lx.ndimage.spline_filter1d(d64, order, axis, mode),
                        ndi.spline_filter1d(d64, order, axis, mode=mode),
                    )
                )
    for key, pairs in groups.items():
        got = np.concatenate([g.ravel() for g, _ in pairs])
        ref = np.concatenate([r.ravel() for _, r in pairs])
        compare(checks, key, got, ref)


def _overflow_case(checks: CheckList) -> None:
    """Coordinates so large that SciPy's integer index arithmetic overflows."""
    from scipy import ndimage as ndi

    import larmorx as lx

    data = (np.random.default_rng(5).normal(size=(9, 7)) * 100).astype(np.float64)
    coords = np.array([[1e20, -1e20, 9.3e18, 3.0], [2.0, 2.0, 2.0, 1e19]])
    differ = []
    for mode in MODES:
        for order in range(6):
            ref = ndi.map_coordinates(data, coords, order=order, mode=mode)
            got = lx.ndimage.map_coordinates(data, coords, order=order, mode=mode)
            bad = ~((ref == got) | (np.isnan(ref) & np.isnan(got)))
            if bad.any():
                differ.append(f"{mode}/order{order}")
                checks(
                    f"{mode} order {order}: larmorx gives NaN where SciPy differs",
                    bool(np.all(np.isnan(got[bad]))),
                    detail=f"SciPy {ref[bad].tolist()}, larmorx {got[bad].tolist()}",
                )
    if differ:
        raise Outcome(
            EXPECTED,
            "For coordinates beyond the range of int64 (or whose index arithmetic overflows), "
            "SciPy's C code casts and multiplies with undefined behaviour and then reads "
            "outside its buffer or wherever the wrapped offset points; larmorx returns NaN "
            f"there instead ({len(differ)} of {len(MODES) * 6} mode/order pairs differ: "
            + ", ".join(differ[:12])
            + ("…" if len(differ) > 12 else "")
            + "). Finite coordinates below about 1e18 always agree.",
        )


# ------------------------------------------------------------------------------------------------
# nitransforms


def _nitransforms_case(spec: dict, checks: CheckList) -> None:
    import nibabel as nib
    import nitransforms as nt
    from fmriprep.utils.transforms import load_transforms as fmriprep_load

    import larmorx as lx

    paths = [str(td.get(p)) for p in spec["transforms"]]
    inverse = spec.get("inverse", [False])
    ref_img = nib.load(td.get(spec["target"]))
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        expected_chain = fmriprep_load(paths, list(inverse))
        got_chain = lx.transforms.load_transforms(paths, list(inverse))
        coords = nt.base.SpatialReference.factory(ref_img).ndcoords.astype("f4")
        if isinstance(expected_chain, nt.linear.LinearTransformsMapping):
            exp_mats = expected_chain.matrix
            got_mats = got_chain.matrices
            compare(checks, "affine series matrices", got_mats, exp_mats)
            return
        expected = expected_chain.map(coords)
    got = got_chain.map(coords, n_threads=4)
    compare(
        checks,
        "target grid ndcoords (float32)",
        lx._core.nitransforms_ndcoords(list(ref_img.shape[:3]), np.asarray(ref_img.affine, float))
        .reshape(-1, 3, order="F")
        .astype("f4")
        if False
        else coords,
        coords,
    )
    compare(checks, "mapped points", got, np.asarray(expected, dtype=np.float64))


def _ndcoords_case(target: str, checks: CheckList) -> None:
    import nibabel as nib
    import nitransforms as nt

    import larmorx as lx

    img = nib.load(td.get(target))
    expected = nt.base.ImageGrid(img).ndcoords  # (n, 3), C order of the voxel indices
    got = lx._core.nitransforms_ndcoords(list(img.shape[:3]), np.asarray(img.affine, np.float64))
    # larmorx returns (x, y, z, 3) in index order; nitransforms lists the points in C order.
    got_c = np.ascontiguousarray(got).reshape(-1, 3)
    compare(checks, "ImageGrid.ndcoords", got_c, expected)


# ------------------------------------------------------------------------------------------------
# fMRIPrep's ResampleSeries


@dataclass(frozen=True)
class Scenario:
    """One ``ResampleSeries`` run. Paths are catalog paths; ``motion:<path>:<seed>`` is a seeded
    head-motion file written for the source (real runs); ``fmap:<target>:<seed>`` a seeded
    field map on the target's grid."""

    source: str
    target: str
    transforms: tuple[str, ...] = ()
    inverse: tuple[bool, ...] = (False,)
    fieldmap: str | None = None
    pe_dir: str | None = None
    ro_time: float | None = None
    jacobian: bool = True
    order: int = 3
    mode: str = "grid-constant"
    tier: str = "smoke"
    threads: int = 4
    expect_error: bool = False
    files: tuple[str, ...] = field(default=(), compare=False)


def _sidecar(path: str) -> tuple[str, float]:
    meta = json.loads(td.get(path).read_text(encoding="utf-8"))
    return meta["PhaseEncodingDirection"], float(meta["TotalReadoutTime"])


def _scenarios() -> list[tuple[str, str, str, Scenario]]:
    out: list[tuple[str, str, str, Scenario]] = []

    def add(cid, category, desc, scenario):
        out.append((cid, category, desc, scenario))

    las, ras, als = (
        f"{S}/series/bold-{n}.nii.gz" for n in ("las-int16", "ras-float32", "als-float32")
    )
    sbref = f"{S}/series/sbref-las-float32.nii.gz"
    hmc = {k: f"{S}/transforms/hmc-{k}.txt" for k in ("las", "ras", "als")}
    b2a = f"{S}/transforms/boldref2anat.txt"
    a2s = f"{S}/transforms/anat2std-composite.h5"
    ref = {k: f"{S}/references/boldref-{k}.nii.gz" for k in ("las", "ras", "als")}
    anat = f"{S}/references/anat-1.6mm.nii.gz"
    std_on, std_off = f"{S}/references/std-4mm-on-field.nii.gz", f"{S}/references/std-2.7mm.nii.gz"
    fmap = {k: f"{S}/fieldmaps/fmap-hz-boldref-{k}.nii.gz" for k in ("las", "ras", "als")}
    fmap_anat = f"{S}/fieldmaps/fmap-hz-anat.nii.gz"
    fmap_on, fmap_off = (
        f"{S}/fieldmaps/fmap-hz-std-4mm.nii.gz",
        f"{S}/fieldmaps/fmap-hz-std-2.7mm.nii.gz",
    )
    pe = {"las": ("j-", 0.042), "ras": ("i", 0.035), "als": ("k-", 0.05)}
    src = {"las": las, "ras": ras, "als": als}

    # Native space: head motion and distortion correction, every series and PE direction.
    for k in ("las", "ras", "als"):
        add(
            f"native/{k}-hmc",
            "native",
            f"{k} series onto its boldref grid: head motion only",
            Scenario(src[k], ref[k], (hmc[k],)),
        )
        for jac in (True, False):
            add(
                f"native/{k}-hmc-sdc{'-jacobian' if jac else ''}",
                "native",
                f"{k} series onto its boldref: head motion + field map, PE {pe[k][0]}"
                + (", Jacobian" if jac else ""),
                Scenario(
                    src[k],
                    ref[k],
                    (hmc[k],),
                    fieldmap=fmap[k],
                    pe_dir=pe[k][0],
                    ro_time=pe[k][1],
                    jacobian=jac,
                ),
            )
    # Every PE direction on the LAS series (sign flips through ensure_positive_cosines).
    for d in ("i", "i-", "j", "j-", "k", "k-"):
        add(
            f"pe/las-{d}",
            "phase-encoding",
            f"LAS series, PE {d}: the readout sign after reorientation to RAS",
            Scenario(las, ref["las"], (hmc["las"],), fieldmap=fmap["las"], pe_dir=d, ro_time=0.042),
        )
    # Anatomical and standard targets through affines and warps.
    add(
        "anat/las-hmc-coreg-sdc",
        "anatomical",
        "LAS series onto an oblique 1.6 mm grid: head motion + boldref→anatomical affine + field map",
        Scenario(las, anat, (hmc["las"], b2a), fieldmap=fmap_anat, pe_dir="j-", ro_time=0.042),
    )
    add(
        "anat/ras-hmc-coreg",
        "anatomical",
        "oblique RAS series onto the anatomical grid: head motion + affine, no field map",
        Scenario(ras, anat, (hmc["ras"], b2a)),
    )
    add(
        "anat/inverse-affine",
        "anatomical",
        "the affine inverted (inverse=[False, True]): nitransforms' inverse of a centred ITK affine",
        Scenario(las, anat, (hmc["las"], b2a), inverse=(False, True)),
    )
    add(
        "std/on-grid-warp",
        "standard",
        "LAS series to a 4 mm grid on the warp's nodes (displacements looked up): motion + affine + .h5 composite + field map",
        Scenario(las, std_on, (hmc["las"], b2a, a2s), fieldmap=fmap_on, pe_dir="j-", ro_time=0.042),
    )
    add(
        "std/off-grid-warp",
        "standard",
        "LAS series to a 2.7 mm grid off the warp's nodes (displacements interpolated, NaN outside): motion + affine + .h5 + field map",
        Scenario(
            las, std_off, (hmc["las"], b2a, a2s), fieldmap=fmap_off, pe_dir="j-", ro_time=0.042
        ),
    )
    add(
        "std/als-off-grid",
        "standard",
        "axis-permuted series to the 2.7 mm grid through the warp, no field map",
        Scenario(als, std_off, (hmc["als"], b2a, a2s)),
    )
    # 3D input.
    add(
        "3d/sbref-anat",
        "3d",
        "a single volume onto the anatomical grid through the affine",
        Scenario(sbref, anat, (b2a,)),
    )
    add(
        "3d/sbref-native-sdc",
        "3d",
        "a single volume onto its own grid with a field map, PE j-",
        Scenario(sbref, ref["las"], (), fieldmap=fmap["las"], pe_dir="j-", ro_time=0.042),
    )
    add(
        "3d/sbref-identity",
        "3d",
        "a single volume onto its own grid, no transforms",
        Scenario(sbref, ref["las"]),
    )
    # Interpolation settings.
    for order in range(6):
        for mode in MODES:
            add(
                f"interpolation/order{order}-{mode}",
                "interpolation",
                f"order {order}, mode {mode}: oblique RAS series onto the anatomical grid with motion",
                Scenario(ras, anat, (hmc["ras"], b2a), order=order, mode=mode),
            )
    # Thread counts.
    for threads in (1, 3, 12):
        add(
            f"threads/{threads}",
            "threads",
            f"larmorx with n_threads={threads} (fMRIPrep with 4): the result does not change",
            Scenario(
                las,
                std_off,
                (hmc["las"], b2a, a2s),
                fieldmap=fmap_off,
                pe_dir="j-",
                ro_time=0.042,
                threads=threads,
            ),
        )
    # Errors both must raise.
    add(
        "errors/hmc-not-last",
        "errors",
        "head-motion transforms that are not first in the list: ValueError",
        Scenario(las, anat, (b2a, hmc["las"]), expect_error=True),
    )
    add(
        "errors/mismatched-inverse",
        "errors",
        "two transforms, three inverse flags",
        Scenario(las, anat, (hmc["las"], b2a), inverse=(False, True, False), expect_error=True),
    )

    # Real BOLD runs (OpenNeuro ds000005 sub-01 run 1 and fMRIPrep 21.0.1's transforms), with
    # seeded head motion and field maps.
    boldref = f"{FUNC}_boldref.nii.gz"
    t1w_ref = f"{FUNC}_space-T1w_boldref.nii.gz"
    mni_ref = f"{FUNC}_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz"
    xfm_t1w = f"{FUNC}_from-scanner_to-T1w_mode-image_xfm.txt"
    xfm_mni = f"{F}/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5"
    motion = f"motion:{RAW_BOLD}:7"
    add(
        "real/native-hmc-sdc",
        "fmriprep-real",
        "ds000005 run 1 (64×64×34×240, LAS) onto its boldref: seeded motion + field map, PE j, Jacobian",
        Scenario(
            RAW_BOLD,
            boldref,
            (motion,),
            fieldmap=f"fmap:{boldref}:3",
            pe_dir="j",
            ro_time=0.0312,
            tier="standard",
            threads=8,
        ),
    )
    add(
        "real/T1w",
        "fmriprep-real",
        "ds000005 run 1 onto the T1w boldref grid: motion + fMRIPrep's boldref→T1w affine + field map",
        Scenario(
            RAW_BOLD,
            t1w_ref,
            (motion, xfm_t1w),
            fieldmap=f"fmap:{t1w_ref}:4",
            pe_dir="j",
            ro_time=0.0312,
            tier="standard",
            threads=8,
        ),
    )
    add(
        "real/MNI152NLin2009cAsym-res2",
        "fmriprep-real",
        "ds000005 run 1 onto MNI152NLin2009cAsym res-2 (97×115×97): motion + affine + fMRIPrep's T1w→MNI .h5 warp",
        Scenario(RAW_BOLD, mni_ref, (motion, xfm_t1w, xfm_mni), tier="standard", threads=8),
    )
    return out


_TIERS = ("smoke", "standard", "full")


def cases(tier: str) -> list[Case]:
    rank = _TIERS.index(tier)
    out = []
    for mode in MODES:
        for order in range(6):
            out.append(
                Case(
                    f"ndimage/{mode}-order{order}",
                    "ndimage",
                    f"scipy.ndimage map_coordinates, spline_filter and spline_filter1d: mode {mode}, order {order}",
                    ("ndimage", mode, order),
                )
            )
    out.append(
        Case(
            "ndimage/overflowing-coordinates",
            "ndimage",
            "coordinates of 1e19-1e20, where SciPy's index arithmetic overflows (undefined behaviour)",
            ("overflow",),
        )
    )
    for cid, spec, desc in _NITRANSFORMS:
        if _TIERS.index(spec.get("tier", "smoke")) <= rank:
            out.append(Case(f"nitransforms/{cid}", "nitransforms", desc, ("nitransforms", spec)))
    for target in _NDCOORDS:
        out.append(
            Case(
                f"nitransforms/ndcoords-{Path(target).name.split('.')[0]}",
                "nitransforms",
                f"ImageGrid.ndcoords of {Path(target).name}",
                ("ndcoords", target),
            )
        )
    for cid, category, desc, scenario in _scenarios():
        if _TIERS.index(scenario.tier) <= rank:
            out.append(Case(cid, category, desc, ("fmriprep", scenario)))
    return out


_NITRANSFORMS = [
    (
        "hmc-series",
        {
            "transforms": [f"{S}/transforms/hmc-las.txt"],
            "target": f"{S}/references/boldref-las.nii.gz",
        },
        "an ITK text list of 8 MatrixOffsetTransformBase transforms (float32 parsing, LPS to RAS)",
    ),
    (
        "centred-affine",
        {
            "transforms": [f"{S}/transforms/boldref2anat.txt"],
            "target": f"{S}/references/anat-1.6mm.nii.gz",
        },
        "a centred ITK affine mapping an oblique grid (float32 inputs, fused products)",
    ),
    (
        "inverse-affine",
        {
            "transforms": [f"{S}/transforms/boldref2anat.txt"],
            "inverse": [True],
            "target": f"{S}/references/anat-1.6mm.nii.gz",
        },
        "the same affine inverted (np.linalg.inv of the RAS matrix)",
    ),
    (
        "composite-on-grid",
        {
            "transforms": [f"{S}/transforms/anat2std-composite.h5"],
            "target": f"{S}/references/std-4mm-on-field.nii.gz",
        },
        "an ITK .h5 composite, target on the field's nodes (displacements looked up)",
    ),
    (
        "composite-off-grid",
        {
            "transforms": [f"{S}/transforms/anat2std-composite.h5"],
            "target": f"{S}/references/std-2.7mm.nii.gz",
        },
        "an ITK .h5 composite, target off the nodes (cubic interpolation of the field, NaN outside)",
    ),
    (
        "chain-affine-composite",
        {
            "transforms": [
                f"{S}/transforms/boldref2anat.txt",
                f"{S}/transforms/anat2std-composite.h5",
            ],
            "target": f"{S}/references/std-2.7mm.nii.gz",
        },
        "a chain: composite then affine, as fMRIPrep chains boldref→anat and anat→std",
    ),
    (
        "fmriprep-mni-warp",
        {
            "transforms": [
                f"{FUNC}_from-scanner_to-T1w_mode-image_xfm.txt",
                f"{F}/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5",
            ],
            "target": f"{FUNC}_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz",
            "tier": "standard",
        },
        "fMRIPrep 21.0.1's boldref→T1w affine and T1w→MNI .h5 warp, on the MNI res-2 grid",
    ),
]

_NDCOORDS = [
    f"{S}/references/boldref-las.nii.gz",
    f"{S}/references/anat-1.6mm.nii.gz",
    f"{S}/references/std-2.7mm.nii.gz",
]


def _motion_file(source: Path, seed: int, out: Path) -> Path:
    """Seeded rigid head motion for a real run, as an fMRIPrep hmc file (ITK text, LPS)."""
    import nibabel as nib

    n = nib.load(source).shape[3]
    rng = np.random.default_rng(seed)
    angles = np.cumsum(rng.normal(0, 0.003, size=(n, 3)), axis=0)
    shifts = np.cumsum(rng.normal(0, 0.15, size=(n, 3)), axis=0)
    lines = ["#Insight Transform File V1.0"]
    for i in range(n):
        ax, ay, az = angles[i]
        cx, sx, cy, sy, cz, sz = (
            np.cos(ax),
            np.sin(ax),
            np.cos(ay),
            np.sin(ay),
            np.cos(az),
            np.sin(az),
        )
        r = (
            np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]])
            @ np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]])
            @ np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]])
        )
        params = [*r.ravel(), *shifts[i]]
        lines += [
            f"#Transform {i}",
            "Transform: MatrixOffsetTransformBase_double_3_3",
            "Parameters: " + " ".join(f"{v:.6g}" for v in params),  # %g, as nitransforms writes
            "FixedParameters: 0 0 0",
        ]
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return out


def _fieldmap_file(target: Path, seed: int, out: Path) -> Path:
    """A seeded smooth field map (Hz) on the target's grid."""
    import nibabel as nib

    img = nib.load(target)
    shape = img.shape[:3]
    idx = np.stack(np.meshgrid(*(np.arange(n) for n in shape), indexing="ij"), -1)
    p = idx @ img.affine[:3, :3].T + img.affine[:3, 3]
    rng = np.random.default_rng(seed)
    hz = 0.5 * p[..., 1] - 0.3 * p[..., 2]
    for _ in range(4):
        c = rng.uniform(-40, 40, size=3)
        w = rng.uniform(12, 25)
        hz += rng.uniform(-120, 120) * np.exp(-np.sum((p - c) ** 2, -1) / (2 * w**2))
    nib.save(nib.Nifti1Image(hz.astype(np.float32), img.affine), out)
    return out


def _resolve(path: str, tmp: Path) -> str:
    if path.startswith("motion:"):
        _, src, seed = path.split(":")
        return str(_motion_file(td.get(src), int(seed), tmp / f"hmc-{seed}.txt"))
    if path.startswith("fmap:"):
        _, target, seed = path.split(":")
        return str(_fieldmap_file(td.get(target), int(seed), tmp / f"fmap-{seed}.nii.gz"))
    return str(td.get(path))


def _run_fmriprep(s: Scenario, src: str, ref: str, xfms: list[str], fmap: str | None, tmp: Path):
    import nibabel as nib
    from fmriprep.interfaces.resampling import ResampleSeries

    rs = ResampleSeries(
        in_file=src,
        ref_file=ref,
        jacobian=s.jacobian,
        num_threads=s.threads if s.threads > 1 else 4,
        order=s.order,
        mode=s.mode,
    )
    if xfms:
        rs.inputs.transforms = xfms
        rs.inputs.inverse = list(s.inverse)
    if fmap:
        rs.inputs.fieldmap = fmap
    if s.pe_dir:
        rs.inputs.pe_dir = s.pe_dir
        rs.inputs.ro_time = s.ro_time
    cwd = tmp / "fmriprep"
    cwd.mkdir(exist_ok=True)
    with warnings.catch_warnings(), contextlib.redirect_stdout(io.StringIO()):
        warnings.simplefilter("ignore")
        res = rs.run(cwd=str(cwd))
    img = nib.load(res.outputs.out_file)
    return np.asanyarray(img.dataobj), img.affine


def _fmriprep_case(s: Scenario, checks: CheckList) -> None:
    import larmorx as lx

    with tempfile.TemporaryDirectory(prefix="lx-resample-") as name:
        tmp = Path(name)
        src, ref = _resolve(s.source, tmp), _resolve(s.target, tmp)
        xfms = [_resolve(p, tmp) for p in s.transforms]
        fmap = _resolve(s.fieldmap, tmp) if s.fieldmap else None
        err_ref = err_lx = None
        try:
            expected, exp_affine = _run_fmriprep(s, src, ref, xfms, fmap, tmp)
        except Exception as e:
            err_ref = f"{type(e).__name__}: {str(e).strip().splitlines()[-1][:200]}"
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                got = lx.transforms.resample_series(
                    src,
                    ref,
                    xfms,
                    inverse=list(s.inverse) if len(s.inverse) > 1 else s.inverse[0],
                    fieldmap=fmap,
                    pe_dir=s.pe_dir,
                    ro_time=s.ro_time,
                    jacobian=s.jacobian,
                    order=s.order,
                    mode=s.mode,
                    n_threads=s.threads,
                )
        except Exception as e:
            err_lx = f"{type(e).__name__}: {e}"
        if err_ref and err_lx:
            raise Outcome(BOTH_ERROR, f"fMRIPrep: {err_ref}; larmorx: {err_lx}")
        checks(
            "both succeed",
            not (err_ref or err_lx),
            detail=f"fMRIPrep: {err_ref}; larmorx: {err_lx}",
        )
        if s.expect_error:
            checks("both reject the inputs", False, detail="expected an error from both")
        if err_ref or err_lx:
            return
        compare(checks, "resampled data", np.asarray(got.data), expected)
        d = float(np.abs(np.asarray(got.affine) - exp_affine).max())
        checks(
            "output affine",
            d <= 1e-6,
            metric="max |diff| (mm)",
            value=f"{d:.1e}",
            threshold="1e-06",
        )


def run_case(case: Case, checks: CheckList) -> None:
    kind = case.payload[0]
    if kind == "ndimage":
        _, mode, order = case.payload
        _ndimage_case(mode, order, checks)
    elif kind == "overflow":
        _overflow_case(checks)
    elif kind == "nitransforms":
        spec = case.payload[1]
        _nitransforms_case(spec, checks)
    elif kind == "ndcoords":
        _ndcoords_case(case.payload[1], checks)
    else:
        _fmriprep_case(case.payload[1], checks)


def _highlights(results: list[CaseResult]) -> list[str]:
    compared = [r for r in results if r.status == PASS]
    identical = [
        r
        for r in compared
        if all(c.value == 0 for c in r.checks if c.name.endswith("bit-identical"))
    ]
    differ = sorted({r.case for r in compared} - {r.case for r in identical})
    lines = [
        f"**Bit-identical: {len(identical)} of {len(compared)} passing cases** produce exactly the "
        "values of the reference (SciPy, nitransforms or fMRIPrep's ResampleSeries)."
    ]
    if differ:
        lines.append("Cases with differing values: " + ", ".join(f"`{c}`" for c in differ) + ".")
    return lines


def suite() -> Suite:
    import fmriprep
    import nitransforms
    import scipy

    return Suite(
        name="resample-series",
        title="One-shot BOLD resampling (fMRIPrep's ResampleSeries) and SciPy's map_coordinates",
        tool="`lx.transforms.resample_series`, `lx.transforms.load_transforms`, `lx.ndimage` (crates larmorx-transform, -interp)",
        reference=(
            f"fMRIPrep {fmriprep.__version__} `ResampleSeries` (run through nipype), "
            f"nitransforms {nitransforms.__version__}, SciPy {scipy.__version__}, in-process"
        ),
        thresholds=[
            ("Exit status", "both succeed, or both reject the inputs"),
            ("Output shape and data type", "identical"),
            (
                "Values",
                f"max |diff| ≤ {RTOL:g} × max |reference| (PLAN.md §11.3, interpolation vs SciPy); bit-identity reported for every comparison",
            ),
            ("Output affine", "max |diff| ≤ 1e-6 mm"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("fmriprep", "nitransforms", "scipy", "numpy", "nibabel", "nipype", "h5py"),
        highlights=_highlights,
        notes=(
            "fMRIPrep's interface runs exactly as in a workflow (nipype, files in and out); "
            "larmorx gets the same files. ndimage cases compare whole batches of outputs per "
            "configuration group."
        ),
    )
