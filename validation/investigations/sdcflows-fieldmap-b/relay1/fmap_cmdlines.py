# SPDX-License-Identifier: Apache-2.0
# ruff: noqa: SIM115 (relay 1's probes, kept as they ran)
"""The command lines nipype built for the programs under the field-map nodes (from each node's
command.txt), with the working-directory paths shortened: one per distinct node name and run.

    python -I fmap_cmdlines.py [run-substring ...]
"""

import os
import re
import sys

ROOT = os.path.join(os.environ["LARMORX_WORKSPACE"], "oracles/fmriprep-25.2.6")
runs = sys.argv[1:]
seen = set()
for run in sorted(os.listdir(ROOT)):
    if runs and not any(r in run for r in runs):
        continue
    wf = os.path.join(ROOT, run, "work", "fmriprep_25_2_wf")
    for dp, _dns, fns in os.walk(wf):
        if "command.txt" not in fns:
            continue
        rel = dp[len(wf) + 1 :]
        if not ("fmap" in rel or "skullstrip_bold_wf" in rel or "syn" in rel):
            continue
        key = (run, re.sub(r"wf_[\w]+|_ds_coeff\d|bold_task_[\w]+_wf", "*", rel.split("/", 1)[-1]))
        if key in seen:
            continue
        seen.add(key)
        cmd = open(os.path.join(dp, "command.txt"), encoding="utf-8").read().strip()
        cmd = re.sub(r"/work/fmriprep_25_2_wf/\S*?/([^/\s,\]]+)", r".../\1", cmd)
        print(f"[{run}] {key[1]}\n    {cmd[:700]}")
