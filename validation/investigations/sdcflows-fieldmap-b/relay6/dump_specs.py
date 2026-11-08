# SPDX-License-Identifier: Apache-2.0
"""Dump nipype 1.12.0's input specs (argstr traits) of the interfaces part B runs."""

import nipype
from nipype.interfaces import afni, fsl
from nipype.interfaces.ants.segmentation import DenoiseImage, N4BiasFieldCorrection
from nipype.interfaces.ants.utils import ImageMath
from niworkflows.interfaces.fixes import FixHeaderApplyTransforms, FixHeaderRegistration

print("nipype", nipype.__version__)
for cls in (
    FixHeaderRegistration,
    FixHeaderApplyTransforms,
    N4BiasFieldCorrection,
    ImageMath,
    DenoiseImage,
    fsl.BET,
    fsl.MultiImageMaths,
    fsl.ApplyMask,
    afni.Automask,
    afni.Volreg,
    fsl.TOPUP,
):
    spec = cls.input_spec()
    print(f"== {cls.__module__}.{cls.__name__} cmd={getattr(cls, '_cmd', None)}")
    for name, t in sorted(spec.traits(argstr=lambda a: a is not None).items()):
        md = {
            k: getattr(t, k)
            for k in (
                "argstr",
                "position",
                "sep",
                "usedefault",
                "genfile",
                "name_source",
                "name_template",
                "keep_extension",
                "xor",
                "requires",
                "hash_files",
            )
            if getattr(t, k, None) is not None
        }
        default = t.default if getattr(t, "usedefault", False) else None
        print(f"  {name}: {t.trait_type.__class__.__name__} default={default!r} {md}")
