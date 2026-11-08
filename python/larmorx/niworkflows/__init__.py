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
- :func:`concatenate_xfms`: ``ConcatenateXFMs``, linear transform files combined into one and
  written as nitransforms writes ITK text files.
- :func:`skullstrip_bold_workflow`: ``init_skullstrip_bold_wf`` (FSL's ``bet``, AFNI's
  ``3dAutomask`` and ``fslmaths`` with nipype's command lines).
- :func:`validate_image`, :func:`nonsteady_states_detector` (with nipype's
  :func:`is_outlier`), :func:`binarize`, :func:`generate_sampling_reference` (with nilearn's
  nearest-neighbour :func:`resample_img_nearest`) and :func:`epi_reference_workflow`:
  ``ValidateImage``, ``NonsteadyStatesDetector``, ``Binarize``, ``GenerateSamplingReference``
  and ``init_epi_reference_wf``.
"""

from larmorx.niworkflows.bold import NonsteadyStatesResult, is_outlier, nonsteady_states_detector
from larmorx.niworkflows.header import (
    SkullstripBoldResult,
    ValidateImageResult,
    apply_mask,
    copy_header,
    copy_xform,
    key_select,
    skullstrip_bold_workflow,
    validate_image,
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
    BinarizeResult,
    IntensityClipResult,
    aff2axcodes,
    axcodes2ornt,
    binarize,
    intensity_clip,
    merge_series,
    ornt_transform,
    reorient_file,
    reorient_image,
)
from larmorx.niworkflows.nitransforms import ConcatenateXfmsResult, concatenate_xfms
from larmorx.niworkflows.refmap import EpiReferenceResult, epi_reference_workflow
from larmorx.niworkflows.sampling import (
    BoundingBoxError,
    generate_sampling_reference,
    resample_img_nearest,
)

__all__ = [
    "BinarizeResult",
    "BoundingBoxError",
    "ConcatenateXfmsResult",
    "EpiReferenceResult",
    "IntensityClipResult",
    "IntraModalMergeResult",
    "NonsteadyStatesResult",
    "RobustAverageResult",
    "SkullstripBoldResult",
    "ValidateImageResult",
    "aff2axcodes",
    "apply_mask",
    "as_closest_canonical",
    "as_reoriented",
    "axcodes2ornt",
    "binarize",
    "concatenate_xfms",
    "copy_header",
    "copy_xform",
    "epi_reference_workflow",
    "four_to_three",
    "generate_sampling_reference",
    "intensity_clip",
    "intra_modal_merge",
    "is_outlier",
    "key_select",
    "merge_series",
    "nonsteady_states_detector",
    "ornt_transform",
    "reorient",
    "reorient_file",
    "reorient_image",
    "resample_img_nearest",
    "robust_average",
    "skullstrip_bold_workflow",
    "squeeze_image",
    "validate_image",
]
