# SPDX-License-Identifier: Apache-2.0
"""How much memory and time N4BiasFieldCorrection needs as its B-spline lattice grows: a random
case's input with smaller variants of its ``-b [distance, order]`` and convergence levels, each
run of ANTs (one thread) and of larmorx (both D13 modes) in a control group of its own with a hard
limit and no swap, its peak memory read from the group's ``memory.peak``; the lattice of each
level computed as ANTs sizes it (``N4BiasFieldCorrection.cxx``: per axis
``ceil((size - 1) * spacing / distance)`` spans, ``spans + order`` control points, the spans
doubled at each fitting level), the outputs compared voxel for voxel, and the peak and the time
fitted against the lattice (the growth law), to predict the case itself.

Runs only inside a memory box (``scripts/orchestration/boxed.sh <GiB> python -P ...``):

    boxed.sh 6 python -P validation/investigations/difftest/n4_memory.py \\
        ants-n4:20261025:24 <out dir> --distances 52000,26000,13000,6500,3250,1625,1150 \\
        --cap-gib 5 --predict 20

The table goes to ``<out dir>/n4_memory.json`` and ``n4_memory.md``.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import subprocess
import sys
import time
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest import core, inputs, limits, tools
from larmorx_validation.parity import ants_programs

SPATIAL = {"meter": 1000.0, "mm": 1.0, "micron": 0.001, "unknown": 1.0}
TEMPORAL = {"sec": 1.0, "msec": 0.001, "usec": 1e-6, "unknown": 1.0}


def itk_spacing(path: Path) -> tuple[list[int], list[float]]:
    """The size and spacing ITK's NIfTI reader gives (metres and microns to mm, the time axis's
    msec and usec to s)."""
    img = nib.load(str(path))
    xyz, t = img.header.get_xyzt_units()
    zooms = [float(z) for z in img.header.get_zooms()]
    spacing = [z * SPATIAL.get(xyz, 1.0) for z in zooms[:3]]
    spacing += [z * TEMPORAL.get(t, 1.0) for z in zooms[3:]]
    return list(img.shape), spacing


def lattice(size, spacing, distance: float, order: int, levels: int) -> list[list[int]]:
    """The control points of each fitting level, as N4BiasFieldCorrection sizes them (float
    arithmetic)."""
    spans = []
    for n, s in zip(size, spacing, strict=True):
        domain = np.float32(n - 1) * np.float32(s)
        spans.append(math.ceil(float(np.float32(domain / np.float32(distance)))))
    return [[sp * (1 << lv) + order for sp in spans] for lv in range(levels)]


def run_in_leaf(box: limits.MemoryBox, name: str, cap: int, cmd, env, timeout: float) -> dict:
    """``cmd`` in a fresh leaf of ``box`` (``cap`` bytes, no swap): exit status, seconds, peak."""
    leaf = box.leaf(name, cap)
    harness = box.root / "harness" / "cgroup.procs"
    oom = leaf.oom_kills()
    leaf.enter()
    try:
        start = time.monotonic()
        p = subprocess.Popen(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    finally:
        harness.write_text("0", encoding="ascii")
    try:
        _, err = p.communicate(timeout=timeout)
        code = p.returncode
    except subprocess.TimeoutExpired:
        p.kill()
        _, err = p.communicate()
        code = "timeout"
    seconds = time.monotonic() - start
    peak = leaf.peak()
    stopped = leaf.oom_kills() > oom
    leaf.remove()
    return {
        "exit": code,
        "seconds": round(seconds, 2),
        "peak_bytes": peak,
        "peak": None if peak is None else limits.gib(peak),
        "memory_limit": stopped,
        "stderr": err.decode("utf-8", "replace")[-600:],
    }


def same_voxels(a: Path, b: Path) -> str:
    if not (a.is_file() and b.is_file()):
        return "missing"
    x = np.asanyarray(nib.load(str(a)).dataobj)
    y = np.asanyarray(nib.load(str(b)).dataobj)
    if x.shape != y.shape:
        return f"shapes {x.shape} {y.shape}"
    if np.array_equal(x, y, equal_nan=True):
        return "bit-identical"
    d = np.abs(x.astype(np.float64) - y.astype(np.float64))
    return f"{int(np.count_nonzero(d))} voxels differ, max {float(np.nanmax(d)):.3g}"


def fit(xs, ys) -> tuple[float, float]:
    """Least squares ``y = a + b x``."""
    A = np.vstack([np.ones(len(xs)), np.asarray(xs, dtype=np.float64)]).T
    (a, b), *_ = np.linalg.lstsq(A, np.asarray(ys, dtype=np.float64), rcond=None)
    return float(a), float(b)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("case", help="TOOL:SEED:N (an ants-n4 case)")
    ap.add_argument("out", type=Path)
    ap.add_argument("--distances", default="52000,26000,13000,6500,3250,1625")
    ap.add_argument("--levels", type=int, default=None, help="fitting levels (default: the case's)")
    ap.add_argument("--iterations", type=int, default=50)
    ap.add_argument("--cap-gib", type=float, default=5.0)
    ap.add_argument("--timeout", type=float, default=1800.0)
    ap.add_argument("--who", default="ANTs,reproduce,fix")
    ap.add_argument("--predict", type=float, default=None, help="the case's own distance")
    args = ap.parse_args()
    box = limits.MemoryBox.find()
    if box is None:
        raise SystemExit(
            "run it in a memory box: scripts/orchestration/boxed.sh <GiB> python -P ..."
        )
    box.nest()
    name, seed, n = args.case.rsplit(":", 2)
    recipe = core.make_recipe(tools.get(name), seed, int(n))
    words = core.arguments(recipe)
    args.out.mkdir(parents=True, exist_ok=True)
    spec = recipe["inputs"]["a"]
    src = args.out / ("a" + inputs.suffix_of(spec))
    inputs.materialize(spec, src)
    size, spacing = itk_spacing(src)
    dim = int(words[words.index("-d") + 1])
    b = words[words.index("-b") + 1]
    order = int(re.findall(r"[\d.]+", b)[1]) if "," in b else 3
    case_levels = 4
    if "-c" in words:
        case_levels = len(words[words.index("-c") + 1].strip("[ ").split(",")[0].split("x"))
    levels = args.levels or case_levels
    print(f"input {size} spacing (ITK) {spacing}; -d {dim} order {order}, {levels} levels")
    env = {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1", "ANTS_RANDOM_SEED": "42"}
    rows = []
    for distance in [float(x) for x in args.distances.split(",") if x]:
        lat = lattice(size[:dim], spacing[:dim], distance, order, levels)
        points = [math.prod(c) for c in lat]
        conv = "x".join([str(args.iterations)] * levels)
        row = {
            "distance": distance,
            "levels": levels,
            "iterations": args.iterations,
            "lattice": lat,
            "points": points,
            "runs": {},
        }
        outs = {}
        for who in args.who.split(","):
            d = args.out / f"d{distance:g}" / who
            d.mkdir(parents=True, exist_ok=True)
            outs[who] = d / "out.nii.gz"
            a = ["-d", str(dim), "-i", str(src), "-s", words[words.index("-s") + 1]]
            a += ["-b", f"[{distance:g},{order}]", "-c", f"[{conv},0]", "-o", str(outs[who])]
            if who == "ANTs":
                cmd = [str(ants_programs.ants_bin() / "N4BiasFieldCorrection"), *a]
                e = env
            else:
                cmd = [sys.executable, "-m", "larmorx", "ants", "N4BiasFieldCorrection", *a]
                e = {**env, "LARMORX_UPSTREAM_DEFECTS": who}
            r = run_in_leaf(
                box, f"n4-{distance:g}-{who}", int(args.cap_gib * limits.GIB), cmd, e, args.timeout
            )
            row["runs"][who] = r
            print(
                f"distance {distance:g}: lattice {lat[-1]} ({points[-1]:.3g} points at the last "
                f"level) {who}: exit {r['exit']} {r['seconds']} s peak {r['peak']}"
                + (" STOPPED BY THE LIMIT" if r["memory_limit"] else ""),
                flush=True,
            )
        for who in outs:
            if who != "ANTs" and "ANTs" in outs:
                row["runs"][who]["against_ANTs"] = same_voxels(outs["ANTs"], outs[who])
        rows.append(row)
    result = {"case": args.case, "size": size, "spacing_itk": spacing, "rows": rows, "fits": {}}
    for who in args.who.split(","):
        ok = [r for r in rows if r["runs"][who]["exit"] == 0 and r["runs"][who]["peak_bytes"]]
        if len(ok) < 3:
            continue
        # The largest lattices dominate: fit on the runs whose last-level lattice holds at least
        # a million control points (the process's own baseline shows below that).
        big = [r for r in ok if r["points"][-1] >= 1e6] or ok
        xs = [r["points"][-1] for r in big]
        a_m, b_m = fit(xs, [r["runs"][who]["peak_bytes"] for r in big])
        work = [sum(p * r["iterations"] for p in r["points"]) for r in big]
        a_t, b_t = fit(work, [r["runs"][who]["seconds"] for r in big])
        result["fits"][who] = {
            "peak_bytes": {"a": a_m, "bytes_per_point_last_level": b_m},
            "seconds": {"a": a_t, "seconds_per_point_iteration": b_t},
            "runs_fitted": len(big),
        }
    if args.predict:
        lat = lattice(size[:dim], spacing[:dim], args.predict, order, case_levels)
        points = [math.prod(c) for c in lat]
        pred = {"distance": args.predict, "lattice": lat, "points": points}
        for who, f in result["fits"].items():
            per_level = [
                f["peak_bytes"]["a"] + f["peak_bytes"]["bytes_per_point_last_level"] * p
                for p in points
            ]
            pred[who] = {
                "peak_by_level": [limits.gib(int(x)) for x in per_level],
                "seconds_by_level": [
                    f["seconds"]["seconds_per_point_iteration"] * p * 50 for p in points
                ],
            }
        result["prediction"] = pred
        print(json.dumps(pred, indent=1))
    (args.out / "n4_memory.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    md = [
        f"# N4 memory growth: {args.case}",
        "",
        f"Input {size}, ITK spacing {spacing}; {levels} fitting levels of {args.iterations} "
        "iterations; one thread; each run in a control group of its own (no swap).",
        "",
        "| distance (mm) | last-level lattice | points (last level) | who | exit | s | peak | against ANTs |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for r in rows:
        for who, x in r["runs"].items():
            md.append(
                f"| {r['distance']:g} | {'x'.join(map(str, r['lattice'][-1]))} | "
                f"{r['points'][-1]:.3g} | {who} | {x['exit']}"
                f"{' (limit)' if x['memory_limit'] else ''} | {x['seconds']} | {x['peak']} | "
                f"{x.get('against_ANTs', '')} |"
            )
    (args.out / "n4_memory.md").write_text("\n".join(md) + "\n", encoding="utf-8")
    print("\n".join(md))
    return 0


if __name__ == "__main__":
    sys.exit(main())
