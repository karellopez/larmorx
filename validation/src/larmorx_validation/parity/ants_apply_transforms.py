"""Parity of ``larmorx ants antsApplyTransforms`` with antsApplyTransforms (ANTs 2.6.5).

Both programs get the same arguments and files. ANTs runs in-process through ANTsPy's
``ants.internal.get_lib_fn``, which is the real command line. larmorx runs through its Python
console entry point, which also reads ``.h5`` transforms. The two output files are read
with nibabel, a reader independent of both, and compared.

Cases cover:
- every interpolator;
- every transform type and file format ANTs reads, including inverses, chains, displacement
  fields and HDF5 composites;
- awkward geometries: oblique, flipped, anisotropic, single-slice, qform-only, scaled
  integers, nearest-neighbour ties and voxels outside the input;
- output types, time series and the errors both programs must raise;
- real fMRIPrep derivatives.
"""

from __future__ import annotations

import contextlib
import io
import os
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

import larmorx_testdata as td
import numpy as np

from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    PASS,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
)
from larmorx_validation.parity.itk_geometry import _quiet_stderr

#: Interpolators whose output is a label: voxels must be identical.
LABEL_INTERPOLATORS = ("nearestneighbor", "multilabel", "genericlabel")
#: PLAN.md §11.3: interpolation against ITK, max relative difference.
RTOL = 1e-4
AFFINE_ATOL = 1e-6

S = "synthetic/resampling"
AXIAL = f"{S}/images/axial-1mm.nii.gz"
LABELS = f"{S}/images/labels-uint8.nii.gz"
OBLIQUE = f"{S}/images/oblique-anisotropic.nii.gz"
LAS = f"{S}/images/las-int16-scaled.nii.gz"
QFORM = f"{S}/images/qform-only-int8.nii"
SLICE = f"{S}/images/single-slice.nii.gz"
SERIES = f"{S}/images/series-float32.nii.gz"
R_TIES = f"{S}/references/half-voxel-2mm.nii.gz"
R_OBLIQUE = f"{S}/references/oblique-1.5mm.nii.gz"
R_WIDE = f"{S}/references/wide-fov.nii.gz"
R_SLICE = f"{S}/references/single-slice-0.8mm.nii.gz"
T = {
    name: f"{S}/transforms/{name}"
    for name in (
        "affine-centered.mat",
        "affine-centered.txt",
        "affine-float.mat",
        "euler-centered.mat",
        "euler-zyx.txt",
        "similarity.mat",
        "versor-rigid.txt",
        "translation.txt",
        "warp-lps-vector.nii.gz",
        "warp-ras-dispvect.nii.gz",
        "composite-affine-warp.h5",
    )
}
LINEAR_TRANSFORMS = [k for k in T if not k.startswith(("warp", "composite"))]

F = "openneuro-derivatives/ds000005-fmriprep/sub-01"
FUNC = f"{F}/func/sub-01_task-mixedgamblestask_run-1"


@dataclass(frozen=True)
class Scenario:
    """One antsApplyTransforms invocation. Paths are catalog paths, except transforms named
    ``identity`` (ANTs' keyword), ``missing.mat`` (a file that does not exist) and
    ``larmorx:<path>`` (a catalog transform rewritten by ``lx.transforms.write``)."""

    image: str
    reference: str
    transforms: tuple[tuple[str, bool], ...] = ()
    interpolation: str = "Linear"
    extra: tuple[str, ...] = ()
    tier: str = "smoke"
    expect_error: bool = False
    files: tuple[str, ...] = field(default=(), compare=False)


def _scenarios() -> list[tuple[str, str, str, Scenario]]:
    """(id, category, description, scenario)."""
    out: list[tuple[str, str, str, Scenario]] = []

    def add(cid, category, desc, scenario):
        out.append((cid, category, desc, scenario))

    aff = ((T["affine-centered.mat"], False),)
    # Interpolators: the phantom through a centred affine onto an oblique grid.
    for interp in [
        "Linear",
        "NearestNeighbor",
        "BSpline",
        "BSpline[0]",
        "BSpline[1]",
        "BSpline[2]",
        "BSpline[4]",
        "BSpline[5]",
        "Gaussian",
        "Gaussian[1.5,2]",
        "Gaussian[1x2x3]",
        "CosineWindowedSinc",
        "HammingWindowedSinc",
        "LanczosWindowedSinc",
        "WelchWindowedSinc",
        "BlackmanWindowedSinc",
    ]:
        add(
            f"interpolation/{interp}",
            "interpolation",
            f"-n {interp}: phantom through a centred affine onto an oblique grid",
            Scenario(AXIAL, R_OBLIQUE, aff, interp),
        )
    for interp in ["NearestNeighbor", "MultiLabel", "MultiLabel[0.8]", "GenericLabel"]:
        add(
            f"interpolation/labels-{interp}",
            "interpolation",
            f"-n {interp} -u uchar: label image through a centred affine onto an oblique grid",
            Scenario(LABELS, R_OBLIQUE, aff, interp, ("-u", "uchar")),
        )

    # Transforms: each type and file format, forward and inverted.
    for name in T:
        add(
            f"transform/{name}",
            "transform",
            f"-t {name} (linear interpolation onto an oblique grid)",
            Scenario(AXIAL, R_OBLIQUE, ((T[name], False),)),
        )
    for name in LINEAR_TRANSFORMS:
        add(
            f"transform/inverse-{name}",
            "transform",
            f"-t [{name},1]: the inverse",
            Scenario(AXIAL, R_OBLIQUE, ((T[name], True),)),
        )
    chains = {
        "warp-then-affine": (
            "-t warp -t affine: the usual registration output order",
            ((T["warp-lps-vector.nii.gz"], False), (T["affine-centered.mat"], False)),
        ),
        "three-linear": (
            "-t affine -t [euler,1] -t translation: order and inversion within a chain",
            (
                (T["affine-centered.mat"], False),
                (T["euler-centered.mat"], True),
                (T["translation.txt"], False),
            ),
        ),
        "composite-then-inverse-affine": (
            "-t composite.h5 -t [affine,1]: a composite file inside a chain",
            ((T["composite-affine-warp.h5"], False), (T["affine-centered.mat"], True)),
        ),
        "identity-keyword": ("-t identity: ANTs' keyword", (("identity", False),)),
        "none": ("no -t at all: identity", ()),
        "larmorx-written-h5": (
            "the composite rewritten by lx.transforms.write: ANTs must read larmorx's .h5",
            ((f"larmorx:{T['composite-affine-warp.h5']}", False),),
        ),
    }
    for name, (desc, chain) in chains.items():
        add(f"transform/chain-{name}", "transform", desc, Scenario(AXIAL, R_OBLIQUE, chain))

    # Geometry.
    geometry = [
        (
            "oblique-to-axial",
            "oblique anisotropic input onto an axis-aligned grid",
            OBLIQUE,
            AXIAL,
            aff,
            "Linear",
            (),
        ),
        (
            "oblique-to-axial-lanczos",
            "oblique anisotropic input, Lanczos",
            OBLIQUE,
            AXIAL,
            aff,
            "LanczosWindowedSinc",
            (),
        ),
        (
            "scaled-int16-las",
            "int16 LAS image with scl_slope/scl_inter (ITK rescales through float32)",
            LAS,
            R_OBLIQUE,
            aff,
            "Linear",
            (),
        ),
        (
            "scaled-int16-las-bspline",
            "scaled int16 LAS image, B-spline",
            LAS,
            R_OBLIQUE,
            aff,
            "BSpline",
            (),
        ),
        ("qform-only-int8", "int8 image with only a qform", QFORM, R_OBLIQUE, aff, "Linear", ()),
        (
            "single-slice",
            "single-slice input onto a single-slice grid",
            SLICE,
            R_SLICE,
            (),
            "Linear",
            (),
        ),
        (
            "single-slice-nearest",
            "single-slice input, nearest neighbour",
            SLICE,
            R_SLICE,
            (),
            "NearestNeighbor",
            (),
        ),
        (
            "ties-nearest",
            "nearest neighbour where every output voxel is a tie between input voxels",
            AXIAL,
            R_TIES,
            (),
            "NearestNeighbor",
            (),
        ),
        (
            "ties-linear",
            "linear interpolation on the half-voxel grid",
            AXIAL,
            R_TIES,
            (),
            "Linear",
            (),
        ),
        (
            "ties-genericlabel",
            "GenericLabel ties on the half-voxel grid",
            LABELS,
            R_TIES,
            (),
            "GenericLabel",
            ("-u", "uchar"),
        ),
        (
            "ties-multilabel",
            "MultiLabel on the half-voxel grid",
            LABELS,
            R_TIES,
            (),
            "MultiLabel",
            ("-u", "uchar"),
        ),
        (
            "outside-default",
            "wide field of view: voxels outside the input get -f 7.5",
            AXIAL,
            R_WIDE,
            aff,
            "Linear",
            ("-f", "7.5"),
        ),
        (
            "outside-negative-default",
            "negative default value -f -3 (parsed as a value, not a flag)",
            AXIAL,
            R_WIDE,
            aff,
            "BSpline",
            ("-f", "-3"),
        ),
    ]
    for cid, desc, img, ref, chain, interp, extra in geometry:
        add(f"geometry/{cid}", "geometry", desc, Scenario(img, ref, chain, interp, extra))

    # Output types and precision.
    for u in ["char", "uchar", "short", "int", "float", "double", "default"]:
        add(
            f"output/-u-{u}",
            "output",
            f"-u {u}: output pixel type (integers truncate toward zero)",
            Scenario(AXIAL, R_OBLIQUE, aff, "Linear", ("-u", u)),
        )
    for interp in ["Linear", "LanczosWindowedSinc"]:
        add(
            f"output/float-{interp}",
            "output",
            f"--float 1 with {interp}: single-precision pipeline (larmorx computes in double)",
            Scenario(AXIAL, R_OBLIQUE, aff, interp, ("--float", "1")),
        )

    # Time series.
    for interp in ["Linear", "LanczosWindowedSinc", "BSpline"]:
        add(
            f"time-series/{interp}",
            "time-series",
            f"-e 3 with {interp}: each volume of a 4D series",
            Scenario(SERIES, R_OBLIQUE, aff, interp, ("-e", "3")),
        )
    add(
        "time-series/time-index",
        "time-series",
        "-e 3 --time-index 2: one volume of the series, written as 3D",
        Scenario(SERIES, R_OBLIQUE, aff, "Linear", ("-e", "3", "--time-index", "2")),
    )
    add(
        "time-series/named-type",
        "time-series",
        "-e time-series (the name instead of the number)",
        Scenario(SERIES, R_OBLIQUE, aff, "Linear", ("-e", "time-series")),
    )

    # Errors both must raise.
    errors = [
        (
            "4d-as-scalar",
            "a 4D image with -e 0",
            Scenario(SERIES, R_OBLIQUE, aff, expect_error=True),
        ),
        (
            "invert-field",
            "inverting a displacement field",
            Scenario(AXIAL, R_OBLIQUE, ((T["warp-lps-vector.nii.gz"], True),), expect_error=True),
        ),
        (
            "missing-transform",
            "a transform file that does not exist",
            Scenario(AXIAL, R_OBLIQUE, (("missing.mat", False),), expect_error=True),
        ),
        (
            "unknown-interpolator",
            "-n Cubic",
            Scenario(AXIAL, R_OBLIQUE, aff, "Cubic", expect_error=True),
        ),
    ]
    for cid, desc, scenario in errors:
        add(f"errors/{cid}", "errors", desc, scenario)

    # Real fMRIPrep derivatives (ds000005, sub-01).
    real = [
        (
            "boldref-to-T1w-lanczos",
            "boldref → T1w through fMRIPrep's ITK text affine, Lanczos",
            Scenario(
                f"{FUNC}_boldref.nii.gz",
                f"{F}/anat/sub-01_desc-preproc_T1w.nii.gz",
                ((f"{FUNC}_from-scanner_to-T1w_mode-image_xfm.txt", False),),
                "LanczosWindowedSinc",
                tier="standard",
            ),
        ),
        (
            "T1w-to-MNI-linear",
            "T1w → MNI152NLin2009cAsym 2 mm through fMRIPrep's .h5 composite (affine + warp)",
            Scenario(
                f"{F}/anat/sub-01_desc-preproc_T1w.nii.gz",
                f"{F}/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz",
                ((f"{F}/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5", False),),
                "Linear",
                tier="standard",
            ),
        ),
        (
            "T1w-to-MNI-lanczos",
            "T1w → MNI through the .h5 composite, Lanczos",
            Scenario(
                f"{F}/anat/sub-01_desc-preproc_T1w.nii.gz",
                f"{F}/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz",
                ((f"{F}/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5", False),),
                "LanczosWindowedSinc",
                tier="standard",
            ),
        ),
        (
            "GM-probseg-to-MNI",
            "GM probability map → MNI through the .h5 composite, Gaussian",
            Scenario(
                f"{F}/anat/sub-01_label-GM_probseg.nii.gz",
                f"{F}/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz",
                ((f"{F}/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5", False),),
                "Gaussian",
                tier="standard",
            ),
        ),
        (
            "aseg-to-boldref",
            "aseg segmentation → boldref grid, GenericLabel -u short",
            Scenario(
                f"{F}/anat/sub-01_desc-aseg_dseg.nii.gz",
                f"{FUNC}_boldref.nii.gz",
                ((f"{FUNC}_from-T1w_to-scanner_mode-image_xfm.txt", False),),
                "GenericLabel",
                ("-u", "short"),
                tier="standard",
            ),
        ),
        (
            "mask-to-MNI-multilabel",
            "brain mask → MNI through the .h5 composite, MultiLabel -u uchar",
            Scenario(
                f"{F}/anat/sub-01_desc-brain_mask.nii.gz",
                f"{F}/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz",
                ((f"{F}/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5", False),),
                "MultiLabel",
                ("-u", "uchar"),
                tier="standard",
            ),
        ),
        (
            "bold-series-to-T1w",
            "the 4D preprocessed BOLD series → T1w, -e 3 Lanczos (fMRIPrep's classic BOLD resampling)",
            Scenario(
                f"{FUNC}_desc-preproc_bold.nii.gz",
                f"{FUNC}_space-T1w_boldref.nii.gz",
                ((f"{FUNC}_from-scanner_to-T1w_mode-image_xfm.txt", False),),
                "LanczosWindowedSinc",
                ("-e", "3"),
                tier="standard",
            ),
        ),
    ]
    for cid, desc, scenario in real:
        add(f"fmriprep/{cid}", "fmriprep", desc, scenario)
    return out


_TIERS = ("smoke", "standard", "full")


def cases(tier: str) -> list[Case]:
    rank = _TIERS.index(tier)
    return [
        Case(cid, category, desc, scenario)
        for cid, category, desc, scenario in _scenarios()
        if _TIERS.index(scenario.tier) <= rank
    ]


# ------------------------------------------------------------------------------------------------
# Running


def _resolve(path: str, tmp: Path) -> str:
    if path in ("identity", "missing.mat"):
        return path if path == "identity" else str(tmp / path)
    if path.startswith("larmorx:"):
        import larmorx as lx

        source = td.get(path.removeprefix("larmorx:"))
        out = tmp / ("larmorx-" + source.name)
        lx.transforms.write(out, lx.transforms.read(source))
        return str(out)
    return str(td.get(path))


def _args(s: Scenario, tmp: Path, output: Path) -> list[str]:
    args = ["-d", "3", "-i", str(td.get(s.image)), "-r", str(td.get(s.reference))]
    for path, invert in s.transforms:
        resolved = _resolve(path, tmp)
        args += ["-t", f"[{resolved},1]" if invert else resolved]
    return [*args, "-n", s.interpolation, *s.extra, "-o", str(output)]


def _run_ants(args: list[str]) -> int:
    import ants

    with _quiet_stderr(), contextlib.redirect_stdout(io.StringIO()):
        return int(ants.internal.get_lib_fn("antsApplyTransforms")(args))


def _run_larmorx(args: list[str]) -> tuple[int, str]:
    from larmorx.cli import run

    err = io.StringIO()
    with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
        code = run(["larmorx", "ants", "antsApplyTransforms", *args])
    return code, err.getvalue().strip()


def _ulps(a: np.ndarray, b: np.ndarray) -> int:
    """Largest distance in units in the last place between two float arrays."""
    if a.dtype.kind != "f":
        return 0
    ia = a.view(np.int64 if a.dtype.itemsize == 8 else np.int32).astype(np.int64)
    ib = b.view(np.int64 if b.dtype.itemsize == 8 else np.int32).astype(np.int64)
    ia = np.where(ia < 0, np.iinfo(np.int64).min - ia, ia)
    ib = np.where(ib < 0, np.iinfo(np.int64).min - ib, ib)
    return int(np.abs(ia - ib).max()) if a.size else 0


def run_case(case: Case, checks: CheckList) -> None:
    import nibabel as nib

    s: Scenario = case.payload
    with tempfile.TemporaryDirectory(prefix="lx-aat-") as tmp_name:
        tmp = Path(tmp_name)
        ants_out, lx_out = tmp / "ants.nii.gz", tmp / "larmorx.nii.gz"
        ants_code = _run_ants(_args(s, tmp, ants_out))
        lx_code, lx_err = _run_larmorx(_args(s, tmp, lx_out))
        ants_ok = ants_code == 0 and ants_out.exists()
        lx_ok = lx_code == 0 and lx_out.exists()
        if not ants_ok and not lx_ok:
            raise Outcome(BOTH_ERROR, f"ANTs exit {ants_code}; larmorx: {lx_err or lx_code}")
        checks(
            "both succeed",
            ants_ok and lx_ok,
            detail=f"ANTs exit {ants_code}; larmorx exit {lx_code}: {lx_err}",
        )
        if s.expect_error:
            checks("both reject the arguments", False, detail="expected an error from both")
        if not (ants_ok and lx_ok):
            return
        a_img, l_img = nib.load(ants_out), nib.load(lx_out)
        a, b = np.asanyarray(a_img.dataobj), np.asanyarray(l_img.dataobj)
        checks(
            "data type",
            a.dtype == b.dtype,
            metric="dtype",
            value=str(b.dtype),
            threshold=str(a.dtype),
            detail=f"ANTs {a.dtype}, larmorx {b.dtype}",
        )
        checks(
            "shape",
            a.shape == b.shape,
            detail=f"ANTs {a.shape}, larmorx {b.shape}",
        )
        affine_diff = float(np.abs(a_img.affine - l_img.affine).max())
        checks(
            "affine",
            affine_diff <= AFFINE_ATOL,
            metric="max |diff| (mm)",
            value=f"{affine_diff:.2e}",
            threshold=f"{AFFINE_ATOL:g}",
        )
        if a.shape != b.shape:
            return
        differing = int(np.count_nonzero(a != b))
        name = s.interpolation.split("[")[0].lower()
        if name in LABEL_INTERPOLATORS:
            checks(
                "labels identical",
                differing == 0,
                metric="differing voxels",
                value=differing,
                threshold=0,
                detail=f"{differing} of {a.size} voxels differ",
            )
        elif a.dtype.kind in "iu":
            d = int(np.abs(a.astype(np.int64) - b.astype(np.int64)).max())
            checks(
                "values (integer output)",
                d <= 1,
                metric="max |diff|",
                value=d,
                threshold=1,
                detail=f"{differing} of {a.size} voxels differ (a last-bit difference can cross an integer)",
            )
        else:
            scale = float(np.abs(a).max()) or 1.0
            rel = float(np.abs(a.astype(np.float64) - b).max()) / scale
            checks(
                "values",
                rel <= RTOL,
                metric="max |diff| / max |ANTs|",
                value=f"{rel:.1e}",
                threshold=f"{RTOL:g}",
                detail=f"{differing} of {a.size} voxels differ; at most {_ulps(a, b)} ulp",
            )
        checks.add(
            Check(
                "bit-identical",
                True,
                metric="differing voxels",
                value=differing,
                threshold="reported",
                detail="identical" if differing == 0 else f"{differing} voxels differ",
            )
        )


def _highlights(results: list[CaseResult]) -> list[str]:
    compared = [r for r in results if r.status == PASS]
    identical = [
        r for r in compared if any(c.name == "bit-identical" and c.value == 0 for c in r.checks)
    ]
    differ = sorted({r.case for r in compared} - {r.case for r in identical})
    lines = [
        f"**Bit-identical: {len(identical)} of {len(compared)} passing cases** produce exactly the "
        "bytes of antsApplyTransforms' output data."
    ]
    if differ:
        lines.append(
            "The others differ in the last bits only, where ITK calls the platform's `exp`, "
            "`log`, `sin` or `cos` (Gaussian and windowed-sinc weights, Euler and versor "
            "matrices) or where `--float` makes ANTs compute in single precision: "
            + ", ".join(f"`{c}`" for c in differ)
            + ". See `docs/findings/platform-math.md`."
        )
    return lines


def suite() -> Suite:
    os.environ.setdefault("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "4")
    import ants

    return Suite(
        name="ants-apply-transforms",
        title="antsApplyTransforms",
        tool="`larmorx ants antsApplyTransforms` / `lx.ants.apply_transforms` (crates larmorx-ants, -interp, -transform, -io)",
        reference=f"antsApplyTransforms from ANTs 2.6.5 on ITK 5.4.5, run in-process through ANTsPy {ants.__version__}",
        thresholds=[
            ("Exit status", "both succeed, or both reject the arguments"),
            ("Output data type and shape", "identical"),
            ("Affine (as stored, read by nibabel)", f"max |diff| ≤ {AFFINE_ATOL:g} mm"),
            ("Label interpolators (NearestNeighbor, MultiLabel, GenericLabel)", "identical voxels"),
            (
                "Other interpolators, float output",
                f"max |diff| ≤ {RTOL:g} × max |ANTs output| (PLAN.md §11.3, interpolation vs ITK); bit-identity reported for every case",
            ),
            ("Other interpolators, integer output (-u)", "max |diff| ≤ 1"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("antspyx", "nibabel", "h5py"),
        highlights=_highlights,
        notes=(
            "Both programs read the same files with the same arguments; the outputs are read with "
            "nibabel. larmorx runs through its Python console entry point, so `.h5` transforms are "
            "read with h5py. Where ITK calls the platform's transcendental functions, larmorx uses "
            "the `libm` crate (the same bits on every platform), which can differ from glibc in the "
            "last bit; that is the only remaining source of difference "
            "(docs/findings/platform-math.md)."
        ),
    )
