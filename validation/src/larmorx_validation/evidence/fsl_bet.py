# SPDX-License-Identifier: Apache-2.0
"""BET's evidence: fMRIPrep's call (``bet <boldref> <out> -f 0.20 -m``) on a real BOLD reference
by FSL 6.0.7.17, by fMRIPrep's own FSL (its image) and by the larmorx-nc replica in its default
and ``reproduce`` modes; ``bet -R -m`` on a real T1w; ``betsurf`` (T1 and T2) on the synthetic
head of the parity suite, whose skull and scalp surfaces can be shown whole without a face; the
cases of the record.

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
import os
import shlex
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core, fsl_step
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
    fsl, nc, sif = binaries(version)
    home = e.dir / ".home"
    home.mkdir(exist_ok=True)
    add, unset = environment(home, version)
    for label, kind, mode in runners:
        stem = str(e.dir / f"{tag}{label}")
        argv = [a.replace("{out}", stem) for a in args]
        if kind == "fsl":
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
        "record's threshold: every file bit-identical to FSL's (header and data), with its exit "
        "status and messages."
    )


def _example_fmriprep(ctx: core.Context, e: fcache.Entry, n: fcache.Entry) -> dict:
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
    for pid, _stem, role, label, sub in PANELS_NEW:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    d = common.d
    diffs = [
        *_diffs()[:4],
        d("replica-node", "replica", "node", "fmriprep", "replica − fMRIPrep's own output",
          common.ROLE_FMRIPREP),
        _diffs()[4],
        *_new_diffs(),
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
    at, _k = mask_view(ex)
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["boldref", "fsl", "replica", "node"],
        "diffs": ["replica-fsl", "image-fsl", "replica-node"],
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
            " FSL 6.0.7.23's output, and the replica's run compared with it, are shown beside them (the replica's reference since the version update of 2026-10-29)."
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
                f"`{info['larmorx_commit'][:12]}`",
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
        ],
    }


def _example_panel(ctx: core.Context, pe: fcache.Entry, pn: fcache.Entry) -> dict:
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
    for pid, _stem, role, label, sub in PANELS_NEW:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    d = common.d
    diffs = [
        *_diffs()[:4],
        d("replica-node", "replica", "node", "fmriprep", "replica − fMRIPrep's own output",
          common.ROLE_FMRIPREP),
        _diffs()[4],
        *_new_diffs(),
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
    at = common.slice_voxel(ex, "replica", "fsl", "mask")
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["boldref", "fsl", "replica", "node"],
        "diffs": ["replica-fsl", "image-fsl", "replica-node"],
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
            " FSL 6.0.7.23's output, and the replica's run compared with it, are shown beside them (the replica's reference since the version update of 2026-10-29)."
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
                f"`{info['larmorx_commit'][:12]}`",
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
        ],
    }


def _example_t1(ctx: core.Context, e: fcache.Entry, n: fcache.Entry) -> dict:
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    img = nib.load(td.get(T1W))
    arrays = runs(e, "t1-")
    arrays.update(runs(n, "t1-", PANELS_NEW))
    # only the brain is shown: the masks' union, dilated by 2 voxels (no face can show)
    show = ndimage.binary_dilation(
        np.logical_or.reduce([a["mask"] != 0 for a in arrays.values()]), iterations=2
    )
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("t1", "input", "ds000005 sub-01 T1w", "the raw image (int16), inside the "
                     "brain masks only", {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    over = {"mask": {"on": "t1", "window": window_of(ex, "t1")}}
    for pid, _stem, role, label, sub in [*PANELS, *PANELS_NEW]:
        if pid in arrays:
            ex.add(exm.Panel(pid, role, label.replace("bet", "bet -R"), sub, arrays[pid],
                             overlay=over))  # fmt: skip
    diffs = [*(x for x in _diffs() if "image" not in (x["a"], x["b"])), *_new_diffs()]
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    h = headers(e, "t1-", {"mask": "_mask.nii.gz", "brain": ".nii.gz"})
    for x in diffs:
        _verdict(x, same, "this T1w", h)
    what = "this T1w"
    _step_verdicts(diffs, same, what, e, n, "t1-", {"mask": "_mask.nii.gz", "brain": ".nii.gz"})
    at, _k = mask_view(ex)
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1", "fsl", "replica", "replica-reproduce"],
        "diffs": ["replica-fsl", "repro-fsl"],
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
            " FSL 6.0.7.23's output, and the replica's run compared with it, are shown beside them (the replica's reference since the version update of 2026-10-29)."
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
                "text": f"{vers.get('fsl', '?')}; {vers.get('larmorx-nc', '?')}. fMRIPrep's image "
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
        ],
    }


def _example_surf(ctx: core.Context, e: fcache.Entry, n: fcache.Entry) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.parity import fsl_bet as fb

    info = e.info()
    img = nib.load(str(fb.input_path("head")))
    t2 = nib.load(str(fb.input_path("head-t2")))
    arrays = surf_runs(e)
    arrays.update(surf_runs(n, PANELS_NEW))
    show = np.ones(img.shape[:3], dtype=bool)
    grid = core.Grid.around(img, show, margin=0)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("t1", "input", "synthetic head, T1-like", "brain, CSF, skull, scalp, 2 mm",
                     {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    ex.add(exm.Panel("t2", "input", "synthetic head, T2-like", "the same shells, T2 contrast",
                     {"all": np.asanyarray(t2.dataobj)}))  # fmt: skip
    over = {o["id"]: {"on": "t1", "window": window_of(ex, "t1")} for o in SURF_OUTPUTS}
    for pid, _stem, role, label, sub in [*PANELS, *PANELS_NEW]:
        ex.add(exm.Panel(pid, role, label.replace("bet", "betsurf"), sub, arrays[pid],
                         overlay=over))  # fmt: skip
    diffs = [*_diffs(), *_new_diffs()]
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
        "panels": ["t1", "fsl", "replica", "image"],
        "diffs": ["replica-fsl", "image-fsl"],
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
            " FSL 6.0.7.23's output, and the replica's run compared with it, are shown beside them (the replica's reference since the version update of 2026-10-29)."
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
                f"{vers.get('larmorx-nc', '?')}",
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
        elif a == "" or re.search(r"\s", a):
            parts.append(shlex.quote(a))
        else:
            parts.append(a)
    env = [f"FSLOUTPUTTYPE={s.output_type}"] + [f"{k}={v}" for k, v in s.env if v is not None]
    files = " ".join(f"(with {n} present)" for n, _ in s.files)
    return " ".join(x for x in (" ".join(env), " ".join(parts), files) if x)


#: The register entries the replica corrects by default.
CORRECTED = frozenset(f"FSL-{n}" for n in (40, 43, 44, 45, 46, 47, 50, 51, 54, 56, 60, 62, 63))
#: Refused by default (rule B): the entry -> what FSL does with the input (observable behaviour).
REFUSED = {
    "FSL-42": "bet2 checks -f and -g against their ranges only (NaN passes) and the other numeric "
    "options not at all, and computes with them",
    "FSL-48": "betsurf samples a T2 on another grid at the T1's voxel coordinates",
    "FSL-53": "betsurf computes with the matrix entries it could not read",
    "FSL-55": "the 98th percentile of values that include NaN is undefined",
    "FSL-61": "bet2 applies the brain mask made on --binaryMask's grid over an input of other "
    "dimensions, reading and writing outside it",
}


def case_verdict(r: dict, desc: str, s) -> tuple[str, str]:
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
    if st == "expected-divergence":
        return "divergence", common.clip("Pending: " + r["reason"].removeprefix(PENDING))
    v = common.outcome_verdict(r)
    if v is not None:
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
    fix = rep.get("D13 fix pass")
    named = [d for d in re.findall(r"\b(FSL-\d+)\b", desc) if d in CORRECTED]
    verdict = "bit-identical"
    if named:
        notes.append(
            f"in its default mode the replica corrects {', '.join(named)} and names it"
            + (f": {fix:,} values change" if fix else "")
            + " (the record's D13 section)"
        )
    return verdict, common.clip("; ".join(notes), 600)


def cases(
    rec, scenarios, examples: dict[str, str], impl: str = "replica", into: list | None = None
) -> list[dict]:
    """The record's cases as the case explorer's, its results as ``impl``'s; ``into``: cases of
    another record, to which this one's results join (a case of its own where that record has
    none)."""
    defs = {cid: (desc, s) for cid, _cat, desc, s in scenarios}
    out = [] if into is None else into
    by_id = {c["id"]: c for c in out}
    for r in rec.results:
        desc, s = defs.get(r["case"], ("", None))
        v, why = case_verdict(r, desc, s)
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


def export(ctx: core.Context) -> dict:
    from larmorx_validation.parity.fsl_bet import _scenarios

    rec = records.load(ctx.repo, RECORD)
    rec_new = records.load(ctx.repo, RECORD_NEW)
    examples = {
        "fmriprep/ds000005-task_mixedgamblestask_run_01-enhance_and_skullstrip_bold": "fmriprep-bet",
        "real/adult-ds000005-R": "t1w-robust",
        "betsurf/t1t2": "betsurf-head",
        "fmriprep/ds000228-task_pixar-enhance_and_skullstrip_bold": "panel-ds000228-child",
    }
    cs = cases(rec, _scenarios(), examples)
    cases(rec_new, _scenarios(), examples, impl="replica-23", into=cs)
    e = entry(ctx)
    s = surf_entry(ctx)
    pe = panel_entry(ctx)
    n, sn, pn = entry_new(ctx), surf_entry_new(ctx), panel_entry_new(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "BET",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source() + rec_new.source(),
            "benchmarks": [],
            "cache": [fcache.provenance(x) for x in (e, s, pe, n, sn, pn)],
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
        ],
        "cases": cs,
        "cases_total": len(cs),
        "speed": None,
        "examples": [
            _example_fmriprep(ctx, e, n),
            _example_t1(ctx, e, n),
            _example_surf(ctx, s, sn),
            _example_panel(ctx, pe, pn),
        ],
    }


if __name__ == "__main__":  # compute the cache entries only
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    print("done", entry(c).dir, surf_entry(c).dir, panel_entry(c).dir)
    print("done", entry_new(c).dir, surf_entry_new(c).dir, panel_entry_new(c).dir)
