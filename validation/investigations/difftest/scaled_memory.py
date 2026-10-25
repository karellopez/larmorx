# SPDX-License-Identifier: Apache-2.0
"""A heavy random case of any ANTs program at smaller scales: the argument words named by
``--scale-args`` (their 0-based positions in the case's arguments) multiplied by each of
``--factors`` (e.g. an output spacing made 8, 4 and 2 times coarser), each run of ANTs (one
thread) and of larmorx (both D13 modes) in a control group of its own with a hard limit and no
swap (``n4_memory.run_in_leaf``), the peak memory, the time and the outputs compared voxel for
voxel; ``--unit`` names what the scale changes (``output voxels`` from the outputs' sizes), and
the peak is fitted against it to predict the case (factor 1).

    boxed.sh 6 python -P validation/investigations/difftest/scaled_memory.py \\
        ants-resample-image-by-spacing:20261025:137 ResampleImageBySpacing <out dir> \\
        --scale-args 3,4 --factors 8,4,2
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
from pathlib import Path

import nibabel as nib
from larmorx_validation.difftest import core, inputs, limits, tools
from larmorx_validation.parity import ants_programs

sys.path.insert(0, str(Path(__file__).resolve().parent))
from n4_memory import fit, run_in_leaf, same_voxels


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("case")
    ap.add_argument("program")
    ap.add_argument("out", type=Path)
    ap.add_argument("--scale-args", required=True)
    ap.add_argument("--factors", default="8,4,2")
    ap.add_argument("--cap-gib", type=float, default=5.0)
    ap.add_argument("--timeout", type=float, default=1800.0)
    ap.add_argument("--who", default="ANTs,reproduce,fix")
    ap.add_argument(
        "--night",
        type=Path,
        default=None,
        help="the night that ran the case: its recipe as the night drew it (the generator "
        "changes; runner.night_recipe)",
    )
    args = ap.parse_args()
    box = limits.MemoryBox.find()
    if box is None:
        raise SystemExit(
            "run it in a memory box: scripts/orchestration/boxed.sh <GiB> python -P ..."
        )
    box.nest()
    name, seed, n = args.case.rsplit(":", 2)
    if args.night is not None:
        from larmorx_validation.difftest import runner

        recipe = runner.night_recipe(args.night, {"tool": name, "seed": seed, "n": int(n)})
    else:
        recipe = core.make_recipe(tools.get(name), seed, int(n))
    args.out.mkdir(parents=True, exist_ok=True)
    paths = {}
    for k, spec in recipe["inputs"].items():
        p = args.out / (k + inputs.suffix_of(spec))
        inputs.materialize(spec, p)
        paths[k] = str(p)
    words = core.substitute(core.arguments(recipe), lambda k: paths[k])
    positions = [int(x) for x in args.scale_args.split(",")]
    env = {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1", "ANTS_RANDOM_SEED": "42"}
    rows = []
    for factor in [float(f) for f in args.factors.split(",")]:
        scaled = list(words)
        for i in positions:
            scaled[i] = f"{float(words[i]) * factor:g}"
        row = {"factor": factor, "args": scaled, "runs": {}}
        outs = {}
        for who in args.who.split(","):
            d = args.out / f"x{factor:g}" / who
            d.mkdir(parents=True, exist_ok=True)
            a = [w.replace("{out}", str(d / "out.nii.gz")) for w in scaled]
            outs[who] = d / "out.nii.gz"
            if who == "ANTs":
                cmd, e = [str(ants_programs.ants_bin() / args.program), *a], env
            else:
                cmd = [sys.executable, "-m", "larmorx", "ants", args.program, *a]
                e = {**env, "LARMORX_UPSTREAM_DEFECTS": who}
            r = run_in_leaf(
                box,
                f"scaled-{factor:g}-{who}",
                int(args.cap_gib * limits.GIB),
                cmd,
                e,
                args.timeout,
            )
            if outs[who].is_file():
                r["output_voxels"] = math.prod(nib.load(str(outs[who])).shape)
            row["runs"][who] = r
            print(
                f"x{factor:g} {who}: exit {r['exit']} {r['seconds']} s peak {r['peak']} "
                f"voxels {r.get('output_voxels')}"
                + (" STOPPED BY THE LIMIT" if r["memory_limit"] else ""),
                flush=True,
            )
        for who in outs:
            if who != "ANTs" and "ANTs" in outs:
                row["runs"][who]["against_ANTs"] = same_voxels(outs["ANTs"], outs[who])
        rows.append(row)
    result = {"case": args.case, "program": args.program, "rows": rows, "fits": {}}
    for who in args.who.split(","):
        ok = [
            r["runs"][who]
            for r in rows
            if r["runs"][who]["exit"] == 0 and r["runs"][who].get("output_voxels")
        ]
        if len(ok) >= 2:
            a, b = fit([x["output_voxels"] for x in ok], [x["peak_bytes"] for x in ok])
            ta, tb = fit([x["output_voxels"] for x in ok], [x["seconds"] for x in ok])
            result["fits"][who] = {
                "bytes": a,
                "bytes_per_output_voxel": b,
                "seconds": ta,
                "seconds_per_output_voxel": tb,
            }
    print(json.dumps(result["fits"], indent=1))
    (args.out / "scaled_memory.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
