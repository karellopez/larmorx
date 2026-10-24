# SPDX-License-Identifier: Apache-2.0
"""Parity of the 3dUnifize replica (``larmorx-gpl afni 3dUnifize``, run by larmorx as a separate
process) with AFNI 25.2.09's ``3dUnifize``: exit status, messages and every output (the
unifized image, ``-ssave``'s scale factors, ``-amsave``'s automasked input), byte for byte
(:mod:`larmorx_validation.parity.afni_replica`).

Cases: every option and its paths on the phantoms of
:mod:`larmorx_validation.parity.afni_phantoms` (``-Urad``, ``-rbt``/``-param``, ``-clfrac``,
``-T2`` once, twice and three times, ``-T2up``, ``-GM``, ``-nosquash``, ``-noduplo`` and the
half-size path above a million voxels, ``-EPI``, ``-ssave``, ``-amsave``, ``-input``/``-inset``,
``-quiet``, ``-verb``, ``-useall``, AFNI's case-insensitive options); every storage type,
scaling and geometry AFNI reads; 2D, 3D and 4D inputs; AFNI's global options and environment
variables; the errors and crashes; the defects (D13: AFNI-01, -02, -07, -11, -12, -16, -18,
-23, -26, -45, -47); and real data: fMRIPrep's own call (``3dUnifize -clfrac 0.2 -rbt 18.3
65.0 90.0 -T2``) on the inputs of every oracle run's BOLD reference, compared with fMRIPrep's
own outputs too; AFNI's default on the oracle runs' T1w images (the half-size path); ``-GM`` on
a T1w; ``-EPI`` on BOLD series. The real cases also measure AFNI's own variability (OpenMP
thread counts, fMRIPrep's build of AFNI in its image).
"""

from __future__ import annotations

import functools

from larmorx_validation.parity import afni_phantoms as ph
from larmorx_validation.parity import afni_replica as ar
from larmorx_validation.parity.afni_automask import _input
from larmorx_validation.parity.harness import Case, CheckList, Suite

PROGRAM = "3dUnifize"
U = "uni.nii"
S = ar.Scenario


def _py(outputs=None, **kw):
    return {"python": kw, "python_outputs": outputs or {"image": U}}


def _scenarios() -> list[tuple[str, str, str, ar.Scenario]]:
    # fmt: off
    out: list[tuple[str, str, str, ar.Scenario]] = []

    def add(cid: str, category: str, desc: str, s: ar.Scenario) -> None:
        out.append((f"{category}/{cid}", category, desc, s))

    t1, t2 = "head-f32", "head-t2-f32"
    p = ("-prefix", U)
    add("default", "options", "the defaults on the T1-like phantom", S((*p, "{0}"), (t1,), (U,), **_py()))
    add("fmriprep-call", "options", "fMRIPrep's options on the T2-like phantom: -clfrac 0.2 -rbt 18.3 65 90 -T2",
        S(("-clfrac", "0.2", "-rbt", "18.3", "65.0", "90.0", *p, "-T2", "-input", "{0}"), (t2,), (U,),
          **_py(clfrac=0.2, radius=18.3, percentiles=(65.0, 90.0), t2=True)))
    for r, tier in (("5", "smoke"), ("12", "standard"), ("30", "standard")):
        add(f"Urad-{r}", "options", f"-Urad {r}", S(("-Urad", r, *p, "{0}"), (t1,), (U,), tier=tier,
                                                   **(_py(radius=float(r)) if r == "5" else {})))
    add("rbt", "options", "-rbt 10 30 60", S(("-rbt", "10", "30", "60", *p, "{0}"), (t1,), (U,),
                                             **_py(radius=10.0, percentiles=(30.0, 60.0))))
    add("param", "options", "-param, the same as -rbt", S(("-param", "14", "75", "90", *p, "{0}"), (t1,), (U,), tier="standard"))
    for c in ("0.1", "0.5", "0.9"):
        add(f"clfrac-{c}", "options", f"-clfrac {c}", S(("-clfrac", c, *p, "{0}"), (t1,), (U,),
                                                      tier="smoke" if c == "0.5" else "standard"))
    add("mfrac", "options", "-mfrac, the same as -clfrac", S(("-mfrac", "0.3", *p, "{0}"), (t1,), (U,), tier="standard"))
    add("T2", "options", "-T2 on the T2-like phantom", S(("-T2", *p, "{0}"), (t2,), (U,), **_py(t2=True)))
    add("T2-T2", "options", "-T2 -T2: no second inversion, bright edges clipped",
        S(("-T2", "-T2", *p, "{0}"), (t2,), (U,), **_py(t2=2)))
    # AFNI-48 (rule B, decided 2026-10-23): three or more -T2 are refused by default; AFNI leaves the
    # output inverted, which `reproduce` gives.
    add("T2-T2-T2", "options", "-T2 three times: AFNI neither inverts back nor clips; refused by default (AFNI-48)",
        S(("-T2", "-T2", "-T2", *p, "{0}"), (t2,), (U,), fix_refuses="AFNI-48"))
    add("T2-x4", "options", "-T2 four times: as three (AFNI-48)",
        S(("-T2", "-T2", "-T2", "-T2", *p, "{0}"), (t2,), (U,), tier="standard", fix_refuses="AFNI-48"))
    add("T2-x3-EPI", "options", "-T2 three times with -EPI: -EPI sets one -T2, nothing refused",
        S(("-T2", "-T2", "-T2", "-EPI", *p, "{0}"), ("series-f32",), (U,), tier="standard"))
    add("T2up-95", "options", "-T2 -T2up 95", S(("-T2", "-T2up", "95", *p, "{0}"), (t2,), (U,), **_py(t2=True, t2_percentile=95.0)))
    add("GM", "options", "-GM", S(("-GM", *p, "{0}"), (t1,), (U,), **_py(gm=True)))
    add("GM-T2", "options", "-GM -T2", S(("-GM", "-T2", *p, "{0}"), (t2,), (U,), tier="standard"))
    add("nosquash", "options", "-nosquash", S(("-nosquash", *p, "{0}"), (t1,), (U,), **_py(squash=False)))
    add("oink", "options", "-oink, the same as -nosquash", S(("-oink", *p, "{0}"), (t1,), (U,), tier="standard"))
    add("ssave-amsave", "options", "-ssave and -amsave", S(("-ssave", "scale.nii", "-amsave", "am.nii", *p, "{0}"), (t1,),
                                                           (U, "scale.nii", "am.nii"),
                                                           **_py({"image": U, "scale": "scale.nii", "automasked": "am.nii"},
                                                                 save_scale=True, save_automask=True)))
    add("quiet", "options", "-quiet", S(("-quiet", *p, "{0}"), (t1,), (U,)))
    add("verb", "options", "-verb: the automask's messages too", S(("-verb", *p, "{0}"), (t1,), (U,)))
    add("verb-verb", "options", "-verb -verb", S(("-verb", "-verb", *p, "{0}"), (t1,), (U,), tier="standard"))
    add("useall", "options", "-useall: disabled, a warning", S(("-useall", *p, "{0}"), (t1,), (U,), tier="standard"))
    add("inset", "options", "-inset", S(("-inset", "{0}", *p), (t1,), (U,), tier="standard"))
    add("case-insensitive", "options", "-t2 -gm -NOSQUASH: AFNI's strcasecmp options",
        S(("-t2", "-gm", "-NOSQUASH", *p, "{0}"), (t2,), (U,), tier="standard"))
    add("duplo", "options", "1.2 million voxels: the half-size path", S(("-prefix", "uni.nii.gz", "{0}"), ("big",), ("uni.nii.gz",)))
    add("noduplo", "options", "1.2 million voxels with -noduplo", S(("-noduplo", "-prefix", "uni.nii.gz", "{0}"), ("big",),
                                                                   ("uni.nii.gz",), tier="standard", **_py({"image": "uni.nii.gz"}, duplo=False)))
    add("nodouble", "options", "-nodouble, the same as -noduplo", S(("-nodouble", "-Urad", "8", "-prefix", "uni.nii.gz", "{0}"), ("big",),
                                                                   ("uni.nii.gz",), tier="full"))
    add("duplo-T2", "options", "the half-size path with -T2 and -ssave", S(("-T2", "-ssave", "scale.nii.gz", "-prefix", "uni.nii.gz", "{0}"),
                                                                          ("big-t2",), ("uni.nii.gz", "scale.nii.gz"), tier="standard"))
    add("EPI", "options", "-EPI on a 4D series", S(("-EPI", *p, "{0}"), ("series-i16",), (U,), **_py(epi=True)))
    add("EPI-GM", "options", "-EPI -GM: -GM turned off", S(("-EPI", "-GM", *p, "{0}"), ("series-f32",), (U,), tier="standard"))
    add("EPI-3D", "options", "-EPI on one volume: a warning, -T2 kept", S(("-EPI", *p, "{0}"), (t2,), (U,), tier="standard"))
    add("4D-without-EPI", "options", "a 4D series without -EPI: sub-brick #0 only", S((*p, "{0}"), ("series-i16",), (U,)))

    # --- Storage types, scaling and geometry ---------------------------------------------------
    for img in ("head-i16", "head-u8", "head-i16-slope2", "head-i16-slope1", "head-f64", "head-i8", "head-nan",
                "head-oblique", "head-qform-only", "head-las"):
        tier = "smoke" if img in ("head-i16", "head-oblique") else "standard"
        add(img, "datatypes", f"{img}, with -ssave", S(("-ssave", "scale.nii", *p, "{0}"), (img,), (U, "scale.nii"), tier=tier))
    add("head-f32-slope-inter", "defects", "float32 with scl_slope and scl_inter (AFNI-02)",
        S((*p, "{0}"), ("head-f32-slope-inter",), (U,)))
    add("head-metre", "defects", "a grid in metres (AFNI-07)", S((*p, "{0}"), ("head-metre",), (U,)))
    add("head-nearly-plumb", "defects", "a grid 0.009 degrees from plumb (AFNI-16)", S((*p, "{0}"), ("head-nearly-plumb",), (U,)))
    add("nifti-scale-no", "defects", "AFNI_NIFTI_SCALE=NO on int16 with an intercept (AFNI-23)",
        S((*p, "{0}"), ("head-i16-intercept",), (U,), env=(("AFNI_NIFTI_SCALE", "NO"),)))
    add("T2up-100", "defects", "-T2 -T2up 100: one past the sorted values (AFNI-45)",
        S(("-T2", "-T2up", "100", *p, "{0}"), (t2,), (U,), **_py(t2=True, t2_percentile=100.0)))
    add("T2up-99.99", "defects", "-T2 -T2up 99.99 on a small mask: one past the values too (AFNI-45)",
        S(("-T2", "-T2up", "99.99", *p, "{0}"), ("slice",), (U,), tier="standard"))
    add("EPI-negative-slope", "defects", "-EPI on a series with scl_slope -2 (AFNI-01)",
        S(("-EPI", *p, "{0}"), ("series-negative",), (U,)))
    add("slab", "defects", "1 million voxels with an axis of 2: the half-size grid keeps 1 (AFNI-47)",
        S(("-prefix", "uni.nii.gz", "{0}"), ("slab",), ("uni.nii.gz",), tier="standard",
          undefined="AFNI-47", defined=720 * 720))

    # --- Dimensions ----------------------------------------------------------------------------
    for img, desc, tier in (
        ("slice", "a 2D slice (64 x 64 x 1)", "smoke"),
        ("two-slices", "two slices", "standard"),
        ("tiny", "2 x 2 x 2 voxels", "standard"),
        ("zeros", "an image of zeros", "standard"),
        ("constant", "a constant image", "standard"),
        ("series-one", "a 4D file of one volume", "standard"),
    ):
        add(img, "dimensions", desc, S((*p, "{0}"), (img,), (U,), tier=tier))
    add("slice-small", "errors", "a 2D slice smaller than 16 x 16: no automask", S((*p, "{0}"), ("slice-small",), (U,), expect="error"))
    add("slice-small-T2", "errors", "-T2 on a 2D slice smaller than 16 x 16: AFNI crashes (AFNI-46)",
        S(("-T2", *p, "{0}"), ("slice-small",), (U,), expect="crash"))

    # --- Errors --------------------------------------------------------------------------------
    for cid, args, expect in (
        ("Urad-low", ("-Urad", "3", "{0}"), "error"),
        ("Urad-high", ("-Urad", "120", "{0}"), "error"),
        ("rbt-bottom", ("-rbt", "18", "20", "80", "{0}"), "error"),
        ("rbt-order", ("-rbt", "18", "70", "60", "{0}"), "error"),
        ("rbt-top", ("-rbt", "18", "70", "95", "{0}"), "error"),
        ("rbt-missing", ("-rbt", "18", "60"), "error"),
        ("clfrac-low", ("-clfrac", "0.05", "{0}"), "error"),
        ("clfrac-missing", ("-clfrac",), "error"),
        ("T2up-low", ("-T2up", "89", "{0}"), "error"),
        ("T2up-high", ("-T2up", "101", "{0}"), "error"),
        ("T2up-missing", ("-T2up",), "crash"),
        ("prefix-missing", ("-prefix",), "error"),
        ("ssave-missing", ("-ssave",), "error"),
        ("input-twice", ("-input", "{0}", "-input", "{0}"), "error"),
        ("unknown-option", ("-bogus", "{0}"), "error"),
        ("no-dataset", ("-quiet",), "error"),
        ("missing-input", (*p, "/nonexistent/x.nii"), "error"),
    ):
        add(cid, "errors", " ".join(args).replace("{0}", "<input>"),
            S(args, (t1,), (U,), expect=expect, tier="smoke" if cid in ("unknown-option", "T2up-missing") else "standard"))
    add("default-prefix", "divergences", "no -prefix: AFNI writes its own format (Unifized+orig)",
        S(("{0}",), (t1,), ("Unifized+orig.HEAD",), expect="divergence", tier="standard",
          reason="AFNI's default prefix 'Unifized' is its own dataset format, which larmorx-gpl does "
          "not write: it refuses (NIfTI only)."))

    # --- AFNI's global options and environment variables --------------------------------------
    taken = ((U, "taken"),)
    add("exists", "global", "an existing output (AFNI-11)", S((*p, "{0}"), (t1,), (U,), files=taken, fix_refuses="AFNI-11"))
    add("overwrite", "global", "-overwrite", S(("-overwrite", *p, "{0}"), (t1,), (U,), files=taken))
    add("deconflict-yes", "global", "AFNI_DECONFLICT=YES: renamed", S((*p, "{0}"), (t1,), (U, "uni_AA1.nii"), files=taken,
                                                                     env=(("AFNI_DECONFLICT", "YES"),)))
    add("ssave-exists", "global", "an existing -ssave output (AFNI-11)", S(("-ssave", "scale.nii", *p, "{0}"), (t1,), (U, "scale.nii"),
                                                                          files=(("scale.nii", "taken"),), fix_refuses="AFNI-11", tier="standard"))
    add("D-write-type-2", "global", "-DAFNI_NIFTI_WRITE_TYPE=2", S(("-DAFNI_NIFTI_WRITE_TYPE=2", *p, "{0}"), (t1,), (U,), tier="standard"))
    add("quiet-startup", "environment", "AFNI_QUIET_STARTUP=YES", S((*p, "{0}"), (t1,), (U,), env=(("AFNI_QUIET_STARTUP", "YES"),)))
    add("message-prefix", "environment", "AFNI_MESSAGE_PREFIX", S(("-useall", *p, "{0}"), (t1,), (U,),
                                                                  env=(("AFNI_MESSAGE_PREFIX", "UN"),), tier="standard"))
    add("echo-commandline", "environment", "AFNI_ECHO_COMMANDLINE=YES", S((*p, "{0}"), (t1,), (U,),
                                                                          env=(("AFNI_ECHO_COMMANDLINE", "YES"), ("AFNI_DONT_LOGFILE", None)), tier="standard"))
    add("prefix-with-blank", "global", "a -prefix with a blank (AFNI-12)", S(("-prefix", "a b.nii", "{0}"), (t1,), ("a b.nii",), fix_writes="AFNI-12"))
    add("input-dir-with-blank", "global", "an input in a directory with a blank (AFNI-18)",
        S((*p, "{0}"), ("blank:head-f32",), (U,), tier="standard", fix_writes="AFNI-18"))
    add("missing-directory", "global", "an output in a missing directory (AFNI-26)",
        S(("-prefix", "nodir/uni.nii", "{0}"), (t1,), ("nodir/uni.nii",), tier="standard",
          fix_refuses="AFNI-26"))

    # --- Real data -----------------------------------------------------------------------------
    for i, (label, _) in enumerate(ph.fmriprep_runs()):
        add(label, "fmriprep", f"fMRIPrep's call on its BOLD reference's input ({label})",
            S(("-clfrac", "0.2", "-rbt", "18.3", "65.0", "90.0", "-prefix", "uni.nii.gz", "-T2", "-input", "{0}"),
              (f"fmriprep:{label}:unifize",), ("uni.nii.gz",), tier="smoke" if i == 0 else "standard",
              band=(1, 6), fmriprep=(("uni.nii.gz", f"fmriprep:{label}:uni"),),
              **_py({"image": "uni.nii.gz"}, clfrac=0.2, radius=18.3, percentiles=(65.0, 90.0), t2=True)))
    o = "td:openneuro"
    for name, t1w, tier in (
        ("ds000005", f"{o}/ds000005/sub-01/anat/sub-01_T1w.nii.gz", "standard"),
        ("ds001600", f"{o}/ds001600/sub-1/anat/sub-1_T1w.nii.gz", "full"),
        ("ds005040", f"{o}/ds005040/sub-002/anat/sub-002_T1w.nii.gz", "full"),
        ("ds005454", f"{o}/ds005454/sub-16/anat/sub-16_acq-mp2rage_T1w.nii.gz", "full"),
        ("ds005752", f"{o}/ds005752/sub-ON01016/ses-01/anat/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w.nii.gz", "full"),
    ):
        add(f"T1w-{name}", "real", f"AFNI's default on the T1w of {name} (the half-size path)",
            S(("-ssave", "scale.nii.gz", "-prefix", "uni.nii.gz", "{0}"), (t1w,), ("uni.nii.gz", "scale.nii.gz"),
              tier=tier, band=(6,)))
    add("T1w-GM-ds000005", "real", "-GM on the T1w of ds000005",
        S(("-GM", "-prefix", "uni.nii.gz", "{0}"), (f"{o}/ds000005/sub-01/anat/sub-01_T1w.nii.gz",), ("uni.nii.gz",), tier="full"))
    for name, bold, tier in (
        ("ds000005", f"{o}/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz", "standard"),
        ("ds005454", f"{o}/ds005454/sub-16/func/sub-16_task-rest_bold.nii.gz", "full"),
    ):
        add(f"EPI-{name}", "real", f"-EPI on the BOLD series of {name}",
            S(("-EPI", "-prefix", "uni.nii.gz", "{0}"), (bold,), ("uni.nii.gz",), tier=tier, band=(6,)))
    # fmt: on
    return out


@functools.cache
def _scenario_list() -> list[tuple[str, str, str, ar.Scenario]]:
    return _scenarios()


#: The implementations this suite validates.
IMPLEMENTATIONS = ("original", "replica")


def spec(implementation: str = "original") -> ar.ToolSpec:
    """The suite's runner for ``implementation``. The clean-room original's float32 outputs
    may differ from AFNI's in the last bits where AFNI's squash calls glibc's ``tanhf``, which
    is not correctly rounded (the original's is): within PLAN.md §11.3's deterministic
    arithmetic threshold, bit-identity reported."""
    if implementation not in IMPLEMENTATIONS:
        raise ValueError(f"implementation must be one of {IMPLEMENTATIONS}, not {implementation!r}")
    tolerance = ar.FLOAT_RTOL if implementation == "original" else None
    return ar.ToolSpec(PROGRAM, "unifize", _scenario_list, _input, implementation, tolerance)


def cases(tier: str) -> list[Case]:
    return ar.cases_of(spec(), tier)


def suite(implementation: str = "original") -> Suite:
    """The 3dUnifize suite for the clean-room original (``larmorx afni 3dUnifize``, in
    process) or the GPL replica (``larmorx-gpl afni 3dUnifize``, which larmorx runs as a
    separate process); both through larmorx's command line and its Python API."""
    tool_spec = spec(implementation)
    replica = implementation == "replica"
    who = "replica" if replica else "original"

    def run_case(case: Case, checks: CheckList) -> None:
        ar.run_case(tool_spec, case, checks)

    tool = (
        f"`larmorx-gpl afni 3dUnifize` ({ar.replica_version()}; crate larmorx-gpl-afni, the "
        "GPL-3.0-or-later replica translated from AFNI 25.2.09's source), run by larmorx as a "
        "separate process: `larmorx afni 3dUnifize` with `LARMORX_IMPLEMENTATION=replica` and "
        '`lx.afni.unifize(..., implementation="replica")`'
        if replica
        else "`larmorx afni 3dUnifize` / `lx.afni.unifize` (crate larmorx-afni, the Apache-2.0 "
        "clean-room original written from `specs/3dUnifize.md`), with "
        '`implementation="original"`'
    )
    outputs = (
        "the same values, bit for bit, and the same NIfTI header bytes"
        if replica
        else "the same NIfTI header bytes; the values bit for bit, or float32 values within "
        "max |diff| ≤ 1e-5 × max |AFNI output| (PLAN.md §11.3) with bit-identity reported (AFNI's "
        "squash calls glibc's tanhf, which is not correctly rounded on some arguments)"
    )
    return Suite(
        name="afni-unifize-replica" if replica else "afni-unifize",
        title="3dUnifize (replica)" if replica else "3dUnifize",
        tool=tool,
        reference=f"3dUnifize from AFNI ({ar.afni_build(PROGRAM)}, with OpenMP), the binary built by `scripts/build_afni_oracle.sh`",
        thresholds=[
            (
                "Exit status",
                "the same (both succeed, or both reject the arguments); where AFNI crashes, larmorx exits 1",
            ),
            (
                "Messages",
                "standard output and error byte for byte (AFNI's banner and compile date, larmorx's own lines and the elapsed time set aside)",
            ),
            ("Every output (the unifized image, -ssave, -amsave)", outputs),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        highlights=lambda results: ar.highlights(
            results,
            PROGRAM,
            "unifize",
            "a missing option value, AFNI-40; an empty automask dereferenced, AFNI-46",
            who,
        ),
        notes=(
            "Both programs run in directories of their own on the same input files, in the same "
            "environment (no AFNI variables of this process, `HOME` an empty directory, "
            "`AFNI_DONT_LOGFILE=YES`, `AFNI_NIFTI_NOEXT=YES`, plus each case's variables); "
            "outputs are compared as stored (data bytes, header bytes decompressed). AFNI runs "
            "with OpenMP at this machine's default thread count unless a case sets "
            "`OMP_NUM_THREADS`; the replica with its own threads (results do not depend on "
            f"them). The {who} is compared in `reproduce` mode; the fix pass runs its default. "
            "The real cases also run AFNI at other OpenMP thread counts and fMRIPrep 25.2.6's own "
            "AFNI (in its image) and report how far they are from the oracle; the fMRIPrep cases "
            f"compare the {who} with fMRIPrep's own output of the node as well."
        ),
    )
