# SPDX-License-Identifier: Apache-2.0
"""topup's and applytopup's evidence (the larmorx-nc replicas, FSL Licence; the clean-room
original is still to be written): fMRIPrep's field-map estimation call (sdcflows' PEPOLAR
workflow, ``fmap_preproc_wf/<estimator>/topup``: ``topup --config=b02b0.cnf --datain=<encoding
file> --imain=<merged pair> --out --iout --fout --jacout --logout --rbmout --dfout``) on its two
oracle runs (ds005040 and ds001600): the reversed-phase-encoding pair before and after the
correction, the field, the Jacobians and the displacements, by FSL 6.0.7.23 (at one and at four
threads: its own thread band) and by the replica in its reproduce and default modes (and at four
threads); a correction by default inside topup (FSL-117: membrane energy with
Levenberg-Marquardt); applytopup's least-squares restoration, Jacobian modulation and vb3D on
topup's own result (FSL-132, FSL-142 and FSL-143 corrected by default); every case of the
record with its verdict; random testing's trial.

Every run of FSL is a run of the parity record (``parity.fsl_topup_replica``), with the same
arguments, environment and inputs, through the oracle-output cache (``use`` mode): the page
computes no original result of its own, apart from FSL at four threads on fMRIPrep's calls
(through the cache too). The replica runs as a separate program; nothing here imports it.
Everything shown lies inside sdcflows' brain mask of the corrected reference (dilated): no face
can show.
"""

from __future__ import annotations

import copy
import gzip
import hashlib
import json
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

TOOL = "fsl-topup"
RECORD = "fsl-topup-replica"
#: fMRIPrep's calls shown as examples: example id, the fMRIPrep run, the output shown first.
FMRIPREP = [
    ("fmriprep-ds005040", "ds005040-sub-002-noreconall", "corrected-1"),
    ("fmriprep-ds001600", "ds001600-sub-1-noreconall", "field"),
]
#: The record's other cases shown as examples: example id -> case.
REAL = {
    "membrane": "opt/membrane-lm",
    "lsr": "at/lsr-y",
    "jac": "at/jac-y",
    "vb3d": "at/vb3d-y",
}
#: Random testing's trial of this part (``<workspace>/oracles/difftest/<trial>/summary.json``).
TRIAL = "2026-11-03-topup-replica"
#: FSL's runs: label, extra arguments, threads (the cache's policy).
FSL_RUNS = [("fsl", (), 1), ("fsl-4", ("--nthr=4",), 4)]
#: The replica's runs: label, D13 mode, extra arguments.
REPLICA_RUNS = [("replica", "reproduce", ()), ("replica-4", "reproduce", ("--nthr=4",)),
                ("replica-fix", "fix", ())]  # fmt: skip
PANELS = [
    ("fsl", "original", "FSL 6.0.7.23, {program}",
     "the conda installation, as built, one thread (the record's run, from the oracle cache)"),
    ("fsl-4", "original", "FSL 6.0.7.23, topup --nthr=4",
     "the same installation at four threads: its own thread band (FSL-116)"),
    ("replica", "larmorx", "larmorx-nc replica", "reproduce mode"),
    ("replica-4", "larmorx", "larmorx-nc replica", "reproduce mode, --nthr=4"),
    ("replica-fix", "larmorx", "larmorx-nc replica", "default mode (D13 corrections)"),
]  # fmt: skip
#: A notice of the replica's (``larmorx-nc: upstream defect FSL-xx (...)``).
NOTICE = re.compile(r"upstream defect (FSL-\d+) \(")


def _suite():
    from larmorx_validation.parity import fsl_topup_replica as fs

    return fs


def _data(run: str) -> dict:
    from larmorx_validation.evidence.ants_panel import data

    return data(run.split("-sub-")[0])


def versions() -> dict[str, str]:
    from larmorx_validation.parity import fsl_versions
    from larmorx_validation.parity.fsl_flirt_replica import fsldir
    from larmorx_validation.parity.fsl_mcflirt_replica import replica_version

    return {
        "fsl": f"FSL {fsl_versions.version_of(fsldir())} (fsl-topup 2203.5; ~/fsl-6.0.7.23, "
        "never on the PATH)",
        "larmorx-nc": replica_version(),
    }


# --------------------------------------------------------------------- running the programs


def _oracle(
    e: fcache.Entry,
    label: str,
    args: list[str],
    put_inputs,
    settings: dict[str, str | None],
    *,
    threads: int = 1,
    never: str = "",
    program: str = "topup",
) -> None:
    """FSL's ``program`` with ``args`` through the oracle-output cache, its working directory
    holding the inputs ``put_inputs(dir)`` copies there (as the record's case does); its files,
    the inputs left out, into ``<entry>/<label>``; whether it was served is kept in the entry."""
    fs = _suite()
    with tempfile.TemporaryDirectory(prefix="lx-nc-topup-ev_") as t:
        cwd = Path(t)
        put_inputs(cwd)
        given = {p.name for p in cwd.iterdir()}
        p = fs.oracle_run(args, cwd=cwd, settings=settings, never=never, threads=threads,
                          program=program)  # fmt: skip
        if p.returncode != 0:
            raise RuntimeError(f"{label}: FSL's {program} exited {p.returncode}: {p.stderr[-800:]}")
        out = e.dir / label
        out.mkdir(exist_ok=True)
        for f in cwd.iterdir():
            if f.name not in given and f.is_file():
                shutil.copy(f, out / f.name)
        (e.dir / f"{label}.stdout.txt").write_text(p.stdout, encoding="utf-8")
    from larmorx_validation.parity.fsl_flirt_replica import _environment, fsldir

    run = fcache.Run(label, [str(fsldir() / "bin" / program), *args], _environment(settings))
    run.exit, run.seconds = p.returncode, round(p.seconds, 2)
    run.stderr_tail = p.stderr[-2000:]
    e.runs.append(run)
    e.extra.setdefault("oracle_cache", {})[label] = "served" if p.cached else "ran"


def _replica(
    e: fcache.Entry,
    label: str,
    mode: str,
    args: list[str],
    put_inputs,
    settings: dict[str, str | None],
    program: str = "topup",
) -> None:
    """The replica on ``args`` in ``mode``, in ``<entry>/<label>`` (it writes its outputs where
    FSL does: its working directory); the inputs removed after."""
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    exe = _suite().replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    unset = tuple(k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_", "OMP_")))
    cwd = e.dir / label
    cwd.mkdir(exist_ok=True)
    put_inputs(cwd)
    given = [p for p in cwd.iterdir()]
    env = {**_environment(settings), "LARMORX_UPSTREAM_DEFECTS": mode, "OMP_NUM_THREADS": "4"}
    e.run(label, [str(exe), "fsl", program, *args], env=env, unset=unset, cwd=cwd)
    log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
    e.extra.setdefault("named", {})[label] = sorted(set(NOTICE.findall(log)))
    for p in given:
        if p.is_file():
            p.unlink()


def _node(run: str) -> Path:
    nodes = [n for r, n in _suite().fmriprep_calls() if r == run]
    if not nodes:
        raise FileNotFoundError(f"fMRIPrep's topup node of {run}")
    return nodes[0]


def _brain_mask(node: Path) -> Path:
    """sdcflows' brain mask of the estimator's corrected reference (its ``brainextraction_wf``,
    on the grid of topup's input)."""
    p = node.parent / "brainextraction_wf" / "masker" / "clipped_mask.nii.gz"
    if not p.is_file():
        raise FileNotFoundError(f"sdcflows' brain mask beside {node}")
    return p


def _compute_fmriprep(run: str):
    def compute(e: fcache.Entry) -> None:
        fs = _suite()
        node = _node(run)
        args = fs.fmriprep_args(node)
        img = fs._fmriprep_image(node)
        if img is None:
            raise FileNotFoundError(f"the merged images of fMRIPrep's topup node of {run}")
        e.input("merged reversed-phase-encoding pair (fMRIPrep's)", img)
        e.input("b02b0.cnf (sdcflows')", fs.workspace() / fs.SDCFLOWS_CONFIG)
        e.input("brain mask (sdcflows')", _brain_mask(node))

        def put(d: Path) -> None:
            fs._prepare_fmriprep(node, d)

        for label, extra, threads in FSL_RUNS:
            _oracle(e, label, [*args, *extra], put, {}, threads=threads)
        for label, mode, extra in REPLICA_RUNS:
            _replica(e, label, mode, [*args, *extra], put, {})
        e.finish(
            versions=versions(),
            command=(node / "command.txt").read_text(encoding="utf-8").strip(),
            node=str(node.relative_to(common.fmriprep_run(run))),
            args=args,
            settings="fMRIPrep 25.2.6's topup node (sdcflows' PEPOLAR estimation) of the run: "
            "its arguments from command.txt, its merged pair from the run's working directory, "
            "sdcflows' b02b0.cnf and the encoding file nipype writes, copied into the run "
            "directory as the record's case does; FSL 6.0.7.23 through the oracle-output cache "
            "(at one thread the record's own run; at --nthr=4 its own thread band) and the "
            "larmorx-nc replica: reproduce mode at --nthr=1 and 4, the default mode",
        )

    return compute


def fmriprep_entry(ctx: core.Context, run: str) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"fmriprep-{run}",
        f"fMRIPrep's topup call of {run}: FSL 6.0.7.23 at one thread (the record's run, from "
        "the oracle cache) and four, and the replica, reproduce and default modes",
        _compute_fmriprep(run),
    )


def case_table() -> dict[str, tuple[str, object]]:
    """Every case of the suite: id -> (description, its ``Run``; None for fMRIPrep's calls)."""
    fs = _suite()
    out: dict[str, tuple[str, object]] = {}
    for c in fs.cases("full"):
        kind, payload = c.payload
        run = fs._all_cases()[payload][2] if kind == "run" else None
        out.setdefault(c.id, (c.description, run))
    return out


def _real_run(case: str):
    return _suite()._all_cases()[case][2]


def _compute_real(case: str):
    def compute(e: fcache.Entry) -> None:
        fs = _suite()
        run = _real_run(case)
        node = fs._source_node()
        with tempfile.TemporaryDirectory(prefix="lx-nc-topup-ev-args_") as t:
            args = fs._prepare(run, Path(t), node)
        for a in run.args:
            for part in a.partition("=")[2].split(","):
                if part.startswith("@") and not part.endswith(".txt"):
                    e.input(part[1:], fs.inputs() / part[1:])
        if run.source:
            for f in sorted(fs.topup_outputs(run.source).glob("tu_*")):
                e.input(f"{f.name} (FSL's topup, the record's {fs.TOPUP_SOURCES[run.source]})", f)

        def put(d: Path) -> None:
            fs._prepare(run, d, node)

        settings = dict(run.settings)
        _oracle(e, "fsl", args, put, settings, never=run.never, program=run.program)
        for label, mode, _extra in (REPLICA_RUNS[0], REPLICA_RUNS[2]):
            _replica(e, label, mode, args, put, settings, run.program)
        e.finish(
            versions=versions(),
            args=args,
            settings=f"The record's case {case}, its inputs from the suite's derived inputs "
            "(oracles/inputs/fsl-topup-replica/; applytopup's from FSL's topup of the record's "
            "case opt/levels, served from the oracle cache), by FSL 6.0.7.23 through the "
            "oracle-output cache (the record's own run) and by the larmorx-nc replica in its "
            "reproduce and default modes",
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
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    fs = _suite()
    rec = records.load(e.ctx.repo, RECORD)
    defs = case_table()
    nodes = {f"fmriprep/{r}": n for r, n in fs.fmriprep_calls()}
    exe = fs.replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    unset = tuple(k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_", "OMP_")))
    default: dict[str, dict] = {}
    for r in rec.results:
        case = r["case"]
        with tempfile.TemporaryDirectory(prefix="lx-nc-topup-ev-named_") as t:
            d = Path(t)
            if case in nodes:
                fs._prepare_fmriprep(nodes[case], d)
                args, settings, program = fs.fmriprep_args(nodes[case]), {}, "topup"
            else:
                run = defs[case][1]
                args = fs._prepare(run, d, fs._source_node())
                settings, program = dict(run.settings), run.program
            env = {**_environment(settings), "LARMORX_UPSTREAM_DEFECTS": "fix",
                   "OMP_NUM_THREADS": "4"}  # fmt: skip
            label = case.replace("/", "-")
            x = e.run(label, [str(exe), "fsl", program, *args], env=env, unset=unset, cwd=d,
                      check=False, timeout=3600)  # fmt: skip
            log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
            default[case] = {"exit": x.exit, "named": sorted(set(NOTICE.findall(log)))}
    e.finish(
        versions=versions(),
        default_mode=default,
        settings="The larmorx-nc replica in its default mode on every case of the record, in a "
        "fresh directory with the case's inputs: its exit status and the register entries its "
        "notices name (no FSL run; the record's fix pass checks that a name appears exactly "
        "when the default's outputs differ)",
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


def _nifti(path: Path) -> bytes:
    raw = path.read_bytes()
    return gzip.decompress(raw) if path.name.endswith(".gz") else raw


def _same_file(a: Path, b: Path) -> tuple[bool, bool]:
    """Two files: (the same data, the same header but ``descrip``); NIfTI-1 images
    decompressed, other files byte for byte."""
    import struct

    if not b.is_file():
        return False, False
    if ".nii" not in a.name:
        same = a.read_bytes() == b.read_bytes()
        return same, same
    x, y = _nifti(a), _nifti(b)
    off = int(struct.unpack_from("<f", x, 108)[0])
    hx = x[:148] + x[228:off]
    hy = y[:148] + y[228:off] if len(y) >= off else b""
    return x[off:] == y[off:], hx == hy


def _pe_rows(text: str) -> list[str]:
    """The phase encodings of an acquisition file's rows (``+y``, ``−y``, ...)."""
    out = []
    for line in text.split("\n"):
        w = line.split()
        if len(w) < 4:
            continue
        v = [float(x) for x in w[:3]]
        k = int(np.argmax(np.abs(v)))
        out.append(("+" if v[k] > 0 else "−") + "xyz"[k])
    return out


def _outputs_of(run_dir: Path, shape: tuple[int, ...], pe: list[str]) -> dict[str, tuple]:
    """The images of ``run_dir`` on the input's grid, as the viewer's outputs: id -> (file,
    volume or None, label, unit). A 4D image of one volume per input volume (``--iout``) gives
    one output per volume; a displacement field (``--dfout``, three components in mm) its
    components that are not 0 everywhere; topup's coefficients (their own grid) are left out."""
    import nibabel as nib

    out: dict[str, tuple] = {}
    for p in sorted(run_dir.glob("*.nii*")):
        img = nib.load(str(p))
        if tuple(img.shape[:3]) != tuple(shape) or int(img.header["intent_code"]) == 2016:
            continue
        a = np.asanyarray(img.dataobj)
        name = p.name.split(".nii")[0]
        m = re.fullmatch(r"(.*?)_?(\d\d)", name)
        if p.name.endswith(("_mean.nii.gz", "_mean.nii")) and a.ndim == 3:  # --iout's mean
            out["corrected-mean"] = (p.name, None, "Corrected: mean of the volumes", "")
        elif name.startswith("warpfield") or (m and a.ndim == 4 and a.shape[3] == 3):
            k = int(m.group(2)) if m else 1
            for c in range(a.shape[3]):
                if np.any(a[..., c] != 0):
                    axis = "xyz"[c]
                    out[f"warp-{k}{axis if axis != 'y' else ''}"] = (
                        p.name,
                        c,
                        f"Displacement along {axis}: volume {k} ({pe[k - 1]})",
                        "mm",
                    )
        elif name.startswith("jac") and m:
            k = int(m.group(2))
            out[f"jac-{k}"] = (p.name, None, f"Jacobian: volume {k} ({pe[k - 1]})", "")
        elif a.ndim == 4 and a.shape[3] == len(pe):
            for k in range(a.shape[3]):
                out[f"corrected-{k + 1}"] = (p.name, k, f"Corrected: volume {k + 1} ({pe[k]})",
                                             "")  # fmt: skip
        elif int(img.header["intent_code"]) == 2018 or name.endswith(("_field", "f")):
            out["field"] = (p.name, None, "Field (Hz)", "Hz")
        else:
            out[re.sub(r"\W", "-", name)] = (p.name, None, "Corrected image", "")
    order = ("corrected", "hifi", "field", "jac", "warp")
    keys = sorted(out, key=lambda k: (next((i for i, o in enumerate(order) if k.startswith(o)),
                                           len(order)), k))  # fmt: skip
    return {k: out[k] for k in keys}


def _arrays(run_dir: Path, outs: dict[str, tuple]) -> dict[str, np.ndarray]:
    res = {}
    for oid, (name, vol, _label, _unit) in outs.items():
        a = common.load(str(run_dir / name))
        res[oid] = np.ascontiguousarray(a[..., vol] if vol is not None else a)
    return res


def _files_table(tid: str, fsl_dir: Path, columns: dict[str, Path | None], note: str) -> dict:
    """Every file the program wrote, FSL's against each run: the data and the header
    (``descrip``, FSL's build string, aside), byte for byte."""
    import nibabel as nib

    rows = []
    for f in sorted(fsl_dir.iterdir()):
        if ".nii" in f.name:
            img = nib.load(str(f))
            kind = [common.cell(str(img.get_data_dtype())),
                    common.cell(" × ".join(map(str, img.shape)))]  # fmt: skip
        else:
            lines = f.read_text(encoding="utf-8", errors="replace").count("\n")
            kind = [common.cell("text"), common.cell(f"{lines} lines")]
        cells = list(kind)
        for d in columns.values():
            if d is None or not (d / f.name).is_file():
                cells.append(common.cell("–"))
                continue
            data, head = _same_file(f, d / f.name)
            cells.append(common.cell("identical" if data and head else
                                     "data identical" if data else "differs"))  # fmt: skip
        rows.append({"head": f"`{f.name}`", "cells": cells})
    return common.table(
        tid,
        "Every file written, against FSL 6.0.7.23's at one thread",
        ["File", "Type", "Size", *columns],
        rows,
        note="Each image decompressed and compared byte for byte with FSL's: *identical*, the "
        "header (its `descrip`, FSL's build string, aside) and every value; *data identical*, "
        "every value but not the header; *differs*, values that differ (see the comparisons "
        "above); text files byte for byte; – where the run keeps no such file. " + note,
    )


# -------------------------------------------------------------------------- examples


def _diffs(fmriprep: bool) -> list[dict]:
    d = common.d
    out = [d("replica-fsl", "replica", "fsl", "main", "replica − FSL 6.0.7.23", common.ROLE_LX)]
    if fmriprep:
        out += [
            d("replica-own", "replica", "own", "fmriprep", "replica − fMRIPrep's own output",
              common.ROLE_FMRIPREP),
            d("replica4-replica", "replica-4", "replica", "check",
              "replica, --nthr=4 − --nthr=1", common.ROLE_SELF),
        ]  # fmt: skip
    out.append(d("fix-replica", "replica-fix", "replica", "check",
                 "replica, default − reproduce mode", common.ROLE_SELF))  # fmt: skip
    if fmriprep:
        out.append(d("fsl4-fsl", "fsl-4", "fsl", "variability",
                     "FSL, --nthr=4 − --nthr=1", common.ROLE_ORIG))  # fmt: skip
    return out


#: Why the default mode differs, per register entry (its notices name them).
FIX_REASONS = {
    "FSL-117": "FSL-117, corrected by default with a notice: with membrane energy, "
    "Levenberg-Marquardt's Hessian of the regularisation divides each derivative by the voxel "
    "size in mm while the cost and its gradient count in voxels; the default takes the Hessian "
    "in the cost's units, so the steps follow the cost they minimise (the final cost is never "
    "higher: −0.00 to −1.66 % on the measured cases). The reproduce mode gives FSL's bytes.",
    "FSL-119": "FSL-119, corrected by default with a notice: the field maps --fout and --featout "
    "write carry the input's intent parameters shifted by one place; the default keeps them in "
    "place.",
    "FSL-132": "FSL-132, corrected by default with a notice: applytopup resamples the first "
    "volume of the first image with trilinear interpolation whatever --interp asks (the "
    "default, spline, reaches every other volume); the default resamples every volume with "
    "the method asked for. The reproduce mode gives FSL's bytes.",
    "FSL-133": "FSL-133, corrected by default with a notice: the least-squares restoration "
    "wraps a negative index along a line by 2^32 modulo the line's length; the default wraps "
    "it around the line.",
    "FSL-142": "FSL-142, corrected by default with a notice: vb3D and vb4D count two data "
    "points per line whatever the collection holds; the default counts every image's.",
    "FSL-143": "FSL-143, corrected by default with a notice: vb3D's and vb4D's prior couples "
    "neighbouring slices with half the weight its own construction gives them, so the "
    "precision it builds is not positive definite and the iterations diverge (FSL's output "
    "here spans about ±4 × 10^10 for an input of 0 to 1.1 × 10^5); the default gives the "
    "prior its full weight, and the restoration stays in the input's range, as vb2D's does.",
}


def _verdicts(diffs: list[dict], same: dict[str, bool], info: dict, what: str) -> None:
    named = [n for n in info.get("named", {}).get("replica-fix", []) if n.startswith("FSL-")]
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if x["kind"] == "variability":
            x["verdict"] = None
            m = x["metrics"]
            field = m.get("field", {}).get("max_abs", 0.0)
            corr = max((v["max_abs"] for k, v in m.items() if k.startswith("corrected")),
                       default=0.0)  # fmt: skip
            x["reason"] = (
                "FSL's own result at four threads: the same bytes on this call."
                if same[i]
                else "FSL's own thread band (FSL-116): above one thread FSL sums the sparse "
                "matrix-vector products of its Hessian solves in per-thread pieces, so its "
                f"result changes with --nthr: here the field by up to {field:.3g} Hz and the "
                f"corrected images by up to {corr:.3g} over the whole field of view (inside the "
                "brain: the thread-band table below the viewer). The replica gives FSL's "
                "one-thread result at any --nthr; this row is the yardstick."
            )
            x["links"].append({"label": "FSL-116", "href": "docs/findings/upstream-defects.md"})
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "replica-fsl": f"The same values, every one, in every image {what} writes (the "
                "record compares every file, the messages and the exit status too).",
                "replica-own": "fMRIPrep's own files (topup in fMRIPrep 25.2.6's image, an older "
                "build) hold the same values.",
                "replica4-replica": "The replica gives FSL's one-thread result at any --nthr "
                "(FSL-116: its sums are taken in one fixed order).",
                "fix-replica": "No correction of the register applies to this call: both modes "
                "give the same values."
                if not named
                else "No correction of the register changes this call's values: the default "
                f"mode names {', '.join(named)}, with the same values in both modes.",
            }.get(i, "The same values, every one.")
        elif i == "fix-replica" and named:
            x["verdict"] = "d13"
            x["reason"] = " ".join(FIX_REASONS.get(n, f"{n}, corrected by default.") for n in named)
            for n in named:
                x["links"].append({"label": n, "href": "docs/findings/upstream-defects.md"})
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example (see the record)."


def _band_table(tid: str, a: dict, b: dict, mask: np.ndarray, outs: dict[str, tuple]) -> dict:
    """FSL's own thread band per output (FSL at four threads − at one): over every voxel of the
    file, and inside sdcflows' brain mask (not dilated)."""
    rows = []
    for oid, (_name, _vol, label, unit) in outs.items():
        x, y = a[oid].astype(np.float64), b[oid].astype(np.float64)
        d = np.abs(x - y)
        inside = d[mask > 0]
        rows.append({
            "head": label,
            "cells": [
                common.cell(int(np.count_nonzero(d)), "count"),
                common.cell(float(d.max()), "sig"),
                common.cell(float(inside.max()), "sig"),
                common.cell(float(np.percentile(inside, 99)), "sig"),
                common.cell(float(np.median(inside)), "sig"),
                common.cell(unit or "–"),
            ],
        })  # fmt: skip
    return common.table(
        tid,
        "FSL's own thread band: FSL 6.0.7.23 at --nthr=4 − at --nthr=1",
        [
            "Output",
            {"label": "Values that differ", "num": True},
            {"label": "Largest |Δ|", "num": True},
            {"label": "Largest |Δ|, brain", "num": True},
            {"label": "99th percentile |Δ|, brain", "num": True},
            {"label": "Median |Δ|, brain", "num": True},
            "Unit",
        ],
        rows,
        note="Computed in float64 over every voxel of FSL's two runs' files; *brain*: inside "
        "sdcflows' brain mask of the corrected reference (not dilated). The replica at "
        "--nthr=4 gives FSL's one-thread values (the comparison *replica, --nthr=4 − --nthr=1* "
        "above: identical); FSL-116 in the register.",
    )


def _note(n: int, extra: str = "") -> str:
    return (
        f"Δ is A − B, computed in float64 over all {n:,} voxels of the outputs, not only those "
        "shown. The record's threshold: every file FSL writes bit-identical (images "
        "decompressed, `descrip` left out), with FSL's exit status and messages." + extra
    )


def _programs(info: dict) -> str:
    vers = info.get("versions", {})
    served = info.get("oracle_cache", {})
    how = (
        "its run served from the oracle cache, as the record ran it"
        if served.get("fsl") == "served"
        else "run for this page through the oracle cache"
    )
    four = ""
    if "fsl-4" in served:
        four = "; at --nthr=4 " + (
            "served from the oracle cache" if served["fsl-4"] == "served" else "run through it"
        )
    return (
        f"{vers.get('fsl', '?')} ({how}{four}); {vers.get('larmorx-nc', '?')} (the replica), "
        f"from larmorx at commit `{info['larmorx_commit'][:12]}`"
    )


def _show(mask: np.ndarray, iterations: int) -> np.ndarray:
    from scipy import ndimage

    return ndimage.binary_dilation(mask > 0, iterations=iterations)


def _example_fmriprep(ctx: core.Context, ex_id: str, run: str, first: str) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    fs = _suite()
    e = fmriprep_entry(ctx, run)
    info = e.info()
    node = _node(run)
    img = nib.load(str(fs._fmriprep_image(node)))
    pe = _pe_rows(fs._encfile(node))
    pair = np.asanyarray(img.dataobj)
    outs = _outputs_of(e.dir / "fsl", img.shape[:3], pe)
    mask = common.load(str(_brain_mask(node)))
    show = _show(mask, 2)
    grid = core.Grid.around(img.slicer[..., 0], show)
    ex = exm.Example(ctx, grid, show)
    ds = run.split("-noreconall")[0]
    for k in range(pair.shape[3]):
        ex.add(exm.Panel(f"in{k + 1}", "input", f"{ds} pair, volume {k + 1} ({pe[k]})",
                         "topup's input in fMRIPrep: the reversed-phase-encoding images, each "
                         "the average of its run, merged (float32)",
                         {"all": np.ascontiguousarray(pair[..., k])}))  # fmt: skip
    arrays = {pid: _arrays(e.dir / pid, outs) for pid, *_ in PANELS}
    for pid, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label.format(program="topup"), sub, arrays[pid]))
    own_dirs = {k: v for k, v in outs.items() if (node / v[0]).is_file()}
    own = _arrays(node, own_dirs)
    diffs = _diffs(True)
    outputs = [{"id": k, "label": v[2], "unit": v[3]} for k, v in outs.items()]
    common.compare_all(ex, diffs, outputs, full={"own": own})
    same = common.identical(diffs)
    _verdicts(diffs, same, info, "topup")
    at, _k = common.largest_in_slice(ex, "fsl-4", "fsl", first, "axial")
    default = {
        "output": first,
        "plane": "axial",
        "voxel": at,
        "panels": ["in1", "in2", "fsl", "replica"],
        "diffs": ["replica-fsl", "fsl4-fsl"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="fsl",
        caption="the crosshair where FSL at four threads differs most from FSL at one in this "
        "slice (the replica's output equals FSL's everywhere).",
    )  # fmt: skip
    cols = {
        "replica (reproduce)": e.dir / "replica",
        "replica --nthr=4": e.dir / "replica-4",
        "replica (default)": e.dir / "replica-fix",
        "FSL --nthr=4": e.dir / "fsl-4",
        "fMRIPrep's own": node,
    }
    tables = [
        _files_table(
            f"{ex_id}-files",
            e.dir / "fsl",
            cols,
            "fMRIPrep's own files are the node's outputs in the run's working directory (topup "
            "in fMRIPrep 25.2.6's image); it keeps no log or movement parameters. The log "
            "records the options as given, so the --nthr=4 runs' logs differ by that option.",
        ),
        _band_table(f"{ex_id}-band", arrays["fsl-4"], arrays["fsl"], mask, outs),
    ]
    warps = sorted({v[0] for k, v in outs.items() if k.startswith("warp")})
    n_vox = int(np.prod(img.shape[:3]))
    vox_ml = float(np.prod(img.header.get_zooms()[:3])) / 1000.0
    data = _data(run)
    rt = sorted({ln.split()[3] for ln in fs._encfile(node).strip().split("\n")})
    title = {
        "fmriprep-ds005040": "fMRIPrep's field map from a reversed-phase-encoding pair",
        "fmriprep-ds001600": "A second run: ds001600's pair",
    }[ex_id]
    summary = (
        f"The topup node fMRIPrep 25.2.6 ran on {ds}'s reversed-phase-encoding pair (sdcflows' "
        "PEPOLAR estimation, its configuration b02b0.cnf: seven levels from 14 mm to 4 mm "
        "knots, the last four by scaled conjugate gradients), given to FSL 6.0.7.23 at one "
        "thread (as fMRIPrep runs it) and at four, and to the larmorx-nc replica in its "
        "reproduce and default modes and at four threads. The two input panels are the pair "
        "before the correction, distorted in opposite directions along the phase encoding; "
        "choose an output above the panels: each volume corrected, the field in Hz, each "
        "volume's Jacobian and its displacement along the phase encoding. Below the viewer, "
        "every file topup writes against FSL's, with fMRIPrep's own."
    )
    return {
        "id": ex_id,
        "title": title,
        "summary": summary,
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): the "
                "field-map images fMRIPrep merged (each run averaged); shown inside sdcflows' "
                "brain mask of the corrected reference, dilated by 2 voxels "
                f"({float(np.count_nonzero(mask)) * vox_ml:,.0f} ml), so no face shows",
            },
            {"label": "Node", "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}`"},
            {"label": "Command", "text": "`" + command_fmriprep(info["args"]) + "`"},
            {
                "label": "Acquisition",
                "text": f"phase encodings {', '.join(pe)}; read-out time {', '.join(rt)} s "
                "(nipype's encoding file)",
            },
            {"label": "Programs", "text": _programs(info)},
            {
                "label": "Displacements",
                "text": f"{', '.join(f'`{w}`' for w in warps)}: three components each, the x "
                "and z ones 0 everywhere (phase encoding along y), so the y component is shown",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["fmriprep", "LARMORX against fMRIPrep's own output"],
            ["check", "LARMORX against itself: threads and modes"],
            ["variability", "FSL against itself: the thread count (the yardstick)"],
        ],
        "tables": tables,
        "metrics_note": _note(n_vox),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
    }


#: The real examples' texts: title, summary.
REAL_TEXT = {
    "membrane": (
        "Membrane energy with Levenberg-Marquardt, and a correction by default",
        "The record's case `opt/membrane-lm`: topup with the membrane energy as its "
        "regulariser (`--regmod=membrane_energy`; fMRIPrep's configuration uses the bending "
        "energy) and Levenberg-Marquardt, one short level on the half-resolution pair of "
        "ds005040. FSL 6.0.7.23 and the replica's reproduce mode give the same bytes; the "
        "replica's default corrects FSL-117 (the energy's Hessian in other units than its cost "
        "and gradient), and so takes other steps: the difference *replica, default − "
        "reproduce mode* shows what that changes in the field.",
    ),
    "lsr": (
        "applytopup: least-squares restoration of the pair (the default method)",
        "The record's case `at/lsr-y`: applytopup's default method combines the two images of "
        "opposite phase encodings into one restored image, line by line along the phase "
        "encoding, with the field and movements of FSL's topup on the same pair (the record's "
        "case `opt/levels`, from the oracle cache). FSL 6.0.7.23 and the replica's reproduce "
        "mode give the same bytes; the replica's default corrects FSL-132 (the first image "
        "resampled with trilinear rather than spline interpolation).",
    ),
    "jac": (
        "applytopup: Jacobian modulation of one image",
        "The record's case `at/jac-y`: `--method=jac` corrects one image by itself, resampled "
        "along the phase encoding and multiplied by the displacement's Jacobian, with the "
        "result of FSL's topup on the half-resolution pair. FSL 6.0.7.23 and the replica's "
        "reproduce mode give the same bytes; the replica's default corrects FSL-132 (that "
        "image's interpolation).",
    ),
    "vb3d": (
        "applytopup --method=vb3D, and a correction by default",
        "The record's case `at/vb3d-y`: one of applytopup's undocumented variational Bayes "
        "restorations (a smoothness prior within and between slices, its weights estimated "
        "over 20 iterations). FSL 6.0.7.23 and the replica's reproduce mode give the same "
        "bytes, and both diverge: the prior FSL builds couples the slices with half its "
        "weight, which makes it indefinite (FSL-143). The replica's default gives the prior its "
        "full weight (and resamples the first image with the spline interpolation asked for, "
        "FSL-132): its restoration stays in the input's range. Choose *each image's own* window "
        "to read FSL's output.",
    ),
}
#: The derived inputs' labels: file name -> (label, sub).
INPUT_TEXT = {
    "half.nii.gz": "ds005040 pair, half resolution",
    "half_up.nii.gz": "ds005040, +y image, half resolution",
    "half_dn.nii.gz": "ds005040, −y image, half resolution",
}


def _example_real(ctx: core.Context, ex_id: str) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    fs = _suite()
    case = REAL[ex_id]
    e = real_entry(ctx, case)
    info = e.info()
    run = _real_run(case)
    args = info["args"]
    images = [n for a in args if a.startswith("--imain=") for n in a.split("=", 1)[1].split(",")]
    base = fs.inputs()
    first = nib.load(str(base / images[0]))
    acq = next(a.split("=", 1)[1] for a in args if a.startswith(("--datain=", "--acqp=")))
    pe = _pe_rows((base / acq).read_text(encoding="utf-8"))
    # sdcflows' brain mask of ds005040's estimator, every second voxel (the half pair's grid)
    mask = common.load(str(_brain_mask(fs._source_node())))[::2, ::2, ::2]
    show = _show(mask, 1)
    grid = core.Grid.around(first.slicer[..., 0] if len(first.shape) == 4 else first, show)
    ex = exm.Example(ctx, grid, show)
    k = 0
    for n in images:
        a = np.asanyarray(nib.load(str(base / n)).dataobj)
        vols = [a[..., t] for t in range(a.shape[3])] if a.ndim == 4 else [a]
        for v in vols:
            k += 1
            ex.add(exm.Panel(f"in{k}", "input", f"{INPUT_TEXT.get(n, n)}"
                             + (f", volume {k}" if a.ndim == 4 else ""),
                             f"phase encoding {pe[k - 1]}; derived from fMRIPrep's merged pair "
                             "(every second voxel: 6 × 6 × 6.6 mm)",
                             {"all": np.ascontiguousarray(v)}))  # fmt: skip
    if run.program == "applytopup":
        src = fs.topup_outputs(run.source)
        ex.add(exm.Panel("tu-field", "input", "the field applied (Hz)",
                         "FSL's topup on the half-resolution pair (the record's opt/levels), "
                         "its --fout: the field its coefficients give",
                         {"all": common.load(str(src / "tu_field.nii.gz"))}))  # fmt: skip
    outs = _outputs_of(e.dir / "fsl", first.shape[:3], pe)
    panels = [p for p in PANELS if p[0] in ("fsl", "replica", "replica-fix")]
    arrays = {pid: _arrays(e.dir / pid, outs) for pid, *_ in panels}
    for pid, role, label, sub in panels:
        sub = sub.replace(" (the record's run, from the oracle cache)", ", the record's run")
        ex.add(exm.Panel(pid, role, label.format(program=run.program), sub, arrays[pid]))
    diffs = _diffs(False)
    outputs = [{"id": k, "label": v[2], "unit": v[3]} for k, v in outs.items()]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    _verdicts(diffs, same, info, run.program)
    out_id = next(iter(outs))
    at, _k = common.largest_in_slice(ex, "replica-fix", "replica", out_id, "axial")
    default = {
        "output": out_id,
        "plane": "axial",
        "voxel": at,
        "panels": [*[p.id for p in ex.panels if p.role == "input"][:3], "fsl", "replica-fix"],
        "diffs": ["replica-fsl", "fix-replica"],
        "compare": {"a": "replica-fix", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default,
        window_panel="replica-fix" if ex_id == "vb3d" else "fsl",
        caption="the crosshair where the default mode differs most from the reproduce mode "
        "(FSL's bytes) in this slice.",
    )  # fmt: skip
    cols = {"replica (reproduce)": e.dir / "replica", "replica (default)": e.dir / "replica-fix"}
    tables = [_files_table(f"{ex_id}-files", e.dir / "fsl", cols, "")]
    title, summary = REAL_TEXT[ex_id]
    data = common.DS005040
    return {
        "id": ex_id,
        "title": title,
        "summary": summary,
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): the "
                "record's inputs derived from fMRIPrep's merged pair (every second voxel; "
                "oracles/inputs/fsl-topup-replica/); shown inside sdcflows' brain mask of the "
                "corrected reference at the same resolution, dilated by one voxel",
            },
            {"label": "Case", "text": f"`{case}` of the record (`{RECORD}`)"},
            {"label": "Command", "text": "`" + command(case, case_table()) + "`"},
            {"label": "Programs", "text": _programs(info)},
            {"label": "Grid", "text": common.describe_grid(first, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: the modes"],
        ],
        "tables": tables,
        "metrics_note": _note(int(np.prod(first.shape[:3]))),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
    }


# ------------------------------------------------------------------------------------ cases


def _input(name: str) -> str:
    if name.startswith("dt:"):
        return f"<random testing's input {name.split(':', 1)[1]}>"
    if name.endswith(".txt"):
        return f"<{name}: acquisition parameters written by the suite>"
    if name.endswith(".cnf"):
        return f"<{name}: a configuration written by the suite>"
    if name.startswith("long"):
        return f"<{name}: ds005040's pair cropped, its lines doubled to 96 voxels>"
    return f"<{name}: derived from ds005040's fMRIPrep pair>"


def _arg(a: str) -> str:
    key, sep, value = a.partition("=")
    if not sep:
        key, value = "", a
    if "@" not in value and "{" not in value:
        return shlex.quote(a) if re.search(r"\s", a) or a == "" else a
    parts = []
    for p in value.split(","):
        if p.startswith("@"):
            parts.append(_input(p[1:]))
        elif p == "{imain}":
            parts.append("<fMRIPrep's merged pair (ds005040)>")
        elif p == "{datain}":
            parts.append("<its encoding file>")
        else:
            parts.append(p)
    return f"{key}{sep}{','.join(parts)}"


def command_fmriprep(args: list[str]) -> str:
    """fMRIPrep's call as the record runs it, its files named by what they are."""
    subs = {"--config": "<sdcflows' b02b0.cnf>", "--datain": "<nipype's encoding file>",
            "--imain": "<fMRIPrep's merged pair>"}  # fmt: skip
    out = []
    for a in args:
        k, sep, _v = a.partition("=")
        out.append(f"{k}={subs[k]}" if sep and k in subs else a)
    return "topup " + " ".join(out)


def command(case: str, defs: dict[str, tuple[str, object]]) -> str:
    if case.startswith("fmriprep/"):
        node = _node(case.split("/", 1)[1])
        return command_fmriprep(_suite().fmriprep_args(node))
    if case not in defs or defs[case][1] is None:
        return ""
    run = defs[case][1]
    parts = [run.program, *(_arg(a) for a in run.args)]
    env = " ".join(f"{k}={v}" for k, v in run.settings if v is not None)
    unset = " ".join(f"{k} unset" for k, v in run.settings if v is None)
    text = " ".join(x for x in (env, " ".join(parts), f"({unset})" if unset else "") if x)
    if run.files:
        text += f" (with {', '.join(_input(f) for f in run.files)} in its directory)"
    if run.source:
        src = _suite().TOPUP_SOURCES.get(run.source, "opt/levels")
        what = f"the case {src}" if isinstance(src, str) else "a short run of its own"
        if run.source == "zero":
            what += ", every coefficient set to 0"
        dropped = f", without {', '.join(run.drop)}" if run.drop else ""
        text += f" (tu_*: FSL's topup outputs of {what}{dropped})"
    return text


def case_verdict(r: dict, dm: dict, run) -> tuple[str, str]:
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
        notes.append(f"fMRIPrep's own files (its image's topup): {own} the same")
    band = next((v for k, v in rep.items() if k.startswith("FSL at 4 threads against")), None)
    if band is not None:
        notes.append(
            f"the replica at four threads gives FSL's one-thread result; FSL's own run at four "
            f"threads keeps {band} files the same (its thread band, FSL-116)"
        )
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
    if run is not None and getattr(run, "profile", False):
        notes.append("the profile's durations (the clock's) masked before comparing")
    return "bit-identical", common.clip("; ".join(notes), 600)


def cases(rec, examples: dict[str, str], default: dict[str, dict]) -> list[dict]:
    defs = case_table()
    out = []
    for r in rec.results:
        desc, run = defs.get(r["case"], ("", None))
        if r["case"].startswith("fmriprep/"):
            desc = (
                "fMRIPrep's field-map estimation call ("
                + r["case"].split("/")[1].split("-noreconall")[0]
                + "; sdcflows' b02b0.cnf)"
            )
        v, why = case_verdict(r, default.get(r["case"], {}), run)
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


# ------------------------------------------------------------------------------ the trial


def trial(ctx: core.Context) -> tuple[dict, dict]:
    """Random testing's trial of this part, from its ``summary.json``: a table, and its
    provenance."""
    path = common.oracles() / "difftest" / TRIAL / "summary.json"
    raw = path.read_bytes()
    s = json.loads(raw)
    t = s["tools"]["fsl-topup"]
    o = t["options"]
    agree = t["implementations"]["replica"].get("agree", 0)
    shipped = sum(v for k, v in o.items() if k.startswith("--config=") and "/" not in k)
    rows = [
        ("Random cases, each compared with FSL 6.0.7.23", t["cases"]),
        ("bit-identical (every file, the messages, the exit status)", agree),
        ("differing", len(t["differences"])),
        ("fMRIPrep's call (sdcflows' b02b0.cnf, every output)", o.get("fMRIPrep's call", 0)),
        ("a configuration file (`--config`)", o.get("--config", 0)),
        ("FSL's shipped configurations by name (b02b0, _1, _2, _4, _7T, test_acqp)", shipped),
        ("one, two or three levels", [o.get(f"{n} levels", 0) for n in (1, 2, 3)]),
        ("`--acqp` instead of `--datain`", o.get("--acqp", 0)),
        ("`--debug`, `--trace`", [o.get("--debug", 0), o.get("--trace", 0)]),
        ("applytopup `--method=jac`, `lsr`", [o.get("--method=jac", 0), o.get("--method=lsr", 0)]),
        ("applytopup `vb2D`, `vb3D`, `vb4D`", [o.get(f"--method=vb{d}D", 0) for d in (2, 3, 4)]),
        ("distinct options and settings drawn", t["options_covered"]),
        ("length (minutes; 12 jobs at one thread each)", round(s["elapsed_s"] / 60, 1)),
    ]
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
    table = common.table(
        "trial",
        f"Random testing: the trial of {s['date']}",
        ["", {"label": "Cases", "num": True}],
        out_rows,
        note=f"Differential random testing's trial of this part (seed `{s['seed']}`, larmorx "
        f"`{s['larmorx_commit']}`): EPI-like phantom heads on grids of 16 to 24 voxels per "
        "side, each volume distorted along its phase encoding by one off-resonance field (a "
        "bump of 25 to 120 Hz and a gradient) with small movements in some; pairs along y or "
        "x, three or four volumes; float, 16-bit and 64-bit images; FSL's four output types; "
        "applytopup on FSL's topup result of the case's own pair (through the oracle cache, "
        f"`use` mode). Every case in a memory box of {s['memory']['case']} (peaks below "
        f"{peaks:.2f} GiB); the record's own refusals, failures and thread counts are left to "
        "the record.",
    )
    prov = {
        "trial": f"oracles/difftest/{TRIAL}/summary.json",
        "sha256": hashlib.sha256(raw).hexdigest(),
    }
    return table, prov


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, RECORD)
    examples = {
        **{f"fmriprep/{run}": ex_id for ex_id, run, _f in FMRIPREP},
        **{case: ex_id for ex_id, case in REAL.items()},
    }
    ne = named_entry(ctx)
    cs = cases(rec, examples, ne.info().get("default_mode", {}))
    entries = [fmriprep_entry(ctx, run) for _i, run, _f in FMRIPREP]
    entries += [real_entry(ctx, case) for case in REAL.values()]
    entries.append(ne)
    tt, tprov = trial(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "topup and applytopup",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": [],
            "cache": [fcache.provenance(x) for x in entries],
            "difftest": tprov,
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
        "tables": [tt],
        "examples": [_example_fmriprep(ctx, *FMRIPREP[0]), _example_fmriprep(ctx, *FMRIPREP[1])]
        + [_example_real(ctx, ex_id) for ex_id in REAL],
    }


if __name__ == "__main__":  # compute the cache entries only: [fmriprep RUN | real CASE | named]
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    what = sys.argv[1:] or ["all"]
    if what[0] in ("fmriprep", "all"):
        for _i, run, _f in FMRIPREP:
            if len(what) < 2 or what[1] == run:
                print("done", fmriprep_entry(c, run).dir)
    if what[0] in ("real", "all"):
        for case in REAL.values():
            if len(what) < 2 or what[1] == case:
                print("done", real_entry(c, case).dir)
    if what[0] in ("named", "all"):
        print("done", named_entry(c).dir)
