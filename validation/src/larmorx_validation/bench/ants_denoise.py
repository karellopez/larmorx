# SPDX-License-Identifier: Apache-2.0
"""Benchmarks of ``DenoiseImage``: larmorx against ANTs 2.6.5's own binary, on sMRIPrep's
command line and the same options on other images, with the harness of
:mod:`larmorx_validation.bench.ants_programs` (same timing rules). larmorx's output is compared
with ANTs' at one thread."""

from __future__ import annotations

import argparse

from larmorx_validation.bench.ants_programs import Job, run_suite

# Importing the parity suite registers its input builders and names its catalog files.
from larmorx_validation.parity.ants_denoise import MAG_DS003012, T1_DS000210
from larmorx_validation.parity.ants_image_math.common import BOLDREF, MNI_2MM, T1_RAW

#: The benchmark jobs; add new ones here.
JOBS: list[Job] = [
    Job(
        "sMRIPrep: -n Rician -s 1, raw T1w ds000005 (160×192×192)",
        "DenoiseImage",
        ("-i", T1_RAW, "-n", "Rician", "-o", "{out}", "-s", "1"),
    ),
    Job(
        "sMRIPrep: -n Rician -s 1, T1w ds000210 (256×256×176)",
        "DenoiseImage",
        ("-i", T1_DS000210, "-n", "Rician", "-o", "{out}", "-s", "1"),
        quick=False,
    ),
    Job(
        "-n Rician, MNI152NLin2009cAsym T1w res-02 (97×115×97)",
        "DenoiseImage",
        ("-i", MNI_2MM, "-n", "Rician", "-o", "{out}", "-s", "1"),
        quick=False,
    ),
    Job(
        "-n Gaussian (ANTs' default model), raw T1w ds000005",
        "DenoiseImage",
        ("-d", "3", "-i", T1_RAW, "-o", "{out}"),
        quick=False,
    ),
    Job(
        "-n Rician, BOLD reference ds000005 (64×64×34)",
        "DenoiseImage",
        ("-i", BOLDREF, "-n", "Rician", "-o", "{out}", "-s", "1"),
    ),
    Job(
        "-n Rician, fieldmap magnitude ds003012 (64×64×36)",
        "DenoiseImage",
        ("-i", MAG_DS003012, "-n", "Rician", "-o", "{out}", "-s", "1"),
        quick=False,
    ),
]

TITLE = "DenoiseImage"

NOTES = [
    "- **Same result.** ANTs' own result changes with its thread count (ITK's threads add to "
    "shared output voxels and overwrite a shared bias image without ordering; with 12 threads "
    "it also changes between runs); larmorx gives ANTs' one-thread result on any number of "
    "threads. Where the table says values differ from ANTs at one thread, the cause is "
    "glibc's `expf`/`exp`, which are not always correctly rounded: the same ANTs binary with "
    "those functions replaced by CORE-MATH's through `LD_PRELOAD` gives larmorx's bits "
    "([validation record](../validation/ants-denoise.md)).",
    "- **Where the time goes.** For each voxel, every neighbour in the 5×5×5 search box that "
    "passes the mean and variance tests gets a patch distance (27 squared differences), an "
    "`expf` and a weighted sum of its 27 patch values: about 490 million such neighbours in a "
    "1 mm T1w. ITK reads every voxel through `GetPixel` with region tests; larmorx works on "
    "padded images and computes eight neighbouring centres side by side, keeping every sum "
    "in ITK's order.",
    "- **Threads.** larmorx's centres run in parallel by rows; the output voxels are then "
    "summed in parallel, each in the single thread's order. Reading the gzipped input, the "
    "local mean and variance and the bias smoothing are a small fixed cost.",
]


def main(args: argparse.Namespace) -> int:
    return run_suite(args, "ants-denoise", JOBS, TITLE, NOTES)
