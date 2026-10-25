# SPDX-License-Identifier: Apache-2.0
"""The memory and time a night's heavy cases (``heavy.jsonl``: a run exceeded the per-case
limit) will need, before ``difftest --heavy`` runs them: each case's size by the original's own
sizing law, times the unit costs measured on smaller variants of the same case
(``n4_memory.py``, ``resample_memory.py``; the units and their sources are in the arguments).

- ``ants-n4``: N4BiasFieldCorrection pads the image so that each axis spans a whole number of
  spline distances (``ceil((n - 1) * spacing / distance)`` spans, padding
  ``(spans * distance - domain) / spacing + 0.5`` voxels) and fits a lattice of
  ``spans * 2**level + order`` control points per axis at each fitting level. Memory: the
  largest level's lattice times bytes per control point, plus the padded image's voxels times
  bytes per voxel; time: iterations times control points times seconds per point and
  iteration, summed over the levels.
- ``ants-resample-image-by-spacing``: the output has ``int(n * spacing_in / spacing_out +
  addvox)`` voxels per axis; memory: output voxels times bytes per voxel.

    python -P validation/investigations/difftest/heavy_predict.py oracles/difftest/<date> \\
        [--n4-point-bytes 16] [--n4-voxel-bytes 24] [--n4-point-seconds 3.7e-7] ...
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
import tempfile
from pathlib import Path

import numpy as np
from larmorx_validation.difftest import core, inputs, limits, report, tools

sys.path.insert(0, str(Path(__file__).resolve().parent))
from n4_memory import itk_spacing

GIB = limits.GIB


def _f32(x: float) -> np.float32:
    return np.float32(x)


def n4_size(words: list[str], size: list[int], spacing: list[float]) -> dict:
    dim = int(words[words.index("-d") + 1])
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
    shrink = int(words[words.index("-s") + 1]) if "-s" in words else 4
    padded = list(size)
    if distance is not None:
        spans = []
        for d in range(dim):
            domain = _f32(size[d] - 1) * _f32(spacing[d])
            sp = math.ceil(float(_f32(domain / _f32(distance))))
            extra = int((sp * distance - float(domain)) / spacing[d] + 0.5)
            padded[d] = size[d] + extra
            spans.append(sp)
    else:
        spans = mesh
    lattice = [[sp * (1 << lv) + order for sp in spans] for lv in range(len(iterations))]
    points = [math.prod(c) for c in lattice]
    return {
        "dimension": dim,
        "order": order,
        "distance": distance,
        "iterations": iterations,
        "shrink": shrink,
        "padded": padded,
        "padded_voxels": math.prod(padded),
        "lattice_last": lattice[-1],
        "points": points,
    }


def by_spacing_size(words: list[str], size: list[int], spacing: list[float]) -> dict:
    dim = int(words[0])
    out_spacing = [float(w) for w in words[3 : 3 + dim]]
    addvox = int(words[3 + dim + 1]) if len(words) > 3 + dim + 1 else 0
    out = [int(size[d] * spacing[d] / out_spacing[d] + addvox) for d in range(dim)]
    return {"output": out, "output_voxels": math.prod(out)}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("night", type=Path)
    ap.add_argument("--n4-point-bytes", type=float, default=16.0, help="ANTs, per control point")
    ap.add_argument("--n4-point-bytes-lx", type=float, default=11.4)
    ap.add_argument("--n4-voxel-bytes", type=float, default=24.0, help="per padded voxel")
    ap.add_argument("--n4-point-seconds", type=float, default=3.7e-7)
    ap.add_argument("--n4-point-seconds-lx", type=float, default=4.0e-7)
    ap.add_argument("--by-spacing-voxel-bytes", type=float, default=8.0)
    ap.add_argument("--by-spacing-voxel-bytes-lx", type=float, default=12.0)
    ap.add_argument("--box-gib", type=float, default=limits.HEAVY_GIB + limits.HEAVY_SWAP_GIB)
    args = ap.parse_args()
    rows = []
    with tempfile.TemporaryDirectory(prefix="lx-heavy-predict-") as t:
        for h in report.heavy(args.night):
            tool = tools.get(h["tool"])
            recipe = core.make_recipe(tool, h["seed"], h["n"])
            spec = recipe["inputs"].get("a")
            row = {
                "case": f"{h['tool']}:{h['seed']}:{h['n']}",
                "args": " ".join(core.arguments(recipe)),
            }
            if spec is None:
                row["prediction"] = "no sizing law here"
                rows.append(row)
                continue
            p = Path(t) / f"{h['n']}-a{inputs.suffix_of(spec)}"
            inputs.materialize(spec, p)
            size, spacing = itk_spacing(p)
            row.update(size=size, spacing_itk=spacing)
            words = core.arguments(recipe)
            if h["tool"] == "ants-n4":
                s = n4_size(words, size, spacing)
                ants = (
                    max(s["points"]) * args.n4_point_bytes
                    + s["padded_voxels"] * args.n4_voxel_bytes
                )
                lx = (
                    max(s["points"]) * args.n4_point_bytes_lx
                    + s["padded_voxels"] * args.n4_voxel_bytes
                )
                work = sum(i * pt for i, pt in zip(s["iterations"], s["points"], strict=True))
                row.update(
                    sizing=s,
                    ants_gib=ants / GIB,
                    larmorx_gib=lx / GIB,
                    ants_hours=work * args.n4_point_seconds / 3600,
                    larmorx_hours=work * args.n4_point_seconds_lx / 3600,
                )
            elif h["tool"] == "ants-resample-image-by-spacing":
                s = by_spacing_size(words, size, spacing)
                row.update(
                    sizing=s,
                    ants_gib=s["output_voxels"] * args.by_spacing_voxel_bytes / GIB,
                    larmorx_gib=s["output_voxels"] * args.by_spacing_voxel_bytes_lx / GIB,
                )
            else:
                row["prediction"] = "no sizing law here: run a smaller variant"
            if "ants_gib" in row:
                row["fits"] = max(row["ants_gib"], row["larmorx_gib"]) < args.box_gib
            rows.append(row)
    (args.night / "heavy-prediction.json").write_text(json.dumps(rows, indent=1), encoding="utf-8")
    for r in rows:
        if "ants_gib" not in r:
            print(f"{r['case']}: {r.get('prediction')}")
            continue
        hours = (
            f", time ~{r['ants_hours']:.2g} h (ANTs), ~{r['larmorx_hours']:.2g} h (larmorx)"
            if "ants_hours" in r
            else ""
        )
        print(
            f"{r['case']}: ANTs ~{r['ants_gib']:.3g} GiB, larmorx ~{r['larmorx_gib']:.3g} GiB"
            f"{hours}; {'fits' if r['fits'] else 'does NOT fit'} the heavy box "
            f"({args.box_gib:g} GiB with swap); {r['args']}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
