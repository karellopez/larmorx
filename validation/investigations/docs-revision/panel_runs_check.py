# SPDX-License-Identifier: Apache-2.0
"""Every fMRIPrep run an evidence page names in its data panel's cases and worked examples is one
of the panel's fourteen (``fmriprep_nodes.PANEL_RUNS``): a check of every exported JSON of the
site, beside summary_check.py in every chunk of the documentation revision (the reviewer's round
12, finding 1: DenoiseImage's panel cases named nodes of the 2026-11-08 runs, which no panel
record compared, after an export that read every run of the folder).

    PYTHONPATH=validation/src python -P panel_runs_check.py <site>

Prints each page and run outside the fourteen; the last line counts them (exit status 1 when
any is found). Other parts of a page (tedana's or sdcflows' own runs) are not the panel's and
are not checked.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

from larmorx_validation.parity.fmriprep_nodes import PANEL_RUNS

#: A run's name ("ds000228-sub-pixar008-noreconall-syn"), not a figures-cache entry named after
#: one ("fsl-flirt/fmriprep-ds000228-sub-pixar008-noreconall-original").
RUN = re.compile(r"(?<!fmriprep-)\bds\d{6}-sub-[A-Za-z0-9]+-noreconall(?:-[a-z]+)?\b")


def outside(doc: dict) -> set[str]:
    """The runs named in ``doc``'s data-panel cases and examples that are not the panel's."""
    parts = [c for c in doc.get("cases", []) if c.get("category", "").startswith("panel")]
    parts += [e for e in doc.get("examples", []) if e.get("id", "").startswith("panel")]
    found = set(RUN.findall(json.dumps(parts)))
    return {r for r in found if r not in PANEL_RUNS}


def main() -> int:
    site = Path(sys.argv[1])
    bad = 0
    pages = sorted((site / "static" / "evidence").glob("*.json"))
    for f in pages:
        runs = outside(json.loads(f.read_text(encoding="utf-8")))
        for r in sorted(runs):
            print(f"{f.name}: {r}")
        bad += len(runs)
    print(f"{len(pages)} evidence files, {bad} runs outside the data panel's fourteen")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
