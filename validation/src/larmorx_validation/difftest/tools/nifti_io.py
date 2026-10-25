# SPDX-License-Identifier: Apache-2.0
"""NIfTI I/O: random files, every header field written explicitly, read by larmorx and by
nibabel and written back (the parity runner, ``larmorx_validation.parity.nifti_io``).

The files vary in form (``.nii``, ``.nii.gz``, ``.hdr``/``.img``, NIfTI-2, big-endian, a gap
before the data, header extensions), in shape (1 to 7 axes, axes of one voxel), in type (every
integer and float type NIfTI stores), in scaling (``scl_slope`` and ``scl_inter``, a NaN,
infinite, zero or negative slope), in geometry (the qform and sform codes and matrices, an
sform not equal to the qform, every orientation, ``qfac``) and in the other fields
(``xyzt_units``, slice timing, ``intent_code``, ``cal_min``/``cal_max``, ``toffset``,
``descrip``). Data bit for bit in every read mode, header fields, affines.
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool
from larmorx_validation.difftest.tools import _ants

DTYPES = (
    "uint8",
    "int8",
    "int16",
    "uint16",
    "int32",
    "uint32",
    "int64",
    "uint64",
    "float32",
    "float64",
)


def draw(d: gen.Draw) -> dict:
    ndim = d.choice("ndim", (2, 3, 4, 5, 6, 7), (1, 4, 2, 0.5, 0.25, 0.25))
    if ndim <= 4:
        shape = gen.draw_shape(d, max(ndim, 3), lo=1, hi=24, volumes=(1, 6), thin=0.25)
        if ndim == 2:
            shape = shape[:2]
    else:
        shape = [d.integer(f"n{k}", 1, 6) for k in range(ndim)]
    spec = gen.draw_image(
        d,
        min(ndim, 4),
        shape=shape,
        contents=("noise", "head", "ramp", "constant", "zeros", "labels", "sparse"),
        dtypes=DTYPES,
        specials=0.15,
        extreme=0.25,
        scaling=0.35,
        suffix=d.choice("form", (".nii", ".nii.gz", ".hdr", ".nii"), (3, 3, 1, 1)),
    )
    header: dict = {}
    if d.chance("nifti2", 0.15) and spec["suffix"] != ".hdr":
        header["nifti2"] = True
    if d.chance("big endian", 0.15):
        header["big_endian"] = True
    if d.chance("gap before data", 0.1) and spec["suffix"] != ".hdr":
        header["vox_pad"] = d.choice("gap", (4, 16, 100))
    if d.chance("extensions", 0.1):
        header["extensions"] = [
            (
                d.choice("ecode", (0, 6, 4, 2)),
                "difftest extension " * d.integer("extension length", 1, 5),
            )
        ]
    if d.chance("slope special", 0.08):
        header["slope_special"] = d.choice("slope value", ("nan", "inf", "0", "-0"))
    for name, p, values in (
        ("cal_min", 0.1, (0.0, -1.0, 10.0)),
        ("cal_max", 0.1, (100.0, 0.0, 1e6)),
        ("toffset", 0.1, (0.0, 2.5, -1.0)),
        ("intent_code", 0.1, (0, 2, 3, 1007, 2001)),
    ):
        if d.chance(name, p):
            header[name] = d.choice(f"{name} value", values)
    if d.chance("descrip", 0.2):
        header["descrip"] = d.choice(
            "descrip text", ("", "x" * 79, "FSL5.0", "ünicode? no: ascii", "a b")
        )
        header["descrip"] = header["descrip"].encode("ascii", "replace").decode("ascii")
    spec["header"] = header
    options = [
        f"form {spec['suffix']}",
        f"dtype {spec['dtype']}",
        f"{len(shape)} axes",
        *(["NIfTI-2"] if header.get("nifti2") else []),
        *(["big-endian"] if header.get("big_endian") else []),
        *(["scaled"] if spec.get("scaling") else []),
        *(["extensions"] if header.get("extensions") else []),
        f"qform {spec['geometry']['qform_code']} sform {spec['geometry']['sform_code']}",
    ]
    return {"groups": [], "inputs": {"file": spec}, "options": options}


def case(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    **kw,
):
    from larmorx_validation.difftest import inputs
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or "random file: " + ", ".join(recipe["options"]),
        "dt:" + inputs.key(prefix, "file"),
    )


def run(impl: str):
    from larmorx_validation.parity.nifti_io import run_case

    return run_case


TOOL = Tool(
    name="nifti-io",
    title="NIfTI I/O",
    suite="nifti-io",
    implementations=("port",),
    draw=draw,
    case=case,
    run=run,
    exact_checks=(),
    timeout=60,
    weight=0.6,
    family="python",
    payload_fields=(),
)
