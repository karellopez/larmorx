# SPDX-License-Identifier: Apache-2.0
"""3dUnifize's evidence: fMRIPrep's call on a real BOLD reference (``-T2``) by AFNI 25.2.09 at 1
and 6 OpenMP threads, fMRIPrep's own output, and the larmorx-gpl replica at 1 and 6 threads in
its default and ``reproduce`` modes; AFNI's default on a real T1w (more than a million voxels:
the half-size path), with its scale image, by AFNI at 1 and 6 threads, fMRIPrep's build of AFNI
and the replica; a deliberate correction (AFNI-45, ``-T2up 100``) on the BOLD reference; the
record's cases.

The replica (``larmorx-gpl``, GPL-3.0-or-later) runs as a separate program; the clean-room
original (``larmorx afni 3dUnifize``, ``LARMORX_IMPLEMENTATION=original``) has cache entries of
its own beside the replica's, with AFNI run again with CORE-MATH's correctly rounded functions
preloaded (the original's squash calls a correctly rounded ``tanhf``, AFNI glibc's).
"""

from __future__ import annotations

import copy
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import afni_automask as am
from larmorx_validation.evidence import afni_step, common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "afni-unifize"
PROGRAM = "3dUnifize"
RUN = am.RUN
NODE = f"{am.WF}/unifize"
INPUT = f"{am.WF}/first_mask/sub-002_task-clip_run-1_desc-hmc_boldref_corrected_brain_masked.nii.gz"
FMRIPREP_OUT = f"{NODE}/uni.nii.gz"
T1W = "openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz"
T1W_RUN = "ds000005-sub-01-noreconall"
T1W_MASK = "out/sub-01/anat/sub-01_desc-brain_mask.nii.gz"
THREADS = ("1", "6")


def _runners(threads: str, image: bool) -> list[tuple[str, str, str | None]]:
    rs = [(f"afni-{threads}", "afni", None), (f"replica-{threads}", "replica", "fix")]
    if threads == THREADS[-1]:
        rs.append(("replica-reproduce", "replica", "reproduce"))
        if image:
            rs.append(("image", "image", None))
    return rs


def _compute(e: fcache.Entry) -> None:
    run = common.fmriprep_run(RUN)
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    inp = e.input("BOLD reference, masked (the node's input)", run / INPUT)
    e.input("fMRIPrep's output (the node's)", run / FMRIPREP_OUT)
    call = am.fmriprep_args(command, {"uni.nii.gz": "{out}.nii.gz", Path(INPUT).name: str(inp)})
    if "-T2" not in call or call[-2:] != ["-input", str(inp)]:
        raise AssertionError(f"{TOOL}: an unexpected fMRIPrep command: {command}")
    for t in THREADS:
        am.run_all(e, PROGRAM, "", call, [inp], runners=_runners(t, image=False), threads=t)
    t2up = [*call[:-2], "-T2up", "100", *call[-2:]]
    am.run_all(e, PROGRAM, "t2up100-", t2up, [inp], runners=[
        ("afni", "afni", None), ("replica", "replica", "fix"),
        ("replica-reproduce", "replica", "reproduce")], threads=THREADS[-1])  # fmt: skip
    e.finish(
        versions=am.versions(PROGRAM),
        command=command,
        threads={"afni": list(THREADS), "larmorx": list(THREADS)},
        settings="fMRIPrep 25.2.6's 3dUnifize node (unifize) of ds005040 sub-002 task-clip "
        "run-1, its arguments from command.txt, its input from the run's working directory; "
        "AFNI 25.2.09 (OMP_NUM_THREADS 1 and 6) and the larmorx-gpl replica (1 and 6, both "
        "modes); the same with -T2up 100 (AFNI-45)",
    )


def _compute_t1w(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    t1 = e.input("T1w", Path(td.get(T1W)))
    args = ["-ssave", "{out}-scale.nii.gz", "-prefix", "{out}.nii.gz", "-input", str(t1)]
    for t in THREADS:
        am.run_all(e, PROGRAM, "", args, [t1], runners=_runners(t, image=True), threads=t)
    e.finish(
        versions=am.versions(PROGRAM),
        command="3dUnifize -ssave scale.nii.gz -prefix uni.nii.gz -input sub-01_T1w.nii.gz",
        threads={"afni": list(THREADS), "larmorx": list(THREADS), "afni_in_fmriprep_image": 6},
        settings="AFNI's default on OpenNeuro ds000005 sub-01's T1w (160 × 192 × 192: the "
        "half-size path), with its scale image; AFNI 25.2.09 at 1 and 6 OpenMP threads, "
        "fMRIPrep's build of AFNI at 6, the larmorx-gpl replica at 1 and 6 (both modes)",
    )


def _afni_correctly_rounded(e: fcache.Entry, args: list[str], threads: str = "1") -> None:
    """AFNI with CORE-MATH's correctly rounded functions preloaded (``afni-cr``)."""
    from larmorx_validation.parity import afni_replica as ar

    lib = ar.crmath_preload()
    if lib is None:
        raise FileNotFoundError("a CORE-MATH preload with tanhf (scripts/build_crmath_preload.sh)")
    afni, _, _ = am.binaries(PROGRAM)
    add, unset = am.environment()
    home = e.dir / ".home"
    home.mkdir(exist_ok=True)
    stem = str(e.dir / "afni-cr")
    argv = [a.replace("{out}", stem) for a in args]
    env = {**add, "HOME": str(home), "OMP_NUM_THREADS": threads, "LD_PRELOAD": str(lib)}
    e.run("afni-cr", [afni, *argv], env=env, unset=unset)


#: The clean-room original's runs.
ORIGINAL_RUNS = [
    ("original-1", "fix", "1"),
    ("original-6", "fix", "6"),
    ("original-reproduce", "reproduce", "1"),
]


def _compute_original(e: fcache.Entry) -> None:
    run = common.fmriprep_run(RUN)
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    inp = e.input("BOLD reference, masked (the node's input)", run / INPUT)
    call = am.fmriprep_args(command, {"uni.nii.gz": "{out}.nii.gz", Path(INPUT).name: str(inp)})
    am.run_original(e, PROGRAM, "", call, runs=ORIGINAL_RUNS)
    t2up = [*call[:-2], "-T2up", "100", *call[-2:]]
    am.run_original(e, PROGRAM, "t2up100-", t2up, runs=[("original", "fix", "6"),
                    ("original-reproduce", "reproduce", "6")])  # fmt: skip
    _afni_correctly_rounded(e, call)
    e.finish(
        versions={"larmorx": am.larmorx_version()},
        command=command,
        threads={"larmorx": ["1", "6"], "afni-cr": ["1"]},
        settings="the clean-room original (larmorx afni 3dUnifize, LARMORX_IMPLEMENTATION="
        "original) on fMRIPrep's call of the replica's entry: default mode at 1 and 6 threads, "
        "reproduce at 1; with -T2up 100 (AFNI-45) in both modes; AFNI 25.2.09 at 1 thread with "
        "CORE-MATH's correctly rounded functions preloaded (tanhf, cbrtf among them)",
    )


def _compute_t1w_original(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    t1 = e.input("T1w", Path(td.get(T1W)))
    args = ["-ssave", "{out}-scale.nii.gz", "-prefix", "{out}.nii.gz", "-input", str(t1)]
    am.run_original(e, PROGRAM, "", args, runs=ORIGINAL_RUNS)
    _afni_correctly_rounded(e, args, threads="6")
    e.finish(
        versions={"larmorx": am.larmorx_version()},
        command="3dUnifize -ssave scale.nii.gz -prefix uni.nii.gz -input sub-01_T1w.nii.gz",
        threads={"larmorx": ["1", "6"], "afni-cr": ["6"]},
        settings="the clean-room original on ds000005 sub-01's T1w (the half-size path), default "
        "mode at 1 and 6 threads, reproduce at 1; AFNI 25.2.09 at 6 OpenMP threads with "
        "CORE-MATH's correctly rounded functions preloaded",
    )


def original_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"{am.CASE}-original",
        "the clean-room original on fMRIPrep's 3dUnifize call of ds005040 sub-002 task-clip "
        "run-1 (and with -T2up 100), and AFNI with correctly rounded maths",
        _compute_original,
    )


def t1w_original_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        "ds000005-t1w-original",
        "the clean-room original on ds000005 sub-01's T1w, and AFNI with correctly rounded maths",
        _compute_t1w_original,
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        am.CASE,
        "fMRIPrep's 3dUnifize of the BOLD reference of ds005040 sub-002 task-clip run-1, and "
        "with -T2up 100: AFNI at 1 and 6 threads, fMRIPrep, the replica in both modes",
        _compute,
    )


def t1w_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        "ds000005-t1w",
        "AFNI's default 3dUnifize of ds000005 sub-01's T1w with its scale image: AFNI at 1 and "
        "6 threads, fMRIPrep's AFNI, the replica in both modes",
        _compute_t1w,
    )


# ----------------------------------------------------------------------------------- export

OUTPUT = [{"id": "uni", "label": "Unifized image", "unit": ""}]
OUTPUTS_T1W = [OUTPUT[0], {"id": "scale", "label": "Scale image (-ssave)", "unit": ""}]
REF_PANELS = [
    ("afni-1", "afni-1", "original", "AFNI 25.2.09, 3dUnifize", "the repository's build, 1 thread"),
    ("afni-6", "afni-6", "original", "AFNI 25.2.09, 3dUnifize", "the repository's build, 6 threads"),
    ("fmriprep", None, "fmriprep", "fMRIPrep 25.2.6, as it ran", "AFNI 25.2.09 in its image"),
    ("replica-1", "replica-1", "larmorx", "larmorx-gpl replica", "default mode, 1 thread"),
    ("replica-6", "replica-6", "larmorx", "larmorx-gpl replica", "default mode, 6 threads"),
    ("replica-reproduce", "replica-reproduce", "larmorx", "larmorx-gpl replica",
     "reproduce mode, 6 threads"),
]  # fmt: skip
#: The clean-room original's panels and AFNI with correctly rounded maths.
ORIGINAL_PANELS = [
    ("original-1", "original-1", "larmorx", "LARMORX clean-room original",
     "default mode, 1 thread"),
    ("original-6", "original-6", "larmorx", "LARMORX clean-room original",
     "default mode, 6 threads"),
    ("original-reproduce", "original-reproduce", "larmorx", "LARMORX clean-room original",
     "reproduce mode, 1 thread"),
    ("afni-cr", "afni-cr", "original", "AFNI 25.2.09, correctly rounded maths",
     "CORE-MATH's tanhf, cbrtf, ... preloaded"),
]  # fmt: skip


#: AFNI 26.3.05 on fMRIPrep's call at 1 thread (the replica's reference since 2026-10-29).
PANEL_26 = (
    "afni26-1",
    "afni26-1",
    "original",
    "AFNI 26.3.05, 3dUnifize",
    "the repository's build, 1 thread; the replica's reference",
)


def afni26_diffs() -> list[dict]:
    """The replica against AFNI 26.3.05, and AFNI's own step from 25.2.09 (1 thread each)."""
    d = common.d
    return [
        d("replica-afni26", "replica-1", "afni26-1", "main",
          "replica − AFNI 26.3.05, 1 thread each", common.ROLE_LX),
        d("afni26-afni", "afni26-1", "afni-1", "variability",
          "AFNI 26.3.05 − AFNI 25.2.09, 1 thread each",
          "the original against itself: the version step"),
    ]  # fmt: skip


def original_diffs() -> list[dict]:
    d = common.d
    return [
        d("original-afni", "original-1", "afni-1", "main",
          "original − AFNI 25.2.09, 1 thread each", common.ROLE_LX),
        d("original-afnicr", "original-1", "afni-cr", "main",
          "original − AFNI with correctly rounded maths", common.ROLE_LX),
        d("original-replica", "original-1", "replica-1", "check",
          "original − replica, 1 thread each", common.ROLE_SELF),
        d("original6-original1", "original-6", "original-1", "check",
          "original, 6 threads − 1 thread", common.ROLE_SELF),
        d("original-repro", "original-1", "original-reproduce", "check",
          "original, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip


def original_verdicts(diffs: list[dict], same: dict[str, bool]) -> None:
    """The original's verdicts: bit-identical, or identical with correctly rounded maths when
    only glibc's rounding separates it from AFNI."""
    for x in diffs:
        i = x["id"]
        if i == "original-afni" and not same[i] and same.get("original-afnicr"):
            x["verdict"] = "identical-cr"
            x["reason"] = (
                "AFNI's squash calls glibc's tanhf, which is not correctly rounded on some "
                "arguments; the original's is. With CORE-MATH's correctly rounded functions "
                "preloaded, AFNI gives the original's bytes, every value."
            )
        elif i == "original-afnicr" and same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = "The same values, every one: AFNI with correctly rounded maths."
        elif i == "original6-original1" and same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = "The original's result does not depend on its thread count."
        elif i == "original-replica" and not same[i] and same.get("original-afnicr"):
            x["verdict"] = "identical-cr"
            x["reason"] = (
                "The replica carries glibc 2.35's tanhf, the original a correctly rounded one: "
                "they differ exactly where AFNI and AFNI with correctly rounded maths do."
            )
        elif (
            i == "original-fmriprep"
            and not same[i]
            and same.get("original-afnicr")
            and same.get("fmriprep-afni")
        ):
            x["verdict"] = "identical-cr"
            x["reason"] = (
                "fMRIPrep's own output is AFNI's; the original differs from it only where "
                "glibc's tanhf is not correctly rounded (AFNI with correctly rounded maths gives "
                "the original's bytes)."
            )
        elif i == "original-repro" and same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = (
                "No correction of the register applies to this input: both modes give the "
                "same bytes."
            )


def _verdict(x: dict, same: dict[str, bool]) -> None:
    i = x["id"]
    x.setdefault("links", [])
    if i == "afni26-afni":
        x["verdict"] = None
        x["reason"] = am.AFNI26_SAME if same[i] else "AFNI 26.3.05 and 25.2.09 differ on this call."
    elif x["kind"] == "variability":
        x["verdict"] = None
        x["reason"] = {
            "afni6-afni1": "AFNI computes every voxel's local mean independently in its OpenMP "
            "loop: the same bytes at 1 and 6 threads."
            if same[i]
            else "AFNI's result changes with its thread count on this input.",
            "fmriprep-afni": "fMRIPrep's own run (its image's build of AFNI) gives the "
            "repository's AFNI result, every value."
            if same[i]
            else "fMRIPrep's own output differs from the repository's AFNI on this input.",
            "image-afni": "Two builds of AFNI 25.2.09 (fMRIPrep's image and the repository's) "
            "give the same bytes."
            if same[i]
            else "AFNI's two builds differ on this input.",
        }.get(i, "The same bytes." if same[i] else "They differ.")
    elif same[i]:
        x["verdict"] = "bit-identical"
        x["reason"] = {
            "replica-repro": "No correction of the register applies to this input: both modes "
            "give the same bytes.",
            "replica6-replica1": "The same bytes on any number of threads.",
        }.get(i, "The same values, every one.")
    else:
        x["verdict"] = "open"
        x["reason"] = "Not accounted for by the other runs of this example."


def _panels(
    ex, exm, e: fcache.Entry, panels, outputs, fmriprep: Path | None = None, full=None
) -> None:
    """The panels of ``panels`` from the entry ``e``; those named in ``full`` (a dict) are
    loaded into it instead, compared but not shown."""
    for pid, stem, role, label, sub in panels:
        if full is not None:
            arrays = {"uni": common.load(str(e.dir / f"{stem}.nii.gz"))}
            if len(outputs) > 1 and (e.dir / f"{stem}-scale.nii.gz").is_file():
                arrays["scale"] = common.load(str(e.dir / f"{stem}-scale.nii.gz"))
            full[pid] = arrays
            continue
        if stem is None:
            arrays = {"uni": common.load(str(fmriprep))}
        else:
            arrays = {"uni": common.load(str(e.dir / f"{stem}.nii.gz"))}
            if len(outputs) > 1:
                arrays["scale"] = common.load(str(e.dir / f"{stem}-scale.nii.gz"))
        ex.add(exm.Panel(pid, role, label, sub, arrays))


def _example_ref(ctx: core.Context, e: fcache.Entry, o: fcache.Entry, step: dict) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    run = common.fmriprep_run(RUN)
    img = nib.load(str(run / INPUT))
    data = np.asanyarray(img.dataobj)
    show = ndimage.binary_dilation(data != 0, iterations=1)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("ref", "input", "ds005040 sub-002 BOLD reference, masked",
                     "3dUnifize's input in fMRIPrep's BOLD reference workflow: N4-corrected, "
                     "skull-stripped once (float32)", {"all": data}))  # fmt: skip
    _panels(ex, exm, e, REF_PANELS, OUTPUT, run / FMRIPREP_OUT)
    _panels(ex, exm, o, ORIGINAL_PANELS[:1] + ORIGINAL_PANELS[2:3], OUTPUT)
    _panels(ex, exm, e, [PANEL_26], OUTPUT)
    full: dict = {}
    _panels(ex, exm, o, [ORIGINAL_PANELS[1], ORIGINAL_PANELS[3]], OUTPUT, full=full)
    d = common.d
    diffs = [
        *original_diffs()[:2],
        d("replica-afni", "replica-1", "afni-1", "main", "replica − AFNI 25.2.09, 1 thread each",
          common.ROLE_LX),
        d("afni6-afni1", "afni-6", "afni-1", "variability", "AFNI 25.2.09, 6 threads − 1 thread",
          common.ROLE_ORIG),
        d("fmriprep-afni", "fmriprep", "afni-1", "variability",
          "fMRIPrep's own output − the repository's AFNI", common.ROLE_ORIG),
        d("replica-fmriprep", "replica-1", "fmriprep", "fmriprep", "replica − fMRIPrep's own output",
          common.ROLE_FMRIPREP),
        d("replica6-replica1", "replica-6", "replica-1", "check", "replica, 6 threads − 1 thread",
          common.ROLE_SELF),
        d("replica-repro", "replica-6", "replica-reproduce", "check",
          "replica, default − reproduce mode", common.ROLE_SELF),
        d("original-fmriprep", "original-1", "fmriprep", "fmriprep",
          "original − fMRIPrep's own output", common.ROLE_FMRIPREP),
        *original_diffs()[2:],
        *afni26_diffs(),
    ]  # fmt: skip
    common.compare_all(ex, diffs, OUTPUT, full=full)
    same = common.identical(diffs)
    for x in diffs:
        _verdict(x, same)
    original_verdicts(diffs, same)
    at, _k = common.centre_view(ex)
    default = {
        "output": "uni",
        "plane": "axial",
        "voxel": at,
        "panels": ["ref", "afni-1", "original-1", "replica-1"],
        "diffs": ["original-afni", "replica-afni", "afni6-afni1"],
        "compare": {"a": "afni-1", "b": "original-1"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(OUTPUT), default=default, window_panel="afni-1",
                        caption="the crosshair at the brain's centre.")  # fmt: skip
    vers = info.get("versions", {})
    u = common.load(str(e.dir / "afni-1.nii.gz"))
    return {
        "id": "fmriprep-unifize",
        "title": "fMRIPrep's uniformization of a real BOLD reference",
        "summary": (
            "The 3dUnifize node fMRIPrep 25.2.6 ran on the BOLD reference of OpenNeuro ds005040 "
            "sub-002 (task clip, run 1), N4-corrected and skull-stripped once: its arguments "
            "(-T2: the contrast inverted, unifized, inverted back) given to AFNI 25.2.09 at 1 "
            "and 6 OpenMP threads, to AFNI 26.3.05 at 1 (the replica's reference since "
            "2026-10-29), to the larmorx-gpl replica and to LARMORX's clean-room original at 1 "
            "and 6 threads in both modes. fMRIPrep's own output is the panel it produced in its "
            "run."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{am.DATA['dataset']}, {am.DATA['subject']} ({am.DATA['licence']}): "
                f"{' × '.join(map(str, img.shape))} float32 values; shown inside the input's "
                "own mask, dilated by one voxel",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `bold_fit_wf/enhance_and_skullstrip_bold_wf/unifize`",
            },
            {"label": "Command", "text": "`" + am.short_command(info["command"]) + "`"},
            {
                "label": "Programs",
                "text": f"{vers.get('afni', '?')} (the repository's oracle build; OpenMP); AFNI "
                f"{vers.get('afni_in_fmriprep_image', '?')} in fMRIPrep's image; "
                f"{vers.get('larmorx-gpl', '?')} (the replica, with a port of glibc 2.35's "
                f"tanhf), from larmorx at commit `{info['larmorx_commit'][:12]}`; "
                f"{o.info().get('versions', {}).get('larmorx', 'larmorx')} at commit "
                f"`{o.info()['larmorx_commit'][:12]}` (a correctly rounded tanhf)",
            },
            {
                "label": "Output",
                "text": f"{int(np.count_nonzero(u)):,} non-zero values (AFNI's final automask), "
                f"up to {float(u.max()):.6g}",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
            afni_step.fact(step, "fMRIPrep's call"),
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": common.provenance(
            info, f"{TOOL}/{e.case}", [am.DATA], [am.original_provenance(o)]
        ),
    }


def _example_t1w(ctx: core.Context, e: fcache.Entry, o: fcache.Entry, step: dict) -> dict:
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    img = nib.load(td.get(T1W))
    brain = common.load(str(common.fmriprep_run(T1W_RUN) / T1W_MASK)) > 0
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("t1w", "input", "ds000005 sub-01 T1w", "as stored (int16), "
                     "3dUnifize's input", {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    panels = [
        ("afni-1", "afni-1", "original", "AFNI 25.2.09, 3dUnifize", "the repository's build, 1 thread"),
        ("afni-6", "afni-6", "original", "AFNI 25.2.09, 3dUnifize", "the repository's build, 6 threads"),
        ("image", "image", "original", "AFNI 25.2.09 in fMRIPrep's image", "fMRIPrep 25.2.6's build, 6 threads"),
        ("replica-1", "replica-1", "larmorx", "larmorx-gpl replica", "default mode, 1 thread"),
        ("replica-6", "replica-6", "larmorx", "larmorx-gpl replica", "default mode, 6 threads"),
        ("replica-reproduce", "replica-reproduce", "larmorx", "larmorx-gpl replica",
         "reproduce mode, 6 threads"),
    ]  # fmt: skip
    _panels(ex, exm, e, panels, OUTPUTS_T1W)
    _panels(ex, exm, o, ORIGINAL_PANELS[:1] + ORIGINAL_PANELS[2:3], OUTPUTS_T1W)
    _panels(ex, exm, e, [PANEL_26], OUTPUTS_T1W)
    full: dict = {}
    _panels(ex, exm, o, [ORIGINAL_PANELS[1], ORIGINAL_PANELS[3]], OUTPUTS_T1W, full=full)
    d = common.d
    diffs = [
        *original_diffs()[:2],
        d("replica-afni", "replica-1", "afni-1", "main", "replica − AFNI 25.2.09, 1 thread each",
          common.ROLE_LX),
        d("afni6-afni1", "afni-6", "afni-1", "variability", "AFNI 25.2.09, 6 threads − 1 thread",
          common.ROLE_ORIG),
        d("image-afni", "image", "afni-6", "variability",
          "fMRIPrep's AFNI − the repository's AFNI, 6 threads each", common.ROLE_ORIG),
        d("replica6-replica1", "replica-6", "replica-1", "check", "replica, 6 threads − 1 thread",
          common.ROLE_SELF),
        d("replica-repro", "replica-6", "replica-reproduce", "check",
          "replica, default − reproduce mode", common.ROLE_SELF),
        *original_diffs()[2:],
        *afni26_diffs(),
    ]  # fmt: skip
    common.compare_all(ex, diffs, OUTPUTS_T1W, full=full)
    same = common.identical(diffs)
    for x in diffs:
        _verdict(x, same)
    original_verdicts(diffs, same)
    at, _k = common.centre_view(ex)
    default = {
        "output": "uni",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1w", "afni-1", "original-1", "replica-1"],
        "diffs": ["original-afni", "replica-afni", "afni6-afni1"],
        "compare": {"a": "afni-1", "b": "original-1"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(OUTPUTS_T1W), default=default, window_panel="afni-1",
                        caption="the crosshair at the brain's centre.")  # fmt: skip
    vers = info.get("versions", {})
    n = int(np.prod(img.shape))
    return {
        "id": "t1w-unifize",
        "title": "AFNI's default on a real T1w",
        "summary": (
            "3dUnifize's main use: a T1w's white matter made uniform and scaled to 1000. The "
            f"T1w of OpenNeuro ds000005 sub-01 has {n:,} voxels, more than a million: the "
            "local white-matter level is computed on a half-size copy and brought back (AFNI's "
            "duplo path). AFNI 25.2.09 at 1 and 6 threads, fMRIPrep's build of AFNI, the "
            "replica and the clean-room original in both modes; -ssave writes the scale image "
            "each voxel is multiplied by."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01's T1w ({common.DS000005['licence']}); "
                "shown inside fMRIPrep's brain mask, dilated by 2 voxels",
            },
            {"label": "Command", "text": f"`{info['command']}`"},
            {
                "label": "Programs",
                "text": f"{vers.get('afni', '?')}; AFNI {vers.get('afni_in_fmriprep_image', '?')} "
                f"in fMRIPrep's image; {vers.get('larmorx-gpl', '?')}; "
                f"{o.info().get('versions', {}).get('larmorx', 'larmorx')}",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
            afni_step.fact(step, "the call"),
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": common.provenance(
            info, f"{TOOL}/{e.case}", [common.DS000005], [am.original_provenance(o)]
        ),
    }


def _example_t2up(
    ctx: core.Context, e: fcache.Entry, o: fcache.Entry, reg: dict, step: dict
) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    run = common.fmriprep_run(RUN)
    img = nib.load(str(run / INPUT))
    data = np.asanyarray(img.dataobj)
    show = ndimage.binary_dilation(data != 0, iterations=1)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("ref", "input", "ds005040 sub-002 BOLD reference, masked",
                     "as in the first example", {"all": data}))  # fmt: skip
    panels = [
        ("afni", "t2up100-afni", "original", "AFNI 25.2.09, -T2up 100", "the repository's build"),
        ("afni26", "t2up100-afni26", "original", "AFNI 26.3.05, -T2up 100",
         "the repository's build; the replica's reference"),
        ("replica", "t2up100-replica", "larmorx", "larmorx-gpl replica, -T2up 100",
         "default mode: corrected"),
        ("replica-reproduce", "t2up100-replica-reproduce", "larmorx",
         "larmorx-gpl replica, -T2up 100", "reproduce mode"),
        ("afni-default", "afni-6", "original", "AFNI 25.2.09, fMRIPrep's call",
         "-T2up 98.5, the default"),
    ]  # fmt: skip
    _panels(ex, exm, e, panels, OUTPUT)
    _panels(ex, exm, o, [
        ("original", "t2up100-original", "larmorx", "LARMORX clean-room original, -T2up 100",
         "default mode: corrected"),
        ("original-reproduce", "t2up100-original-reproduce", "larmorx",
         "LARMORX clean-room original, -T2up 100", "reproduce mode"),
    ], OUTPUT)  # fmt: skip
    d = common.d
    diffs = [
        d("replica-afni", "replica", "afni", "main", "replica (default) − AFNI, -T2up 100",
          common.ROLE_LX),
        d("repro-afni", "replica-reproduce", "afni", "main",
          "replica (reproduce) − AFNI, -T2up 100", common.ROLE_LX),
        d("original-afni", "original", "afni", "main", "original (default) − AFNI, -T2up 100",
          common.ROLE_LX),
        d("original-repro-afni", "original-reproduce", "afni", "main",
          "original (reproduce) − AFNI, -T2up 100", common.ROLE_LX),
        d("repro-afni26", "replica-reproduce", "afni26", "main",
          "replica (reproduce) − AFNI 26.3.05, -T2up 100", common.ROLE_LX),
        d("afni26-afni", "afni26", "afni", "variability",
          "AFNI 26.3.05 − AFNI 25.2.09, -T2up 100",
          "the original against itself: the version step"),
    ]  # fmt: skip
    common.compare_all(ex, diffs, OUTPUT)
    same = common.identical(diffs)
    a = common.load(str(e.dir / "t2up100-afni.nii.gz"))
    zero = not np.any(a)
    link = [am._link(reg, "AFNI-45")]
    for x in diffs:
        x["links"] = link
        if x["id"] == "afni26-afni":
            x["verdict"] = None
            x["reason"] = (
                "AFNI 26.3.05 gives AFNI 25.2.09's bytes on this call: the read one past the "
                "sorted values (AFNI-45) is in both releases, and both read 0 there on this machine."
                if same[x["id"]]
                else "AFNI 26.3.05 and 25.2.09 differ on this call: the read one past the sorted "
                "values (AFNI-45) is of memory neither writes."
            )
        elif x["id"] in ("replica-afni", "original-afni"):
            repro = "repro-afni" if x["id"] == "replica-afni" else "original-repro-afni"
            x["verdict"] = "d13" if not same[x["id"]] and same[repro] else "open"
            x["reason"] = (
                "AFNI-45, corrected by default with a warning: at -T2up 100 the percentile's "
                "index falls one past the sorted values, where AFNI reads memory it never wrote "
                + ("(here 0: every value inverts to 0, and so does the output)" if zero else "")
                + "; the default inverts about the largest value, the 100th percentile. "
                "Reproduce mode fills that read with 0."
            )
        elif same[x["id"]]:
            x["verdict"] = "bit-identical"
            x["reason"] = (
                "reproduce mode gives AFNI's result: the read past the end filled with 0, as "
                "AFNI read it on this machine."
            )
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example."
    at, _k = common.centre_view(ex)
    default = {
        "output": "uni",
        "plane": "axial",
        "voxel": at,
        "panels": ["ref", "afni", "original", "afni-default"],
        "diffs": ["original-afni", "original-repro-afni", "replica-afni"],
        "compare": {"a": "afni-default", "b": "original"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(OUTPUT), default=default, window_panel="replica",
                        caption="the crosshair at the brain's centre.")  # fmt: skip
    a45 = reg.get("AFNI-45", {})
    return {
        "id": "afni-45",
        "title": "AFNI-45: -T2up 100 reads past the sorted values",
        "summary": (
            "With -T2, 3dUnifize inverts the contrast about an upper percentile of the masked "
            "values (-T2up, 98.5 by default, 90 to 100 allowed). At 100 the percentile's index "
            "is one past the end of the sorted values. fMRIPrep's call with -T2up 100 added, "
            "given to AFNI 25.2.09, to AFNI 26.3.05, to the replica and to the clean-room "
            "original in both modes, beside fMRIPrep's own call."
        ),
        "facts": [
            {"label": "Input", "text": "the masked BOLD reference of the first example"},
            {
                "label": "Command",
                "text": "`3dUnifize -clfrac 0.2 -rbt 18.3 65.0 90.0 -prefix uni.nii.gz -T2 "
                "-T2up 100 -input ref.nii.gz`",
            },
            {
                "label": "Defect",
                "text": f"[AFNI-45](repo:{link[0]['href']}): "
                + (a45.get("title") or "-T2up at or near 100 reads one past the sorted values")
                + "; corrected by default, with a warning",
            },
            {
                "label": "AFNI's output",
                "text": f"all {a.size:,} values 0"
                if zero
                else f"{int(np.count_nonzero(a)):,} non-zero values",
            },
            afni_step.fact({"t2up100-afni": step["t2up100-afni"]}, "the call with -T2up 100"),
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": common.provenance(
            info, f"{TOOL}/{e.case}", [am.DATA], [am.original_provenance(o)]
        ),
    }


def _note(img) -> str:
    n = int(np.prod(img.shape[:3]))
    return (
        f"Δ is A − B. Computed in float64 over all {n:,} voxels of the outputs, not only those "
        "shown; the range is B's maximum minus its minimum. The records' thresholds: every output "
        "bit-identical to AFNI's (the replica), or float32 values within 1e-5 of AFNI's largest "
        "with bit-identity reported (the clean-room original); with their exit status and "
        "messages. The rows *the original against itself* are AFNI's own variability on this "
        "input: its thread counts and builds."
    )


def export(ctx: core.Context) -> dict:
    from larmorx_validation.parity.afni_unifize import _scenario_list

    reg = core.register(ctx.repo)
    recs = {
        impl: records.load(ctx.repo, f"{TOOL}{suffix}")
        for impl, _label, suffix in am.IMPLEMENTATIONS
    }
    examples = {
        "fmriprep/ds005040-task_clip_run_1": "fmriprep-unifize",
        "real/T1w-ds000005": "t1w-unifize",
    }
    cs = am.cases(recs, _scenario_list(), PROGRAM, examples)
    e, t = entry(ctx), t1w_entry(ctx)
    o, ot = original_entry(ctx), t1w_original_entry(ctx)
    step = afni_step.ensure(e, ["afni-1", "t2up100-afni"], ctx.compute)
    tstep = afni_step.ensure(t, ["afni-1"], ctx.compute)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "3dUnifize",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": [x for rec in recs.values() for x in rec.source()],
            "benchmarks": am.timings_source(ctx.repo),
            "cache": [fcache.provenance(x) for x in (e, t, o, ot)],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": am.implementations(
            cs, TOOL, {impl: am.status_of(rec) for impl, rec in recs.items()}
        ),
        "cases": cs,
        "cases_total": len(cs),
        "speed": am.informal_speed(ctx.repo, PROGRAM),
        "examples": [
            _example_ref(ctx, e, o, {"afni-1": step["afni-1"]}),
            _example_t1w(ctx, t, ot, tstep),
            _example_t2up(ctx, e, o, reg, step),
        ],
    }


if __name__ == "__main__":  # compute the cache entries only
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    print("done", entry(c).dir, t1w_entry(c).dir, original_entry(c).dir, t1w_original_entry(c).dir)
