# SPDX-License-Identifier: Apache-2.0
"""Inputs shared by the ImageMath case modules (catalog paths and generated inputs)."""

from __future__ import annotations

from larmorx_validation.parity.ants_programs import AntsCase

F = "openneuro-derivatives/ds000005-fmriprep/sub-01"
FUNC = f"{F}/func/sub-01_task-mixedgamblestask_run-1"
TPL = "templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym"

#: Real images (catalog paths) and their tiers.
T1_RAW = "{in:openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz}"  # smoke
T1_PREP = "{in:" + F + "/anat/sub-01_desc-preproc_T1w.nii.gz}"  # standard
T1_MASK = "{in:" + F + "/anat/sub-01_desc-brain_mask.nii.gz}"  # smoke
GM = "{in:" + F + "/anat/sub-01_label-GM_probseg.nii.gz}"  # standard
WM = "{in:" + F + "/anat/sub-01_label-WM_probseg.nii.gz}"  # standard
CSF = "{in:" + F + "/anat/sub-01_label-CSF_probseg.nii.gz}"  # standard
BOLDREF = "{in:" + FUNC + "_boldref.nii.gz}"  # smoke
BOLD_MASK = "{in:" + FUNC + "_desc-brain_mask.nii.gz}"  # smoke
BOLD_4D = "{in:" + FUNC + "_space-T1w_desc-preproc_bold.nii.gz}"  # standard (28 MB)
MNI_1MM = "{in:" + TPL + "_res-01_T1w.nii.gz}"  # standard
MNI_2MM = "{in:" + TPL + "_res-02_T1w.nii.gz}"  # smoke
MNI_2MM_MASK = "{in:" + TPL + "_res-02_desc-brain_mask.nii.gz}"  # smoke
MNI_BRAIN_PROB = "{in:" + TPL + "_res-01_label-brain_probseg.nii.gz}"  # standard

#: Generated inputs (`larmorx_validation.parity.ants_inputs`).
HEAD = "{gen:head-float32}"
HEAD_I16 = "{gen:head-int16-scaled}"
OUTLIERS = "{gen:head-outliers}"
FIRST_MAX = "{gen:head-first-voxel-max}"
POSINF = "{gen:head-posinf}"
OPERAND = "{gen:operand-zeros}"
LARGER = "{gen:operand-larger}"
CONSTANT = "{gen:constant}"
INT32 = "{gen:int32-large}"
SERIES = "{gen:series-4d}"
SLICE = "{gen:slice-2d}"
NEG_PIXDIM = "{gen:negative-pixdim-qform}"
LAS = "{gen:las-sform-only}"
MASK = "{gen:brain-uint8}"
FRACTIONAL = "{gen:fractional-float32}"
TISSUES = "{gen:tissues-int16}"
PROBSEG = "{gen:gm-probseg}"
COMPONENTS = "{gen:components-uint8}"
HOLES = "{gen:holes-uint8}"
THIN = "{gen:thin-3-slices}"
TINY = "{gen:tiny-4}"


def image_math(
    cid: str,
    category: str,
    description: str,
    op: str,
    *operands: str,
    dim: str = "3",
    **kw,
) -> AntsCase:
    """``ImageMath <dim> {out} <op> <operands...>``."""
    return AntsCase(cid, category, description, "ImageMath", (dim, "{out}", op, *operands), **kw)
