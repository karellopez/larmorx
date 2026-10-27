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
GROUPS = ("dispatch", "arithmetic", "intensity", "gaussian", "morphology", "components")


def all_cases() -> list[AntsCase]:
    out: list[AntsCase] = []
    for group in GROUPS:
        out += importlib.import_module(f"{__name__}.{group}").CASES
    return out


def panel_cases() -> list[AntsCase]:
    """The data panel's tier: every ImageMath node of the fourteen fMRIPrep oracle runs, and
    the operations fMRIPrep's pipeline uses, on panel images it never gives them."""
    from larmorx_validation.parity import fmriprep_nodes
    from larmorx_validation.parity import panel_images as P

    def im(cid, description, op, image, *operands, dim="3", images=()):
        return AntsCase(
            P.PREFIX + cid,
            P.CATEGORY,
            P.describe(description, *(images or (image,))),
            "ImageMath",
            (dim, "{out}", op, image.arg, *operands),
            tier="panel",
        )

    trunc = ("0.01", "0.999", "256")
    return [
        *fmriprep_nodes.ants_cases("ImageMath"),
        im(
            "truncate/7t-mp2rage",
            "sMRIPrep's TruncateImageIntensity 0.01 0.999 256 on the raw 7 T MP2RAGE: a noisy "
            "background as wide as the head's values",
            "TruncateImageIntensity",
            P.MP2RAGE_7T,
            *trunc,
        ),
        im(
            "truncate/flair",
            "TruncateImageIntensity 0.01 0.999 256 on a FLAIR of 0.42 mm in-plane voxels",
            "TruncateImageIntensity",
            P.FLAIR_OLDER,
            *trunc,
        ),
        im(
            "truncate/1.5t-float32",
            "TruncateImageIntensity 0.01 0.999 256 on a float32 MPRAGE stored uncompressed",
            "TruncateImageIntensity",
            P.MPRAGE_15T,
            *trunc,
        ),
        im(
            "laplacian/tumour-t2w",
            "sMRIPrep's Laplacian 1.5 1 on a T2w of 6 mm slices",
            "Laplacian",
            P.TUMOUR_T2W,
            "1.5",
            "1",
        ),
        im(
            "laplacian/t2w-older",
            "sMRIPrep's Laplacian 1.5 1 on an older adult's T2w",
            "Laplacian",
            P.T2W_OLDER,
            "1.5",
            "1",
        ),
        im(
            "getlargestcomponent/stroke-lesion",
            "GetLargestComponent (sMRIPrep's brain-mask cleaning) on a manual stroke mask",
            "GetLargestComponent",
            P.STROKE_LESION,
        ),
        im(
            "me/stroke-lesion",
            "ME 2 (sMRIPrep's erosion) on a manual stroke mask",
            "ME",
            P.STROKE_LESION,
            "2",
        ),
        im(
            "fillholes/stroke-lesion",
            "FillHoles 2 on a manual stroke mask",
            "FillHoles",
            P.STROKE_LESION,
            "2",
        ),
        im(
            "m/bold-mb8",
            "m 2 on a 600-volume multiband-8 series (uint16 values times 2): ImageMath 4 on the "
            "panel's largest series",
            "m",
            P.BOLD_MB8,
            "2",
            dim="4",
        ),
    ]


def suite():
    return make_suite(
        "ants-image-math",
        "ImageMath",
        "`larmorx ants ImageMath` / `lx.ants.image_math` and the typed `lx.ants` wrappers "
        "(crates larmorx-ants, larmorx-image)",
        all_cases,
        notes="Real images come from `larmorx-testdata` (OpenNeuro ds000005, its fMRIPrep "
        "derivatives, TemplateFlow); the panel tier's from the data panel "
        "(`larmorx_testdata.panel()`) and the fMRIPrep 25.2.6 oracle runs.",
        panel=panel_cases,
    )
