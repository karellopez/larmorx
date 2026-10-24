# SPDX-License-Identifier: Apache-2.0
"""Parity of the 3dAutomask replica (``larmorx-gpl afni 3dAutomask``, run by larmorx as a
separate process) with AFNI 25.2.09's ``3dAutomask``: exit status, messages and every output
(the mask, ``-apply_prefix``'s masked input, ``-depth``), byte for byte
(:mod:`larmorx_validation.parity.afni_replica`).

Cases: every option and its paths on the phantoms of
:mod:`larmorx_validation.parity.afni_phantoms` (``-clfrac``, ``-nograd``, ``-peels``,
``-nbhrs``, ``-NN1/2/3``, ``-eclip``, ``-dilate``, ``-erode``, ``-SI`` along each axis and from
each end, ``-depth``, ``-apply_prefix``, ``-q``); every storage type, scaling and geometry
AFNI reads; 2D, 3D and 4D inputs, and the sizes where AFNI's steps stop; AFNI's global options
and environment variables; the errors and crashes; the defects (D13: AFNI-01, -02, -07, -11,
-12, -16, -18, -23, -26, -43); and real data: fMRIPrep's own call
(``3dAutomask -apply_prefix ... -dilate 1``) on the inputs of every oracle run's BOLD
reference, compared with fMRIPrep's own masks too, and AFNI's default on BOLD series and a
skull-stripped anatomical. The real cases also measure AFNI's own variability (thread counts,
fMRIPrep's build of AFNI in its image).
"""

from __future__ import annotations

import functools

from larmorx_validation.parity import afni_phantoms as ph
from larmorx_validation.parity import afni_replica as ar
from larmorx_validation.parity.harness import Case, CheckList, Suite

PROGRAM = "3dAutomask"
M = "mask.nii"
S = ar.Scenario


def _python_options(**kw):
    return {"python": kw, "python_outputs": {"mask": M}}


def _scenarios() -> list[tuple[str, str, str, ar.Scenario]]:
    # fmt: off
    out: list[tuple[str, str, str, ar.Scenario]] = []

    def add(cid: str, category: str, desc: str, s: ar.Scenario) -> None:
        out.append((f"{category}/{cid}", category, desc, s))

    # --- Options, on the T1-like head phantom --------------------------------------------------
    h = "head-f32"
    add("default", "options", "the defaults (gradual clip level 0.5, 1 peel, NN2)",
        S(("-prefix", M, "{0}"), (h,), (M,), **_python_options()))
    add("fmriprep-call", "options", "fMRIPrep's options: -apply_prefix, -dilate 1",
        S(("-apply_prefix", "masked.nii", "-dilate", "1", "-prefix", M, "{0}"), (h,), (M, "masked.nii"),
          python={"dilate": 1, "apply": True}, python_outputs={"mask": M, "masked": "masked.nii"}))
    for c in ("0.1", "0.3", "0.9"):
        add(f"clfrac-{c}", "options", f"-clfrac {c}",
            S(("-clfrac", c, "-prefix", M, "{0}"), (h,), (M,), tier="smoke" if c == "0.3" else "standard",
              **_python_options(clfrac=float(c))))
    add("mfrac", "options", "-mfrac, the same as -clfrac",
        S(("-mfrac", "0.35", "-prefix", M, "{0}"), (h,), (M,), tier="standard"))
    add("nograd", "options", "-nograd: one clip level", S(("-nograd", "-prefix", M, "{0}"), (h,), (M,),
                                                          **_python_options(gradual=False)))
    add("nogr-prefix", "options", "-nogr: AFNI compares 5 characters",
        S(("-nogrxyz", "-prefix", M, "{0}"), (h,), (M,), tier="standard"))
    for p in ("2", "3"):
        add(f"peels-{p}", "options", f"-peels {p}", S(("-peels", p, "-prefix", M, "{0}"), (h,), (M,),
                                                    tier="smoke" if p == "2" else "standard",
                                                    **_python_options(peels=int(p))))
    for n in ("6", "15", "18", "26", "40"):
        # AFNI-41 (decided 2026-10-23, rule B): outside 6..26 the default refuses.
        add(f"nbhrs-{n}", "options", f"-nbhrs {n} (AFNI uses 17 outside 6..26, AFNI-41)",
            S(("-nbhrs", n, "-prefix", M, "{0}"), (h,), (M,), tier="smoke" if n in ("15", "40") else "standard",
              fix_refuses="AFNI-41" if n == "40" else None))
    # AFNI-41 (rule B, decided 2026-10-23): outside 6..26 the default refuses; AFNI peels as with 17
    # (or as -NNx says), which `reproduce` gives.
    for n in ("5", "27", "0"):
        add(f"nbhrs-{n}", "options", f"-nbhrs {n}: outside 6..26, refused by default (AFNI-41)",
            S(("-nbhrs", n, "-prefix", M, "{0}"), (h,), (M,), tier="standard", fix_refuses="AFNI-41"))
    add("nbhrs-40-then-NN1", "options", "-nbhrs 40 then -NN1: AFNI peels as -NN1 says; refused by default (AFNI-41)",
        S(("-nbhrs", "40", "-NN1", "-prefix", M, "{0}"), (h,), (M,), tier="standard", fix_refuses="AFNI-41"))
    for nn in ("1", "2", "3"):
        add(f"NN{nn}-dilate-erode-depth", "options",
            f"-NN{nn} with -dilate 2, -erode 1 and -depth (and the peeling count it sets, AFNI-42)",
            S((f"-NN{nn}", "-dilate", "2", "-erode", "1", "-depth", "depth.nii", "-prefix", M, "{0}"),
              (h,), (M, "depth.nii"), tier="smoke" if nn == "1" else "standard"))
    add("nbhrs-then-NN2", "options", "-nbhrs 12 then -NN2: -NN2 sets 18 (AFNI-42)",
        S(("-nbhrs", "12", "-NN2", "-prefix", M, "{0}"), (h,), (M,), tier="standard"))
    add("eclip", "options", "-eclip", S(("-eclip", "-prefix", M, "{0}"), (h,), (M,),
                                       **_python_options(exterior_clip=True)))
    add("eclip-nograd-clfrac", "options", "-eclip -nograd -clfrac 0.2",
        S(("-eclip", "-nograd", "-clfrac", "0.2", "-prefix", M, "{0}"), (h,), (M,), tier="standard"))
    add("dilate-3", "options", "-dilate 3", S(("-dilate", "3", "-prefix", M, "{0}"), (h,), (M,), tier="standard"))
    add("erode-2", "options", "-erode 2", S(("-erode", "2", "-prefix", M, "{0}"), (h,), (M,),
                                           **_python_options(erode=2)))
    add("depth", "options", "-depth alone", S(("-depth", "depth.nii", "-prefix", M, "{0}"), (h,), (M, "depth.nii"),
                                              python={"depth": True}, python_outputs={"mask": M, "depth": "depth.nii"}))
    add("apply-only", "options", "-apply_prefix without -prefix: the only output",
        S(("-apply_prefix", "masked.nii", "{0}"), (h,), ("masked.nii",)))
    add("quiet", "options", "-q", S(("-q", "-prefix", M, "{0}"), (h,), (M,)))
    for img, label in (("head-f32", "z, superior last"), ("head-superior-first", "z, superior first"),
                       ("head-coronal", "y"), ("head-las", "z, LAS storage")):
        add(f"SI-{img}", "options", f"-SI 60 along {label}",
            S(("-SI", "60", "-prefix", M, "{0}"), (img,), (M,), tier="smoke" if img == h else "standard",
              **({} if img != h else _python_options(si=60.0))))
    add("SI-everything", "options", "-SI 1: clipping beyond the mask", S(("-SI", "1", "-prefix", M, "{0}"), (h,), (M,), tier="standard"))
    add("t2-contrast", "options", "the T2-like phantom", S(("-prefix", M, "{0}"), ("head-t2-f32",), (M,)))
    add("big", "options", "a volume of 1.2 million voxels", S(("-apply_prefix", "masked.nii.gz", "-prefix", "mask.nii.gz", "{0}"),
                                                               ("big",), ("mask.nii.gz", "masked.nii.gz"), tier="standard"))

    # --- Storage types, scaling and geometry ---------------------------------------------------
    for img in ("head-i16", "head-u8", "head-i16-slope2", "head-i16-slope1", "head-f64", "head-i8",
                "head-u16", "head-i32", "head-nan", "head-oblique", "head-qform-only", "head-sform-only",
                "head-las"):
        tier = "smoke" if img in ("head-i16", "head-i16-slope1", "head-oblique") else "standard"
        add(img, "datatypes", f"{img}: the mask, the masked input and the depth",
            S(("-apply_prefix", "masked.nii", "-depth", "depth.nii", "-prefix", M, "{0}"), (img,),
              (M, "masked.nii", "depth.nii"), tier=tier))
    add("head-f32-slope-inter", "defects", "float32 with scl_slope and scl_inter (AFNI-02)",
        S(("-apply_prefix", "masked.nii", "-prefix", M, "{0}"), ("head-f32-slope-inter",), (M, "masked.nii")))
    add("head-metre", "defects", "a grid in metres (AFNI-07)", S(("-prefix", M, "{0}"), ("head-metre",), (M,)))
    add("head-nearly-plumb", "defects", "a grid 0.009 degrees from plumb (AFNI-16)",
        S(("-prefix", M, "{0}"), ("head-nearly-plumb",), (M,)))
    add("nifti-scale-no", "defects", "AFNI_NIFTI_SCALE=NO on int16 with an intercept (AFNI-23)",
        S(("-apply_prefix", "masked.nii", "-prefix", M, "{0}"), ("head-i16-intercept",), (M, "masked.nii"),
          env=(("AFNI_NIFTI_SCALE", "NO"),)))
    add("peels-0", "defects", "-peels 0 (AFNI-43: AFNI peels once)",
        S(("-peels", "0", "-prefix", M, "{0}"), (h,), (M,), **_python_options(peels=0)))
    add("peels-0-fmriprep", "defects", "-peels 0 added to fMRIPrep's call on its BOLD reference of ds005040 (AFNI-43)",
        S(("-apply_prefix", "masked.nii.gz", "-dilate", "1", "-peels", "0", "-prefix", "mask.nii.gz", "{0}"),
          ("fmriprep:ds005040-task_clip_run_1:automask",), ("mask.nii.gz", "masked.nii.gz"), tier="standard"))
    add("negative-slope-4d", "defects", "a 4D series with scl_slope -2 (AFNI-01)",
        S(("-apply_prefix", "masked.nii", "-prefix", M, "{0}"), ("series-negative",), (M, "masked.nii")))

    # --- Dimensions ----------------------------------------------------------------------------
    for img, desc, tier in (
        ("series-i16", "a 4D int16 series: the mean of |values|", "smoke"),
        ("series-f32", "a 4D float32 series", "standard"),
        ("series-slope", "a 4D series with scl_slope 2", "standard"),
        ("series-one", "a 4D file of one volume", "standard"),
        ("slice", "a 2D slice (64 x 64 x 1): AFNI's 2D path", "smoke"),
        ("two-slices", "two slices (64 x 64 x 2)", "standard"),
        ("tiny", "2 x 2 x 2 voxels: nothing to cluster", "standard"),
        ("zeros", "an image of zeros: the clip level is 0", "standard"),
        ("constant", "a constant image", "standard"),
    ):
        add(img, "dimensions", desc, S(("-apply_prefix", "masked.nii", "-prefix", M, "{0}"), (img,), (M, "masked.nii"), tier=tier))
    add("slice-small", "errors", "a 2D slice smaller than 16 x 16: no mask",
        S(("-prefix", M, "{0}"), ("slice-small",), (M,), expect="error"))
    add("row", "errors", "one row (64 x 1 x 1): no mask", S(("-prefix", M, "{0}"), ("row",), (M,), expect="error"))

    # --- Errors and crashes --------------------------------------------------------------------
    for opt in ("-peels", "-nbhrs", "-clfrac", "-SI", "-dilate", "-erode"):
        add(f"missing-value{opt}", "errors", f"{opt} as the last argument: AFNI crashes (AFNI-40)",
            S(("-q", opt), (), (), expect="crash", tier="smoke" if opt == "-peels" else "standard"))
    for opt in ("-prefix", "-depth", "-apply_prefix"):
        add(f"missing-name{opt}", "errors", f"{opt} as the last argument: '(null) is illegal'",
            S(("-q", opt), (), (), expect="error", tier="standard"))
    for cid, args, desc in (
        ("clfrac-high", ("-clfrac", "0.95", "{0}"), "-clfrac 0.95"),
        ("clfrac-low", ("-clfrac", "0.05", "{0}"), "-clfrac 0.05"),
        ("SI-negative", ("-SI", "-3", "{0}"), "-SI -3"),
        ("dilate-negative", ("-dilate", "-1", "{0}"), "-dilate -1"),
        ("erode-negative", ("-erode", "-2", "{0}"), "-erode -2"),
        ("unknown-option", ("-bogus", "{0}"), "an unknown option"),
        ("no-dataset", ("-prefix", M), "no dataset"),
        ("missing-input", ("-prefix", M, "/nonexistent/x.nii"), "a missing input"),
    ):
        add(cid, "errors", desc, S(args, (h,), (M,), expect="error", tier="smoke" if cid == "unknown-option" else "standard"))
    add("default-prefix", "divergences", "no -prefix: AFNI writes its own format (automask+orig)",
        S(("{0}",), (h,), ("automask+orig.HEAD",), expect="divergence", tier="standard",
          reason="AFNI's default prefix 'automask' is its own dataset format, which larmorx-gpl does "
          "not write: it refuses (NIfTI only)."))

    # --- AFNI's global options and environment variables --------------------------------------
    taken = (("mask.nii", "taken"),)
    add("exists", "global", "an existing output (AFNI-11)", S(("-prefix", M, "{0}"), (h,), (M,), files=taken,
                                                               fix_refuses="AFNI-11"))
    add("overwrite", "global", "-overwrite", S(("-overwrite", "-prefix", M, "{0}"), (h,), (M,), files=taken))
    add("deconflict-yes", "global", "AFNI_DECONFLICT=YES: renamed", S(("-prefix", M, "{0}"), (h,), (M, "mask_AA1.nii"), files=taken,
                                                                     env=(("AFNI_DECONFLICT", "YES"),)))
    add("D-deconflict-overwrite", "global", "-DAFNI_DECONFLICT=OVERWRITE", S(("-DAFNI_DECONFLICT=OVERWRITE", "-prefix", M, "{0}"),
                                                                             (h,), (M,), files=taken))
    add("D-write-type-2", "global", "-DAFNI_NIFTI_WRITE_TYPE=2: NIfTI-2", S(("-DAFNI_NIFTI_WRITE_TYPE=2", "-prefix", M, "{0}"), (h,), (M,)))
    add("quiet-startup", "environment", "AFNI_QUIET_STARTUP=YES: no author or output lines",
        S(("-prefix", M, "{0}"), (h,), (M,), env=(("AFNI_QUIET_STARTUP", "YES"),)))
    add("message-prefix", "environment", "AFNI_MESSAGE_PREFIX", S(("-prefix", M, "{0}"), (h,), (M,),
                                                                  env=(("AFNI_MESSAGE_PREFIX", "AM"),), tier="standard"))
    add("echo-commandline", "environment", "AFNI_ECHO_COMMANDLINE=YES", S(("-prefix", M, "{0}"), (h,), (M,),
                                                                          env=(("AFNI_ECHO_COMMANDLINE", "YES"), ("AFNI_DONT_LOGFILE", None)), tier="standard"))
    add("colorize-no", "environment", "AFNI_MESSAGE_COLORIZE=NO with a warning", S(("-dilate", "1", "-erode", "1", "-prefix", M, "{0}"), (h,), (M,),
                                                                                   env=(("AFNI_MESSAGE_COLORIZE", "NO"),), tier="standard"))
    add("nifti-priority-q", "environment", "AFNI_NIFTI_PRIORITY=Q on an oblique input", S(("-prefix", M, "{0}"), ("head-oblique",), (M,),
                                                                                            env=(("AFNI_NIFTI_PRIORITY", "Q"),), tier="standard"))
    add("prefix-with-blank", "global", "a -prefix with a blank (AFNI-12)", S(("-prefix", "a b.nii", "{0}"), (h,), ("a b.nii",), fix_writes="AFNI-12"))
    add("input-dir-with-blank", "global", "an input in a directory with a blank (AFNI-18)",
        S(("-prefix", M, "{0}"), ("blank:head-f32",), (M,), tier="standard", fix_writes="AFNI-18"))
    add("missing-directory", "global", "an output in a missing directory (AFNI-26)",
        S(("-prefix", "nodir/mask.nii", "{0}"), (h,), ("nodir/mask.nii",), tier="standard",
          fix_refuses="AFNI-26"))

    # --- Real data -----------------------------------------------------------------------------
    for i, (label, _) in enumerate(ph.fmriprep_runs()):
        add(label, "fmriprep", f"fMRIPrep's call on the input of its BOLD reference's second skull strip ({label})",
            S(("-apply_prefix", "uni_xform_masked.nii.gz", "-dilate", "1", "-prefix", "uni_xform_mask.nii.gz", "{0}"),
              (f"fmriprep:{label}:automask",), ("uni_xform_mask.nii.gz", "uni_xform_masked.nii.gz"),
              tier="smoke" if i == 0 else "standard", band=(1, 6),
              fmriprep=(("uni_xform_mask.nii.gz", f"fmriprep:{label}:mask"),),
              python={"dilate": 1, "apply": True},
              python_outputs={"mask": "uni_xform_mask.nii.gz", "masked": "uni_xform_masked.nii.gz"}))
    o = "td:openneuro"
    for bold, tier in (
        (f"{o}/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz", "standard"),
        (f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-v4_bold.nii.gz", "standard"),
        (f"{o}/ds005040/sub-002/func/sub-002_task-clip_run-1_bold.nii.gz", "full"),
        (f"{o}/ds005454/sub-16/func/sub-16_task-rest_bold.nii.gz", "full"),
        (f"{o}/ds005752/sub-ON01016/ses-01/func/sub-ON01016_ses-01_task-rest_dir-forward_bold.nii.gz", "full"),
    ):
        name = bold.rsplit("/", 1)[1].removesuffix("_bold.nii.gz")
        add(name, "real", f"AFNI's default on a BOLD series ({name})",
            S(("-prefix", "mask.nii.gz", "{0}"), (bold,), ("mask.nii.gz",), tier=tier, band=(1,)))
    add("t1w-brain", "real", "the skull-stripped T1w of an fMRIPrep run (ds000005, sMRIPrep's desc-brain)",
        S(("-prefix", "mask.nii.gz", "{0}"), ("fmriprep-anat:ds000005:brain",), ("mask.nii.gz",), tier="standard", band=(1,)))
    # fmt: on
    return out


@functools.cache
def _scenario_list() -> list[tuple[str, str, str, ar.Scenario]]:
    return _scenarios()


def _input(name: str):
    from pathlib import Path

    if name.startswith("blank:"):
        src = ph.input_path(name.removeprefix("blank:"))
        d = ar.generated_dir() / "with blank"
        d.mkdir(exist_ok=True)
        target = d / src.name
        if not target.exists():
            target.write_bytes(src.read_bytes())
        return target
    if name.startswith("fmriprep-anat:"):
        # sMRIPrep's preprocessed T1w times its brain mask: a skull-stripped anatomical.
        import nibabel as nib
        import numpy as np

        _, run, _ = name.split(":")
        target = ar.generated_dir() / f"{run}-T1w-brain.nii.gz"
        if target.exists():
            return target
        ws = ar.workspace()
        root = ws / ar.FMRIPREP_RUNS if ws else Path("/nonexistent")
        t1 = sorted(root.glob(f"{run}-*/out/sub-*/anat/*_desc-preproc_T1w.nii.gz"))
        mask = sorted(root.glob(f"{run}-*/out/sub-*/anat/*_desc-brain_mask.nii.gz"))
        if not t1 or not mask:
            raise FileNotFoundError(name)
        img = nib.load(t1[0])
        brain = np.asarray(img.dataobj, dtype=np.float32) * (
            np.asarray(nib.load(mask[0]).dataobj) > 0
        )
        nib.save(nib.Nifti1Image(brain.astype(np.float32), img.affine), target)
        return target
    return ph.input_path(name)


#: The implementations this suite validates.
IMPLEMENTATIONS = ("original", "replica")


def spec(implementation: str = "original") -> ar.ToolSpec:
    if implementation not in IMPLEMENTATIONS:
        raise ValueError(f"implementation must be one of {IMPLEMENTATIONS}, not {implementation!r}")
    return ar.ToolSpec(PROGRAM, "automask", _scenario_list, _input, implementation)


def cases(tier: str) -> list[Case]:
    return ar.cases_of(spec(), tier)


def suite(implementation: str = "original") -> Suite:
    """The 3dAutomask suite for the clean-room original (``larmorx afni 3dAutomask``, in
    process) or the GPL replica (``larmorx-gpl afni 3dAutomask``, which larmorx runs as a
    separate process); both through larmorx's command line and its Python API."""
    tool_spec = spec(implementation)
    replica = implementation == "replica"
    who = "replica" if replica else "original"

    def run_case(case: Case, checks: CheckList) -> None:
        ar.run_case(tool_spec, case, checks)

    tool = (
        f"`larmorx-gpl afni 3dAutomask` ({ar.replica_version()}; crate larmorx-gpl-afni, the "
        "GPL-3.0-or-later replica translated from AFNI 25.2.09's source), run by larmorx as a "
        "separate process: `larmorx afni 3dAutomask` with `LARMORX_IMPLEMENTATION=replica` and "
        '`lx.afni.automask(..., implementation="replica")`'
        if replica
        else "`larmorx afni 3dAutomask` / `lx.afni.automask` (crate larmorx-afni, the Apache-2.0 "
        "clean-room original written from `specs/3dAutomask.md`), with "
        '`implementation="original"`'
    )
    return Suite(
        name="afni-automask-replica" if replica else "afni-automask",
        title="3dAutomask (replica)" if replica else "3dAutomask",
        tool=tool,
        reference=f"3dAutomask from AFNI ({ar.afni_build(PROGRAM)}), the binary built by `scripts/build_afni_oracle.sh`",
        thresholds=[
            (
                "Exit status",
                "the same (both succeed, or both reject the arguments); where AFNI crashes, larmorx exits 1",
            ),
            (
                "Messages",
                "standard output and error byte for byte (AFNI's banner and compile date, larmorx's own lines set aside)",
            ),
            (
                "Every output (mask, masked input, depth)",
                "the same values, bit for bit, and the same NIfTI header bytes",
            ),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        highlights=lambda results: ar.highlights(
            results, PROGRAM, "automask", "a missing option value, AFNI-40", who
        ),
        notes=(
            "Both programs run in directories of their own on the same input files, in the same "
            "environment (no AFNI variables of this process, `HOME` an empty directory, "
            "`AFNI_DONT_LOGFILE=YES`, `AFNI_NIFTI_NOEXT=YES`, plus each case's variables); "
            f"outputs are compared as stored (data bytes, header bytes decompressed). The {who} "
            "is compared in `reproduce` mode; the fix pass runs its default. The real cases also "
            "run AFNI at other thread counts and fMRIPrep 25.2.6's own AFNI (in its image) and "
            "report how far they are from the oracle (AFNI's own variability); the fMRIPrep cases "
            f"compare the {who} with fMRIPrep's own output of the node as well."
        ),
    )
