# SPDX-License-Identifier: Apache-2.0
"""Print the execution inputs (and outputs) of every node of fMRIPrep's confounds workflow
in one oracle run's working directory, with whether each input file still exists."""

import os
import re
import sys
from pathlib import Path

# <workspace>: LARMORX_WORKSPACE, else the folder that holds the repository.
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
ROOT = WORKSPACE / "oracles" / "fmriprep-25.2.6"
run = ROOT / sys.argv[1]
show_outputs = "--outputs" in sys.argv
only = [a for a in sys.argv[2:] if not a.startswith("--")]

work = run / "work"
for wf in sorted(work.glob("fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf")):
    print(f"#### {wf.relative_to(work)}")
    for node in sorted(p for p in wf.iterdir() if p.is_dir()):
        if only and node.name not in only:
            continue
        reports = sorted(node.rglob("_report/report.rst"))
        for rep in reports:
            text = rep.read_text(errors="replace")
            m = re.search(
                r"Execution Inputs\n-+\n(.*?)\n\nExecution Outputs\n-+\n(.*?)\n\nRuntime info",
                text,
                re.S,
            )
            if not m:
                continue
            print(f"== {rep.parent.parent.relative_to(wf)}")
            for line in m.group(1).splitlines():
                if not line.startswith("* "):
                    continue
                if "<undefined>" in line:
                    continue
                paths = re.findall(r"/work/[^\s',\]]+", line)
                flags = "".join("+" if (work / p[len("/work/") :]).exists() else "-" for p in paths)
                print(f"   {line[:300]} {flags}")
            if show_outputs:
                for line in m.group(2).splitlines():
                    if line.startswith("* ") and "<undefined>" not in line:
                        print(f"   OUT {line[:300]}")
