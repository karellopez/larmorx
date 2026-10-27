# SPDX-License-Identifier: Apache-2.0
"""The data panel's cases on the ANTs tools' pages (the re-validation of 2026-10-26, part a).

Each page keeps every existing example and gains, from here: its panel examples
(:mod:`~larmorx_validation.evidence.panel_examples`; two or three key datasets as whole volumes,
the others as three full-resolution slices), the cases of its panel records
(``docs/validation/<record>-panel.md``: fMRIPrep's calls on the fourteen oracle runs and the
panel's images) in its case explorer, and the inventory of fMRIPrep's nodes as a table. The
choice of cases follows the panel's criteria (larmorx-testdata ``panel.toml``, ``covers``): one
case per distinct difficulty, not one per image.
"""

from __future__ import annotations

import functools

from larmorx_validation.evidence import core
from larmorx_validation.evidence.panel_examples import Spec, example, transform_table
from larmorx_validation.parity import fmriprep_nodes
from larmorx_validation.parity import panel_images as P


@functools.cache
def data(dataset: str) -> dict:
    """A panel dataset's credit facts (dataset, subject, licence) from larmorx-testdata's
    catalog (``credits.attach`` checks the licence again)."""
    import larmorx_testdata as td

    for ds in td.panel():
        if ds.id == dataset:
            return {
                "dataset": f"OpenNeuro {ds.id}",
                "subject": ds.subject,
                "licence": ds.collection.licence,
            }
    raise KeyError(dataset)


def node_call(program: str, run: str, norm: str) -> fmriprep_nodes.NodeCall:
    """fMRIPrep's call of ``program`` in ``run`` at the normalised node path ``norm``."""
    for c in fmriprep_nodes.calls(program):
        if c.run == run and c.norm == norm:
            return c
    raise FileNotFoundError(f"{run}: no {program} node {norm}")


def _cmd(program: str, args) -> str:
    import re

    def name(m: re.Match[str]) -> str:
        return m.group(0).rsplit("/", 1)[-1]

    text = " ".join(args).replace("{in:", "").replace("{out:", "").replace("}", "")
    return f"{program} " + re.sub(r"/[^\s,\[\]]+", name, text)


# ----------------------------------------------------------------------------- the specs

RUN_STROKE = "ds004884-sub-M2079-noreconall"
RUN_TUMOUR = "ds005003-sub-RS001-noreconall"
RUN_TODDLER = "ds000228-sub-pixar008-noreconall"
RUN_OLDER = "ds003592-sub-280-noreconall"
RUN_7T = "ds005454-sub-16-noreconall"


def _programs() -> list[Spec]:
    T = "ants-programs"
    trunc = ("3", "{out:out.nii.gz}", "TruncateImageIntensity", P.MP2RAGE_7T.arg, "0.01", "0.999",
             "256")  # fmt: skip
    smooth = ("3", P.FLAIR_OLDER.arg, "1.5", "{out:out.nii.gz}", "1")
    otsu = ("3", P.MP2RAGE_7T.arg, "{out:out.nii.gz}", "Otsu", "4")
    mult = ("3", P.STROKE_T2W.arg, P.STROKE_LESION.arg, "{out:out.nii.gz}")
    lap = node_call("ImageMath", RUN_TODDLER, "anat_fit_wf/brain_extraction_wf/lap_target")
    lap_in = next(a for a in lap.args if a.startswith("/"))
    return [
        Spec(T, "panel-truncate-7t-mp2rage", "panel-truncate-7t",
             "Data panel: sMRIPrep's truncation on the raw 7 T MP2RAGE",
             "ImageMath's TruncateImageIntensity with sMRIPrep's arguments (0.01 0.999 256: "
             "the brain extraction's first call) on the panel's largest anatomical image, the raw "
             "7 T MP2RAGE uniform image of ds005454 sub-16 (0.8 mm, 224 × 272 × 288), whose noisy "
             "background is as bright as the head: the histogram the quantiles come from is "
             "mostly noise.",
             "ImageMath", trunc, (("out", "out.nii.gz", "truncated", ""),), P.MP2RAGE_7T.arg,
             (("input", P.MP2RAGE_7T.arg, "ds005454 sub-16, raw 7 T MP2RAGE"),),
             (data("ds005454"),), band=(), command=_cmd("ImageMath", trunc)),
        Spec(T, "panel-smooth-flair", "panel-smooth-flair",
             "Data panel: SmoothImage on a FLAIR of 3 mm slices",
             "SmoothImage with a Gaussian of 1.5 mm on an 82-year-old's FLAIR (ds003592 sub-280: "
             "0.42 × 0.42 × 3 mm voxels, 384 × 512 × 44): ITK's recursive Gaussian with a sigma "
             "of 3.6 voxels in-plane and half a voxel across the slices.",
             "SmoothImage", smooth, (("out", "out.nii.gz", "smoothed", ""),), P.FLAIR_OLDER.arg,
             (("input", P.FLAIR_OLDER.arg, "ds003592 sub-280, FLAIR"),),
             (data("ds003592"),), band=(), command=_cmd("SmoothImage", smooth)),
        Spec(T, "panel-otsu-7t-mp2rage", "panel-otsu-7t",
             "Data panel: Otsu's thresholds of the raw 7 T MP2RAGE",
             "ThresholdImage's Otsu with 4 thresholds on the raw 7 T MP2RAGE (five classes, the "
             "noisy background among them), drawn over the image.",
             "ThresholdImage", otsu, (("out", "out.nii.gz", "classes", ""),), P.MP2RAGE_7T.arg,
             (("input", P.MP2RAGE_7T.arg, "ds005454 sub-16, raw 7 T MP2RAGE"),),
             (data("ds005454"),), band=(), labels=("out",), command=_cmd("ThresholdImage", otsu)),
        Spec(T, "panel-multiply-stroke", "panel-multiply-stroke",
             "Data panel: a stroke's T2w times its manual lesion mask",
             "MultiplyImages on ds004884 sub-M2079's T2w and the authors' manual lesion mask "
             "(uint8, the same grid): the lesion's intensities kept, everything else 0.",
             "MultiplyImages", mult, (("out", "out.nii.gz", "product", ""),), P.STROKE_T2W.arg,
             (("input", P.STROKE_T2W.arg, "ds004884 sub-M2079, T2w"),
              ("lesion", P.STROKE_LESION.arg, "the manual lesion mask")),
             (data("ds004884"),), band=(), command=_cmd("MultiplyImages", mult)),
        Spec(T, "panel-laplacian-toddler", "panel-laplacian-toddler",
             "Data panel: fMRIPrep's Laplacian of a 3.5-year-old's T1w",
             "fMRIPrep 25.2.6's own call (sMRIPrep's brain extraction, lap_target: ImageMath "
             "Laplacian 1.5 1) on the N4-corrected T1w of ds000228 sub-pixar008, a 3.5-year-old "
             "(a small head, immature contrast), its command.txt and input from the oracle run.",
             "ImageMath", lap.args, (("out", lap.outputs[0][5:-1], "Laplacian", ""),), lap_in,
             (("input", lap_in, "ds000228 sub-pixar008, N4-corrected T1w (fMRIPrep's)"),),
             (data("ds000228"),), band=(), command=_cmd("ImageMath", lap.command().split()[1:]),
             node=f"`{fmriprep_nodes.label(lap.node)}` of the oracle run {RUN_TODDLER}"),
    ]  # fmt: skip


def _n4_spec(cid, eid, title, summary, image: P.PanelImage, extra=(), view="slices",
             inputs=None, dataset=None) -> Spec:  # fmt: skip
    args = ("-d", "3", "--bspline-fitting", "[", "200", "]", "--convergence", "[",
            "50x50x50x50x50,", "1e-07", "]", "--shrink-factor", "4", "--input-image", image.arg,
            "--output", "[", "{out:corrected.nii.gz},", "{out:bias.nii.gz}", "]", *extra)  # fmt: skip
    return Spec("ants-n4", cid, eid, title, summary, "N4BiasFieldCorrection", args,
                (("corrected", "corrected.nii.gz", "corrected", ""),
                 ("bias", "bias.nii.gz", "bias field", "")),
                image.arg, inputs or (("input", image.arg, f"{image.dataset} {image.subject}"),),
                (data(dataset or image.dataset),), view=view, band=(8,),
                command=_cmd("N4BiasFieldCorrection", args))  # fmt: skip


def _n4() -> list[Spec]:
    final = node_call("N4BiasFieldCorrection", RUN_TUMOUR,
                      "anat_fit_wf/brain_extraction_wf/atropos_wf/inu_n4_final")  # fmt: skip
    f_in = final.args[final.args.index("--input-image") + 1]
    bold = node_call(
        "N4BiasFieldCorrection",
        "ds004958-sub-fsm64ps-noreconall",
        "bold_wf/bold_fit_wf/enhance_and_skullstrip_bold_wf/n4_correct",
    )
    b_in = bold.args[bold.args.index("--input-image") + 1]
    return [
        Spec("ants-n4", "panel-fmriprep-boldref-mb8", "panel-boldref-mb8",
             "Data panel: fMRIPrep's N4 of a multiband-8 BOLD reference",
             "fMRIPrep 25.2.6's own call (the BOLD reference's correction before its skull "
             "stripping, n4_correct: -b [200], -r without a mask, its weight image) on ds004958 "
             "sub-fsm64ps's multiband-8 run (600 volumes of 104 × 104 × 72 at 2 mm): its "
             "command.txt and inputs from the oracle run. A key dataset, shown as a whole volume.",
             "N4BiasFieldCorrection", bold.args,
             tuple(("corrected", o[5:-1], "corrected", "") for o in bold.outputs[:1]),
             b_in, (("input", b_in, "ds004958 sub-fsm64ps, BOLD reference (fMRIPrep's)"),),
             (data("ds004958"),), view="volume", band=(8,),
             command=_cmd("N4BiasFieldCorrection", bold.command().split()[1:]),
             node=f"`{fmriprep_nodes.label(bold.node)}` of the oracle run {bold.run}"),
        _n4_spec("panel-final-7t-mp2rage", "panel-7t",
                 "Data panel: sMRIPrep's final N4 options on the raw 7 T MP2RAGE",
                 "N4 with sMRIPrep's final options (-b [200] -c [50x50x50x50x50,1e-7] -s 4, the "
                 "bias field saved) on the raw 7 T MP2RAGE uniform image of ds005454 sub-16, the "
                 "panel's largest anatomical image: its background noise as bright as the head, "
                 "fitted with it (no mask).",
                 P.MP2RAGE_7T),
        Spec("ants-n4", "panel-fmriprep-final-tumour", "panel-tumour",
             "Data panel: fMRIPrep's final N4 of a brain with a tumour",
             "fMRIPrep 25.2.6's own call (sMRIPrep's final N4, inu_n4_final, its white-matter "
             "weight image) on ds005003 sub-RS001, a 70 mL right frontal oligodendroglioma: its "
             "command.txt and inputs from the oracle run.",
             "N4BiasFieldCorrection", final.args,
             tuple((o[5:-1].split("_")[-1].removesuffix(".nii.gz"), o[5:-1],
                    "corrected" if "corrected" in o else "bias field", "")
                   for o in final.outputs),
             f_in, (("input", f_in, "ds005003 sub-RS001, denoised T1w (fMRIPrep's)"),),
             (data("ds005003"),), band=(8,),
             command=_cmd("N4BiasFieldCorrection", final.command().split()[1:]),
             node=f"`{fmriprep_nodes.label(final.node)}` of the oracle run {RUN_TUMOUR}"),
        _n4_spec("panel-final-flair", "panel-flair",
                 "Data panel: N4 on a FLAIR of 3 mm slices",
                 "The same options on an 82-year-old's FLAIR (ds003592 sub-280, 0.42 × 0.42 × 3 "
                 "mm): shrink 4 leaves 11 slices for the fit.", P.FLAIR_OLDER),
        _n4_spec("panel-final-tumour-t2w", "panel-tumour-t2w",
                 "Data panel: N4 on a T2w of 6 mm slices with a tumour",
                 "The same options on the tumour's T2w (ds005003 sub-RS001: 5 mm slices 6 mm "
                 "apart): shrink 4 leaves 7 slices for the fit.", P.TUMOUR_T2W),
        _n4_spec("panel-final-unit1", "panel-unit1",
                 "Data panel: N4 on a 3 T MP2RAGE UNIT1",
                 "The same options on a 68-year-old's 3 T MP2RAGE UNIT1 (ds004958 sub-fsm64ps), "
                 "its salt-and-pepper background fitted with the head.", P.UNIT1_3T),
    ]  # fmt: skip


def _denoise() -> list[Spec]:
    def d(cid, eid, title, summary, image, view="slices", noise="Rician"):
        args = ("-i", image.arg, "-n", noise, "-o", "[", "{out:denoised.nii.gz},",
                "{out:noise.nii.gz}", "]", "-s", "1")  # fmt: skip
        return Spec("ants-denoise", cid, eid, title, summary, "DenoiseImage", args,
                    (("denoised", "denoised.nii.gz", "denoised", ""),
                     ("noise", "noise.nii.gz", "noise", "")),
                    image.arg, (("input", image.arg, f"{image.dataset} {image.subject}"),),
                    (data(image.dataset),), view=view, band=(8,),
                    command=_cmd("DenoiseImage", args))  # fmt: skip

    motion = node_call("DenoiseImage", "ds004173-sub-603309-noreconall",
                       "anat_fit_wf/anat_template_wf/denoise")  # fmt: skip
    for c in fmriprep_nodes.calls("DenoiseImage"):
        if c.run == "ds004173-sub-603309-noreconall" and "headmotion2" in " ".join(c.args):
            motion = c
    m_in = motion.args[motion.args.index("-i") + 1]
    return [
        d("panel-rician-7t-mp2rage", "panel-7t",
          "Data panel: sMRIPrep's denoising of the raw 7 T MP2RAGE",
          "sMRIPrep's DenoiseImage command (-n Rician -s 1, the noise saved) on the raw 7 T "
          "MP2RAGE of ds005454 sub-16, the panel's largest anatomical image (17.5 million voxels, "
          "a noisy background).", P.MP2RAGE_7T),
        Spec("ants-denoise", "panel-fmriprep-motion", "panel-motion",
             "Data panel: fMRIPrep's denoising of a T1w with strong head motion",
             "fMRIPrep 25.2.6's own call (sMRIPrep's denoise of each T1w) on ds004173 "
             "sub-603309's MPRAGE acquired with strong instructed head motion (rated 3): its "
             "command.txt and input from the oracle run.",
             "DenoiseImage", motion.args,
             (("denoised", motion.outputs[0][5:-1], "denoised", ""),), m_in,
             (("input", m_in, "ds004173 sub-603309, MPRAGE with strong motion"),),
             (data("ds004173"),), band=(8,),
             command=_cmd("DenoiseImage", motion.command().split()[1:]),
             node=f"`{fmriprep_nodes.label(motion.node)}` of the oracle run {motion.run}"),
        d("panel-rician-flair", "panel-flair", "Data panel: denoising a FLAIR of 3 mm slices",
          "The same command on an 82-year-old's FLAIR (ds003592 sub-280: 384 × 512 × 44 voxels "
          "of 0.42 × 0.42 × 3 mm).", P.FLAIR_OLDER),
        d("panel-rician-child-t2w", "panel-child-t2w", "Data panel: denoising a child's T2w",
          "The same command on a 7.5-year-old's 0.8 mm T2w (ds000256 sub-CTS203).", P.CHILD_T2W),
    ]  # fmt: skip


def _atropos() -> list[Spec]:
    call = next(c for c in fmriprep_nodes.calls("Atropos") if c.run == RUN_TUMOUR)
    args = list(call.args)
    i = args.index("--output")
    args[i + 1] = "[{out:seg.nii.gz},{out:POSTERIOR_%02d.nii.gz}]"
    image = args[args.index("--intensity-image") + 1]
    outs = (("seg", "seg.nii.gz", "classes", ""),
            *((f"post{k}", f"POSTERIOR_0{k}.nii.gz", f"posterior {k}", "") for k in (1, 2, 3)))  # fmt: skip
    stroke = node_call("Atropos", RUN_STROKE,
                       "anat_fit_wf/brain_extraction_wf/atropos_wf/01_atropos")  # fmt: skip
    s_args = list(stroke.args)
    j = s_args.index("--output")
    s_args[j + 1] = "[{out:seg.nii.gz},{out:POSTERIOR_%02d.nii.gz}]"
    s_image = s_args[s_args.index("--intensity-image") + 1]
    return [
        Spec("ants-atropos", "panel-fmriprep-tumour", "panel-tumour",
             "Data panel: fMRIPrep's Atropos on a brain with a tumour",
             "fMRIPrep 25.2.6's own call (sMRIPrep's brain extraction, 01_atropos: K-means "
             "initialisation, 3 classes, MRF 0.1, 3 iterations) on ds005003 sub-RS001's "
             "N4-corrected T1w with a 70 mL right frontal oligodendroglioma, in the brain "
             "extraction's mask: its command.txt and inputs from the oracle run.",
             "Atropos", tuple(args), outs, image,
             (("input", image, "ds005003 sub-RS001, N4-corrected T1w (fMRIPrep's)"),),
             (data("ds005003"),), band=(8,), labels=("seg",),
             command=_cmd("Atropos", call.command().split()[1:]),
             node=f"`{fmriprep_nodes.label(call.node)}` of the oracle run {RUN_TUMOUR}"),
        Spec("ants-atropos", "panel-fmriprep-stroke", "panel-stroke",
             "Data panel: fMRIPrep's Atropos on a brain with a stroke",
             "fMRIPrep 25.2.6's own call on ds004884 sub-M2079, a chronic left-hemisphere stroke "
             "(176 mL): the lesion's fluid-filled tissue enters the K-means classes.",
             "Atropos", tuple(s_args), outs, s_image,
             (("input", s_image, "ds004884 sub-M2079, N4-corrected T1w (fMRIPrep's)"),),
             (data("ds004884"),), band=(8,), labels=("seg",),
             command=_cmd("Atropos", stroke.command().split()[1:]),
             node=f"`{fmriprep_nodes.label(stroke.node)}` of the oracle run {RUN_STROKE}"),
    ]  # fmt: skip


def _apply_transforms() -> list[Spec]:
    root = fmriprep_nodes.root()

    def run_file(run: str, pattern: str) -> str:
        return str(sorted((root / run / "out").glob(pattern))[0])

    tpl = "{in:templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-01_T1w.nii.gz}"
    seven = ("-d", "3", "-i", P.MP2RAGE_7T.arg, "-r", tpl,
             "-t", run_file(RUN_7T, "sub-*/anat/*_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5"),
             "-t", run_file(RUN_7T, "sub-*/anat/*_acq-mp2rage_from-orig_to-T1w_mode-image_xfm.txt"),
             "-n", "LanczosWindowedSinc", "-o", "{out:out.nii.gz}")  # fmt: skip
    t1w = run_file(RUN_OLDER, "sub-*/ses-*/anat/*_desc-preproc_T1w.nii.gz")
    flair = ("-d", "3", "-i", P.FLAIR_OLDER.arg, "-r", t1w, "-t", "identity",
             "-n", "LanczosWindowedSinc", "-o", "{out:out.nii.gz}")  # fmt: skip
    roi = str(sorted(fmriprep_nodes.oracle_run(RUN_STROKE).mounts["/data"].glob(
        "sub-*/ses-*/anat/*_label-lesion_roi.nii.gz"))[0])  # fmt: skip
    mni2 = run_file(
        RUN_STROKE, "sub-*/ses-*/anat/*_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz"
    )
    lesion = ("-d", "3", "-i", roi, "-r", mni2,
              "-t", run_file(RUN_STROKE, "sub-*/ses-*/anat/*_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5"),
              "-n", "GenericLabel", "-u", "uchar", "-o", "{out:out.nii.gz}")  # fmt: skip
    T = "ants-apply-transforms"
    return [
        Spec(T, "panel-7t-to-mni", "panel-7t-to-mni",
             "Data panel: the raw 7 T MP2RAGE into MNI space in one interpolation",
             "antsApplyTransforms with fMRIPrep's transforms of ds005454 sub-16 (its raw-to-T1w "
             "affine, then its T1w-to-MNI152NLin2009cAsym composite: an affine and a SyN "
             "displacement field) and windowed-sinc (Lanczos) interpolation onto the 1 mm "
             "template: the panel's largest anatomical image resampled once. Shown beside the "
             "template it lands on.",
             "antsApplyTransforms", seven, (("out", "out.nii.gz", "resampled", ""),), tpl,
             (("template", tpl, "MNI152NLin2009cAsym, 1 mm (the reference grid)"),),
             (data("ds005454"),), band=(), command=_cmd("antsApplyTransforms", seven)),
        Spec(T, "panel-lesion-to-mni", "panel-lesion-to-mni",
             "Data panel: a manual stroke mask into MNI space",
             "The stroke's manual lesion mask (ds004884 sub-M2079, on its T1w's grid, as "
             "fMRIPrep read it) through fMRIPrep's T1w-to-MNI composite, GenericLabel, onto the "
             "2 mm grid of fMRIPrep's MNI-space T1w, drawn over it. A key dataset, shown as a whole "
             "volume.",
             "antsApplyTransforms", lesion, (("out", "out.nii.gz", "lesion in MNI space", ""),),
             mni2, (("t1w-mni", mni2, "ds004884 sub-M2079, T1w in MNI space (fMRIPrep's)"),),
             (data("ds004884"),), view="volume", band=(), labels=("out",),
             command=_cmd("antsApplyTransforms", lesion)),
        Spec(T, "panel-flair-to-t1w", "panel-flair-to-t1w",
             "Data panel: a FLAIR of 3 mm slices onto its T1w's grid",
             "An 82-year-old's FLAIR (ds003592 sub-280, 0.42 × 0.42 × 3 mm) onto her "
             "preprocessed T1w's 1 mm grid through the scanner's coordinates (identity), "
             "Lanczos: windowed sinc across 3 mm slices.",
             "antsApplyTransforms", flair, (("out", "out.nii.gz", "resampled", ""),), t1w,
             (("t1w", t1w, "ds003592 sub-280, preprocessed T1w (fMRIPrep's)"),),
             (data("ds003592"),), band=(), command=_cmd("antsApplyTransforms", flair)),
    ]  # fmt: skip


def _ai() -> list[Spec]:
    from larmorx_validation.parity import niworkflows_inputs

    brain = next(
        c for c in fmriprep_nodes.calls("antsAI")
        if c.run == RUN_TUMOUR and c.norm.endswith("brain_extraction_wf/init_aff")
    )  # fmt: skip
    args = [("{out:initialization.mat}" if a == "{out:initialization.mat}" else a)
            for a in brain.args]  # fmt: skip
    mattes = next(a for a in args if a.startswith("Mattes["))
    fixed, moving = mattes[len("Mattes[") :].split(",")[:2]
    files = niworkflows_inputs.masked(RUN_STROKE)
    f_m, m_m = str(files["fixed_masked.nii.gz"]), str(files["moving_masked.nii.gz"])
    init = ("3", f_m, m_m, "{out:transform.mat}", *niworkflows_inputs.AFFINE_INITIALIZER)
    return [
        Spec("ants-ai", "panel-fmriprep-init-aff-tumour", "panel-init-aff-tumour",
             "Data panel: fMRIPrep's brain-extraction antsAI on a brain with a tumour",
             "fMRIPrep 25.2.6's own call (sMRIPrep's init_aff: OASIS30ANTs regridded to 4 mm "
             "against the N4-corrected T1w regridded alike, Mattes, an Affine search from "
             "fMRIPrep's seed) on ds005003 sub-RS001: each run's transform places the T1w "
             "(4 mm) on the template's grid (ANTs' antsApplyTransforms, linear). A key dataset, "
             "shown as a whole volume; every run's parameters are in the table below it.",
             "antsAI", tuple(args), (("placed", "initialization.mat", "T1w placed", ""),), fixed,
             (("template", fixed, "OASIS30ANTs at 4 mm (fMRIPrep's res_tmpl)"),),
             (data("ds005003"),), view="volume", seed="50495", band=(8,),
             place=(moving, fixed, "initialization.mat"),
             command=_cmd("antsAI", brain.command().split()[1:]),
             node=f"`{fmriprep_nodes.label(brain.node)}` of the oracle run {RUN_TUMOUR}"),
        Spec("ants-ai", "panel-affine-initializer-stroke", "panel-init-stroke",
             "Data panel: sMRIPrep's antsAffineInitializer on a brain with a stroke",
             "The antsAffineInitializer call niworkflows' SpatialNormalization makes inside "
             "fMRIPrep's template registration of ds004884 sub-M2079 (no node of its own): the "
             "masked 1 mm MNI template and the masked T1w, made again exactly from the node's "
             "inputs; each run's transform places the T1w on the template's grid. Shown as "
             "three slices.",
             "antsAffineInitializer", init, (("placed", "transform.mat", "T1w placed", ""),), f_m,
             (("template", f_m, "MNI152NLin2009cAsym 1 mm, masked (niworkflows')"),),
             (data("ds004884"),), band=(8,), place=(m_m, f_m, "transform.mat"),
             command=_cmd("antsAffineInitializer", init),
             node=f"inside `{fmriprep_nodes.label(niworkflows_inputs.registration_call(RUN_STROKE).node)}` "
             f"of the oracle run {RUN_STROKE}"),
    ]  # fmt: skip


def _registration() -> list[Spec]:
    """fMRIPrep's BOLD-reference registrations (affine to MNI's BOLD reference; rigid to a
    field map's magnitude): each run's transform places the moving image on the fixed grid."""

    def reg(c: fmriprep_nodes.NodeCall) -> tuple[tuple[str, ...], str, str]:
        args = list(c.args)
        i = args.index("--output")
        args[i + 1] = "{out:transform}"
        metric = args[args.index("--metric") + 1 :]
        words = " ".join(metric).split("]", 1)[0]
        fixed, moving = (w.strip() for w in words.split("[", 1)[1].split(",")[:2])
        return tuple(args), fixed, moving

    out = []
    seven = next(
        c for c in fmriprep_nodes.calls("antsRegistration")
        if c.run == RUN_7T and c.norm.endswith("enhance_and_skullstrip_bold_wf/norm")
    )  # fmt: skip
    args, fixed, moving = reg(seven)
    out.append(
        Spec("ants-registration", "panel-fmriprep-boldref-7t", "panel-boldref-7t",
             "Data panel: fMRIPrep's BOLD reference to MNI at 7 T",
             "fMRIPrep 25.2.6's own call (the BOLD reference's registration to MNI152NLin2009cAsym's "
             "fMRIPrep BOLD reference at 2 mm: Affine, Mattes with random sampling from fMRIPrep's "
             "seed) on ds005454 sub-16's 7 T multiband BOLD reference (1.4 mm): each run's "
             "transform places the BOLD reference on the template's grid (ANTs' "
             "antsApplyTransforms, linear). A key dataset, shown as a whole volume; every run's "
             "parameters are in the table below it.",
             "antsRegistration", args, (("placed", "transform0GenericAffine.mat",
                                         "BOLD reference placed", ""),), fixed,
             (("template", fixed, "MNI152NLin2009cAsym's BOLD reference, 2 mm"),),
             (data("ds005454"),), view="volume", seed=seven.environ("ANTS_RANDOM_SEED"),
             band=(8,), place=(moving, fixed, "transform0GenericAffine.mat"),
             command=_cmd("antsRegistration", seven.command().split()[1:]),
             node=f"`{fmriprep_nodes.label(seven.node)}` of the oracle run {RUN_7T}")
    )  # fmt: skip
    philips = next(
        (c for c in fmriprep_nodes.calls("antsRegistration")
         if c.run.startswith("ds002603") and c.norm.endswith("fmapreg_wf/coregister")
         and "Mb4" in c.node),
        None,
    )  # fmt: skip
    if philips is not None:
        args, fixed, moving = reg(philips)
        out.append(
            Spec("ants-registration", "panel-fmriprep-coregister-philips",
                 "panel-coregister-philips",
                 "Data panel: fMRIPrep's field map to a Philips multiband BOLD reference",
                 "fMRIPrep 25.2.6's own call (sdcflows' coregistration of the field map's "
                 "magnitude and the BOLD reference: two Rigid stages, Mattes with random "
                 "sampling, masks) on ds002603 sub-01's multiband-4 run (Philips, 588 volumes): "
                 "each run's transform places the BOLD reference on the magnitude's grid.",
                 "antsRegistration", args, (("placed", "transform0GenericAffine.mat",
                                             "BOLD reference placed", ""),), fixed,
                 (("magnitude", fixed, "the field map's magnitude (fMRIPrep's)"),),
                 (data("ds002603"),), seed=philips.environ("ANTS_RANDOM_SEED"), band=(8,),
                 place=(moving, fixed, "transform0GenericAffine.mat"),
                 command=_cmd("antsRegistration", philips.command().split()[1:]),
                 node=f"`{fmriprep_nodes.label(philips.node)}` of the oracle run {philips.run}")
        )  # fmt: skip
    return out


SPECS = {
    "ants-registration": _registration,
    "ants-programs": _programs,
    "ants-n4": _n4,
    "ants-denoise": _denoise,
    "ants-atropos": _atropos,
    "ants-apply-transforms": _apply_transforms,
    "ants-ai": _ai,
}


def specs(tool: str) -> list[Spec]:
    return SPECS[tool]() if tool in SPECS else []


def examples(ctx: core.Context, tool: str) -> list[dict]:
    """The page's panel examples (their cache entries computed where missing)."""
    return [example(ctx, s) for s in specs(tool)]


def tables(ctx: core.Context, tool: str) -> list[dict]:
    """The page's panel tables: every run's transform of a transform program's examples."""
    out = []
    for s in specs(tool):
        if s.place is not None:
            out.append(transform_table(ctx, s, f"{s.id}-transforms",
                                       f"{s.title}: every run's transform"))  # fmt: skip
    return out


def entries(ctx: core.Context, tool: str) -> list:
    from larmorx_validation.evidence.panel_examples import entry

    return [entry(ctx, s) for s in specs(tool)]


# ------------------------------------------------------------------- records and the inventory


def tidy(text: str) -> str:
    """A command as a reader needs it: no machine's paths (an oracle run's files named from the
    run, the test data's from the catalog, outputs by their names)."""
    import re

    text = re.sub(r"\{out:([^}]+)\}", r"\1", text)
    root = fmriprep_nodes.root()
    if root is not None:
        text = text.replace(str(root) + "/", "<fMRIPrep oracle run>/")
    text = re.sub(r"[^\s\[,]*/larmorx-testdata/data/", "", text)
    text = re.sub(r"[^\s\[,]*/oracles/templateflow/", "templateflow/", text)
    text = re.sub(r"[^\s\[,]*/oracles/inputs/", "<oracles/inputs>/", text)
    return text


def panel_cases_tidy(cases: list[dict]) -> list[dict]:
    """The panel record's cases with their commands :func:`tidy`."""
    for c in cases:
        if c.get("command"):
            c["command"] = tidy(c["command"])
    return cases


def panel_record(repo, name: str):
    """The panel record ``<name>-panel`` (``docs/validation``), or ``None`` before it exists."""
    from larmorx_validation.figures import records

    if not (repo / "docs" / "validation" / f"{name}-panel.json").is_file():
        return None
    return records.load(repo, f"{name}-panel")


def inventory_table(rec, tid: str, program: str) -> dict | None:
    """The panel record's inventory of fMRIPrep's nodes (its table "Node (normalised path)") as
    a table of the page."""
    from larmorx_validation.evidence import common
    from larmorx_validation.figures import records

    if rec is None:
        return None
    t = next(
        (t for t in rec.tables if t.columns and t.columns[0] == "Node (normalised path)"), None
    )
    if t is None:
        return None
    cols = list(t.columns[1:])
    numeric = ("Nodes", "Runs", "Compared", "Same call as a compared one")
    rows = []
    for r in t.rows:
        cells = []
        for c in cols:
            v = r.get(c, "")
            cells.append(common.cell(int(v), "count") if v.isdigit() else common.cell(v))
        rows.append({"head": records.strip_code(r[t.columns[0]]), "cells": cells})
    columns = [t.columns[0], *({"label": c, "num": c in numeric} for c in cols)]
    return common.table(
        tid,
        f"fMRIPrep's {program} nodes in the fourteen oracle runs, and what the panel record "
        "compares of them",
        columns,
        rows,
        note=f"From the panel record ({rec.path_md.relative_to(rec.repo).as_posix()}): every "
        "node of the finished fMRIPrep 25.2.6 oracle runs that ran the program, grouped by its "
        "path with the subject, run, space and item removed; a call repeated word for word is "
        "compared once.",
    )


__all__ = [
    "SPECS",
    "data",
    "entries",
    "examples",
    "inventory_table",
    "node_call",
    "panel_record",
    "specs",
    "tables",
]
