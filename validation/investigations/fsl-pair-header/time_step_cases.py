# SPDX-License-Identifier: Apache-2.0
"""Writes the permanent cases of the time step as FSL reads it (``time_step.py``'s finding, PRELUDE's
clean-room part, relay 3): for the clean-room originals whose random testing never draws a series
of time step 0 or below (their draws' time steps are 0.5 to 3 s), the first realistic case of
their own draws (seed ``time-step``) with a 4D input, its time step set to 0 or -2. Each case runs
in its tool's parity suite (``validation/difftest/cases/<tool>/``), which compares the headers
with FSL's.

    python -P time_step_cases.py [--out <cases root>]   (PYTHONPATH=<checkout>/validation/src)
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from larmorx_validation.difftest import core, inputs, tools

#: (tool, case id, time step, what the case is, the draw's number or None for the first series;
#: mri-hmc's case 10 of the seed: ``-plots -rmsrel``, the default cost, so that the case tests the
#: header and not a registration's random divergence)
CASES = (
    ("fsl-maths-original", "time-step-zero-series", 0.0, "fslmaths on a series of time step 0", None),
    (
        "fsl-maths-original",
        "time-step-negative-series",
        -2.0,
        "fslmaths on a series of time step -2",
        None,
    ),
    ("mri-hmc", "time-step-negative-series", -2.0, "mcflirt's engine on a series of time step -2", 10),
    ("fsl-topup-original", "time-step-zero-series", 0.0, "topup on a series of time step 0", None),
)


def first_series(tool, seed: str, start: int = 0):
    """The first realistic case from ``start`` whose inputs hold a 4D image."""
    for n in range(start, start + 4000):
        r = core.make_recipe(tool, seed, n, extreme_share=0.0)
        four = [k for k, v in r["inputs"].items() if len(v.get("shape", [])) == 4]
        if four:
            return n, r, four
    raise SystemExit(f"{tool.name}: no case with a 4D input in 4000 draws")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", type=Path, default=inputs.CASES)
    a = ap.parse_args()
    used: dict[str, int] = {}
    for tool_name, case_id, tr, what, fixed in CASES:
        t = tools.get(tool_name)
        start = fixed if fixed is not None else used.get(tool_name, -1) + 1
        n, recipe, four = first_series(t, "time-step", start)
        used[tool_name] = n
        for k in four:
            timing = recipe["inputs"][k].setdefault("timing", {})
            timing["tr"] = tr
            timing.setdefault("tunits", "sec")
            # a negative or zero time step has no slice timing
            for f in ("slice_code", "slice_start", "slice_end", "slice_duration"):
                timing.pop(f, None)
        doc = {
            "id": case_id,
            "tool": tool_name,
            "suite": t.suite,
            "implementations": [t.implementations[0]],
            "description": (
                f"{what}: FSL writes the time step as its image layer reads it (its magnitude, "
                "1 for a series of 0); the clean-room originals wrote the stored value"
            ),
            "found": {
                "seed": "time-step",
                "n": n,
                "signature": ["time step as read"],
                "detail": (
                    "validation/investigations/fsl-pair-header/time_step.py: 47 of 66 runs of the "
                    "FSL originals differed in pixdim[4] before the shared readers' fix"
                ),
                "shrink_moves": [],
            },
            "status": "fixed",
            "defect": None,
            "rule": None,
            "tier": "smoke",
            "expect": "pass",
            "reason": "",
            "recipe": recipe,
        }
        d = a.out / tool_name
        d.mkdir(parents=True, exist_ok=True)
        p = d / f"{case_id}.json"
        p.write_text(json.dumps(doc, indent=1, sort_keys=True) + "\n", encoding="utf-8")
        print(f"{p}: case {n}, inputs {four}, time step {tr}, options {recipe['options']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
