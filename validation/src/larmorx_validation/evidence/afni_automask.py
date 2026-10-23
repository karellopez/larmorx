# SPDX-License-Identifier: Apache-2.0
"""3dAutomask's evidence: fMRIPrep's call on a real BOLD reference (its second skull strip) by
AFNI 25.2.09, by fMRIPrep's own AFNI and by the larmorx-gpl replica in its default and
``reproduce`` modes; AFNI's default on the raw BOLD series it came from (the functional path:
the series averaged first); a deliberate correction (AFNI-43, ``-peels 0``) on the same
reference; the record's cases.

The replica (``larmorx-gpl``, GPL-3.0-or-later) runs as a separate program, as everywhere in
the validation package (Apache-2.0). There is no clean-room original yet (its specification is
``specs/3dAutomask.md``). The masks are drawn over the image they were made from.

The helpers that run AFNI, fMRIPrep's AFNI and the replica are shared with 3dUnifize's evidence
(:mod:`larmorx_validation.evidence.afni_unifize`).
"""

from __future__ import annotations

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
    """The version banner of ``program`` in fMRIPrep's image (its first line)."""
    import subprocess

    _, _, sif = binaries(program)
    p = subprocess.run(
        ["apptainer", "exec", "--cleanenv", "--no-home", str(sif), program],
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


def runs(e: fcache.Entry, tag: str, outputs: list[dict]) -> dict[str, dict[str, np.ndarray]]:
    out = {}
    for pid, stem, *_ in PANELS:
        f = e.dir / f"{tag}{stem}.nii.gz"
        if not f.is_file():
            continue
        arrays = {"mask": common.load(str(f))}
        if any(o["id"] == "masked" for o in outputs):
            arrays["masked"] = common.load(str(e.dir / f"{tag}{stem}-masked.nii.gz"))
        out[pid] = arrays
    return out


def _example_ref(ctx: core.Context, e: fcache.Entry) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    run = common.fmriprep_run(RUN)
    img = nib.load(str(run / INPUT))
    arrays = runs(e, "", OUTPUTS)
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
    for pid, _stem, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    d = common.d
    diffs = [
        d("replica-afni", "replica", "afni", "main", "replica − AFNI 25.2.09", common.ROLE_LX),
        d("image-afni", "image", "afni", "variability",
          "fMRIPrep's AFNI − the repository's AFNI build", common.ROLE_ORIG),
        d("replica-image", "replica", "image", "fmriprep", "replica − fMRIPrep's AFNI",
          common.ROLE_FMRIPREP),
        d("replica-repro", "replica", "replica-reproduce", "check",
          "replica, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    for x in diffs:
        _verdict(x, same)
    node_same = bool(
        np.array_equal(node, arrays["image"]["mask"])
        and node.dtype == arrays["image"]["mask"].dtype
    )
    at, _k = mask_view(ex)
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["uni", "afni", "image", "replica"],
        "diffs": ["replica-afni", "image-afni"],
        "compare": {"a": "afni", "b": "replica"},
    }
    out = common.finish(ctx, ex, diffs, OUTPUTS, default=default, window_panel="afni",
                        caption="the crosshair on the mask's edge.")  # fmt: skip
    vers = info.get("versions", {})
    a = arrays["afni"]["mask"]
    m = core.compare(arrays["replica"]["mask"], a)
    return {
        "id": "fmriprep-automask",
        "title": "fMRIPrep's second skull strip of a real BOLD reference",
        "summary": (
            "The 3dAutomask node fMRIPrep 25.2.6 ran on the unifized BOLD reference of OpenNeuro "
            "ds005040 sub-002 (task clip, run 1): its arguments given to AFNI 25.2.09, to "
            "fMRIPrep's own build of AFNI in its image, and to the larmorx-gpl replica in its "
            "default and reproduce modes. The masks are drawn over the input; choose *Masked "
            "input* for -apply_prefix's output."
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
            {"label": "Command", "text": "`" + core.short_path(info["command"]) + "`"},
            {
                "label": "Programs",
                "text": f"{vers.get('afni', '?')} (the repository's oracle build); AFNI "
                f"{vers.get('afni_in_fmriprep_image', '?')} in fMRIPrep's image; "
                f"{vers.get('larmorx-gpl', '?')} (the replica), from larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`",
            },
            {
                "label": "Mask",
                "text": f"{int(np.count_nonzero(a)):,} of {a.size:,} voxels; the replica's "
                + (
                    "the same, voxel for voxel" if m["identical"] else f"differs in {m['differ']:,}"
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
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [DATA]),
    }


def _example_bold(ctx: core.Context, e: fcache.Entry) -> dict:
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    img = nib.load(td.get(BOLD))
    arrays = runs(e, "bold-", MASK_ONLY)
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
    for pid, _stem, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    d = common.d
    diffs = [
        d("replica-afni", "replica", "afni", "main", "replica − AFNI 25.2.09", common.ROLE_LX),
        d("image-afni", "image", "afni", "variability",
          "fMRIPrep's AFNI − the repository's AFNI build", common.ROLE_ORIG),
        d("replica-repro", "replica", "replica-reproduce", "check",
          "replica, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    common.compare_all(ex, diffs, MASK_ONLY)
    same = common.identical(diffs)
    for x in diffs:
        _verdict(x, same)
    at, _k = mask_view(ex)
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["bold-mean", "afni", "image", "replica"],
        "diffs": ["replica-afni", "image-afni"],
        "compare": {"a": "afni", "b": "replica"},
    }
    out = common.finish(ctx, ex, diffs, MASK_ONLY, default=default, window_panel="afni",
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
            "AFNI and the replica in both modes."
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
                f"in fMRIPrep's image; {vers.get('larmorx-gpl', '?')}",
            },
            {"label": "Mask", "text": f"{int(np.count_nonzero(a)):,} of {a.size:,} voxels"},
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "metrics_note": _note(img),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [DATA]),
    }


def _example_peels(ctx: core.Context, e: fcache.Entry, reg: dict) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    run = common.fmriprep_run(RUN)
    img = nib.load(str(run / INPUT))
    arrays = runs(e, "peels0-", OUTPUTS)
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
        ("afni-peels1", "original", "AFNI 25.2.09, fMRIPrep's call", "-peels 1, the default"),
    ]
    for pid, role, label, sub in panels:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    d = common.d
    diffs = [
        d("replica-afni", "replica", "afni", "main", "replica (default) − AFNI, -peels 0",
          common.ROLE_LX),
        d("repro-afni", "replica-reproduce", "afni", "main", "replica (reproduce) − AFNI, -peels 0",
          common.ROLE_LX),
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
        elif i == "replica-afni":
            x["verdict"] = "d13" if not same[i] and same["repro-afni"] else "open"
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
    at, _k = diff_view(ex, "replica", "afni", "mask")
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["uni", "afni", "replica", "replica-reproduce"],
        "diffs": ["replica-afni", "repro-afni", "afni-peels"],
        "compare": {"a": "afni", "b": "replica"},
    }
    out = common.finish(ctx, ex, diffs, OUTPUTS, default=default, window_panel="afni",
                        caption="the crosshair where the corrected mask differs from AFNI's.")  # fmt: skip
    m = next(x for x in diffs if x["id"] == "replica-afni")["metrics"]["mask"]
    a43 = reg.get("AFNI-43", {})
    return {
        "id": "afni-43",
        "title": "AFNI-43: -peels 0 peels once",
        "summary": (
            "3dAutomask's help: “-peels p = Peel the mask p times, then unpeel. Designed to "
            "clip off protuberances less than 2·p voxels thick. Turn off by setting to 0.” AFNI "
            "peels once for p = 0. fMRIPrep's call with -peels 0 added, given to AFNI 25.2.09 "
            "and to the replica in both modes, beside fMRIPrep's own call."
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
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [DATA]),
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
        return f"'<generated: {rest}, in a directory with a blank>'"
    return f"<generated: {name}>"


def command(program: str, s) -> str:
    import re

    names = [input_name(n) for n in s.inputs]
    args = [re.sub(r"\{(\d+)\}", lambda m: names[int(m.group(1))], a) for a in s.args]
    env = " ".join(f"{k}={v}" for k, v in s.env if v is not None)
    files = " ".join(f"(with {n} present)" for n, _ in s.files)
    text = common.command_text(program, args, lambda a: a)
    return " ".join(x for x in (env, text, files) if x)


def case_verdict(r: dict) -> tuple[str, str]:
    """A case's verdict: the record compares the replica in reproduce mode with AFNI (every
    output byte for byte, exit status, messages); the fix pass and AFNI's variability are
    reported."""
    st = r["status"]
    if st == "both-error" and "crash" in r["reason"]:
        return "divergence", common.clip(r["reason"])
    v = common.outcome_verdict(r)
    if v is not None:
        return v
    rep = r.get("reported", {})
    notes = []
    band = {k.removeprefix("band: "): v for k, v in rep.items() if k.startswith("band: ")}
    if band:
        same = [k for k, v in band.items() if v == 0]
        other = [f"{k}: {v:,} values differ" for k, v in band.items() if v]
        if same:
            notes.append("AFNI's own runs (" + ", ".join(same) + ") give the same bytes")
        notes += other
    for k, v in rep.items():
        if k.endswith("fMRIPrep's own output (as it ran)"):
            notes.append(
                f"{k.split(':')[0]}: fMRIPrep's own output, "
                + ("the same bytes" if not v else f"{v:,} values differ")
            )
    fix = rep.get("D13 fix pass")
    if fix:
        notes.append(
            f"the default mode corrects a defect of the register here: {fix:,} values change, "
            "the defect named (the record's D13 section)"
        )
    return "bit-identical", common.clip("; ".join(notes), 400)


def cases(rec, scenarios, program: str, examples: dict[str, str]) -> list[dict]:
    defs = {cid: (desc, s) for cid, _cat, desc, s in scenarios}
    out = []
    for r in rec.results:
        desc, s = defs.get(r["case"], ("", None))
        v, why = case_verdict(r)
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": desc,
                "command": command(program, s) if s else "",
                "results": {
                    "replica": {
                        "outcome": r["status"],
                        "verdict": v,
                        "checks": f"{r['checks_passed']}/{r['checks_total']}",
                        "reason": why,
                    }
                },
                **({"example": examples[r["case"]]} if r["case"] in examples else {}),
            }
        )
    return out


def export(ctx: core.Context) -> dict:
    from larmorx_validation.parity.afni_automask import _scenario_list

    reg = core.register(ctx.repo)
    rec = records.load(ctx.repo, TOOL)
    examples = {"fmriprep/ds005040-task-clip_run-1": "fmriprep-automask"}
    cs = cases(rec, _scenario_list(), PROGRAM, examples)
    e = entry(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "3dAutomask",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": [],
            "cache": [fcache.provenance(e)],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cs,
                "replica",
                "larmorx-gpl replica",
                "replica",
                f"docs/validation/{TOOL}.md",
                "validated",
            )
        ],
        "cases": cs,
        "cases_total": len(cs),
        "examples": [_example_ref(ctx, e), _example_bold(ctx, e), _example_peels(ctx, e, reg)],
    }


if __name__ == "__main__":  # compute the cache entry only
    from larmorx_validation.figures.core import REPO, default_cache

    print("done", entry(core.Context(repo=REPO, site=Path("."), cache=default_cache())).dir)
