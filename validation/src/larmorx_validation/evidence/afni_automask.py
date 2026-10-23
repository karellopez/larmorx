# SPDX-License-Identifier: Apache-2.0
"""3dAutomask's evidence: fMRIPrep's call on a real BOLD reference (its second skull strip) by
AFNI 25.2.09, by fMRIPrep's own AFNI, by the larmorx-gpl replica in its default and
``reproduce`` modes and by the clean-room original (both modes, 1 and 6 threads); AFNI's default
on the raw BOLD series it came from (the functional path: the series averaged first); a
deliberate correction (AFNI-43, ``-peels 0``) on the same reference; the cases of both records.

The replica (``larmorx-gpl``, GPL-3.0-or-later) runs as a separate program, as everywhere in
the validation package (Apache-2.0); the original (``larmorx afni 3dAutomask`` with
``LARMORX_IMPLEMENTATION=original``) has a cache entry of its own, beside the replica's. The
masks are drawn over the image they were made from.

The helpers that run AFNI, fMRIPrep's AFNI and the replica are shared with 3dUnifize's evidence
(:mod:`larmorx_validation.evidence.afni_unifize`).
"""

from __future__ import annotations

import copy
import os
import shlex
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "afni-automask"
PROGRAM = "3dAutomask"
CASE = "fmriprep-ds005040-clip-1"
RUN = "ds005040-sub-002-noreconall"
WF = (
    "work/fmriprep_25_2_wf/sub_002_wf/bold_task_clip_run_1_wf/bold_fit_wf/"
    "enhance_and_skullstrip_bold_wf"
)
NODE = f"{WF}/skullstrip_second_pass"
INPUT = f"{WF}/fixhdr_unifize/uni_xform.nii.gz"
FMRIPREP_MASK = f"{NODE}/uni_xform_mask.nii.gz"
BOLD = "openneuro/ds005040/sub-002/func/sub-002_task-clip_run-1_bold.nii.gz"
DATA = common.DS005040
#: The runs of every call: label suffix, program (afni, image: fMRIPrep's AFNI, replica), D13
#: mode
RUNNERS = [
    ("afni", "afni", None),
    ("image", "image", None),
    ("replica", "replica", "fix"),
    ("replica-reproduce", "replica", "reproduce"),
]


# --------------------------------------------------------------------- running the programs


def environment() -> tuple[dict[str, str], tuple[str, ...]]:
    """The environment of every run, as the parity records': an empty home directory (no
    ``.afnirc``), AFNI's log file off, no ``.nii`` added to names; no AFNI variable, preload,
    thread count or larmorx setting of this process. Returns (added, removed)."""
    unset = tuple(
        sorted(
            {k for k in os.environ if k.upper().startswith(("AFNI_", "NIML_", "LARMORX_"))}
            | {"LD_PRELOAD", "OMP_NUM_THREADS"}
        )
    )
    return {"AFNI_DONT_LOGFILE": "YES", "AFNI_NIFTI_NOEXT": "YES"}, unset


def binaries(program: str) -> tuple[str, str, Path]:
    from larmorx_validation.parity import afni_replica as ar

    afni, gpl, sif = ar.afni_binary(program), ar.replica_binary(), ar.fmriprep_image()
    if afni is None or gpl is None or sif is None:
        raise FileNotFoundError(
            f"the AFNI {program} oracle, the larmorx-gpl replica or fMRIPrep's image"
        )
    return afni, gpl, sif


def run_all(
    e: fcache.Entry,
    program: str,
    tag: str,
    args: list[str],
    inputs: list[Path],
    runners=RUNNERS,
    threads: str | None = None,
) -> None:
    """``program`` with ``args`` (``{out}``: the run's output stem) by each runner, into the
    entry's directory: ``<tag><label>.nii.gz`` (and the other names ``args`` derive from
    ``{out}``)."""
    afni, gpl, sif = binaries(program)
    add, unset = environment()
    home = e.dir / ".home"
    home.mkdir(exist_ok=True)
    base = {**add, "HOME": str(home)}
    for label, kind, mode in runners:
        stem = str(e.dir / f"{tag}{label}")
        argv = [a.replace("{out}", stem) for a in args]
        env = dict(base)
        if threads is not None:
            env["OMP_NUM_THREADS"] = threads
        if kind == "afni":
            e.run(f"{tag}{label}", [afni, *argv], env=env, unset=unset)
        elif kind == "image":
            cmd = ["apptainer", "exec", "--cleanenv", "--no-home"]
            for d in sorted({str(e.dir), *(str(p.parent) for p in inputs)}):
                cmd += ["--bind", f"{d}:{d}"]
            for k, v in env.items():
                cmd += ["--env", f"{k}={v}"]
            e.run(f"{tag}{label}", [*cmd, str(sif), program, *argv], unset=unset)
        else:
            env["LARMORX_UPSTREAM_DEFECTS"] = mode
            e.run(f"{tag}{label}", [gpl, "afni", program, *argv], env=env, unset=unset)


def image_version(program: str) -> str:
    """The version in the banner ``program`` prints in fMRIPrep's image (``-ver`` is not an
    option: the banner, then an error)."""
    import subprocess

    _, _, sif = binaries(program)
    p = subprocess.run(
        ["apptainer", "exec", "--cleanenv", "--no-home", str(sif), program, "-ver"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    line = next((x for x in (p.stdout + p.stderr).splitlines() if "AFNI version" in x), "?")
    return line.split("AFNI version=")[-1].strip()


def versions(program: str) -> dict[str, str]:
    from larmorx_validation.parity import afni_replica as ar

    return {
        "afni": ar.afni_build(program),
        "afni_in_fmriprep_image": image_version(program),
        "larmorx-gpl": ar.replica_version(),
        "fmriprep_image": (common.oracles() / "images" / "fmriprep-25.2.6.sif.sha256")
        .read_text(encoding="utf-8")
        .split()[0],
    }


def short_command(command: str) -> str:
    """A node's command line with fMRIPrep's working directory shortened to ``…/``."""
    import re

    return re.sub(r"/work/fmriprep_25_2_wf/\S*/", "…/", " ".join(command.split()))


def fmriprep_args(command: str, subs: dict[str, str]) -> list[str]:
    """A node's command line (``command.txt``) as arguments, its file names replaced."""
    out = []
    for a in shlex.split(command)[1:]:
        out.append(next((v for k, v in subs.items() if a == k or a.endswith("/" + k)), a))
    return out


# ------------------------------------------------------------------------------- the entry


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    run = common.fmriprep_run(RUN)
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    uni = e.input("BOLD reference, unifized (the node's input)", run / INPUT)
    e.input("fMRIPrep's mask (the node's output)", run / FMRIPREP_MASK)
    bold = e.input("BOLD", Path(td.get(BOLD)))
    call = fmriprep_args(
        command,
        {
            "uni_xform_masked.nii.gz": "{out}-masked.nii.gz",
            "uni_xform_mask.nii.gz": "{out}.nii.gz",
            "uni_xform.nii.gz": str(uni),
        },
    )
    if call[:4] != ["-apply_prefix", "{out}-masked.nii.gz", "-dilate", "1"]:
        raise AssertionError(f"{TOOL}: an unexpected fMRIPrep command: {command}")
    # fMRIPrep's call; AFNI's default on the raw series; fMRIPrep's call with -peels 0
    run_all(e, PROGRAM, "", call, [uni])
    run_all(e, PROGRAM, "bold-", ["-prefix", "{out}.nii.gz", str(bold)], [bold])
    peels0 = [*call[:4], "-peels", "0", *call[4:]]
    run_all(e, PROGRAM, "peels0-", peels0, [uni], runners=[RUNNERS[0], *RUNNERS[2:]])
    e.finish(
        versions=versions(PROGRAM),
        command=command,
        settings="fMRIPrep 25.2.6's 3dAutomask node (skullstrip_second_pass) of ds005040 sub-002 "
        "task-clip run-1, its arguments from command.txt; AFNI's default on the run's raw BOLD "
        "series; fMRIPrep's arguments with -peels 0 (AFNI-43). AFNI 25.2.09 (the repository's "
        "build and fMRIPrep's), the larmorx-gpl replica in both modes; 3dAutomask is "
        "single-threaded",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's second skull strip of the BOLD reference of ds005040 sub-002 task-clip "
        "run-1, the raw series' automask and -peels 0: AFNI, fMRIPrep's AFNI and the replica, "
        "default and reproduce",
        _compute,
    )


#: The clean-room original's runs: label suffix, D13 mode, threads.
ORIGINAL_RUNS = [
    ("original", "fix", "1"),
    ("original-reproduce", "reproduce", "1"),
    ("original-6", "fix", "6"),
]


def larmorx_version() -> str:
    import larmorx

    return f"larmorx {larmorx.__version__} (the clean-room original)"


def run_original(
    e: fcache.Entry, program: str, tag: str, args: list[str], runs=ORIGINAL_RUNS
) -> None:
    """``program`` with ``args`` (``{out}``: the run's output stem) by the clean-room original
    (``larmorx afni <program>``, ``LARMORX_IMPLEMENTATION=original``), in each D13 mode and
    thread count of ``runs``, into the entry's directory, in the environment of
    :func:`run_all`."""
    add, unset = environment()
    home = e.dir / ".home"
    home.mkdir(exist_ok=True)
    for label, mode, threads in runs:
        stem = str(e.dir / f"{tag}{label}")
        argv = [a.replace("{out}", stem) for a in args]
        env = {
            **add,
            "HOME": str(home),
            "LARMORX_IMPLEMENTATION": "original",
            "LARMORX_UPSTREAM_DEFECTS": mode,
            "OMP_NUM_THREADS": threads,
        }
        e.run(f"{tag}{label}", fcache.larmorx_cli("afni", program, *argv), env=env, unset=unset)


def _compute_original(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    run = common.fmriprep_run(RUN)
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    uni = e.input("BOLD reference, unifized (the node's input)", run / INPUT)
    bold = e.input("BOLD", Path(td.get(BOLD)))
    call = fmriprep_args(
        command,
        {
            "uni_xform_masked.nii.gz": "{out}-masked.nii.gz",
            "uni_xform_mask.nii.gz": "{out}.nii.gz",
            "uni_xform.nii.gz": str(uni),
        },
    )
    run_original(e, PROGRAM, "", call)
    run_original(e, PROGRAM, "bold-", ["-prefix", "{out}.nii.gz", str(bold)])
    peels0 = [*call[:4], "-peels", "0", *call[4:]]
    run_original(e, PROGRAM, "peels0-", peels0, runs=ORIGINAL_RUNS[:2])
    e.finish(
        versions={"larmorx": larmorx_version()},
        command=command,
        threads={"larmorx": ["1", "6"]},
        settings="the clean-room original (larmorx afni 3dAutomask, LARMORX_IMPLEMENTATION="
        "original) on the inputs of the replica's entry: fMRIPrep's call, AFNI's default on the "
        "raw series, -peels 0; default mode at 1 and 6 threads, reproduce at 1",
    )


def original_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"{CASE}-original",
        "the clean-room original on the replica's entry's calls: default (1 and 6 threads) and "
        "reproduce",
        _compute_original,
    )


# ----------------------------------------------------------------------------------- export

OUTPUTS = [
    {"id": "mask", "label": "Mask", "unit": ""},
    {"id": "masked", "label": "Masked input (-apply_prefix)", "unit": ""},
]
MASK_ONLY = OUTPUTS[:1]
PANELS = [
    ("afni", "afni", "original", "AFNI 25.2.09, 3dAutomask", "the repository's oracle build"),
    ("image", "image", "original", "AFNI 25.2.09 in fMRIPrep's image", "fMRIPrep 25.2.6's build"),
    ("replica", "replica", "larmorx", "larmorx-gpl replica", "default mode"),
    ("replica-reproduce", "replica-reproduce", "larmorx", "larmorx-gpl replica", "reproduce mode"),
]  # fmt: skip


def mask_view(ex, out: str = "mask") -> tuple[list[int], int]:
    """The default view of a mask: the axial slice with the most brain; the crosshair on the
    mask's edge there (a voxel of the mask beside one outside it), nearest the brain's
    centre."""
    from scipy import ndimage

    k = int(np.argmax(ex.m.sum(axis=(0, 1))))
    m = ex.values(ex.key("afni", out))[:, :, k] != 0
    edge = m & ~ndimage.binary_erosion(m)
    pts = np.argwhere(edge)
    if not len(pts):
        return common.centre_view(ex)
    c = np.argwhere(ex.m[:, :, k]).mean(0)
    i, j = pts[int(np.argmin(((pts - c) ** 2).sum(1)))]
    return [int(i), int(j), k], k


def diff_view(ex, a: str, b: str, out: str) -> tuple[list[int], int]:
    """The default view of a difference: the axial slice where A and B differ in the most
    voxels; the crosshair on the differing voxel nearest the brain's centre in it (the mask's
    view where they agree)."""
    va, vb = ex.values(ex.key(a, out)), ex.values(ex.key(b, out))
    differ = (va != vb) & ex.m
    if not differ.any():
        return mask_view(ex, out)
    k = int(np.argmax(differ.sum(axis=(0, 1))))
    pts = np.argwhere(differ[:, :, k])
    c = np.argwhere(ex.m[:, :, k]).mean(0)
    i, j = pts[int(np.argmin(((pts - c) ** 2).sum(1)))]
    return [int(i), int(j), k], k


def window_of(ex, pid: str) -> list[float]:
    from larmorx_validation.evidence import example as exm

    v = ex.values(ex.key(pid, "all"))
    m = ex.m if v.ndim == 3 else np.broadcast_to(ex.m[..., None], v.shape)
    w = exm.windows(v[m])[0]
    return [w["lo"], w["hi"]]


#: The clean-room original's panels (and its run at 6 threads, compared but not shown).
ORIGINAL_PANELS = [
    ("original", "original", "larmorx", "LARMORX clean-room original", "default mode, 1 thread"),
    ("original-reproduce", "original-reproduce", "larmorx", "LARMORX clean-room original",
     "reproduce mode"),
]  # fmt: skip
ORIGINAL_6 = [("original-6", "original-6", "larmorx", "LARMORX clean-room original", "6 threads")]


def runs(
    e: fcache.Entry, tag: str, outputs: list[dict], panels=PANELS
) -> dict[str, dict[str, np.ndarray]]:
    out = {}
    for pid, stem, *_ in panels:
        f = e.dir / f"{tag}{stem}.nii.gz"
        if not f.is_file():
            continue
        arrays = {"mask": common.load(str(f))}
        if any(o["id"] == "masked" for o in outputs):
            arrays["masked"] = common.load(str(e.dir / f"{tag}{stem}-masked.nii.gz"))
        out[pid] = arrays
    return out


def original_provenance(o: fcache.Entry) -> str:
    info = o.info()
    return (
        f"Runs of the clean-room original: figures cache entry `{o.tool}/{o.case}`, computed "
        f"{info['computed']} with larmorx `{info['larmorx_commit'][:12]}` (default mode at 1 and "
        "6 threads, reproduce mode at 1)"
    )


def original_diffs(prefix: str = "") -> list[dict]:
    """The clean-room original's comparisons: with AFNI, with the replica, with itself at 6
    threads."""
    d = common.d
    return [
        d("original-afni", "original", "afni", "main", f"original − AFNI 25.2.09{prefix}",
          common.ROLE_LX),
        d("original-replica", "original", "replica", "check",
          "original − replica (both default mode)", common.ROLE_SELF),
        d("original-threads", "original-6", "original", "check",
          "original, 6 threads − 1 thread", common.ROLE_SELF),
    ]  # fmt: skip


def original_verdicts(diffs: list[dict], same: dict[str, bool]) -> None:
    for x in diffs:
        if x["id"] == "original-threads" and same["original-threads"]:
            x["verdict"] = "bit-identical"
            x["reason"] = (
                "The original's result does not depend on its thread count: the same bytes at 1 "
                "and 6 threads."
            )
        elif x["id"] == "original-repro" and same["original-repro"]:
            x["verdict"] = "bit-identical"
            x["reason"] = (
                "No correction of the register applies to this input: both modes give the same "
                "bytes."
            )
        elif x["id"] == "original-replica" and same["original-replica"]:
            x["verdict"] = "bit-identical"
            x["reason"] = (
                "The two implementations, written independently (one translated from AFNI's "
                "source, one from a behavioural specification), give the same bytes."
            )


def _example_ref(ctx: core.Context, e: fcache.Entry, o: fcache.Entry) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    run = common.fmriprep_run(RUN)
    img = nib.load(str(run / INPUT))
    arrays = runs(e, "", OUTPUTS)
    arrays.update(runs(o, "", OUTPUTS, ORIGINAL_PANELS + ORIGINAL_6))
    node = common.load(str(run / FMRIPREP_MASK))
    masks = [a["mask"] for a in arrays.values()]
    show = ndimage.binary_dilation(
        np.logical_or.reduce([m != 0 for m in [*masks, node]]), iterations=2
    )
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("uni", "input", "ds005040 sub-002 BOLD reference, unifized",
                     "3dAutomask's input in fMRIPrep's BOLD reference workflow (float32)",
                     {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    over = {"mask": {"on": "uni", "window": window_of(ex, "uni")}}
    for pid, _stem, role, label, sub in PANELS + ORIGINAL_PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    d = common.d
    diffs = [
        *original_diffs()[:1],
        d("replica-afni", "replica", "afni", "main", "replica − AFNI 25.2.09", common.ROLE_LX),
        d("image-afni", "image", "afni", "variability",
          "fMRIPrep's AFNI − the repository's AFNI build", common.ROLE_ORIG),
        d("original-image", "original", "image", "fmriprep", "original − fMRIPrep's AFNI",
          common.ROLE_FMRIPREP),
        d("replica-image", "replica", "image", "fmriprep", "replica − fMRIPrep's AFNI",
          common.ROLE_FMRIPREP),
        *original_diffs()[1:],
        d("original-repro", "original", "original-reproduce", "check",
          "original, default − reproduce mode", common.ROLE_SELF),
        d("replica-repro", "replica", "replica-reproduce", "check",
          "replica, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    common.compare_all(ex, diffs, OUTPUTS, full={"original-6": arrays["original-6"]})
    same = common.identical(diffs)
    for x in diffs:
        _verdict(x, same)
    original_verdicts(diffs, same)
    node_same = bool(
        np.array_equal(node, arrays["image"]["mask"])
        and node.dtype == arrays["image"]["mask"].dtype
    )
    at, _k = mask_view(ex)
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["uni", "afni", "original", "replica"],
        "diffs": ["original-afni", "replica-afni", "image-afni"],
        "compare": {"a": "afni", "b": "original"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(OUTPUTS), default=default, window_panel="afni",
                        caption="the crosshair on the mask's edge.")  # fmt: skip
    vers = info.get("versions", {})
    a = arrays["afni"]["mask"]
    m = core.compare(arrays["replica"]["mask"], a)
    mo = core.compare(arrays["original"]["mask"], a)
    oinfo = o.info()
    return {
        "id": "fmriprep-automask",
        "title": "fMRIPrep's second skull strip of a real BOLD reference",
        "summary": (
            "The 3dAutomask node fMRIPrep 25.2.6 ran on the unifized BOLD reference of OpenNeuro "
            "ds005040 sub-002 (task clip, run 1): its arguments given to AFNI 25.2.09, to "
            "fMRIPrep's own build of AFNI in its image, to the larmorx-gpl replica and to "
            "LARMORX's clean-room original, each in its default and reproduce modes. The masks "
            "are drawn over the input; choose *Masked input* for -apply_prefix's output."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{DATA['dataset']}, {DATA['subject']} ({DATA['licence']}): the BOLD "
                "reference fMRIPrep computed, N4-corrected, skull-stripped once and unifized "
                "(3dUnifize) by fMRIPrep; shown around the masks, dilated by 2 voxels",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `bold_fit_wf/enhance_and_skullstrip_bold_wf/"
                "skullstrip_second_pass`",
            },
            {"label": "Command", "text": "`" + short_command(info["command"]) + "`"},
            {
                "label": "Programs",
                "text": f"{vers.get('afni', '?')} (the repository's oracle build); AFNI "
                f"{vers.get('afni_in_fmriprep_image', '?')} in fMRIPrep's image; "
                f"{vers.get('larmorx-gpl', '?')} (the replica), from larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`; "
                f"{oinfo.get('versions', {}).get('larmorx', 'larmorx')} at commit "
                f"`{oinfo['larmorx_commit'][:12]}`",
            },
            {
                "label": "Mask",
                "text": f"{int(np.count_nonzero(a)):,} of {a.size:,} voxels; the replica's "
                + ("the same, voxel for voxel" if m["identical"] else f"differs in {m['differ']:,}")
                + "; the original's "
                + (
                    "the same, voxel for voxel"
                    if mo["identical"]
                    else f"differs in {mo['differ']:,}"
                ),
            },
            {
                "label": "fMRIPrep's run",
                "text": "the mask its node wrote has the same bytes as its AFNI's run here"
                if node_same
                else "the mask its node wrote differs from its AFNI's run here",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [DATA], [original_provenance(o)]),
    }


def _example_bold(ctx: core.Context, e: fcache.Entry, o: fcache.Entry) -> dict:
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    img = nib.load(td.get(BOLD))
    arrays = runs(e, "bold-", MASK_ONLY)
    arrays.update(runs(o, "bold-", MASK_ONLY, ORIGINAL_PANELS + ORIGINAL_6))
    show = ndimage.binary_dilation(
        np.logical_or.reduce([a["mask"] != 0 for a in arrays.values()]), iterations=2
    )
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    raw = np.asanyarray(img.dataobj, dtype=np.float64)
    mean = np.abs(raw).mean(axis=3).astype(np.float32)
    ex.add(exm.Panel("bold-mean", "input", "ds005040 sub-002 BOLD, task clip run 1",
                     f"the mean of |value| over its {img.shape[3]} volumes (computed for this "
                     "page in float64, shown as float32): what 3dAutomask masks",
                     {"all": mean}))  # fmt: skip
    over = {"mask": {"on": "bold-mean", "window": window_of(ex, "bold-mean")}}
    for pid, _stem, role, label, sub in PANELS + ORIGINAL_PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    d = common.d
    diffs = [
        *original_diffs()[:1],
        d("replica-afni", "replica", "afni", "main", "replica − AFNI 25.2.09", common.ROLE_LX),
        d("image-afni", "image", "afni", "variability",
          "fMRIPrep's AFNI − the repository's AFNI build", common.ROLE_ORIG),
        *original_diffs()[1:],
        d("original-repro", "original", "original-reproduce", "check",
          "original, default − reproduce mode", common.ROLE_SELF),
        d("replica-repro", "replica", "replica-reproduce", "check",
          "replica, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    common.compare_all(ex, diffs, MASK_ONLY, full={"original-6": arrays["original-6"]})
    same = common.identical(diffs)
    for x in diffs:
        _verdict(x, same)
    original_verdicts(diffs, same)
    at, _k = mask_view(ex)
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["bold-mean", "afni", "original", "replica"],
        "diffs": ["original-afni", "replica-afni", "image-afni"],
        "compare": {"a": "afni", "b": "original"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(MASK_ONLY), default=default, window_panel="afni",
                        caption="the crosshair on the mask's edge.")  # fmt: skip
    vers = info.get("versions", {})
    a = arrays["afni"]["mask"]
    return {
        "id": "bold-automask",
        "title": "AFNI's default on the raw BOLD series",
        "summary": (
            "3dAutomask's usual use: a brain mask of an EPI series. With no options it averages "
            f"the absolute values of the {img.shape[3]} volumes, then masks the average. The "
            "same run as above, before fMRIPrep touched it, given to AFNI 25.2.09, fMRIPrep's "
            "AFNI, the replica and the clean-room original in both modes."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{DATA['dataset']}, {DATA['subject']} ({DATA['licence']}): "
                f"{' × '.join(map(str, img.shape))} int16 values × scale factor; shown around "
                "the masks, dilated by 2 voxels",
            },
            {"label": "Command", "text": f"`3dAutomask -prefix mask.nii.gz {Path(BOLD).name}`"},
            {
                "label": "Programs",
                "text": f"{vers.get('afni', '?')}; AFNI {vers.get('afni_in_fmriprep_image', '?')} "
                f"in fMRIPrep's image; {vers.get('larmorx-gpl', '?')}; "
                f"{o.info().get('versions', {}).get('larmorx', 'larmorx')}",
            },
            {"label": "Mask", "text": f"{int(np.count_nonzero(a)):,} of {a.size:,} voxels"},
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [DATA], [original_provenance(o)]),
    }


def _example_peels(ctx: core.Context, e: fcache.Entry, o: fcache.Entry, reg: dict) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    run = common.fmriprep_run(RUN)
    img = nib.load(str(run / INPUT))
    arrays = runs(e, "peels0-", OUTPUTS)
    arrays.update(runs(o, "peels0-", OUTPUTS, ORIGINAL_PANELS))
    arrays["afni-peels1"] = runs(e, "", OUTPUTS)["afni"]
    show = ndimage.binary_dilation(
        np.logical_or.reduce([a["mask"] != 0 for a in arrays.values()]), iterations=2
    )
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("uni", "input", "ds005040 sub-002 BOLD reference, unifized",
                     "as in the first example", {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    over = {"mask": {"on": "uni", "window": window_of(ex, "uni")}}
    panels = [
        ("afni", "original", "AFNI 25.2.09, -peels 0", "the repository's oracle build"),
        ("replica", "larmorx", "larmorx-gpl replica, -peels 0", "default mode: corrected"),
        ("replica-reproduce", "larmorx", "larmorx-gpl replica, -peels 0", "reproduce mode"),
        ("original", "larmorx", "LARMORX clean-room original, -peels 0", "default mode: corrected"),
        ("original-reproduce", "larmorx", "LARMORX clean-room original, -peels 0",
         "reproduce mode"),
        ("afni-peels1", "original", "AFNI 25.2.09, fMRIPrep's call", "-peels 1, the default"),
    ]  # fmt: skip
    for pid, role, label, sub in panels:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    d = common.d
    diffs = [
        d("replica-afni", "replica", "afni", "main", "replica (default) − AFNI, -peels 0",
          common.ROLE_LX),
        d("repro-afni", "replica-reproduce", "afni", "main", "replica (reproduce) − AFNI, -peels 0",
          common.ROLE_LX),
        d("original-afni", "original", "afni", "main", "original (default) − AFNI, -peels 0",
          common.ROLE_LX),
        d("original-repro-afni", "original-reproduce", "afni", "main",
          "original (reproduce) − AFNI, -peels 0", common.ROLE_LX),
        d("afni-peels", "afni", "afni-peels1", "variability", "AFNI: -peels 0 − -peels 1",
          "the defect, in AFNI"),
    ]  # fmt: skip
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    link = [_link(reg, "AFNI-43")]
    for x in diffs:
        i = x["id"]
        x["links"] = link
        if i == "afni-peels":
            x["verdict"] = None
            x["reason"] = (
                "AFNI's -peels 0 gives its -peels 1 result, every voxel: it peels once whatever "
                "0 says (AFNI-43)."
                if same[i]
                else "AFNI's -peels 0 and -peels 1 differ on this input."
            )
        elif i in ("replica-afni", "original-afni"):
            repro = "repro-afni" if i == "replica-afni" else "original-repro-afni"
            x["verdict"] = "d13" if not same[i] and same[repro] else "open"
            x["reason"] = (
                "AFNI-43, corrected by default with a warning: 3dAutomask's help says -peels 0 "
                "turns the peeling off, and the default does not peel; reproduce mode peels once, "
                "as AFNI does."
                if not same[i]
                else "No voxel changes: the peel removes nothing from this mask."
            )
            if same[i]:
                x["verdict"] = "bit-identical"
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = "reproduce mode gives AFNI's result, defect included."
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example."
    at, _k = diff_view(ex, "original", "afni", "mask")
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["uni", "afni", "original", "original-reproduce"],
        "diffs": ["original-afni", "original-repro-afni", "afni-peels"],
        "compare": {"a": "afni", "b": "original"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(OUTPUTS), default=default, window_panel="afni",
                        caption="the crosshair where the corrected mask differs from AFNI's.")  # fmt: skip
    m = next(x for x in diffs if x["id"] == "original-afni")["metrics"]["mask"]
    a43 = reg.get("AFNI-43", {})
    return {
        "id": "afni-43",
        "title": "AFNI-43: -peels 0 peels once",
        "summary": (
            "3dAutomask's help: “-peels pp = Peel (erode) the mask 'pp' times, then unpeel "
            "(dilate). Using NN2 neighborhoods, clips off protuberances less than 2*pp voxels "
            "thick. Turn off by setting to 0.” AFNI peels once for 0. fMRIPrep's call with "
            "-peels 0 added, given to AFNI 25.2.09, to the replica and to the clean-room "
            "original in both modes, beside fMRIPrep's own call."
        ),
        "facts": [
            {"label": "Input", "text": "the unifized BOLD reference of the first example"},
            {
                "label": "Command",
                "text": "`3dAutomask -apply_prefix masked.nii.gz -dilate 1 -peels 0 -prefix "
                "mask.nii.gz uni_xform.nii.gz`",
            },
            {
                "label": "Defect",
                "text": f"[AFNI-43](repo:{_link(reg, 'AFNI-43')['href']}): "
                + (a43.get("title") or "-peels 0 peels once")
                + "; corrected by default, with a warning",
            },
            {
                "label": "Effect",
                "text": f"the corrected mask differs from AFNI's in {m['differ']:,} of "
                f"{m['voxels']:,} voxels",
            },
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [DATA], [original_provenance(o)]),
    }


def _link(reg: dict, rid: str) -> dict:
    """A register entry's link; the register itself until the entry is in it."""
    r = reg.get(rid)
    return {"label": rid, "href": r["link"] if r else "docs/findings/upstream-defects.md"}


def _verdict(x: dict, same: dict[str, bool]) -> None:
    i = x["id"]
    x.setdefault("links", [])
    if x["kind"] == "variability":
        x["verdict"] = None
        x["reason"] = (
            "Two builds of AFNI 25.2.09 (fMRIPrep's image and the repository's) give the same "
            "bytes: AFNI's own variability on this input is zero."
            if same[i]
            else "AFNI's two builds differ on this input."
        )
    elif same[i]:
        x["verdict"] = "bit-identical"
        x["reason"] = {
            "replica-repro": "No correction of the register applies to this input: both modes "
            "give the same bytes.",
        }.get(i, "The same values, every one.")
    else:
        x["verdict"] = "open"
        x["reason"] = "Not accounted for by the other runs of this example."


def _note(img) -> str:
    n = int(np.prod(img.shape[:3]))
    return (
        f"Δ is A − B: for the masks +1 where only A holds a voxel, −1 where only B does. "
        f"Computed in float64 over all {n:,} voxels of the outputs, not only those shown. The "
        "record's threshold: every output bit-identical to AFNI's, with its exit status and "
        "messages."
    )


# ------------------------------------------------------------------------------------ cases


def input_name(name: str) -> str:
    """A case's input, as a reader needs it."""
    kind, _, rest = name.partition(":")
    if kind == "td":
        return core.short_path(rest).removeprefix("openneuro/")
    if kind == "fmriprep":
        label, _, what = rest.rpartition(":")
        return {
            "automask": f"<fMRIPrep {label}: unifized BOLD reference>",
            "unifize": f"<fMRIPrep {label}: masked BOLD reference>",
        }.get(what, f"<fMRIPrep {label}: {what}>")
    if kind == "fmriprep-anat":
        return f"<{rest.split(':')[0]}: sMRIPrep's T1w × its brain mask>"
    if kind == "blank":
        return f"<generated: {rest}, in a directory with a blank>"
    return f"<generated: {name}>"


def command(program: str, s) -> str:
    """A case's command line: its inputs named as placeholders (``<...>``, not quoted), the
    other arguments quoted where a shell needs it, the environment and the files present."""
    import re
    import shlex

    names = [input_name(n) for n in s.inputs]
    parts = [program]
    for a in s.args:
        m = re.fullmatch(r"\{(\d+)\}", a)
        if m:
            parts.append(names[int(m.group(1))])
        else:
            parts.append(shlex.quote(a) if re.search(r"\s", a) else a)
    env = " ".join(f"{k}={v}" for k, v in s.env if v is not None)
    files = " ".join(f"(with {n} present)" for n, _ in s.files)
    return " ".join(x for x in (env, " ".join(parts), files) if x)


#: The register's AFNI entries the replicas correct by default (the others are kept as AFNI's
#: until decided, or refused in both modes).
CORRECTED = frozenset(
    f"AFNI-{n:02d}" for n in (1, 2, 7, 8, 11, 12, 16, 18, 20, 22, 23, 26, 43, 45, 47)
)


def case_verdict(r: dict, desc: str = "", who: str = "replica") -> tuple[str, str]:
    """A case's verdict: the record compares ``who`` (the replica or the clean-room original)
    in reproduce mode with AFNI (every output byte for byte, or the original's float32 values
    within the threshold; exit status, messages); the fix pass and AFNI's variability are
    reported. A compared case without outputs (``desc`` names its defect) is AFNI's refusal
    or AFNI's silence, reproduced."""
    import re

    st = r["status"]
    if st == "both-error" and "crash" in r["reason"]:
        return "divergence", common.clip(r["reason"])
    v = common.outcome_verdict(r)
    if v is not None:
        return v
    measured = r.get("measured", {})
    if not any(k.endswith(": data") for k in measured) and "exit status" in measured:
        m = re.search(r"\((AFNI-\d+)\)", desc)
        defect = m.group(1) if m else "the defect"
        if measured["exit status"]:
            return "both-reject", (
                f"AFNI refuses it (exit {measured['exit status']}), and so does the {who} in "
                f"reproduce mode, with AFNI's messages; its default mode accepts it and names "
                f"{defect} (the record's D13 section)."
            )
        return "bit-identical", (
            f"Neither writes an output; the exit status (0) and the messages are AFNI's. The "
            f"default mode refuses instead (exit 1) and names {defect} (the record's D13 section)."
        )
    rep = r.get("reported", {})
    notes = []
    verdict = "bit-identical"
    from larmorx_validation.parity import afni_replica as ar

    headers = [k.split(":")[0] for k, v in measured.items() if v == ar.AFNI_10]
    if headers:
        verdict = "d13"
        notes.append(
            f"{', '.join(headers)}: the data are AFNI's, bit for bit; the header differs where "
            "AFNI reads the geometry from its own header extension of this AFNI-written input "
            "(7 significant digits; AFNI-10, which the clean-room original ignores, as 3dTshift's "
            "does): AFNI run on the input without its extension writes the original's header byte "
            "for byte"
        )
    differ = {
        k.split(":")[0]: v
        for k, v in rep.items()
        if k.endswith(": values bit-identical to AFNI's") and v
    }
    if differ:
        cr = rep.get(ar.CORRECTLY_ROUNDED)
        if cr == 0:
            verdict = "identical-cr"
        elif verdict == "bit-identical":
            verdict = "open"
        notes.append(
            ", ".join(
                f"{k}: {v:,} values differ from AFNI's in their last bits"
                for k, v in differ.items()
            )
            + " (AFNI's squash calls glibc's tanhf, which is not correctly rounded on some "
            "arguments; the original's is correctly rounded)"
            + (
                "; with CORE-MATH's correctly rounded functions preloaded into AFNI, every value "
                "is the original's"
                if cr == 0
                else f"; AFNI with correctly rounded functions: {cr} values still differ"
                if cr is not None
                else ""
            )
        )
    band = {k.removeprefix("band: "): v for k, v in rep.items() if k.startswith("band: ")}
    if band:
        same = [k for k, v in band.items() if v == 0]
        other = [f"{k}: {v:,} values differ" for k, v in band.items() if v]
        threads = [
            k.removeprefix("AFNI at ").removesuffix(" thread(s)")
            for k in same
            if k.startswith("AFNI at ")
        ]
        builds = [k for k in same if not k.startswith("AFNI at ")]
        parts = (
            [f"AFNI at {' and '.join(threads)} thread{'s' if threads != ['1'] else ''}"]
            if threads
            else []
        ) + builds
        if parts:
            notes.append(" and ".join(parts) + " give the oracle's bytes")
        notes += other
    own = {k: v for k, v in {**measured, **rep}.items() if k.endswith(ar.FMRIPREP_OWN)}
    for k, v in own.items():
        name = k.split(":")[0]
        oracle = rep.get(f"{name}: {ar.FMRIPREP_AFNI}")
        notes.append(
            f"{name}: fMRIPrep's own output, "
            + (
                "the same bytes"
                if not v
                else f"{v:,} values differ"
                + (
                    " (fMRIPrep's output is AFNI's oracle output bit for bit: the same last-bit "
                    "differences)"
                    if oracle == 0
                    else ""
                )
            )
        )
    fix = rep.get("D13 fix pass")
    named = [d for d in re.findall(r"\b(AFNI-\d+)\b", desc) if d in CORRECTED]
    if fix or named:
        what = ", ".join(named) if named else "a defect of the register"
        notes.append(
            f"in its default mode the {who} corrects {what} and names it: "
            + (f"{fix:,} values change" if fix else "the header or the messages change")
            + " (the record's D13 section)"
        )
    return verdict, common.clip("; ".join(notes), 600)


def cases(recs: dict, scenarios, program: str, examples: dict[str, str]) -> list[dict]:
    """Every case of the records ``recs`` (implementation id -> record), each with every
    implementation's result."""
    defs = {cid: (desc, s) for cid, _cat, desc, s in scenarios}
    by_case: dict[str, dict] = {}
    for impl, rec in recs.items():
        who = "replica" if impl == "replica" else "original"
        for r in rec.results:
            desc, s = defs.get(r["case"], ("", None))
            v, why = case_verdict(r, desc, who)
            c = by_case.setdefault(
                r["case"],
                {
                    "id": r["case"],
                    "category": r["category"],
                    "description": desc,
                    "command": command(program, s) if s else "",
                    "results": {},
                    **({"example": examples[r["case"]]} if r["case"] in examples else {}),
                },
            )
            c["results"][impl] = {
                "outcome": r["status"],
                "verdict": v,
                "checks": f"{r['checks_passed']}/{r['checks_total']}",
                "reason": why,
            }
    return list(by_case.values())


#: The implementations of the two tools' pages: id, label, record suffix.
IMPLEMENTATIONS = [
    ("original", "LARMORX clean-room original", ""),
    ("replica", "larmorx-gpl replica", "-replica"),
]


def implementations(cs: list[dict], tool: str, status: dict[str, str]) -> list[dict]:
    return [
        common.implementation(
            cs, impl, label, impl, f"docs/validation/{tool}{suffix}.md", status[impl]
        )
        for impl, label, suffix in IMPLEMENTATIONS
    ]


#: The informal timings of both tools (validation/investigations/afni-masks-original/timing.py).
TIMINGS = "validation/investigations/afni-masks-original/timings-2026-10-24.json"

#: The series of the speed chart: the timing's column, its label and the site's mark.
SPEED_SERIES = [
    ("AFNI 1", "AFNI 25.2.09, 1 thread", "original"),
    ("AFNI 6", "AFNI 25.2.09, 6 threads", "original-open"),
    ("replica 1", "replica, 1 thread", "replica"),
    ("replica 6", "replica, 6 threads", "replica-open"),
    ("original 1", "clean-room original, 1 thread", "larmorx"),
    ("original 6", "clean-room original, 6 threads", "larmorx-open"),
]


def timings_source(repo: Path) -> list[dict]:
    """How the provenance names the informal timings (as a benchmark report's source)."""
    import hashlib

    path = repo / TIMINGS
    if not path.is_file():
        return []
    return [{"path": TIMINGS, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}]


def informal_speed(repo: Path, program: str) -> dict | None:
    """The speed chart's data from the informal timings of the clean-room originals' part
    (not a benchmark report): rows (inputs) × series of wall seconds, the speed-up AFNI ÷ the
    original at one thread each."""
    import json

    path = repo / TIMINGS
    if not path.is_file():
        return None
    data = json.loads(path.read_text(encoding="utf-8"))
    series = [{"id": i, "label": label, "mark": mark} for i, label, mark in SPEED_SERIES]
    rows = []
    for name, seconds in data["runs"].items():
        if not name.startswith(program + ", "):
            continue
        s = {i: float(seconds[i]) for i, _, _ in SPEED_SERIES if i in seconds}
        a, b = s.get("AFNI 1"), s.get("original 1")
        rows.append(
            {
                "label": name.removeprefix(program + ", "),
                "input": name,
                "seconds": s,
                "speedup": a / b if a and b else None,
            }
        )
    if not rows:
        return None
    present = {i for r in rows for i in r["seconds"]}
    axis, ticks = common.ticks_log([v for r in rows for v in r["seconds"].values()], " s")
    load = data.get("machine", {}).get("load", [])
    return {
        "title": f"{program}: wall time of the whole program (informal timings, 2026-10-24)",
        "axis": axis,
        "ticks": ticks,
        "series": [x for x in series if x["id"] in present],
        "rows": rows,
        "note": (
            "Each program run once (larmorx after one warm-up run), reading and writing "
            f"compressed NIfTI included, on a {data.get('machine', {}).get('nproc', '?')}-CPU "
            "machine shared with another job throughout (load "
            + (f"{min(load):.0f} to {max(load):.0f}" if load else "?")
            + "): indications, not a benchmark report. The ratio is AFNI's time over the "
            "clean-room original's, at one thread each. Tool: "
            "validation/investigations/afni-masks-original/timing.py."
        ),
    }


def status_of(rec) -> str:
    """``validated`` when no case of the record fails, else ``experimental``."""
    return "experimental" if rec.data.get("summary", {}).get("fail") else "validated"


def export(ctx: core.Context) -> dict:
    from larmorx_validation.parity.afni_automask import _scenario_list

    reg = core.register(ctx.repo)
    recs = {
        impl: records.load(ctx.repo, f"{TOOL}{suffix}") for impl, _label, suffix in IMPLEMENTATIONS
    }
    examples = {"fmriprep/ds005040-task_clip_run_1": "fmriprep-automask"}
    cs = cases(recs, _scenario_list(), PROGRAM, examples)
    e = entry(ctx)
    o = original_entry(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "3dAutomask",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": [x for rec in recs.values() for x in rec.source()],
            "benchmarks": timings_source(ctx.repo),
            "cache": [fcache.provenance(e), fcache.provenance(o)],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": implementations(
            cs, TOOL, {impl: status_of(rec) for impl, rec in recs.items()}
        ),
        "cases": cs,
        "cases_total": len(cs),
        "speed": informal_speed(ctx.repo, PROGRAM),
        "examples": [
            _example_ref(ctx, e, o),
            _example_bold(ctx, e, o),
            _example_peels(ctx, e, o, reg),
        ],
    }


if __name__ == "__main__":  # compute the cache entry only
    from larmorx_validation.figures.core import REPO, default_cache

    print("done", entry(core.Context(repo=REPO, site=Path("."), cache=default_cache())).dir)
