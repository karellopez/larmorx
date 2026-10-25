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
- ``afni-unifize`` (AFNI-75): ``-EPI`` sets the ball's radius to ``18.3 / cbrt(|dx dy dz|)``
  voxels (at least 4.14; the voxel sizes in mm, as AFNI and ITK read a header's units), else
  ``-Urad`` or ``-rbt``'s first value (18.3 by default), halved (plus 0.001) for the half-size
  copy of more than a million voxels; the ball has ``4/3 pi r^3`` points, built whole first: 10
  bytes each in AFNI (three shorts and a float), 24 plus 4 per worker in the GPL replica.

``ants_gib`` is the original program's prediction (ANTs or AFNI), ``larmorx_gib`` larmorx's.

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


def unifize_ball(words: list[str], size: list[int], spacing: list[float]) -> dict:
    nvox = math.prod(size[:3])
    radius = 18.3
    if "-Urad" in words:
        radius = float(words[words.index("-Urad") + 1])
    for opt in ("-rbt", "-param"):
        if opt in words:
            radius = float(words[words.index(opt) + 1])
    epi = "-EPI" in words
    if epi:
        uu = float(np.cbrt(np.float32(abs(spacing[0] * spacing[1] * spacing[2]))))
        radius = max(18.3 / uu, 4.14)
    elif nvox > 1_000_000 and "-noduplo" not in words and "-nodouble" not in words:
        radius = 0.5 * radius + 0.001
    points = 4.0 / 3.0 * math.pi * radius**3 + 1.0
    return {"epi": epi, "radius": radius, "points": points, "voxels": nvox}


def by_spacing_size(words: list[str], size: list[int], spacing: list[float]) -> dict:
    dim = int(words[0])
    out_spacing = [float(w) for w in words[3 : 3 + dim]]
    addvox = int(words[3 + dim + 1]) if len(words) > 3 + dim + 1 else 0
    out = [int(size[d] * spacing[d] / out_spacing[d] + addvox) for d in range(dim)]
    return {"output": out, "output_voxels": math.prod(out)}


def resample_size(words: list[str], size: list[int], spacing: list[float]) -> dict:
    """ResampleImage's output grid: ``MxNxO`` voxels (size mode, ``1``) or
    ``int(spacing_in * n / spacing_out + 0.5)`` per axis (spacing mode, the default)."""
    dim = int(words[0])
    values = [float(v) for v in words[3].split("x")]
    values = values * dim if len(values) == 1 else values
    by_size = len(words) > 4 and words[4].strip() not in ("", "0")
    if by_size:
        out = [int(v) for v in values[:dim]]
    else:
        out = [int(spacing[d] * size[d] / values[d] + 0.5) for d in range(dim)]
    return {"output": out, "output_voxels": math.prod(out)}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("night", type=Path)
    # The unit costs measured on smaller variants (docs/findings/ants-n4.md, "Memory"):
    ap.add_argument("--n4-point-bytes", type=float, default=17.0, help="ANTs, per control point")
    ap.add_argument("--n4-point-bytes-lx", type=float, default=12.0)
    ap.add_argument("--n4-voxel-bytes", type=float, default=25.0, help="ANTs, per padded voxel")
    ap.add_argument("--n4-voxel-bytes-lx", type=float, default=21.0)
    ap.add_argument("--n4-point-seconds", type=float, default=3.3e-7)
    ap.add_argument("--n4-point-seconds-lx", type=float, default=2.6e-7)
    ap.add_argument("--n4-voxel-seconds", type=float, default=7.3e-8)
    ap.add_argument("--n4-voxel-seconds-lx", type=float, default=2.1e-8)
    # In 2D (n4_memory.py on ants-n4:20261025:406, distances 480 to 120): ANTs 26.2 bytes per
    # control point of the last level and 1.73e-8 s per point and iteration; larmorx 78.7 and
    # 2.17e-7 (the same results, bit for bit).
    ap.add_argument("--n4-point-bytes-2d", type=float, default=26.2)
    ap.add_argument("--n4-point-bytes-lx-2d", type=float, default=78.7)
    ap.add_argument("--n4-point-seconds-2d", type=float, default=1.73e-8)
    ap.add_argument("--n4-point-seconds-lx-2d", type=float, default=2.17e-7)
    ap.add_argument("--by-spacing-voxel-bytes", type=float, default=7.3)
    ap.add_argument("--by-spacing-voxel-bytes-lx", type=float, default=16.0)
    ap.add_argument("--box-gib", type=float, default=limits.HEAVY_GIB + limits.HEAVY_SWAP_GIB)
    ap.add_argument(
        "--record-beyond",
        action="store_true",
        help="write a result 'beyond this machine' (over the heavy box) or 'beyond any machine' "
        "(over 1 TiB) into heavy-results.jsonl for each case whose prediction exceeds the heavy "
        "box (difftest --heavy then skips it)",
    )
    args = ap.parse_args()
    rows = []
    with tempfile.TemporaryDirectory(prefix="lx-heavy-predict-") as t:
        for h in report.heavy(args.night):
            tool = tools.get(h["tool"])
            recipe = core.make_recipe(tool, h["seed"], h["n"])
            spec = recipe["inputs"].get("a") or recipe["inputs"].get("input")
            row = {
                "tool": h["tool"],
                "seed": h["seed"],
                "n": h["n"],
                "case": f"{h['tool']}:{h['seed']}:{h['n']}",
                "args": " ".join(core.arguments(recipe)),
                "result": h.get("result"),
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
                two = s["dimension"] == 2
                pb = args.n4_point_bytes_2d if two else args.n4_point_bytes
                pb_lx = args.n4_point_bytes_lx_2d if two else args.n4_point_bytes_lx
                ps = args.n4_point_seconds_2d if two else args.n4_point_seconds
                ps_lx = args.n4_point_seconds_lx_2d if two else args.n4_point_seconds_lx
                ants = max(s["points"]) * pb + s["padded_voxels"] * args.n4_voxel_bytes
                lx = max(s["points"]) * pb_lx + s["padded_voxels"] * args.n4_voxel_bytes_lx
                work = sum(i * pt for i, pt in zip(s["iterations"], s["points"], strict=True))
                pad_work = sum(s["iterations"]) * s["padded_voxels"]
                row.update(
                    sizing=s,
                    ants_gib=ants / GIB,
                    larmorx_gib=lx / GIB,
                    ants_hours=(work * ps + pad_work * args.n4_voxel_seconds) / 3600,
                    larmorx_hours=(work * ps_lx + pad_work * args.n4_voxel_seconds_lx) / 3600,
                )
            elif h["tool"] in ("ants-resample-image-by-spacing", "ants-resample-image"):
                s = (by_spacing_size if h["tool"].endswith("spacing") else resample_size)(
                    words, size, spacing
                )
                row.update(
                    sizing=s,
                    ants_gib=s["output_voxels"] * args.by_spacing_voxel_bytes / GIB,
                    larmorx_gib=s["output_voxels"] * args.by_spacing_voxel_bytes_lx / GIB,
                )
            elif h["tool"] == "afni-unifize":
                s = unifize_ball(words, size, spacing)
                row.update(
                    sizing=s,
                    ants_gib=s["points"] * 10.0 / GIB,
                    larmorx_gib=s["points"] * (24.0 + 4.0) / GIB,
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
    if args.record_beyond:
        beyond = [
            r
            for r in rows
            if r.get("result") is None
            and "ants_gib" in r
            and min(r["ants_gib"], r["larmorx_gib"]) > args.box_gib
        ]
        with (args.night / "heavy-results.jsonl").open("a", encoding="utf-8") as f:
            for r in beyond:
                s = r["sizing"]
                afni = r["tool"].startswith("afni-")
                last = (
                    f"a ball of radius {s['radius']:.1f} voxels, {s['points']:.3g} points"
                    if afni
                    else "largest lattice "
                    + "x".join(map(str, s.get("lattice_last", s.get("output", []))))
                )
                hours = (
                    f", about {r['ants_hours']:.3g} h of computation for ANTs"
                    if "ants_hours" in r
                    else ""
                )
                verdict = (
                    "beyond any machine"
                    if min(r["ants_gib"], r["larmorx_gib"]) > 1024
                    else "beyond this machine"
                )
                detail = (
                    (
                        f"{verdict} (AFNI-75): predicted from AFNI's sizing of its ball of "
                        f"neighbours, AFNI about {r['ants_gib']:.4g} GiB and the replica about "
                        f"{r['larmorx_gib']:.4g} GiB ({last}); not run in the heavy box; the "
                        "replica's default stops before building the ball (permanent case "
                        "epi-ball-beyond-any-machine)"
                    )
                    if afni
                    else (
                        f"{verdict} (ANTS-162): predicted from the growth law measured on "
                        f"smaller variants of the same input, ANTs about {r['ants_gib']:.4g} GiB "
                        f"and larmorx about {r['larmorx_gib']:.4g} GiB ({last}){hours}; not run "
                        "in the heavy box; larmorx's default stops before allocating (permanent "
                        "case), its replica bit-identical to ANTs on every variant both "
                        "finished (docs/findings/ants-n4.md, Memory)"
                    )
                )
                line = {
                    "tool": r["tool"],
                    "seed": r["seed"],
                    "n": r["n"],
                    "verdict": verdict,
                    "detail": detail,
                    "impls": {},
                    "predicted": {"ants_gib": r["ants_gib"], "larmorx_gib": r["larmorx_gib"]},
                }
                f.write(json.dumps(line, sort_keys=True) + "\n")
                print(f"recorded {verdict}: {r['case']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
