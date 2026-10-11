# SPDX-License-Identifier: Apache-2.0
"""Benchmarks of ``N4BiasFieldCorrection``: larmorx against ANTs 2.6.5's own binary, on the
command lines fMRIPrep, sMRIPrep and sdcflows give it, with the harness of
:mod:`larmorx_validation.bench.ants_programs` (same timing rules). larmorx's output is compared
with ANTs' at one thread."""

from __future__ import annotations

import argparse

from larmorx_validation.bench.ants_programs import Job, run_suite
from larmorx_validation.parity.ants_image_math.common import BOLDREF, T1_RAW, WM

# Importing the parity suite registers its input builders (``{gen:n4-...}``).
from larmorx_validation.parity.ants_n4 import (
    MAG_DS003012,
    MNI_BOLDREF,
    MNI_PREMASK,
    PREMASK,
    T1_TRUNC,
)

B200 = ("--bspline-fitting", "[", "200", "]")
C4 = ("--convergence", "[", "50x50x50x50,", "1e-07", "]")
C5 = ("--convergence", "[", "50x50x50x50x50,", "1e-07", "]")

#: The benchmark jobs; add new ones here.
JOBS: list[Job] = [
    Job(
        "fMRIPrep's BOLD reference: -b [200] -r -w premask, boldref ds000005 (64×64×34)",
        "N4BiasFieldCorrection",
        ("-d", "3", *B200, "-i", BOLDREF, "-o", "{out}", "-r", "--weight-image", PREMASK),
    ),
    Job(
        "fMRIPrep's BOLD reference, MNI152NLin2009cAsym boldref res-02 (97×115×97)",
        "N4BiasFieldCorrection",
        ("-d", "3", *B200, "-i", MNI_BOLDREF, "-o", "{out}", "-r", "--weight-image", MNI_PREMASK),
        quick=False,
    ),
    Job(
        "sMRIPrep before the template: ANTs' defaults, raw T1w ds000005 (160×192×192)",
        "N4BiasFieldCorrection",
        ("-d", "3", "--input-image", T1_RAW, "--output", "{out}"),
    ),
    Job(
        "sMRIPrep brain extraction, first N4: -b [200] -c [50x50x50x50,1e-7] -s 4, truncated T1w",
        "N4BiasFieldCorrection",
        ("-d", "3", *B200, *C4, "--input-image", T1_TRUNC, "--output", "{out}", "-s", "4"),
        quick=False,
    ),
    Job(
        "sMRIPrep brain extraction, final N4: -b [200] -c [50x50x50x50x50,1e-7] -s 4 -r -w WM, "
        "raw T1w",
        "N4BiasFieldCorrection",
        (
            "-d",
            "3",
            *B200,
            *C5,
            "--input-image",
            T1_RAW,
            "--output",
            "{out}",
            "-r",
            "--shrink-factor",
            "4",
            "--weight-image",
            WM,
        ),
    ),
    Job(
        "sdcflows' magnitude: -c [50x50x50x50x50,1e-7] -s 4, ds003012 magnitude1",
        "N4BiasFieldCorrection",
        ("-d", "3", *C5, "--input-image", MAG_DS003012, "--output", "{out}", "-s", "4"),
        quick=False,
    ),
]

TITLE = "N4BiasFieldCorrection"

NOTES = [
    "- **Same result.** ANTs' own result changes with its thread count (ITK's B-spline fit "
    "sums per-thread lattices); larmorx gives ANTs' one-thread result on any number of "
    "threads. Where the table says values differ from ANTs at one thread, the cause is "
    "glibc's `expf`/`logf`/`exp`, which are not always correctly rounded: the same ANTs "
    "binary with those functions replaced by CORE-MATH's through `LD_PRELOAD` gives "
    "larmorx's bits ([validation record](../validation/ants-n4.md)).",
    "- **Where the time goes.** Each iteration sharpens the histogram (seven FFTs of 512 "
    "points), fits a B-spline to the residual at every voxel of the shrunk image, and "
    "evaluates the field on the shrunk grid; the default runs 200 iterations. ITK builds a "
    "point set and recomputes every point's span index and kernel weights at each fit. The "
    "points do not move within a level, so larmorx computes the weights once per level and "
    "reuses them in every iteration (the same arithmetic).",
    "- **Threads.** larmorx splits the fit by blocks of control points, the evaluation by "
    "planes and the histogram mapping by voxels; the histogram and the FFTs are sequential. "
    "Reading the gzipped input and the full-resolution field and division at the end are a "
    "fixed cost.",
]


def main(args: argparse.Namespace) -> int:
    return run_suite(args, "ants-n4", JOBS, TITLE, NOTES)
