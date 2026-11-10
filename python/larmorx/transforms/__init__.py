# SPDX-License-Identifier: Apache-2.0
"""Transforms: ``lx.transforms``.

- **ITK transform files**, kept in ITK's stored form (:class:`ItkTransform`, :func:`read`,
  :func:`write`), points in LPS as ANTs uses them.
- **Transform chains as fMRIPrep loads them** (:func:`load_transforms`, :class:`Affine`,
  :class:`AffineSeries`, :class:`DenseField`, :class:`TransformChain`): RAS+ points,
  nitransforms' conventions.
- **fMRIPrep's one-shot BOLD resampler** (:func:`resample_series`): head motion,
  susceptibility distortion and the transforms to the target applied in one interpolation.
- **fMRIPrep's field-map steps** (:func:`reconstruct_fieldmap`,
  :func:`reconstruct_fieldmap_node`, :func:`distortion_parameters`): the field map from its
  B-spline coefficients on a target grid, and a BOLD run's readout time and phase-encoding
  direction.
"""

from larmorx.transforms.chain import (
    Affine,
    AffineSeries,
    DenseField,
    Transform,
    TransformChain,
    load_itk_composite,
    load_itk_linear,
    load_transforms,
)
from larmorx.transforms.fieldmap import (
    FIELDMAP_INTENT_NAME,
    NOT_ALIGNED_REFERENCE,
    DistortionParametersResult,
    ReconstructFieldmapResult,
    aligned,
    as_affine,
    distortion_parameters,
    reconstruct_fieldmap,
    reconstruct_fieldmap_node,
)
from larmorx.transforms.itk import Hdf5WriteOptions, ItkTransform, _read_parts, read, write
from larmorx.transforms.resample import (
    PE_DIRECTIONS,
    ensure_positive_cosines,
    resample_series,
)

__all__ = [
    "FIELDMAP_INTENT_NAME",
    "NOT_ALIGNED_REFERENCE",
    "PE_DIRECTIONS",
    "Affine",
    "AffineSeries",
    "DenseField",
    "DistortionParametersResult",
    "Hdf5WriteOptions",
    "ItkTransform",
    "ReconstructFieldmapResult",
    "Transform",
    "TransformChain",
    "_read_parts",
    "aligned",
    "as_affine",
    "distortion_parameters",
    "ensure_positive_cosines",
    "load_itk_composite",
    "load_itk_linear",
    "load_transforms",
    "read",
    "reconstruct_fieldmap",
    "reconstruct_fieldmap_node",
    "resample_series",
    "write",
]
