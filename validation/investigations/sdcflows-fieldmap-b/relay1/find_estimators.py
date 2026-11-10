# SPDX-License-Identifier: Apache-2.0

"""List the field-map workflows (wf_*, pepolar, syn) in every fMRIPrep oracle run's work dir."""

import os

root = os.path.join(os.environ["LARMORX_WORKSPACE"], "oracles/fmriprep-25.2.6")
for run in sorted(os.listdir(root)):
    wf = os.path.join(root, run, "work", "fmriprep_25_2_wf")
    if not os.path.isdir(wf):
        continue
    print("==", run)
    for dp, dns, _fns in os.walk(wf):
        rel = dp[len(wf) + 1 :]
        depth = rel.count(os.sep)
        name = os.path.basename(dp)
        if name.startswith("wf_") or "syn" in name or "pepolar" in name:
            print("  ", rel)
        if depth >= 4:
            dns[:] = []
