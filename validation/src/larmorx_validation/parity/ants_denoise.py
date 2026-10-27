# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants DenoiseImage`` with ANTs 2.6.5's ``DenoiseImage``.

DenoiseImage's result in ANTs depends on ANTs' thread count: ITK's threads add to output
voxels near the borders of their regions without locks, so the result changes with the number
of threads (and can change between runs). larmorx computes ANTs' **single-thread** result on
any number of threads. The comparison (ANTs at one thread as built, ANTs with correctly
rounded math, ANTs' thread band) is shared with N4: :mod:`~larmorx_validation.parity.ants_threaded`.

The real cases are sMRIPrep's command line (``DenoiseImage -i <T1w> -n Rician -o <out> -s 1``,
as nipype writes it) on T1-weighted images, and the same options on other MR images.
"""

from __future__ import annotations

import os

import numpy as np

from larmorx_validation.parity import ants_inputs, ants_threaded, fmriprep_nodes
from larmorx_validation.parity.ants_image_math.common import (
    BOLDREF,
    CONSTANT,
    FRACTIONAL,
    HEAD,
    HEAD_I16,
    LAS,
    MASK,
    MNI_2MM,
    NEG_PIXDIM,
    OUTLIERS,
    SERIES,
    SLICE,
    T1_MASK,
    T1_PREP,
    T1_RAW,
)
from larmorx_validation.parity.ants_programs import AntsCase, ants_version, cases_for
from larmorx_validation.parity.harness import Case, CaseResult, CheckList, Suite

PROGRAM = "DenoiseImage"
LX_THREADS = ants_threaded.LX_THREADS
#: ANTs' thread band: 2, 4, 12 threads unless ``LARMORX_PARITY_BAND_THREADS``
#: (e.g. ``2,3`` on a shared machine) says otherwise.
BAND_THREADS = tuple(
    int(v) for v in os.environ.get("LARMORX_PARITY_BAND_THREADS", "2,4,12").split(",") if v
)
NCC_MIN = ants_threaded.NCC_MIN
AFFINE_ATOL = ants_threaded.AFFINE_ATOL

MAG_DS003012 = "{in:openneuro/ds003012/sub-01/fmap/sub-01_magnitude1.nii.gz}"  # smoke
T1_DS000210 = "{in:openneuro/ds000210/sub-02/anat/sub-02_T1w.nii.gz}"  # standard


# ------------------------------------------------------------------------------------------------
# Inputs built for this suite


def _shrunk_affine(affine: np.ndarray, shape: tuple[int, ...], factor: int) -> np.ndarray:
    """The grid ITK's ShrinkImageFilter gives an image with ``affine`` and ``shape``: spacing
    times ``factor``, ``floor(n / factor)`` voxels, the centres of the two regions at the same
    point."""
    out = affine.copy()
    out[:3, :3] = affine[:3, :3] * factor
    n = np.array(shape[:3], dtype=np.float64)
    m = np.floor(n / factor)
    centre_in = affine[:3, :3] @ ((n - 1) / 2) + affine[:3, 3]
    centre_out = out[:3, :3] @ ((m - 1) / 2) + affine[:3, 3]
    out[:3, 3] = affine[:3, 3] + (centre_in - centre_out)
    return out


@ants_inputs.builder("denoise-mask-shrunk-2")
def _(t):
    """The phantom's brain mask on the grid of the phantom shrunk by 2 (``-s 2``): the only
    grid a mask may have then."""
    mask = ants_inputs.brain_mask()
    shape = tuple(n // 2 for n in mask.shape)
    data = mask[1::2, 1::2, 1::2][: shape[0], : shape[1], : shape[2]]
    ants_inputs.save(
        np.ascontiguousarray(data),
        t,
        affine=_shrunk_affine(ants_inputs.AFFINE, mask.shape, 2),
    )


@ants_inputs.builder("denoise-mask-larger")
def _(t):
    """The brain mask with 3 more voxels along each axis (the same origin and spacing): ITK
    reads it by the image's indices."""
    mask = ants_inputs.brain_mask()
    big = np.zeros(tuple(n + 3 for n in mask.shape), np.uint8)
    big[: mask.shape[0], : mask.shape[1], : mask.shape[2]] = mask
    big[-2:, :, :] = 1
    ants_inputs.save(big, t)


@ants_inputs.builder("denoise-mask-smaller")
def _(t):
    """The brain mask without its last 2 planes: it does not cover the image."""
    ants_inputs.save(np.ascontiguousarray(ants_inputs.brain_mask()[:, :, :-2]), t)


@ants_inputs.builder("denoise-mask-other-grid")
def _(t):
    """The brain mask on a grid shifted by 3 mm."""
    a = ants_inputs.AFFINE.copy()
    a[0, 3] += 3.0
    ants_inputs.save(ants_inputs.brain_mask(), t, affine=a)


@ants_inputs.builder("denoise-mask-values")
def _(t):
    """A float mask with the values 0, 0.4, 1, 255, 256, 300.5 and -1 inside the brain (ANTs
    reads masks as unsigned char: 0.4 and 256 become 0, -1 becomes 255)."""
    rng = np.random.default_rng(21)
    values = np.array([0, 0.4, 1.0, 255.0, 256.0, 300.5, -1.0], np.float32)
    mask = ants_inputs.brain_mask()
    data = values[rng.integers(0, len(values), mask.shape)] * mask
    ants_inputs.save(data.astype(np.float32), t)


@ants_inputs.builder("denoise-slice-mask")
def _(t):
    """A 2D brain mask on the 2D slice's grid."""
    ants_inputs.save(
        np.ascontiguousarray(ants_inputs.brain_mask()[:, :, 14]),
        t,
        affine=np.diag([1.5, 1.2, 1.0, 1.0]),
    )


MASK_SHRUNK = "{gen:denoise-mask-shrunk-2}"
MASK_LARGER = "{gen:denoise-mask-larger}"
MASK_SMALLER = "{gen:denoise-mask-smaller}"
MASK_OTHER = "{gen:denoise-mask-other-grid}"
MASK_VALUES = "{gen:denoise-mask-values}"
SLICE_MASK = "{gen:denoise-slice-mask}"


# ------------------------------------------------------------------------------------------------
# Cases


def dn(cid, category, description, *args, outputs=("{out}",), **kw) -> AntsCase:
    return AntsCase(
        f"{category}/{cid}", category, description, PROGRAM, tuple(args), outputs=outputs, **kw
    )


def all_cases() -> list[AntsCase]:
    R, OPT, INP, E = "fmriprep", "options", "inputs", "errors"
    both = ("{out}", "{out2}")
    return [
        # -- sMRIPrep's command line, on real data --------------------------------------------
        dn(
            "smriprep-t1w",
            R,
            "sMRIPrep's denoising of each T1w (anatomical.py:1461), as nipype writes it: "
            "-i <T1w> -n Rician -o <out> -s 1, no -d; ds000005's raw T1w (160 x 192 x 192)",
            "-i",
            T1_RAW,
            "-n",
            "Rician",
            "-o",
            "{out}",
            "-s",
            "1",
            tier="standard",
        ),
        dn(
            "smriprep-t1w-ds000210",
            R,
            "the same command on ds000210's T1w (256 x 256 x 176, float64 on disk), the noise "
            "saved as nipype writes it with save_noise=True: -o [ out, noise ]",
            "-i",
            T1_DS000210,
            "-n",
            "Rician",
            "-o",
            "[",
            "{out},",
            "{out2}",
            "]",
            "-s",
            "1",
            outputs=both,
            tier="standard",
        ),
        dn(
            "t1w-preproc-mask",
            R,
            "fMRIPrep's preprocessed T1w (ds000005) with -n Rician and its brain mask",
            "-d",
            "3",
            "-i",
            T1_PREP,
            "-n",
            "Rician",
            "-x",
            T1_MASK,
            "-o",
            "[{out},{out2}]",
            outputs=both,
            tier="standard",
        ),
        dn(
            "mni-t1w",
            R,
            "sMRIPrep's options on MNI152NLin2009cAsym's T1w template (res-02, 97 x 115 x 97)",
            "-i",
            MNI_2MM,
            "-n",
            "Rician",
            "-o",
            "{out}",
            "-s",
            "1",
            tier="standard",
        ),
        dn(
            "boldref",
            R,
            "sMRIPrep's options on ds000005's BOLD reference (64 x 64 x 34, int16)",
            "-i",
            BOLDREF,
            "-n",
            "Rician",
            "-o",
            "[{out},{out2}]",
            "-s",
            "1",
            outputs=both,
        ),
        dn(
            "magnitude",
            R,
            "sMRIPrep's options on ds003012's fieldmap magnitude (64 x 64 x 36, int32)",
            "-i",
            MAG_DS003012,
            "-n",
            "Rician",
            "-o",
            "{out}",
            "-s",
            "1",
        ),
        # -- Options, on the head phantom ------------------------------------------------------
        dn(
            "defaults",
            OPT,
            "no option but input and output: the Gaussian noise model",
            "-d",
            "3",
            "-i",
            HEAD,
            "-o",
            "{out}",
        ),
        dn(
            "rician",
            OPT,
            "-n Rician, with the noise image",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-o",
            "[{out},{out2}]",
            outputs=both,
        ),
        dn(
            "gaussian",
            OPT,
            "-n Gaussian, with the noise image",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Gaussian",
            "-o",
            "[{out},{out2}]",
            outputs=both,
        ),
        dn(
            "model-case",
            OPT,
            "-n RICIAN: the model name is case-insensitive",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "RICIAN",
            "-o",
            "{out}",
        ),
        dn(
            "mask-rician",
            OPT,
            "-x a binary mask (uint8), Rician: only voxels in the mask are centres; the bias "
            "correction only in the mask",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-x",
            MASK,
            "-o",
            "[{out},{out2}]",
            outputs=both,
        ),
        dn(
            "mask-gaussian",
            OPT,
            "-x a binary mask, Gaussian",
            "-d",
            "3",
            "-i",
            HEAD,
            "-x",
            MASK,
            "-o",
            "{out}",
        ),
        dn(
            "mask-fractional",
            OPT,
            "a float mask (0, 0.3, 0.6, 0.999, 1, 1.7, 2) read as unsigned char: below 1 is 0",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-x",
            FRACTIONAL,
            "-o",
            "{out}",
        ),
        dn(
            "mask-values",
            OPT,
            "a float mask with 0.4, 255, 256, 300.5 and -1: the C++ casts to unsigned char "
            "(256 is 0, -1 is 255)",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-x",
            MASK_VALUES,
            "-o",
            "{out}",
        ),
        dn(
            "mask-larger",
            OPT,
            "a mask larger than the image on the same grid: read by the image's indices",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-x",
            MASK_LARGER,
            "-o",
            "{out}",
        ),
        dn(
            "missing-mask",
            OPT,
            "a mask that does not exist: ANTs prints a message and denoises without a mask",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-x",
            "{missing:none.nii.gz}",
            "-o",
            "{out}",
        ),
        dn(
            "shrink-2",
            OPT,
            "-s 2: the filter runs on every other voxel; the noise is resampled linearly "
            "(float coordinates) onto the full grid",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-s",
            "2",
            "-o",
            "[{out},{out2}]",
            outputs=both,
        ),
        dn(
            "shrink-3",
            OPT,
            "-s 3, Gaussian",
            "-d",
            "3",
            "-i",
            HEAD,
            "-s",
            "3",
            "-o",
            "[{out},{out2}]",
            outputs=both,
        ),
        dn(
            "shrink-0", OPT, "-s 0: ITK makes it 1", "-d", "3", "-i", HEAD, "-s", "0", "-o", "{out}"
        ),
        dn(
            "shrink-2-mask",
            OPT,
            "-s 2 with a mask on the shrunk grid",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-s",
            "2",
            "-x",
            MASK_SHRUNK,
            "-o",
            "{out}",
        ),
        dn(
            "patch-2",
            OPT,
            "-p 2: patches of 5 x 5 x 5",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-p",
            "2",
            "-o",
            "{out}",
        ),
        dn(
            "patch-0",
            OPT,
            "-p 0: patches of one voxel",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-p",
            "0",
            "-o",
            "{out}",
        ),
        dn("search-1", OPT, "-r 1", "-d", "3", "-i", HEAD, "-r", "1", "-o", "{out}"),
        dn(
            "search-3",
            OPT,
            "-r 3, Rician",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-r",
            "3",
            "-o",
            "{out}",
        ),
        dn(
            "radii-per-axis",
            OPT,
            "-p 1x2x1 -r 2x1x3: one radius per axis",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-p",
            "1x2x1",
            "-r",
            "2x1x3",
            "-o",
            "{out}",
        ),
        dn(
            "output-one-parameter",
            OPT,
            "-o [out]: one bracketed name",
            "-d",
            "3",
            "-i",
            HEAD,
            "-o",
            "[{out}]",
        ),
        dn(
            "no-output",
            OPT,
            "no -o: the filter runs, nothing is written, exit 0",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Rician",
            outputs=(),
        ),
        dn(
            "verbose",
            OPT,
            "-v 1 (stdout is not compared: it holds timings and ITK's object dump)",
            "-d",
            "3",
            "-i",
            HEAD,
            "-v",
            "1",
            "-o",
            "{out}",
        ),
        # -- Inputs ---------------------------------------------------------------------------
        dn(
            "int16-scaled",
            INP,
            "scaled int16 input",
            "-d",
            "3",
            "-i",
            HEAD_I16,
            "-n",
            "Rician",
            "-o",
            "{out}",
        ),
        dn(
            "outliers",
            INP,
            "hot, negative and zero voxels, a NaN and -inf: they spread to their patches",
            "-d",
            "3",
            "-i",
            OUTLIERS,
            "-n",
            "Rician",
            "-o",
            "[{out},{out2}]",
            outputs=both,
        ),
        dn(
            "outliers-gaussian",
            INP,
            "the same, Gaussian",
            "-d",
            "3",
            "-i",
            OUTLIERS,
            "-o",
            "{out}",
        ),
        dn(
            "constant",
            INP,
            "a constant image (every local variance 0: no centre takes part)",
            "-d",
            "3",
            "-i",
            CONSTANT,
            "-n",
            "Rician",
            "-o",
            "{out}",
        ),
        dn(
            "las-sform-only",
            INP,
            "an sform-only LAS image, -s 2 (the noise resampled through the oblique grid)",
            "-d",
            "3",
            "-i",
            LAS,
            "-n",
            "Rician",
            "-s",
            "2",
            "-o",
            "{out}",
        ),
        dn(
            "negative-pixdim",
            INP,
            "a qform-only image with a negative pixdim",
            "-d",
            "3",
            "-i",
            NEG_PIXDIM,
            "-n",
            "Rician",
            "-o",
            "{out}",
        ),
        dn(
            "dim2",
            INP,
            "-d 2 on a 2D image, with the noise",
            "-d",
            "2",
            "-i",
            SLICE,
            "-n",
            "Rician",
            "-o",
            "[{out},{out2}]",
            outputs=both,
        ),
        dn(
            "dim2-mask-shrink",
            INP,
            "-d 2 with a 2D mask and -p 2, Gaussian",
            "-d",
            "2",
            "-i",
            SLICE,
            "-x",
            SLICE_MASK,
            "-p",
            "2",
            "-o",
            "{out}",
        ),
        dn(
            "dim2-shrink-2",
            INP,
            "-d 2 with -s 2",
            "-d",
            "2",
            "-i",
            SLICE,
            "-n",
            "Rician",
            "-s",
            "2",
            "-o",
            "{out}",
        ),
        dn(
            "dim4",
            INP,
            "-d 4: 4D patches and search boxes (time is an axis like the others)",
            "-d",
            "4",
            "-i",
            SERIES,
            "-n",
            "Rician",
            "-o",
            "[{out},{out2}]",
            outputs=both,
        ),
        dn(
            "dim4-shrink-2",
            INP,
            "-d 4 -s 2: time is shrunk too; the noise resampled in 4D",
            "-d",
            "4",
            "-i",
            SERIES,
            "-s",
            "2",
            "-r",
            "1",
            "-o",
            "[{out},{out2}]",
            outputs=both,
        ),
        dn(
            "dim3-of-4d",
            INP,
            "-d 3 on a 4D file: its first volume, with an identity direction",
            "-d",
            "3",
            "-i",
            SERIES,
            "-n",
            "Rician",
            "-o",
            "{out}",
        ),
        dn(
            "dimension-from-file",
            INP,
            "no -d: the dimension is the file's",
            "-i",
            HEAD,
            "-n",
            "Rician",
            "-o",
            "{out}",
        ),
        # -- Errors -----------------------------------------------------------------------------
        AntsCase(f"{E}/help", E, "--help: the usage, exit 0", PROGRAM, ("--help",), outputs=()),
        dn(
            "shrink-not-integer",
            E,
            "-s 1.5: the parser's Convert<int> throws (ANTs aborts)",
            "-d",
            "3",
            "-i",
            HEAD,
            "-s",
            "1.5",
            "-o",
            "{out}",
            expect="error",
        ),
        dn(
            "dimension-5",
            E,
            "-d 5: 'Unsupported dimension'",
            "-d",
            "5",
            "-i",
            HEAD,
            "-o",
            "{out}",
            expect="error",
        ),
        dn(
            "missing-input",
            E,
            "an input that does not exist (ANTs crashes)",
            "-d",
            "3",
            "-i",
            "{missing:none.nii.gz}",
            "-o",
            "{out}",
            expect="error",
        ),
        dn(
            "unknown-model",
            E,
            "-n Poisson: exit 1",
            "-d",
            "3",
            "-i",
            HEAD,
            "-n",
            "Poisson",
            "-o",
            "{out}",
            expect="error",
        ),
        dn(
            "patch-wrong-length",
            E,
            "-p 1x1 in 3D: exit 1",
            "-d",
            "3",
            "-i",
            HEAD,
            "-p",
            "1x1",
            "-o",
            "{out}",
            expect="error",
        ),
        dn(
            "search-bracketed",
            E,
            "-r [2]: the option's name is empty, so no radius is read: exit 1",
            "-d",
            "3",
            "-i",
            HEAD,
            "-r",
            "[2]",
            "-o",
            "{out}",
            expect="error",
        ),
        dn(
            "mask-other-grid",
            E,
            "a mask on another grid: ITK throws, exit 1",
            "-d",
            "3",
            "-i",
            HEAD,
            "-x",
            MASK_OTHER,
            "-o",
            "{out}",
            expect="error",
        ),
        dn(
            "mask-full-resolution-shrink-2",
            E,
            "-s 2 with a full-resolution mask: not on the shrunk grid, ITK throws, exit 1",
            "-d",
            "3",
            "-i",
            HEAD,
            "-s",
            "2",
            "-x",
            MASK,
            "-o",
            "{out}",
            expect="error",
        ),
        dn(
            "mask-smaller",
            E,
            "a mask that does not cover the image: ITK's requested region is outside it, exit 1",
            "-d",
            "3",
            "-i",
            HEAD,
            "-x",
            MASK_SMALLER,
            "-o",
            "{out}",
            expect="error",
        ),
    ]


# ------------------------------------------------------------------------------------------------
# The data panel's tier


def panel_cases() -> list[AntsCase]:
    """The data panel's tier: every DenoiseImage node of the fourteen fMRIPrep oracle runs
    (sMRIPrep's denoising of each T1w; ANTs' thread band on the first), and sMRIPrep's command
    on panel images it never denoises."""
    from larmorx_validation.parity import panel_images as P

    def d(cid, description, image, noise="Rician"):
        return P.ants_case(
            PROGRAM,
            cid,
            description,
            ("-i", image, "-n", noise, "-o", "[", "{out},", "{out2}", "]", "-s", "1"),
            [image],
            outputs=("{out}", "{out2}"),
        )

    return [
        *fmriprep_nodes.ants_cases(PROGRAM, band_once=True),
        d("rician/7t-mp2rage", "sMRIPrep's command (-n Rician -s 1, the noise saved) on the raw "
          "7 T MP2RAGE: the panel's largest anatomical image, its noisy background",
          P.MP2RAGE_7T),
        d("rician/flair", "the same on a FLAIR of 0.42 x 0.42 x 3 mm voxels (384 x 512 x 44)",
          P.FLAIR_OLDER),
        d("rician/child-t2w", "the same on a child's 0.8 mm T2w", P.CHILD_T2W),
        d("gaussian/1.5t-float32", "-n Gaussian on a float32 1.5 T MPRAGE stored uncompressed",
          P.MPRAGE_15T, noise="Gaussian"),
    ]  # fmt: skip


# ------------------------------------------------------------------------------------------------
# Running, comparing and the report (ants_threaded)


def run_case(case: Case, checks: CheckList) -> None:
    c: AntsCase = case.payload
    labels = ("denoised", "noise") if len(c.outputs) == 2 else ("denoised",)
    ants_threaded.compare(c, checks, labels, BAND_THREADS, "lx-denoise-")


def highlights(results: list[CaseResult]) -> list[str]:
    return ants_threaded.highlights(
        results, "`expf` (or `exp`)", BAND_THREADS
    ) + fmriprep_nodes.record_lines(PROGRAM, results)


def suite():
    return Suite(
        name="ants-denoise",
        title="DenoiseImage",
        tool="`larmorx ants DenoiseImage` / `lx.ants.denoise_image` "
        "(crates larmorx-image, larmorx-ants)",
        reference=f"DenoiseImage from {ants_version()}, the binary built by "
        "`scripts/build_ants_oracle.sh` (with ITK's remote module AdaptiveDenoising at "
        "`24825c8d`), run with one thread",
        thresholds=[
            ("Exit status", "both produce their outputs, or neither does"),
            ("Header", "every field of the 348-byte header identical"),
            ("Data type, shape, affine", f"identical (affine within {AFFINE_ATOL:g} mm)"),
            ("larmorx's threads", f"{LX_THREADS} threads and 1 thread give the same bits"),
            (
                "Values",
                "with ANTs as built (glibc's math), bit-identity reported and NCC > 0.995 "
                "where they differ; bit-identical to ANTs at one thread with correctly rounded "
                "math (CORE-MATH preloaded)",
            ),
            (
                "Reported",
                "the difference from ANTs as built next to ANTs' thread band (ANTs at "
                f"{', '.join(map(str, BAND_THREADS))} threads vs 1 thread)",
            ),
        ],
        cases=lambda tier: cases_for(tier, [*all_cases(), *panel_cases()]),
        run_case=run_case,
        packages=("nibabel",),
        highlights=highlights,
        notes=(
            "ANTs runs as its own binary, larmorx in-process through its Python console entry "
            "point. Outputs are read with nibabel (values as stored) and the headers from their "
            "raw bytes. Inputs named `gen:<name>` are built by "
            "`larmorx_validation.parity.ants_inputs` and this suite's builders; real images "
            "come from `larmorx-testdata`."
        ),
    )


__all__ = ["all_cases", "run_case", "suite"]
