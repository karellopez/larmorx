# SPDX-License-Identifier: Apache-2.0
"""The descriptors of every tool under differential random testing.

Adding a tool is one short module with a :class:`~larmorx_validation.difftest.core.Tool`
(``TOOL``, or several named in :data:`TOOLS`), registered here: how to draw a case, which parity
suite's runner decides it, which implementations run.
"""

from __future__ import annotations

import importlib

from larmorx_validation.difftest.core import Tool

#: tool name -> (module in this package, attribute).
TOOLS: dict[str, tuple[str, str]] = {
    "nifti-io": ("nifti_io", "TOOL"),
    "itk-transforms": ("itk_transforms", "TOOL"),
    "ants-apply-transforms": ("ants_apply_transforms", "TOOL"),
    "ants-image-math": ("ants_image_math", "TOOL"),
    "ants-threshold-image": ("ants_filters", "THRESHOLD"),
    "ants-multiply-images": ("ants_filters", "MULTIPLY"),
    "ants-smooth-image": ("ants_filters", "SMOOTH"),
    "ants-resample-image": ("ants_filters", "RESAMPLE"),
    "ants-resample-image-by-spacing": ("ants_filters", "BY_SPACING"),
    "ants-n4": ("ants_threaded", "N4"),
    "ants-denoise": ("ants_threaded", "DENOISE"),
    "ants-registration": ("ants_registration", "TOOL"),
    "ants-ai": ("ants_ai", "AI"),
    "ants-affine-initializer": ("ants_ai", "AFFINE_INITIALIZER"),
    "ants-atropos": ("ants_atropos", "TOOL"),
    "afni-tshift": ("afni_tshift", "TOOL"),
    "afni-automask": ("afni_masks", "AUTOMASK"),
    "afni-unifize": ("afni_masks", "UNIFIZE"),
    "mri-hmc": ("mri_hmc", "ORIGINAL"),
    "fsl-mcflirt": ("mri_hmc", "REPLICA"),
    "resample-series": ("resample_series", "TOOL"),
}


def get(name: str) -> Tool:
    if name not in TOOLS:
        raise KeyError(f"unknown tool {name!r}; choose from {', '.join(TOOLS)}")
    module, attr = TOOLS[name]
    return getattr(importlib.import_module(f"{__name__}.{module}"), attr)


def names() -> list[str]:
    return list(TOOLS)


__all__ = ["TOOLS", "get", "names"]
