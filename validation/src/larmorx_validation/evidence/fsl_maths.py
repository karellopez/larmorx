# SPDX-License-Identifier: Apache-2.0
"""fslmaths' evidence (the larmorx-nc replica, FSL Licence, and LARMORX's clean-room original,
crate larmorx-fsl, Apache-2.0, written from ``specs/fslmaths.md``: its parts in
``fsl_maths_original``): fMRIPrep's own call (niworkflows' ``combine_masks``:
``fslmaths <mask> -mul <mask> <out>`` in every BOLD reference's brain mask; the record's four
calls on three oracle runs) with the masks drawn over the reference they were made from;
fMRIPrep's goodvoxels mask (``--project-goodvoxels``: its fourteen fslmaths calls in the
workflow's order on ds005040's BOLD series in T1w space, the cortical ribbon a stand-in: the
segmentation's grey matter); one call per operation family on the same real series (the direct
and the FFT convolutions among them); GLIBC-07 on a real image (glibc 2.35's double functions
against the correctly rounded ones); every case of both records with its verdict; random
testing's two trials, the original's with its triage; the speed of the benchmark report.

Every run of FSL goes through the oracle-output cache (``use`` mode; fslmaths is
single-threaded and deterministic); the replica runs as a separate program, nothing here imports
it. Everything shown lies inside fMRIPrep's brain mask of the BOLD series (dilated): no face can
show.
"""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
import shutil
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.evidence import fsl_maths_original as fo
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "fsl-maths"
RECORD = "fsl-maths-replica"
#: Random testing's trial of this part (``<workspace>/oracles/difftest/<trial>/summary.json``).
TRIAL = "2026-11-03-fsl-maths-replica"
#: The oracle run whose calls the examples show.
RUN = "ds005040-sub-002-noreconall"
#: A notice of the replica's (``larmorx-nc: upstream defect FSL-xx (...)``, GLIBC-07 too).
NOTICE = re.compile(r"upstream defect ((?:FSL|GLIBC)-\d+)")
#: The replica's runs of an example: label, D13 mode.
REPLICA_RUNS = [("replica", "reproduce"), ("replica-fix", "fix")]
PANELS = [
    ("fsl", "original", "FSL 6.0.7.23, fslmaths",
     "the conda installation, as built (through the oracle cache)"),
    ("replica", "larmorx", "larmorx-nc replica", "reproduce mode"),
    ("replica-fix", "larmorx", "larmorx-nc replica", "default mode (D13 corrections)"),
]  # fmt: skip


def _suite():
    from larmorx_validation.parity import fsl_maths_replica as fs

    return fs


def versions() -> dict[str, str]:
    from larmorx_validation.parity import fsl_versions
    from larmorx_validation.parity.fsl_flirt_replica import fsldir
    from larmorx_validation.parity.fsl_mcflirt_replica import replica_version

    return {
        "fsl": f"FSL {fsl_versions.version_of(fsldir())} (fsl-avwutils 2209.6, fsl-newimage "
        "2601.0; ~/fsl-6.0.7.23, never on the PATH)",
        "larmorx-nc": replica_version(),
    }


# --------------------------------------------------------------------- running the programs


def unset_env() -> tuple[str, ...]:
    return tuple(k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_", "OMP_")))


def oracle(e: fcache.Entry, label: str, args: list[str], put, program: str = "fslmaths"):
    """FSL's ``program`` with ``args`` through the oracle-output cache, in a fresh directory
    holding what ``put(dir)`` copies there; the files it writes into ``<entry>/<label>``, its
    standard output into ``<entry>/<label>.stdout.txt``; whether it was served is kept."""
    from larmorx_validation.parity import fsl_stats_replica as st
    from larmorx_validation.parity.fsl_flirt_replica import _environment, fsldir

    run_ = _suite().oracle_run if program == "fslmaths" else st.oracle_run
    with tempfile.TemporaryDirectory(prefix="lx-nc-maths-ev_") as t:
        cwd = Path(t)
        put(cwd)
        given = {p.name for p in cwd.iterdir()}
        p = run_(args, cwd=cwd, settings={})
        if p.returncode != 0:
            raise RuntimeError(f"{label}: FSL's {program} exited {p.returncode}: {p.stderr[-800:]}")
        out = e.dir / label
        out.mkdir(exist_ok=True)
        for f in cwd.iterdir():
            if f.name not in given and f.is_file():
                shutil.copy(f, out / f.name)
        (e.dir / f"{label}.stdout.txt").write_text(p.stdout, encoding="utf-8")
    run = fcache.Run(label, [str(fsldir() / "bin" / program), *args], _environment({}))
    run.exit, run.seconds = p.returncode, round(p.seconds, 2)
    run.stderr_tail = p.stderr[-2000:]
    e.runs.append(run)
    e.extra.setdefault("oracle_cache", {})[label] = "served" if p.cached else "ran"
    return p


def replica(
    e: fcache.Entry,
    label: str,
    mode: str,
    args: list[str],
    put,
    program: str = "fslmaths",
    *,
    exe: Path | None = None,
    extra_env: dict[str, str] | None = None,
) -> None:
    """The replica (or ``exe``) on ``args`` in ``mode``, in ``<entry>/<label>`` (it writes where
    FSL does: its working directory); its standard output into ``<label>.stdout.txt``, the
    register entries its notices name kept; the inputs removed after."""
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    exe = exe or _suite().replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    cwd = e.dir / label
    cwd.mkdir(exist_ok=True)
    put(cwd)
    given = list(cwd.iterdir())
    env = {**_environment({}), "LARMORX_UPSTREAM_DEFECTS": mode, "OMP_NUM_THREADS": "1",
           **(extra_env or {})}  # fmt: skip
    e.run(label, [str(exe), "fsl", program, *args], env=env, unset=unset_env(), cwd=cwd)
    log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
    e.extra.setdefault("named", {})[label] = sorted(set(NOTICE.findall(log)))
    stdout = log.split("--- stdout\n", 1)[1].split("\n--- stderr\n", 1)[0]
    (e.dir / f"{label}.stdout.txt").write_text(stdout, encoding="utf-8")
    for p in given:
        if p.is_file():
            p.unlink()


def _copier(files: dict[str, Path]):
    def put(d: Path) -> None:
        for name, src in files.items():
            shutil.copy(src, d / name)

    return put


# ------------------------------------------------------------------------------ the data


def _out_dir() -> Path:
    return common.fmriprep_run(RUN) / "out" / "sub-002" / "func"


def bold() -> Path:
    """ds005040's preprocessed BOLD series in T1w space (fMRIPrep's output; goodvoxels' input)."""
    return _out_dir() / "sub-002_task-clip_run-1_space-T1w_desc-preproc_bold.nii.gz"


def brain_mask() -> Path:
    return _out_dir() / "sub-002_task-clip_run-1_space-T1w_desc-brain_mask.nii.gz"


def chain() -> Path:
    """FSL's goodvoxels chain on the run, as the record's cases made it (each node's output,
    fslstats' numbers in ``values.json``)."""
    base = _suite()._goodvoxels_chain(RUN)
    if base is None:
        raise FileNotFoundError(f"the goodvoxels chain of {RUN}")
    return base


def _show(mask: np.ndarray, iterations: int = 2) -> np.ndarray:
    from scipy import ndimage

    return ndimage.binary_dilation(mask > 0, iterations=iterations)


def _window(a: np.ndarray, m: np.ndarray) -> list[float]:
    v = a[m]
    return [float(np.percentile(v, 0.5)), float(np.percentile(v, 99.5))]


# ----------------------------------------------------------------- fMRIPrep's combine_masks


def _combine_node() -> Path:
    """The HMC reference's combine_masks node of the run (``enhance_and_skullstrip_bold_wf``)."""
    nodes = [n for r, n in _suite().fmriprep_calls()
             if r == RUN and n.parent.name == "enhance_and_skullstrip_bold_wf"]  # fmt: skip
    if not nodes:
        raise FileNotFoundError(f"fMRIPrep's combine_masks node of {RUN}")
    return nodes[0]


def _combine_inputs(node: Path) -> dict[str, Path]:
    """The two masks the call multiplies, and the reference both were computed from (the
    BOLD reference after N4, the input of BET's first pass)."""
    args = _suite().fmriprep_args(RUN, node)
    ref = next((node.parent / "n4_correct").glob("*_corrected.nii.gz"), None)
    if ref is None:
        raise FileNotFoundError(f"the N4-corrected reference beside {node}")
    return {"a": Path(args[0]), "b": Path(args[2]), "ref": ref}


def _compute_combine(e: fcache.Entry) -> None:
    fs = _suite()
    node = _combine_node()
    args = fs.fmriprep_args(RUN, node)
    for k, p in _combine_inputs(node).items():
        e.input({"a": "BET's first-pass mask", "b": "the template's mask, on the reference",
                 "ref": "the BOLD reference after N4"}[k], p)  # fmt: skip

    def put(d: Path) -> None:
        pass

    oracle(e, "fsl", args, put)
    for label, mode in REPLICA_RUNS:
        replica(e, label, mode, args, put)
    e.finish(
        versions=versions(),
        command=(node / "command.txt").read_text(encoding="utf-8").strip(),
        node=str(node.relative_to(common.fmriprep_run(RUN))),
        args=args,
        settings="fMRIPrep 25.2.6's combine_masks node of the HMC reference (niworkflows' "
        "enhance_and_skullstrip_bold_wf): its arguments from command.txt, its masks read from "
        "the run's working directory; FSL 6.0.7.23 through the oracle-output cache (the "
        "record's own run) and the larmorx-nc replica in its reproduce and default modes",
    )


def combine_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "fmriprep-combine-masks",
        f"fMRIPrep's combine_masks call of {RUN}: FSL 6.0.7.23 (from the oracle cache) and the "
        "replica, reproduce and default modes", _compute_combine,
    )  # fmt: skip


def _combine_calls_table(rec, rec_o) -> dict:
    """The records' combine_masks calls: each run's two masks, their product, and the records'
    results (FSL's file, fMRIPrep's own; the clean-room original's record beside the
    replica's)."""
    import nibabel as nib

    fs = _suite()
    by = {r["case"]: r for r in rec.results}
    by_o = {r["case"]: r for r in rec_o.results}
    seen: set[str] = set()
    rows = []
    for run, node in fs.fmriprep_calls():
        rel = node.relative_to(node.parents[4])
        cid = f"fmriprep/{run}/{rel.parts[0]}/{node.parent.name}"
        if cid not in by or cid in seen:  # the record's case: the first call of its kind
            continue
        seen.add(cid)
        r = by[cid]
        args = fs.fmriprep_args(run, node)
        a = np.asanyarray(nib.load(args[0]).dataobj) != 0
        b = np.asanyarray(nib.load(args[2]).dataobj) != 0
        own = next(node.glob("*_maths.nii.gz"))
        prod = np.asanyarray(nib.load(str(own)).dataobj) != 0
        kind = ("HMC reference (BET's first pass × the template's mask)"
                if node.parent.name == "enhance_and_skullstrip_bold_wf"
                else "coregistration reference (BET's two passes)")  # fmt: skip
        rep = {**r.get("reported", {}), **r.get("measured", {})}
        same_own = rep.get("fMRIPrep's own mask (its container's FSL 6.0.7.x) has the same values")
        ro = by_o.get(cid)
        o_same = ro is not None and ro["status"] == "pass" and not fo._differs(ro["measured"])
        rows.append({
            "head": run.split("-noreconall")[0],
            "sub": kind,
            "cells": [
                common.cell(" × ".join(map(str, a.shape))),
                common.cell(int(a.sum()), "count"),
                common.cell(int(b.sum()), "count"),
                common.cell(int(prod.sum()), "count"),
                common.cell("identical" if r["status"] == "pass" else r["status"]),
                common.cell("identical" if o_same else ro["status"] if ro else "–"),
                common.cell("the same values" if same_own == "yes" else "differs"),
            ],
            "verdict": "bit-identical" if r["status"] == "pass" and o_same else "open",
        })  # fmt: skip
    return common.table(
        "combine-calls",
        "fMRIPrep's combine_masks calls of the records",
        ["Run", "Grid", {"label": "Mask A voxels", "num": True},
         {"label": "Mask B voxels", "num": True}, {"label": "Product voxels", "num": True},
         "Replica against FSL 6.0.7.23", "Clean-room original against FSL 6.0.7.23",
         "fMRIPrep's own output"],
        rows,
        note="The record's four calls on three oracle runs: ds000005 (single-band 3 T), ds005040 "
        "(both kinds: the head-motion reference's mask, BET's first pass times the template's "
        "brain mask on the reference, and the coregistration reference's, BET's first pass "
        "times its second) and ds003592 (multi-echo). Voxels counted where the mask is not 0. "
        "*Identical*: every file fslmaths writes byte for byte (decompressed, `descrip` aside), "
        "with FSL's exit status and messages (the clean-room original's record: its values bit "
        "for bit and its header byte for byte); fMRIPrep's own output is the node's file in the "
        "run's working directory (fslmaths in fMRIPrep 25.2.6's image).",
    )  # fmt: skip


def _example_combine(ctx: core.Context, rec) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    e = combine_entry(ctx)
    info = e.info()
    node = _combine_node()
    ins = _combine_inputs(node)
    ref_img = nib.load(str(ins["ref"]))
    ref = np.asanyarray(ref_img.dataobj)
    a = np.asanyarray(nib.load(str(ins["a"])).dataobj)
    b = np.asanyarray(nib.load(str(ins["b"])).dataobj)
    show = _show((a != 0) | (b != 0), 2)
    grid = core.Grid.around(ref_img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("ref", "input", "ds005040 BOLD reference, after N4",
                     "the image both masks were computed from (fMRIPrep's HMC reference, "
                     "float32)", {"all": ref}))  # fmt: skip
    win = _window(ref, show)
    over = {"mask": {"on": "ref", "window": win}}
    ex.add(exm.Panel("mask-a", "input", "Mask A: BET's first pass",
                     "skullstrip_first_pass (bet -f 0.2 -m), float32", {"all": a},
                     overlay=over))  # fmt: skip
    ex.add(exm.Panel("mask-b", "input", "Mask B: the template's brain mask",
                     "map_brainmask: the template's mask on the reference (fixhdr_skullstrip2), "
                     "uint8", {"all": b}, overlay=over))  # fmt: skip
    name = Path(info["args"][-1]).name
    arrays = {pid: {"mask": common.load(str(e.dir / pid / name))} for pid, *_ in PANELS}
    for pid, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    eo = fo.entry(ctx, TOOL, "combine")
    oinfo = eo.info()
    oarrays = {pid: {"mask": common.load(str(eo.dir / pid / name))} for pid, *_ in fo.PANELS}
    for pid, role, label, sub in fo.PANELS:
        ex.add(exm.Panel(pid, role, label, sub, oarrays[pid], overlay=over))
    own = {"mask": common.load(str(node / name))}
    ex.add(exm.Panel("own", "fmriprep", "fMRIPrep 25.2.6's own output",
                     "the node's file in the run's working directory (fslmaths in fMRIPrep's "
                     "image)", own, overlay=over))  # fmt: skip
    d = common.d
    diffs = [
        d("replica-fsl", "replica", "fsl", "main", "replica − FSL 6.0.7.23", common.ROLE_LX),
        d("original-fsl", "original", "fsl", "main", "clean-room original − FSL 6.0.7.23",
          common.ROLE_LX),
        d("replica-own", "replica", "own", "fmriprep", "replica − fMRIPrep's own output",
          common.ROLE_FMRIPREP),
        d("original-own", "original", "own", "fmriprep",
          "clean-room original − fMRIPrep's own output", common.ROLE_FMRIPREP),
        d("fix-replica", "replica-fix", "replica", "check", "replica, default − reproduce mode",
          common.ROLE_SELF),
        d("original1-original", "original-1", "original", "check",
          "clean-room original, 1 thread − 14 threads", common.ROLE_SELF),
        d("ofix-original", "original-fix", "original", "check",
          "clean-room original, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    outputs = [{"id": "mask", "label": "Combined mask (A × B)", "unit": ""}]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    verdicts(diffs, same, info, "fslmaths")
    result, _m = fo.outcome({"mask": arrays["fsl"]["mask"]}, {"mask": oarrays["original"]["mask"]},
                            {"mask": "the combined mask"}, {}, "fslmaths writes")  # fmt: skip
    fo.verdicts(diffs, same, oinfo.get("named", {}).get("original-fix", []), result,
                same.get("replica-own"))  # fmt: skip
    at, _k = common.largest_in_slice(ex, "fsl", "replica", "mask", "axial")
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["mask-a", "mask-b", "fsl", "replica", "original"],
        "diffs": ["replica-fsl", "original-fsl"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="ref",
        caption="each mask drawn over the BOLD reference it was computed from.",
    )  # fmt: skip
    data = common.DS005040
    return {
        "id": "combine",
        "title": "fMRIPrep's brain mask of a BOLD reference: two masks combined",
        "summary": "The combine_masks node fMRIPrep 25.2.6 ran on ds005040's head-motion "
        "reference: niworkflows multiplies the mask of BET's first pass by the template's "
        "brain mask brought onto the reference, and the product is the reference's brain mask. "
        "Given to FSL 6.0.7.23, to the larmorx-nc replica in its reproduce and default "
        "modes, and to LARMORX's clean-room original in its reproduce mode at 14 threads and "
        "at one and in its default mode; each mask is drawn over the reference it was computed "
        "from. Below the viewer, every combine_masks call of both records, three oracle runs.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): fMRIPrep's "
                "BOLD reference and its two masks from the run's working directory; shown "
                "inside the union of the masks, dilated by 2 voxels, so no face shows",
            },
            {"label": "Node", "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}`"},
            {
                "label": "Command",
                "text": "`fslmaths <BET's first-pass mask> -mul <the template's mask> <out>`",
            },
            {"label": "Programs", "text": programs(info) + "; " + fo.programs(oinfo)},
            {"label": "Grid", "text": common.describe_grid(ref_img, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["fmriprep", "LARMORX against fMRIPrep's own output"],
            ["check", "LARMORX against itself: the threads and the modes"],
        ],
        "tables": [_combine_calls_table(rec, records.load(ctx.repo, fo.RECORD["fslmaths"]))],
        "metrics_note": note(int(np.prod(ref.shape)), fo.note()),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data])
        + fo.provenance(oinfo, eo, [data]),
    }


# ------------------------------------------------------------------ fMRIPrep's goodvoxels mask

#: The goodvoxels nodes' labels.
GOODVOXELS_TEXT = {
    "stdev_volume": "standard deviation over time (-Tstd)",
    "mean_volume": "mean over time (-Tmean)",
    "cov_volume": "coefficient of variation (std ÷ mean)",
    "cov_ribbon": "CoV inside the ribbon (-mas)",
    "cov_ribbon_norm": "CoV in the ribbon ÷ its mean",
    "smooth_norm": "the ribbon, smoothed (-bin -s 5)",
    "cov_ribbon_norm_smooth": "normalised CoV smoothed and dilated (-s 5 -div -dilD)",
    "cov_norm": "CoV ÷ the ribbon's mean CoV",
    "cov_norm_modulate": "normalised CoV ÷ its local mean",
    "cov_norm_modulate_ribbon": "modulated CoV inside the ribbon",
    "bin_mean_volume": "where the mean is not 0 (-bin)",
    "goodvoxels_thr": "modulated CoV above mean + std / 2 (-thr)",
    "goodvoxels_mask": "good voxels: not above the threshold (-bin -sub -mul -1)",
    "goodvoxels_ribbon_mask": "good voxels inside the ribbon (-mas)",
}
#: Masks among them, drawn over the series' mean.
GOODVOXELS_MASKS = ("bin_mean_volume", "goodvoxels_mask", "goodvoxels_ribbon_mask")


def _goodvoxels_files(base: Path) -> dict[str, Path]:
    fs = _suite()
    files = {"bold": bold(), "ribbon": base / "ribbon.nii.gz"}
    for n, prog, _ in fs.GOODVOXELS_STEPS:
        if prog == "fslmaths":
            files[n] = base / n / f"{n}.nii.gz"
    return files


def _compute_goodvoxels(e: fcache.Entry) -> None:
    fs = _suite()
    base = chain()
    values = json.loads((base / "values.json").read_text(encoding="utf-8"))
    files = _goodvoxels_files(base)
    e.input("BOLD series in T1w space (fMRIPrep's)", bold())
    e.input("the ribbon's stand-in (the segmentation's grey matter)", files["ribbon"])
    calls = {}
    for node, prog, template in fs.GOODVOXELS_STEPS:
        if prog != "fslmaths":
            continue

        def put(d: Path, template=template) -> None:
            fs._goodvoxels_args(template, files, values, d)

        with tempfile.TemporaryDirectory(prefix="lx-nc-maths-ev-gv_") as t:
            args = [*fs._goodvoxels_args(template, files, values, Path(t)), f"{node}.nii.gz"]
        calls[node] = args
        oracle(e, f"fsl-{node}", args, put)
        for label, mode in REPLICA_RUNS:
            replica(e, f"{label}-{node}", mode, args, put)
    e.finish(
        versions=versions(),
        calls=calls,
        values=values,
        settings="fMRIPrep 25.2.6's init_goodvoxels_bold_mask_wf (--project-goodvoxels; not in "
        "the oracle runs) on ds005040's BOLD series in T1w space: each fslmaths node's "
        "arguments as the workflow makes them, its inputs FSL's outputs of the earlier nodes "
        "(the record's chain, oracles/inputs/fsl-maths-replica/goodvoxels/), fslstats' numbers "
        "as nipype formats them; the cortical ribbon a stand-in (the anatomical segmentation's "
        "grey matter at each BOLD voxel); FSL 6.0.7.23 through the oracle-output cache (the "
        "record's own runs) and the larmorx-nc replica in its reproduce and default modes",
    )


def goodvoxels_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "fmriprep-goodvoxels",
        f"fMRIPrep's goodvoxels mask on {RUN}: its fourteen fslmaths calls by FSL 6.0.7.23 "
        "(from the oracle cache) and the replica, reproduce and default modes",
        _compute_goodvoxels,
    )  # fmt: skip


def _stats_numbers_table(ctx: core.Context, values: dict) -> dict:
    """goodvoxels' four fslstats calls: FSL's printed numbers (the record's chain) and the
    replica's (``fsl-stats``'s entry)."""
    from larmorx_validation.evidence import fsl_stats

    st = fsl_stats.goodvoxels_entry(ctx).info()
    so = fo.entry(ctx, "fsl-stats", "goodvoxels").info()["printed"]
    rows = []
    for node, prog, template in _suite().GOODVOXELS_STEPS:
        if prog != "fslstats":
            continue
        f = st["printed"][node]
        o = so[node]["original"]
        rows.append({
            "head": node,
            "sub": f"fslstats <{template[0][1:-1]}> {' '.join(template[1:])}",
            "cells": [common.cell(f["fsl"].strip()), common.cell(f["replica"].strip()),
                      common.cell(o.strip()), common.cell(float(values[node]), "sig6")],
            "verdict": "bit-identical" if f["fsl"] == f["replica"] == o else "open",
        })  # fmt: skip
    up = values["upper_thresh"]
    rows.append({
        "head": "upper_thresh",
        "sub": "mod_ribbon_mean + mod_ribbon_std × 0.5 (fMRIPrep's Python), given to -thr",
        "cells": [common.cell(f"{up:.10f}"), common.cell(f"{up:.10f}"), common.cell(f"{up:.10f}"),
                  common.cell(up, "sig6")],
    })  # fmt: skip
    return common.table(
        "goodvoxels-stats",
        "The numbers between the steps: fslstats' four calls",
        [
            "Node",
            "FSL 6.0.7.23 prints",
            "The replica prints",
            "The clean-room original prints",
            {"label": "As read", "num": True},
        ],
        rows,
        note="Each fslstats call on FSL's output of the node before it (through the oracle "
        "cache) and the larmorx-nc replica's and the clean-room original's (reproduce mode, 14 "
        "threads) on the same file: the printed text compared byte for byte. fMRIPrep reads each number as Python's float, divides by the means and "
        "passes the threshold to fslmaths as nipype formats them (`%.8f`, `%.10f`); the "
        "fsl-stats page has the four calls in its record.",
    )


def _example_goodvoxels(ctx: core.Context) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    fs = _suite()
    e = goodvoxels_entry(ctx)
    info = e.info()
    base = chain()
    files = _goodvoxels_files(base)
    img = nib.load(str(bold()))
    series = np.asanyarray(img.dataobj)
    mask = common.load(str(brain_mask()))
    show = _show(mask, 2)
    grid = core.Grid.around(img.slicer[..., 0], show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("bold1", "input", "ds005040 BOLD series in T1w space, volume 1",
                     f"fMRIPrep's preprocessed series ({series.shape[3]} volumes, float32): the "
                     "workflow's input", {"all": np.ascontiguousarray(series[..., 0])}))  # fmt: skip
    del series
    mean = common.load(str(files["mean_volume"]))
    ex.add(exm.Panel("mean", "input", "its mean over time",
                     "FSL's -Tmean (the node mean_volume): the image the masks are drawn over",
                     {"all": mean}))  # fmt: skip
    win = _window(mean, show)
    nodes = [n for n, prog, _ in fs.GOODVOXELS_STEPS if prog == "fslmaths"]
    rib_over = {n: {"on": "mean", "window": win} for n in nodes}
    ex.add(exm.Panel("ribbon", "input", "the ribbon's stand-in",
                     "the segmentation's grey matter at each BOLD voxel (uint8): the no-reconall "
                     "oracle runs have no FreeSurfer ribbon", {"all": common.load(str(files["ribbon"]))},
                     overlay=rib_over))  # fmt: skip
    over = {n: {"on": "mean", "window": win} for n in GOODVOXELS_MASKS}
    arrays = {pid: {n: common.load(str(e.dir / f"{pid}-{n}" / f"{n}.nii.gz")) for n in nodes}
              for pid, *_ in PANELS}  # fmt: skip
    for pid, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    eo = fo.entry(ctx, TOOL, "goodvoxels")
    oinfo = eo.info()
    oarrays = {pid: {n: common.load(str(eo.dir / f"{pid}-{n}" / f"{n}.nii.gz")) for n in nodes}
               for pid, *_ in fo.PANELS}  # fmt: skip
    for pid, role, label, sub in fo.PANELS:
        ex.add(exm.Panel(pid, role, label, sub, oarrays[pid], overlay=over))
    d = common.d
    diffs = [
        d("replica-fsl", "replica", "fsl", "main", "replica − FSL 6.0.7.23", common.ROLE_LX),
        d("original-fsl", "original", "fsl", "main", "clean-room original − FSL 6.0.7.23",
          common.ROLE_LX),
        d("fix-replica", "replica-fix", "replica", "check", "replica, default − reproduce mode",
          common.ROLE_SELF),
        d("original1-original", "original-1", "original", "check",
          "clean-room original, 1 thread − 14 threads", common.ROLE_SELF),
        d("ofix-original", "original-fix", "original", "check",
          "clean-room original, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    outputs = [{"id": n, "label": f"{k + 1}. {n}: {GOODVOXELS_TEXT[n]}", "unit": ""}
               for k, n in enumerate(nodes)]  # fmt: skip
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    named = sorted({x for k, v in info.get("named", {}).items() if k.startswith("replica-fix")
                    for x in v})  # fmt: skip
    verdicts(diffs, same, {"named": {"replica-fix": named}}, "fslmaths")
    onamed = sorted({x for k, v in oinfo.get("named", {}).items() if k.startswith("original-fix")
                     for x in v})  # fmt: skip
    result, _m = fo.outcome({n: arrays["fsl"][n] for n in nodes},
                            {n: oarrays["original"][n] for n in nodes},
                            {n: f"{n}" for n in nodes}, {}, "of the fourteen calls")  # fmt: skip
    fo.verdicts(diffs, same, onamed, result)
    first = "goodvoxels_ribbon_mask"
    at, _k = common.largest_in_slice(ex, "fsl", "replica", first, "axial")
    default = {
        "output": first,
        "plane": "axial",
        "voxel": at,
        "panels": ["mean", "ribbon", "fsl", "replica", "original"],
        "diffs": ["replica-fsl", "original-fsl"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="fsl",
        caption="the final mask, good voxels inside the ribbon, drawn over the series' mean.",
    )  # fmt: skip
    values = info["values"]
    data = common.DS005040
    return {
        "id": "goodvoxels",
        "title": "fMRIPrep's goodvoxels mask: fourteen fslmaths calls",
        "summary": "fMRIPrep 25.2.6's goodvoxels workflow (`--project-goodvoxels`, used before "
        "projecting a BOLD series onto the cortical surface; not in the oracle runs) built from "
        "its code on ds005040's BOLD series in T1w space: each fslmaths node with its arguments "
        "as the workflow makes them, in its order, its inputs FSL's outputs of the nodes before "
        "it, the numbers between them printed by fslstats. The runs have no FreeSurfer ribbon, "
        "so a stand-in takes its place: the anatomical segmentation's grey matter. Choose a "
        "node above the panels; each was given to FSL 6.0.7.23, to the larmorx-nc replica in "
        "its reproduce and default modes, and to LARMORX's clean-room original in its "
        "reproduce mode at 14 threads and at one and in its default mode.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): fMRIPrep "
                "25.2.6's preprocessed BOLD series in T1w space (out/); shown inside fMRIPrep's "
                "brain mask of the series, dilated by 2 voxels, so no face shows",
            },
            {
                "label": "Workflow",
                "text": "fMRIPrep 25.2.6, `init_goodvoxels_bold_mask_wf` "
                "(fmriprep/workflows/bold/resampling.py)",
            },
            {
                "label": "Ribbon",
                "text": "a stand-in: the segmentation's grey matter (label 1) at "
                "each BOLD voxel's centre; the no-reconall oracle runs have no FreeSurfer ribbon, so "
                "the mask differs from the one fMRIPrep would make with recon-all",
            },
            {
                "label": "Numbers",
                "text": f"the ribbon's mean CoV {values['cov_ribbon_mean']}, "
                f"the modulated CoV's mean {values['mod_ribbon_mean']} and standard deviation "
                f"{values['mod_ribbon_std']}: the threshold {values['upper_thresh']:.10f} "
                "(fslstats -M and -S, as the table below the viewer shows)",
            },
            {"label": "Programs", "text": programs(info) + "; " + fo.programs(oinfo)},
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: the threads and the modes"],
        ],
        "tables": [_stats_numbers_table(ctx, values)],
        "metrics_note": note(int(np.prod(img.shape[:3])), fo.note()),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data])
        + fo.provenance(oinfo, eo, [data]),
    }


# --------------------------------------------------------------- one call per operation family

#: One fslmaths call per operation family on ds005040's series (``{bold}``), FSL's mean of it
#: (``{mean}``), FSL's standard deviation of it (``{std}``) and fMRIPrep's brain mask
#: (``{mask}``): output id -> (family, arguments before the output's name, label).
FAMILIES: dict[str, tuple[str, tuple[str, ...], str]] = {
    "tsnr": ("binary operations with images", ("{mean}", "-div", "{std}", "-mas", "{mask}"),
             "temporal SNR: mean ÷ std, masked"),
    "thrp": ("thresholds (the robust range)", ("{mean}", "-thrP", "30", "-bin"),
             "-thrP 30 -bin: above 30 % of the robust range"),
    "sqrt": ("voxelwise functions", ("{mean}", "-abs", "-sqrt"), "-abs -sqrt"),
    "tmedian": ("reductions along time", ("{bold}", "-Tmedian"), "-Tmedian"),
    "tar1": ("reductions along time", ("{bold}", "-Tar1"), "-Tar1: lag-1 autocorrelation"),
    "ero": ("morphology", ("{mask}", "-kernel", "sphere", "6", "-ero"),
            "the mask eroded (-kernel sphere 6 -ero)"),
    "dilm": ("morphology", ("{mean}", "-mas", "{mask}", "-kernel", "3D", "-dilM"),
             "mean dilation of the masked mean (-kernel 3D -dilM)"),
    "smooth": ("filters: the separable convolution", ("{mean}", "-s", "3"),
               "Gaussian smoothing, sigma 3 mm (-s 3)"),
    "fmean-fft": ("filters: the FFT convolution", ("{mean}", "-kernel", "sphere", "21", "-fmean"),
                  "-kernel sphere 21 -fmean: a 15 × 15 × 13 kernel, by the FFT"),
    "fmedian": ("filters: order statistics", ("{mean}", "-kernel", "3D", "-fmedian"),
                "median filter (-kernel 3D -fmedian)"),
    "edge": ("edges", ("{mean}", "-edge"), "-edge: the gradient's magnitude"),
    "inm": ("intensity normalisation", ("{mean}", "-mas", "{mask}", "-inm", "10000"),
            "masked, its mean scaled to 10,000 (-inm)"),
    "bptf": ("temporal filtering", ("{bold}", "-bptf", "25", "-1", "-Tstd"),
             "high-pass filtered (-bptf 25 -1), then -Tstd"),
    "roi": ("regions", ("{mean}", "-roi", "10", "30", "10", "40", "5", "30", "0", "1"),
            "-roi: zero outside a box"),
    "tfce": ("threshold-free cluster enhancement",
             ("{mean}", "-div", "{std}", "-mas", "{mask}", "-tfce", "2", "0.5", "6"),
             "TFCE of the temporal SNR (-tfce 2 0.5 6)"),
}  # fmt: skip


#: The families FSL convolves by its FFT (the original by the direct sum).
FFT_FAMILIES = ("fmean-fft",)


def _family_files() -> dict[str, Path]:
    base = chain()
    return {
        "bold": bold(),
        "mean": base / "mean_volume" / "mean_volume.nii.gz",
        "std": base / "stdev_volume" / "stdev_volume.nii.gz",
        "mask": brain_mask(),
    }


def family_args(oid: str) -> tuple[list[str], dict[str, Path]]:
    """A family's call: its arguments (inputs by their names in the run directory, the output
    ``<oid>.nii.gz``) and the inputs to copy there."""
    files = _family_files()
    names = {"bold": "bold.nii.gz", "mean": "mean.nii.gz", "std": "std.nii.gz",
             "mask": "mask.nii.gz"}  # fmt: skip
    args, used = [], {}
    for a in FAMILIES[oid][1]:
        if a.startswith("{"):
            k = a[1:-1]
            args.append(names[k])
            used[names[k]] = files[k]
        else:
            args.append(a)
    return [*args, f"{oid}.nii.gz"], used


def _compute_families(e: fcache.Entry) -> None:
    for k, p in _family_files().items():
        e.input(k, p)
    calls = {}
    for oid in FAMILIES:
        args, used = family_args(oid)
        calls[oid] = args
        put = _copier(used)
        oracle(e, f"fsl-{oid}", args, put)
        for label, mode in REPLICA_RUNS:
            replica(e, f"{label}-{oid}", mode, args, put)
    e.finish(
        versions=versions(),
        calls=calls,
        settings="One fslmaths call per operation family on ds005040's BOLD series in T1w space "
        "(fMRIPrep's output), FSL's -Tmean and -Tstd of it (the goodvoxels chain's) and "
        "fMRIPrep's brain mask of it; FSL 6.0.7.23 through the oracle-output cache and the "
        "larmorx-nc replica in its reproduce and default modes",
    )


def families_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "families",
        f"One fslmaths call per operation family on {RUN}'s BOLD series: FSL 6.0.7.23 (through "
        "the oracle cache) and the replica, reproduce and default modes", _compute_families,
    )  # fmt: skip


def _example_families(ctx: core.Context) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    e = families_entry(ctx)
    info = e.info()
    files = _family_files()
    img = nib.load(str(files["mean"]))
    mean = common.load(str(files["mean"]))
    mask = common.load(str(files["mask"]))
    show = _show(mask, 2)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("mean", "input", "ds005040 BOLD series' mean over time",
                     "FSL's -Tmean of fMRIPrep's series in T1w space (float32); the series "
                     "itself is the input of the calls along time", {"all": mean}))  # fmt: skip
    ex.add(exm.Panel("std", "input", "its standard deviation over time", "FSL's -Tstd (float32)",
                     {"all": common.load(str(files["std"]))}))  # fmt: skip
    win = _window(mean, show)
    over = {oid: {"on": "mean", "window": win} for oid in FAMILIES}
    ex.add(exm.Panel("mask", "input", "fMRIPrep's brain mask of the series", "uint8",
                     {"all": mask}, overlay=over))  # fmt: skip
    masks = {"thrp", "ero"}
    mover = {oid: {"on": "mean", "window": win} for oid in masks}
    arrays = {pid: {oid: common.load(str(e.dir / f"{pid}-{oid}" / f"{oid}.nii.gz"))
                    for oid in FAMILIES} for pid, *_ in PANELS}  # fmt: skip
    for pid, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=mover))
    eo = fo.entry(ctx, TOOL, "families")
    oinfo = eo.info()
    oarrays = {pid: {oid: common.load(str(eo.dir / f"{pid}-{oid}" / f"{oid}.nii.gz"))
                     for oid in FAMILIES} for pid, *_ in fo.PANELS}  # fmt: skip
    for pid, role, label, sub in fo.PANELS:
        ex.add(exm.Panel(pid, role, label, sub, oarrays[pid], overlay=mover))
    d = common.d
    diffs = [
        d("replica-fsl", "replica", "fsl", "main", "replica − FSL 6.0.7.23", common.ROLE_LX),
        d("original-fsl", "original", "fsl", "main", "clean-room original − FSL 6.0.7.23",
          common.ROLE_LX),
        d("fix-replica", "replica-fix", "replica", "check", "replica, default − reproduce mode",
          common.ROLE_SELF),
        d("original1-original", "original-1", "original", "check",
          "clean-room original, 1 thread − 14 threads", common.ROLE_SELF),
        d("ofix-original", "original-fix", "original", "check",
          "clean-room original, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    outputs = [{"id": oid, "label": f"{fam}: {label}", "unit": ""}
               for oid, (fam, _a, label) in FAMILIES.items()]  # fmt: skip
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    named = sorted({x for k, v in info.get("named", {}).items() if k.startswith("replica-fix")
                    for x in v})  # fmt: skip
    verdicts(diffs, same, {"named": {"replica-fix": named}}, "fslmaths")
    onamed = sorted({x for k, v in oinfo.get("named", {}).items() if k.startswith("original-fix")
                     for x in v})  # fmt: skip
    result, ometrics = fo.outcome(
        {oid: arrays["fsl"][oid] for oid in FAMILIES},
        {oid: oarrays["original"][oid] for oid in FAMILIES},
        {oid: f"`{' '.join(family_args(oid)[0][:-1])}`" for oid in FAMILIES},
        {oid: "fft" for oid in FFT_FAMILIES},
        "of the calls",
    )
    # the FFT family against the exact result (float64, §10.2's definition): which convolution
    # lies closer to it (findings 31 of the original's)
    oid = FFT_FAMILIES[0]
    words = FAMILIES[oid][1]
    kernel = fo.sphere_kernel(float(words[words.index("sphere") + 1]), img.header.get_zooms()[:3])
    exact, _count = fo.exact_fmean(mean, kernel)
    fe, oe_ = fo.against_exact(exact, arrays["fsl"][oid], oarrays["original"][oid])
    vs_exact = (
        f"against the exact result (computed in float64 from §10.2's definition; {kernel.shape[0]} "
        f"× {kernel.shape[1]} × {kernel.shape[2]} kernel of {int(kernel.sum())} voxels) FSL's FFT "
        f"lies within {fe['max']:.2g} of the largest value (median {fe['median']:.2g}), the "
        f"original's single-precision direct sum within {oe_['max']:.2g} (median "
        f"{oe_['median']:.2g}): on this call the original is the less accurate of the two "
        "(findings 31: a follow-up, its sums in double where FSL takes its FFT)"
    )
    if oid in ometrics:
        result = (result[0], common.clip(result[1].rstrip(".") + "; " + vs_exact + ".", 1400))
    fo.verdicts(diffs, same, onamed, result)
    first = "smooth"
    at, _k = common.largest_in_slice(ex, "fsl", "replica", first, "axial")
    default = {
        "output": first,
        "plane": "axial",
        "voxel": at,
        "panels": ["mean", "fsl", "replica", "replica-fix", "original"],
        "diffs": ["replica-fsl", "original-fsl"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="fsl",
        caption="choose another call above the panels.",
    )  # fmt: skip
    rows = []
    for oid, (fam, _a, _label) in FAMILIES.items():
        args, _used = family_args(oid)
        m = next(x for x in diffs if x["id"] == "replica-fsl")["metrics"][oid]
        rows.append({
            "head": fam,
            "sub": "`fslmaths " + " ".join(args) + "`",
            "cells": [common.cell(m["voxels"], "count"), common.cell(m["differ"], "count"),
                      common.cell("–" if m["identical"] else m["max_abs"])],
            "verdict": "bit-identical" if m["identical"] else "open",
        })  # fmt: skip
    orows = []
    for oid, (fam, _a, _label) in FAMILIES.items():
        args, _used = family_args(oid)
        m = next(x for x in diffs if x["id"] == "original-fsl")["metrics"][oid]
        sm = ometrics.get(oid)
        orows.append({
            "head": fam,
            "sub": "`fslmaths " + " ".join(args) + "`",
            "cells": [common.cell(m["voxels"], "count"), common.cell(m["differ"], "count"),
                      common.cell("–" if m["identical"] else m["max_abs"]),
                      common.cell("–" if sm is None else sm["relative"]),
                      common.cell("–" if sm is None else f"{sm['share']:.1%}")],
            "verdict": "bit-identical" if m["identical"] else "open",
        })  # fmt: skip
    otable = common.table(
        "families-calls-original",
        "The calls, the clean-room original against FSL 6.0.7.23",
        ["Family", {"label": "Voxels", "num": True}, {"label": "Values that differ", "num": True},
         {"label": "Largest |Δ|", "num": True},
         {"label": "Largest |Δ| ÷ FSL's largest value (§21: ≤ 1e-5)", "num": True},
         {"label": "Within 4 units in the last place (§21: ≥ 99 %)", "num": True}],
        orows,
        note="Each call's output by the clean-room original (reproduce mode, 14 threads; the "
        "same bits at one thread) against FSL's, every voxel of the files; where a value differs, "
        "specs/fslmaths.md §21's metric of the sums over every voxel (the non-finite voxels "
        "identical). FSL convolves the 15 × 15 × 13 spherical kernel by its FFT, the original by "
        f"a direct sum in single precision; {vs_exact}.",
    )  # fmt: skip
    table = common.table(
        "families-calls",
        "The calls, against FSL 6.0.7.23",
        ["Family", {"label": "Voxels", "num": True}, {"label": "Values that differ", "num": True},
         {"label": "Largest |Δ|", "num": True}],
        rows,
        note="Each call's output by the replica (reproduce mode) against FSL's, every voxel of "
        "the files. `bold.nii.gz` is fMRIPrep's series, `mean.nii.gz` and `std.nii.gz` FSL's "
        "-Tmean and -Tstd of it, `mask.nii.gz` fMRIPrep's brain mask; with `-kernel sphere 21` "
        "(15 × 15 × 13 voxels on this grid) fslmaths convolves by the FFT, with `-s` by "
        "separable sums.",
    )  # fmt: skip
    data = common.DS005040
    return {
        "id": "families",
        "title": "One call per operation family, on a real series",
        "summary": "fslmaths' operation families, one call each, on ds005040's BOLD series in T1w "
        "space (the goodvoxels workflow's input), FSL's mean and standard deviation of it, and "
        "fMRIPrep's brain mask: arithmetic with images, the thresholds, the voxelwise functions, "
        "the reductions along time, morphology, the convolutions (separable, by the FFT, the "
        "median), edges, normalisation, temporal filtering, regions and the threshold-free "
        "cluster enhancement. Choose a call above the panels; each was given to FSL 6.0.7.23, "
        "to the larmorx-nc replica in its reproduce and default modes, and to LARMORX's "
        "clean-room original in its reproduce mode at 14 threads and at one and in its default "
        "mode.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): fMRIPrep "
                "25.2.6's preprocessed BOLD series in T1w space and its brain mask (out/); shown "
                "inside that mask, dilated by 2 voxels, so no face shows",
            },
            {"label": "Programs", "text": programs(info) + "; " + fo.programs(oinfo)},
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: the threads and the modes"],
        ],
        "tables": [table, otable],
        "metrics_note": note(int(np.prod(img.shape[:3])), fo.note()),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data])
        + fo.provenance(oinfo, eo, [data]),
    }


# ------------------------------------------------------------------- GLIBC-07 on a real image

#: glibc 2.35's double functions as fslmaths calls them, on the series' mean scaled into each
#: function's range (``-dt double``, ``-odt double``): row id -> (function, operations).
GLIBC_CALLS: dict[str, tuple[str, tuple[str, ...]]] = {
    "sin": ("sin", ("-mul", "1e-4", "-sin")),
    "cos": ("cos", ("-mul", "1e-4", "-cos")),
    "tan": ("tan", ("-mul", "1e-4", "-tan")),
    "asin": ("asin", ("-mul", "5e-6", "-asin")),
    "acos": ("acos", ("-mul", "5e-6", "-acos")),
    "atan": ("atan", ("-mul", "1e-4", "-atan")),
    "exp": ("exp", ("-mul", "1e-4", "-exp")),
    "log": ("log", ("-abs", "-add", "1", "-log")),
}
#: The functions whose glibc code is IBM's (LGPL, never translated): correctly rounded in both
#: modes; the others' glibc bits come with ``reproduce``.
IBM = ("sin", "cos", "tan", "asin", "acos", "atan")


def glibc_args(rid: str) -> list[str]:
    return ["-dt", "double", "mean.nii.gz", *GLIBC_CALLS[rid][1], f"{rid}.nii.gz", "-odt",
            "double"]  # fmt: skip


def _compute_glibc(e: fcache.Entry) -> None:
    mean = _family_files()["mean"]
    e.input("mean", mean)
    put = _copier({"mean.nii.gz": mean})
    for rid in GLIBC_CALLS:
        args = glibc_args(rid)
        oracle(e, f"fsl-{rid}", args, put)
        for label, mode in REPLICA_RUNS:
            replica(e, f"{label}-{rid}", mode, args, put)
    e.finish(
        versions=versions(),
        settings="fslmaths' double-precision functions (-dt double ... -odt double) on FSL's "
        "-Tmean of ds005040's BOLD series in T1w space, scaled into each function's range; FSL "
        "6.0.7.23 through the oracle-output cache (glibc 2.35's functions, as built) and the "
        "larmorx-nc replica in its reproduce and default modes",
    )


def glibc_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "glibc-double",
        "fslmaths' double-precision functions on a real image: FSL 6.0.7.23 (glibc 2.35) and "
        "the replica, reproduce and default modes", _compute_glibc,
    )  # fmt: skip


def _ulps(x: np.ndarray, y: np.ndarray) -> tuple[int, int]:
    """Values that differ, and the largest distance in units in the last place (doubles)."""
    xi, yi = x.view(np.int64), y.view(np.int64)
    same = (xi < 0) == (yi < 0)
    dist = np.where(same, np.abs(xi - yi), np.iinfo(np.int64).max)
    differ = int(np.count_nonzero(dist))
    return differ, int(dist.max()) if differ else 0


def _glibc_table(ctx: core.Context) -> tuple[dict, dict]:
    import nibabel as nib

    e = glibc_entry(ctx)
    info = e.info()
    eo = fo.entry(ctx, TOOL, "glibc")
    rows = []
    for rid, (fn, _ops) in GLIBC_CALLS.items():
        fsl = np.asarray(nib.load(str(e.dir / f"fsl-{rid}" / f"{rid}.nii.gz")).dataobj).ravel()
        rep = np.asarray(nib.load(str(e.dir / f"replica-{rid}" / f"{rid}.nii.gz")).dataobj).ravel()
        fix = np.asarray(
            nib.load(str(e.dir / f"replica-fix-{rid}" / f"{rid}.nii.gz")).dataobj
        ).ravel()
        org = np.asarray(
            nib.load(str(eo.dir / f"original-{rid}" / f"{rid}.nii.gz")).dataobj
        ).ravel()
        ofix = np.asarray(
            nib.load(str(eo.dir / f"original-fix-{rid}" / f"{rid}.nii.gz")).dataobj
        ).ravel()
        finite = np.isfinite(fsl)
        n_r, u_r = _ulps(fsl[finite], rep[finite])
        n_f, _u_f = _ulps(fsl[finite], fix[finite])
        n_o, u_o = _ulps(fsl[finite], org[finite])
        n_of, _u_of = _ulps(fsl[finite], ofix[finite])
        same_nan = bool(np.array_equal(np.isnan(fsl), np.isnan(rep)))
        same_nan_o = bool(np.array_equal(np.isnan(fsl), np.isnan(org)))
        named = info.get("named", {}).get(f"replica-fix-{rid}", [])
        if fn in IBM:
            verdict = "divergence" if n_r or n_o else "bit-identical"
            how = "correctly rounded in both modes (IBM's code in glibc, LGPL, never translated)"
        else:
            verdict = "bit-identical" if n_r == 0 and n_o == 0 else "open"
            how = "glibc's bits with reproduce, correctly rounded by default (GLIBC-07 named)"
        rows.append({
            "head": f"`{fn}`",
            "sub": "`fslmaths " + " ".join(glibc_args(rid)) + "`",
            "cells": [common.cell(int(finite.sum()), "count"), common.cell(n_r, "count"),
                      common.cell(u_r, "count"), common.cell(n_f, "count"),
                      common.cell(", ".join(named) or "–"), common.cell(n_o, "count"),
                      common.cell(u_o, "count"), common.cell(n_of, "count"),
                      common.cell(how)],
            "verdict": verdict,
        })  # fmt: skip
        assert same_nan and same_nan_o, rid
    table = common.table(
        "glibc-double",
        "GLIBC-07 on a real image: glibc 2.35's double functions",
        ["Function", {"label": "Values", "num": True},
         {"label": "Replica, reproduce mode: values that differ from FSL's", "num": True},
         {"label": "Largest distance (units in the last place)", "num": True},
         {"label": "Replica, default mode: values that differ from FSL's", "num": True},
         "Named by the replica's default",
         {"label": "Clean-room original, reproduce mode: values that differ", "num": True},
         {"label": "Largest distance (units in the last place)", "num": True},
         {"label": "Clean-room original, default mode: values that differ", "num": True},
         "The maths (both implementations)"],
        rows,
        note="fslmaths with `-dt double` and `-odt double` on FSL's mean of ds005040's BOLD "
        "series in T1w space, scaled into each function's range; every finite value of the "
        "output compared bit for bit with FSL 6.0.7.23's (through the oracle cache: glibc 2.35's "
        "functions as FSL's build calls them, the FMA builds this CPU selects). Where glibc's "
        "result is not correctly rounded and the replica's is, they differ by one unit in the "
        "last place: GLIBC-07, an expected divergence for IBM's six functions (no permissive "
        "code gives glibc's bits), a correction by default for exp and log, whose glibc bits "
        "the reproduce mode gives. Float results (fslmaths' default type) are the same: no "
        "float differs after rounding. The clean-room original follows the same rule (CORE-MATH's "
        "functions; glibc's exp, log and pow bits in its reproduce mode, from their documented "
        "behaviour), at 14 threads.",
    )  # fmt: skip
    return table, fcache.provenance(e)


# ------------------------------------------------------------------------------ shared texts


def note(n: int, extra: str = "") -> str:
    return (
        f"Δ is A − B, computed in float64 over all {n:,} voxels of the outputs, not only those "
        "shown. The record's threshold: every file fslmaths writes bit-identical (images "
        "decompressed, `descrip` left out), with FSL's exit status and messages." + extra
    )


def programs(info: dict) -> str:
    vers = info.get("versions", {})
    served = info.get("oracle_cache", {})
    n = len(served)
    k = sum(1 for v in served.values() if v == "served")
    how = (
        "served from the oracle cache, as the record ran it"
        if n and k == n
        else f"through the oracle cache ({k} of {n} runs served, the others run for this page)"
    )
    return (
        f"{vers.get('fsl', '?')} ({how}); {vers.get('larmorx-nc', '?')} (the replica), from "
        f"larmorx at commit `{info['larmorx_commit'][:12]}`"
    )


#: Why the default mode differs, per register entry (its notices name them).
FIX_REASONS = {
    "FSL-151": "FSL-151, corrected by default with a notice: a value converted to an integer "
    "type outside its range (or NaN) wraps around in FSL; the default clamps it to the type's "
    "range (NaN to 0).",
    "FSL-167": "FSL-167, corrected by default with a notice: the standard deviation along a "
    "dimension takes the variance without the clamp at 0 and the guard for one value that "
    "the masked variance has; the default takes the masked form.",
    "GLIBC-07": "GLIBC-07, corrected by default with a notice: glibc 2.35's double exp, log and "
    "pow are not always correctly rounded; the default's are, and the reproduce mode gives "
    "glibc's bits.",
}


def verdicts(diffs: list[dict], same: dict[str, bool], info: dict, what: str) -> None:
    named = [n for n in info.get("named", {}).get("replica-fix", [])]
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "replica-fsl": f"The same values, every one, in every output {what} writes (the "
                "record compares every file, the messages and the exit status too).",
                "replica-own": "fMRIPrep's own file (fslmaths in fMRIPrep 25.2.6's image) holds "
                "the same values.",
                "fix-replica": "No correction of the register applies to these calls: both modes "
                "give the same values."
                if not named
                else "No correction of the register changes these values: the default mode "
                f"names {', '.join(named)}, with the same values in both modes.",
            }.get(i, "The same values, every one.")
        elif i == "fix-replica" and named:
            x["verdict"] = "d13"
            x["reason"] = " ".join(FIX_REASONS.get(n, f"{n}, corrected by default.") for n in named)
            for n in named:
                x["links"].append({"label": n, "href": "docs/findings/upstream-defects.md"})
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example (see the record)."


# ------------------------------------------------------------------------------------ cases


def case_table() -> dict[str, tuple[str, tuple]]:
    """Every case of the suite: id -> (description, payload)."""
    out: dict[str, tuple[str, tuple]] = {}
    for c in _suite().suite().cases("full"):
        out.setdefault(c.id, (c.description, c.payload))
    return out


def _input(name: str) -> str:
    if name.startswith("dt:"):
        return f"<random testing's input {name.split(':', 1)[1][:12]}>"
    return f"<{name}: a synthetic input>"


def _arg(a: str) -> str:
    import shlex

    if a.startswith("@"):
        return _input(a[1:])
    return shlex.quote(a) if re.search(r"\s", a) or a == "" else a


def command(case: str, defs: dict, program: str = "fslmaths") -> str:
    if case not in defs:
        return ""
    kind, payload = defs[case][1]
    if kind == "fmriprep":
        return f"{program} <BET's first-pass mask> -mul <the second mask> <out>"
    if kind == "goodvoxels":
        _run, node = payload
        template = next(t for n, _, t in _suite().GOODVOXELS_STEPS if n == node)
        parts = [f"<{a[1:-1].split(':')[0]}>" if a.startswith("{") else a for a in template]
        return f"{program} {' '.join(parts)} {node}.nii.gz"
    if kind == "bet":
        return f"{program} <fMRIPrep's preprocessed T1w> -l 0.001 -C"
    run = payload if kind == "direct" else _suite_of(program)._cases()[payload][2]
    parts = [program, *(_arg(a) for a in run.args)]
    env = " ".join(f"{k}={v}" for k, v in run.settings if v is not None)
    unset = " ".join(f"{k} unset" for k, v in run.settings if v is None)
    return " ".join(x for x in (env, " ".join(parts), f"({unset})" if unset else "") if x)


def _suite_of(program: str):
    if program == "fslmaths":
        return _suite()
    from larmorx_validation.parity import fsl_stats_replica as st

    return st


#: The reasons of the record's expected divergences, in plain words.
DIVERGENCE_REASONS = {
    "fn/double-tan": "GLIBC-07: with -dt double, glibc 2.35's tan (IBM's code in glibc, LGPL) "
    "is not always correctly rounded and the replica's is, in both modes: here 3 of 210 double "
    "values one unit in the last place from FSL's (tan differs on 0.21 % of image intensities). "
    "No permissive code gives glibc's bits; an exact reproduction is planned by a clean-room "
    "implementation of glibc's behaviour.",
    "difftest/fft-twiddles-glibc-sincos": "GLIBC-07, from random testing: fslmaths convolves "
    "this kernel by the FFT, whose twiddle factors FSL's library takes from glibc's sincos "
    "(IBM's code, LGPL, not always correctly rounded: 0.148 % of the angles), while the "
    "replica computes them correctly rounded, in both modes: 1 of 90 float voxels one unit in "
    "the last place. Given FSL's own factors the replica's output is FSL's bit for bit (the FFT "
    "probe: every length from 1 to 256, and this case). Never met on fMRIPrep's calls (they "
    "convolve by separable sums).",
}


def case_verdict(r: dict, dm: dict) -> tuple[str, str]:
    """A case's verdict and reason from the record and the replica's default mode on it
    (``dm``: its exit status and the entries it names)."""
    named, code = dm.get("named", []), dm.get("exit")
    v = common.outcome_verdict(r)
    if v is not None:
        verdict, why = v
        if verdict == "divergence" and r["case"] in DIVERGENCE_REASONS:
            why = DIVERGENCE_REASONS[r["case"]]
        if verdict == "both-reject" and named:
            why += (
                f" The replica's default mode finishes instead (exit status 0), naming "
                f"{', '.join(named)}."
                if code == 0
                else f" The replica's default mode ends with exit status {code}, naming "
                f"{', '.join(named)}."
            )
        return verdict, common.clip(why, 700)
    rep = {**r.get("reported", {}), **r.get("measured", {})}
    files = rep.get("every file bit-identical")
    notes = [
        f"every file written{f' ({files})' if files else ''} byte for byte, with FSL's exit "
        "status and messages"
    ]
    own = rep.get("fMRIPrep's own mask (its container's FSL 6.0.7.x) has the same values")
    if own:
        notes.append("fMRIPrep's own mask: the same values" if own == "yes" else
                     "fMRIPrep's own mask differs")  # fmt: skip
    fix = rep.get("D13 fix pass")
    if isinstance(fix, int) and fix:
        notes.append(
            f"in its default mode the replica corrects {', '.join(named) or 'a register entry'} "
            f"and names it: {fix:,} values change"
        )
    elif named and code not in (0, None):
        notes.append(f"its default mode refuses the call (exit {code}), naming {', '.join(named)}")
    elif named:
        notes.append(f"its default mode names {', '.join(named)}, with the same values")
    return "bit-identical", common.clip("; ".join(notes), 600)


def _default_args(kind: str, payload, d: Path, program: str) -> list[str]:
    """A case's arguments with its inputs in ``d`` (for the replica's default-mode run)."""
    su = _suite_of(program)
    if kind == "fmriprep":
        run, node = payload
        return _suite().fmriprep_args(run, Path(node))
    if kind == "goodvoxels":
        run, node = payload
        fs = _suite()
        base = fs._goodvoxels_chain(run)
        values = json.loads((base / "values.json").read_text(encoding="utf-8"))
        if program == "fslmaths":
            template = next(t for n, _, t in fs.GOODVOXELS_STEPS if n == node)
            return [*fs._goodvoxels_args(template, _goodvoxels_files(base), values, d),
                    f"{node}.nii.gz"]  # fmt: skip
        template = next(t for n, _, t in fs.GOODVOXELS_STEPS if n == node)
        src = template[0][1:-1]
        shutil.copy(base / src / f"{src}.nii.gz", d / f"{src}.nii.gz")
        return [f"{src}.nii.gz", *template[1:]]
    if kind == "bet":
        shutil.copy(su._bet_image(), d / "t1w.nii.gz")
        return ["t1w.nii.gz", "-l", "0.001", "-C"]
    run = payload if kind == "direct" else su._cases()[payload][2]
    return su._prepare(run, d)


def _settings(kind: str, payload, program: str) -> dict:
    if kind in ("run", "direct"):
        run = payload if kind == "direct" else _suite_of(program)._cases()[payload][2]
        return dict(run.settings)
    return {}


def compute_default_mode(e: fcache.Entry, record: str, program: str) -> None:
    """The replica's default mode on every case of ``record`` (no FSL run): its exit status and
    the register entries it names, for the case explorer's reasons."""
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    rec = records.load(e.ctx.repo, record)
    defs = case_table() if program == "fslmaths" else _stats_case_table()
    exe = _suite().replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    default: dict[str, dict] = {}
    for r in rec.results:
        case = r["case"]
        kind, payload = defs[case][1]
        with tempfile.TemporaryDirectory(prefix="lx-nc-maths-ev-named_") as t:
            d = Path(t)
            args = _default_args(kind, payload, d, program)
            env = {**_environment(_settings(kind, payload, program)),
                   "LARMORX_UPSTREAM_DEFECTS": "fix", "OMP_NUM_THREADS": "1"}  # fmt: skip
            label = case.replace("/", "-")
            x = e.run(label, [str(exe), "fsl", program, *args], env=env, unset=unset_env(),
                      cwd=d, check=False, timeout=600)  # fmt: skip
            log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
            default[case] = {"exit": x.exit, "named": sorted(set(NOTICE.findall(log)))}
    e.finish(
        versions=versions(),
        default_mode=default,
        settings=f"The larmorx-nc replica's {program} in its default mode on every case of the "
        "record, in a fresh directory with the case's inputs: its exit status and the register "
        "entries its notices name (no FSL run; the record's fix pass checks that a name appears "
        "exactly when the default's outputs differ)",
    )


def _stats_case_table() -> dict[str, tuple[str, tuple]]:
    from larmorx_validation.parity import fsl_stats_replica as st

    out: dict[str, tuple[str, tuple]] = {}
    for c in st.suite().cases("full"):
        out.setdefault(c.id, (c.description, c.payload))
    return out


def named_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "cases-default-mode",
        "The replica's default mode on every case of the record: its exit status and the "
        "entries it names", lambda e: compute_default_mode(e, RECORD, "fslmaths"),
    )  # fmt: skip


def cases(rec, defs: dict, examples: dict[str, str], default: dict[str, dict],
          program: str = "fslmaths") -> list[dict]:  # fmt: skip
    out = []
    for r in rec.results:
        desc, _payload = defs.get(r["case"], ("", None))
        v, why = case_verdict(r, default.get(r["case"], {}))
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": desc,
                "command": command(r["case"], defs, program),
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


# ------------------------------------------------------------------------------ the trial


def trial(ctx: core.Context, tool: str) -> tuple[dict, dict]:
    """Random testing's trial of this part (``tool``: fsl-maths or fsl-stats), from its
    ``summary.json``: a table, and its provenance."""
    del ctx
    path = common.oracles() / "difftest" / TRIAL / "summary.json"
    raw = path.read_bytes()
    s = json.loads(raw)
    t = s["tools"][tool]
    impl = t["implementations"]["replica"]
    o = t["options"]
    rows = [
        (f"Random cases of {'fslmaths' if tool == 'fsl-maths' else 'fslstats'}, each compared "
         "with FSL 6.0.7.23", t["cases"]),
        ("bit-identical (every file, the messages, the exit status)",
         impl.get("agree", 0) + impl.get("notice", 0)),
        ("of them, a notice of the default mode without a visible change", impl.get("notice", 0)),
        ("differing at the trial (each triaged below)", impl.get("differ", 0)),
        ("distinct options drawn", t["options_covered"]),
    ]  # fmt: skip
    if tool == "fsl-maths":
        rows += [
            ("chains with -fmean or -fmeanu (convolutions, the FFT's among them)",
             o.get("-fmean", 0) + o.get("-fmeanu", 0)),
            ("-dt double", o.get("-dt double", 0)),
        ]  # fmt: skip
    else:
        rows += [
            ("-t (per volume), -K (per label), -json",
             [o.get("-t", 0), o.get("-K", 0), o.get("-json", 0)]),
        ]  # fmt: skip
    rows.append((f"length (minutes; {s['jobs']} jobs at one thread each, both programs)",
                 round(s["elapsed_s"] / 60, 1)))  # fmt: skip
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
    triage = TRIAGE[tool]
    table = common.table(
        "trial",
        f"Random testing: the trial of {s['date']}",
        ["", {"label": "Cases", "num": True}],
        out_rows,
        note=f"Differential random testing's trial of this part (seed `{s['seed']}`, larmorx "
        f"`{s['larmorx_commit']}`): {DRAWN[tool]} Every case in a memory box of "
        f"{s['memory']['case']} (peaks below {peaks:.2f} GiB), FSL through the oracle cache "
        f"(`use` mode). {triage}",
    )  # fmt: skip
    prov = {"trial": f"oracles/difftest/{TRIAL}/summary.json",
            "sha256": hashlib.sha256(raw).hexdigest()}  # fmt: skip
    return table, prov


#: What the trial draws, per tool.
DRAWN = {
    "fsl-maths": "chains of one to four operations drawn for the current image (every family), "
    "on six kinds of images (scan-like, series, masks, z and p maps, tensors) on grids of 6 to "
    "32 voxels per axis, with `-dt`, `-odt`, the output's name and FSLOUTPUTTYPE.",
    "fsl-stats": "one to five options (or fMRIPrep's and BET's forms) with `-t`, `-K` and "
    "`-json`, on the same kinds of images and masks.",
}
#: What was done about the trial's differences (``validation/difftest/triage.toml``).
TRIAGE = {
    "fsl-maths": "Its one difference (a convolution by the FFT of a 30 × 1 × 3 image: 1 of 90 "
    "voxels one unit in the last place) is GLIBC-07, the twiddle factors FSL takes from glibc's "
    "sincos: a permanent case of the record, an expected divergence. The notices without a "
    "visible change name a correction of an intermediate result that a later operation removes "
    "(a threshold, -bin, -restart, a reduction). The check run before the trial found three "
    "mistakes of the replica's, fixed with permanent cases.",
    "fsl-stats": "Its nine differences were mistakes of the replica's, fixed with permanent "
    "cases: the sign of NaN in -c and -C with NaN and infinities in the image (FSL's build adds "
    "each block's partial sums in its own order), and -json's notices before an error.",
}


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, RECORD)
    defs = case_table()
    combine_case = next(
        r["case"] for r in rec.results
        if r["case"].startswith(f"fmriprep/{RUN}/") and "enhance_and_skullstrip" in r["case"]
    )  # fmt: skip
    examples = {combine_case: "combine"}
    examples.update({r["case"]: "goodvoxels" for r in rec.results
                     if r["case"].startswith("fmriprep-goodvoxels/")})  # fmt: skip
    ne = named_entry(ctx)
    cs = cases(rec, defs, examples, ne.info().get("default_mode", {}))
    rec_o, de = fo.add_cases(ctx, cs, "fslmaths", examples)
    entries = [combine_entry(ctx), goodvoxels_entry(ctx), families_entry(ctx), glibc_entry(ctx),
               ne]  # fmt: skip
    entries += [fo.entry(ctx, TOOL, w) for w in ("combine", "goodvoxels", "families", "glibc")]
    entries.append(de)
    tt, tprov = trial(ctx, TOOL)
    to, toprov = fo.trial(ctx, TOOL)
    gt, _gprov = _glibc_table(ctx)
    bench, chart, bt = fo.speed(ctx, "fslmaths")
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "fslmaths",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source() + rec_o.source(),
            "benchmarks": bench,
            "cache": [fcache.provenance(x) for x in entries],
            "difftest": tprov,
            "difftest_original": toprov,
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cs,
                "replica",
                "larmorx-nc replica, against FSL 6.0.7.23 (standard tier)",
                "replica",
                f"docs/validation/{RECORD}.md",
                "experimental",
            ),
            fo.implementation(cs, "fslmaths"),
        ],
        "cases": cs,
        "cases_total": len(cs),
        "speed": chart,
        "tables": [tt, gt, *to, bt],
        "examples": [
            _example_combine(ctx, rec),
            _example_goodvoxels(ctx),
            _example_families(ctx),
        ],
    }


if __name__ == "__main__":  # compute the cache entries only:
    # [combine|goodvoxels|families|glibc|named|original-<which>|original-default]
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    what = sys.argv[1:] or ["combine", "goodvoxels", "families", "glibc", "named"]
    fns = {"combine": combine_entry, "goodvoxels": goodvoxels_entry, "families": families_entry,
           "glibc": glibc_entry, "named": named_entry,
           "original-default": lambda x: fo.default_entry(x, "fslmaths")}  # fmt: skip
    for w in ("combine", "goodvoxels", "families", "glibc"):
        fns[f"original-{w}"] = lambda x, w=w: fo.entry(x, TOOL, w)
    for w in what:
        print("done", fns[w](c).dir, flush=True)
