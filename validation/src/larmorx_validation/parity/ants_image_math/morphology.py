# SPDX-License-Identifier: Apache-2.0
"""ImageMath's morphology and mask operations: ``MD``, ``ME``, ``MO``, ``MC`` (ITK's binary
filters with a ball), ``GD``, ``GE``, ``GO``, ``GC`` (grayscale), ``FillHoles`` and
``PadImage``. sMRIPrep's brain extraction runs ``MD 2/4/5``, ``ME 10/2/5``, ``FillHoles 2`` and
``PadImage ±10`` on masks."""

from __future__ import annotations

from larmorx_validation.parity.ants_image_math.common import (
    BOLD_MASK,
    COMPONENTS,
    FRACTIONAL,
    HEAD,
    HEAD_I16,
    HOLES,
    LAS,
    MASK,
    MNI_2MM_MASK,
    NEG_PIXDIM,
    OUTLIERS,
    SERIES,
    SLICE,
    T1_MASK,
    T1_RAW,
    TISSUES,
    image_math,
)
from larmorx_validation.parity.ants_programs import AntsCase

B, G, F, P = "binary-morphology", "grayscale-morphology", "fill-holes", "pad-image"

HOLE_FIRST_SLICE = "{gen:hole-first-slice-uint8}"
HOLES_4D = "{gen:holes-labels-4d}"
SIGNED_ZEROS = "{gen:signed-zeros-float32}"

#: Why the grayscale cases with signed zeros may differ in the sign of zero.
SIGNED_ZERO_REASON = (
    "ITK's moving-histogram dilation keeps -0.0 and +0.0 under one std::map key, the one "
    "inserted first, so the sign of a zero maximum depends on the order the histogram was "
    "filled in (and on ITK's thread split); larmorx returns +0.0 for a zero maximum and -0.0 "
    "for a zero minimum. Only the sign of zero differs (0 ulp)."
)

CASES: list[AntsCase] = []


def add(*args, **kw):
    CASES.append(image_math(*args, **kw))


# MD: BinaryDilateImageFilter with BinaryBallStructuringElement, foreground = value (default 1).
add(f"{B}/MD-components", B, "MD 2 on components of 1 to 343 voxels", "MD", COMPONENTS, "2")
add(f"{B}/MD-default-radius", B, "MD without a radius: 1", "MD", COMPONENTS)
add(f"{B}/MD-radius-0", B, "MD 0: the ball is the centre voxel only", "MD", COMPONENTS, "0")
add(
    f"{B}/MD-radius-fraction",
    B,
    "MD 2.9: static_cast<unsigned long> truncates to 2",
    "MD",
    COMPONENTS,
    "2.9",
)
add(
    f"{B}/MD-radius-minus-half",
    B,
    "MD -0.5: truncates to 0",
    "MD",
    COMPONENTS,
    "-0.5",
)
add(
    f"{B}/MD-radius-negative",
    B,
    "MD -1: a radius near 2^64; ANTs aborts (std::bad_alloc), larmorx refuses",
    "MD",
    COMPONENTS,
    "-1",
    expect="error",
)
add(
    f"{B}/MD-labels-value-2",
    B,
    "MD 2 2: label 2 of a 3-tissue map dilates",
    "MD",
    TISSUES,
    "2",
    "2",
)
add(
    f"{B}/MD-value-0",
    B,
    "MD 1 0: the background (0) is the foreground and grows",
    "MD",
    COMPONENTS,
    "1",
    "0",
)
add(
    f"{B}/MD-fractional-value",
    B,
    "MD 1 0.6 on a mask with values 0.3, 0.6, 0.999, 1, 1.7, 2",
    "MD",
    FRACTIONAL,
    "1",
    "0.6",
)
add(
    f"{B}/MD-real-T1w-mask-2", B, "MD 2 on fMRIPrep's T1w brain mask (sMRIPrep)", "MD", T1_MASK, "2"
)
add(
    f"{B}/MD-real-T1w-mask-5",
    B,
    "MD 5 on fMRIPrep's T1w brain mask (sMRIPrep's superstep 7)",
    "MD",
    T1_MASK,
    "5",
    tier="standard",
)
add(f"{B}/MD-real-bold-mask", B, "MD 4 on a BOLD brain mask", "MD", BOLD_MASK, "4")
add(
    f"{B}/MD-dim2",
    B,
    "ImageMath 2: a 2D ball (radius 1 is the full 3 x 3)",
    "MD",
    SLICE,
    "1",
    "0",
    dim="2",
)
add(f"{B}/MD-dim4", B, "ImageMath 4: a 4D ball, across time too", "MD", HOLES_4D, "1", dim="4")

# ME: BinaryErodeImageFilter (the border does not erode), then 1 where eroded > 0.5 and input > 0.5.
add(f"{B}/ME-components", B, "ME 1 on components (the border one stays)", "ME", COMPONENTS, "1")
add(f"{B}/ME-holes", B, "ME 2 around holes", "ME", HOLES, "2")
add(
    f"{B}/ME-labels",
    B,
    "ME 1 on a 3-tissue map: labels 2 and 3 are not the foreground but above 0.5, so they stay 1",
    "ME",
    TISSUES,
    "1",
)
add(f"{B}/ME-value-2", B, "ME 2 2: label 2 erodes, the output is 0/1", "ME", TISSUES, "2", "2")
add(f"{B}/ME-fractional", B, "ME 1 on a fractional mask", "ME", FRACTIONAL, "1")
add(f"{B}/ME-brain-mask", B, "ME 3 on a brain mask", "ME", MASK, "3")
add(
    f"{B}/ME-real-T1w-mask-10",
    B,
    "ME 10 on fMRIPrep's T1w brain mask (sMRIPrep's CSF step)",
    "ME",
    T1_MASK,
    "10",
)
add(
    f"{B}/ME-real-T1w-mask-2",
    B,
    "ME 2 on fMRIPrep's T1w brain mask (sMRIPrep's superstep 7)",
    "ME",
    T1_MASK,
    "2",
    tier="standard",
)
add(f"{B}/ME-real-mni-mask", B, "ME 5 on the MNI 2 mm brain mask", "ME", MNI_2MM_MASK, "5")
add(f"{B}/ME-dim2", B, "ImageMath 2 on a 2D image", "ME", SLICE, "1", "0", dim="2")
add(f"{B}/ME-dim4", B, "ImageMath 4: eroded across time too", "ME", HOLES_4D, "1", dim="4")

# MO and MC.
add(f"{B}/MO-components", B, "MO 1: objects smaller than the ball vanish", "MO", COMPONENTS, "1")
add(f"{B}/MO-holes", B, "MO 2 around holes", "MO", HOLES, "2")
add(
    f"{B}/MO-value-0",
    B,
    "MO 1 0: the opening's eroded voxels become 0, the foreground itself, so it dilates only",
    "MO",
    TISSUES,
    "1",
    "0",
)
add(f"{B}/MO-labels-value-3", B, "MO 1 3 on a 3-tissue map", "MO", TISSUES, "1", "3")
add(f"{B}/MO-dim4", B, "ImageMath 4 opening", "MO", HOLES_4D, "1", dim="4")
add(f"{B}/MC-holes-1", B, "MC 1 closes the one-voxel hole", "MC", HOLES, "1")
add(f"{B}/MC-holes-2", B, "MC 2 (SafeBorder: padded by the radius first)", "MC", HOLES, "2")
add(
    f"{B}/MC-value-0",
    B,
    "MC 1 0: the foreground is 0, so the pad value is FLT_MAX",
    "MC",
    TISSUES,
    "1",
    "0",
)
add(
    f"{B}/MC-labels-value-3",
    B,
    "MC 2 3: closing label 3 of a 3-tissue map",
    "MC",
    TISSUES,
    "2",
    "3",
)
add(f"{B}/MC-components-border", B, "MC 3 with an object on the border", "MC", COMPONENTS, "3")
add(
    f"{B}/MC-real-T1w-mask",
    B,
    "MC 4 on fMRIPrep's T1w brain mask",
    "MC",
    T1_MASK,
    "4",
    tier="standard",
)
add(f"{B}/MC-dim2", B, "ImageMath 2 closing", "MC", SLICE, "2", "0", dim="2")
add(f"{B}/MC-dim4", B, "ImageMath 4: holes closed across time", "MC", HOLES_4D, "1", dim="4")

# Grayscale: maximum / minimum over the ball; opening and closing pad by the radius.
for op, name in [("GD", "dilation"), ("GE", "erosion"), ("GO", "opening"), ("GC", "closing")]:
    add(f"{G}/{op}-phantom-1", G, f"{op} 1: grayscale {name} of the phantom", op, HEAD, "1")
    add(f"{G}/{op}-phantom-2", G, f"{op} 2", op, HEAD, "2")
    add(f"{G}/{op}-outliers", G, f"{op} 1 with hot and negative voxels", op, OUTLIERS, "1")
    add(f"{G}/{op}-dim2", G, f"ImageMath 2 {op} 2", op, SLICE, "2", dim="2")
    add(f"{G}/{op}-dim4", G, f"ImageMath 4 {op} 1 (across time too)", op, SERIES, "1", dim="4")
    add(
        f"{G}/{op}-real-T1w",
        G,
        f"{op} 2 on the raw T1w",
        op,
        T1_RAW,
        "2",
        tier="standard",
    )
add(f"{G}/GD-radius-0", G, "GD 0: the image unchanged", "GD", HEAD, "0")
add(f"{G}/GE-int16-scaled", G, "GE 1 on scaled int16 input", "GE", HEAD_I16, "1")
add(f"{G}/GO-radius-4", G, "GO 4 (a 9-voxel ball)", "GO", HEAD, "4")
add(f"{G}/GE-signed-zeros", G, "GE 1 with -0.0 and +0.0 side by side", "GE", SIGNED_ZEROS, "1")
add(
    f"{G}/GD-signed-zeros",
    G,
    "GD 1 with -0.0 and +0.0 side by side: the sign of a zero maximum",
    "GD",
    SIGNED_ZEROS,
    "1",
    tolerance_ulps=0,
    reason=SIGNED_ZERO_REASON,
)
add(
    f"{G}/GC-signed-zeros",
    G,
    "GC 1 with -0.0 and +0.0 side by side",
    "GC",
    SIGNED_ZEROS,
    "1",
    tolerance_ulps=0,
    reason=SIGNED_ZERO_REASON,
)

# FillHoles: the background's face-connected components other than the largest are holes.
add(f"{F}/default", F, "no parameter: 2, every hole filled", "FillHoles", HOLES)
add(f"{F}/param-2", F, "FillHoles 2, as sMRIPrep calls it", "FillHoles", HOLES, "2")
add(
    f"{F}/param-almost-2",
    F,
    "2.0000001 is within FloatAlmostEqual of 2: every hole",
    "FillHoles",
    HOLES,
    "2.0000001",
)
add(
    f"{F}/param-1.5",
    F,
    "between 1 and 2: every hole (the ratio is not computed)",
    "FillHoles",
    HOLES,
    "1.5",
)
add(f"{F}/param-3", F, "above 2: no hole", "FillHoles", HOLES, "3")
add(
    f"{F}/param-1",
    F,
    "1: holes whose object-edge ratio exceeds 1 (none can)",
    "FillHoles",
    HOLES,
    "1",
)
add(
    f"{F}/param-0.99",
    F,
    "0.99: holes bounded by the object only (the border hole's out-of-image neighbours wrap "
    "to other rows)",
    "FillHoles",
    HOLES,
    "0.99",
)
add(f"{F}/param-0.5", F, "0.5", "FillHoles", HOLES, "0.5")
add(f"{F}/components-0.3", F, "0.3 on separate components", "FillHoles", COMPONENTS, "0.3")
add(f"{F}/labels", F, "a 3-tissue map: every label ≥ 0.5 is object", "FillHoles", TISSUES, "2")
add(f"{F}/fractional", F, "values 0.3 (background) and 0.6 (object)", "FillHoles", FRACTIONAL, "2")
add(f"{F}/real-T1w-mask", F, "fMRIPrep's T1w brain mask", "FillHoles", T1_MASK, "2")
add(f"{F}/real-mni-mask", F, "the MNI 2 mm brain mask", "FillHoles", MNI_2MM_MASK, "2")
add(f"{F}/dim2", F, "ImageMath 2: 2D components", "FillHoles", SLICE, "2", dim="2")
add(
    f"{F}/dim4",
    F,
    "ImageMath 4: 4D components, holes closed across time",
    "FillHoles",
    HOLES_4D,
    "2",
    dim="4",
)
add(
    f"{F}/hole-first-slice-2",
    F,
    "a pocket touching the first slice is a hole",
    "FillHoles",
    HOLE_FIRST_SLICE,
    "2",
)
add(
    f"{F}/hole-first-slice-0.5",
    F,
    "0.5 with a hole on the first slice: ANTs reads neighbours outside the image's memory",
    "FillHoles",
    HOLE_FIRST_SLICE,
    "0.5",
    expect="divergence",
    reason="For a hole on the first or last slice, ANTs reads the binary image at neighbour "
    "indices outside the image (NeighborhoodIterator::GetIndex is not clamped), which land "
    "before or after its buffer: the result depends on whatever memory lies there. larmorx "
    "reproduces reads that land inside the buffer and refuses the others.",
)

# PadImage.
add(f"{P}/pad-10", P, "PadImage 10, as sMRIPrep pads before Atropos", "PadImage", HEAD, "10")
add(f"{P}/depad-5", P, "PadImage -5 crops", "PadImage", HEAD, "-5")
add(f"{P}/pad-value", P, "PadImage 3 7: the new voxels are 7", "PadImage", HEAD, "3", "7")
add(
    f"{P}/pad-fraction",
    P,
    "PadImage 2.5: the size grows by 5 but the image shifts by 2",
    "PadImage",
    HEAD,
    "2.5",
)
add(f"{P}/depad-fraction", P, "PadImage -2.5", "PadImage", HEAD, "-2.5")
add(f"{P}/pad-0", P, "PadImage 0: a copy (a new image without descrip)", "PadImage", HEAD, "0")
add(
    f"{P}/depad-to-one",
    P,
    "PadImage -13.5: one voxel on the shortest axis",
    "PadImage",
    HEAD,
    "-13.5",
)
add(
    f"{P}/depad-to-zero",
    P,
    "PadImage -14: an axis of 0 voxels; ANTs crashes (munmap_chunk), larmorx refuses",
    "PadImage",
    HEAD,
    "-14",
    expect="error",
)
add(
    f"{P}/depad-negative",
    P,
    "PadImage -20: a negative size wraps to 2^32 - 8; ANTs crashes, larmorx refuses",
    "PadImage",
    HEAD,
    "-20",
    expect="error",
)
add(
    f"{P}/no-pad",
    P,
    "no pad argument: ANTs reads argv[5] (null) and crashes",
    "PadImage",
    HEAD,
    expect="error",
)
add(f"{P}/int16-scaled", P, "scaled int16 input", "PadImage", HEAD_I16, "4")
add(
    f"{P}/las-sform-only",
    P,
    "an sform-only LAS image: the origin moves along -x",
    "PadImage",
    LAS,
    "3",
)
add(f"{P}/negative-pixdim", P, "a qform image with a negative pixdim", "PadImage", NEG_PIXDIM, "-3")
add(
    f"{P}/real-T1w-mask-pad", P, "fMRIPrep's T1w brain mask padded by 10", "PadImage", T1_MASK, "10"
)
add(
    f"{P}/real-T1w-depad",
    P,
    "the raw T1w cropped by 10 (sMRIPrep's de-padding)",
    "PadImage",
    T1_RAW,
    "-10",
    tier="standard",
)
add(f"{P}/dim2", P, "ImageMath 2 PadImage -3 1", "PadImage", SLICE, "-3", "1", dim="2")
add(f"{P}/dim4", P, "ImageMath 4: time is padded too", "PadImage", SERIES, "2", dim="4")
