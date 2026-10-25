# SPDX-License-Identifier: Apache-2.0
"""ITK transform files: random transform lists in text, MATLAB and HDF5 files, read by larmorx
and by ITK (ANTs writes back what it read, in double and with ``--float 1``), h5py and
nitransforms (the parity runner's ``read`` kind, ``larmorx_validation.parity.itk_hdf5``).

Text files spell numbers in many ways (exponents, many digits, signed zeros, integers) and hold
one to four transforms of every linear type ITK reads (affine, matrix-offset, Euler, versor,
quaternion rigid, similarity, scale-versor, scale-skew-versor, translation, scale, identity; in
double and float; 2D and 3D) and composites; HDF5 files are written by larmorx's writer (with
and without compression) and hold the same and displacement fields.
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool
from larmorx_validation.difftest.tools import _ants
from larmorx_validation.difftest.tools.ants_apply_transforms import _linear


def _spelling(d: gen.Draw, v: float, k: str) -> str:
    form = d.choice(f"spelling {k}", ("repr", "e", "short", "int"), (4, 1, 1, 0.5))
    if form == "e":
        return f"{v:.17e}"
    if form == "short":
        return f"{v:.6g}"
    if form == "int" and float(v).is_integer():
        return str(int(v))
    return repr(float(v))


def _extra(d: gen.Draw, k: int) -> dict | None:
    kind = d.choice(
        f"other type{k}",
        (
            "QuaternionRigidTransform",
            "ScaleVersor3DTransform",
            "ScaleSkewVersor3DTransform",
            "ScaleTransform",
            "IdentityTransform",
            "Euler2DTransform",
            "AffineTransform_2",
        ),
    )
    q = (
        d.integer(f"q{k}a", 6, 30),
        d.integer(f"q{k}b", -2, 2),
        d.integer(f"q{k}c", -2, 2),
        d.integer(f"q{k}d", -2, 2),
    )
    a, b, c, dd = gen._unit_quaternion(q)
    shift = [d.step(f"x{k}{j}", -5.0, 5.0, 0.25) for j in range(3)]
    centre = [d.step(f"c{k}{j}", -5.0, 5.0, 0.5) for j in range(3)]
    if kind == "QuaternionRigidTransform":
        return {
            "type": "QuaternionRigidTransform_double_3_3",
            "parameters": [b, c, dd, a, *shift],
            "fixed": centre,
        }
    if kind == "ScaleVersor3DTransform":
        return {
            "type": "ScaleVersor3DTransform_double_3_3",
            "parameters": [b, c, dd, *shift, 1.1, 0.9, 1.0],
            "fixed": centre,
        }
    if kind == "ScaleSkewVersor3DTransform":
        return {
            "type": "ScaleSkewVersor3DTransform_double_3_3",
            "parameters": [b, c, dd, *shift, 1.1, 0.9, 1.0, 0.01, 0.0, -0.02, 0.0, 0.03, 0.0],
            "fixed": centre,
        }
    if kind == "ScaleTransform":
        return {"type": "ScaleTransform_double_3_3", "parameters": [1.2, 0.8, 1.0], "fixed": centre}
    if kind == "IdentityTransform":
        return {"type": "IdentityTransform_double_3_3", "parameters": [], "fixed": []}
    if kind == "Euler2DTransform":
        return {
            "type": "Euler2DTransform_double_2_2",
            "parameters": [d.step(f"angle{k}", -0.3, 0.3, 0.01), *shift[:2]],
            "fixed": centre[:2],
        }
    return {
        "type": "AffineTransform_double_2_2",
        "parameters": [1.0, 0.05, -0.02, 0.98, *shift[:2]],
        "fixed": centre[:2],
    }


def draw(d: gen.Draw) -> dict:
    fmt = d.choice("format", (".txt", ".mat", ".h5"), (3, 1, 3))
    n = 1 if fmt == ".mat" else d.integer("transforms", 1, 4)
    transforms = []
    for k in range(n):
        t = _linear(d, k) if d.chance(f"common type{k}", 0.75) else _extra(d, k)
        if d.chance(f"float{k}", 0.2) and fmt != ".mat":
            t = {**t, "type": t["type"].replace("_double_", "_float_")}
        transforms.append(t)
    options = [
        f"{fmt} file",
        f"{n} transforms",
        *sorted({t["type"].split("_")[0] for t in transforms}),
    ]
    if fmt == ".txt":
        lines = ["#Insight Transform File V1.0"]
        composite = n > 1 and d.chance("composite", 0.4)
        if composite:
            lines += [
                "#Transform 0",
                "Transform: CompositeTransform_double_3",
                "Parameters: ",
                "FixedParameters: ",
            ]
            options.append("composite")
        for i, t in enumerate(transforms):
            j = i + (1 if composite else 0)
            lines += [
                f"#Transform {j}",
                f"Transform: {t['type']}",
                "Parameters: "
                + " ".join(_spelling(d, v, f"p{i}{m}") for m, v in enumerate(t["parameters"])),
                "FixedParameters: "
                + " ".join(_spelling(d, v, f"f{i}{m}") for m, v in enumerate(t["fixed"])),
            ]
        spec = {"kind": "text", "text": "\n".join(lines) + "\n", "suffix": ".txt"}
    elif fmt == ".mat":
        t = transforms[0]
        if not t["type"].startswith(
            ("AffineTransform_double_3_3", "MatrixOffsetTransformBase_double_3_3")
        ):
            t = {
                "type": "AffineTransform_double_3_3",
                "parameters": [1, 0, 0, 0, 1, 0, 0, 0, 1, 0.5, -0.25, 1],
                "fixed": [0.0, 0.0, 0.0],
            }
        spec = {
            "kind": "itk-mat",
            "transform": {**t, "type": t["type"].replace("_float_", "_double_")},
            "suffix": ".mat",
        }
    else:
        if d.chance("displacement field", 0.25):
            shape = [d.integer(f"field n{j}", 2, 8) for j in range(3)]
            nvox = shape[0] * shape[1] * shape[2]
            rng_values = [((i * 7919) % 101 - 50) / 25.0 for i in range(nvox * 3)]
            transforms.append(
                {
                    "type": "DisplacementFieldTransform_double_3_3",
                    "parameters": rng_values,
                    "fixed": [*shape, -5.0, -4.0, -3.0, 2.0, 2.0, 2.0, 1, 0, 0, 0, 1, 0, 0, 0, 1],
                }
            )
            options.append("displacement field")
        if len(transforms) > 1 and d.chance("composite h5", 0.5):
            transforms.insert(
                0, {"type": "CompositeTransform_double_3", "parameters": [], "fixed": []}
            )
            options.append("composite")
        spec = {"kind": "itk-h5", "transforms": transforms, "suffix": ".h5"}
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
    from larmorx_validation.parity.itk_hdf5 import Spec

    prefix = _ants.prefix_of(recipe, case_prefix)
    path = inputs.path(inputs.key(prefix, "file"))
    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or "random transform file: " + ", ".join(recipe["options"]),
        Spec("read", str(path), **kw),
    )


def run(impl: str):
    from larmorx_validation.parity.itk_hdf5 import run_case

    return run_case


TOOL = Tool(
    name="itk-transforms",
    title="ITK transform files",
    suite="itk-hdf5",
    implementations=("port",),
    draw=draw,
    case=case,
    run=run,
    exact_checks=(),
    timeout=120,
    weight=0.8,
    payload_fields=("tier",),
    unavailable=_ants.ants_unavailable,
)
