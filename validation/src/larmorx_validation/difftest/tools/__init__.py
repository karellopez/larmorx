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
    "afni-volreg": ("afni_volreg", "TOOL"),
    "afni-volreg-original": ("afni_volreg_clean_room", "TOOL"),
    "mri-hmc": ("mri_hmc", "ORIGINAL"),
    "fsl-mcflirt": ("mri_hmc", "REPLICA"),
    "fsl-bet": ("fsl_bet", "TOOL"),
    "fsl-flirt": ("fsl_flirt", "TOOL"),
    "fsl-flirt-programs": ("fsl_flirt", "PROGRAMS"),
    "fsl-flirt-part3": ("fsl_flirt", "PART3"),
    "fsl-flirt-original": ("fsl_flirt", "ORIGINAL"),
    "fsl-fast": ("fsl_fast", "TOOL"),
    "fsl-fast-original": ("fsl_fast_clean_room", "TOOL"),
    "fsl-topup": ("fsl_topup", "TOOL"),
    "fsl-topup-original": ("fsl_topup_clean_room", "TOOL"),
    "fsl-maths": ("fsl_avwutils", "MATHS"),
    "fsl-stats": ("fsl_avwutils", "STATS_TOOL"),
    "fsl-maths-original": ("fsl_avwutils_clean_room", "MATHS"),
    "fsl-stats-original": ("fsl_avwutils_clean_room", "STATS_TOOL"),
    "fsl-prelude": ("fsl_prelude", "TOOL"),
    "fsl-prelude-original": ("fsl_prelude_clean_room", "TOOL"),
    "resample-series": ("resample_series", "TOOL"),
    "sdcflows-fieldmap": ("sdcflows_fieldmap", "TOOL"),
    "tedana-t2smap": ("tedana_t2smap", "TOOL"),
    "tedana-t2smap-original": ("tedana_t2smap_clean_room", "TOOL"),
    "ndimage-skimage": ("fieldmaps_b", "NDIMAGE"),
    "niworkflows-helpers": ("fieldmaps_b", "NIWORKFLOWS"),
    "sdcflows-estimation": ("fieldmaps_b", "ESTIMATION"),
}

#: Tools that test the harness itself: run only when named, never part of ``all``.
SELFTESTS: dict[str, tuple[str, str]] = {
    "selftest-memory": ("selftest", "MEMORY_HOG"),
}


def get(name: str) -> Tool:
    if name not in TOOLS and name not in SELFTESTS:
        raise KeyError(f"unknown tool {name!r}; choose from {', '.join(TOOLS)}")
    module, attr = TOOLS.get(name) or SELFTESTS[name]
    return getattr(importlib.import_module(f"{__name__}.{module}"), attr)


def names() -> list[str]:
    return list(TOOLS)


#: Modules renamed by the documentation revision (2026-11-12): ``<name>_original`` named a clean
#: room, ``<name>_clean_room`` now. The old names still import (``from larmorx_validation.difftest.tools import
#: <name>_original``), for the investigations that use them; history stays as written.
_RENAMED = {
    f"{m}_original": f"{m}_clean_room"
    for m in (
        "afni_volreg",
        "fsl_avwutils",
        "fsl_fast",
        "fsl_prelude",
        "fsl_topup",
        "tedana_t2smap",
    )
}


def __getattr__(name: str):
    if name in _RENAMED:
        import importlib

        return importlib.import_module(f"{__name__}.{_RENAMED[name]}")
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


__all__ = ["SELFTESTS", "TOOLS", "get", "names"]
