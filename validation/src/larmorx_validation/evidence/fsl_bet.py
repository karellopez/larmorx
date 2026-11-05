# SPDX-License-Identifier: Apache-2.0
"""BET's evidence: fMRIPrep's call (``bet <boldref> <out> -f 0.20 -m``) on a real BOLD reference
by FSL 6.0.7.17, by fMRIPrep's own FSL (its image) and by the larmorx-nc replica in its default
and ``reproduce`` modes; ``bet -R -m`` on a real T1w; ``betsurf`` (T1 and T2) on the synthetic
head of the parity suite, whose skull and scalp surfaces can be shown whole without a face;
``bet -S``, ``-B``, ``-A`` and ``-A2`` on six real heads of the data panel (FSL 6.0.7.23 and the
replica, :mod:`fsl_bet_options`); the cases of the records.

The replica (``larmorx-nc``, under the FSL Licence's terms) runs as a separate program, as
everywhere in the validation package (Apache-2.0). Every run of FSL goes through the parity
suite's environment: FSL never on the ``PATH``, ``FSLDIR`` set, an empty ``HOME``. The masks are
drawn over the image they were made from; the T1w is shown only inside its brain mask, dilated by
two voxels: no face can show.

FSL 6.0.7.23 (the replica's reference since the version update of 2026-10-29) runs beside FSL
6.0.7.17 in entries of their own (``fsl_step``): each example shows its output, the replica's
against it and FSL's own step from 6.0.7.17. The case explorer shows the full-tier record (FSL
6.0.7.17) and the standard record (FSL 6.0.7.23) side by side.
"""

from __future__ import annotations

import copy
import hashlib
import json
import os
import shlex
import sys
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import (
    common,
    core,
    fsl_bet_options,
    fsl_bet_options_original,
    fsl_step,
)
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "fsl-bet"
RECORD = "fsl-bet-replica-full"  # the full tier: every case of the standard one and more
#: The standard record against FSL 6.0.7.23 (the full tier waits for the final validation
#: phase).
RECORD_NEW = "fsl-bet-replica"
CASE = "fmriprep-ds000005-run-01"
RUN = "ds000005-sub-01-noreconall"
NODE = (
    "work/fmriprep_25_2_wf/sub_01_wf/bold_task_mixedgamblestask_run_01_wf/bold_fit_wf/"
    "enhance_and_skullstrip_bold_wf/skullstrip_first_pass"
)
INPUT_NAME = "sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref_corrected.nii.gz"
INPUT = f"{NODE}/../n4_correct/{INPUT_NAME}"
FMRIPREP_BRAIN = (
    f"{NODE}/sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref_corrected_brain.nii.gz"
)
FMRIPREP_MASK = (
    f"{NODE}/sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref_corrected_brain_mask.nii.gz"
)
T1W = "openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz"
DATA = common.DS000005
SYNTHETIC = {
    "dataset": "the parity suite's synthetic head (larmorx_validation.parity.fsl_bet: shells of "
    "brain, CSF, skull and scalp, 2 mm voxels, seeded noise)",
    "licence": "generated",
}
#: The runs of every call: label suffix, program (fsl, image: fMRIPrep's FSL, replica), D13 mode.
RUNNERS = [
    ("fsl", "fsl", None),
    ("image", "image", None),
    ("replica", "replica", "fix"),
    ("replica-reproduce", "replica", "reproduce"),
]
#: FSL 6.0.7.23's runs and the replica's compared with them (entries of their own: ``fsl_step``).
RUNNERS_NEW = [
    ("fsl23", "fsl", None),
    ("replica23", "replica", "fix"),
    ("replica23-reproduce", "replica", "reproduce"),
]
INNER_FSL = "/app/.pixi/envs/fmriprep"


# --------------------------------------------------------------------- running the programs


def environment(home: Path, version: str | None = None) -> tuple[dict[str, str], tuple[str, ...]]:
    """The environment of every run, as the parity record's: FSL never on the PATH, ``FSLDIR``
    set (FSL ``version``, 6.0.7.17 unless given), an empty home, NIfTI-GZ outputs; no FSL or
    larmorx variable, preload or thread count of this process. Returns (added, removed)."""
    from larmorx_validation.parity import fsl_bet as fb

    unset = tuple(
        sorted(
            {k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_"))}
            | {"LD_PRELOAD", "OMP_NUM_THREADS"}
        )
    )
    add = {
        "HOME": str(home),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(fb.fsl_dir(version or fsl_step.OLD)),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        "LC_ALL": "C",
        # FSL 6.0.7.23's matrix arithmetic on OpenBLAS's Haswell kernels (fsl_versions).
        **fb.fsl_versions.openblas_env(),
    }
    return add, unset


def binaries(version: str | None = None) -> tuple[Path, str, Path]:
    from larmorx_validation.parity import fsl_bet as fb

    v = version or fsl_step.OLD
    fsl, nc, sif = fb.fsl_dir(v), fb.replica_binary(), fb.fmriprep_image()
    if fsl is None or nc is None or sif is None:
        raise FileNotFoundError(f"FSL {v}, the larmorx-nc replica or fMRIPrep's image")
    return fsl, nc, sif


def run_all(
    e: fcache.Entry,
    program: str,
    tag: str,
    args: list[str],
    inputs: list[Path],
    runners=RUNNERS,
    version: str | None = None,
) -> None:
    """``program`` with ``args`` (``{out}``: the run's output stem) by each runner, into the
    entry's directory: ``<tag><label>`` (and the files FSL derives from it); FSL ``version``
    (6.0.7.17 unless given)."""
    if any(kind != "original" for _l, kind, _m in runners):
        fsl, nc, sif = binaries(version)
    home = e.dir / ".home"
    home.mkdir(exist_ok=True)
    add, unset = environment(home, version)
    for label, kind, mode in runners:
        stem = str(e.dir / f"{tag}{label}")
        argv = [a.replace("{out}", stem) for a in args]
        if kind == "original":
            # larmorx's clean-room original (crates/larmorx-fsl), in process of `python -m
            # larmorx`, from this checkout's build
            env = {
                **add,
                "LARMORX_IMPLEMENTATION": "original",
                "LARMORX_UPSTREAM_DEFECTS": mode,
                "OMP_NUM_THREADS": "2",
                "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
            }
            e.run(
                f"{tag}{label}",
                [sys.executable, "-m", "larmorx", "fsl", program, *argv],
                env=env,
                unset=unset,
            )
        elif kind == "fsl":
            e.run(f"{tag}{label}", [str(fsl / "bin" / program), *argv], env=add, unset=unset)
        elif kind == "image":
            cmd = ["apptainer", "exec", "--cleanenv", "--no-home"]
            for d in sorted({str(e.dir), str(home), *(str(p.parent) for p in inputs)}):
                cmd += ["--bind", f"{d}:{d}"]
            inner = {**add, "FSLDIR": INNER_FSL}
            for k, v in inner.items():
                cmd += ["--env", f"{k}={v}"]
            e.run(
                f"{tag}{label}",
                [*cmd, str(sif), f"{INNER_FSL}/bin/{program}", *argv],
                unset=unset,
            )
        else:
            env = {**add, "LARMORX_UPSTREAM_DEFECTS": mode, "OMP_NUM_THREADS": "2"}
            e.run(f"{tag}{label}", [nc, "fsl", program, *argv], env=env, unset=unset)


def versions(version: str | None = None) -> dict[str, str]:
    from larmorx_validation.parity import fsl_bet as fb

    return {
        "fsl": fb.fsl_version(version or fsl_step.OLD),
        "fsl_in_fmriprep_image": "FSL in fMRIPrep 25.2.6's image (fsl-bet2 2111.8, "
        "fsl-newimage 2501.4, fsl-miscmaths 2412.4)",
        "larmorx-nc": fb.replica_version(),
        "fmriprep_image": (common.oracles() / "images" / "fmriprep-25.2.6.sif.sha256")
        .read_text(encoding="utf-8")
        .split()[0],
    }


def short_command(command: str) -> str:
    return " ".join(command.split())


# ------------------------------------------------------------------------------- the entries


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    run = common.fmriprep_run(RUN)
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    boldref = e.input("BOLD reference, N4-corrected (the node's input)", (run / INPUT).resolve())
    e.input("fMRIPrep's brain mask (the node's output)", run / FMRIPREP_MASK)
    t1 = e.input("T1w", Path(td.get(T1W)))
    words = shlex.split(command)
    if words[:1] != ["bet"] or words[3:] != ["-f", "0.20", "-m"]:
        raise AssertionError(f"{TOOL}: an unexpected fMRIPrep command: {command}")
    run_all(e, "bet", "", [str(boldref), "{out}", "-f", "0.20", "-m"], [boldref])
    # fMRIPrep's image has no dc, which bet -R needs: its bet -R stops ("dc: command not found")
    run_all(
        e, "bet", "t1-", [str(t1), "{out}", "-R", "-m"], [t1], runners=[RUNNERS[0], *RUNNERS[2:]]
    )
    e.finish(
        versions=versions(),
        command=command,
        settings="fMRIPrep 25.2.6's BET node (skullstrip_first_pass) of ds000005 sub-01 "
        "task-mixedgamblestask run-01, its arguments from command.txt; bet -R -m on the "
        "subject's raw T1w. FSL 6.0.7.17 (~/fsl, never on the PATH) and fMRIPrep's FSL in its "
        "image (not for bet -R: the image has no dc, which the bet script's -R needs), the "
        "larmorx-nc replica in both modes; FSL is single-threaded, the replica ran "
        "with OMP_NUM_THREADS=2",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's BET call on the BOLD reference of ds000005 sub-01 run 1 and bet -R -m on the "
        "subject's T1w: FSL, fMRIPrep's FSL and the replica, default and reproduce",
        _compute,
    )


def _compute_new(e: fcache.Entry) -> None:
    """FSL 6.0.7.23 and the replica on :func:`_compute`'s two calls."""
    import larmorx_testdata as td

    run = common.fmriprep_run(RUN)
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    boldref = e.input("BOLD reference, N4-corrected (the node's input)", (run / INPUT).resolve())
    t1 = e.input("T1w", Path(td.get(T1W)))
    new = fsl_step.NEW
    run_all(e, "bet", "", [str(boldref), "{out}", "-f", "0.20", "-m"], [boldref], RUNNERS_NEW, new)
    run_all(e, "bet", "t1-", [str(t1), "{out}", "-R", "-m"], [t1], RUNNERS_NEW, new)
    e.finish(
        versions=versions(new),
        command=command,
        settings=f"FSL {new} (~/fsl-{new}, never on the PATH) and the larmorx-nc replica in both "
        "modes on the calls of the FSL 6.0.7.17 entry beside it: fMRIPrep 25.2.6's BET node "
        "(skullstrip_first_pass) of ds000005 sub-01 task-mixedgamblestask run-01, and bet -R -m "
        "on the subject's raw T1w; the replica ran with OMP_NUM_THREADS=2",
    )


def entry_new(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        fsl_step.case(CASE),
        f"{fsl_step.NEW_LABEL} and the replica on the calls of {CASE}: fMRIPrep's BET call on "
        "the BOLD reference of ds000005 sub-01 run 1 and bet -R -m on the subject's T1w",
        _compute_new,
    )


#: A data-panel case (larmorx-testdata's panel): fMRIPrep's BET call on the BOLD reference of the
#: panel's youngest subject, ds000228 sub-pixar008 (3.5 years), shown as three slices.
PANEL_CASE = "panel-ds000228-pixar008"
PANEL_RUN = "ds000228-sub-pixar008-noreconall"
PANEL_NODE = (
    "work/fmriprep_25_2_wf/sub_pixar008_wf/bold_task_pixar_wf/bold_fit_wf/"
    "enhance_and_skullstrip_bold_wf/skullstrip_first_pass"
)
PANEL_STEM = "sub-pixar008_task-pixar_desc-hmc_boldref_corrected"
PANEL_INPUT = f"{PANEL_NODE}/../n4_correct/{PANEL_STEM}.nii.gz"
PANEL_BRAIN = f"{PANEL_NODE}/{PANEL_STEM}_brain.nii.gz"
PANEL_MASK = f"{PANEL_NODE}/{PANEL_STEM}_brain_mask.nii.gz"
PANEL_DATA = {
    "dataset": "OpenNeuro ds000228 (Richardson et al., 2018), version 1.1.1",
    "subject": "sub-pixar008 (3.5 years), task pixar",
    "licence": "CC0-1.0",
}


def _compute_panel(e: fcache.Entry) -> None:
    run = common.fmriprep_run(PANEL_RUN)
    command = (run / PANEL_NODE / "command.txt").read_text(encoding="utf-8").strip()
    boldref = e.input(
        "BOLD reference, N4-corrected (the node's input)", (run / PANEL_INPUT).resolve()
    )
    e.input("fMRIPrep's brain mask (the node's output)", run / PANEL_MASK)
    words = shlex.split(command)
    if words[:1] != ["bet"] or words[3:] != ["-f", "0.20", "-m"]:
        raise AssertionError(f"{TOOL}: an unexpected fMRIPrep command: {command}")
    run_all(e, "bet", "", [str(boldref), "{out}", "-f", "0.20", "-m"], [boldref])
    e.finish(
        versions=versions(),
        command=command,
        settings="fMRIPrep 25.2.6's BET node (skullstrip_first_pass) of the data panel's "
        "ds000228 sub-pixar008 (3.5 years), task pixar, its arguments from command.txt. FSL "
        "6.0.7.17 (~/fsl, never on the PATH), fMRIPrep's FSL in its image and the larmorx-nc "
        "replica in both modes; FSL is single-threaded, the replica ran with OMP_NUM_THREADS=2",
    )


def panel_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        PANEL_CASE,
        "fMRIPrep's BET call on the BOLD reference of the data panel's ds000228 sub-pixar008 "
        "(3.5 years): FSL, fMRIPrep's FSL and the replica, default and reproduce",
        _compute_panel,
    )


def _compute_panel_new(e: fcache.Entry) -> None:
    run = common.fmriprep_run(PANEL_RUN)
    command = (run / PANEL_NODE / "command.txt").read_text(encoding="utf-8").strip()
    boldref = e.input(
        "BOLD reference, N4-corrected (the node's input)", (run / PANEL_INPUT).resolve()
    )
    new = fsl_step.NEW
    run_all(e, "bet", "", [str(boldref), "{out}", "-f", "0.20", "-m"], [boldref], RUNNERS_NEW, new)
    e.finish(
        versions=versions(new),
        command=command,
        settings=f"FSL {new} (~/fsl-{new}, never on the PATH) and the larmorx-nc replica in both "
        "modes on fMRIPrep 25.2.6's BET node of the data panel's ds000228 sub-pixar008, as the "
        "FSL 6.0.7.17 entry beside it; the replica ran with OMP_NUM_THREADS=2",
    )


def panel_entry_new(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        fsl_step.case(PANEL_CASE),
        f"{fsl_step.NEW_LABEL} and the replica on fMRIPrep's BET call on the BOLD reference of "
        "the data panel's ds000228 sub-pixar008 (3.5 years)",
        _compute_panel_new,
    )


def _compute_surf(e: fcache.Entry) -> None:
    from larmorx_validation.parity import fsl_bet as fb

    t1 = e.input("synthetic head, T1-like", fb.input_path("head"))
    t2 = e.input("synthetic head, T2-like", fb.input_path("head-t2"))
    mesh = e.input("FSL bet2's surface of it (bet2 -e)", fb.input_path("head-mesh"))
    mat = e.input("its matrix to MNI152", fb.input_path("head-mat"))
    e.input("its known brain", fb.input_path("truth:head"))
    args = ["-o", "-m", "-s", str(t1), str(t2), str(mesh), str(mat), "{out}"]
    run_all(e, "betsurf", "", args, [t1, t2, mesh, mat])
    e.finish(
        versions=versions(),
        command="betsurf -o -m -s <t1> <t2> <bet_mesh.vtk> <t1_to_standard.mat> <out>",
        settings="betsurf with the T2 (bet -A2's call) on the parity suite's synthetic head: "
        "FSL 6.0.7.17, fMRIPrep's FSL and the replica in both modes",
    )


def surf_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        "betsurf-synthetic-head",
        "betsurf -o -m -s with the T2 on the synthetic head: FSL, fMRIPrep's FSL and the replica",
        _compute_surf,
    )


def _compute_surf_new(e: fcache.Entry) -> None:
    from larmorx_validation.parity import fsl_bet as fb

    t1 = e.input("synthetic head, T1-like", fb.input_path("head"))
    t2 = e.input("synthetic head, T2-like", fb.input_path("head-t2"))
    mesh = e.input("FSL bet2's surface of it (bet2 -e)", fb.input_path("head-mesh"))
    mat = e.input("its matrix to MNI152", fb.input_path("head-mat"))
    args = ["-o", "-m", "-s", str(t1), str(t2), str(mesh), str(mat), "{out}"]
    run_all(e, "betsurf", "", args, [t1, t2, mesh, mat], RUNNERS_NEW, fsl_step.NEW)
    e.finish(
        versions=versions(fsl_step.NEW),
        command="betsurf -o -m -s <t1> <t2> <bet_mesh.vtk> <t1_to_standard.mat> <out>",
        settings=f"betsurf with the T2 on the synthetic head (the inputs of the FSL 6.0.7.17 "
        f"entry beside it): FSL {fsl_step.NEW} and the replica in both modes",
    )


def surf_entry_new(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        fsl_step.case("betsurf-synthetic-head"),
        f"betsurf -o -m -s with the T2 on the synthetic head: {fsl_step.NEW_LABEL} and the replica",
        _compute_surf_new,
    )


#: larmorx's clean-room original (``crates/larmorx-fsl``, 2026-10-30 and 31) in its default and
#: ``reproduce`` modes, in entries of its own beside FSL 6.0.7.23's (its reference).
RUNNERS_ORIG = [("orig", "original", "fix"), ("orig-reproduce", "original", "reproduce")]
#: The original's record (standard tier, against FSL 6.0.7.23).
RECORD_ORIG = "fsl-bet"


def versions_orig() -> dict[str, str]:
    import larmorx
    from larmorx_validation.parity import fsl_bet as fb

    return {
        "fsl": fb.fsl_version(fsl_step.NEW),
        "larmorx": f"larmorx {larmorx.__version__} (crate larmorx-fsl: the clean-room original)",
    }


def _orig_entry(ctx: core.Context, case: str, what: str, compute) -> fcache.Entry:
    return fcache.entry(ctx.figures_context(), TOOL, f"{case}-original", what, compute)


def _compute_orig(e: fcache.Entry) -> None:
    """The clean-room original on :func:`_compute`'s two calls."""
    import larmorx_testdata as td

    run = common.fmriprep_run(RUN)
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    boldref = e.input("BOLD reference, N4-corrected (the node's input)", (run / INPUT).resolve())
    t1 = e.input("T1w", Path(td.get(T1W)))
    args = [str(boldref), "{out}", "-f", "0.20", "-m"]
    run_all(e, "bet", "", args, [boldref], RUNNERS_ORIG, fsl_step.NEW)
    run_all(e, "bet", "t1-", [str(t1), "{out}", "-R", "-m"], [t1], RUNNERS_ORIG, fsl_step.NEW)
    e.finish(
        versions=versions_orig(),
        command=command,
        settings="larmorx's clean-room original (python -m larmorx fsl bet, "
        "LARMORX_IMPLEMENTATION=original) in its default and reproduce modes on the calls of the "
        "FSL entries beside it: fMRIPrep 25.2.6's BET node of ds000005 sub-01 run 1, and bet -R "
        "-m on the subject's raw T1w; OMP_NUM_THREADS=2 (the result does not depend on it)",
    )


def entry_orig(ctx: core.Context) -> fcache.Entry:
    return _orig_entry(
        ctx,
        CASE,
        "the clean-room original on fMRIPrep's BET call (ds000005 sub-01 run 1) and bet -R -m on "
        "the subject's T1w, default and reproduce",
        _compute_orig,
    )


def _compute_panel_orig(e: fcache.Entry) -> None:
    run = common.fmriprep_run(PANEL_RUN)
    command = (run / PANEL_NODE / "command.txt").read_text(encoding="utf-8").strip()
    boldref = e.input(
        "BOLD reference, N4-corrected (the node's input)", (run / PANEL_INPUT).resolve()
    )
    args = [str(boldref), "{out}", "-f", "0.20", "-m"]
    run_all(e, "bet", "", args, [boldref], RUNNERS_ORIG, fsl_step.NEW)
    e.finish(
        versions=versions_orig(),
        command=command,
        settings="larmorx's clean-room original in both modes on fMRIPrep 25.2.6's BET node of "
        "the data panel's ds000228 sub-pixar008; OMP_NUM_THREADS=2",
    )


def panel_entry_orig(ctx: core.Context) -> fcache.Entry:
    return _orig_entry(
        ctx,
        PANEL_CASE,
        "the clean-room original on fMRIPrep's BET call on the BOLD reference of the data "
        "panel's ds000228 sub-pixar008 (3.5 years)",
        _compute_panel_orig,
    )


def _compute_surf_orig(e: fcache.Entry) -> None:
    from larmorx_validation.parity import fsl_bet as fb

    t1 = e.input("synthetic head, T1-like", fb.input_path("head"))
    t2 = e.input("synthetic head, T2-like", fb.input_path("head-t2"))
    mesh = e.input("FSL bet2's surface of it (bet2 -e)", fb.input_path("head-mesh"))
    mat = e.input("its matrix to MNI152", fb.input_path("head-mat"))
    args = ["-o", "-m", "-s", str(t1), str(t2), str(mesh), str(mat), "{out}"]
    run_all(e, "betsurf", "", args, [t1, t2, mesh, mat], RUNNERS_ORIG, fsl_step.NEW)
    e.finish(
        versions=versions_orig(),
        command="betsurf -o -m -s <t1> <t2> <bet_mesh.vtk> <t1_to_standard.mat> <out>",
        settings="betsurf with the T2 on the synthetic head (the inputs of the FSL entries beside "
        "it): larmorx's clean-room original in both modes; OMP_NUM_THREADS=2",
    )


def surf_entry_orig(ctx: core.Context) -> fcache.Entry:
    return _orig_entry(
        ctx,
        "betsurf-synthetic-head",
        "betsurf -o -m -s with the T2 on the synthetic head: the clean-room original",
        _compute_surf_orig,
    )


#: The original's panels (from its entry).
PANELS_ORIG = [
    ("orig", "orig", "larmorx", "larmorx's clean-room original",
     f"default mode (Apache-2.0, crate larmorx-fsl), against {fsl_step.NEW_LABEL}"),
    ("orig-reproduce", "orig-reproduce", "larmorx", "larmorx's clean-room original",
     "reproduce mode"),
]  # fmt: skip


def _orig_diffs() -> list[dict]:
    d = common.d
    return [
        d("orig-fsl23", "orig", "fsl23", "main", f"original − {fsl_step.NEW_LABEL}",
          common.ROLE_LX),
        d("orig-repro-fsl23", "orig-reproduce", "fsl23", "main",
          f"original (reproduce) − {fsl_step.NEW_LABEL}", common.ROLE_LX),
        d("orig-replica23", "orig", "replica23", "check", "original − replica (both following "
          f"{fsl_step.NEW_LABEL})", common.ROLE_SELF),
    ]  # fmt: skip


def _orig_verdicts(diffs: list[dict], same: dict[str, bool], arrays: dict, outputs: list) -> None:
    """The original's comparisons: bit-identical, or an open difference with the masks' Dice
    (betsurf is held to its masks: Dice >= 0.99, boundary p95 <= 1 voxel, its record)."""
    for x in diffs:
        if not x["id"].startswith("orig-"):
            continue
        x["links"] = []
        if same[x["id"]]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "orig-replica23": "The clean-room original gives the replica's values, every one.",
            }.get(x["id"], f"The same values as {fsl_step.NEW_LABEL}'s, every one.")
            continue
        parts = []
        for o in outputs:
            a, b = arrays.get(x["a"], {}).get(o["id"]), arrays.get(x["b"], {}).get(o["id"])
            if a is None or b is None:
                continue
            ma, mb = np.asarray(a) != 0, np.asarray(b) != 0
            if np.array_equal(ma, mb):
                continue
            dice = (
                2 * np.count_nonzero(ma & mb) / max(1, np.count_nonzero(ma) + np.count_nonzero(mb))
            )
            n = int(np.count_nonzero(ma != mb))
            parts.append(
                f"{o['label'].lower()}: {n:,} voxel{'s' if n != 1 else ''} of "
                f"{int(np.count_nonzero(mb)):,} (Dice {dice:.5f})"
            )
        x["verdict"] = "open"
        who = "the replica's" if x["b"].startswith("replica") else f"{fsl_step.NEW_LABEL}'s"
        x["reason"] = (
            f"The clean-room original's masks differ from {who} in a few voxels at their edges: "
            + ("; ".join(parts) or "values")
            + ". betsurf's profiles agree with FSL's one by one (330 of 330 probed); its surfaces "
            "settle a little elsewhere where few profiles vote and at the scalp "
            "(docs/findings/fsl-bet-original.md, betsurf). The record holds betsurf to its masks "
            "(Dice >= 0.99, boundary p95 <= 1 voxel), which these meet; the difference is not "
            "closed: the original could still match FSL."
        )


# ----------------------------------------------------------------------------------- export

OUTPUTS = [
    {"id": "mask", "label": "Brain mask (-m)", "unit": ""},
    {"id": "brain", "label": "Brain", "unit": ""},
]
SURF_OUTPUTS = [
    {"id": "inskull", "label": "Inner skull mask", "unit": ""},
    {"id": "outskull", "label": "Outer skull mask", "unit": ""},
    {"id": "outskin", "label": "Scalp mask", "unit": ""},
    {"id": "skull", "label": "Skull mask (-s)", "unit": ""},
]
PANELS = [
    ("fsl", "fsl", "original", "FSL 6.0.7.17, bet", "the conda installation, as run"),
    ("image", "image", "original", "FSL in fMRIPrep's image", "fMRIPrep 25.2.6's build"),
    ("replica", "replica", "larmorx", "larmorx-nc replica", "default mode"),
    ("replica-reproduce", "replica-reproduce", "larmorx", "larmorx-nc replica", "reproduce mode"),
]  # fmt: skip
#: FSL 6.0.7.23's panels (from its entry: ``fsl_step``).
PANELS_NEW = [
    ("fsl23", "fsl23", "original", f"{fsl_step.NEW_LABEL}, bet",
     f"the conda installation, as run; {fsl_step.REFERENCE}"),
    ("replica23", "replica23", "larmorx", "larmorx-nc replica",
     f"default mode, following {fsl_step.NEW_LABEL}"),
]  # fmt: skip
#: Where each page's records ran.
RECORDS_TEXT = (
    "The case explorer below shows the full-tier record (against FSL 6.0.7.17) and the standard "
    "record (against FSL 6.0.7.23) side by side."
)


def runs(e: fcache.Entry, tag: str, panels=PANELS) -> dict[str, dict[str, np.ndarray]]:
    out = {}
    for pid, stem, *_ in panels:
        f = e.dir / f"{tag}{stem}.nii.gz"
        if not f.is_file():
            continue
        out[pid] = {
            "mask": common.load(str(e.dir / f"{tag}{stem}_mask.nii.gz")),
            "brain": common.load(str(f)),
        }
    return out


def surf_runs(e: fcache.Entry, panels=PANELS) -> dict[str, dict[str, np.ndarray]]:
    out = {}
    for pid, stem, *_ in panels:
        f = e.dir / f"{stem}_inskull_mask.nii.gz"
        if not f.is_file():
            continue
        out[pid] = {
            "inskull": common.load(str(f)),
            "outskull": common.load(str(e.dir / f"{stem}_outskull_mask.nii.gz")),
            "outskin": common.load(str(e.dir / f"{stem}_outskin_mask.nii.gz")),
            "skull": common.load(str(e.dir / f"{stem}_skull_mask.nii.gz")),
        }
    return out


def mask_view(ex, pid: str = "fsl", out: str = "mask") -> tuple[list[int], int]:
    """The default view of a mask: the axial slice with the most brain; the crosshair on the
    mask's edge there, nearest the brain's centre."""
    from scipy import ndimage

    k = int(np.argmax(ex.m.sum(axis=(0, 1))))
    m = ex.values(ex.key(pid, out))[:, :, k] != 0
    edge = m & ~ndimage.binary_erosion(m)
    pts = np.argwhere(edge)
    if not len(pts):
        return common.centre_view(ex)
    c = np.argwhere(ex.m[:, :, k]).mean(0)
    i, j = pts[int(np.argmin(((pts - c) ** 2).sum(1)))]
    return [int(i), int(j), k], k


def window_of(ex, pid: str) -> list[float]:
    from larmorx_validation.evidence import example as exm

    v = ex.values(ex.key(pid, "all"))
    m = ex.m if v.ndim == 3 else np.broadcast_to(ex.m[..., None], v.shape)
    w = exm.windows(v[m])[0]
    return [w["lo"], w["hi"]]


def _diffs() -> list[dict]:
    d = common.d
    return [
        d("replica-fsl", "replica", "fsl", "main", "replica − FSL 6.0.7.17", common.ROLE_LX),
        d("repro-fsl", "replica-reproduce", "fsl", "main", "replica (reproduce) − FSL 6.0.7.17",
          common.ROLE_LX),
        d("image-fsl", "image", "fsl", "variability", "fMRIPrep's FSL − FSL 6.0.7.17",
          common.ROLE_ORIG),
        d("replica-image", "replica", "image", "fmriprep", "replica − fMRIPrep's FSL",
          common.ROLE_FMRIPREP),
        d("replica-repro", "replica", "replica-reproduce", "check",
          "replica, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip


def head(f: Path) -> bytes:
    """A NIfTI file's header (the bytes before the data, decompressed)."""
    import gzip

    raw = gzip.decompress(f.read_bytes())
    return raw[: int(np.frombuffer(raw[108:112], "<f4")[0])]


def headers(
    e: fcache.Entry, tag: str, files: dict[str, str], panels=PANELS, ref: str = "fsl"
) -> dict[str, dict[str, bool]]:
    """Whether each run's NIfTI headers are FSL's (the run ``ref`` of the same entry): run
    label -> output id -> same. ``files``: output id -> the file's suffix."""
    out: dict[str, dict[str, bool]] = {}
    for pid, stem, *_ in panels:
        if not (e.dir / f"{tag}{stem}{next(iter(files.values()))}").is_file():
            continue
        out[pid] = {
            oid: head(e.dir / f"{tag}{stem}{suf}") == head(e.dir / f"{tag}{ref}{suf}")
            for oid, suf in files.items()
        }
    return out


def _new_diffs() -> list[dict]:
    """The replica against FSL 6.0.7.23, and FSL's own step from 6.0.7.17."""
    return [fsl_step.replica_diff("replica23", "fsl23"), fsl_step.step_diff("fsl23", "fsl")]


def _step_verdicts(
    diffs: list[dict],
    same: dict[str, bool],
    what: str,
    e: fcache.Entry,
    n: fcache.Entry,
    tag: str,
    files: dict[str, str],
) -> None:
    """The verdicts of :func:`_new_diffs`: the replica against FSL 6.0.7.23 (its headers
    too), and the version step (FSL's headers of the two versions compared)."""
    h = headers(n, tag, files, PANELS_NEW, ref="fsl23")
    step_heads = all(
        head(n.dir / f"{tag}fsl23{suf}") == head(e.dir / f"{tag}fsl{suf}") for suf in files.values()
    )
    for x in diffs:
        if x["id"] == "replica23-fsl23":
            x["verdict"] = "bit-identical" if same[x["id"]] else "open"
            x["reason"] = fsl_step.replica_reason(same[x["id"]], header_note(h, "replica23"))
            x["links"] = []
        elif x["id"] == "fsl23-fsl":
            x["verdict"] = None
            x["reason"] = fsl_step.step_reason(same[x["id"]], what, step_heads)
            x["links"] = []


def header_note(h: dict[str, dict[str, bool]], pid: str) -> str:
    if pid not in h:
        return ""
    if all(h[pid].values()):
        return " The NIfTI headers too, byte for byte."
    other = [o for o, v in h[pid].items() if not v]
    return f" The NIfTI headers of {', '.join(other)} differ (FSL's build string, `descrip`)."


def _verdict(x: dict, same: dict[str, bool], what: str = "this input", h=None) -> None:
    i = x["id"]
    x.setdefault("links", [])
    h = h or {}
    if x["kind"] == "variability":
        x["verdict"] = None
        x["reason"] = (
            "Two builds of FSL (fMRIPrep's image, with newer image libraries, and the "
            f"installation in ~/fsl) give the same values on {what}: FSL's own variability is "
            "zero here (FSL is deterministic, single-threaded)." + header_note(h, x["a"])
            if same[i]
            else f"FSL's two builds differ on {what}."
        )
    elif same[i]:
        x["verdict"] = "bit-identical"
        x["reason"] = {
            "replica-repro": "No correction of the register applies to this input: both modes "
            "give the same values.",
            "replica-image": "The replica gives the values of fMRIPrep's own build of FSL too.",
        }.get(i, "The same values, every one." + header_note(h, x["a"]))
    else:
        x["verdict"] = "open"
        x["reason"] = "Not accounted for by the other runs of this example."


def _note(img) -> str:
    n = int(np.prod(img.shape[:3]))
    return (
        f"Δ is A − B: for the masks +1 where only A holds a voxel, −1 where only B does. "
        f"Computed in float64 over all {n:,} voxels of the outputs, not only those shown. The "
        "replica's records' threshold: every file bit-identical to FSL's (header and data), with "
        "its exit status and messages. The clean-room original's record (specs/bet.md §11): exit "
        "status and messages FSL's, each file bit-identical or, for the masks, within a Dice "
        "coefficient and a boundary distance (betsurf's: Dice >= 0.99, the boundary distance's "
        "95th percentile <= 1 voxel)."
    )


def _example_fmriprep(ctx: core.Context, e: fcache.Entry, n: fcache.Entry, o: fcache.Entry) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    run = common.fmriprep_run(RUN)
    img = nib.load(str((run / INPUT).resolve()))
    arrays = runs(e, "")
    node_mask = common.load(str(run / FMRIPREP_MASK))
    node_brain = common.load(str(run / FMRIPREP_BRAIN))
    arrays["node"] = {"mask": node_mask, "brain": node_brain}
    arrays.update(runs(n, "", PANELS_NEW))
    arrays.update(runs(o, "", PANELS_ORIG))
    show = ndimage.binary_dilation(
        np.logical_or.reduce([a["mask"] != 0 for a in arrays.values()]), iterations=2
    )
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("boldref", "input", "ds000005 sub-01 BOLD reference, N4-corrected",
                     "BET's input in fMRIPrep's BOLD reference workflow (float32)",
                     {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    over = {"mask": {"on": "boldref", "window": window_of(ex, "boldref")}}
    for pid, _stem, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    ex.add(exm.Panel("node", "fmriprep", "fMRIPrep 25.2.6's own output",
                     "the node's brain and mask, as fMRIPrep wrote them", arrays["node"],
                     overlay=over))  # fmt: skip
    for pid, _stem, role, label, sub in [*PANELS_NEW, *PANELS_ORIG]:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    d = common.d
    diffs = [
        *_diffs()[:4],
        d("replica-node", "replica", "node", "fmriprep", "replica − fMRIPrep's own output",
          common.ROLE_FMRIPREP),
        _diffs()[4],
        *_new_diffs(),
        *_orig_diffs(),
    ]  # fmt: skip
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    h = headers(e, "", {"mask": "_mask.nii.gz", "brain": ".nii.gz"})
    for x in diffs:
        _verdict(x, same, h=h)
        if x["id"] == "replica-node" and same["replica-node"]:
            x["reason"] = (
                "The replica gives the values fMRIPrep's node wrote, every voxel of the brain and "
                "of the mask (the headers differ only in FSL's build string, `descrip`)."
            )
    what = "this BOLD reference"
    _step_verdicts(diffs, same, what, e, n, "", {"mask": "_mask.nii.gz", "brain": ".nii.gz"})
    _orig_verdicts(diffs, same, arrays, OUTPUTS)
    at, _k = mask_view(ex)
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["boldref", "fsl", "replica", "node", "orig"],
        "diffs": ["replica-fsl", "image-fsl", "replica-node", "orig-fsl23"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(OUTPUTS), default=default,
                        window_panel="fsl", caption="the crosshair on the mask's edge.")  # fmt: skip
    vers = info.get("versions", {})
    a = arrays["fsl"]["mask"]
    m = core.compare(arrays["replica"]["mask"], a)
    return {
        "id": "fmriprep-bet",
        "title": "fMRIPrep's skull strip of a real BOLD reference",
        "summary": (
            "The BET node fMRIPrep 25.2.6 ran on the N4-corrected BOLD reference of OpenNeuro "
            "ds000005 sub-01 (task mixed gambles, run 1): `bet <boldref> <out> -f 0.20 -m`, given "
            "to FSL 6.0.7.17, to fMRIPrep's own build of FSL in its image and to the larmorx-nc "
            "replica in its default and reproduce modes, beside the outputs fMRIPrep's node "
            "wrote. The masks are drawn over the input; choose *Brain* for the extracted brain."
            " FSL 6.0.7.23's output, and the replica's run compared with it, are shown beside them (the replica's reference since the version update of 2026-10-29). LARMORX's clean-room original (Apache-2.0, written from the specification without FSL's source) runs the same call in its default and reproduce modes, compared with FSL 6.0.7.23, its reference."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{DATA['dataset']}, {DATA['subject']} ({DATA['licence']}): the BOLD "
                "reference fMRIPrep computed and N4-corrected; shown around the masks, dilated "
                "by 2 voxels",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `bold_fit_wf/enhance_and_skullstrip_bold_wf/"
                "skullstrip_first_pass` (niworkflows' first skull strip)",
            },
            {"label": "Command", "text": "`" + short_command(info["command"]) + "`"},
            {
                "label": "Programs",
                "text": f"{vers.get('fsl', '?')}; {vers.get('fsl_in_fmriprep_image', '?')}; "
                f"{vers.get('larmorx-nc', '?')} (the replica), from larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`; "
                + o.info().get("versions", {}).get("larmorx", "the clean-room original"),
            },
            {
                "label": "Mask",
                "text": f"{int(np.count_nonzero(a)):,} of {a.size:,} voxels; the replica's "
                + (
                    "the same, voxel for voxel" if m["identical"] else f"differs in {m['differ']:,}"
                ),
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
            fsl_step.fact(same["fsl23-fsl"], what, RECORDS_TEXT),
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": [
            *common.provenance(info, f"{TOOL}/{e.case}", [DATA]),
            *common.provenance(n.info(), f"{TOOL}/{n.case}", [DATA])[1:],
            *common.provenance(o.info(), f"{TOOL}/{o.case}", [DATA])[1:],
        ],
    }


def _example_panel(ctx: core.Context, pe: fcache.Entry, pn: fcache.Entry, po: fcache.Entry) -> dict:
    """The data-panel case as three full-resolution slices (``view="slices"``), around the masks
    (dilated by 2 voxels: the BOLD reference is shown brain-masked)."""
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = pe.info()
    run = common.fmriprep_run(PANEL_RUN)
    img = nib.load(str((run / PANEL_INPUT).resolve()))
    arrays = runs(pe, "")
    arrays["node"] = {
        "mask": common.load(str(run / PANEL_MASK)),
        "brain": common.load(str(run / PANEL_BRAIN)),
    }
    arrays.update(runs(pn, "", PANELS_NEW))
    arrays.update(runs(po, "", PANELS_ORIG))
    show = ndimage.binary_dilation(
        np.logical_or.reduce([a["mask"] != 0 for a in arrays.values()]), iterations=2
    )
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show, view="slices")
    ex.add(exm.Panel("boldref", "input", "ds000228 sub-pixar008 BOLD reference, N4-corrected",
                     "BET's input in fMRIPrep's BOLD reference workflow (float32)",
                     {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    over = {"mask": {"on": "boldref", "window": window_of(ex, "boldref")}}
    for pid, _stem, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    ex.add(exm.Panel("node", "fmriprep", "fMRIPrep 25.2.6's own output",
                     "the node's brain and mask, as fMRIPrep wrote them", arrays["node"],
                     overlay=over))  # fmt: skip
    for pid, _stem, role, label, sub in [*PANELS_NEW, *PANELS_ORIG]:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    d = common.d
    diffs = [
        *_diffs()[:4],
        d("replica-node", "replica", "node", "fmriprep", "replica − fMRIPrep's own output",
          common.ROLE_FMRIPREP),
        _diffs()[4],
        *_new_diffs(),
        *_orig_diffs(),
    ]  # fmt: skip
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    h = headers(pe, "", {"mask": "_mask.nii.gz", "brain": ".nii.gz"})
    what = "this child's BOLD reference"
    for x in diffs:
        _verdict(x, same, what=what, h=h)
        if x["id"] == "replica-node" and same["replica-node"]:
            x["reason"] = (
                "The replica gives the values fMRIPrep's node wrote, every voxel of the brain and "
                "of the mask (the headers differ only in FSL's build string, `descrip`)."
            )
    _step_verdicts(diffs, same, what, pe, pn, "", {"mask": "_mask.nii.gz", "brain": ".nii.gz"})
    _orig_verdicts(diffs, same, arrays, OUTPUTS)
    at = common.slice_voxel(ex, "replica", "fsl", "mask")
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["boldref", "fsl", "replica", "node", "orig"],
        "diffs": ["replica-fsl", "image-fsl", "replica-node", "orig-fsl23"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    differs = not same["replica-fsl"]
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(OUTPUTS), default=default, window_panel="fsl",
        caption="the crosshair " + ("where the replica's mask differs from FSL's" if differs
                                    else "at the centre of the shown region")
        + "; the three slices through it are stored at full resolution.",
    )  # fmt: skip
    vers = info.get("versions", {})
    a = arrays["fsl"]["mask"]
    m = core.compare(arrays["replica"]["mask"], a)
    return {
        "id": "panel-ds000228-child",
        "title": "Data panel: a three-year-old's BOLD reference",
        "summary": (
            "The data panel's youngest subject: the BET node fMRIPrep 25.2.6 ran on the "
            "N4-corrected BOLD reference of OpenNeuro ds000228 sub-pixar008, aged 3.5 (a small "
            "head, an oblique field of view; task pixar), `bet <boldref> <out> -f 0.20 -m`, "
            "given to FSL 6.0.7.17, fMRIPrep's build of FSL and the replica in both modes, "
            "beside the outputs fMRIPrep's node wrote. As a panel case beside the examples "
            "above, it is stored as three full-resolution slices."
            " FSL 6.0.7.23's output, and the replica's run compared with it, are shown beside them (the replica's reference since the version update of 2026-10-29). LARMORX's clean-room original (Apache-2.0, written from the specification without FSL's source) runs the same call in its default and reproduce modes, compared with FSL 6.0.7.23, its reference."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{PANEL_DATA['dataset']}, {PANEL_DATA['subject']} "
                f"({PANEL_DATA['licence']}): the BOLD reference fMRIPrep computed and "
                "N4-corrected; shown around the masks, dilated by 2 voxels",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `bold_fit_wf/enhance_and_skullstrip_bold_wf/"
                "skullstrip_first_pass` (niworkflows' first skull strip)",
            },
            {"label": "Command", "text": "`" + short_command(info["command"]) + "`"},
            {
                "label": "Programs",
                "text": f"{vers.get('fsl', '?')}; {vers.get('fsl_in_fmriprep_image', '?')}; "
                f"{vers.get('larmorx-nc', '?')} (the replica), from larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`; "
                + po.info().get("versions", {}).get("larmorx", "the clean-room original"),
            },
            {
                "label": "Mask",
                "text": f"{int(np.count_nonzero(a)):,} of {a.size:,} voxels; the replica's "
                + (
                    "the same, voxel for voxel" if m["identical"] else f"differs in {m['differ']:,}"
                ),
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
            fsl_step.fact(same["fsl23-fsl"], what, RECORDS_TEXT),
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": [
            *common.provenance(info, f"{TOOL}/{pe.case}", [PANEL_DATA]),
            *common.provenance(pn.info(), f"{TOOL}/{pn.case}", [PANEL_DATA])[1:],
            *common.provenance(po.info(), f"{TOOL}/{po.case}", [PANEL_DATA])[1:],
        ],
    }


def _example_t1(ctx: core.Context, e: fcache.Entry, n: fcache.Entry, o: fcache.Entry) -> dict:
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    img = nib.load(td.get(T1W))
    arrays = runs(e, "t1-")
    arrays.update(runs(n, "t1-", PANELS_NEW))
    arrays.update(runs(o, "t1-", PANELS_ORIG))
    # only the brain is shown: the masks' union, dilated by 2 voxels (no face can show)
    show = ndimage.binary_dilation(
        np.logical_or.reduce([a["mask"] != 0 for a in arrays.values()]), iterations=2
    )
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("t1", "input", "ds000005 sub-01 T1w", "the raw image (int16), inside the "
                     "brain masks only", {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    over = {"mask": {"on": "t1", "window": window_of(ex, "t1")}}
    for pid, _stem, role, label, sub in [*PANELS, *PANELS_NEW, *PANELS_ORIG]:
        if pid in arrays:
            ex.add(exm.Panel(pid, role, label.replace("bet", "bet -R"), sub, arrays[pid],
                             overlay=over))  # fmt: skip
    diffs = [
        *(x for x in _diffs() if "image" not in (x["a"], x["b"])),
        *_new_diffs(),
        *_orig_diffs(),
    ]
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    h = headers(e, "t1-", {"mask": "_mask.nii.gz", "brain": ".nii.gz"})
    for x in diffs:
        _verdict(x, same, "this T1w", h)
    what = "this T1w"
    _step_verdicts(diffs, same, what, e, n, "t1-", {"mask": "_mask.nii.gz", "brain": ".nii.gz"})
    _orig_verdicts(diffs, same, arrays, OUTPUTS)
    at, _k = mask_view(ex)
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1", "fsl", "replica", "replica-reproduce", "orig"],
        "diffs": ["replica-fsl", "repro-fsl", "orig-fsl23"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(OUTPUTS), default=default,
                        window_panel="fsl", caption="the crosshair on the mask's edge.")  # fmt: skip
    vers = info.get("versions", {})
    a = arrays["fsl"]["mask"]
    return {
        "id": "t1w-robust",
        "title": "A real T1w: bet -R",
        "summary": (
            "BET's anatomical use with its robust centre estimation (`-R`: bet2 run again from "
            "the brain's centre until it moves less than a voxel), on the raw T1w of the same "
            "subject. The `bet` script itself is replicated: its options, its loop, the "
            "centre-of-gravity computation it asks of `fslstats -C`, its messages."
            " FSL 6.0.7.23's output, and the replica's run compared with it, are shown beside them (the replica's reference since the version update of 2026-10-29). LARMORX's clean-room original (Apache-2.0, written from the specification without FSL's source) runs the same call in its default and reproduce modes, compared with FSL 6.0.7.23, its reference."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{DATA['dataset']}, {DATA['subject']} ({DATA['licence']}): the raw T1w; "
                "shown only inside the brain masks, dilated by 2 voxels",
            },
            {"label": "Command", "text": "`bet sub-01_T1w.nii.gz out -R -m`"},
            {
                "label": "Programs",
                "text": f"{vers.get('fsl', '?')}; {vers.get('larmorx-nc', '?')}; "
                + o.info().get("versions", {}).get("larmorx", "the clean-room original")
                + ". fMRIPrep's image "
                "is not shown: it has no `dc`, which the bet script's `-R` needs, so its `bet -R` "
                "stops (`dc: command not found`)",
            },
            {"label": "Mask", "text": f"{int(np.count_nonzero(a)):,} of {a.size:,} voxels"},
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
            fsl_step.fact(same["fsl23-fsl"], what, RECORDS_TEXT),
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": [
            *common.provenance(info, f"{TOOL}/{e.case}", [DATA]),
            *common.provenance(n.info(), f"{TOOL}/{n.case}", [DATA])[1:],
            *common.provenance(o.info(), f"{TOOL}/{o.case}", [DATA])[1:],
        ],
    }


def _example_surf(ctx: core.Context, e: fcache.Entry, n: fcache.Entry, o: fcache.Entry) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.parity import fsl_bet as fb

    info = e.info()
    img = nib.load(str(fb.input_path("head")))
    t2 = nib.load(str(fb.input_path("head-t2")))
    arrays = surf_runs(e)
    arrays.update(surf_runs(n, PANELS_NEW))
    arrays.update(surf_runs(o, PANELS_ORIG))
    show = np.ones(img.shape[:3], dtype=bool)
    grid = core.Grid.around(img, show, margin=0)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("t1", "input", "synthetic head, T1-like", "brain, CSF, skull, scalp, 2 mm",
                     {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    ex.add(exm.Panel("t2", "input", "synthetic head, T2-like", "the same shells, T2 contrast",
                     {"all": np.asanyarray(t2.dataobj)}))  # fmt: skip
    over = {o["id"]: {"on": "t1", "window": window_of(ex, "t1")} for o in SURF_OUTPUTS}
    for pid, _stem, role, label, sub in [*PANELS, *PANELS_NEW, *PANELS_ORIG]:
        ex.add(exm.Panel(pid, role, label.replace("bet", "betsurf"), sub, arrays[pid],
                         overlay=over))  # fmt: skip
    diffs = [*_diffs(), *_new_diffs(), *_orig_diffs()]
    common.compare_all(ex, diffs, SURF_OUTPUTS)
    same = common.identical(diffs)
    h = headers(
        e, "", {o: f"_{o}_mask.nii.gz" for o in ("inskull", "outskull", "outskin", "skull")}
    )
    for x in diffs:
        _verdict(x, same, "this head", h)
    what = "this head"
    surf_files = {o: f"_{o}_mask.nii.gz" for o in ("inskull", "outskull", "outskin", "skull")}
    _step_verdicts(diffs, same, what, e, n, "", surf_files)
    _orig_verdicts(diffs, same, arrays, SURF_OUTPUTS)
    # the middle axial slice of the display grid; the crosshair on the skull mask nearest its centre
    sk = ex.values(ex.key("fsl", "skull")) != 0
    k = sk.shape[2] // 2
    pts = np.argwhere(sk[:, :, k])
    c = np.array(sk.shape[:2]) / 2
    i, j = pts[int(np.argmin(((pts - c) ** 2).sum(1)))] if len(pts) else c.astype(int)
    default = {
        "output": "skull",
        "plane": "axial",
        "voxel": [int(i), int(j), k],
        "panels": ["t1", "fsl", "replica", "image", "orig"],
        "diffs": ["replica-fsl", "image-fsl", "orig-fsl23"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(SURF_OUTPUTS), default=default,
                        window_panel="fsl", caption="the crosshair on the skull's mask.")  # fmt: skip
    vers = info.get("versions", {})
    return {
        "id": "betsurf-head",
        "title": "betsurf: the skull and the scalp",
        "summary": (
            "`betsurf` with a T2 (the call `bet -A2` makes, after FLIRT) on the parity suite's "
            "synthetic head, from FSL bet2's surface of it: profiles along the surface's normals "
            "through both images find the inner skull, the outer skull and the scalp; three "
            "surfaces are deformed onto them in turn. A synthetic head shows them whole: on a "
            "real head the scalp would show a face. FSL 6.0.7.17, fMRIPrep's FSL and the replica "
            "in both modes; choose the mask above the panels."
            " FSL 6.0.7.23's output, and the replica's run compared with it, are shown beside them (the replica's reference since the version update of 2026-10-29). LARMORX's clean-room original (Apache-2.0, written from the specification without FSL's source) runs the same call in its default and reproduce modes, compared with FSL 6.0.7.23, its reference."
        ),
        "facts": [
            {
                "label": "Data",
                "text": "the parity suite's synthetic head (96 × 112 × 84 voxels of 2 mm: "
                "ellipsoidal shells of brain, CSF, skull and scalp, an intensity ramp, seeded "
                "noise), T1- and T2-like",
            },
            {
                "label": "Command",
                "text": "`betsurf -o -m -s head.nii.gz head-t2.nii.gz head_mesh.vtk "
                "head_to_std.mat out`",
            },
            {
                "label": "Programs",
                "text": f"{vers.get('fsl', '?')}; {vers.get('fsl_in_fmriprep_image', '?')}; "
                f"{vers.get('larmorx-nc', '?')}; "
                + o.info().get("versions", {}).get("larmorx", "the clean-room original"),
            },
            {
                "label": "Grid",
                "text": f"{' × '.join(map(str, img.shape))} voxels of 2 mm, shown whole",
            },
            fsl_step.fact(same["fsl23-fsl"], what, RECORDS_TEXT),
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": [
            *common.provenance(info, f"{TOOL}/{e.case}", [SYNTHETIC]),
            *common.provenance(n.info(), f"{TOOL}/{n.case}", [SYNTHETIC])[1:],
            *common.provenance(o.info(), f"{TOOL}/{o.case}", [SYNTHETIC])[1:],
        ],
    }


# ------------------------------------------------------------------------------------ cases


def input_name(name: str) -> str:
    """A case's input, as a reader needs it."""
    kind, _, rest = name.partition(":")
    if name.startswith("/"):
        p = Path(name)
        if "fmriprep-25.2.6" in p.parts:
            run = p.parts[p.parts.index("fmriprep-25.2.6") + 1].split("-sub")[0]
            return f"<fMRIPrep {run}: {p.name}>"
        return f"<{p.name}>"
    if kind == "real":
        return f"<{rest}: a real T1w or EPI of the test data>"
    if kind in ("mesh", "mat", "t2"):
        what = {
            "mesh": "bet2 -e's mesh",
            "mat": "FLIRT's matrix to MNI152",
            "t2": "the T2w, FLIRT-registered",
        }
        return f"<{rest}: {what[kind]}>"
    return f"<generated: {name}>"


def command(s) -> str:
    """A case's command line: its inputs named as placeholders, the environment and the files
    present."""
    import re

    names = [input_name(n) for n in s.inputs]
    parts = [s.tool]
    for a in s.args:
        m = re.fullmatch(r"\{(\d+)\}", a)
        if m:
            parts.append(names[int(m.group(1))])
        elif re.match(r"/\S*/fsl-\d+(?:\.\d+)+/", a):
            # a file of the FSL installation (standard_space_roi's -roiMASK, -maskMASK): this
            # machine's path never reaches the page
            parts.append(re.sub(r"^/\S*?/fsl-\d+(?:\.\d+)+/", "$FSLDIR/", a))
        elif a == "" or re.search(r"\s", a):
            parts.append(shlex.quote(a))
        else:
            parts.append(a)
    env = [f"FSLOUTPUTTYPE={s.output_type}"] + [f"{k}={v}" for k, v in s.env if v is not None]
    files = " ".join(f"(with {n} present)" for n, _ in s.files)
    return " ".join(x for x in (" ".join(env), " ".join(parts), files) if x)


#: The register entries the replica corrects by default (FSL-211 to FSL-217: bet's -S, -B, -A,
#: -A2, standard_space_roi and fslroi, 2026-11-04).
CORRECTED = frozenset(
    f"FSL-{n}"
    for n in (40, 43, 44, 45, 46, 47, 50, 51, 54, 56, 60, 62, 63, 211, 212, 213, 215, 216, 217)
)
#: The corrections inside the programs the variations run (FLIRT, FAST, fslstats), which the
#: record's D13 section names per case.
INNER = {
    "FSL-09": "FLIRT's integer output inside standard_space_roi",
    "FSL-14": "FLIRT's NaN voxels",
    "FSL-182": "fslstats' standard deviation",
    "FSL-202": "FAST's class variances, which -B's bias field comes from",
}
#: The record's D13 section: case -> the entries the default named there (``d13_named``).
D13_BY_CASE: dict[str, list[str]] = {}


def d13_named(repo: Path, record: str) -> dict[str, list[str]]:
    """The record's D13 section read back: each case with the register entries the default mode
    named on it (``- FSL-nn: ... : `case`, `case` …``; a list cut short names fewer)."""
    import re

    out: dict[str, list[str]] = {}
    md = repo / "docs" / "validation" / f"{record}.md"
    for line in md.read_text(encoding="utf-8").splitlines():
        m = re.match(r"\s+- ((?:FSL|GLIBC)-\d+): \d+ case\(s\) named it", line)
        if not m:
            continue
        for case in re.findall(r"`([a-z0-9]+/[^`]+)`", line.split("): ", 1)[-1]):
            out.setdefault(case, []).append(m.group(1))
    return out


#: Refused by default (rule B): the entry -> what FSL does with the input (observable behaviour).
REFUSED = {
    "FSL-42": "bet2 checks -f and -g against their ranges only (NaN passes) and the other numeric "
    "options not at all, and computes with them",
    "FSL-48": "betsurf samples a T2 on another grid at the T1's voxel coordinates",
    "FSL-53": "betsurf computes with the matrix entries it could not read",
    "FSL-55": "the 98th percentile of values that include NaN is undefined",
    "FSL-61": "bet2 applies the brain mask made on --binaryMask's grid over an input of other "
    "dimensions, reading and writing outside it",
    "FSL-214": "standard_space_roi checks no step's status: after a program fails, the next "
    "steps run on missing files and the script exits with its cleanup's status",
}


def _d13_note(case: str, desc: str, fix, refuses, d13: dict[str, list[str]]) -> str:
    """What the default mode does on a case: the corrections and refusals its description names
    and the record's D13 section names on it (``d13``), the corrections inside the programs a
    script runs among them; the values that change (``fix``)."""
    import re

    ids = re.findall(r"\b(FSL-\d+)\b", desc)
    named = [d for d in dict.fromkeys([*ids, *d13.get(case, [])]) if d in CORRECTED]
    refused = [d for d in dict.fromkeys(ids) if d in REFUSED and d != refuses]
    inner = [d for d in d13.get(case, []) if d in INNER and d not in named]
    parts = []
    if named:
        parts.append(f"the replica corrects {', '.join(named)} and names it")
    if refused:
        parts.append(
            "it stops with an error naming "
            + ", ".join(f"{d} (rule B: {REFUSED[d]})" for d in refused)
        )
    if inner:
        parts.append(
            "the programs the script runs correct " + ", ".join(f"{d} ({INNER[d]})" for d in inner)
        )
    if not parts:
        return ""
    size = f": {fix:,} values change" if fix else (": no output changes" if inner else "")
    return "in its default mode " + ", and ".join(parts) + size + " (the record's D13 section)"


def case_verdict(r: dict, desc: str, s, d13: dict[str, list[str]] | None = None) -> tuple[str, str]:
    """A case's verdict: the record compares the replica in reproduce mode with FSL (every file
    byte for byte, exit status, messages); the fix pass, fMRIPrep's own outputs and FSL's
    variability are reported."""
    import re

    from larmorx_validation.parity.fsl_bet import FILLED, PENDING

    st = r["status"]
    refuses = getattr(s, "fix_refuses", None)
    if refuses in REFUSED and st == "pass":
        return "d13", common.clip(
            f"Refused by default ({refuses}, rule B: the input makes no sense and FSL silently "
            f"does something arbitrary): {REFUSED[refuses]}. The replica's default stops with an "
            "error naming the entry (exit status 1); its reproduce mode gives FSL's files, which "
            "the record compares byte for byte.",
            400,
        )
    if st == "both-error" and "crash" in r["reason"]:
        return "divergence", common.clip(
            r["reason"].replace(
                "FSL crashes, larmorx refuses: ",
                "FSL aborts or crashes here; "
                "the replica prints FSL's messages before it and stops: ",
            )
        )
    if st == "expected-divergence" and r["reason"].startswith(FILLED):
        status = r["reason"].removeprefix(FILLED).split(")", 1)[0]
        # a random case names its entry in the record's reason only
        refuses = refuses or next(iter(re.findall(r"names (FSL-\d+)", r["reason"])), "FSL")
        return "divergence", common.clip(
            f"FSL crashes here (status {status}) on what it leaves undefined ({refuses}). The "
            "replica's default refuses with the reason; with reproduce it keeps every part FSL "
            "defines, fills the rest deterministically, names the entry and writes its outputs "
            "(run twice: the same bytes; D13's rule for failure cases). FSL's messages before "
            "the crash are compared; it writes no output to compare.",
            400,
        )
    if st == "expected-divergence" and r["reason"].startswith("FSL-43: "):
        return "divergence", common.clip(
            "FSL's fill of the mask starts outside the image and reads memory beyond the volume "
            "(FSL-43): its values there depend on that memory. The replica's reproduce does "
            "FSL's fill point by point inside the volume, reads that memory as not part of the "
            "mask and names the entry; its default starts the fill at the nearest voxel inside.",
            400,
        )
    if st == "expected-divergence" and "fslpy converts" in r["reason"]:
        return "divergence", common.clip(
            "Not replicated yet: standard_space_roi copies an input of another file type than "
            "FSLOUTPUTTYPE's with fslpy's imcp, which converts it with nibabel. The replica "
            "refuses that conversion clearly in both modes, naming it (findings/fsl-flirt.md, "
            '"Not replicated yet"); every other step of the script is FSL\'s.',
            400,
        )
    if st == "expected-divergence":
        return "divergence", common.clip("Pending: " + r["reason"].removeprefix(PENDING))
    d13 = d13 or {}
    fix = {**r.get("reported", {}), **r.get("measured", {})}.get("D13 fix pass")
    v = common.outcome_verdict(r)
    if v is not None:
        note = _d13_note(r["case"], desc, fix, refuses, d13)
        if v[0] == "both-reject" and note:
            return v[0], common.clip(f"{v[1].rstrip('.')}. {note[0].upper()}{note[1:]}.", 600)
        return v
    rep = {**r.get("reported", {}), **r.get("measured", {})}
    notes = ["every file FSL writes, byte for byte, with FSL's exit status and messages"]
    own = [k for k in rep if k.endswith("fMRIPrep's own output (as it ran)")]
    if own:
        notes.append("the values of fMRIPrep's own outputs of the node")
    fm = rep.get("fMRIPrep's FSL against the oracle")
    if fm == 0:
        notes.append(
            "fMRIPrep's FSL gives the same values (its headers differ in the build string)"
        )
    if "FSL run again" in rep:
        notes.append("FSL run again: the same bytes")
    dice = {k: v for k, v in rep.items() if k.startswith("Dice with the known brain (FSL)")}
    for v in dice.values():
        notes.append(
            f"FSL's mask against the phantom's known brain: Dice {v:.4f}, the replica's the same"
        )
    verdict = "bit-identical"
    note = _d13_note(r["case"], desc, fix, refuses, d13)
    if note:
        notes.append(note)
    return verdict, common.clip("; ".join(notes), 600)


def case_verdict_orig(r: dict, desc: str, s, d13: dict | None = None) -> tuple[str, str]:
    """A case's verdict for the clean-room original: its record compares it with FSL 6.0.7.23 by
    specs/bet.md §11's thresholds (bit-identical where it is; betsurf's masks Dice >= 0.99 and
    boundary p95 <= 1 voxel), exit status and messages identical."""
    v, why = case_verdict(r, desc, s)
    who = "the clean-room original"
    why = (
        why.replace("The replica's", "The clean-room original's")
        .replace("The replica", "The clean-room original")
        .replace("the replica's", f"{who}'s")
        .replace("the replica", who)
    )
    if v != "bit-identical":
        return v, why
    differ = {k: x for k, x in r.get("measured", {}).items() if not _zero(x)}
    if not differ:
        why = why.replace(
            "every file FSL writes, byte for byte",
            "every value of every file FSL writes, its headers but for the program's name in "
            "`descrip`",
        )
        return v, why
    shown = "; ".join(f"{k}: {x}" for k, x in list(differ.items())[:4])
    surf = getattr(s, "tool", "") == "betsurf"
    return "open", common.clip(
        "Within the record's thresholds, not bit-identical (masks: Dice; boundary distance p95 / "
        f"max in voxels; drawn surfaces: their distance p95 / max): {shown}."
        + (
            " betsurf's profiles agree with FSL's one by one; where few of them vote, and at the "
            "scalp, its surfaces settle a little elsewhere (docs/findings/fsl-bet-original.md, "
            "betsurf)."
            if surf
            else ""
        ),
        600,
    )


def _zero(x) -> bool:
    """A measured value that says "no difference": 0, or a text of numbers that are all 0 (a
    mesh's vertex distance, ``0.0000 / 0.0000``)."""
    import re

    if isinstance(x, (int, float)):
        return x == 0
    nums = re.findall(r"[-+]?\d+(?:\.\d+)?(?:e[-+]?\d+)?", str(x))
    return bool(nums) and all(float(n) == 0 for n in nums)


def cases(
    rec, scenarios, examples: dict[str, str], impl: str = "replica", into: list | None = None
) -> list[dict]:
    """The record's cases as the case explorer's, its results as ``impl``'s; ``into``: cases of
    another record, to which this one's results join (a case of its own where that record has
    none)."""
    defs = {cid: (desc, s) for cid, _cat, desc, s in scenarios}
    out = [] if into is None else into
    by_id = {c["id"]: c for c in out}
    verdict_of = case_verdict_orig if impl == "original" else case_verdict
    for r in rec.results:
        desc, s = defs.get(r["case"], ("", None))
        v, why = verdict_of(r, desc, s, d13=D13_BY_CASE if impl == "replica-23" else None)
        result = {
            "outcome": r["status"],
            "verdict": v,
            "checks": f"{r['checks_passed']}/{r['checks_total']}",
            "reason": why,
        }
        if r["case"] in by_id:
            by_id[r["case"]]["results"][impl] = result
            continue
        c = {
            "id": r["case"],
            "category": r["category"],
            "description": desc,
            "command": command(s) if s else "",
            "results": {impl: result},
            **({"example": examples[r["case"]]} if r["case"] in examples else {}),
        }
        out.append(c)
        by_id[c["id"]] = c
    return out


def status_of(rec) -> str:
    return "experimental" if rec.data.get("summary", {}).get("fail") else "validated"


#: The benchmark's series (``docs/benchmarks/fsl-bet.json``, from
#: ``validation/investigations/cleanroom-bet/bench.py``): FSL is single-threaded.
SPEED_THREADS = 24
SPEED_SERIES = [
    {"id": "FSL 6.0.7.23|1 thread", "label": "FSL 6.0.7.23 (single-threaded)", "mark": "original"},
    {"id": "replica (larmorx-nc)|1 thread", "label": "replica, 1 thread", "mark": "replica"},
    {"id": f"replica (larmorx-nc)|{SPEED_THREADS} threads",
     "label": f"replica, {SPEED_THREADS} threads", "mark": "replica-open"},
    {"id": "original (larmorx)|1 thread", "label": "clean-room original, 1 thread",
     "mark": "larmorx"},
    {"id": f"original (larmorx)|{SPEED_THREADS} threads",
     "label": f"clean-room original, {SPEED_THREADS} threads", "mark": "larmorx-open"},
]  # fmt: skip


def speed(ctx: core.Context):
    bench = records.load_bench(ctx.repo, TOOL)
    chart = common.speed(
        bench,
        SPEED_SERIES,
        {},
        title="Seconds for the whole command (read, compute, write), median of repeated runs, "
        "log scale; the factor is the clean-room original's speed-up over FSL at one thread",
        base=(SPEED_SERIES[0]["id"], SPEED_SERIES[3]["id"]),
        note="From the benchmark report `docs/benchmarks/fsl-bet.json` ({cpu}, {cpus} logical "
        "CPUs; each program in a clean environment, outputs NIfTI-1 gzipped).",
    )
    return bench, chart


#: The clean-room catch-up's trial of the original and the replica (2026-11-05), and the
#: original's findings in categories (``validation/investigations/cleanroom-bet/
#: trial_catchup_categories.py``'s ``--json`` output, with the replay of every finding by the
#: final build).
TRIAL_CATCHUP = "2026-11-05-catchup-bet"
CATCHUP_TRIAGE = "validation/investigations/cleanroom-bet/trial_catchup_triage.json"
#: category -> (what the page says, the verdict of the row; None: fixed after the trial).
CATCHUP_TEXT: dict[str, tuple[str, str | None]] = {
    "fixed: FLIRT's search, starting points of equal cost (std::sort)": (
        "Fixed after the trial (relay 10), with permanent cases: standard_space_roi's FLIRT (and "
        "bet -B's, inside its standard_space_roi) ended 7 to 300 mm (RMS) from FSL's matrix on "
        "small heads whose costs tie at the coarse scales (every cost 1, FSL's own measure of "
        "both matrices): FLIRT's search kept its starting points of equal cost in the order "
        "found, where FSL sorts them with libstdc++'s std::sort (trial_ssroi_matrices.py); each "
        "agrees on replay with the final build",
        None,
    ),
    "fixed: the output's box, from FLIRT's matrix (the search's equal costs)": (
        "Fixed after the trial by the same change: standard_space_roi's output box, which follows "
        "the region brought back by FLIRT's matrix, a few voxels from FSL's on small heads; each "
        "agrees on replay with the final build",
        None,
    ),
    "betsurf's surfaces on small heads: the scalp's deformation": (
        "betsurf's surfaces on small synthetic heads (betsurf, bet -A, -A2): the profiles are "
        "decided as FSL decides them (162 of 164 images' shares of profiles set aside FSL's), "
        "the skulls follow; the scalp's deformation settles a little elsewhere (0.24 to 3.4 mm on "
        "a clean testbed), relay 7's open class, for BET's next clean-room part",
        "open",
    ),
    "standard_space_roi: FSL's FLIRT crashes (FSL-75)": (
        "standard_space_roi runs whose FLIRT crashes in FSL (Segmentation fault, free(): invalid "
        "pointer or size: FSL-75's corrupted heap, undefined behaviour never reproduced), so "
        "FSL's later steps run on missing files (fslroi's usage), where the original registers "
        "and goes on",
        "divergence",
    ),
    "standard_space_roi: FLIRT's registration ends elsewhere": (
        "After the fix, a registration of a small head that still ends elsewhere than FSL's: "
        "88 at a lower cost than FSL's by FSL's own measure (0.964919 against 0.967734 at 1 mm), "
        "114 at a higher one (0.989526 against 0.985458), 803 (its output box following the "
        "matrix; given FSL's matrix every file FSL's): for FLIRT's next clean-room part",
        "open",
    ),
    "standard_space_roi: the cut's translation one float unit from FSL's": (
        "standard_space_roi's output header, FSL's matrix given, every value FSL's: the cut's "
        "translation one float unit in the last place from FSL's (e.g. qoffset_z 8.25 against "
        "8.250000954), as fslroi's corner below: a question for the specification "
        "(specs/standard_space_roi.md §3.2)",
        "open",
    ),
    "standard_space_roi: fslpy's imcp conversion (nibabel)": (
        "-maskNONE -roiNONE (or -b -maskNONE) on an input in another format than "
        "FSLOUTPUTTYPE's: fslpy's imcp converts it with nibabel, whose header has pixdim[4..7] 1 "
        "where the input's are 0 and scl_slope 1 where it is 0, and NIfTI-2 for a NIFTI2 type; "
        "the original converts the layout and keeps the header (specs/standard_space_roi.md "
        "§2.5: pending until its bytes are checked)",
        "open",
    ),
    "standard_space_roi: the cut's corner whole voxels from FSL's": (
        "standard_space_roi's output header, FSL's matrix given, on inputs whose qform and sform "
        "differ: one form's translation whole voxels from FSL's (e.g. qoffset_x -14.25 against "
        "-38.25), the box, its values and the other form FSL's: which form FSL's fslroi moves "
        "by which corner is a question for the specification (specs/standard_space_roi.md §3.2)",
        "open",
    ),
    "standard_space_roi: a NIfTI-2 output's matrices": (
        "standard_space_roi's NIfTI-2 output of a NIfTI-1 input, FSL's matrix given: FSL's "
        "matrices in single precision (-44.40308380 against the original's -44.40308296), and a "
        "sform of code 0 moved by the cut too (its translation -1.2412 where the original keeps "
        "the input's 51.25): a question for the specification (specs/standard_space_roi.md §3.2)",
        "open",
    ),
    "fslroi: the corner's translation one float unit from FSL's": (
        "fslroi's boxes of a neurologically stored oblique image reaching its last x voxel: the "
        "corner's translation one float unit in the last place from FSL's (FSL's equals the "
        "double product of the matrix and the corner rounded once): a question for the "
        "specification (specs/standard_space_roi.md §3.2)",
        "open",
    ),
    "bet -B, -A2: FSL fails or crashes midway": (
        "bet -B and -A2 on small heads where FSL's script fails or crashes midway (Error: input "
        "image ... not found; a program crashing) and the original's messages or exit status "
        "differ: not yet looked at, for BET's next clean-room part",
        "open",
    ),
    "bet with -R or -c on a neurologically stored image: the centre": (
        "bet with -R or -c on a neurologically stored image: the centre's conversion into FSL's "
        "radiological frame starts bet2 a few bits elsewhere (the specification's question of "
        "cases 622 and 677)",
        "open",
    ),
}


def trial_catchup(ctx: core.Context) -> tuple[list[dict], dict]:
    """The clean-room catch-up's random-testing trial of both implementations (its
    ``summary.json``) and the original's findings in categories (``trial_catchup_triage.json``):
    two tables, and their provenance."""
    path = common.oracles() / "difftest" / TRIAL_CATCHUP / "summary.json"
    raw = path.read_bytes()
    s = json.loads(raw)
    t = s["tools"]["fsl-bet"]
    o = t["options"]
    orig = t["implementations"]["original"]
    rep = t["implementations"]["replica"]
    types = sum(v for k, v in o.items() if k.startswith("FSLOUTPUTTYPE=")) - o.get(
        "FSLOUTPUTTYPE=NIFTI_GZ", 0
    )
    rows = [
        ("Random cases, each run by FSL 6.0.7.23 and both implementations", t["cases"]),
        ("the original: within the record's thresholds, every exact check passed",
         orig.get("agree", 0)),
        ("the original: differing (each one in a category below)", orig.get("differ", 0)),
        ("the original: a register entry named in one mode only (a notice)", orig.get("notice", 0)),
        ("the original: undefined (FSL's values depend on memory it never wrote)",
         orig.get("undefined", 0)),
        ("the replica: FSL's bytes", rep.get("agree", 0)),
        ("the replica: differing (FSL's FLIRT crashing inside standard_space_roi: FSL-75)",
         rep.get("differ", 0)),
        ("bet2, bet, betsurf", [o.get(k, 0) for k in ("bet2", "bet", "betsurf")]),
        ("standard_space_roi, fslroi", [o.get(k, 0) for k in ("standard_space_roi", "fslroi")]),
        ("bet's variations -S, -B, -A, -A2", [o.get(k, 0) for k in ("-S", "-B", "-A", "-A2")]),
        ("bet's -R, -F, -Z", [o.get(k, 0) for k in ("-R", "-F", "-Z")]),
        ("an output type other than NIFTI_GZ", types),
        ("distinct options and settings drawn", t["options_covered"]),
        (f"length (minutes; {s['jobs']} jobs at one thread each)", round(s["elapsed_s"] / 60, 1)),
    ]  # fmt: skip
    out_rows = []
    for head, v in rows:
        if isinstance(v, list):
            cell = common.cell(" / ".join(f"{x:,}" for x in v))
        elif isinstance(v, float):
            cell = common.cell(v, "sig")
        else:
            cell = common.cell(int(v), "count")
        out_rows.append({"head": head, "cells": [cell]})
    peaks = max(float(p.split()[0]) for p in s["memory"]["peaks"])
    commit = s["larmorx_commit"].removesuffix("-dirty")
    summary = common.table(
        "trial-catchup",
        f"Random testing after the clean-room catch-up: the trial of {s['date']}",
        ["", {"label": "Cases", "num": True}],
        out_rows,
        note=f"Differential random testing's trial of both implementations after the clean-room "
        f"original gained bet's variations, standard_space_roi and fslroi (seed `{s['seed']}`: "
        "the BET part's trial of 2026-11-04 drawn again and further, FSL's runs served by the "
        "oracle cache where they had run), each case compared as the records compare (the "
        "replica: FSL's bytes; the original: specs/bet.md §11 and §11.1, "
        "specs/standard_space_roi.md §8). The build: larmorx "
        f"`{commit}`; two classes were fixed after it, from its findings. Every case in a memory "
        f"box of {s['memory']['case']} (peaks below {peaks:.2f} GiB).",
    )
    tpath = ctx.repo / CATCHUP_TRIAGE
    traw = tpath.read_bytes()
    tri = json.loads(traw)
    counts = tri["counts"]
    if sum(counts.values()) != orig.get("differ", 0):
        raise ValueError(
            f"{CATCHUP_TRIAGE}: {sum(counts.values())} findings, the trial {orig['differ']}"
        )
    trows = []
    for cat, n in sorted(counts.items(), key=lambda kv: -kv[1]):
        text, verdict = CATCHUP_TEXT[cat]
        # the verdict as a cell of its own (the site draws a cell's verdict, not a row's)
        vcell = (
            {"verdict": verdict}
            if verdict
            else {"text": "fixed: agrees on replay with the final build"}
        )
        row = {"head": text, "cells": [common.cell(int(n), "count"), vcell,
                                       common.cell(tri["reproducers"][cat])]}  # fmt: skip
        if verdict:
            row["verdict"] = verdict
        trows.append(row)
    triage = common.table(
        "trial-catchup-triage",
        "The original's differing cases, each in one category",
        ["Category", {"label": "Cases", "num": True}, "Verdict", "Reproducer"],
        trows,
        note="Every differing case of the clean-room original in one category by its program, "
        "its failed checks and their details, and by its replay with the final build "
        "(`validation/investigations/cleanroom-bet/trial_catchup_categories.py`, its output "
        f"`{CATCHUP_TRIAGE}`; the replays: `replay_trial.py`), each category's cause established "
        "by replaying findings and by probes (`trial_ssroi_matrices.py`, `finding_dirs.py`, "
        "`betsurf_shares.py`; `docs/findings/fsl-bet-original.md`); the triage's entries: "
        "`validation/difftest/triage.toml`. The reproducer is a finding of the trial "
        "(`difftest --replay`).",
    )
    prov = {
        "trial": f"oracles/difftest/{TRIAL_CATCHUP}/summary.json",
        "sha256": hashlib.sha256(raw).hexdigest(),
        "triage": CATCHUP_TRIAGE,
        "triage_sha256": hashlib.sha256(traw).hexdigest(),
    }
    return [summary, triage], prov


def refresh_original(ctx: core.Context, why: str) -> None:
    """After a change of the clean-room original: its entries (every run in them is the
    original's) computed again: fMRIPrep's call and bet -R on ds000005, the data panel's child,
    betsurf on the synthetic head, and its runs of the variations' examples
    (:mod:`fsl_bet_options_original`)."""
    fctx = ctx.figures_context()
    for case, what, compute in (
        (CASE, "the clean-room original on fMRIPrep's BET call (ds000005 sub-01 run 1) and bet "
         "-R -m on the subject's T1w, default and reproduce", _compute_orig),
        (PANEL_CASE, "the clean-room original on fMRIPrep's BET call on the BOLD reference of the "
         "data panel's ds000228 sub-pixar008 (3.5 years)", _compute_panel_orig),
        ("betsurf-synthetic-head", "betsurf -o -m -s with the T2 on the synthetic head: the "
         "clean-room original", _compute_surf_orig),
    ):  # fmt: skip
        fcache.recompute(fctx, TOOL, f"{case}-original", what, compute, why)
    fsl_bet_options_original.refresh(ctx, why)


def export(ctx: core.Context) -> dict:
    from larmorx_validation.parity.fsl_bet import _scenarios

    rec = records.load(ctx.repo, RECORD)
    rec_new = records.load(ctx.repo, RECORD_NEW)
    rec_orig = records.load(ctx.repo, RECORD_ORIG)
    examples = {
        "fmriprep/ds000005-task_mixedgamblestask_run_01-enhance_and_skullstrip_bold": "fmriprep-bet",
        "real/adult-ds000005-R": "t1w-robust",
        "betsurf/t1t2": "betsurf-head",
        "fmriprep/ds000228-task_pixar-enhance_and_skullstrip_bold": "panel-ds000228-child",
        **{s.case: s.example for s in fsl_bet_options.SPECS},
    }
    D13_BY_CASE.clear()
    D13_BY_CASE.update(d13_named(ctx.repo, RECORD_NEW))
    cs = cases(rec, _scenarios(), examples)
    cases(rec_new, _scenarios(), examples, impl="replica-23", into=cs)
    cases(rec_orig, _scenarios(), examples, impl="original", into=cs)
    e = entry(ctx)
    s = surf_entry(ctx)
    pe = panel_entry(ctx)
    n, sn, pn = entry_new(ctx), surf_entry_new(ctx), panel_entry_new(ctx)
    o, so, po = entry_orig(ctx), surf_entry_orig(ctx), panel_entry_orig(ctx)
    opts = fsl_bet_options.entries(ctx)
    opts_orig = fsl_bet_options_original.entries(ctx)
    tc, tcprov = trial_catchup(ctx)
    bench, chart = speed(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "BET",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source() + rec_new.source() + rec_orig.source(),
            "benchmarks": bench.source() if bench else [],
            "cache": [
                fcache.provenance(x) for x in (e, s, pe, n, sn, pn, o, so, po, *opts, *opts_orig)
            ],
            "difftest_catchup": tcprov,
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cs,
                "replica",
                "larmorx-nc replica, against FSL 6.0.7.17 (full tier)",
                "replica",
                f"docs/validation/{RECORD}.md",
                status_of(rec),
            ),
            common.implementation(
                cs,
                "replica-23",
                "larmorx-nc replica, against FSL 6.0.7.23 (standard tier)",
                "replica, 6.0.7.23",
                f"docs/validation/{RECORD_NEW}.md",
                status_of(rec_new),
            ),
            common.implementation(
                cs,
                "original",
                f"larmorx clean-room original, against FSL {fsl_step.NEW} (standard tier)",
                "original",
                f"docs/validation/{RECORD_ORIG}.md",
                status_of(rec_orig),
            ),
        ],
        "cases": cs,
        "cases_total": len(cs),
        "speed": chart,
        "tables": tc,
        "examples": [
            _example_fmriprep(ctx, e, n, o),
            _example_t1(ctx, e, n, o),
            _example_surf(ctx, s, sn, so),
            _example_panel(ctx, pe, pn, po),
            # the replica's examples of the variations, each with the clean-room original's
            # panels (fsl_bet_options_original)
            *fsl_bet_options_original.examples(ctx, rec_new, rec_orig),
        ],
    }


if __name__ == "__main__":  # compute the cache entries only; refresh-original <why>
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    if sys.argv[1:2] == ["refresh-original"]:
        refresh_original(c, sys.argv[2])
        print("refreshed", [x.dir for x in fsl_bet_options_original.entries(c)])
        raise SystemExit(0)
    print("done", entry(c).dir, surf_entry(c).dir, panel_entry(c).dir)
    print("done", entry_new(c).dir, surf_entry_new(c).dir, panel_entry_new(c).dir)
    print("done", entry_orig(c).dir, surf_entry_orig(c).dir, panel_entry_orig(c).dir)
