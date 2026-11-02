# SPDX-License-Identifier: Apache-2.0
"""FAST's evidence (the larmorx-nc replica, FSL Licence; the clean-room original is still to be
written): fMRIPrep's tissue segmentation call (``anat_fit_wf/fast``: ``fast -I 0 -N -p -g -S 1
<T1w brain>``) on three oracle runs (an adult, and the data panel's 3.5-year-old and glioma
patient), the bias field FAST estimates by default, FSL's tissue priors (``-a``, where the
replica's default corrects FSL-09 inside FLIRT's resampling of the priors) and a three-channel
segmentation, by FSL 6.0.7.23 and by the replica in its ``reproduce`` and default modes (and at
one thread); every file FAST writes, the tissue volumes the partial-volume maps give, fMRIPrep's
own outputs, and every case of the record with its verdict.

Every run of FSL is a run of the parity record (``parity.fsl_fast_replica``), with the same
arguments, environment and inputs, through the oracle-output cache (``use`` mode): the page
computes no original result of its own. The replica runs as a separate program; nothing here
imports it. Everything shown lies inside the brain: fMRIPrep's skull-stripped T1w (dilated by
two voxels), or the template's brain mask: no face can show.
"""

from __future__ import annotations

import copy
import gzip
import os
import re
import shlex
import shutil
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "fsl-fast"
RECORD = "fsl-fast-replica"
#: fMRIPrep's calls shown as examples: example id, the fMRIPrep run, its view, the words.
FMRIPREP = [
    ("fmriprep-fast", "ds000005-sub-01-noreconall", "volume", "an adult's T1w (ds000005)"),
    ("panel-ds000228-child", "ds000228-sub-pixar008-noreconall", "slices",
     "a 3.5-year-old's T1w (ds000228)"),
    ("panel-ds005003-glioma", "ds005003-sub-RS001-noreconall", "slices",
     "a glioma patient's T1w (ds005003)"),
]  # fmt: skip
#: Where a data-panel example's three slices go (a file voxel of fMRIPrep's T1w brain), when not
#: through the brain's centre: ds005003's tumour (the data panel's "70 mL right frontal
#: oligodendroglioma"; a voxel inside its hypointense core, chosen on the T1w at slice 172).
VIEW_AT = {"ds005003-sub-RS001-noreconall": (45, 150, 172)}
#: The record's other cases shown as examples: example id -> case.
REAL = {"bias-field": "opt/bias-outputs", "priors": "priors/a-verbose", "multi-channel": "multi/s3"}
#: The replica's runs: label, D13 mode, threads.
REPLICA_RUNS = [("replica", "reproduce", "4"), ("replica-1", "reproduce", "1"),
                ("replica-fix", "fix", "4")]  # fmt: skip
PANELS = [
    ("fsl", "original", "FSL 6.0.7.23, fast", "the conda installation, as built (the record's "
     "run, from the oracle cache)"),
    ("replica", "larmorx", "larmorx-nc replica", "reproduce mode, 4 threads"),
    ("replica-1", "larmorx", "larmorx-nc replica", "reproduce mode, 1 thread"),
    ("replica-fix", "larmorx", "larmorx-nc replica", "default mode (D13 corrections), 4 threads"),
]  # fmt: skip
#: The tissues of a three-class T1-weighted segmentation (``-t 1``: classes by intensity).
TISSUES = ("CSF", "grey matter", "white matter")
#: The order of the outputs in the viewer (by their suffix after the output's base name).
ORDER = ("seg", "pve_0", "pve_1", "pve_2", "pveseg", "prob_0", "prob_1", "prob_2", "mixeltype",
         "seg_0", "seg_1", "seg_2", "bias", "restore", "bias_1", "bias_2", "bias_3",
         "restore_1", "restore_2", "restore_3")  # fmt: skip


def _data(run: str) -> dict:
    if run.startswith("ds000005-"):
        return common.DS000005
    from larmorx_validation.evidence.ants_panel import data

    return data(run.split("-sub-")[0])


# --------------------------------------------------------------------- running the programs


def _suite():
    from larmorx_validation.parity import fsl_fast_replica as fs

    return fs


def _oracle(
    e: fcache.Entry,
    label: str,
    args: list[str],
    put_inputs,
    settings: dict[str, str | None],
    never: str = "",
) -> None:
    """FSL's fast with ``args`` through the oracle-output cache, its working directory holding
    the inputs ``put_inputs(dir)`` copies there (as the record's case does); its files, the
    inputs left out, into ``<entry>/<label>``; whether it was served is kept in the entry."""
    fs = _suite()
    with tempfile.TemporaryDirectory(prefix="lx-nc-fast-ev_") as t:
        cwd = Path(t)
        put_inputs(cwd)
        given = {p.name for p in cwd.iterdir()}
        p = fs.oracle_run(args, cwd=cwd, settings=settings, never=never)
        if p.returncode != 0:
            raise RuntimeError(f"{label}: FSL's fast exited {p.returncode}: {p.stderr[-800:]}")
        out = e.dir / label
        out.mkdir(exist_ok=True)
        for f in cwd.iterdir():
            if f.name not in given:
                shutil.copy(f, out / f.name)
        (out / "stdout.txt").write_text(p.stdout, encoding="utf-8")
    from larmorx_validation.parity.fsl_flirt_replica import _environment, fsldir

    run = fcache.Run(label, [str(fsldir() / "bin" / "fast"), *args], _environment(settings))
    run.exit, run.seconds = p.returncode, round(p.seconds, 2)
    run.stderr_tail = p.stderr[-2000:]
    e.runs.append(run)
    e.extra.setdefault("oracle_cache", {})[label] = "served" if p.cached else "ran"


def _replica(e: fcache.Entry, args: list[str], put_inputs, settings: dict[str, str | None]) -> None:
    """The replica on ``args``, once per :data:`REPLICA_RUNS`, each in ``<entry>/<label>`` (it
    writes its outputs where FSL does: its working directory); the inputs removed after."""
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    fs = _suite()
    exe = fs.replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    env = _environment(settings)
    unset = tuple(k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_", "OMP_")))
    for label, mode, threads in REPLICA_RUNS:
        cwd = e.dir / label
        cwd.mkdir(exist_ok=True)
        put_inputs(cwd)
        given = [p for p in cwd.iterdir()]
        run_env = {**env, "LARMORX_UPSTREAM_DEFECTS": mode, "OMP_NUM_THREADS": threads}
        e.run(label, [str(exe), "fsl", "fast", *args], env=run_env, unset=unset, cwd=cwd)
        log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
        e.extra.setdefault("named", {})[label] = sorted(set(NOTICE.findall(log)))
        for p in given:
            p.unlink()


def versions() -> dict[str, str]:
    from larmorx_validation.parity import fsl_versions
    from larmorx_validation.parity.fsl_flirt_replica import fsldir
    from larmorx_validation.parity.fsl_mcflirt_replica import replica_version

    return {
        "fsl": f"FSL {fsl_versions.version_of(fsldir())} (fsl-fast4 2111.3; ~/fsl-6.0.7.23, "
        "never on the PATH)",
        "larmorx-nc": replica_version(),
    }


#: A notice of the replica's (``larmorx-nc: upstream defect FSL-xx (...)``).
NOTICE = re.compile(r"upstream defect (FSL-\d+) \(")


def _node(run: str) -> Path:
    nodes = [n for r, n in _suite().fmriprep_calls() if r == run]
    if not nodes:
        raise FileNotFoundError(f"fMRIPrep's FAST node of {run}")
    return nodes[0]


def _compute_fmriprep(run: str):
    def compute(e: fcache.Entry) -> None:
        fs = _suite()
        node = _node(run)
        args = fs.fmriprep_args(node)
        src = fs.fmriprep_input(run, str(node))
        if src is None:
            raise FileNotFoundError(f"the input of fMRIPrep's FAST node of {run}")
        e.input("T1w brain (fMRIPrep's)", src)

        def put(d: Path) -> None:
            shutil.copy(src, d / args[-1])

        _oracle(e, "fsl", args, put, {})
        _replica(e, args, put, {})
        e.finish(
            versions=versions(),
            command=(node / "command.txt").read_text(encoding="utf-8").strip(),
            node=str(node.relative_to(common.fmriprep_run(run))),
            input=args[-1],
            settings="fMRIPrep 25.2.6's FAST node (anat_fit_wf/fast) of the run, its arguments "
            "from command.txt, its input (the skull-stripped T1w) found by name in the run's "
            "working directory and copied into the run directory as the record's case does; "
            "FSL 6.0.7.23 through the oracle-output cache (the record's own run) and the "
            "larmorx-nc replica: reproduce mode at 4 and 1 threads, the default mode at 4",
        )

    return compute


def fmriprep_entry(ctx: core.Context, run: str) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"fmriprep-{run}",
        f"fMRIPrep's FAST call of {run}: FSL 6.0.7.23 (the record's run, from the oracle "
        "cache) and the replica, reproduce and default modes",
        _compute_fmriprep(run),
    )


def _real_run(case: str):
    return _suite()._all_cases()[case][2]


def case_table() -> dict[str, tuple[str, object]]:
    """Every case of the suite, random testing's permanent cases included: id ->
    (description, its ``Run``; None for fMRIPrep's calls)."""
    from larmorx_validation.difftest import permanent

    s = _suite().suite()
    permanent.attach(s)
    out: dict[str, tuple[str, object]] = {}
    for c in s.cases("full"):
        kind, payload = c.payload
        run = (
            None
            if kind == "fmriprep"
            else payload
            if kind == "direct"
            else _suite()._all_cases()[payload][2]
        )
        out.setdefault(c.id, (c.description, run))
    return out


def _compute_real(case: str):
    def compute(e: fcache.Entry) -> None:
        fs = _suite()
        run = _real_run(case)
        for a in run.args:
            if a.startswith("@"):
                e.input(a[1:], fs.inputs() / a[1:])
        # the arguments as the record's case gives them (an input by its file name)
        args = [Path(a[1:]).name if a.startswith("@") else a for a in run.args]

        def put(d: Path) -> None:
            fs._prepare(run, d)

        settings = dict(run.settings)
        _oracle(e, "fsl", args, put, settings, never=run.never)
        _replica(e, args, put, settings)
        e.finish(
            versions=versions(),
            command="fast " + " ".join(a.lstrip("@") for a in run.args),
            settings=f"The record's case {case}, its inputs from the suite's derived inputs "
            "(oracles/inputs/fsl-fast-replica/), by FSL 6.0.7.23 through the oracle-output "
            "cache (the record's own run) and by the larmorx-nc replica: reproduce mode at 4 "
            "and 1 threads, the default mode at 4",
        )

    return compute


def real_entry(ctx: core.Context, case: str) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        case.replace("/", "-"),
        f"The record's case {case}: FSL 6.0.7.23 (from the oracle cache) and the replica",
        _compute_real(case),
    )


def _compute_named(e: fcache.Entry) -> None:
    """The replica's default mode on every case of the record (no FSL run): its exit status and
    the register entries it names, for the case explorer's reasons."""
    fs = _suite()
    rec = records.load(e.ctx.repo, RECORD)
    defs = case_table()
    nodes = {f"fmriprep/{r}": n for r, n in fs.fmriprep_calls()}
    exe = fs.replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    unset = tuple(k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_", "OMP_")))
    default: dict[str, dict] = {}
    for r in rec.results:
        case = r["case"]
        with tempfile.TemporaryDirectory(prefix="lx-nc-fast-ev-named_") as t:
            d = Path(t)
            if case.startswith("fmriprep/"):
                node = nodes[case]
                args = fs.fmriprep_args(node)
                shutil.copy(fs.fmriprep_input(case.split("/")[1], str(node)), d / args[-1])
                settings: dict[str, str | None] = {}
            else:
                run = defs[case][1]
                args = fs._prepare(run, d)
                settings = dict(run.settings)
            env = {**_environment(settings), "LARMORX_UPSTREAM_DEFECTS": "fix",
                   "OMP_NUM_THREADS": "4"}  # fmt: skip
            label = case.replace("/", "-")
            x = e.run(label, [str(exe), "fsl", "fast", *args], env=env, unset=unset, cwd=d,
                      check=False)  # fmt: skip
            log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
            default[case] = {"exit": x.exit, "named": sorted(set(NOTICE.findall(log)))}
    e.finish(
        versions=versions(),
        default_mode=default,
        settings="The larmorx-nc replica in its default mode (4 threads) on every case of the "
        "record, in a fresh directory with the case's inputs: its exit status and the register "
        "entries its notices name (no FSL run; the record's fix pass checks that a name appears "
        "exactly when the default's outputs differ)",
    )


def named_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        "cases-default-mode",
        "The replica's default mode on every case of the record: its exit status and the "
        "entries it names",
        _compute_named,
    )


# ------------------------------------------------------------------------- the files


def _base(run_dir: Path, inputs: set[str]) -> str:
    """The outputs' base name: the name before ``_seg`` (``-o``, else the input's)."""
    seg = next(p for p in sorted(run_dir.glob("*_seg.nii.gz")) if p.name not in inputs)
    return seg.name[: -len("_seg.nii.gz")]


def _files(run_dir: Path, base: str) -> dict[str, Path]:
    """FAST's images in ``run_dir``, by their suffix after ``base`` (the registered priors FAST
    leaves behind, ``*_stdspace``, apart)."""
    out = {}
    for p in sorted(run_dir.glob(f"{base}_*.nii.gz")):
        sfx = p.name[len(base) + 1 : -len(".nii.gz")]
        if "stdspace" not in sfx:
            out[sfx] = p
    return {k: out[k] for k in sorted(out, key=lambda k: ORDER.index(k) if k in ORDER else 99)}


#: The tissues' short names in the viewer's output selector.
SHORT = ("CSF", "GM", "WM")


def _label(sfx: str, channels: list[str] | None = None) -> str:
    """An output's name in the viewer (short: the selector holds up to 12; the files' names are
    in the table below the viewer)."""
    m = re.fullmatch(r"(seg|pve|prob)_(\d)", sfx)
    if m:
        what = {"seg": "Binary", "pve": "Partial volume", "prob": "Probability"}[m.group(1)]
        k = int(m.group(2))
        return f"{what}: {SHORT[k] if k < len(SHORT) else f'class {k}'}"
    m = re.fullmatch(r"(bias|restore)_(\d)", sfx)
    if m:
        k = int(m.group(2))
        name = channels[k - 1] if channels and k <= len(channels) else f"channel {k}"
        return f"{'Bias field' if m.group(1) == 'bias' else 'Corrected'}: {name}"
    return {
        "seg": "Classes (1 CSF, 2 GM, 3 WM)",
        "pveseg": "Classes of the partial volumes",
        "mixeltype": "Mixel type",
        "bias": "Bias field",
        "restore": "Corrected image",
    }.get(sfx, f"_{sfx}")


def _outputs(files: dict[str, Path], channels: list[str] | None = None) -> list[dict]:
    return [{"id": k, "label": _label(k, channels), "unit": ""} for k in files]


def _nifti(path: Path) -> bytes:
    raw = path.read_bytes()
    return gzip.decompress(raw) if path.name.endswith(".gz") else raw


def _same_file(a: Path, b: Path) -> tuple[bool, bool]:
    """Two NIfTI-1 files: (the same data, the same header but ``descrip``): bytes, decompressed."""
    import struct

    x, y = _nifti(a), _nifti(b)
    off = int(struct.unpack_from("<f", x, 108)[0])
    hx = x[:148] + x[228:off]
    hy = y[:148] + y[228:off] if len(y) >= off else b""
    return x[off:] == y[off:], hx == hy


def _files_table(tid: str, files: dict[str, Path], run_dir, labels: dict[str, str]) -> dict:
    """Every image FAST wrote, FSL's against each run: the data and the header (``descrip``,
    FSL's build string, aside), byte for byte."""
    import nibabel as nib

    rows = []
    for f in files.values():
        img = nib.load(str(f))
        cells = [common.cell(str(img.get_data_dtype())),
                 common.cell(" × ".join(map(str, img.shape)))]  # fmt: skip
        for pid in labels:
            data, head = _same_file(f, run_dir(pid) / f.name)
            cells.append(common.cell("identical" if data and head else
                                     "data identical" if data else "differs"))  # fmt: skip
        rows.append({"head": f"`{f.name}`", "cells": cells})
    return common.table(
        tid,
        "Every image FAST writes, against FSL 6.0.7.23's",
        ["File", "Type", "Voxels", *labels.values()],
        rows,
        note="Each file decompressed and compared byte for byte with FSL's: *identical*, the "
        "header (its `descrip`, FSL's build string, aside) and every value; *data identical*, "
        "every value but not the header; *differs*, values that differ (see the comparisons "
        "above).",
    )


def _volumes_table(tid: str, pve: dict[str, dict[str, np.ndarray]], vox_ml: float, note: str):
    """The tissue volumes the partial-volume maps give (ml): sum of each map × the voxel."""
    rows = []
    ref = None
    for head, maps in pve.items():
        vals = [float(np.sum(maps[f"pve_{k}"], dtype=np.float64)) * vox_ml for k in range(3)]
        ref = ref or vals
        rows.append({"head": head, "cells": [common.cell(v, "sig6") for v in vals]
                     + [common.cell("the same" if vals == ref else "differs")]})  # fmt: skip
    return common.table(
        tid,
        "The tissue volumes of the partial-volume maps (ml)",
        ["Run", *[{"label": t, "num": True} for t in TISSUES], "against FSL's"],
        rows,
        note=note,
    )


# -------------------------------------------------------------------------- examples


def _diffs(own: bool) -> list[dict]:
    d = common.d
    out = [
        d("replica-fsl", "replica", "fsl", "main", "replica − FSL 6.0.7.23", common.ROLE_LX),
        d("replica1-replica", "replica-1", "replica", "check",
          "replica, 1 thread − 4 threads", common.ROLE_SELF),
        d("fix-replica", "replica-fix", "replica", "check",
          "replica, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    if own:
        out.append(d("replica-own", "replica", "own", "fmriprep",
                     "replica − fMRIPrep's own output", common.ROLE_FMRIPREP))  # fmt: skip
    return out


def _verdicts(diffs: list[dict], same: dict[str, bool], what: str, info: dict, bias: bool):
    named = info.get("named", {})
    fix_named = [n for n in named.get("replica-fix", []) if n.startswith("FSL-")]
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "replica-fsl": f"The same values, every one, in every image FAST writes on {what} "
                "(the record compares its messages and exit status too).",
                "replica1-replica": (
                    "The thread count changes nothing: the replica uses threads only in the bias "
                    "field's smoothing, whose sums it takes in a fixed order."
                    if bias
                    else "The thread count changes nothing (with -N there is no bias field, the "
                    "only step the replica runs in threads)."
                ),
                "fix-replica": _fix_same(fix_named),
                "replica-own": "fMRIPrep's own files (FSL in fMRIPrep 25.2.6's image), the "
                "partial-volume maps and their segmentation its node keeps, hold the same values.",
            }.get(i, "The same values, every one.")
        elif i == "fix-replica" and fix_named:
            x["verdict"] = "d13"
            reasons = {
                "FSL-09": "FSL-09, corrected by default with a notice: FAST registers FSL's tissue "
                "priors (8-bit integers) onto the image with FLIRT, which writes its resampled "
                "values in that integer type truncated toward zero; the replica's default "
                "rounds them to the nearest integer, so the priors FAST starts from, and its "
                "result, differ. The reproduce mode gives FSL's bytes (the row above).",
                "FSL-98": "FSL-98, corrected by default with a notice: with several channels, "
                "sorting the classes gives each class another class's covariance and posteriors "
                "when the classes move in a cycle; the default gives each class its own.",
                "FSL-103": "FSL-103, corrected by default with a notice: the brain mask leaves "
                "out the voxels equal to the maximum when it is 2^24 or more.",
            }
            x["reason"] = " ".join(reasons.get(n, f"{n}, corrected by default.") for n in fix_named
                                   if n not in ("FSL-96", "FSL-97"))  # fmt: skip
            for n in fix_named:
                x["links"].append({"label": n, "href": "docs/findings/upstream-defects.md"})
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example (see the record)."


def _fix_same(named: list[str]) -> str:
    """The reason of a default mode that gives the reproduce mode's values."""
    if not named:
        return (
            "No correction of the register applies to this call: both modes give the same values."
        )
    what = {
        "FSL-96": "FSL-96 (the probability maps outside the brain mask, which FSL never writes: "
        "0 in both modes)",
        "FSL-97": "FSL-97 (the multi-channel bias field FSL never computes with -N: no field in "
        "both modes)",
    }
    return (
        "No correction of the register changes this call's values: the default mode names "
        + ", ".join(what.get(n, n) for n in named)
        + ", whose values are the same in both modes."
    )


def _note(n: int, extra: str = "") -> str:
    return (
        f"Δ is A − B, computed in float64 over all {n:,} voxels of the outputs, not only those "
        "shown. The record's threshold: every file FSL writes bit-identical (images "
        "decompressed, `descrip` left out), with FSL's exit status and messages." + extra
    )


def _load(run_dir: Path, files: dict[str, Path]) -> dict[str, np.ndarray]:
    return {k: common.load(str(run_dir / f.name)) for k, f in files.items()}


def _programs(info: dict) -> str:
    vers = info.get("versions", {})
    served = info.get("oracle_cache", {})
    return (
        f"{vers.get('fsl', '?')} ("
        + (
            "its run served from the oracle cache, as the record ran it"
            if served.get("fsl") == "served"
            else "run for this page through the oracle cache"
        )
        + f"); {vers.get('larmorx-nc', '?')} (the replica), from larmorx at commit "
        f"`{info['larmorx_commit'][:12]}`"
    )


def _default_view(ex, view: str, a: str, b: str, out: str) -> list[int]:
    if view == "slices":
        return common.slice_voxel(ex, a, b, out)
    at, _k = common.largest_in_slice(ex, a, b, out, "axial")
    return at


def _example_fmriprep(ctx: core.Context, ex_id: str, run: str, view: str, words: str) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = fmriprep_entry(ctx, run)
    info = e.info()
    node = _node(run)
    src = e.dir / "fsl"
    name = info["input"]
    base = _base(src, {name})
    files = _files(src, base)
    t1_path = _suite().fmriprep_input(run, str(node))
    img = nib.load(str(t1_path))
    t1 = common.load(str(t1_path))
    show = ndimage.binary_dilation(t1 > 0, iterations=2)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show, view=view)
    data = _data(run)
    ex.add(exm.Panel("t1w", "input", f"{run.split('-noreconall')[0]} T1w brain",
                     "FAST's input in fMRIPrep: the T1w, denoised, N4-corrected and "
                     "skull-stripped (float32)", {"all": t1}))  # fmt: skip
    arrays = {pid: _load(e.dir / pid, files) for pid, *_ in PANELS}
    for pid, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    own_files = {k: node / f.name for k, f in files.items() if (node / f.name).is_file()}
    own = {k: common.load(str(p)) for k, p in own_files.items()}
    diffs = _diffs(True)
    outputs = _outputs(files)
    common.compare_all(ex, diffs, outputs, full={"own": own})
    same = common.identical(diffs)
    _verdicts(diffs, same, "this call", info, bias=False)
    at = _default_view(ex, view, "fsl", "replica", "pve_1")
    where = "the brain's centre"
    if run in VIEW_AT:
        at = grid.display_index(VIEW_AT[run])
        where = "the tumour"
    default = {
        "output": "pve_1",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1w", "fsl", "replica", "replica-fix"],
        "diffs": ["replica-fsl", "fix-replica"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="fsl",
        caption=(f"the three slices through {where} are stored at full resolution (the "
                 "outputs agree everywhere)." if view == "slices"
                 else f"the crosshair at {where} (the outputs agree everywhere)."),
    )  # fmt: skip
    vox_ml = float(np.prod(img.header.get_zooms()[:3])) / 1000.0
    pve = {
        "FSL 6.0.7.23": arrays["fsl"],
        "replica (reproduce)": arrays["replica"],
        "replica (default)": arrays["replica-fix"],
        "fMRIPrep's own (its image's FSL)": own,
    }
    run_dirs = {"replica": e.dir / "replica", "replica-fix": e.dir / "replica-fix"}
    tables = [
        _files_table(
            f"{ex_id}-files",
            files,
            lambda pid: run_dirs[pid],
            {"replica": "replica (reproduce)", "replica-fix": "replica (default)"},
        ),
        _volumes_table(
            f"{ex_id}-volumes",
            pve,
            vox_ml,
            "Each partial-volume map summed over every voxel in float64, times the voxel's "
            f"volume ({vox_ml * 1000:.4g} mm³); fMRIPrep's own maps are the node's outputs in "
            "the run's working directory (FSL in fMRIPrep 25.2.6's image). *the same*: the "
            "three volumes equal FSL's to the last digit of float64.",
        ),
    ]
    n_vox = int(np.prod(img.shape[:3]))
    brain_ml = float(np.count_nonzero(t1 > 0)) * vox_ml
    title = {
        "fmriprep-fast": "fMRIPrep's tissue segmentation of a T1w brain",
        "panel-ds000228-child": "Data panel: a 3.5-year-old's T1w",
        "panel-ds005003-glioma": "Data panel: a glioma patient's T1w",
    }[ex_id]
    return {
        "id": ex_id,
        "title": title,
        "summary": (
            f"The FAST node fMRIPrep 25.2.6 ran on {words}: `fast -I 0 -N -p -g -S 1 <T1w "
            "brain>` (no bias field: fMRIPrep corrected it with N4 before; the posterior "
            "probabilities and the binary class images written too), given to FSL 6.0.7.23 and "
            "to the larmorx-nc replica in its reproduce and default modes and at one thread. "
            "Choose the output above the panels: the segmentation, the three partial-volume "
            "maps fMRIPrep keeps, the probabilities, the mixel types, the binary classes. "
            "Below the viewer, every file FAST writes and the tissue volumes the maps give, "
            "with fMRIPrep's own."
            + (
                " As a data-panel case beside the first example, it is stored as three "
                "full-resolution slices."
                if view == "slices"
                else ""
            )
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): the T1w "
                "brain fMRIPrep computed (skull-stripped, no face); shown inside its brain, "
                f"dilated by 2 voxels ({brain_ml:,.0f} ml of brain)",
            },
            {"label": "Node", "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}`"},
            {"label": "Command", "text": "`fast -I 0 -N -p -g -S 1 " + name + "`"},
            {"label": "Programs", "text": _programs(info)},
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: threads and modes"],
            ["fmriprep", "LARMORX against fMRIPrep's own output"],
        ],
        "tables": tables,
        "metrics_note": _note(n_vox),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
    }


#: The real examples' texts: title, summary, the input panels (argument -> label, sub), data.
REAL_TEXT = {
    "bias-field": (
        "The bias field: FAST's default",
        "The record's case `opt/bias-outputs`: FAST's default segmentation, which estimates "
        "a smooth bias field along with the classes and divides it out at each iteration "
        "(`-b` writes the field, `-B` the corrected image, `-g` the binary classes), on "
        "ds000005's skull-stripped T1w at half resolution, by FSL 6.0.7.23 and the replica in "
        "its reproduce and default modes; choose the bias field above the panels.",
    ),
    "priors": (
        "FSL's tissue priors (-a), and a correction by default",
        "The record's case `priors/a-verbose`: `-a` with a standard-to-input matrix brings "
        "FSL's tissue priors (avg152T1's CSF, grey and white matter) onto the image with FLIRT "
        "and starts the segmentation from them, here on MNI152NLin2009cAsym's 2 mm T1w inside "
        "its brain mask; `-v` prints every step. FSL 6.0.7.23 and the replica's reproduce mode "
        "give the same bytes; the replica's default corrects FSL-09 in FLIRT's resampling of "
        "the priors, and so starts from other priors: the difference *replica, default − "
        "reproduce mode* shows what that changes.",
    ),
    "multi-channel": (
        "Three channels: T1w, T2w and BOLD reference",
        "The record's case `multi/s3`: `-S 3` segments three registered images together "
        "(MNI152NLin2009cAsym's 2 mm T1w, T2w and BOLD reference inside its brain mask; the "
        "last image named, the T1w, is channel 1, which names the outputs and orders the "
        "classes), each class a Gaussian in three dimensions with its 3 × 3 covariance, a bias "
        "field per channel (`-b`, `-B`); by FSL 6.0.7.23 and the replica in its reproduce and "
        "default modes.",
    ),
}
#: The derived inputs' labels.
INPUTS = {
    "t1_half.nii.gz": ("ds000005 sub-01 T1w brain, half resolution",
                       "fMRIPrep's skull-stripped T1w, every second voxel (2 × 2.67 × 2.67 mm)"),
    "mni_t1.nii.gz": ("MNI152NLin2009cAsym T1w", "2 mm, inside the template's brain mask"),
    "mni_t2.nii.gz": ("MNI152NLin2009cAsym T2w", "2 mm, inside the template's brain mask"),
    "mni_bold.nii.gz": ("MNI152NLin2009cAsym BOLD reference",
                        "fMRIPrep's BOLD reference template, 2 mm, inside the brain mask"),
}  # fmt: skip


def _example_real(ctx: core.Context, ex_id: str) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    case = REAL[ex_id]
    e = real_entry(ctx, case)
    info = e.info()
    run = _real_run(case)
    fs = _suite()
    images = [a[1:] for a in run.args if a.startswith("@") and a.endswith(".nii.gz")]
    src = e.dir / "fsl"
    base = _base(src, set(images))
    channels = [INPUTS[n][0].split(" ", 1)[1] for n in reversed(images)]
    files = _files(src, base)
    first = nib.load(str(fs.inputs() / images[-1]))
    chans = {n: common.load(str(fs.inputs() / n)) for n in images}
    show = ndimage.binary_dilation(
        np.logical_or.reduce([c > 0 for c in chans.values()]), iterations=2
    )
    grid = core.Grid.around(first, show)
    ex = exm.Example(ctx, grid, show)
    for k, n in enumerate(reversed(images)):
        label, sub = INPUTS[n]
        if len(images) > 1:
            sub = f"channel {k + 1}; {sub}"
        ex.add(exm.Panel(f"in{k + 1}", "input", label, sub, {"all": chans[n]}))
    arrays = {pid: _load(e.dir / pid, files) for pid, *_ in PANELS}
    for pid, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    diffs = _diffs(False)
    outputs = _outputs(files, channels if len(images) > 1 else None)
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    bias = "-N" not in run.args
    _verdicts(diffs, same, "this case", info, bias=bias)
    if ex_id == "priors":
        out_id, a, b = "pve_1", "replica-fix", "replica"
        panels = ["in1", "fsl", "replica", "replica-fix"]
        cap = "the crosshair where the default mode's grey matter differs most from FSL's."
    elif ex_id == "bias-field":
        out_id, a, b = "bias", "fsl", "replica"
        panels = ["in1", "fsl", "replica"]
        cap = "the crosshair at the brain's centre (the outputs agree everywhere)."
    else:
        out_id, a, b = "seg", "fsl", "replica"
        panels = ["in1", "in2", "in3", "fsl", "replica"]
        cap = "the crosshair at the brain's centre (the outputs agree everywhere)."
    at = _default_view(ex, "volume", a, b, out_id)
    default = {
        "output": out_id,
        "plane": "axial",
        "voxel": at,
        "panels": panels,
        "diffs": ["replica-fsl", "fix-replica"],
        "compare": {"a": a, "b": b},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(outputs), default=default,
                        window_panel="fsl", caption=cap)  # fmt: skip
    run_dirs = {"replica": e.dir / "replica", "replica-fix": e.dir / "replica-fix"}
    tables = [
        _files_table(
            f"{ex_id}-files",
            files,
            lambda pid: run_dirs[pid],
            {"replica": "replica (reproduce)", "replica-fix": "replica (default)"},
        )
    ]
    facts_extra = []
    if ex_id == "priors":
        pri = sorted(src.glob("*_stdspace.nii.gz"))
        ok = all(_same_file(p, e.dir / "replica" / p.name) == (True, True) for p in pri)
        fix_same = all(_same_file(p, e.dir / "replica-fix" / p.name)[0] for p in pri)
        facts_extra.append(
            {
                "label": "Priors",
                "text": f"FAST's {len(pri)} registered priors (`{base}_{{csf,gm,wm}}_stdspace`, "
                "written by FLIRT and left behind): the replica's reproduce mode writes FSL's "
                + ("bytes" if ok else "other bytes")
                + "; its default mode "
                + ("the same values" if fix_same else "other values (FSL-09: rounded)")
                + "; FSL's priors are not shown (FSL's data, FSL's licence)",
            }
        )
    if ex_id == "multi-channel" or ex_id == "priors":
        data = [common.MNI]
        data_text = (
            "MNI152NLin2009cAsym's 2 mm images from TemplateFlow, inside its brain mask (the "
            f"record's derived inputs, oracles/inputs/fsl-fast-replica/); {common.MNI_NOTICE}"
        )
    else:
        data = [common.DS000005]
        data_text = (
            f"{common.DS000005['dataset']}, {common.DS000005['subject']} "
            f"({common.DS000005['licence']}): the record's input derived from fMRIPrep's run "
            "(its skull-stripped T1w, every second voxel; oracles/inputs/fsl-fast-replica/); "
            "shown inside the brain, dilated by 2 voxels"
        )
    title, summary = REAL_TEXT[ex_id]
    n_vox = int(np.prod(first.shape[:3]))
    return {
        "id": ex_id,
        "title": title,
        "summary": summary,
        "facts": [
            {"label": "Data", "text": data_text},
            {"label": "Case", "text": f"`{case}` of the record (`{RECORD}`)"},
            {"label": "Command", "text": "`" + info["command"] + "`"},
            {"label": "Programs", "text": _programs(info)},
            *facts_extra,
            {"label": "Grid", "text": common.describe_grid(first, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: threads and modes"],
        ],
        "tables": tables,
        "metrics_note": _note(n_vox),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", data),
    }


# ------------------------------------------------------------------------------------ cases


def _input(a: str) -> str:
    name = a[1:]
    if a.startswith("@dt:"):
        return f"<random testing's input {name.split(':', 1)[1]}>"
    if name.startswith("mni"):
        return f"<{name}: MNI152NLin2009cAsym, derived>"
    if name.startswith(("phantom", "zeros", "const")):
        return f"<{name}: synthetic>"
    if name.endswith((".mat", ".txt")):
        return f"<{name}: written by the suite>"
    if name.startswith("tpl_"):
        return f"<{name}: ds000005's fMRIPrep tissue maps in the template's space>"
    return f"<{name}: derived from ds000005's fMRIPrep run>"


def command(case: str, defs: dict[str, tuple[str, object]]) -> str:
    if case.startswith("fmriprep/"):
        return "fast -I 0 -N -p -g -S 1 <fMRIPrep's T1w brain>"
    if case not in defs or defs[case][1] is None:
        return ""
    run = defs[case][1]
    # an input is named by a placeholder (never quoted); a real argument quoted as a shell would
    parts = ["fast"] + [
        _input(a) if a.startswith("@") else shlex.quote(a) if re.search(r"\s", a) or a == "" else a
        for a in run.args
    ]
    env = " ".join(f"{k}={v}" for k, v in run.settings if v is not None)
    unset = " ".join(f"{k} unset" for k, v in run.settings if v is None)
    return " ".join(x for x in (env, " ".join(parts), f"({unset})" if unset else "") if x)


def case_verdict(r: dict, dm: dict) -> tuple[str, str]:
    """A case's verdict and reason from the record and the replica's default mode on it
    (``dm``: its exit status and the entries it names)."""
    named, code = dm.get("named", []), dm.get("exit")
    v = common.outcome_verdict(r)
    if v is not None:
        verdict, why = v
        if verdict == "both-reject" and named:
            why += (
                f" The replica's default mode finishes instead (exit status 0), naming "
                f"{', '.join(named)}."
                if code == 0
                else f" The replica's default mode ends with exit status {code}, naming "
                f"{', '.join(named)}."
            )
        return verdict, common.clip(why, 600)
    rep = {**r.get("reported", {}), **r.get("measured", {})}
    files = rep.get("every file bit-identical")
    notes = [
        f"every file FSL writes{f' ({files})' if files else ''} byte for byte, with FSL's exit "
        "status and messages"
    ]
    own = rep.get("fMRIPrep's outputs (its container's FSL) identical")
    if own:
        notes.append(f"fMRIPrep's own files (its image's FSL): {own} the same")
    cleared = rep.get("FSL's voxels never written (outside the mask) set to 0")
    if cleared not in (None, "0"):
        notes.append(
            f"FSL's probability maps outside the mask, memory it never writes (FSL-96), set to "
            f"0 first in {cleared} file(s)"
        )
    if rep.get("FSL's bias field never written (-N): its values set to the defined ones"):
        notes.append("FSL's bias field, never computed (FSL-97), compared with the defined one")
    fix = rep.get("D13 fix pass")
    shown = [n for n in named if n not in ("FSL-96", "FSL-97")]
    if isinstance(fix, int) and fix:
        notes.append(
            f"in its default mode the replica corrects {', '.join(shown or named)} and names "
            f"it: {fix:,} values change"
        )
    elif named and code not in (0, None):
        notes.append(
            f"its default mode refuses the call (exit {code}), naming {', '.join(named)}: an "
            "argument without meaning, which FSL uses as given"
        )
    elif named:
        notes.append(f"its default mode names {', '.join(named)}, with the same values")
    return "bit-identical", common.clip("; ".join(notes), 600)


def cases(rec, examples: dict[str, str], default: dict[str, dict]) -> list[dict]:
    defs = case_table()
    out = []
    for r in rec.results:
        desc = defs.get(r["case"], ("", None))[0]
        if r["case"].startswith("fmriprep/"):
            desc = (
                "fMRIPrep's segmentation call ("
                + r["case"].split("/")[1].split("-noreconall")[0]
                + ")"
            )
        v, why = case_verdict(r, default.get(r["case"], {}))
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": desc,
                "command": command(r["case"], defs),
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
    rec = records.load(ctx.repo, RECORD)
    examples = {
        **{f"fmriprep/{run}": ex_id for ex_id, run, _v, _w in FMRIPREP},
        **{case: ex_id for ex_id, case in REAL.items()},
    }
    ne = named_entry(ctx)
    cs = cases(rec, examples, ne.info().get("default_mode", {}))
    entries = [fmriprep_entry(ctx, run) for _i, run, _v, _w in FMRIPREP]
    entries += [real_entry(ctx, case) for case in REAL.values()]
    entries.append(ne)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "FAST",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": [],
            "cache": [fcache.provenance(x) for x in entries],
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
        ],
        "cases": cs,
        "cases_total": len(cs),
        "examples": [_example_fmriprep(ctx, *FMRIPREP[0])]
        + [_example_real(ctx, ex_id) for ex_id in REAL]
        + [_example_fmriprep(ctx, *x) for x in FMRIPREP[1:]],
    }


if __name__ == "__main__":  # compute the cache entries only
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    for _i, run, _v, _w in FMRIPREP:
        print("done", fmriprep_entry(c, run).dir)
    for case in REAL.values():
        print("done", real_entry(c, case).dir)
    print("done", named_entry(c).dir)
