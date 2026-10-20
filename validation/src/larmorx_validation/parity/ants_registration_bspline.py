# SPDX-License-Identifier: Apache-2.0
"""The B-spline and displacement-field cases of the ``ants-registration`` parity suite (A5
part 2): every option of the ``BSplineSyN``, ``GaussianDisplacementField``,
``BSplineDisplacementField`` and ``BSpline`` stages, their outputs (``Warp`` images,
``BSpline.txt``, composites), B-spline transforms among the initial transforms, and every
message and failure.

The phantoms are the suite's (``gen:reg-*``); the B-spline transform the cases start from is
built here (``{gen-bsp:<name>}``): a cubic B-spline of mesh 2 x 2 x 2 over the fixed phantom
with smooth coefficients of a few millimetres, written as ITK writes it.
"""

from __future__ import annotations

from pathlib import Path

import numpy as np

from larmorx_validation.parity.ants_registration_syn import (
    PSF,
    PSI,
    PSIM,
    PSK,
    PSKM,
    PSM,
    _stage,
)

F, M, K = "{gen:reg-fixed}", "{gen:reg-moving}", "{gen:reg-mask}"
F2, M2 = "{gen:reg-fixed-2d}", "{gen:reg-moving-2d}"
F4, M4 = "{gen:reg-fixed-4d}", "{gen:reg-moving-4d}"
FO, MO = "{gen:reg-fixed-oblique}", "{gen:reg-moving-oblique}"
KO = "{gen:reg-mask-oblique}"
OUT = "[{out}/r,{out}/w.nii.gz,{out}/iw.nii.gz]"
PRE = "{out}/r"
WARPED = "[{out}/r,{out}/w.nii.gz]"
BSYN, FIELD, BSP = "bspline-syn", "displacement-field", "bspline"


def transform_file(tmp: Path, name: str) -> Path:
    """A file the cases start from (`{gen-bsp:<name>}`): ``bspline-3d.txt``, a
    ``BSplineTransform_double_3_3`` of mesh 2 x 2 x 2 over the fixed phantom (its grid in
    ITK's LPS axes) with smooth coefficients of up to about 2 mm; ``bspline-3d.h5``, the same
    in an HDF5 composite after an affine transform."""
    import larmorx as lx

    path = tmp / name
    if path.exists():
        return path
    size, mesh, order = np.array([40, 44, 36]), np.array([2, 2, 2]), 3
    origin = np.array([40.0, 44.0, -36.0])
    direction = np.diag([-1.0, -1.0, 1.0])
    grid_spacing = 2.0 * (size - 1) / mesh
    grid_origin = origin + direction @ (-0.5 * grid_spacing * (order - 1))
    n = mesh + order
    fixed = [*map(float, n), *grid_origin, *grid_spacing, *direction.ravel()]
    idx = np.indices(tuple(n), dtype=float)
    coefficients = []
    for k in range(3):
        c = 2.0 * np.sin(0.7 * idx[0] + 0.5 * idx[1] + 0.3 * idx[2] + k)
        # ITK's coefficient images: x fastest.
        coefficients.append(c.transpose(2, 1, 0).ravel())
    bspline = lx.transforms.ItkTransform(
        "BSplineTransform_double_3_3", np.concatenate(coefficients), np.array(fixed)
    )
    if name == "bspline-3d.txt":
        lx.transforms.write(path, bspline)
    elif name == "bspline-3d.h5":
        th = np.deg2rad(3.0)
        matrix = np.array([[np.cos(th), -np.sin(th), 0], [np.sin(th), np.cos(th), 0], [0, 0, 1]])
        affine = lx.transforms.ItkTransform(
            "AffineTransform_double_3_3",
            np.concatenate([matrix.ravel(), [1.5, -1.0, 0.5]]),
            [1.0, -2.0, 0.5],
        )
        head = lx.transforms.ItkTransform("CompositeTransform_double_3_3", [], [])
        lx.transforms.write(path, [head, affine, bspline])
    else:
        raise KeyError(f"unknown B-spline input {name!r}")
    return path


def bspline_cases(make) -> list:
    """The cases; `make(id, category, description, *args, d=, out=, **kw)` builds one."""
    return _bspline_syn(make) + _fields(make) + _bspline(make)


def _bspline_syn(make) -> list:
    cc = f"CC[{F},{M},1,2]"
    ms = f"MeanSquares[{F},{M},1]"
    one = {"c": "[3,1e-6,3]", "f": "1", "s": "0"}
    affine = _stage("Affine[0.1]", [ms], c="[6,1e-6,5]", f="1", s="0vox")

    def bsyn(name: str, description: str, *args: str, **kw):
        return make(f"{BSYN}/{name}", BSYN, description, *args, **kw)

    return [
        bsyn("cc", "BSplineSyN (mesh 2x2x2) with CC (radius 2), 2 levels, warped images",
             *_stage("BSplineSyN[0.2,2x2x2,0,3]", [cc]), band=True),
        bsyn("knot-spacing", "BSplineSyN[0.1,26,0,3] (antsRegistrationSyN.sh's): a knot spacing of 26 mm",
             *_stage("BSplineSyN[0.1,26,0,3]", [ms]), out=PRE, band=True),
        bsyn("total-mesh", "a total-field mesh (3x3x3, update 4x4x4)",
             *_stage("BSplineSyN[0.25,4x4x4,3x3x3,3]", [ms]), out=PRE),
        bsyn("order-2", "B-splines of order 2", *_stage("BSplineSyN[0.25,3x3x3,0,2]", [ms]), out=PRE),
        bsyn("order-5", "B-splines of order 5", *_stage("BSplineSyN[0.25,2x2x2,0,5]", [ms], **one), out=PRE),
        bsyn("mesh-0", "an update mesh of 0: the update field is not smoothed",
             *_stage("BSplineSyN[0.25,0x0x0,0,3]", [ms], **one), out=PRE),
        bsyn("mattes", "Mattes (24 bins)", *_stage("BSplineSyN[0.25,2x2x2,0,3]", [f"Mattes[{F},{M},1,24]"]), out=PRE),
        bsyn("mi2", "the joint-histogram MI (MI2)", *_stage("BSplineSyN[0.25,2x2x2,0,3]", [f"MI2[{F},{M},1,20]"]), out=PRE),
        bsyn("gc", "GC: ITK adds every point's derivative to the first voxel (ITK-30)",
             *_stage("BSplineSyN[0.25,2x2x2,0,3]", [f"GC[{F},{M},1]"]), out=PRE),
        bsyn("demons", "Demons", *_stage("BSplineSyN[0.25,2x2x2,0,3]", [f"Demons[{F},{M},1]"]), out=PRE),
        bsyn("multi", "CC (0.7) and Mattes (0.3)",
             *_stage("BSplineSyN[0.25,2x2x2,0,3]", [f"CC[{F},{M},0.7,2]", f"Mattes[{F},{M},0.3,20]"]), out=PRE),
        bsyn("masks", "-x [mask,mask]: the B-spline fits weighted by the masks resampled through the fields",
             "-x", f"[{K},{K}]", *_stage("BSplineSyN[0.25,2x2x2,0,3]", [ms]), out=PRE),
        bsyn("fixed-mask-only", "a fixed mask on another grid, no moving mask",
             "-x", KO, *_stage("BSplineSyN[0.25,2x2x2,0,3]", [ms], **one), out=PRE),
        bsyn("regular", "MeanSquares, regular sampling 0.5",
             *_stage("BSplineSyN[0.25,2x2x2,0,3]", [f"MeanSquares[{F},{M},1,1,Regular,0.5]"]), out=PRE),
        bsyn("random", "Mattes, random sampling 0.6",
             *_stage("BSplineSyN[0.25,2x2x2,0,3]", [f"Mattes[{F},{M},1,24,Random,0.6]"]), out=PRE),
        bsyn("float", "--float 1", "--float", "1", *_stage("BSplineSyN[0.2,2x2x2,0,3]", [cc]), out=PRE),
        bsyn("2d", "2D, CC", *_stage("BSplineSyN[0.3,3x3,0,3]", [f"CC[{F2},{M2},1,2]"]), d="2", out=PRE),
        bsyn("4d", "4D, MeanSquares", *_stage("BSplineSyN[0.3,2x2x2x1,0,3]", [f"MeanSquares[{F4},{M4},1]"], **one),
             d="4", out=PRE, tier="full"),
        bsyn("affine-then", "Affine then BSplineSyN, collapsed: 0GenericAffine.mat, 1Warp, 1InverseWarp",
             *affine, *_stage("BSplineSyN[0.2,2x2x2,0,3]", [cc])),
        bsyn("composite", "Affine then BSplineSyN, -a 1", "-a", "1",
             *affine, *_stage("BSplineSyN[0.2,2x2x2,0,3]", [cc]), out=PRE),
        bsyn("no-collapse", "Affine then BSplineSyN, -z 0", "-z", "0",
             *affine, *_stage("BSplineSyN[0.2,2x2x2,0,3]", [cc]), out=PRE),
        bsyn("per-stage", "-i 1: the listing of the composite (a BSplineSmoothingOnUpdateDisplacementFieldTransform) for the next stage",
             "-a", "1", "-z", "0", "-i", "1",
             *_stage("BSplineSyN[0.2,2x2x2,0,3]", [ms], **one),
             *_stage("GaussianDisplacementField[0.2,3,0]", [ms], **one), out=PRE),
        bsyn("restrict-deformation", "-g 1x1x0", "-g", "1x1x0",
             *_stage("BSplineSyN[0.25,2x2x2,0,3]", [ms]), out=PRE),
        bsyn("save-state", "--save-state: BSplineSyN does not set the registration state (ANTS-50)",
             "-j", "{out}/state.h5", *_stage("BSplineSyN[0.25,2x2x2,0,3]", [ms], **one), out=PRE, expect="error"),
        bsyn("initial-field", "-r a displacement field", "-r", "{gen-syn:warp-3d.nii.gz}",
             *_stage("BSplineSyN[0.25,2x2x2,0,3]", [ms], **one), out=PRE),
        bsyn("initial-bspline", "-r a B-spline transform: collapsed, ANTs loses the stage's fields (ANTS-76)",
             "-r", "{gen-bsp:bspline-3d.txt}", *_stage("BSplineSyN[0.25,2x2x2,0,3]", [ms], **one), out=PRE),
        bsyn("icp", "ICP alone: each fixed point's tangent-space derivative fitted by a B-spline",
             "-x", PSK, *_stage("BSplineSyN[0.3,2x2x2,0,3]", [f"ICP[{PSF},{PSM},1]"], f="2x1", s="1x0vox"), out=PRE),
        bsyn("cc-and-icp", "CC and ICP (ITK-33)",
             "-x", PSK, *_stage("BSplineSyN[0.3,2x2x2,0,3]", [f"CC[{PSI},{PSIM},1,2]", f"ICP[{PSF},{PSM},0.5]"]), out=PRE),
        bsyn("igdm", "IGDM alone", "-x", PSK,
             *_stage("BSplineSyN[0.3,2x2x2,0,3]", [f"IGDM[{PSI},{PSIM},1,{PSK},{PSKM}]"], f="2x1", s="1x0vox"), out=PRE),
        bsyn("mesh-dimension", "a 2D mesh in 3D: ERROR, the mesh sizes do not match the dimension",
             *_stage("BSplineSyN[0.25,2x2,0,3]", [ms]), out=PRE, expect="error"),
        bsyn("order-0", "spline order 0: the B-spline filter throws",
             *_stage("BSplineSyN[0.25,2x2x2,0,0]", [ms], **one), out=PRE, expect="error"),
        bsyn("bad-mesh", "a mesh that does not convert", *_stage("BSplineSyN[0.25,2xa,0,3]", [ms]),
             out=PRE, expect="error"),
    ]  # fmt: skip


def _fields(make) -> list:
    cc = f"CC[{F},{M},1,2]"
    ms = f"MeanSquares[{F},{M},1]"
    one = {"c": "[3,1e-6,3]", "f": "1", "s": "0"}
    affine = _stage("Affine[0.1]", [ms], c="[6,1e-6,5]", f="1", s="0vox")
    gdf = "GaussianDisplacementField[0.25,3,0.5]"
    bdf = "BSplineDisplacementField[0.25,3x3x3,0,3]"

    def field(name: str, description: str, *args: str, **kw):
        return make(f"{FIELD}/{name}", FIELD, description, *args, **kw)

    return [
        field("gaussian-cc", "GaussianDisplacementField with CC (radius 2), 2 levels, warped images",
              *_stage(gdf, [cc]), out=WARPED, band=True),
        field("gaussian-meansquares", "MeanSquares, no total-field smoothing",
              *_stage("GaussianDisplacementField[0.25,2,0]", [ms]), out=PRE),
        field("gaussian-mattes", "Mattes (24 bins)", *_stage(gdf, [f"Mattes[{F},{M},1,24]"]), out=PRE),
        field("gaussian-mi2", "MI2", *_stage(gdf, [f"MI2[{F},{M},1,20]"]), out=PRE),
        field("gaussian-gc", "GC (ITK-30)", *_stage(gdf, [f"GC[{F},{M},1]"]), out=PRE),
        field("gaussian-demons", "Demons", *_stage(gdf, [f"Demons[{F},{M},1]"]), out=PRE),
        field("gaussian-multi", "CC and Mattes", *_stage(gdf, [f"CC[{F},{M},0.7,2]", f"Mattes[{F},{M},0.3,20]"]), out=PRE),
        field("gaussian-masks", "-x [mask,mask]", "-x", f"[{K},{K}]", *_stage(gdf, [ms]), out=PRE),
        field("gaussian-regular", "regular sampling 0.5", *_stage(gdf, [f"MeanSquares[{F},{M},1,1,Regular,0.5]"]), out=PRE),
        field("gaussian-random", "random sampling 0.6", *_stage(gdf, [f"Mattes[{F},{M},1,24,Random,0.6]"]), out=PRE),
        field("gaussian-float", "--float 1", "--float", "1", *_stage(gdf, [cc]), out=PRE),
        field("gaussian-2d", "2D", *_stage("GaussianDisplacementField[0.3,2,0]", [f"CC[{F2},{M2},1,2]"]), d="2", out=PRE),
        field("gaussian-oblique", "oblique grids", *_stage(gdf, [f"MeanSquares[{FO},{MO},1]"], **one), out=PRE),
        field("gaussian-4d", "4D, MeanSquares", *_stage("GaussianDisplacementField[0.3,2,0]", [f"MeanSquares[{F4},{M4},1]"], **one),
              d="4", out=PRE, tier="full"),
        field("bspline-4d", "4D B-spline field, MeanSquares",
              *_stage("BSplineDisplacementField[0.3,4x4x4x2,0,3]", [f"MeanSquares[{F4},{M4},1]"], **one),
              d="4", out=PRE, tier="full"),
        field("gaussian-affine-then", "Affine then the field, collapsed: 0GenericAffine.mat, 1Warp (no inverse)",
              *affine, *_stage(gdf, [ms])),
        field("gaussian-composite", "-a 1: the composite holds the field; no inverse composite", "-a", "1",
              *affine, *_stage(gdf, [ms]), out=PRE),
        field("gaussian-no-collapse", "-z 0", "-z", "0", *affine, *_stage(gdf, [ms]), out=PRE),
        field("gaussian-restrict", "-g 1x1x0", "-g", "1x1x0", *_stage(gdf, [ms]), out=PRE),
        field("gaussian-icp", "MeanSquares and ICP: each point's derivative at its virtual voxel",
              *_stage(gdf, [ms, f"ICP[{K},{K},0.5]"]), out=PRE),
        field("gaussian-initial-field", "-r a displacement field", "-r", "{gen-syn:warp-3d.nii.gz}",
              *_stage(gdf, [ms], **one), out=PRE),
        field("gaussian-print-similarity", "--print-similarity-measure-interval: the observer's copy of the field fails (ANTS-75)",
              "--print-similarity-measure-interval", "1", *_stage(gdf, [ms], **one), out=PRE, expect="error"),
        field("gaussian-interval-volumes", "--write-interval-volumes: the observer's copy of the field fails (ANTS-75)",
              "--write-interval-volumes", "1", *_stage(gdf, [ms], **one), out=PRE, expect="error"),
        field("gaussian-save-state", "--save-state: no state (ANTS-50)", "-j", "{out}/state.h5",
              *_stage(gdf, [ms], **one), out=PRE, expect="error"),
        field("gaussian-igdm", "IGDM first: not implemented in ANTs", "-x", PSK,
              *_stage(gdf, [f"IGDM[{PSI},{PSIM},1,{PSK},{PSKM}]"], **one), out=PRE, expect="error"),
        field("gaussian-icp-first", "ICP first: ANTs allocates the field on a null image",
              "-x", PSK, *_stage(gdf, [f"ICP[{PSF},{PSM},1]"], **one), out=PRE, expect="crash"),
        field("gaussian-variance-missing", "GaussianDisplacementField[0.25,3]: the total variance is required",
              *_stage("GaussianDisplacementField[0.25,3]", [ms]), out=PRE, expect="error"),
        field("bspline-meansquares", "BSplineDisplacementField (mesh 3x3x3, doubled at the first level), MeanSquares, warped",
              *_stage(bdf, [ms]), out=WARPED, band=True),
        field("bspline-cc", "CC", *_stage(bdf, [cc]), out=PRE),
        field("bspline-total-mesh", "a total-field mesh (4x4x4, update 5x5x5)",
              *_stage("BSplineDisplacementField[0.25,5x5x5,4x4x4,3]", [ms]), out=PRE),
        field("bspline-coarse-total", "a total mesh of 1: the stationary boundary dominates the fit, the scales fall below epsilon and the stage fails",
              *_stage("BSplineDisplacementField[0.3,3x3x2,1x1x1,3]", [ms]), out=PRE, expect="error"),
        field("bspline-order-2", "order 2", *_stage("BSplineDisplacementField[0.25,3x3x3,0,2]", [ms]), out=PRE),
        field("bspline-knot", "a knot spacing (12 mm)", *_stage("BSplineDisplacementField[0.3,12,0,3]", [ms]), out=PRE),
        field("bspline-mattes", "Mattes", *_stage(bdf, [f"Mattes[{F},{M},1,24]"]), out=PRE),
        field("bspline-masks", "-x [mask,mask]", "-x", f"[{K},{K}]", *_stage(bdf, [ms]), out=PRE),
        field("bspline-float", "--float 1", "--float", "1", *_stage(bdf, [ms]), out=PRE),
        field("bspline-2d", "2D", *_stage("BSplineDisplacementField[0.3,3x3,0,3]", [f"MeanSquares[{F2},{M2},1]"]), d="2", out=PRE),
        field("bspline-mesh-dimension", "a 2D mesh in 3D: ERROR", *_stage("BSplineDisplacementField[0.25,3x3,0,3]", [ms]),
              out=PRE, expect="error"),
        field("bspline-order-0", "order 0: the B-spline filter throws",
              *_stage("BSplineDisplacementField[0.25,3x3x3,0,0]", [ms], **one), out=PRE, expect="error"),
        field("gaussian-then-bspline", "the two field stages, collapsed into one field", *_stage(gdf, [ms]),
              *_stage(bdf, [ms])),
    ]  # fmt: skip


def _bspline(make) -> list:
    ms = f"MeanSquares[{F},{M},1]"
    one = {"c": "[3,1e-6,3]", "f": "1", "s": "0"}
    two = {"c": "[3x2,1e-6,3]", "f": "2x1", "s": "1x0vox"}
    affine = _stage("Affine[0.1]", [ms], c="[6,1e-6,5]", f="1", s="0vox")
    b = "BSpline[0.2,2x2x2]"

    def bsp(name: str, description: str, *args: str, **kw):
        return make(f"{BSP}/{name}", BSP, description, *args, **kw)

    return [
        bsp("meansquares", "BSpline (mesh 2x2x2, doubled at each level), MeanSquares, 2 levels, warped",
            *_stage(b, [ms], **two), out=WARPED, band=True),
        bsp("cc", "CC (radius 2)", *_stage(b, [f"CC[{F},{M},1,2]"], **one), out=PRE),
        bsp("mattes", "Mattes (24 bins)", *_stage(b, [f"Mattes[{F},{M},1,24]"], **one), out=PRE),
        bsp("mi2", "MI2", *_stage(b, [f"MI2[{F},{M},1,20]"], **one), out=PRE),
        bsp("gc", "GC", *_stage(b, [f"GC[{F},{M},1]"], **one), out=PRE),
        bsp("demons", "Demons: the metric needs a displacement field (ITK's exception)",
            *_stage(b, [f"Demons[{F},{M},1]"], **one), out=PRE, expect="error"),
        bsp("multi", "MeanSquares and CC", *_stage(b, [ms, f"CC[{F},{M},0.5,2]"], **one), out=PRE),
        bsp("masks", "-x [mask,mask]", "-x", f"[{K},{K}]", *_stage(b, [ms], **one), out=PRE),
        bsp("regular", "regular sampling 0.5", *_stage(b, [f"MeanSquares[{F},{M},1,1,Regular,0.5]"], **one), out=PRE),
        bsp("random", "random sampling 0.5", *_stage(b, [f"MeanSquares[{F},{M},1,1,Random,0.5]"], **one), out=PRE),
        bsp("float", "--float 1, 2 levels", "--float", "1", *_stage(b, [ms], **two), out=PRE),
        bsp("2d", "2D", *_stage("BSpline[0.2,3x2]", [f"MeanSquares[{F2},{M2},1]"], **two), d="2", out=PRE),
        bsp("oblique", "oblique grids", *_stage(b, [f"MeanSquares[{FO},{MO},1]"], **one), out=PRE),
        bsp("4d", "4D, MeanSquares", *_stage("BSpline[0.2,2x2x2x1]", [f"MeanSquares[{F4},{M4},1]"], **one),
            d="4", out=PRE, tier="full"),
        bsp("knot-spacing", "a knot spacing (30 mm)", *_stage("BSpline[0.2,30]", [ms], **one), out=PRE),
        bsp("mesh-1", "mesh 1x1x1", *_stage("BSpline[0.2,1x1x1]", [ms], **one), out=PRE),
        bsp("icp", "MeanSquares and ICP", *_stage(b, [ms, f"ICP[{K},{K},0.5]"], **one), out=PRE),
        bsp("affine-then", "Affine then BSpline, collapsed: 0GenericAffine.mat, 1BSpline.txt",
            *affine, *_stage(b, [ms], **one), out=WARPED),
        bsp("composite", "-a 1: the composite holds the B-spline transform; no inverse",
            "-a", "1", *affine, *_stage(b, [ms], **one), out=PRE),
        bsp("no-collapse", "-z 0", "-z", "0", *affine, *_stage(b, [ms], **one), out=PRE),
        bsp("per-stage", "-i 1 -a 1 -z 0: ANTs cannot initialise a B-spline transform from the affine one",
            "-a", "1", "-z", "0", "-i", "1", *affine, *_stage(b, [ms], **one), out=PRE),
        bsp("then-affine", "BSpline then Affine: no Jacobian with respect to position; the Affine stage fails and is dropped (ANTS-49)",
            *_stage(b, [ms], **one), *affine, out=WARPED),
        bsp("then-bspline", "two BSpline stages: the second fails (no Jacobian with respect to position)",
            *_stage(b, [ms], **one), *_stage("BSpline[0.2,1x1x1]", [ms], **one), out=PRE, expect="error"),
        bsp("print-similarity", "--print-similarity-measure-interval: the observer's copy of the transform fails (ANTS-75)",
            "--print-similarity-measure-interval", "1", *_stage(b, [ms], **one), out=PRE, expect="error"),
        bsp("print-similarity-mesh-1", "--print-similarity-measure-interval with mesh 1: the copy works",
            "--print-similarity-measure-interval", "1", *_stage("BSpline[0.2,1x1x1]", [ms], **one), out=PRE),
        bsp("restrict", "-g 1x1x0: weights for the dimension, not the parameters (ITK's exception)",
            "-g", "1x1x0", *_stage(b, [ms], **one), out=PRE, expect="error"),
        bsp("save-state", "--save-state: no state (ANTS-50)", "-j", "{out}/state.h5",
            *_stage(b, [ms], **one), out=PRE, expect="error"),
        bsp("initial-bspline-syn", "-r a B-spline transform, then SyN: collapsed, ANTs loses the SyN fields (ANTS-76)",
            "-r", "{gen-bsp:bspline-3d.txt}", *_stage("SyN[0.25,3,0]", [ms], **one), out=WARPED),
        bsp("initial-bspline-syn-composite", "-r an affine and B-spline composite, then SyN, -a 1",
            "-a", "1", "-r", "{gen-bsp:bspline-3d.h5}", *_stage("SyN[0.25,3,0]", [ms], **one), out=PRE),
        bsp("initial-bspline-affine", "-r a B-spline transform, then Affine: the stage fails and is dropped (ANTS-49)",
            "-r", "{gen-bsp:bspline-3d.txt}", *affine, out=WARPED),
        bsp("fixed-bspline", "-q a B-spline transform", "-q", "{gen-bsp:bspline-3d.txt}", *affine, out=WARPED),
        bsp("mesh-0", "a mesh size of 0: ANTs divides by zero and fails on garbage indices (ANTS-77)",
            *_stage("BSpline[0.2,0x2x2]", [ms], **one), out=PRE, expect="divergence",
            reason="ANTs computes a grid of NaN and infinite geometry, prints vnl's SVD warnings and fails with an exception on garbage indices (ANTS-77); larmorx stops with an error after the same messages"),
        bsp("mesh-short", "a 2D mesh in 3D: ANTs reads past the end of the list (ANTS-77)",
            *_stage("BSpline[0.2,2x2]", [ms], **one), out=PRE, expect="divergence",
            reason="ANTs reads the missing mesh size past the end of its vector (here a 0) and fails as with a mesh size of 0 (ANTS-77); larmorx stops with an error"),
        bsp("icp-first", "ICP first: ANTs reads the null image of the first metric",
            "-x", PSK, *_stage(b, [f"ICP[{PSF},{PSM},1]"], **one), out=PRE, expect="crash"),
    ]  # fmt: skip
