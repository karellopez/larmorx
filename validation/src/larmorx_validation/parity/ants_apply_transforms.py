# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants antsApplyTransforms`` with antsApplyTransforms (ANTs 2.6.5).

Both programs get the same arguments and files. ANTs is the oracle's ``antsApplyTransforms``
binary (``LARMORX_ANTS_BIN``, else the workspace's ``oracles/ants-2.6.5/bin``, built by
``scripts/build_ants_oracle.sh``), with ``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=4`` unless set;
without it, ANTs runs in-process through ANTsPy's ``ants.internal.get_lib_fn``, the same
command line in ANTsPy's own build. larmorx runs through its Python console entry point (its
command line reads ``.h5`` transforms in Rust, crate ``larmorx-hdf5``). The two output files are
read with nibabel, a reader independent of both, and compared.

Cases cover:
- every interpolator;
- every transform type and file format ANTs reads, including inverses, chains, displacement
  fields, B-spline transforms (``antsRegistration``'s ``BSpline`` stage output, in text,
  MATLAB and HDF5 files, in float, inside composites and chains, and the inverse ANTs
  refuses; ``ants_bspline_transforms``) and HDF5 composites (fMRIPrep's, and files ITK
  writes: ``-o Linear[...]``, float parameters, dense groups, larmorx's own rewrites);
- awkward geometries: oblique, flipped, anisotropic, single-slice, qform-only, scaled
  integers, nearest-neighbour ties and voxels outside the input;
- output types, time series and the errors both programs must raise;
- real fMRIPrep derivatives;
- all of the above again with ``--float 1``, ANTs' single-precision pipeline (category
  ``float``), plus the parsing of float numbers in text transforms and ``-f``;
- the transform outputs ``-o Linear[...]``, ``-o CompositeTransform[...]`` and
  ``-o DisplacementField[...]`` (category ``transform-output``, ``ants_transform_output``), whose files
  are compared byte for byte (text, MATLAB), dataset by dataset (HDF5) or header and vectors
  (NIfTI).

ANTs runs twice: as built (glibc's maths), the headline, and with CORE-MATH's correctly
rounded functions preloaded (``scripts/build_crmath_preload.sh``), which larmorx computes with
(CLAUDE.md rule 5); the second run explains the differences of the first.
"""

from __future__ import annotations

import contextlib
import functools
import io
import itertools
import os
import subprocess
import tempfile
from dataclasses import dataclass, field, replace
from pathlib import Path

import larmorx_testdata as td
import numpy as np

from larmorx_validation.parity import ants_bspline_transforms, defects
from larmorx_validation.parity import ants_transform_output as outputs
from larmorx_validation.parity.ants_programs import (
    affine_difference,
    ants_bin,
    ants_version,
    crmath_preload,
    header_differences,
    raw_header,
)
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    PASS,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
    in_tier,
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

#: Just below the midpoint 26843547·2⁻²⁸ between two floats: read as double it lands on the
#: midpoint, which a cast to float rounds to even (up); read as float (ANTs' ``>> float``) it
#: rounds down.
STRTOF_TRAP = "0.1000000052154064178466796874999999999999"
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
    #: The whole command line as given (the data panel's tier: fMRIPrep's nodes, the panel's
    #: images): ``{out:<name>}`` the output, ``{in:<path>}`` a catalog file; ``image``,
    #: ``reference`` and ``transforms`` are then not used.
    raw: tuple[str, ...] = ()


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
        "h5-itk-linear": (
            "-t linear.h5: an affine ITK wrote (-o Linear[...])",
            (("ants-h5:linear", False),),
        ),
        "h5-itk-linear-inverse": (
            "-t [linear.h5,1]: the inverse of an affine ITK wrote",
            (("ants-h5:linear", True),),
        ),
        "h5-itk-linear-float": (
            "-t linear-float.h5: float parameters (ITK's --float 1 -o Linear[...])",
            (("ants-h5:linear-float", False),),
        ),
        "h5-itk-types": (
            "-t composite.h5: six transform types in one composite ITK wrote",
            (("ants-h5:composite-types", False),),
        ),
        "h5-itk-types-inverse": (
            "-t [composite.h5,1]: the inverse of a composite of six linear transforms",
            (("ants-h5:composite-types", True),),
        ),
        "h5-itk-dense": (
            "-t composite.h5: nine affines ITK wrote (a dense group: fractal heap, v2 B-tree)",
            (("ants-h5:composite-9-affines", False),),
        ),
        "h5-itk-warp-float": (
            "-t composite.h5: a field and an affine in float (ITK's --float 1 output)",
            (("ants-h5:composite-float", False),),
        ),
        "h5-larmorx-dense": (
            "the nine-affine composite rewritten by lx.transforms.write (dense)",
            (("larmorx:ants-h5:composite-9-affines", False),),
        ),
    }
    for name, (desc, chain) in chains.items():
        add(f"transform/chain-{name}", "transform", desc, Scenario(AXIAL, R_OBLIQUE, chain))

    # B-spline transforms (antsRegistration's BSpline stage output), mesh 3x3x3 over the
    # phantom.
    b = "gen-bspline:"
    bsp = ((f"{b}bspline.txt", False),)
    bspline = [
        ("text", "-t bspline.txt: a BSplineTransform_double_3_3 in ITK's text format", bsp, "Linear"),
        ("matlab", "-t bspline.mat: the same in ITK's MATLAB format", ((f"{b}bspline.mat", False),), "Linear"),
        ("hdf5", "-t bspline.h5: the same in HDF5", ((f"{b}bspline.h5", False),), "Linear"),
        ("float-class", "-t a BSplineTransform_float_3_3 (float coefficients)", ((f"{b}bspline-float.txt", False),), "Linear"),
        ("composite-hdf5", "-t composite.h5: an affine and a B-spline transform in one HDF5 composite (-a 1)", ((f"{b}affine-bspline.h5", False),), "Linear"),
        ("chain-affine", "-t bspline -t affine: the affine first, then the B-spline transform", (*bsp, (T["affine-centered.mat"], False)), "Linear"),
        ("chain-warp", "-t warp -t bspline: the B-spline transform first, then the displacement field", ((T["warp-lps-vector.nii.gz"], False), *bsp), "Linear"),
        ("chain-inverse-affine", "-t [affine,1] -t bspline: an inverted affine after the B-spline transform", ((T["affine-centered.mat"], True), *bsp), "Linear"),
        ("nearest", "-n NearestNeighbor through the B-spline transform", bsp, "NearestNeighbor"),
        ("bspline-interpolation", "-n BSpline through the B-spline transform", bsp, "BSpline"),
        ("lanczos", "-n LanczosWindowedSinc through the B-spline transform", bsp, "LanczosWindowedSinc"),
    ]  # fmt: skip
    for cid, desc, chain, interp in bspline:
        add(f"bspline/{cid}", "bspline", desc, Scenario(AXIAL, R_OBLIQUE, chain, interp))
    add(
        "bspline/time-series",
        "bspline",
        "-e 3: each volume of a 4D series through the B-spline transform",
        Scenario(SERIES, R_OBLIQUE, bsp, "Linear", ("-e", "3")),
    )
    add(
        "bspline/labels-genericlabel",
        "bspline",
        "-n GenericLabel -u uchar: a label image through the B-spline transform",
        Scenario(LABELS, R_OBLIQUE, bsp, "GenericLabel", ("-u", "uchar")),
    )

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
            f"--float 1 with {interp}: ANTs' single-precision pipeline",
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
            "invert-bspline",
            "inverting a B-spline transform: it has no inverse",
            Scenario(AXIAL, R_OBLIQUE, (("gen-bspline:bspline.txt", True),), expect_error=True),
        ),
        (
            "invert-bspline-composite",
            "inverting a composite that holds a B-spline transform",
            Scenario(
                AXIAL, R_OBLIQUE, (("gen-bspline:affine-bspline.h5", True),), expect_error=True
            ),
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

    # --float 1: every case above again in single precision.
    twins = []
    for cid, _category, desc, scenario in out:
        if "--float" in scenario.extra:
            continue
        twins.append(
            (
                f"float/{cid}",
                "float",
                f"--float 1: {desc}",
                replace(scenario, extra=(*scenario.extra, "--float", "1")),
            )
        )
    out.extend(twins)
    add(
        "float/parse/text-transform",
        "float",
        "--float 1: a text affine whose numbers round differently read as float (strtof) and "
        "as double then cast",
        Scenario(AXIAL, R_OBLIQUE, (("gen:strtof-affine.txt", False),), "Linear", ("--float", "1")),
    )
    add(
        "float/parse/default-value",
        "float",
        "--float 1 -f 0.1000000052154064178466796874999999999999: the default value read as float "
        "(strtof), not as double then cast",
        Scenario(
            AXIAL,
            R_WIDE,
            aff,
            "Linear",
            ("-f", STRTOF_TRAP, "--float", "1"),
        ),
    )
    return out


def _interpolation(args) -> str:
    """The ``-n``/``--interpolation`` value of a command line (ANTs' default: Linear)."""
    for i, a in enumerate(args[:-1]):
        if a in ("-n", "--interpolation"):
            return args[i + 1]
    return "Linear"


def _panel_scenarios() -> list[tuple[str, str, str, Scenario]]:
    """The data panel's tier: every antsApplyTransforms node of the fourteen fMRIPrep oracle
    runs (their command lines as written), and fMRIPrep's transforms applied to panel images
    it never resamples."""
    from larmorx_validation.parity import fmriprep_nodes
    from larmorx_validation.parity import panel_images as P

    out = []
    for c in fmriprep_nodes.calls("antsApplyTransforms"):
        out.append(
            (
                fmriprep_nodes.PREFIX + c.id,
                fmriprep_nodes.CATEGORY,
                fmriprep_nodes.describe(c),
                Scenario("", "", interpolation=_interpolation(c.args), tier="panel", raw=c.args),
            )
        )
    root = fmriprep_nodes.root()
    if root is None:
        return out

    def run_file(run: str, pattern: str) -> str | None:
        found = sorted((root / run / "out").glob(pattern))
        return str(found[0]) if found else None

    mb8 = "ds004958-sub-fsm64ps-noreconall"
    seven = "ds005454-sub-16-noreconall"
    stroke = "ds004884-sub-M2079-noreconall"
    older = "ds003592-sub-280-noreconall"
    tpl = "{in:templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-01_T1w.nii.gz}"
    specs = [
        (
            "bold-series-to-T1w/mb8-600",
            "fMRIPrep's classic BOLD resampling (-e 3, Lanczos, --float 1) of a 600-volume "
            "multiband-8 series (fMRIPrep's preprocessed BOLD in its boldref space) onto its "
            "T1w-space reference through fMRIPrep's coregistration transform: the panel's "
            "largest series",
            [P.BOLD_MB8],
            [
                ("-i", run_file(mb8, "sub-*/func/*_task-rest_desc-preproc_bold.nii.gz")),
                ("-r", run_file(mb8, "sub-*/func/*_task-rest_space-T1w_boldref.nii.gz")),
                ("-t", run_file(mb8, "sub-*/func/*_from-boldref_to-T1w_mode-image_desc-coreg_xfm.txt")),
            ],
            ("-e", "3", "-n", "LanczosWindowedSinc", "--float", "1"),
        ),
        (
            "raw-to-MNI/7t-mp2rage",
            "the raw 7 T MP2RAGE into MNI152NLin2009cAsym at 1 mm in one interpolation "
            "(Lanczos): fMRIPrep's T1w-to-MNI composite after its raw-to-T1w transform",
            [P.MP2RAGE_7T],
            [
                ("-i", P.MP2RAGE_7T.arg),
                ("-r", tpl),
                ("-t", run_file(seven, "sub-*/anat/*_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5")),
                ("-t", run_file(seven, "sub-*/anat/*_acq-mp2rage_from-orig_to-T1w_mode-image_xfm.txt")),
            ],
            ("-n", "LanczosWindowedSinc"),
        ),
        (
            "lesion-to-MNI/stroke",
            "the stroke's manual lesion mask (on its T1w's grid, as fMRIPrep read it) into "
            "MNI152NLin2009cAsym at 2 mm through fMRIPrep's composite, GenericLabel -u uchar",
            [P.STROKE_LESION],
            [
                ("-i", str(next(iter(sorted(fmriprep_nodes.oracle_run(stroke).mounts["/data"].glob(
                    "sub-*/ses-*/anat/*_label-lesion_roi.nii.gz"))), "")) or None),
                ("-r", run_file(stroke, "sub-*/ses-*/anat/*_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz")),
                ("-t", run_file(stroke, "sub-*/ses-*/anat/*_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5")),
            ],
            ("-n", "GenericLabel", "-u", "uchar"),
        ),
        (
            "flair-to-T1w/identity",
            "an 82-year-old's FLAIR of 0.42 x 0.42 x 3 mm voxels onto her T1w's 1 mm grid "
            "through the scanner's coordinates (identity), Lanczos",
            [P.FLAIR_OLDER],
            [
                ("-i", P.FLAIR_OLDER.arg),
                ("-r", run_file(older, "sub-*/ses-*/anat/*_desc-preproc_T1w.nii.gz")),
                ("-t", "identity"),
            ],
            ("-n", "LanczosWindowedSinc"),
        ),
    ]  # fmt: skip
    for cid, description, images, pairs, extra in specs:
        if any(v is None for _, v in pairs):
            continue
        raw = ("-d", "3", *(w for pair in pairs for w in pair), *extra, "-o", "{out:out.nii.gz}")
        out.append(
            (
                P.PREFIX + cid,
                P.CATEGORY,
                P.describe(description, *images),
                Scenario("", "", interpolation=_interpolation(raw), tier="panel", raw=raw),
            )
        )
    return out


def cases(tier: str) -> list[Case]:
    panel = _panel_scenarios() if tier == "panel" else []
    return [
        Case(cid, category, desc, scenario)
        for cid, category, desc, scenario in [*_scenarios(), *outputs.scenarios(), *panel]
        if in_tier(scenario.tier, tier)
    ]


# ------------------------------------------------------------------------------------------------
# Running


def _resolve(path: str, tmp: Path) -> str:
    if path in ("identity", "missing.mat"):
        return path if path == "identity" else str(tmp / path)
    if path == "gen:strtof-affine.txt":
        # A centred affine near the identity, each number the float-rounding trap scaled.
        out = tmp / "strtof-affine.txt"
        matrix = [
            "1.0000001788139343261718749999999999",
            STRTOF_TRAP,
            "0",
            "0",
            "0.99999997019767761230468749999999999",
            "0",
            "0",
            "0",
            "1",
        ]
        translation = ["1.5000001788139343261718749999999999", "-2", "0.75"]
        center = [STRTOF_TRAP, "-3.0000003576278686523437499999999999", "1"]
        out.write_text(
            "#Insight Transform File V1.0\n#Transform 0\n"
            "Transform: AffineTransform_double_3_3\n"
            f"Parameters: {' '.join(matrix + translation)}\n"
            f"FixedParameters: {' '.join(center)}\n",
            encoding="utf-8",
        )
        return str(out)
    if path.startswith("gen-bspline:"):
        return str(ants_bspline_transforms.bspline_file(path.removeprefix("gen-bspline:"), tmp))
    if path.startswith("ants-h5:"):
        # A file ITK writes (the oracle's antsApplyTransforms or CompositeTransformUtil).
        from larmorx_validation.parity import itk_hdf5

        return str(itk_hdf5.itk_file(path.removeprefix("ants-h5:"), tmp))
    if path.startswith("larmorx:"):
        import larmorx as lx

        inner = path.removeprefix("larmorx:")
        source = Path(_resolve(inner, tmp)) if inner.startswith("ants-h5:") else td.get(inner)
        out = tmp / ("larmorx-" + source.name)
        lx.transforms.write(out, lx.transforms.read(source))
        return str(out)
    return str(_catalog(path))


def _catalog(path: str) -> Path:
    """A catalog file, or (``dt:<key>``) an input of a differential test's case
    (:mod:`larmorx_validation.difftest.inputs`)."""
    if path.startswith("dt:"):
        from larmorx_validation.difftest import inputs

        return inputs.path(path)
    return td.get(path)


def _args(s: Scenario, tmp: Path, output: Path) -> list[str]:
    if s.raw:
        import re

        def sub(m: re.Match[str]) -> str:
            key = m.group(1)
            if key.startswith("out:"):
                return str(output)
            if key.startswith("in:"):
                return str(td.get(key[3:]))
            raise KeyError(f"unknown placeholder {{{key}}}")

        return [re.sub(r"\{([^{}]+)\}", sub, a) for a in s.raw]
    args = ["-d", "3", "-i", str(_catalog(s.image)), "-r", str(_catalog(s.reference))]
    for path, invert in s.transforms:
        resolved = _resolve(path, tmp)
        args += ["-t", f"[{resolved},1]" if invert else resolved]
    return [*args, "-n", s.interpolation, *s.extra, "-o", str(output)]


def _oracle() -> Path | None:
    """The ANTs oracle's ``antsApplyTransforms`` (``LARMORX_ANTS_BIN``, else the workspace's
    ``oracles/ants-2.6.5/bin``), or ``None``: then ANTsPy runs it in-process."""
    b = ants_bin()
    return b / "antsApplyTransforms" if b and (b / "antsApplyTransforms").is_file() else None


def _run_ants(args: list[str], *, correctly_rounded: bool = False) -> int:
    """Run antsApplyTransforms; with ``correctly_rounded``, with CORE-MATH's functions
    preloaded (the oracle binary only)."""
    if (program := _oracle()) is not None:
        from larmorx_validation import oracle_cache

        env = {**os.environ}
        env.setdefault("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "4")
        if correctly_rounded:
            lib = crmath_preload()
            if lib is None:
                raise FileNotFoundError("libcrmath.so (scripts/build_crmath_preload.sh)")
            env["LD_PRELOAD"] = str(lib)
        argv = [str(program), *args]
        p = oracle_cache.run(
            argv,
            env=env,
            policy=oracle_cache.Policy(threads=oracle_cache.itk_threads(env), family="itk"),
            timeout=1800,
            runner="ants-apply-transforms",
        )
        if p.timed_out:
            raise subprocess.TimeoutExpired(argv, 1800)
        return p.returncode
    import ants

    with _quiet_stderr(), contextlib.redirect_stdout(io.StringIO()):
        return int(ants.internal.get_lib_fn("antsApplyTransforms")(args))


def _run_larmorx(args: list[str], mode: str = defects.REPRODUCE) -> tuple[int, str]:
    """``larmorx ants antsApplyTransforms`` in-process; ``mode`` is D13's
    ``LARMORX_UPSTREAM_DEFECTS``: ``reproduce`` for the comparison with ANTs."""
    from larmorx.cli import run

    err = io.StringIO()
    with (
        defects.mode(mode),
        contextlib.redirect_stderr(err),
        contextlib.redirect_stdout(io.StringIO()),
    ):
        code = run(["larmorx", "ants", "antsApplyTransforms", *args])
    return code, err.getvalue().strip()


def _fix_pass(s: Scenario, tmp: Path, checks: CheckList, code: int, err: str, out: Path) -> bool:
    """D13: larmorx's default (``fix``) on the same arguments, compared with its
    ``reproduce`` run (exit ``code``, standard error ``err``, output ``out``): the defects
    named, and what the corrections changed. Returns whether the check passed."""
    import nibabel as nib

    target = tmp / "larmorx-fix.nii.gz"
    fix_code, fix_err = _run_larmorx(_args(s, tmp, target), mode=defects.FIX)
    ok, r_ok = fix_code == 0 and target.exists(), code == 0 and out.exists()
    other = []
    if fix_code != code:
        other.append(f"exit {code} -> {fix_code}")
    if ok != r_ok:
        other.append("outputs written" if ok else "outputs not written")
    change = defects.Change(0, 0, 0.0)
    if ok and r_ok:
        fields = [d.split(":")[0] for d in header_differences(raw_header(out), raw_header(target))]
        if fields:
            other.append(f"header {', '.join(fields)}")
        change = defects.Change.of(
            np.asanyarray(nib.load(target).dataobj), np.asanyarray(nib.load(out).dataobj)
        )
    entries = defects.notices_in(fix_err)
    check = defects.add_fix_check(checks, entries, change, other_changes="; ".join(other))
    if ok and r_ok and entries != defects.notices_in(err):
        checks(
            "D13 notices in both modes",
            False,
            detail=f"fix: {entries}; reproduce: {defects.notices_in(err)}",
        )
        return False
    return check.passed


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

    if isinstance(case.payload, outputs.OutputScenario):
        with tempfile.TemporaryDirectory(prefix="lx-aat-") as tmp_name:
            outputs.run_output_case(case.id, case.payload, checks, Path(tmp_name))
        return
    s: Scenario = case.payload
    with tempfile.TemporaryDirectory(prefix="lx-aat-") as tmp_name:
        tmp = Path(tmp_name)
        ants_out, lx_out = tmp / "ants.nii.gz", tmp / "larmorx.nii.gz"
        ants_code = _run_ants(_args(s, tmp, ants_out))
        lx_code, lx_err = _run_larmorx(_args(s, tmp, lx_out))
        ants_ok = ants_code == 0 and ants_out.exists()
        lx_ok = lx_code == 0 and lx_out.exists()
        if not ants_ok and not lx_ok:
            if _fix_pass(s, tmp, checks, lx_code, lx_err, lx_out):
                raise Outcome(BOTH_ERROR, f"ANTs exit {ants_code}; larmorx: {lx_err or lx_code}")
            return
        checks(
            "both succeed",
            ants_ok and lx_ok,
            detail=f"ANTs exit {ants_code}; larmorx exit {lx_code}: {lx_err}",
        )
        if s.expect_error:
            checks("both reject the arguments", False, detail="expected an error from both")
        if not (ants_ok and lx_ok):
            return
        _fix_pass(s, tmp, checks, lx_code, lx_err, lx_out)
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
        affine_diff = affine_difference(a_img.affine, l_img.affine)
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
        if _oracle() is None or crmath_preload() is None:
            return
        # ANTs with correctly rounded maths, as larmorx computes.
        cr_out = tmp / "ants-cr.nii.gz"
        cr_code = _run_ants(_args(s, tmp, cr_out), correctly_rounded=True)
        if cr_code != 0 or not cr_out.exists():
            checks("ANTs with CORE-MATH succeeds", False, detail=f"exit {cr_code}")
            return
        c = np.asanyarray(nib.load(cr_out).dataobj)
        cr_differing = int(np.count_nonzero(c != b)) if c.shape == b.shape else b.size
        checks.add(
            Check(
                "bit-identical with CORE-MATH",
                True,
                metric="differing voxels",
                value=cr_differing,
                threshold="reported",
                detail="identical"
                if cr_differing == 0
                else f"{cr_differing} voxels differ; at most {_ulps(c, b)} ulp",
            )
        )


def _identical(r: CaseResult, name: str) -> bool | None:
    for c in r.checks:
        if c.name == name:
            return c.value == 0
    return None


@functools.cache
def _panel_float_ids() -> frozenset[str]:
    """The panel cases whose command line has ``--float 1`` (fMRIPrep's own calls as written,
    and the panel's own)."""

    def single(words) -> bool:
        return any(a == "--float" and b == "1" for a, b in itertools.pairwise(words))

    return frozenset(
        cid
        for cid, _cat, _desc, sc in _panel_scenarios()
        if single(sc.raw or ()) or single(sc.extra)
    )


def _highlights(results: list[CaseResult]) -> list[str]:
    produced = [r for r in results if r.case.startswith("transform-output/") and r.status == PASS]
    compared = [
        r for r in results if r.status == PASS and not r.case.startswith("transform-output/")
    ]
    single = [
        r
        for r in compared
        if r.case.startswith("float/")
        or r.case.startswith("output/float-")
        or r.case in _panel_float_ids()
    ]
    lines = []
    for label, group in (("all cases", compared), ("`--float 1` cases", single)):
        identical = [r for r in group if _identical(r, "bit-identical")]
        with_cr = [r for r in group if _identical(r, "bit-identical with CORE-MATH") is not None]
        cr_identical = [r for r in with_cr if _identical(r, "bit-identical with CORE-MATH")]
        line = (
            f"**Bit-identical to the unmodified antsApplyTransforms, {label}: {len(identical)} "
            f"of {len(group)}** passing cases produce exactly the bytes of its output data"
        )
        if with_cr:
            line += (
                f"; with CORE-MATH's correctly rounded functions preloaded into ANTs, "
                f"{len(cr_identical)} of {len(with_cr)}"
            )
        lines.append(line + ".")
    differ = sorted(r.case for r in compared if not _identical(r, "bit-identical"))
    if differ:
        explained = [
            c
            for c in differ
            if _identical(next(r for r in compared if r.case == c), "bit-identical with CORE-MATH")
        ]
        rest = [c for c in differ if c not in explained]
        lines.append(
            "Differing from ANTs as built only through glibc's `exp`, `log`, `sin`, `cos`, "
            "`sincos`, `sinf`, `cosf` or `sincosf` (bit-identical with CORE-MATH preloaded; "
            "Gaussian and windowed-sinc weights, Euler and versor matrices; "
            "docs/findings/platform-math.md): "
            + (", ".join(f"`{c}`" for c in explained) or "none")
            + "."
        )
        if rest:
            lines.append(
                "**Not explained by the maths library**: " + ", ".join(f"`{c}`" for c in rest) + "."
            )
    if produced:
        exact = [r for r in produced if all(c.passed for c in r.checks)]
        h5 = [
            c
            for r in produced
            for c in r.checks
            if c.name.endswith("bytes, ITK's strings and object times")
        ]
        h5_same = [c for c in h5 if c.value == 0]
        h5_chunks = [c for c in h5 if c.passed]
        lines.append(
            f"**Transform outputs** (`-o Linear[...]`, `-o CompositeTransform[...]`, "
            f"`-o DisplacementField[...]`): {len(exact)} of {len(produced)} cases write ANTs's "
            "files: text and MATLAB files byte for byte, NIfTI fields header and vectors bit for "
            "bit, HDF5 files dataset for dataset"
            + (
                f"; written with ITK's strings and object times, {len(h5_chunks)} of {len(h5)} "
                f"HDF5 files are ITK's byte for byte given ITK's compressed chunks, {len(h5_same)} "
                "with larmorx's own deflate"
                if h5
                else ""
            )
            + "."
        )
    lines += defects.summary(results)
    from larmorx_validation.parity import fmriprep_nodes

    return lines + fmriprep_nodes.record_lines("antsApplyTransforms", results)


def suite() -> Suite:
    os.environ.setdefault("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "4")
    import ants

    threads = os.environ["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"]
    if _oracle() is not None:
        reference = (
            f"antsApplyTransforms from {ants_version()}, the binary built by "
            f"`scripts/build_ants_oracle.sh`, run with {threads} threads"
        )
    else:
        reference = (
            "antsApplyTransforms from ANTs 2.6.5 on ITK 5.4.5, run in-process through ANTsPy "
            f"{ants.__version__} (the oracle binary was not found)"
        )
    return Suite(
        name="ants-apply-transforms",
        title="antsApplyTransforms",
        tool="`larmorx ants antsApplyTransforms` / `lx.ants.apply_transforms` (crates larmorx-ants, -interp, -transform, -io)",
        reference=reference,
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
            (
                "Transform outputs",
                "text and MATLAB files byte for byte; HDF5 files: every dataset bit for bit "
                "(bytes reported, object times set alike); NIfTI fields: header bytes and every "
                "vector bit for bit (differing vectors reported)",
            ),
            (
                "Reported",
                "bit-identity with ANTs as built, and with ANTs with CORE-MATH's correctly "
                "rounded functions preloaded (`LD_PRELOAD`, the oracle binary only)",
            ),
        ],
        cases=cases,
        run_case=run_case,
        packages=("antspyx", "nibabel"),
        highlights=_highlights,
        notes=(
            "Both programs read the same files with the same arguments; the outputs are read with "
            "nibabel. larmorx runs through its Python console entry point; its command line reads "
            "`.h5` transforms in Rust (crate larmorx-hdf5). Where ITK calls the platform's transcendental functions, larmorx uses "
            "correctly rounded ones (`larmorx_core::math`; the `libm` crate where none is ported "
            "yet), the same bits on every platform, which can differ from glibc in the last bit "
            "(docs/findings/platform-math.md). With `--float 1` both compute in single precision "
            "as ITK's float instantiation defines it (docs/findings/ants-float.md)."
        ),
    )
