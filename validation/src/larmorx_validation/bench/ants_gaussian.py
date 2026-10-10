# SPDX-License-Identifier: Apache-2.0
"""Benchmarks of ANTs' Gaussian filters (ImageMath ``Laplacian``, ``G``, ``Grad``,
``SmoothImage``, ``ResampleImageBySpacing``): larmorx against ANTs 2.6.5's own binaries, with
the harness of :mod:`larmorx_validation.bench.ants_programs` (same timing rules)."""

from __future__ import annotations

import argparse

from larmorx_validation.bench.ants_programs import Job, run_suite
from larmorx_validation.parity.ants_image_math.common import (
    BOLD_4D,
    BOLDREF,
    MNI_1MM,
    T1_RAW,
)

#: The benchmark jobs; add new ones here.
JOBS: list[Job] = [
    Job(
        "ImageMath Laplacian 1.5 1 (fMRIPrep's call), raw T1w ds000005 (160×192×192 int16)",
        "ImageMath",
        ("3", "{out}", "Laplacian", T1_RAW, "1.5", "1"),
    ),
    Job(
        "ImageMath Laplacian 1.5 1 (fMRIPrep's call), boldref ds000005 (64×64×34)",
        "ImageMath",
        ("3", "{out}", "Laplacian", BOLDREF, "1.5", "1"),
    ),
    Job(
        "ImageMath Laplacian 1.5 1, MNI152NLin2009cAsym 1 mm (193×229×193)",
        "ImageMath",
        ("3", "{out}", "Laplacian", MNI_1MM, "1.5", "1"),
        quick=False,
    ),
    Job(
        "ImageMath Grad 1, raw T1w",
        "ImageMath",
        ("3", "{out}", "Grad", T1_RAW, "1"),
        quick=False,
    ),
    Job(
        "ImageMath G 2 (discrete Gaussian, sigma 2 mm), raw T1w",
        "ImageMath",
        ("3", "{out}", "G", T1_RAW, "2"),
    ),
    Job(
        "SmoothImage 3 sigma 1 voxel, raw T1w",
        "SmoothImage",
        ("3", T1_RAW, "1", "{out}"),
    ),
    Job(
        "SmoothImage 3 median radius 1, raw T1w",
        "SmoothImage",
        ("3", T1_RAW, "1", "{out}", "0", "1"),
        quick=False,
    ),
    Job(
        "SmoothImage 4 sigma 1 voxel, fMRIPrep BOLD series (4D, 28 MB gzipped)",
        "SmoothImage",
        ("4", BOLD_4D, "1", "{out}"),
        quick=False,
    ),
    Job(
        "ResampleImageBySpacing 3 to 2 mm (smoothed, linear), raw T1w",
        "ResampleImageBySpacing",
        ("3", T1_RAW, "{out}", "2", "2", "2"),
    ),
    Job(
        "ResampleImageBySpacing 3 to 3 mm, MNI 1 mm",
        "ResampleImageBySpacing",
        ("3", MNI_1MM, "{out}", "3", "3", "3"),
        quick=False,
    ),
]

TITLE = "ANTs Gaussian filters (ImageMath Laplacian, G, Grad; SmoothImage; ResampleImageBySpacing)"

NOTES = [
    "- **Where the time goes.** Reading the gzipped input is sequential in both programs "
    "(zlib-rs in larmorx, zlib in ITK): about 50 ms of larmorx's time for the raw T1w, which "
    "is why 4 and 12 threads differ little for the faster filters. The recursive filters cost "
    "a few multiply-adds per voxel and pass; the Laplacian and the gradient magnitude make "
    "nine passes over a 3D image, `SmoothImage` three.",
    "- **How larmorx runs the passes.** Every pass filters 32 neighbouring lines together, "
    "laid out so that the 32 recursions advance side by side on contiguous values (lines "
    "along the first axis are transposed into that layout). Each line's arithmetic is "
    "exactly ITK's, so the result does not depend on the thread count or on the grouping. "
    "The resampler computes the part of the linear interpolation that is constant along an "
    "output row once per row when the row runs along an input axis (axis-aligned images).",
    "- **Threads.** ITK splits every pass into regions per thread, so ANTs gains 3–5× from 12 "
    "threads. larmorx gains less from many threads on these images: reading the input stays "
    "sequential, and a pass over a 6-million-voxel image is short.",
]


def main(args: argparse.Namespace) -> int:
    return run_suite(args, "ants-gaussian", JOBS, TITLE, NOTES)
