# SPDX-License-Identifier: Apache-2.0
"""The SyN cases of the ``ants-registration`` parity suite (A5 part 1): every option of a SyN
stage, the outputs a SyN stage gives (``Warp``/``InverseWarp`` images, composites, the saved
state), displacement fields among the initial transforms, and every message and failure.

The phantoms are the suite's (``gen:reg-*``); the displacement fields, composites and states
the cases start from are built here (``{gen-syn:<name>}``): smooth fields of a few
millimetres on the phantoms' grids, written as ANTs writes them (NIfTI vector images in ITK's
LPS axes, or ``DisplacementFieldTransform`` entries of HDF5 composites).
"""

from __future__ import annotations

from pathlib import Path

import numpy as np

from larmorx_validation.parity import ants_inputs

F, M, K = "{gen:reg-fixed}", "{gen:reg-moving}", "{gen:reg-mask}"
F2, M2 = "{gen:reg-fixed-2d}", "{gen:reg-moving-2d}"
F4, M4 = "{gen:reg-fixed-4d}", "{gen:reg-moving-4d}"
FO, MO = "{gen:reg-fixed-oblique}", "{gen:reg-moving-oblique}"
KO = "{gen:reg-mask-oblique}"
OUT = "[{out}/r,{out}/w.nii.gz,{out}/iw.nii.gz]"
PRE = "{out}/r"
SYN = "syn"


@ants_inputs.builder("reg-mask-2d")
def _(t):
    """The 2D phantom's head region (uint8)."""
    import nibabel as nib

    a = np.diag([2.0, 2.0, 1.0, 1.0])
    a[:2, 3] = (-40, -44)
    idx = np.indices((40, 44), dtype=float)
    r2 = ((idx[0] - 19.5) / 15) ** 2 + ((idx[1] - 21.5) / 17) ** 2
    nib.save(nib.Nifti1Image((r2 < 0.8).astype(np.uint8), a), t)


@ants_inputs.builder("reg-mask-oblique")
def _(t):
    """The phantom's head region on the oblique fixed grid (uint8): a mask whose grid is not the
    virtual domain's."""
    import nibabel as nib

    a = ants_inputs.oblique_affine()
    a[:3, 3] = (-30, -26, -36)
    idx = np.indices((40, 44, 36), dtype=float)
    c = [19.5, 21.5, 17.5]
    r2 = ((idx[0] - c[0]) / 15) ** 2 + ((idx[1] - c[1]) / 17) ** 2 + ((idx[2] - c[2]) / 13) ** 2
    nib.save(nib.Nifti1Image((r2 < 0.7).astype(np.uint8), a), t)


# ------------------------------------------------------------------------------------------------
# Displacement fields, composites and states


def _grid(dimension: int) -> tuple[tuple[int, ...], np.ndarray]:
    """The fixed phantom's grid in `dimension` dimensions: its shape and RAS affine."""
    if dimension == 2:
        a = np.diag([2.0, 2.0, 1.0, 1.0])
        a[:2, 3] = (-40, -44)
        return (40, 44), a
    a = np.diag([2.0, 2.0, 2.0, 1.0])
    a[:3, 3] = (-40, -44, -36)
    return (40, 44, 36), a


def _field(dimension: int, amplitude: float, phase: float) -> np.ndarray:
    """A smooth field on the fixed grid: per voxel, `dimension` components (mm, LPS)."""
    shape, _ = _grid(dimension)
    idx = np.indices(shape, dtype=float)
    out = np.zeros((*shape, dimension))
    for k in range(dimension):
        arg = sum(idx[j] * (0.11 + 0.03 * j) for j in range(dimension)) + phase + k
        out[..., k] = amplitude * np.sin(arg) * np.exp(-((idx[0] - shape[0] / 2) ** 2) / 300)
    return out


def _lps_geometry(dimension: int) -> list[float]:
    """ITK's fixed parameters of a field on the fixed grid: size, origin, spacing, direction
    (row by row), in LPS."""
    shape, a = _grid(dimension)
    flip = np.diag([-1.0, -1.0] + [1.0] * (dimension - 2))
    rot = a[:dimension, :dimension] / np.array([2.0] * dimension)
    origin = flip @ a[:dimension, 3]
    direction = flip @ rot
    return [
        *map(float, shape),
        *map(float, origin),
        *([2.0] * dimension),
        *map(float, direction.ravel()),
    ]


def _write_field_image(path: Path, field: np.ndarray) -> None:
    """A field as ANTs writes it: a NIfTI vector image (x, y, z, 1, components), double."""
    import nibabel as nib

    dimension = field.shape[-1]
    shape, a = _grid(dimension)
    data = field.reshape(*shape, *([1] * (3 - dimension)), 1, dimension)
    img = nib.Nifti1Image(data.astype(np.float64), a)
    img.header.set_intent("vector")
    nib.save(img, path)


def _field_entry(field: np.ndarray):
    import larmorx as lx

    d = field.shape[-1]
    # ITK's parameters: the field's buffer, the vectors interleaved, x fastest.
    params = np.ascontiguousarray(field.transpose(*reversed(range(d)), d)).reshape(-1)
    return lx.transforms.ItkTransform(
        f"DisplacementFieldTransform_double_{d}_{d}", params, np.array(_lps_geometry(d))
    )


def _affine_entry():
    import larmorx as lx

    th = np.deg2rad(3.0)
    matrix = np.array([[np.cos(th), -np.sin(th), 0], [np.sin(th), np.cos(th), 0], [0, 0, 1]])
    params = np.concatenate([matrix.ravel(), [1.5, -1.0, 0.5]])
    return lx.transforms.ItkTransform("AffineTransform_double_3_3", params, [1.0, -2.0, 0.5])


def transform_file(tmp: Path, name: str) -> Path:
    """A file the SyN cases start from (`{gen-syn:<name>}`):

    - ``warp-3d.nii.gz``, ``warp-2d.nii.gz``: a field image on the fixed grid;
    - ``composite-3d.h5``: an affine transform then a field (no inverse);
    - ``state-3d.h5``: a saved SyN state (an affine transform, then the fixed-to-middle and
      moving-to-middle fields, each followed by its inverse).
    """
    import larmorx as lx

    path = tmp / name
    if path.exists():
        return path
    head = lx.transforms.ItkTransform("CompositeTransform_double_3_3", [], [])
    if name == "warp-3d.nii.gz":
        _write_field_image(path, _field(3, 1.5, 0.0))
    elif name == "warp-2d.nii.gz":
        _write_field_image(path, _field(2, 1.5, 0.0))
    elif name == "composite-3d.h5":
        lx.transforms.write(path, [head, _affine_entry(), _field_entry(_field(3, 1.2, 0.5))])
    elif name == "state-3d.h5":
        lx.transforms.write(
            path,
            [
                head,
                _affine_entry(),
                _field_entry(_field(3, 0.8, 0.2)),
                _field_entry(_field(3, -0.8, 0.2)),
                _field_entry(_field(3, 0.6, 1.1)),
                _field_entry(_field(3, -0.6, 1.1)),
            ],
        )
    else:
        raise KeyError(f"unknown SyN input {name!r}")
    return path


# ------------------------------------------------------------------------------------------------
# Cases


def _stage(
    t: str, metrics: list[str], c: str = "[4x3,1e-6,3]", f: str = "2x1", s: str = "1x0vox"
) -> tuple[str, ...]:
    out: list[str] = ["-t", t]
    for m in metrics:
        out += ["-m", m]
    return (*out, "-c", c, "-f", f, "-s", s)


def syn_cases(make) -> list:
    """The SyN cases; `make(id, category, description, *args, d=, out=, **kw)` builds one."""
    cc = f"CC[{F},{M},1,2]"
    syn = _stage("SyN[0.2,3,0]", [cc])
    rigid = _stage("Rigid[0.1]", [f"MI[{F},{M},1,32,Regular,0.25]"], c="[10,1e-6,5]", f="1", s="0")
    affine = _stage("Affine[0.1]", [f"MeanSquares[{F},{M},1]"], c="[6,1e-6,5]", f="1", s="0vox")
    syn2 = _stage("SyN[0.15,2,0.3]", [f"MeanSquares[{F},{M},1]"], c="[3,1e-6,3]", f="1", s="0")
    cases = [
        make(f"{SYN}/cc", SYN, "SyN with CC (radius 2), 2 levels, warped images", *syn, band=True),
        make(
            f"{SYN}/meansquares-total",
            SYN,
            "SyN with MeanSquares, a total field variance of 0.5",
            *_stage("SyN[0.25,3,0.5]", [f"MeanSquares[{F},{M},1]"]),
            out=PRE,
            band=True,
        ),
        make(
            f"{SYN}/mattes",
            SYN,
            "SyN with Mattes (24 bins), every voxel",
            *_stage("SyN[0.25,3,0]", [f"Mattes[{F},{M},1,24]"]),
            out=PRE,
            band=True,
        ),
        make(
            f"{SYN}/mattes-regular",
            SYN,
            "SyN with Mattes, regular sampling 0.5",
            *_stage("SyN[0.25,3,0]", [f"Mattes[{F},{M},1,24,Regular,0.5]"]),
            out=PRE,
        ),
        make(
            f"{SYN}/mattes-random",
            SYN,
            "SyN with Mattes, random sampling 0.6",
            *_stage("SyN[0.25,3,0]", [f"Mattes[{F},{M},1,24,Random,0.6]"]),
            out=PRE,
        ),
        make(
            f"{SYN}/mi2",
            SYN,
            "SyN with the joint-histogram MI (MI2, ITK-18 reproduced)",
            *_stage("SyN[0.25,3,0]", [f"MI2[{F},{M},1,20]"]),
            out=PRE,
        ),
        make(
            f"{SYN}/demons",
            SYN,
            "SyN with Demons (the fixed image's gradient)",
            *_stage("SyN[0.25,3,0]", [f"Demons[{F},{M},1]"]),
            out=PRE,
        ),
        make(
            f"{SYN}/gc",
            SYN,
            "SyN with GC: ITK adds every point's derivative to the first voxel (ITK-30)",
            *_stage("SyN[0.25,3,0]", [f"GC[{F},{M},1]"]),
            out=PRE,
        ),
        make(
            f"{SYN}/multi",
            SYN,
            "two metrics (CC 0.7, Mattes 0.3): the weights normalised at each evaluation",
            *_stage("SyN[0.25,3,0]", [f"CC[{F},{M},0.7,2]", f"Mattes[{F},{M},0.3,20]"]),
            out=PRE,
        ),
        make(
            f"{SYN}/masks",
            SYN,
            "-x [mask,mask]: both masks resampled through the fields",
            "-x",
            f"[{K},{K}]",
            *_stage("SyN[0.25,3,0]", [f"MeanSquares[{F},{M},1]"]),
            out=PRE,
        ),
        make(
            f"{SYN}/fixed-mask-only",
            SYN,
            "a fixed mask on another grid, no moving mask (ITK-31)",
            "-x",
            KO,
            *_stage("SyN[0.25,3,0]", [f"MeanSquares[{F},{M},1]"], c="[3,1e-6,3]", f="1", s="0"),
            out=PRE,
        ),
        make(
            f"{SYN}/moving-mask-only",
            SYN,
            "-x [NULL,mask] with the mask on another grid (ITK-31)",
            "-x",
            f"[NULL,{KO}]",
            *_stage("SyN[0.25,3,0]", [f"MeanSquares[{F},{M},1]"], c="[3,1e-6,3]", f="1", s="0"),
            out=PRE,
        ),
        make(
            f"{SYN}/rigid-affine-syn",
            SYN,
            "Rigid, Affine then SyN, collapsed: 0GenericAffine.mat, 1Warp, 1InverseWarp",
            "-r",
            f"[{F},{M},1]",
            *rigid,
            *affine,
            *syn,
        ),
        make(
            f"{SYN}/no-collapse",
            SYN,
            "Affine then SyN, -z 0",
            "-z",
            "0",
            *affine,
            *syn,
        ),
        make(
            f"{SYN}/composite",
            SYN,
            "Affine then SyN, -a 1: the composite and its inverse hold the fields",
            "-a",
            "1",
            *affine,
            *syn,
            out=PRE,
        ),
        make(
            f"{SYN}/composite-no-collapse",
            SYN,
            "Affine then SyN, -a 1 -z 0",
            "-a",
            "1",
            "-z",
            "0",
            *affine,
            *syn,
            out=PRE,
        ),
        make(
            f"{SYN}/two-stages",
            SYN,
            "two SyN stages, collapsed into one field",
            *syn,
            *syn2,
        ),
        make(
            f"{SYN}/two-stages-no-collapse",
            SYN,
            "two SyN stages, -z 0",
            "-z",
            "0",
            *syn,
            *syn2,
            out=PRE,
        ),
        make(
            f"{SYN}/save-state",
            SYN,
            "--save-state after SyN: the fixed-to-middle and moving-to-middle fields and inverses",
            "-a",
            "1",
            "--save-state",
            "{out}/state.h5",
            *affine,
            *syn,
            out=PRE,
        ),
        make(
            f"{SYN}/restore-state",
            SYN,
            "--restore-state of a SyN state: the composite ends with SyN's transform",
            "-a",
            "1",
            "-k",
            "{gen-syn:state-3d.h5}",
            *syn,
            out=PRE,
        ),
        make(
            f"{SYN}/restore-state-per-stage",
            SYN,
            "--restore-state with -i 1: SyN starts from the restored fields",
            "-a",
            "1",
            "-z",
            "0",
            "-i",
            "1",
            "-k",
            "{gen-syn:state-3d.h5}",
            *syn,
            out=PRE,
        ),
        make(
            f"{SYN}/per-stage",
            SYN,
            "-i 1: the second SyN stage starts from the first one's fields",
            "-a",
            "1",
            "-z",
            "0",
            "-i",
            "1",
            *affine,
            *syn,
            *syn2,
            out=PRE,
        ),
        make(
            f"{SYN}/interval-volumes",
            SYN,
            "--write-interval-volumes 2",
            "--write-interval-volumes",
            "2",
            *syn,
            out=PRE,
        ),
        make(
            f"{SYN}/print-similarity",
            SYN,
            "--print-similarity-measure-interval 2 with initial transforms on both sides",
            "-r",
            "identity",
            "-q",
            "identity",
            "--print-similarity-measure-interval",
            "2",
            *syn,
            out=PRE,
        ),
        make(
            f"{SYN}/print-similarity-crash",
            SYN,
            "--print-similarity-measure-interval without -q: a null transform (ANTS-71)",
            "--print-similarity-measure-interval",
            "2",
            *syn,
            out=PRE,
            expect="crash",
        ),
        make(
            f"{SYN}/print-similarity-multi-crash",
            SYN,
            "--print-similarity-measure-interval with two metrics: a null metric (ANTS-71)",
            "-r",
            "identity",
            "-q",
            "identity",
            "--print-similarity-measure-interval",
            "2",
            *_stage("SyN[0.2,3,0]", [cc, f"MeanSquares[{F},{M},1]"]),
            out=PRE,
            expect="crash",
        ),
        make(
            f"{SYN}/restrict-deformation",
            SYN,
            "-g 1x0.5x0: the update weighted per axis",
            "-g",
            "1x0.5x0",
            *syn,
            out=PRE,
        ),
        make(
            f"{SYN}/2d",
            SYN,
            "SyN in 2D, three levels",
            *_stage(
                "SyN[0.3,2,0.5]",
                [f"CC[{F2},{M2},1,3]"],
                c="[5x4x3,1e-7,3]",
                f="4x2x1",
                s="2x1x0vox",
            ),
            d="2",
        ),
        make(
            f"{SYN}/2d-float",
            SYN,
            "SyN in 2D, --float 1",
            "--float",
            "1",
            *_stage("SyN[0.3,2,0]", [f"MeanSquares[{F2},{M2},1]"], c="[4x3,1e-7,3]"),
            d="2",
            out=PRE,
        ),
        make(
            f"{SYN}/4d",
            SYN,
            "SyN in 4D",
            *_stage("SyN[0.3,2,0]", [f"MeanSquares[{F4},{M4},1]"], c="[3x2,1e-7,3]"),
            d="4",
            out="[{out}/r,{out}/w.nii.gz]",
        ),
        make(
            f"{SYN}/float",
            SYN,
            "SyN, --float 1",
            "--float",
            "1",
            *syn,
        ),
        make(
            f"{SYN}/float-mattes-mask",
            SYN,
            "SyN with Mattes and a fixed mask, --float 1",
            "--float",
            "1",
            "-x",
            K,
            *_stage("SyN[0.25,3,0]", [f"Mattes[{F},{M},1,24]"]),
            out=PRE,
        ),
        make(
            f"{SYN}/gradient-filter",
            SYN,
            "CC and Demons with the gradient filter",
            *_stage(
                "SyN[0.2,3,0]",
                [f"CC[{F},{M},1,2,None,1,1]", f"Demons[{F},{M},1,0,None,1,1]"],
            ),
            out=PRE,
        ),
        make(
            f"{SYN}/mm-winsorize-histogram",
            SYN,
            "sigmas in mm, -w and -u, an update variance of 0",
            "-w",
            "[0.01,0.99]",
            "-u",
            "1",
            *_stage("SyN[0.2,0,1]", [f"MeanSquares[{F},{M},1]"], s="2x0mm"),
            out=PRE,
        ),
        make(
            f"{SYN}/fixed-initial",
            SYN,
            "-q [fixed,moving,1]: the fixed images resampled through it",
            "-q",
            f"[{F},{M},1]",
            *syn,
        ),
        make(
            f"{SYN}/convergence",
            SYN,
            "a level stopped by the convergence test, a level of 0 iterations",
            *_stage("SyN[0.2,3,0]", [cc], c="[20x0x5,1e-1,4]", f="4x2x1", s="2x1x0vox"),
            out=PRE,
        ),
        make(
            f"{SYN}/oblique",
            SYN,
            "SyN between oblique grids",
            *_stage("SyN[0.2,3,0]", [f"CC[{FO},{MO},1,2]"]),
        ),
        make(
            f"{SYN}/default-variances",
            SYN,
            "SyN[0.2]: the variances default to 3 and 0",
            *_stage("SyN[0.2]", [cc], c="[3,1e-6,3]", f="1", s="0"),
            out=PRE,
        ),
        make(
            f"{SYN}/quiet",
            SYN,
            "Affine then SyN without -v",
            *affine,
            *syn,
            verbose=False,
        ),
        make(
            f"{SYN}/then-linear",
            SYN,
            "SyN then Affine: the linear stage's scales from random voxels (ANTS-72)",
            *syn,
            *affine,
            expect="divergence",
            reason="a linear stage after a displacement field estimates its parameter scales from "
            "voxels drawn with a generator ANTs seeds from the clock (ANTS-72): two runs of ANTs "
            "differ; larmorx seeds it from the registration's seed",
        ),
        make(
            f"{SYN}/initial-field",
            SYN,
            "-r a displacement field image",
            "-r",
            "{gen-syn:warp-3d.nii.gz}",
            *syn,
        ),
        make(
            f"{SYN}/initial-field-linear",
            SYN,
            "-r a displacement field image, then Affine (ANTS-72)",
            "-r",
            "{gen-syn:warp-3d.nii.gz}",
            *affine,
            out=PRE,
            expect="divergence",
            reason="a linear stage after a displacement field estimates its parameter scales from "
            "voxels drawn with a generator ANTs seeds from the clock (ANTS-72)",
        ),
        make(
            f"{SYN}/initial-field-inverse",
            SYN,
            "-r [field,1]: a field without an inverse",
            "-r",
            "[{gen-syn:warp-3d.nii.gz},1]",
            *syn,
            out=PRE,
            expect="error",
        ),
        make(
            f"{SYN}/initial-composite",
            SYN,
            "-r a composite holding an affine transform and a field",
            "-r",
            "{gen-syn:composite-3d.h5}",
            *syn,
        ),
        make(
            f"{SYN}/initial-composite-inverse",
            SYN,
            "-r [composite,1]: its field has no inverse",
            "-r",
            "[{gen-syn:composite-3d.h5},1]",
            *syn,
            out=PRE,
            expect="error",
        ),
        make(
            f"{SYN}/fixed-field",
            SYN,
            "-q a displacement field image",
            "-q",
            "{gen-syn:warp-3d.nii.gz}",
            *syn,
        ),
        make(
            f"{SYN}/initial-field-2d",
            SYN,
            "-r a 2D displacement field image",
            "-r",
            "{gen-syn:warp-2d.nii.gz}",
            *_stage("SyN[0.3,2,0]", [f"CC[{F2},{M2},1,2]"]),
            d="2",
        ),
        make(
            f"{SYN}/initial-scalar",
            SYN,
            "-r a scalar image: ITK reads it as a field of equal components (ANTS-73)",
            "-r",
            "{gen:reg-mask}",
            *syn,
            out=PRE,
        ),
        make(
            f"{SYN}/initial-scalar-2d",
            SYN,
            "-r a 2D scalar image: ITK reads it as complex pixels, both parts equal (ANTS-73)",
            "-r",
            "{gen:reg-mask-2d}",
            *_stage("SyN[0.3,2,0]", [f"CC[{F2},{M2},1,2]"]),
            d="2",
            out=PRE,
        ),
        make(
            f"{SYN}/initial-field-2d-in-3d",
            SYN,
            "-r a 2D field in 3D: one slice, each vector the product of its components (ANTS-73)",
            "-r",
            "{gen-syn:warp-2d.nii.gz}",
            *syn,
            out=PRE,
        ),
        make(
            f"{SYN}/igdm",
            SYN,
            "IGDM first in a SyN stage: not implemented in ANTs",
            "-x",
            K,
            *_stage(
                "SyN[0.2,3,0]",
                [f"IGDM[{F},{M},1,{K},{K}]"],
                c="[3,1e-6,3]",
                f="1",
                s="0",
            ),
            out=PRE,
            expect="error",
        ),
        make(
            f"{SYN}/bad-variance",
            SYN,
            "SyN[0.2,3x]: a variance that does not convert",
            *_stage("SyN[0.2,3x,0]", [cc]),
            expect="error",
        ),
        make(
            f"{SYN}/bsplinesyn",
            SYN,
            "BSplineSyN: A5 part 2",
            *_stage("BSplineSyN[0.1,26,0,3]", [cc]),
            out=PRE,
            expect="divergence",
            reason="B-spline SyN is milestone A5 part 2; larmorx refuses with a message",
        ),
        make(
            f"{SYN}/exponential",
            SYN,
            "Exponential: A5 part 3",
            *_stage("Exponential[0.1,3,0]", [cc]),
            out=PRE,
            expect="divergence",
            reason="the exponential and velocity-field transforms are milestone A5 part 3; larmorx "
            "refuses with a message",
        ),
    ]
    return cases + _point_set_cases(make)


# The point-set inputs of the suite's point-set cases (:mod:`ants_point_sets`).
PSF, PSM = "{gen:ps-labels-fixed}", "{gen:ps-labels-moving}"
PSK, PSKM = "{gen:ps-mask}", "{gen:ps-mask-moving}"
PSI, PSIM = "{gen:ps-intensity-fixed}", "{gen:ps-intensity-moving}"
F2D, M2D, K2D = "{gen:ps-labels-fixed-2d}", "{gen:ps-labels-moving-2d}", "{gen:ps-mask-2d}"


def _point_set_cases(make) -> list:
    """SyN with the point-set metrics: in tangent space, each point's derivative at its
    nearest voxel (ITK-32 and ITK-33 reproduced)."""
    PS = "syn-point-sets"
    icp = f"ICP[{PSF},{PSM},1]"
    igdm = f"IGDM[{PSI},{PSIM},1,{PSK},{PSKM}]"

    def ps(name: str, description: str, *metrics: str, t: str = "SyN[0.3,2,0]", **kw):
        extra = kw.pop("extra", ())
        mask = kw.pop("mask", PSK)
        stage = _stage(t, list(metrics), c=kw.pop("c", "[4x3,1e-6,3]"), f="2x1", s="1x0vox")
        head = ("-x", mask) if mask else ()
        return make(f"{PS}/{name}", PS, description, *head, *extra, *stage, out=PRE, **kw)

    return [
        ps("icp", "ICP alone: the fixed mask's grid as the virtual domain", icp, band=True),
        ps("pse", "PSE (sigma 1, 20 neighbours)", f"PSE[{PSF},{PSM},1,1,0,1,20]"),
        ps("jhct", "JHCT, anisotropic (sigma 1, alpha 1.1)", f"JHCT[{PSF},{PSM},1,1,0,1,20,1.1,1]"),
        ps("icp-float", "ICP, --float 1", icp, extra=("--float", "1")),
        ps(
            "icp-moving-initial",
            "ICP after -r (a matrix)",
            icp,
            extra=("-r", "{gen-transform:initial.mat}"),
        ),
        ps(
            "icp-fixed-initial",
            "ICP after -q (a matrix)",
            icp,
            extra=("-q", "{gen-transform:initial.mat}"),
        ),
        ps(
            "icp-not-bijective",
            "a fixed label the moving set lacks: ITK throws at the first such point (ITK-22)",
            f"ICP[{PSF},{{gen:ps-labels-moving-partial}},1]",
        ),
        ps(
            "icp-moving-extra-label",
            "a moving label the fixed set lacks: ITK throws in the moving-to-middle update (ITK-22)",
            f"ICP[{{gen:ps-labels-moving-partial}},{PSM},1]",
        ),
        ps(
            "cc-and-icp",
            "CC and ICP: ITK gives ICP identity transforms (ITK-33)",
            f"CC[{PSI},{PSIM},1,2]",
            f"ICP[{PSF},{PSM},0.5]",
        ),
        ps(
            "icp-and-cc",
            "ICP first, then CC: ICP's virtual domain (ITK-33)",
            f"ICP[{PSF},{PSM},0.5]",
            f"CC[{PSI},{PSIM},1,2]",
        ),
        ps(
            "cc-and-igdm",
            "CC and IGDM: SyN holds labelled point sets only, IGDM gets no input (ANTS-61)",
            f"CC[{PSI},{PSIM},1,2]",
            igdm,
        ),
        ps(
            "cc-igdm-icp",
            "CC, IGDM and ICP: IGDM without point sets before a metric with them (ANTS-61)",
            f"CC[{PSI},{PSIM},1,2]",
            igdm,
            icp,
            expect="crash",
        ),
        ps("icp-2d", "ICP in 2D", f"ICP[{F2D},{M2D},1]", mask=K2D, d="2"),
        ps(
            "cc-regular-and-icp",
            "CC sampled (Regular) and ICP: ANTs gives ICP's sample points to a null image metric",
            f"CC[{PSI},{PSIM},1,2,Regular,0.5]",
            icp,
            expect="crash",
        ),
        ps(
            "icp-field-initial",
            "ICP after -r a displacement field without an inverse: ITK maps the moving points with a null transform",
            icp,
            extra=("-r", "{gen-syn:warp-3d.nii.gz}"),
            expect="crash",
        ),
        ps(
            "icp-no-mask",
            "ICP alone without a fixed mask: no virtual domain",
            icp,
            mask=None,
            expect="crash",
        ),
    ]
