# SPDX-License-Identifier: Apache-2.0
# ruff: noqa: E402, F401 (relay 1's probes, kept as they ran)
import os

"""Probe: can sdcflows 2.17.1's init_3dQwarp_wf be built at all (its unwarp_reference node reads
``inputnode.in_reference``, a field the inputnode does not have)? Also builds init_topup_wf,
init_syn_sdc_wf, init_syn_preprocessing_wf and init_fmap_wf (both modes) and prints their nodes.

    TEMPLATEFLOW_HOME=<workspace>/oracles/templateflow python -I probe_qwarp_wf.py
"""

import traceback

from sdcflows.workflows.fit import fieldmap, pepolar, syn

for name, build in (
    ("init_3dQwarp_wf", lambda: pepolar.init_3dQwarp_wf(omp_nthreads=1)),
    ("init_topup_wf", lambda: pepolar.init_topup_wf()),
    ("init_topup_wf(debug)", lambda: pepolar.init_topup_wf(debug=True)),
    ("init_syn_sdc_wf", lambda: syn.init_syn_sdc_wf(omp_nthreads=1)),
    (
        "init_syn_preprocessing_wf",
        lambda: syn.init_syn_preprocessing_wf(omp_nthreads=1, auto_bold_nss=True),
    ),
    ("init_fmap_wf(phasediff)", lambda: fieldmap.init_fmap_wf()),
    ("init_fmap_wf(mapped)", lambda: fieldmap.init_fmap_wf(mode="mapped")),
):
    try:
        wf = build()
        names = sorted(n.split(".", 1)[-1] for n in wf.list_node_names())
        print(f"{name}: OK, {len(names)} nodes: {', '.join(names)}")
    except Exception as e:
        print(f"{name}: FAILS: {type(e).__name__}: {str(e)[:400]}")
        traceback.print_exc(limit=2)
