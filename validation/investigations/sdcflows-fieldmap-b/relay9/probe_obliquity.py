# SPDX-License-Identifier: Apache-2.0
import os
from pathlib import Path

import nibabel as nb
import numpy as np

import larmorx as lx

# The workspace (scripts/orchestration/env.sh sets LARMORX_WORKSPACE).
WORKSPACE = Path(
    os.environ.get("LARMORX_WORKSPACE", Path.home() / "VScode_projects/super_fmriprep")
)
RUNS = WORKSPACE / "archive/2026-11-08-fmriprep-runs-avx512-kernels"

for run in sorted(
    RUNS.glob(
        "*-syn/work/fmriprep_25_2_wf/*/syn_preprocessing_*/epi_reference_wf/clip_bg_noise/mapflow/_clip_bg_noise0/clipped.nii.gz"
    )
):
    a = nb.load(run).affine
    parts = run.relative_to(RUNS).parts  # the run, work, fmriprep_25_2_wf, the subject, the stage
    print(parts[0], parts[4], np.round(np.degrees(lx.sdcflows.obliquity(a)), 2))
