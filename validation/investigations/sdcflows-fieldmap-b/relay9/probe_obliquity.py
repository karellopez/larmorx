# SPDX-License-Identifier: Apache-2.0
import glob

import nibabel as nb
import numpy as np

import larmorx as lx

for run in sorted(
    glob.glob(
        "/home/karelo/VScode_projects/super_fmriprep/archive/2026-11-08-fmriprep-runs-avx512-kernels/*-syn/work/fmriprep_25_2_wf/*/syn_preprocessing_*/epi_reference_wf/clip_bg_noise/mapflow/_clip_bg_noise0/clipped.nii.gz"
    )
):
    a = nb.load(run).affine
    print(run.split("/")[7], run.split("/")[11], np.round(np.degrees(lx.sdcflows.obliquity(a)), 2))
