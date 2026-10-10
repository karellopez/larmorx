# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants ImageMath`` with ANTs 2.6.5's ``ImageMath``.

The cases come from one module per operation group, listed in :data:`GROUPS`; each defines
``CASES``, a list of :class:`~larmorx_validation.parity.ants_programs.AntsCase`. To add a
group, write ``<group>.py`` and add it to :data:`GROUPS`.
"""

from __future__ import annotations

import importlib

from larmorx_validation.parity.ants_programs import AntsCase, make_suite

#: The case modules, one per ImageMath operation group.
GROUPS = ("dispatch", "arithmetic", "intensity", "gaussian", "morphology")


def all_cases() -> list[AntsCase]:
    out: list[AntsCase] = []
    for group in GROUPS:
        out += importlib.import_module(f"{__name__}.{group}").CASES
    return out


def suite():
    return make_suite(
        "ants-image-math",
        "ImageMath",
        "`larmorx ants ImageMath` / `lx.ants.image_math` and the typed `lx.ants` wrappers "
        "(crates larmorx-ants, larmorx-image)",
        all_cases,
        notes="Real images come from `larmorx-testdata` (OpenNeuro ds000005, its fMRIPrep "
        "derivatives, TemplateFlow).",
    )
