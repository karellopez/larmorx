# SPDX-License-Identifier: Apache-2.0
"""Every ANTs/ITK program node of the finished fMRIPrep oracle runs, by program and run.

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P inventory.py <workspace>/oracles/fmriprep-25.2.6 [run ...] [--json out.json]

Prints a table program x run (node counts) and, per program, every node's normalised path
(the catalogue's normalisation: subject and run names replaced) with its count, so that the
data-panel re-validation can say for each node whether a parity suite replays it.
"""

from __future__ import annotations

import collections
import json
import sys
from pathlib import Path

from larmorx_validation.stages import catalogue
from larmorx_validation.stages.oracle import OracleRun, run_finished

#: ANTs' and ITK's programs (what an oracle run can call of them).
ANTS_PROGRAMS = {
    "antsRegistration",
    "antsApplyTransforms",
    "N4BiasFieldCorrection",
    "DenoiseImage",
    "ImageMath",
    "ThresholdImage",
    "MultiplyImages",
    "SmoothImage",
    "ResampleImage",
    "ResampleImageBySpacing",
    "antsAI",
    "antsAffineInitializer",
    "Atropos",
    "CreateJacobianDeterminantImage",
    "LabelGeometryMeasures",
    "antsJointFusion",
    "antsBrainExtraction.sh",
    "antsCorticalThickness.sh",
    "CompositeTransformUtil",
}


def inventory(root: Path, names: list[str]) -> dict:
    table: dict[str, dict[str, int]] = collections.defaultdict(dict)
    paths: dict[str, dict[str, list[str]]] = collections.defaultdict(
        lambda: collections.defaultdict(list)
    )
    nodes: list[dict] = []
    for name in names:
        run_dir = root / name
        if not run_finished(run_dir):
            print(f"{name}: not finished, skipped", file=sys.stderr)
            continue
        run = OracleRun(run_dir)
        for n in run.all_nodes:
            prog = n.program
            if not prog or prog not in ANTS_PROGRAMS:
                continue
            table[prog][name] = table[prog].get(name, 0) + 1
            norm, _ = catalogue.normalise(n.path)
            paths[prog][norm].append(f"{name}:{n.path}")
            st = catalogue.stage_of(n)
            nodes.append(
                {
                    "run": name,
                    "path": n.path,
                    "program": prog,
                    "norm": norm,
                    "stage": getattr(st, "id", None) if st is not None else None,
                    "threads": n.threads,
                    "duration": n.duration,
                    "command": n.command,
                }
            )
    return {"table": table, "paths": paths, "nodes": nodes}


def main() -> int:
    root = Path(sys.argv[1])
    args = sys.argv[2:]
    out_json = None
    if "--json" in args:
        i = args.index("--json")
        out_json = Path(args[i + 1])
        del args[i : i + 2]
    names = args or sorted(p.name for p in root.iterdir() if (p / "work").is_dir())
    inv = inventory(root, names)
    short = [n.split("-sub-")[0] for n in names]
    print(f"{'program':26s} " + " ".join(f"{s[:8]:>8s}" for s in short) + "   total")
    for prog in sorted(inv["table"]):
        row = [inv["table"][prog].get(n, 0) for n in names]
        print(f"{prog:26s} " + " ".join(f"{c:8d}" for c in row) + f"   {sum(row):5d}")
    for prog in sorted(inv["paths"]):
        print(f"\n== {prog}")
        for norm, ps in sorted(inv["paths"][prog].items()):
            print(f"  {len(ps):3d}  {norm}")
    if out_json:
        out_json.write_text(json.dumps(inv["nodes"], indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
