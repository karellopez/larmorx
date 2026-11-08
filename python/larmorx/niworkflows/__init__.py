# SPDX-License-Identifier: Apache-2.0
"""niworkflows' interfaces under sdcflows' and fMRIPrep's field-map steps, ported from
niworkflows 1.15.4 (the same on these paths in 1.15.2, fMRIPrep 25.2.6's) with the numpy, SciPy
and scikit-image arithmetic under them.

- :func:`intensity_clip`: ``IntensityClip`` (``_advanced_clip``): the intensities clipped at
  the percentiles of the median-filtered image's positive values and scaled to 0-255.
- :func:`intra_modal_merge`: ``IntraModalMerge``: images of one modality merged into a series
  and averaged (with :func:`reorient` and nibabel's :func:`as_closest_canonical`).
- :func:`robust_average`, :func:`merge_series`, :func:`reorient_file`: ``RobustAverage`` (with
  AFNI's ``3dvolreg``), ``MergeSeries``, ``ReorientImage``.
- :func:`copy_xform`, :func:`copy_header`, :func:`apply_mask`, :func:`key_select`:
  ``_copyxform`` (``FixHeaderApplyTransforms``' and ``FixHeaderRegistration``'s header fix),
  ``CopyHeader``, ``ApplyMask``, ``KeySelect``.
- :func:`skullstrip_bold_workflow`: ``init_skullstrip_bold_wf`` (FSL's ``bet``, AFNI's
  ``3dAutomask`` and ``fslmaths`` with nipype's command lines).
"""

from larmorx.niworkflows.header import (
    SkullstripBoldResult,
    apply_mask,
    copy_header,
    copy_xform,
    key_select,
    skullstrip_bold_workflow,
)
from larmorx.niworkflows.images import (
    IntraModalMergeResult,
    RobustAverageResult,
    as_closest_canonical,
    as_reoriented,
    four_to_three,
    intra_modal_merge,
    reorient,
    robust_average,
    squeeze_image,
)
from larmorx.niworkflows.nibabel import (
    IntensityClipResult,
    aff2axcodes,
    axcodes2ornt,
    intensity_clip,
    merge_series,
    ornt_transform,
    reorient_file,
    reorient_image,
)

__all__ = [
    "IntensityClipResult",
    "IntraModalMergeResult",
    "RobustAverageResult",
    "SkullstripBoldResult",
    "aff2axcodes",
    "apply_mask",
    "as_closest_canonical",
    "as_reoriented",
    "axcodes2ornt",
    "copy_header",
    "copy_xform",
    "four_to_three",
    "intensity_clip",
    "intra_modal_merge",
    "key_select",
    "merge_series",
    "ornt_transform",
    "reorient",
    "reorient_file",
    "reorient_image",
    "robust_average",
    "skullstrip_bold_workflow",
    "squeeze_image",
]
