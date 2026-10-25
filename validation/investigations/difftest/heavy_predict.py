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
import sys
import tempfile
from pathlib import Path

from larmorx_validation.difftest import core, inputs, limits, report, runner, sizing

sys.path.insert(0, str(Path(__file__).resolve().parent))

GIB = limits.GIB


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("night", type=Path)
    # The unit costs are sizing.py's (larmorx_validation.difftest.sizing).
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
            recipe = runner.night_recipe(args.night, h)
            if recipe is None:
                rows.append(
                    {"case": f"{h['tool']}:{h['seed']}:{h['n']}", "prediction": "no recipe kept"}
                )
                continue
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
            size, spacing = sizing.itk_spacing(p)
            row.update(size=size, spacing_itk=spacing)
            words = core.arguments(recipe)
            if h["tool"] in sizing.TOOLS:
                pred = sizing.predict_from(h["tool"], words, size, spacing)
                row.update({k: v for k, v in pred.items() if k != "law"})
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
                # Over 1 TiB: beyond any machine; over this machine's heavy box: pending, to
                # run on a larger machine (heavy_pending.py adds its command line).
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
