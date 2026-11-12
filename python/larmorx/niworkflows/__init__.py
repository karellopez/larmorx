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
- :func:`regrid_to_zooms` (with :func:`resample_by_spacing`): ``RegridToZooms``, an image on a
  grid of new voxel sizes over the same field of view.
- :func:`derivatives_data_sink`, :func:`copy_any`: ``DerivativesDataSink`` (a derivative named
  by niworkflows' ``nipreps.json`` patterns, its header fixed, its JSON sidecar) and
  ``_copy_any``.
- :func:`collect_participants`: the participants of a BIDS dataset to process (with
  :class:`BIDSError` and :class:`BIDSWarning`).
- :func:`robust_acompcor`, :func:`robust_tcompcor`: ``RobustACompCor`` and ``RobustTCompCor``,
  nipype's ``ACompCor`` and ``TCompCor`` (:mod:`larmorx.confounds`) without niworkflows' retries
  (PIPE-48).
- :func:`add_tsv_header`, :func:`dict_merge`, :func:`tsv2json`: ``AddTSVHeader``,
  ``DictMerge`` and ``TSV2JSON`` (PIPE-61, PIPE-62; PD-01, PD-02).
- :mod:`larmorx.niworkflows.confounds`: ``ExpandModel`` (``parse_formula``), ``SpikeRegressors``
  and ``NormalizeMotionParams`` (PIPE-43, PIPE-64 to PIPE-70), importable from that module until
  their golden cases (the confounds' golden relay).
"""

from larmorx.niworkflows.bids import (
    DEFAULT_DTYPES,
    STANDARD_SPACES,
    DerivativesDataSinkResult,
    copy_any,
    derivatives_data_sink,
    get_tf_resolution,
)
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
from larmorx.niworkflows.participants import BIDSError, BIDSWarning, collect_participants

# robust_acompcor and robust_tcompcor join __all__ (the golden registry's namespace) with their
# golden cases, in the confounds' golden relay; until then they are importable here.
from larmorx.niworkflows.patches import robust_acompcor, robust_tcompcor  # noqa: F401
from larmorx.niworkflows.refmap import EpiReferenceResult, epi_reference_workflow
from larmorx.niworkflows.regrid import RegridToZoomsResult, regrid_to_zooms, resample_by_spacing
from larmorx.niworkflows.sampling import (
    BoundingBoxError,
    generate_sampling_reference,
    resample_img_nearest,
)

# add_tsv_header, dict_merge and tsv2json join __all__ with their golden cases (the confounds'
# golden relay), as robust_acompcor and robust_tcompcor.
from larmorx.niworkflows.utility import add_tsv_header, dict_merge, tsv2json  # noqa: F401

__all__ = [
    "DEFAULT_DTYPES",
    "STANDARD_SPACES",
    "BIDSError",
    "BIDSWarning",
    "BinarizeResult",
    "BoundingBoxError",
    "ConcatenateXfmsResult",
    "DerivativesDataSinkResult",
    "EpiReferenceResult",
    "IntensityClipResult",
    "IntraModalMergeResult",
    "NonsteadyStatesResult",
    "RegridToZoomsResult",
    "RobustAverageResult",
    "SkullstripBoldResult",
    "ValidateImageResult",
    "aff2axcodes",
    "apply_mask",
    "as_closest_canonical",
    "as_reoriented",
    "axcodes2ornt",
    "binarize",
    "collect_participants",
    "concatenate_xfms",
    "copy_any",
    "copy_header",
    "copy_xform",
    "derivatives_data_sink",
    "epi_reference_workflow",
    "four_to_three",
    "generate_sampling_reference",
    "get_tf_resolution",
    "intensity_clip",
    "intra_modal_merge",
    "is_outlier",
    "key_select",
    "merge_series",
    "nonsteady_states_detector",
    "ornt_transform",
    "regrid_to_zooms",
    "reorient",
    "reorient_file",
    "reorient_image",
    "resample_by_spacing",
    "resample_img_nearest",
    "robust_average",
    "skullstrip_bold_workflow",
    "squeeze_image",
    "validate_image",
]
