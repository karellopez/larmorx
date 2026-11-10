# SPDX-License-Identifier: Apache-2.0
"""``lx.niworkflows.robust_average`` of ds005040's ``dir-A`` EPI (``sys.argv[1]``) with numpy
2.2.6's, 2.3.5's and 2.5.4's arithmetic against fMRIPrep 25.2.6's own ``runwise_avg`` file: only
2.2.6's is fMRIPrep's (numpy 2.3 sums the drift's volumes pairwise). Run with
``PYTHONPATH=<repo>/python``, ``LARMORX_WORKSPACE`` set (relay 14)."""

import os
import sys
import tempfile
from pathlib import Path

import nibabel as nb
import numpy as np

from larmorx.niworkflows import robust_average

R = Path(os.environ["LARMORX_WORKSPACE"]) / (
    "oracles/fmriprep-25.2.6/ds005040-sub-002-noreconall/work"
    "/fmriprep_25_2_wf/sub_002_wf/fmap_preproc_wf/wf_auto_00000/runwise_avg/mapflow/_runwise_avg0"
)
src = sys.argv[1]
ref = next(R.glob("*_average.nii.gz"))
for npv in ("2.2.6", "2.3.5", "2.5.4"):
    for impl in ("replica",):
        with tempfile.TemporaryDirectory() as tmp:
            m = robust_average(
                src,
                num_threads=8,
                out_dir=tmp,
                numpy_version=npv,
                volreg_implementation=impl,
                upstream_defects="reproduce",
                n_threads=8,
            )
            a = np.asanyarray(nb.load(ref).dataobj)
            b = np.asanyarray(nb.load(m.out_file).dataobj)
            print(npv, impl, "same" if a.tobytes() == b.tobytes() else f"DIFF {(a != b).sum()}")
