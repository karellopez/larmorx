# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's processing steps, how to find them in a working directory, and what larmorx
needs for each.

A **stage** is one step of fMRIPrep's pipeline (e.g. "the final N4 of brain extraction");
it may have several **nodes** (one per BOLD run, per output space, per map-node item, or the
dozens of ImageMath calls of one brain-extraction step). Nodes are matched by their path in
the working directory with the run, space and map-node parts removed (:func:`normalise`).

Nodes that ran a program larmorx has are compared (:data:`STAGES`); every other node that ran
a neuroimaging program, or a Python step larmorx will need, goes into the roadmap
(:data:`MISSING`).
"""

from __future__ import annotations

import re
from dataclasses import dataclass

from larmorx_validation.stages.oracle import Node

#: Programs larmorx runs with the original program's arguments (``larmorx <family> <program>``).
CLI_PROGRAMS = {
    "antsApplyTransforms": "ants",
    "DenoiseImage": "ants",
    "ImageMath": "ants",
    "MultiplyImages": "ants",
    "N4BiasFieldCorrection": "ants",
    "ResampleImage": "ants",
    "ResampleImageBySpacing": "ants",
    "SmoothImage": "ants",
    "ThresholdImage": "ants",
    "3dTshift": "afni",
}


@dataclass(frozen=True)
class Stage:
    """A step of fMRIPrep's pipeline that larmorx can run."""

    id: str
    group: str  # "anatomical", "fieldmap" or "functional"
    title: str
    #: Regular expression on the normalised node path (:func:`normalise`).
    pattern: str
    #: ``cli`` (a program run with its own arguments), ``tshift`` (3dTshift, both
    #: implementations), ``hmc`` or ``resample``.
    kind: str
    #: larmorx's tool.
    tool: str
    #: PLAN.md §11.3's row for intensity outputs (label outputs use the mask row).
    threshold: str
    #: When the stage does not run (a note for the report).
    absent: str = ""


#: PLAN.md §11.3 thresholds, by name: (description, metric, value).
THRESHOLDS = {
    "arithmetic": ("deterministic arithmetic (f32)", "max |diff| / max |ref|", 1e-5),
    "itk-interpolation": ("interpolation (ITK)", "max |diff| / max |ref|", 1e-4),
    "scipy-interpolation": ("interpolation (SciPy)", "max |diff| / max |ref|", 1e-6),
    "n4-denoise": ("N4 / denoise", "NCC", 0.995),
    "masks": ("masks", "Dice", 0.98),
    # docs/validation/afni-tshift.md (specs/3dTshift.md §9): float32 outputs within 1e-5 of
    # the largest |AFNI output|, integer outputs within 1
    "tshift": ("slice timing (3dTshift)", "max |diff| / max |ref|; integers max |diff|", 1e-5),
    "motion": ("motion parameters", "median |diff|; FD r", (0.05, 0.05, 0.99)),
    "bold": (
        "preprocessed BOLD",
        "temporal-mean NCC; tSNR r; median time-series r",
        (0.99, 0.98, 0.98),
    ),
}

STAGES: tuple[Stage, ...] = (
    # --- anatomical ------------------------------------------------------------------------
    Stage(
        "A01",
        "anatomical",
        "Denoise the T1w (`DenoiseImage -n Rician`)",
        r"anat_template_wf/denoise$",
        "cli",
        "lx.ants.denoise_image",
        "n4-denoise",
    ),
    Stage(
        "A02",
        "anatomical",
        "N4 before the anatomical template (ANTs defaults)",
        r"anat_template_wf/n4_correct$",
        "cli",
        "lx.ants.n4_bias_field_correction",
        "n4-denoise",
        absent="runs only with more than one T1w; this subject has one",
    ),
    Stage(
        "A03",
        "anatomical",
        "Brain extraction: truncate intensities (`TruncateImageIntensity`)",
        r"brain_extraction_wf/truncate_images$",
        "cli",
        "lx.ants.image_math",
        "arithmetic",
    ),
    Stage(
        "A04",
        "anatomical",
        "Brain extraction: N4, first pass",
        r"brain_extraction_wf/inu_n4$",
        "cli",
        "lx.ants.n4_bias_field_correction",
        "n4-denoise",
    ),
    Stage(
        "A05",
        "anatomical",
        "Brain extraction: Laplacian of the T1w and the template",
        r"brain_extraction_wf/lap_(tmpl|target)$",
        "cli",
        "lx.ants.image_math",
        "arithmetic",
    ),
    Stage(
        "A06",
        "anatomical",
        "Brain extraction: template priors to the T1w (Gaussian, inverse warp)",
        r"brain_extraction_wf/map_(brainmask|wmmask)$",
        "cli",
        "lx.ants.apply_transforms",
        "itk-interpolation",
    ),
    Stage(
        "A07",
        "anatomical",
        "Brain extraction: threshold the brain prior (`ThresholdImage`)",
        r"brain_extraction_wf/thr_brainmask$",
        "cli",
        "lx.ants.threshold_image",
        "masks",
    ),
    Stage(
        "A08",
        "anatomical",
        "Brain extraction: mask and label morphology (`ImageMath` MD/ME/GetLargestComponent/"
        "FillHoles/PadImage/addtozero, `MultiplyImages`)",
        r"atropos_wf/(?!01_atropos|inu_n4_final)[^/]+$",
        "cli",
        "lx.ants.image_math",
        "masks",
    ),
    Stage(
        "A09",
        "anatomical",
        "Brain extraction: final N4 (weighted by the WM posterior)",
        r"atropos_wf/inu_n4_final$",
        "cli",
        "lx.ants.n4_bias_field_correction",
        "n4-denoise",
    ),
    Stage(
        "A10",
        "anatomical",
        "Normalisation: truncate the moving T1w",
        r"register_template_wf/trunc_mov$",
        "cli",
        "lx.ants.image_math",
        "arithmetic",
    ),
    Stage(
        "A11",
        "anatomical",
        "Anatomical outputs to MNI (`.h5` composite; Lanczos, MultiLabel, Gaussian)",
        r"(ds_std_volumes_wf/anat2std_\w+|anat_reports_wf/(t1w|mask)_std)$",
        "cli",
        "lx.ants.apply_transforms",
        "itk-interpolation",
    ),
    # --- susceptibility distortion (field maps) -----------------------------------------------
    Stage(
        "S01",
        "fieldmap",
        "Field map: N4 of the field-map reference (sdcflows' brain extraction)",
        r"fmap_preproc_wf/.*brainextraction_wf/n4$",
        "cli",
        "lx.ants.n4_bias_field_correction",
        "n4-denoise",
        absent="no field map estimated (no estimator assigned to a BOLD run)",
    ),
    Stage(
        "S02",
        "fieldmap",
        "Unwarp the HMC boldref for coregistration (`ResampleSeries` with the field map)",
        r"bold_fit_wf/unwarp_boldref$",
        "resample",
        "lx.transforms.resample_series",
        "scipy-interpolation",
        absent="no field map applied to a BOLD run",
    ),
    Stage(
        "S03",
        "fieldmap",
        "Field-map report: the field map and its reference to the boldref (Lanczos, `--float 1`)",
        r"func_fit_reports_wf/(fmap_boldref|fmapref_boldref)$",
        "cli",
        "lx.ants.apply_transforms",
        "itk-interpolation",
        absent="no field map applied to a BOLD run",
    ),
    # --- functional --------------------------------------------------------------------------
    Stage(
        "F01",
        "functional",
        "Head-motion estimation (mcflirt on the raw BOLD, to the HMC boldref)",
        r"bold_hmc_wf/mcflirt$",
        "hmc",
        "lx.mri.hmc",
        "motion",
    ),
    Stage(
        "F02",
        "functional",
        "Slice-timing correction (`3dTshift`)",
        # fMRIPrep 25.2.6 names the node slice_timing_correction (fmriprep/workflows/bold/
        # stc.py:121-131); the catalogue's first pattern (slice_timing_corrected) never
        # matched it.
        r"bold_stc_wf/slice_timing_correct(ion|ed)$",
        "tshift",
        "lx.afni.tshift",
        "tshift",
        absent="no `SliceTiming` in the BOLD runs' metadata, so fMRIPrep skips it",
    ),
    Stage(
        "F03",
        "functional",
        "Coregistration reference: N4 of the HMC boldref",
        r"enhance_and_skullstrip_bold_wf/n4_correct$",
        "cli",
        "lx.ants.n4_bias_field_correction",
        "n4-denoise",
    ),
    Stage(
        "F04",
        "functional",
        "Coregistration reference: MNI brain prior to the boldref (Linear)",
        r"enhance_and_skullstrip_bold_wf/map_brainmask$",
        "cli",
        "lx.ants.apply_transforms",
        "itk-interpolation",
    ),
    Stage(
        "F05",
        "functional",
        "One-shot resampling to the boldref (HMC)",
        r"bold_native_wf/boldref_bold$",
        "resample",
        "lx.transforms.resample_series",
        "scipy-interpolation",
    ),
    Stage(
        "F06",
        "functional",
        "One-shot resampling to T1w (HMC + coregistration)",
        r"bold_anat_wf/resample$",
        "resample",
        "lx.transforms.resample_series",
        "scipy-interpolation",
    ),
    Stage(
        "F07",
        "functional",
        "One-shot resampling to MNI res-2 (HMC + coregistration + `.h5` warp)",
        r"bold_std_wf/resample$",
        "resample",
        "lx.transforms.resample_series",
        "scipy-interpolation",
    ),
    Stage(
        "F08",
        "functional",
        "boldref and BOLD mask to T1w (Lanczos, MultiLabel)",
        r"ds_bold_t1_wf/resample_(ref|mask)$",
        "cli",
        "lx.ants.apply_transforms",
        "itk-interpolation",
    ),
    Stage(
        "F09",
        "functional",
        "boldref and BOLD mask to MNI (Lanczos, MultiLabel, `.h5`)",
        r"ds_bold_std_wf/resample_(ref|mask)$",
        "cli",
        "lx.ants.apply_transforms",
        "itk-interpolation",
    ),
    Stage(
        "F10",
        "functional",
        "Confound masks to the boldref (Gaussian, MultiLabel, inverse)",
        r"bold_confounds_wf/(acc_msk_tfm|t1w_mask_tfm)$",
        "cli",
        "lx.ants.apply_transforms",
        "itk-interpolation",
    ),
    Stage(
        "F11",
        "functional",
        "Carpet-plot segmentation to the boldref (MultiLabel, `.h5`, `-u int`)",
        r"carpetplot_wf/resample_parc$",
        "cli",
        "lx.ants.apply_transforms",
        "itk-interpolation",
    ),
    Stage(
        "F12",
        "functional",
        "Report: T1w and WM mask to the boldref (Lanczos, NearestNeighbor)",
        r"func_fit_reports_wf/(t1w_boldref|boldref_wm)$",
        "cli",
        "lx.ants.apply_transforms",
        "itk-interpolation",
    ),
)


@dataclass(frozen=True)
class Missing:
    """A tool fMRIPrep runs that larmorx does not have yet."""

    tool: str
    #: The programs (first word of ``command.txt``) or node names that identify it.
    programs: tuple[str, ...]
    needs: str
    milestone: str
    #: Matched on node names (Python nodes that run programs internally).
    names: tuple[str, ...] = ()


MISSING: tuple[Missing, ...] = (
    Missing(
        "antsRegistration",
        ("antsRegistration",),
        "rigid/affine (BOLD premask) and SyN (brain extraction, MNI normalisation)",
        "A4 (linear), A5 (SyN)",
        names=("registration",),
    ),
    Missing("antsAI", ("antsAI",), "affine initialisation by rotation search", "A4"),
    Missing("Atropos", ("Atropos",), "K-means + MRF segmentation in brain extraction", "A6"),
    Missing(
        "3dvolreg",
        ("3dvolreg",),
        "aligns the reference volumes before the median "
        "(niworkflows' RobustAverage, inside the HMC-reference node)",
        "AFNI",
        names=("gen_avg",),
    ),
    Missing("3dUnifize", ("3dUnifize",), "T2-contrast intensity unifying of the boldref", "AFNI"),
    Missing("3dAutomask", ("3dAutomask",), "BOLD brain mask, second pass", "AFNI"),
    Missing("BET", ("bet",), "BOLD brain mask, first pass", "FSL (clean-room + `larmorx-nc`)"),
    Missing("FAST", ("fast",), "GM/WM/CSF segmentation and probability maps", "FSL"),
    Missing("FLIRT-BBR", ("flirt",), "boundary-based BOLD-to-T1w registration", "FSL"),
    Missing("fslmaths", ("fslmaths",), "mask product of the two BOLD masks", "FSL (trivial)"),
    Missing("mri_coreg", ("mri_coreg",), "BOLD-to-T1w initialisation", "FreeSurfer"),
    Missing(
        "lta_convert",
        ("lta_convert",),
        "transform format conversion (LTA <-> FSL)",
        "FreeSurfer (trivial; nitransforms has it)",
    ),
    Missing(
        "topup",
        ("topup",),
        "PEPOLAR field-map estimation (sdcflows' PEPOLAR workflow)",
        "FSL (clean-room + `larmorx-nc`)",
    ),
    Missing(
        "prelude",
        ("prelude",),
        "phase unwrapping of the phase-difference field map",
        "FSL (clean-room + `larmorx-nc`)",
    ),
    Missing(
        "mri_robust_template",
        ("mri_robust_template",),
        "anatomical template of several T1w images",
        "FreeSurfer",
    ),
    Missing(
        "field-map B-splines",
        (),
        "sdcflows' B-spline fit of the field (`BSplineApprox`, `TOPUPCoeffReorient`) and "
        "fMRIPrep's `ReconstructFieldmap` (the field from its coefficients on a target grid, "
        "the input of `ResampleSeries`)",
        "larmorprepx (Rust: B-spline evaluation)",
        names=("bs_filter", "fix_coeff", "fmap_recon", "boldref_fmap"),
    ),
    Missing(
        "field-map preparation",
        (),
        "sdcflows' Python steps: phase difference to Hz (`calc_phdiff`, `phmap2rads`, "
        "`compfmap`), PEPOLAR series sorted, padded, regridded and averaged, the field-map "
        "reference's brain mask, readout times and distortion parameters",
        "larmorprepx (numpy)",
        names=(
            "calc_phdiff",
            "phmap2rads",
            "compfmap",
            "check_register",
            "sort_pe_blips",
            "pad_blip_slices",
            "regrid",
            "to_las",
            "from_las",
            "from_las_fmap",
            "concat_blips",
            "runwise_avg",
            "setwise_avg",
            "ref_average",
            "magmrg",
            "masker",
            "clipper_pre",
            "clipper_post",
            "readout_time",
            "distortion_params",
        ),
    ),
    Missing(
        "confounds",
        (),
        "DVARS, CompCor (a/t/crown), global signals, expansion, spikes "
        "(fMRIPrep's Python); FD, RMSD and motion parameters are computed here from "
        "larmorx's HMC",
        "larmorprepx (numpy)",
        names=(
            "dvars",
            "acompcor",
            "tcompcor",
            "crowncompcor",
            "signals",
            "model_expansion",
            "spike_regressors",
            "acc_masks",
            "motion_params",
            "fdisp",
            "rmsd",
        ),
    ),
)


def normalise(path: str) -> tuple[str, dict[str, str]]:
    """A node path without its subject, run, space and map-node parts, and those parts.

    ``fmriprep_25_2_wf/sub_01_wf/bold_task_x_run_01_wf/bold_std_wf/_in_tuple_MNI.res2/resample``
    becomes ``bold_wf/bold_std_wf/resample`` with ``{"run": "task_x_run_01", "space": ...}``.
    """
    parts = path.split("/")
    keys: dict[str, str] = {}
    out: list[str] = []
    for i, p in enumerate(parts):
        if i == 0 and re.fullmatch(r"fmriprep_\w+_wf", p):
            continue
        if re.fullmatch(r"sub_\w+_wf", p) and i <= 1:
            keys["subject"] = p[4:-3]
            continue
        m = re.fullmatch(r"bold_(\w+)_wf", p)
        if m and ("run" in p or "task" in p) and not keys.get("run"):
            keys["run"] = m.group(1)
            out.append("bold_wf")
            continue
        m = re.fullmatch(r"_(?:in_tuple|template)_(.+)", p)
        if m:
            keys["space"] = m.group(1)
            continue
        if p == "mapflow":
            continue
        if i > 0 and parts[i - 1] == "mapflow":
            keys["item"] = p
            continue
        out.append(p)
    return "/".join(out), keys


def stage_of(node: Node) -> Stage | None:
    """The stage a node belongs to, if larmorx can run it."""
    norm, _ = normalise(node.path)
    for stage in STAGES:
        if not re.search(stage.pattern, norm):
            continue
        if stage.kind == "cli" and node.program not in CLI_PROGRAMS:
            continue
        if stage.kind == "tshift" and node.program != "3dTshift":
            continue
        if stage.kind == "hmc" and node.program != "mcflirt":
            continue
        if stage.kind == "resample" and node.interface != "resampling":
            continue
        return stage
    return None


def missing_of(node: Node) -> Missing | None:
    """The roadmap entry a node belongs to, if it runs something larmorx does not have."""
    for m in MISSING:
        if node.program and node.program in m.programs:
            return m
    for m in MISSING:
        if node.name in m.names:
            return m
    return None


__all__ = [
    "CLI_PROGRAMS",
    "MISSING",
    "STAGES",
    "THRESHOLDS",
    "Missing",
    "Stage",
    "missing_of",
    "normalise",
    "stage_of",
]
