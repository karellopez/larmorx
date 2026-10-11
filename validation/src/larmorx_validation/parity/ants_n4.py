# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx ants N4BiasFieldCorrection`` with ANTs 2.6.5's ``N4BiasFieldCorrection``.

N4's result in ANTs depends on ANTs' thread count: ITK's B-spline fit accumulates its sums per
work unit. larmorx computes ANTs' **single-thread** result on any number of threads, so the
references run ANTs with ``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=1``; larmorx runs with 4
threads, and once more with 1, which must give the same bits.

Two references:

- **ANTs with correctly rounded math**: the same binary with CORE-MATH's ``expf``, ``logf``,
  ``exp``, ``log``, ``sin``, ``cos`` and ``sincos`` preloaded (``scripts/build_crmath_preload.sh``),
  which is what larmorx computes with (CLAUDE.md rule 5). larmorx must be **bit-identical** to
  it: this is the replica's exactness.
- **ANTs as built** (glibc's math). Bit-identity is reported; where glibc's functions are not
  correctly rounded for a value the run meets, the outputs differ, and the difference must
  keep NCC > 0.995 (PLAN.md §11.3). It is reported next to ANTs' own thread band: ANTs run
  again at 2, 3, 4, 8 and 12 threads, compared with ANTs at one thread.

The real cases are the command lines fMRIPrep, sMRIPrep and sdcflows give N4, written as
nipype writes them (``[ 200 ]`` with spaces).
"""

from __future__ import annotations

import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.parity import ants_inputs
from larmorx_validation.parity.ants_image_math.common import (
    BOLD_MASK,
    BOLDREF,
    CONSTANT,
    FRACTIONAL,
    HEAD,
    HEAD_I16,
    LAS,
    MASK,
    NEG_PIXDIM,
    OUTLIERS,
    PROBSEG,
    SLICE,
    T1_PREP,
    T1_RAW,
    WM,
)
from larmorx_validation.parity.ants_programs import (
    AntsCase,
    ants_bin,
    ants_version,
    cases_for,
    crmath_preload,
    header_differences,
    raw_header,
    resolve_arg,
    run_ants,
    run_larmorx,
)
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    PASS,
    SKIPPED,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
)
from larmorx_validation.report import table

PROGRAM = "N4BiasFieldCorrection"
#: larmorx's thread count (its result does not depend on it).
LX_THREADS = 4
#: ANTs' thread counts that define its own variability.
BAND_THREADS = (2, 3, 4, 8, 12)
NCC_MIN = 0.995
AFFINE_ATOL = 1e-6

TPL = "templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym"
MNI_BOLDREF = "{in:" + TPL + "_res-02_desc-fMRIPrep_boldref.nii.gz}"  # smoke
MAG_DS003012 = "{in:openneuro/ds003012/sub-01/fmap/sub-01_magnitude1.nii.gz}"  # smoke
MAG_DS001600 = "{in:openneuro/ds001600/sub-1/fmap/sub-1_acq-v4_magnitude1.nii.gz}"  # smoke
T1_DS000210 = "{in:openneuro/ds000210/sub-02/anat/sub-02_T1w.nii.gz}"  # standard


# ------------------------------------------------------------------------------------------------
# Inputs built for this suite


def _td(path: str) -> Path:
    import larmorx_testdata as td

    return Path(td.get(path))


@ants_inputs.builder("n4-boldref-premask")
def _(t):
    """A stand-in for fMRIPrep's premask of the ds000005 boldref: its brain mask blurred into a
    probability map (Gaussian, sigma 1.5 voxels), float32 in [0, 1]."""
    import nibabel as nib
    from scipy.ndimage import gaussian_filter

    m = nib.load(_td(BOLD_MASK[4:-1]))
    p = gaussian_filter(np.asarray(m.dataobj, dtype=np.float64), 1.5)
    out = nib.Nifti1Image(np.clip(p, 0, 1).astype(np.float32), m.affine, m.header)
    out.set_data_dtype(np.float32)
    nib.save(out, t)


@ants_inputs.builder("n4-mni-premask")
def _(t):
    """fMRIPrep's premask for the MNI152NLin2009cAsym res-02 boldref template: the res-01 brain
    probability map resampled onto the res-02 grid (antsApplyTransforms, Linear, identity),
    as fMRIPrep resamples it into BOLD space."""
    import larmorx as lx

    ref = _td(TPL + "_res-02_desc-fMRIPrep_boldref.nii.gz")
    prob = _td(TPL + "_res-01_label-brain_probseg.nii.gz")
    out = lx.ants.apply_transforms(prob, reference=ref, transforms=[], interpolation="linear")
    lx.save(out, t)


@ants_inputs.builder("n4-t1-truncated")
def _(t):
    """sMRIPrep's input to its first N4: ds000005's raw T1w after ``ImageMath 3 out
    TruncateImageIntensity in 0.01 0.999 256`` (larmorx's, bit-identical to ANTs')."""
    import larmorx as lx

    out = lx.ants.truncate_image_intensity(
        _td(T1_RAW[4:-1]), lower_quantile=0.01, upper_quantile=0.999, bins=256
    )
    lx.save(out, t)


@ants_inputs.builder("n4-t1-brain")
def _(t):
    """A skull-stripped T1w (sMRIPrep's N4-only path): ds000005's raw T1w times fMRIPrep's
    brain mask, int16."""
    import nibabel as nib

    img = nib.load(_td(T1_RAW[4:-1]))
    mask = np.asarray(
        nib.load(
            _td("openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_desc-brain_mask.nii.gz")
        ).dataobj
    )
    data = (np.asarray(img.dataobj) * (mask > 0)).astype(np.int16)
    out = nib.Nifti1Image(data, img.affine, img.header)
    nib.save(out, t)


@ants_inputs.builder("n4-ramp")
def _(t):
    """Values strictly increasing in memory order (100 to about 4000): every voxel raises the
    running maximum of N4's histogram range, so ITK never sets its minimum
    (``SharpenImage``'s order-dependent range)."""
    shape = (24, 20, 16)
    n = int(np.prod(shape))
    data = (100.0 + np.arange(n, dtype=np.float64) * 0.5).astype(np.float32)
    ants_inputs.save(data.reshape(shape, order="F"), t, affine=np.diag([2.0, 2.0, 2.5, 1.0]))


@ants_inputs.builder("n4-series-4d")
def _(t):
    """A short 4D head-like series: the phantom (16 x 20 x 14, cropped) with a bias that
    drifts over 5 volumes (TR 2 s)."""
    import nibabel as nib

    head = ants_inputs.head(11)[8:24, 10:30, 7:21]
    vols = [head * (1.0 + 0.04 * v) for v in range(5)]
    img = nib.Nifti1Image(np.stack(vols, axis=-1).astype(np.float32), ants_inputs.AFFINE)
    img.header.set_xyzt_units("mm", "sec")
    img.header.set_zooms((*img.header.get_zooms()[:3], 2.0))
    img.header.set_qform(ants_inputs.AFFINE, code=1)
    img.header.set_sform(ants_inputs.AFFINE, code=1)
    nib.save(img, t)


@ants_inputs.builder("n4-mask-other-grid")
def _(t):
    """The phantom's brain mask on a grid shifted by 3 mm (ITK: "Inputs do not occupy the
    same physical space")."""
    a = ants_inputs.AFFINE.copy()
    a[0, 3] += 3.0
    ants_inputs.save(ants_inputs.brain_mask(), t, affine=a)


PREMASK = "{gen:n4-boldref-premask}"
MNI_PREMASK = "{gen:n4-mni-premask}"
T1_TRUNC = "{gen:n4-t1-truncated}"
T1_BRAIN = "{gen:n4-t1-brain}"
RAMP = "{gen:n4-ramp}"
SERIES = "{gen:n4-series-4d}"
OTHER_GRID = "{gen:n4-mask-other-grid}"


# ------------------------------------------------------------------------------------------------
# Cases


def n4(cid, category, description, *args, outputs=("{out}",), **kw) -> AntsCase:
    return AntsCase(
        f"{category}/{cid}", category, description, PROGRAM, tuple(args), outputs=outputs, **kw
    )


def all_cases() -> list[AntsCase]:
    R, OPT, INP, E = "fmriprep", "options", "inputs", "errors"
    bias = ("{out}", "{out2}")
    return [
        # -- fMRIPrep, sMRIPrep and sdcflows command lines, on real data ----------------------
        n4(
            "boldref-coregistration",
            R,
            "fMRIPrep's BOLD reference (niworkflows func/util.py:487): -d 3, b-spline distance "
            "200, rescale (no effect without a mask), weight = premask (a blurred brain mask), "
            "ANTs' default shrink 4 (nipype drops shrink_factor=2); ds000005's boldref",
            "-d",
            "3",
            "--bspline-fitting",
            "[",
            "200",
            "]",
            "-i",
            BOLDREF,
            "-o",
            "{out}",
            "-r",
            "--weight-image",
            PREMASK,
        ),
        n4(
            "boldref-template",
            R,
            "the same command on MNI152NLin2009cAsym's fMRIPrep boldref (res-02), weight = the "
            "brain probability map resampled onto it, as fMRIPrep's premask",
            "-d",
            "3",
            "--bspline-fitting",
            "[",
            "200",
            "]",
            "-i",
            MNI_BOLDREF,
            "-o",
            "{out}",
            "-r",
            "--weight-image",
            MNI_PREMASK,
        ),
        n4(
            "smriprep-pre-template",
            R,
            "sMRIPrep's N4 before the anatomical template (anatomical.py:1502): -d 3, ANTs' "
            "defaults (4 levels of 50 iterations, threshold 0, 4 control points, shrink 4); "
            "ds000005's raw T1w",
            "-d",
            "3",
            "--input-image",
            T1_RAW,
            "--output",
            "{out}",
            tier="standard",
        ),
        n4(
            "brain-extraction-pass1",
            R,
            "sMRIPrep's brain extraction, first N4 (niworkflows anat/ants.py:243): "
            "-b [200] -c [50x50x50x50,1e-7] -s 4, on the truncated T1w",
            "-d",
            "3",
            "--bspline-fitting",
            "[",
            "200",
            "]",
            "--convergence",
            "[",
            "50x50x50x50,",
            "1e-07",
            "]",
            "--input-image",
            T1_TRUNC,
            "--output",
            "{out}",
            "--shrink-factor",
            "4",
            tier="standard",
        ),
        n4(
            "brain-extraction-final",
            R,
            "sMRIPrep's final N4 (anat/ants.py:704): -b [200] -c [50x50x50x50x50,1e-7] -s 4 -r, "
            "weight = the WM probability map, bias field saved; ds000005's raw T1w",
            "-d",
            "3",
            "--bspline-fitting",
            "[",
            "200",
            "]",
            "--convergence",
            "[",
            "50x50x50x50x50,",
            "1e-07",
            "]",
            "--input-image",
            T1_RAW,
            "--output",
            "[",
            "{out},",
            "{out2}",
            "]",
            "-r",
            "--shrink-factor",
            "4",
            "--weight-image",
            WM,
            outputs=bias,
            tier="standard",
        ),
        n4(
            "n4-only",
            R,
            "sMRIPrep's N4 for skull-stripped T1w images (anat/ants.py:927): the final N4's "
            "options without a weight image, on a brain-masked T1w",
            "-d",
            "3",
            "--bspline-fitting",
            "[",
            "200",
            "]",
            "--convergence",
            "[",
            "50x50x50x50x50,",
            "1e-07",
            "]",
            "--input-image",
            T1_BRAIN,
            "--output",
            "[",
            "{out},",
            "{out2}",
            "]",
            "-r",
            "--shrink-factor",
            "4",
            outputs=bias,
            tier="standard",
        ),
        n4(
            "sdcflows-magnitude",
            R,
            "sdcflows' brain extraction of a fieldmap magnitude (workflows/ancillary.py:87): "
            "-c [50x50x50x50x50,1e-7] -s 4; ds003012's magnitude1",
            "-d",
            "3",
            "--convergence",
            "[",
            "50x50x50x50x50,",
            "1e-07",
            "]",
            "--input-image",
            MAG_DS003012,
            "--output",
            "{out}",
            "--shrink-factor",
            "4",
        ),
        n4(
            "sdcflows-magnitude-ds001600",
            R,
            "the same command on ds001600's magnitude1 (acq-v4)",
            "-d",
            "3",
            "--convergence",
            "[",
            "50x50x50x50x50,",
            "1e-07",
            "]",
            "--input-image",
            MAG_DS001600,
            "--output",
            "{out}",
            "--shrink-factor",
            "4",
        ),
        n4(
            "t1w-ds000210-final",
            R,
            "the final N4's options (without weights) on ds000210's T1w",
            "-d",
            "3",
            "--bspline-fitting",
            "[",
            "200",
            "]",
            "--convergence",
            "[",
            "50x50x50x50x50,",
            "1e-07",
            "]",
            "--input-image",
            T1_DS000210,
            "--output",
            "[",
            "{out},",
            "{out2}",
            "]",
            "--shrink-factor",
            "4",
            outputs=bias,
            tier="standard",
        ),
        n4(
            "boldref-mask",
            R,
            "ds000005's boldref with its brain mask: outside-mask voxels restored, the rest "
            "rescaled to the input's range in the mask (ANTs' default with -x); bias field saved",
            "-d",
            "3",
            "-b",
            "[200]",
            "-i",
            BOLDREF,
            "-x",
            BOLD_MASK,
            "-o",
            "[{out},{out2}]",
            outputs=bias,
        ),
        n4(
            "t1w-preproc-mask",
            R,
            "fMRIPrep's preprocessed T1w with -x the brain mask, -r 0, shrink 2",
            "-d",
            "3",
            "-i",
            T1_PREP,
            "-x",
            "{in:openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_desc-brain_mask.nii.gz}",
            "-r",
            "0",
            "-s",
            "2",
            "-c",
            "[20x20x20]",
            "-o",
            "{out}",
            tier="standard",
        ),
        # -- Options, on the head phantom -------------------------------------------------------
        n4("defaults", OPT, "no option but input and output", "-d", "3", "-i", HEAD, "-o", "{out}"),
        n4(
            "mask",
            OPT,
            "a binary mask: restoration outside, rescaling inside; bias field",
            "-d",
            "3",
            "-i",
            HEAD,
            "-x",
            MASK,
            "-o",
            "[{out},{out2}]",
            outputs=bias,
        ),
        n4(
            "mask-no-rescale",
            OPT,
            "a mask with -r 0",
            "-d",
            "3",
            "-i",
            HEAD,
            "-x",
            MASK,
            "-r",
            "0",
            "-o",
            "{out}",
        ),
        n4(
            "fractional-mask",
            OPT,
            "a float mask (0, 0.3, 0.6, 0.999, 1, 1.7, 2): N4 uses every non-zero voxel; only "
            "voxels within 4 ulps of 1 are rescaled",
            "-d",
            "3",
            "-i",
            HEAD,
            "-x",
            FRACTIONAL,
            "-o",
            "{out}",
        ),
        n4(
            "weight",
            OPT,
            "a probability map as weight image (exact 0, 0.5 and 1)",
            "-d",
            "3",
            "-i",
            HEAD,
            "-w",
            PROBSEG,
            "-o",
            "[{out},{out2}]",
            outputs=bias,
        ),
        n4(
            "mask-and-weight",
            OPT,
            "mask and weight image together",
            "-d",
            "3",
            "-i",
            HEAD,
            "-x",
            MASK,
            "-w",
            PROBSEG,
            "-o",
            "{out}",
        ),
        n4(
            "shrink-1", OPT, "-s 1: no subsampling", "-d", "3", "-i", HEAD, "-s", "1", "-o", "{out}"
        ),
        n4("shrink-3", OPT, "-s 3", "-d", "3", "-i", HEAD, "-s", "3", "-o", "{out}"),
        n4(
            "shrink-0", OPT, "-s 0: ITK makes it 1", "-d", "3", "-i", HEAD, "-s", "0", "-o", "{out}"
        ),
        n4(
            "distance-30",
            OPT,
            "-b [30]: padding to whole spans of 30 mm, 2 or 3 spans per axis",
            "-d",
            "3",
            "-i",
            HEAD,
            "-b",
            "[30]",
            "-s",
            "2",
            "-o",
            "[{out},{out2}]",
            outputs=bias,
        ),
        n4(
            "distance-order-2",
            OPT,
            "-b [25,2]: quadratic splines",
            "-d",
            "3",
            "-i",
            HEAD,
            "-b",
            "[25,2]",
            "-s",
            "2",
            "-o",
            "{out}",
        ),
        n4(
            "mesh",
            OPT,
            "-b [2x3x1]: a mesh of 2 x 3 x 1 elements",
            "-d",
            "3",
            "-i",
            HEAD,
            "-b",
            "[2x3x1]",
            "-o",
            "{out}",
        ),
        n4(
            "order-5",
            OPT,
            "-b [1x1x1,5]: quintic splines (Cox-de Boor kernels)",
            "-d",
            "3",
            "-i",
            HEAD,
            "-b",
            "[1x1x1,5]",
            "-c",
            "[10x10]",
            "-o",
            "{out}",
        ),
        n4(
            "convergence-stop",
            OPT,
            "-c [30x30,0.001]: levels end as soon as the convergence value is not above 0.001",
            "-d",
            "3",
            "-i",
            HEAD,
            "-c",
            "[30x30,0.001]",
            "-o",
            "{out}",
        ),
        n4(
            "one-level",
            OPT,
            "-c [15]: one fitting level, no refinement",
            "-d",
            "3",
            "-i",
            HEAD,
            "-c",
            "[15]",
            "-o",
            "{out}",
        ),
        n4(
            "no-brackets",
            OPT,
            "-c 50x50 and -b 200 without brackets set nothing: ITK's one level of 50 iterations "
            "with threshold 0.001, 4 control points",
            "-d",
            "3",
            "-i",
            HEAD,
            "-c",
            "50x50",
            "-b",
            "200",
            "-o",
            "{out}",
        ),
        n4(
            "histogram-100",
            OPT,
            "-t [0.3,0.02,100]: 100 bins (FFT of 256)",
            "-d",
            "3",
            "-i",
            HEAD,
            "-t",
            "[0.3,0.02,100]",
            "-c",
            "[20x20]",
            "-o",
            "{out}",
        ),
        n4(
            "histogram-256",
            OPT,
            "-t [0.15,0.01,256]: 256 bins; log(256)/log(2) rounds above 8, so the FFT has 1024 "
            "points",
            "-d",
            "3",
            "-i",
            HEAD,
            "-t",
            "[0.15,0.01,256]",
            "-c",
            "[20x20]",
            "-o",
            "{out}",
        ),
        n4(
            "histogram-600",
            OPT,
            "-t [0.15,0.01,600]: an FFT of 2048 points, where glibc's sincos is not correctly "
            "rounded for 4 twiddle factors",
            "-d",
            "3",
            "-i",
            HEAD,
            "-t",
            "[0.15,0.01,600]",
            "-c",
            "[20x20]",
            "-o",
            "{out}",
        ),
        n4(
            "output-one-parameter",
            OPT,
            "-o [out]: one bracketed name",
            "-d",
            "3",
            "-i",
            HEAD,
            "-c",
            "[10]",
            "-o",
            "[{out}]",
        ),
        n4(
            "missing-mask",
            OPT,
            "a mask that does not exist: ones are used, but the mask counts as given "
            "(restoration and rescaling run)",
            "-d",
            "3",
            "-i",
            HEAD,
            "-x",
            "{missing:none.nii.gz}",
            "-c",
            "[10x10]",
            "-o",
            "{out}",
        ),
        n4(
            "verbose",
            OPT,
            "-v 1 (stdout is not compared: it holds timings and ITK's object dump)",
            "-d",
            "3",
            "-i",
            HEAD,
            "-v",
            "1",
            "-c",
            "[10x10]",
            "-o",
            "{out}",
        ),
        # -- Inputs -----------------------------------------------------------------------------
        n4("int16-scaled", INP, "scaled int16 input", "-d", "3", "-i", HEAD_I16, "-o", "{out}"),
        n4(
            "outliers",
            INP,
            "hot, negative and zero voxels (NaN and -inf read as 0): non-positive values keep "
            "their value in the log image",
            "-d",
            "3",
            "-i",
            OUTLIERS,
            "-c",
            "[20x20]",
            "-o",
            "{out}",
        ),
        n4(
            "constant",
            INP,
            "a constant image (the histogram has no width)",
            "-d",
            "3",
            "-i",
            CONSTANT,
            "-c",
            "[5]",
            "-o",
            "{out}",
        ),
        n4(
            "ramp",
            INP,
            "values increasing in memory order: ITK's histogram minimum is never set",
            "-d",
            "3",
            "-i",
            RAMP,
            "-c",
            "[10x10]",
            "-o",
            "{out}",
        ),
        n4(
            "las-sform-only",
            INP,
            "an sform-only LAS image",
            "-d",
            "3",
            "-i",
            LAS,
            "-c",
            "[20x20]",
            "-o",
            "{out}",
        ),
        n4(
            "negative-pixdim",
            INP,
            "a qform-only image with a negative pixdim",
            "-d",
            "3",
            "-i",
            NEG_PIXDIM,
            "-c",
            "[20x20]",
            "-o",
            "{out}",
        ),
        n4(
            "dim2",
            INP,
            "-d 2 on a 2D image",
            "-d",
            "2",
            "-i",
            SLICE,
            "-o",
            "[{out},{out2}]",
            outputs=bias,
        ),
        n4(
            "dim2-distance",
            INP,
            "-d 2 with -b [20] and -s 2",
            "-d",
            "2",
            "-i",
            SLICE,
            "-b",
            "[20]",
            "-s",
            "2",
            "-o",
            "{out}",
        ),
        n4(
            "dim4",
            INP,
            "-d 4: the series shrunk along space only, a spline along time too",
            "-d",
            "4",
            "-i",
            SERIES,
            "-s",
            "2",
            "-c",
            "[10x10]",
            "-o",
            "[{out},{out2}]",
            outputs=bias,
        ),
        n4(
            "dim3-of-4d",
            INP,
            "-d 3 on a 4D file: its first volume, with an identity direction",
            "-d",
            "3",
            "-i",
            SERIES,
            "-s",
            "2",
            "-c",
            "[10x10]",
            "-o",
            "{out}",
        ),
        n4(
            "dimension-from-file",
            INP,
            "no -d: the dimension is the file's",
            "-i",
            HEAD,
            "-c",
            "[10]",
            "-o",
            "{out}",
        ),
        n4(
            "bold-first-volume",
            INP,
            "-d 3 on fMRIPrep's preprocessed BOLD series: its first volume",
            "-d",
            "3",
            "-i",
            "{in:openneuro-derivatives/ds000005-fmriprep/sub-01/func/sub-01_task-mixedgamblestask_run-1_space-T1w_desc-preproc_bold.nii.gz}",
            "-b",
            "[200]",
            "-o",
            "{out}",
            tier="standard",
        ),
        # -- Errors -----------------------------------------------------------------------------
        AntsCase(f"{E}/help", E, "--help: the usage, exit 0", PROGRAM, ("--help",), outputs=()),
        n4(
            "shrink-not-integer",
            E,
            "-s 2.5: the parser's Convert<int> throws (ANTs aborts)",
            "-d",
            "3",
            "-i",
            HEAD,
            "-s",
            "2.5",
            "-o",
            "{out}",
            expect="error",
        ),
        n4(
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
        n4(
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
        n4(
            "mesh-wrong-length",
            E,
            "-b [2x2] in 3D: 'Incorrect mesh resolution', exit 1",
            "-d",
            "3",
            "-i",
            HEAD,
            "-b",
            "[2x2]",
            "-o",
            "{out}",
            expect="error",
        ),
        n4(
            "mask-other-grid",
            E,
            "a mask on another grid: ITK throws (ANTs aborts)",
            "-d",
            "3",
            "-i",
            HEAD,
            "-x",
            OTHER_GRID,
            "-o",
            "{out}",
            expect="error",
        ),
    ]


# ------------------------------------------------------------------------------------------------
# Running and comparing


def _load(path: Path) -> tuple[np.ndarray, np.ndarray]:
    import nibabel as nib

    img = nib.load(path)
    return np.asanyarray(img.dataobj.get_unscaled()), img.affine


def _metrics(a: np.ndarray, b: np.ndarray) -> dict[str, float]:
    """How far ``b`` is from ``a``: values differing, largest absolute and relative
    difference, and 1 - NCC (normalized cross-correlation)."""
    af, bf = a.astype(np.float64).ravel(), b.astype(np.float64).ravel()
    d = np.abs(af - bf)
    rel = d / np.maximum(np.abs(af), 1e-30)
    x, y = af - af.mean(), bf - bf.mean()
    den = np.sqrt((x @ x) * (y @ y))
    ncc = float((x @ y) / den) if den > 0 else (1.0 if np.array_equal(af, bf) else 0.0)
    return {
        "differing": int(
            np.count_nonzero(
                np.ascontiguousarray(a).view(np.uint32) != np.ascontiguousarray(b).view(np.uint32)
            )
        ),
        "max_abs": float(d.max()) if d.size else 0.0,
        "max_rel": float(rel.max()) if rel.size else 0.0,
        # Rounding can put the NCC of nearly identical images a hair above 1.
        "one_minus_ncc": max(0.0, 1.0 - ncc),
    }


def _run(tool, c: AntsCase, tmp: Path, name: str, threads: int, **kw):
    outdir = tmp / name
    outdir.mkdir()
    args = [resolve_arg(a, tmp, outdir) for a in c.args]
    outputs = [Path(resolve_arg(o, tmp, outdir)) for o in c.outputs]
    code, out, err = tool(c.program, args, threads, **kw)
    produced = code == 0 and all(o.exists() for o in outputs)
    return code, out, err, outputs, produced


def _describe(m: dict[str, float], size: int) -> str:
    if m["differing"] == 0:
        return "identical"
    return (
        f"{m['differing']} of {size} differ; max |diff| {m['max_abs']:.3g}, max rel "
        f"{m['max_rel']:.2g}, 1 - NCC {m['one_minus_ncc']:.2g}"
    )


def run_case(case: Case, checks: CheckList) -> None:
    c: AntsCase = case.payload
    if ants_bin() is None:
        raise Outcome(SKIPPED, "the ANTs 2.6.5 oracle was not found (LARMORX_ANTS_BIN)")
    with tempfile.TemporaryDirectory(prefix="lx-n4-") as tmp_name:
        tmp = Path(tmp_name)
        a_code, a_out, a_err, a_outputs, a_ok = _run(run_ants, c, tmp, "ants", 1)
        l_code, l_out, l_err, l_outputs, l_ok = _run(run_larmorx, c, tmp, "larmorx", LX_THREADS)
        summary = (
            f"ANTs exit {a_code}: {(a_err or a_out).strip()[-200:]!r}; "
            f"larmorx exit {l_code}: {(l_err or l_out).strip()[-200:]!r}"
        ).replace(str(tmp), "<tmp>")
        if not a_ok and not l_ok:
            raise Outcome(BOTH_ERROR, summary)
        checks("both succeed", a_ok and l_ok, detail=summary)
        if c.expect == "error":
            checks("both reject the arguments", False, detail="expected an error from both")
        if not (a_ok and l_ok) or not a_outputs:
            return
        # larmorx's result does not depend on its thread count.
        *_, l1_outputs, l1_ok = _run(run_larmorx, c, tmp, "larmorx-1", 1)
        same = l1_ok and all(
            _load(p)[0].tobytes() == _load(q)[0].tobytes()
            for p, q in zip(l_outputs, l1_outputs, strict=True)
        )
        checks(
            f"larmorx: {LX_THREADS} threads and 1 thread give the same bits",
            same,
            metric="identical",
            value=str(same),
            threshold="True",
        )
        # ANTs with correctly rounded math (CORE-MATH preloaded): the exact reference.
        cr_outputs: list[Path] | None = None
        if crmath_preload() is not None:
            *_, outs, ok = _run(run_ants, c, tmp, "ants-cr", 1, correctly_rounded=True)
            checks("ANTs with correctly rounded math succeeds", ok)
            cr_outputs = outs if ok else None
        band_outputs: dict[int, list[Path]] = {}

        def band(index: int) -> dict[str, float]:
            if not band_outputs:
                for t in BAND_THREADS:
                    *_, outs, ok = _run(run_ants, c, tmp, f"ants-{t}", t)
                    if ok:
                        band_outputs[t] = outs
            ref = _load(a_outputs[index])[0]
            worst = {"max_abs": 0.0, "max_rel": 0.0, "one_minus_ncc": 0.0}
            for outs in band_outputs.values():
                m = _metrics(ref, _load(outs[index])[0])
                for k in worst:
                    worst[k] = max(worst[k], m[k])
            return worst

        for i, (pa, pl) in enumerate(zip(a_outputs, l_outputs, strict=True)):
            label = ("corrected", "bias field")[i] if len(a_outputs) == 2 else "corrected"
            ha, hb = raw_header(pa), raw_header(pl)
            differing = header_differences(ha, hb)
            checks(
                f"{label}: header (every field)",
                not differing,
                metric="fields differing",
                value=len(differing),
                threshold=0,
                detail="; ".join(differing),
            )
            a, a_aff = _load(pa)
            b, b_aff = _load(pl)
            checks(
                f"{label}: data type and shape",
                a.dtype == b.dtype and a.shape == b.shape,
                detail=f"ANTs {a.dtype} {a.shape}, larmorx {b.dtype} {b.shape}",
            )
            aff = float(np.abs(a_aff - b_aff).max())
            checks(
                f"{label}: affine",
                aff <= AFFINE_ATOL,
                metric="max |diff| (mm)",
                value=f"{aff:.1e}",
                threshold=f"{AFFINE_ATOL:g}",
            )
            if a.shape != b.shape or a.dtype != b.dtype:
                continue
            if cr_outputs is not None:
                cr = _load(cr_outputs[i])[0]
                mc = _metrics(cr, b)
                checks(
                    f"{label}: bit-identical to ANTs with correctly rounded math",
                    mc["differing"] == 0,
                    metric="values differing",
                    value=mc["differing"],
                    threshold=0,
                    detail=_describe(mc, cr.size),
                )
            m = _metrics(a, b)
            checks.add(
                Check(
                    f"{label}: bit-identical to ANTs as built",
                    True,
                    metric="values differing",
                    value=m["differing"],
                    threshold="reported",
                    detail=_describe(m, a.size),
                )
            )
            if m["differing"] == 0:
                continue
            checks(
                f"{label}: NCC with ANTs as built",
                1.0 - m["one_minus_ncc"] > NCC_MIN,
                metric="NCC",
                value=f"{1.0 - m['one_minus_ncc']:.10f}",
                threshold=f"> {NCC_MIN}",
            )
            w = band(i)
            for key, name in (("max_abs", "max |diff|"), ("one_minus_ncc", "1 - NCC")):
                ratio = m[key] / w[key] if w[key] > 0 else (0.0 if m[key] == 0 else float("inf"))
                checks.add(
                    Check(
                        f"{label}: {name} vs ANTs' thread band",
                        True,
                        metric=name,
                        value=f"{m[key]:.3g}",
                        threshold="reported",
                        detail=f"ANTs at {', '.join(map(str, BAND_THREADS))} threads vs 1: up to "
                        f"{w[key]:.3g} (ratio {ratio:.2g})",
                    )
                )


# ------------------------------------------------------------------------------------------------
# Report


def _values(r: CaseResult, suffix: str) -> list:
    return [ch.value for ch in r.checks if ch.name.endswith(suffix)]


def _ratio(r: CaseResult, output: str, metric: str) -> str:
    for ch in r.checks:
        if ch.name == f"{output}: {metric} vs ANTs' thread band":
            return ch.detail.split("ratio ")[-1].rstrip(")")
    return "–"


def highlights(results: list[CaseResult]) -> list[str]:
    compared = [r for r in results if r.status == PASS and _values(r, "to ANTs as built")]
    exact = [r for r in compared if all(v == 0 for v in _values(r, "correctly rounded math"))]
    built = [r for r in compared if all(v == 0 for v in _values(r, "to ANTs as built"))]
    lines = [
        f"**Bit-identical to ANTs with correctly rounded math (CORE-MATH preloaded, one thread): "
        f"{len(exact)} of {len(compared)} compared cases.**",
        f"**Bit-identical to ANTs as built (glibc's math, one thread): {len(built)} of "
        f"{len(compared)}.** The others differ where glibc's `expf`, `logf`, `exp` or `sincos` "
        "is not correctly rounded for a value the run meets; the table compares each "
        "difference with ANTs' own difference between one thread and 2, 3, 4, 8 and 12 threads "
        "(a ratio below 1 is inside ANTs' thread band).",
    ]
    rows = []
    for r in compared:
        for ch in r.checks:
            if ch.name.endswith("bit-identical to ANTs as built") and ch.value:
                output = ch.name.split(":")[0]
                rows.append(
                    (
                        f"`{r.case}`",
                        output,
                        ch.detail,
                        _ratio(r, output, "max |diff|"),
                        _ratio(r, output, "1 - NCC"),
                    )
                )
    if rows:
        lines += [
            "",
            table(
                (
                    "Case",
                    "Output",
                    "Difference from ANTs as built",
                    "max |diff| / band",
                    "(1 - NCC) / band",
                ),
                rows,
            ),
        ]
    return lines


def suite():
    return Suite(
        name="ants-n4",
        title="N4BiasFieldCorrection",
        tool="`larmorx ants N4BiasFieldCorrection` / `lx.ants.n4_bias_field_correction` "
        "(crates larmorx-image, larmorx-ants)",
        reference=f"N4BiasFieldCorrection from {ants_version()}, the binary built by "
        "`scripts/build_ants_oracle.sh`, run with one thread",
        thresholds=[
            ("Exit status", "both produce their outputs, or neither does"),
            ("Header", "every field of the 348-byte header identical"),
            ("Data type, shape, affine", f"identical (affine within {AFFINE_ATOL:g} mm)"),
            ("larmorx's threads", f"{LX_THREADS} threads and 1 thread give the same bits"),
            (
                "Values",
                "bit-identical to ANTs at one thread with correctly rounded math (CORE-MATH "
                "preloaded); with ANTs as built (glibc's math), bit-identity reported and "
                "NCC > 0.995 where they differ",
            ),
            (
                "Reported",
                "the difference from ANTs as built next to ANTs' thread band (ANTs at "
                f"{', '.join(map(str, BAND_THREADS))} threads vs 1 thread)",
            ),
        ],
        cases=lambda tier: cases_for(tier, all_cases()),
        run_case=run_case,
        packages=("nibabel", "scipy"),
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
