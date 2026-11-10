# SPDX-License-Identifier: Apache-2.0
"""The table of part B's register entries at a glance, for docs/findings/sdcflows-estimation.md
(its section "Part B's register entries at a glance"), from the register's text of the part
(``relay17/register_part_b.py``, the source of the handoff's register rows and sections): one
row per entry, ID, the original, the class, the defect, fMRIPrep's calls and what LARMORX does.
The site's sdcflows-estimation page reads this table (``evidence/sdcflows_estimation.py``).

Usage: ``python -P findings_table.py`` (prints the table; paste it into the section).
"""

from __future__ import annotations

import contextlib
import io
import runpy
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "relay17" / "register_part_b.py"


def entries() -> list[tuple]:
    with contextlib.redirect_stdout(io.StringIO()):
        ns = runpy.run_path(str(SOURCE))
    return [*ns["SDC"], *ns["PIPE"], *ns["SKI"], *ns["SCI"]]


def cell(text: str) -> str:
    return " ".join(str(text).split()).replace("|", "\\|")


def main() -> None:
    print("| ID | Original | Class | The defect | fMRIPrep's calls | LARMORX |")
    print("|---|---|---|---|---|---|")
    for i, _title, cls, tool, short, users, today, _gap, _ev in entries():
        print(f"| {i} | {cell(tool)} | {cls} | {cell(short)} | {cell(users)} | {cell(today)} |")


if __name__ == "__main__":
    main()
