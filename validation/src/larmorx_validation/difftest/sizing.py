# SPDX-License-Identifier: Apache-2.0
"""A case's peak memory and time, predicted before it runs, from the original program's own
sizing law and unit costs measured on smaller variants of real cases
(``validation/investigations/difftest/n4_memory.py``, ``scaled_memory.py``; the findings in
``docs/findings/ants-n4.md``, "Memory", and ``afni-unifize.md``, AFNI-75):

- ``ants-n4``: N4BiasFieldCorrection pads the image so that each axis spans a whole number of
  spline distances (``ceil((n - 1) * spacing / distance)`` spans, padding
  ``(spans * distance - domain) / spacing + 0.5`` voxels) and fits a lattice of
  ``spans * 2**level + order`` control points per axis at each fitting level; memory is the
  largest level's lattice times bytes per control point plus the padded image's voxels times
  bytes per voxel, time the iterations times the control points times seconds per point and
  iteration;
- ``ants-resample-image-by-spacing`` and ``ants-resample-image``: the output grid,
  ``int(n * spacing_in / spacing_out + addvox)`` voxels per axis (by spacing) or the size given;
  memory the output voxels times bytes per voxel;
- ``afni-unifize``: ``-EPI`` sets the radius of its ball of neighbours to ``18.3 / cbrt(|dx dy dz|)``
  voxels (at least 4.14; voxel sizes in mm), else ``-Urad`` or ``-rbt`` (18.3), halved (plus
  0.001) for the half-size copy of more than a million voxels; the ball has ``4/3 pi r^3``
  points, built whole first: 10 bytes each in AFNI, 24 plus 4 per worker in the GPL replica.

Random testing predicts every case of these tools before running it (``runner.run_case``): a
case predicted over the case's memory limit is not compared but checked as a guard case
(larmorx's default must stop at once with its clear error: ANTS-162, AFNI-75), its prediction
recorded and the case queued for the heavy box. ``ants_gib`` is the original program's
prediction (ANTs or AFNI), ``larmorx_gib`` larmorx's (in the mode compared, ``reproduce``).
"""

from __future__ import annotations

import math
import re
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np

GIB = float(1 << 30)

#: Unit costs (bytes and seconds), each measured on smaller variants of a real case.
N4_POINT_BYTES = {3: 17.0, 2: 18.7}
N4_POINT_BYTES_LX = {3: 12.0, 2: 16.1}
N4_VOXEL_BYTES, N4_VOXEL_BYTES_LX = 25.0, 21.0
N4_POINT_SECONDS = {3: 3.3e-7, 2: 2.3e-8}
N4_POINT_SECONDS_LX = {3: 2.6e-7, 2: 1.9e-8}
N4_VOXEL_SECONDS, N4_VOXEL_SECONDS_LX = 7.3e-8, 2.1e-8
GRID_BYTES, GRID_BYTES_LX = 7.3, 7.3
BALL_BYTES, BALL_BYTES_LX = 10.0, 28.0

SPATIAL = {"meter": 1000.0, "mm": 1.0, "micron": 0.001, "unknown": 1.0}
TEMPORAL = {"sec": 1.0, "msec": 0.001, "usec": 1e-6, "unknown": 1.0}

TOOLS = ("ants-n4", "ants-resample-image", "ants-resample-image-by-spacing", "afni-unifize")


def itk_spacing(path: Path) -> tuple[list[int], list[float]]:
    """The size and spacing ITK's NIfTI reader gives (metres and microns to mm, the time axis's
    msec and usec to s); AFNI reads a header's units the same way."""
    img = nib.load(str(path))
    xyz, t = img.header.get_xyzt_units()
    zooms = [float(z) for z in img.header.get_zooms()]
    spacing = [z * SPATIAL.get(xyz, 1.0) for z in zooms[:3]]
    spacing += [z * TEMPORAL.get(t, 1.0) for z in zooms[3:]]
    return list(img.shape), spacing


def _f32(x: float) -> np.float32:
    return np.float32(x)


def n4_size(words: list[str], size: list[int], spacing: list[float]) -> dict:
    dim = int(words[words.index("-d") + 1]) if "-d" in words else len(size)
    size, spacing = size[:dim], spacing[:dim]
    order, distance, mesh = 3, None, [1] * dim
    if "-b" in words:
        b = words[words.index("-b") + 1].strip("[] ")
        first = b.split(",")[0].strip()
        if "," in b:
            order = int(float(b.split(",")[1]))
        if "x" in first:
            mesh = [int(float(v)) for v in first.split("x")]
        elif re.fullmatch(r"[-+\d.eE]+", first):
            distance = float(first)
    iterations = [50, 50, 50, 50]
    if "-c" in words:
        c = words[words.index("-c") + 1].strip("[] ").split(",")[0].strip()
        iterations = [int(v) for v in c.split("x") if v]
    padded = list(size)
    if distance is not None and distance > 0:
        spans = []
        for k in range(dim):
            domain = _f32(size[k] - 1) * _f32(spacing[k])
            sp = math.ceil(float(_f32(domain / _f32(distance))))
            extra = int((sp * distance - float(domain)) / spacing[k] + 0.5) if spacing[k] else 0
            padded[k] = size[k] + extra
            spans.append(sp)
    else:
        spans = mesh
    lattice = [[sp * (1 << lv) + order for sp in spans] for lv in range(len(iterations))]
    return {
        "dimension": dim,
        "iterations": iterations,
        "padded_voxels": math.prod(padded),
        "lattice_last": lattice[-1],
        "points": [math.prod(c) for c in lattice],
    }


def by_spacing_size(words: list[str], size: list[int], spacing: list[float]) -> dict:
    dim = int(words[0])
    out_spacing = [float(w) for w in words[3 : 3 + dim]]
    addvox = int(words[3 + dim + 1]) if len(words) > 3 + dim + 1 else 0
    out = [int(size[k] * spacing[k] / out_spacing[k] + addvox) for k in range(dim)]
    return {"output": out, "output_voxels": math.prod(max(0, v) for v in out)}


def resample_size(words: list[str], size: list[int], spacing: list[float]) -> dict:
    dim = int(words[0])
    values = [float(v) for v in words[3].split("x")]
    values = values * dim if len(values) == 1 else values
    by_size = len(words) > 4 and words[4].strip() not in ("", "0")
    if by_size:
        out = [int(v) for v in values[:dim]]
    else:
        out = [int(spacing[k] * size[k] / values[k] + 0.5) for k in range(dim)]
    return {"output": out, "output_voxels": math.prod(max(0, v) for v in out)}


def unifize_ball(words: list[str], size: list[int], spacing: list[float]) -> dict:
    nvox = math.prod(size[:3])
    radius = 18.3
    if "-Urad" in words:
        radius = float(words[words.index("-Urad") + 1])
    for opt in ("-rbt", "-param"):
        if opt in words:
            radius = float(words[words.index(opt) + 1])
    epi = "-EPI" in words and len(size) > 3 and size[3] > 1
    if epi:
        uu = float(np.cbrt(np.float32(abs(spacing[0] * spacing[1] * spacing[2]))))
        radius = max(18.3 / uu, 4.14) if uu > 0 else float("inf")
    elif nvox > 1_000_000 and "-noduplo" not in words and "-nodouble" not in words:
        radius = 0.5 * radius + 0.001
    points = 4.0 / 3.0 * math.pi * radius**3 + 1.0
    return {"epi": epi, "radius": radius, "points": points, "voxels": nvox}


def predict_from(tool_name: str, words: list[str], size: list[int], spacing: list[float]) -> dict:
    """The prediction for a case of ``tool_name`` with arguments ``words`` on an input of
    ``size`` voxels and ITK ``spacing``."""
    if tool_name == "ants-n4":
        s = n4_size(words, size, spacing)
        d = 2 if s["dimension"] == 2 else 3
        last = max(s["points"])
        work = sum(i * p for i, p in zip(s["iterations"], s["points"], strict=False))
        pad_work = sum(s["iterations"]) * s["padded_voxels"]
        return {
            "law": "N4 lattice and padding",
            "sizing": s,
            "ants_gib": (last * N4_POINT_BYTES[d] + s["padded_voxels"] * N4_VOXEL_BYTES) / GIB,
            "larmorx_gib": (last * N4_POINT_BYTES_LX[d] + s["padded_voxels"] * N4_VOXEL_BYTES_LX)
            / GIB,
            "ants_hours": (work * N4_POINT_SECONDS[d] + pad_work * N4_VOXEL_SECONDS) / 3600,
            "larmorx_hours": (work * N4_POINT_SECONDS_LX[d] + pad_work * N4_VOXEL_SECONDS_LX)
            / 3600,
        }
    if tool_name in ("ants-resample-image-by-spacing", "ants-resample-image"):
        law = by_spacing_size if tool_name.endswith("spacing") else resample_size
        s = law(words, size, spacing)
        return {
            "law": "output grid",
            "sizing": s,
            "ants_gib": s["output_voxels"] * GRID_BYTES / GIB,
            "larmorx_gib": s["output_voxels"] * GRID_BYTES_LX / GIB,
        }
    if tool_name == "afni-unifize":
        s = unifize_ball(words, size, spacing)
        return {
            "law": "3dUnifize's ball of neighbours",
            "sizing": s,
            "ants_gib": s["points"] * BALL_BYTES / GIB,
            "larmorx_gib": s["points"] * BALL_BYTES_LX / GIB,
        }
    raise KeyError(tool_name)


def predict(tool_name: str, recipe: dict) -> dict | None:
    """The prediction for ``recipe`` of ``tool_name``, or ``None`` for a tool without a sizing
    law here (its first image input is written to a temporary file to read its header as ITK
    and AFNI read it)."""
    if tool_name not in TOOLS:
        return None
    from larmorx_validation.difftest import core, inputs

    spec = recipe["inputs"].get("a") or recipe["inputs"].get("input")
    if spec is None or spec.get("kind", "nifti") != "nifti":
        return None
    with tempfile.TemporaryDirectory(prefix="lx-dt-size-") as t:
        p = Path(t) / ("a" + inputs.suffix_of(spec))
        inputs.materialize(spec, p)
        size, spacing = itk_spacing(p)
    try:
        return predict_from(tool_name, core.arguments(recipe), size, spacing)
    except (ValueError, IndexError, ZeroDivisionError, OverflowError):
        return None


__all__ = [
    "GIB",
    "TOOLS",
    "by_spacing_size",
    "itk_spacing",
    "n4_size",
    "predict",
    "predict_from",
    "resample_size",
    "unifize_ball",
]
