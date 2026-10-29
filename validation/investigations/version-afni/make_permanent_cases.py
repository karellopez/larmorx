# SPDX-License-Identifier: Apache-2.0
"""The replicas' permanent cases of AFNI 26.3.05's AFNI-03 fix in 3dAutomask and 3dUnifize.

Random testing found AFNI-03 there (header slice code 2: the slice timing an output keeps) with
the clean-room originals; their permanent cases run the original only. AFNI 26.3.05 reads the
code as NIfTI defines it, and the replicas follow it in both modes: these cases run the same
recipes against the replicas (and AFNI 26.3.05, their default build), so the changed path is
tested where it changed. Run once from the repository: ``python -P <this file>``.
"""

from __future__ import annotations

import json
from pathlib import Path

CASES = Path(__file__).resolve().parents[2] / "difftest" / "cases"
MADE = {
    ("afni-automask", "slice-code-2-masked-notices"): (
        "slice-code-2-masked-afni26",
        "Header slice code 2 with -apply_prefix, against the replica and AFNI 26.3.05: AFNI "
        "26.3.05 reads the code as NIfTI defines it (AFNI-03 fixed upstream), so the masked "
        "series keeps the header's slice timing in both modes and no AFNI-03 is named "
        "(the version update, 2026-10-29)",
    ),
    ("afni-unifize", "epi-slice-code-2-fix-notice"): (
        "epi-slice-code-2-afni26",
        "-EPI on a series with header slice code 2, against the replica and AFNI 26.3.05: the "
        "output keeps the header's slice timing in both modes and no AFNI-03 is named "
        "(AFNI-03 fixed upstream; the version update, 2026-10-29)",
    ),
}


def main() -> None:
    for (tool, source), (new_id, description) in MADE.items():
        src = json.loads((CASES / tool / f"{source}.json").read_text(encoding="utf-8"))
        case = dict(src)
        case.update(
            id=new_id,
            description=description,
            implementations=["replica"],
            status="register",
            defect="AFNI-03",
            reason="",
        )
        case["found"] = {**src.get("found", {}), "from_case": source}
        out = CASES / tool / f"{new_id}.json"
        out.write_text(json.dumps(case, indent=1, sort_keys=True) + "\n", encoding="utf-8")
        print(out)


if __name__ == "__main__":
    main()
