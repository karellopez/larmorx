# SPDX-License-Identifier: Apache-2.0
"""Benchmarks of ANTs' morphology and mask operations (ImageMath ``MD``, ``ME``, ``MC``,
``GD``, ``GO``, ``FillHoles``, ``PadImage``), as sMRIPrep's brain extraction runs them on
masks: larmorx against ANTs 2.6.5's own binaries, with the harness of
:mod:`larmorx_validation.bench.ants_programs` (same timing rules)."""

from __future__ import annotations

import argparse

from larmorx_validation.bench.ants_programs import Job, run_suite
from larmorx_validation.parity.ants_image_math.common import T1_MASK, T1_RAW

#: The benchmark jobs; add new ones here.
JOBS: list[Job] = [
    Job(
        "ImageMath MD 2 (sMRIPrep), T1w brain mask ds000005 (160×192×192)",
        "ImageMath",
        ("3", "{out}", "MD", T1_MASK, "2"),
    ),
    Job(
        "ImageMath MD 5 (sMRIPrep), T1w brain mask",
        "ImageMath",
        ("3", "{out}", "MD", T1_MASK, "5"),
    ),
    Job(
        "ImageMath ME 2 (sMRIPrep), T1w brain mask",
        "ImageMath",
        ("3", "{out}", "ME", T1_MASK, "2"),
        quick=False,
    ),
    Job(
        "ImageMath ME 10 (sMRIPrep's CSF step), T1w brain mask",
        "ImageMath",
        ("3", "{out}", "ME", T1_MASK, "10"),
    ),
    Job(
        "ImageMath MC 4, T1w brain mask",
        "ImageMath",
        ("3", "{out}", "MC", T1_MASK, "4"),
        quick=False,
    ),
    Job(
        "ImageMath FillHoles 2 (sMRIPrep), T1w brain mask",
        "ImageMath",
        ("3", "{out}", "FillHoles", T1_MASK, "2"),
    ),
    Job(
        "ImageMath PadImage 10 (sMRIPrep), T1w brain mask",
        "ImageMath",
        ("3", "{out}", "PadImage", T1_MASK, "10"),
    ),
    Job(
        "ImageMath GD 2 (grayscale dilation), raw T1w (160×192×192 int16)",
        "ImageMath",
        ("3", "{out}", "GD", T1_RAW, "2"),
    ),
    Job(
        "ImageMath GO 2 (grayscale opening), raw T1w",
        "ImageMath",
        ("3", "{out}", "GO", T1_RAW, "2"),
        quick=False,
    ),
]

TITLE = "ANTs morphology and mask operations (ImageMath MD, ME, MC, GD, GO, FillHoles, PadImage)"

NOTES = [
    "- **Binary morphology** (`MD`, `ME`, `MC`): ITK traces the object's border and paints "
    "the ball along it, so its time grows with the border and the ball (`ME 10` paints a "
    "ball of 4,945 voxels). larmorx computes the same voxel sets from an exact integer "
    "distance transform, whose cost does not depend on the radius, in parallel over lines.",
    "- **FillHoles**: ANTs computes a Danielsson distance map (sequential) only to find the "
    "background, which larmorx takes directly; the connected components are run-length "
    "encoded in both.",
    "- **Grayscale morphology** (`GD`, `GO`): ITK scans the ball or keeps a moving "
    "histogram per voxel; larmorx splits the ball into runs along x and reuses the running "
    "maximum of each run width for every row.",
    "- **PadImage** is a copy: reading the gzipped input and writing the output dominate.",
]


def main(args: argparse.Namespace) -> int:
    return run_suite(args, "ants-morphology", JOBS, TITLE, NOTES)
