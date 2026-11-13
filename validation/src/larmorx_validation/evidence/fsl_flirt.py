# SPDX-License-Identifier: Apache-2.0
"""FLIRT's evidence (the larmorx-nc replica, FSL Licence, and larmorx's clean-room original,
Apache-2.0): fMRIPrep's boundary-based registration call (``fsl_bbr_wf/flt_bbr``) on three
oracle runs, a default 12-dof registration and ``-applyxfm`` on four volumes of a raw BOLD
series, by FSL 6.0.7.23, by the replica and by the clean-room original, each in its
``reproduce`` and default modes (and at one thread); FSL's own variability (its input perturbed
by noise of half an intensity unit, the record's band) for each registration the original is
shown on; the matrices as text with fMRIPrep's own, the displacement each pair of matrices gives
over the reference's brain, every case of both records, random testing's trial of the original
after the clean-room catch-up (2026-11-05) with its triage, and the benchmark's speed.

Every run of FSL is a run of the parity record (``parity.fsl_flirt_replica``), with the same
arguments, environment and inputs, through the oracle-output cache: the page computes no
original result of its own but two: the BOLD reference resampled at fMRIPrep's initial
alignment (``-applyxfm`` with the call's ``-init``), which shows what BBR changed, and FSL on its
perturbed input (never cached: variation is measured fresh). The replica runs as a separate
program; nothing here imports it. The clean-room original runs as ``python -m larmorx`` from
this checkout's build. Everything shown lies inside the
reference's brain mask (fMRIPrep's masked T1w), dilated by two voxels: no face can show.
"""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
import shlex
import shutil
import sys
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core, fsl_pages
from larmorx_validation.evidence import present as present_
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "fsl-flirt"
RECORD = "fsl-flirt-replica"
#: The BBR examples: example id, the fMRIPrep run, its view, the run's words.
BBR = [
    ("fmriprep-bbr", "ds005040-sub-002-noreconall", "volume",
     "a young adult's BOLD reference (ds005040, slice timing, field maps)"),
    ("panel-ds003592-older", "ds003592-sub-280-noreconall", "slices",
     "an 82-year-old's first echo (ds003592, multi-echo)"),
    ("panel-ds000228-child", "ds000228-sub-pixar008-noreconall", "slices",
     "a three-year-old's BOLD reference (ds000228)"),
]  # fmt: skip
DATA = {
    "ds005040-sub-002-noreconall": common.DS005040,
    "ds003592-sub-280-noreconall": None,  # from the data panel (``_data``)
    "ds000228-sub-pixar008-noreconall": None,
}
#: The replica's runs: label, D13 mode, threads.
REPLICA_RUNS = [("replica", "reproduce", "4"), ("replica-1", "reproduce", "1"),
                ("replica-fix", "fix", "4")]  # fmt: skip
#: The record's real cases shown as examples: example id -> case.
REAL = {"dof12": "real/default", "applyxfm-4d": "real/applyxfm-4d"}
OUTPUTS = [{"id": "registered", "label": "Registered input (-out)", "unit": ""}]


def _data(run: str) -> dict:
    d = DATA.get(run)
    if d is not None:
        return d
    from larmorx_validation.evidence.ants_panel import data

    return data(run.split("-sub-")[0])


# --------------------------------------------------------------------- running the programs


def _suite():
    from larmorx_validation.parity import fsl_flirt_replica as fp

    return fp


def _oracle(e: fcache.Entry, label: str, args: list[str], inputs: dict[str, Path]) -> None:
    """FSL's flirt with ``args`` (``@name`` an input copied into its working directory first, as
    the record's case does) through the oracle-output cache, its files into ``<entry>/<label>``;
    whether it was served is kept in the entry."""
    fp = _suite()
    with tempfile.TemporaryDirectory(prefix="lx-nc-flirt-ev_") as t:
        cwd = Path(t)
        argv = []
        for a in args:
            if a.startswith("@"):
                shutil.copy(inputs[a[1:]], cwd / a[1:])
                argv.append(a[1:])
            else:
                argv.append(a)
        p = fp.oracle_run("flirt", argv, cwd=cwd, settings={})
        if p.returncode != 0:
            raise RuntimeError(f"{label}: FSL's flirt exited {p.returncode}: {p.stderr[-800:]}")
        out = e.dir / label
        out.mkdir(exist_ok=True)
        for f in cwd.iterdir():
            if f.name not in {a[1:] for a in args if a.startswith("@")}:
                shutil.copy(f, out / f.name)
    run = fcache.Run(label, [str(fp.fsldir() / "bin" / "flirt"), *argv], fp._environment({}))
    run.exit, run.seconds = p.returncode, round(p.seconds, 2)
    run.stderr_tail = p.stderr[-2000:]
    e.runs.append(run)
    e.extra.setdefault("oracle_cache", {})[label] = "served" if p.cached else "ran"


def _replica(e: fcache.Entry, args: list[str], inputs: dict[str, Path]) -> None:
    """The replica on ``args``, once per :data:`REPLICA_RUNS`, each in ``<entry>/<label>``."""
    fp = _suite()
    exe = fp.replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    env = fp._environment({})
    unset = tuple(k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_")))
    for label, mode, threads in REPLICA_RUNS:
        cwd = e.dir / label
        cwd.mkdir(exist_ok=True)
        argv = []
        for a in args:
            if a.startswith("@"):
                shutil.copy(inputs[a[1:]], cwd / a[1:])
                argv.append(a[1:])
            else:
                argv.append(a)
        run_env = {**env, "LARMORX_UPSTREAM_DEFECTS": mode, "OMP_NUM_THREADS": threads}
        # in its own directory: the replica writes its outputs where FSL does
        e.run(label, [str(exe), "fsl", "flirt", *argv], env=run_env, unset=unset, cwd=cwd)
        for a in args:
            if a.startswith("@"):
                (cwd / a[1:]).unlink()


def versions() -> dict[str, str]:
    from larmorx_validation.parity import fsl_versions
    from larmorx_validation.parity.fsl_mcflirt_replica import replica_version

    fp = _suite()
    return {
        "fsl": f"FSL {fsl_versions.version_of(fp.fsldir())} (fsl-flirt 2111.4; ~/fsl-6.0.7.23, "
        "never on the PATH, OpenBLAS on its Haswell kernels)",
        "larmorx-nc": replica_version(),
    }


def _bbr_node(run: str) -> Path:
    fp = _suite()
    nodes = [n for r, n in fp.fmriprep_calls() if r == run]
    if not nodes:
        raise FileNotFoundError(f"fMRIPrep's BBR node of {run}")
    return nodes[0]  # the record's standard-tier call (the run's first)


def _arg(args: list[str], option: str) -> str:
    return args[args.index(option) + 1]


def _compute_bbr(run: str):
    def compute(e: fcache.Entry) -> None:
        fp = _suite()
        node = _bbr_node(run)
        args = fp.fmriprep_args(run, node)
        for o in ("-in", "-ref", "-wmseg", "-init"):
            e.input(f"{o} (fMRIPrep's)", Path(_arg(args, o)))
        _oracle(e, "fsl", args, {})
        _replica(e, args, {})
        # The BOLD reference at fMRIPrep's initial alignment (the call's -init): what BBR changed
        init = ["-in", _arg(args, "-in"), "-ref", _arg(args, "-ref"), "-applyxfm", "-init",
                _arg(args, "-init"), "-out", "init"]  # fmt: skip
        _oracle(e, "init", init, {})
        e.finish(
            versions=versions(),
            command=(node / "command.txt").read_text(encoding="utf-8").strip(),
            node=str(node.relative_to(common.fmriprep_run(run))),
            settings="fMRIPrep 25.2.6's BBR node (fsl_bbr_wf/flt_bbr) of the run, its arguments "
            "from command.txt mapped onto the run's directories as the record's case maps them "
            "(FSL's own bbr.sch), by FSL 6.0.7.23 through the oracle-output cache (the record's "
            "own run) and by the larmorx-nc replica: reproduce mode at 4 and 1 threads, the "
            "default mode at 4; the BOLD reference at the initial alignment by FSL's -applyxfm "
            "with the call's -init",
        )

    return compute


def bbr_entry(ctx: core.Context, run: str) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"fmriprep-{run}",
        f"fMRIPrep's BBR call of {run}: FSL 6.0.7.23 (the record's run, from the oracle cache) "
        "and the replica, reproduce and default modes; the initial alignment by -applyxfm",
        _compute_bbr(run),
    )


def _real_args(case: str) -> tuple[list[str], dict[str, Path]]:
    fp = _suite()
    run = fp._all_cases()[case][2]
    base = fp.inputs()
    files = {a[1:]: base / a[1:] for a in run.args if a.startswith("@")}
    return [*run.args], files


def _compute_real(case: str):
    def compute(e: fcache.Entry) -> None:
        args, files = _real_args(case)
        for name, f in files.items():
            e.input(name, f)
        _oracle(e, "fsl", args, files)
        _replica(e, args, files)
        e.finish(
            versions=versions(),
            command="flirt " + " ".join(a.lstrip("@") for a in args),
            settings=f"The record's case {case}, its inputs from the suite's derived inputs "
            "(oracles/inputs/fsl-flirt-replica/, made from ds005040's fMRIPrep run), by FSL "
            "6.0.7.23 through the oracle-output cache (the record's own run) and by the "
            "larmorx-nc replica: reproduce mode at 4 and 1 threads, the default mode at 4",
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


# ------------------------------------------------------------- the clean-room original

#: The clean-room original's record (standard tier, against FSL 6.0.7.23).
ORIG_RECORD = "fsl-flirt"
#: The clean-room original's runs (crate larmorx-fsl, Apache-2.0): label, D13 mode, threads;
#: its record runs it in reproduce mode at 4 threads.
ORIG_RUNS = [("orig", "reproduce", "4"), ("orig-1", "reproduce", "1"),
             ("orig-fix", "fix", "4")]  # fmt: skip
#: FSL's own variability (the record's band, ``parity.fsl_flirt._band``): FSL on its input with
#: uniform noise of ±0.5 intensity units (this seed), its matrix against its unperturbed one.
BAND_SEED = 20261102


def versions_orig() -> dict[str, str]:
    import larmorx

    return {
        "fsl": versions()["fsl"],
        "larmorx": f"larmorx {larmorx.__version__} (crate larmorx-fsl: the clean-room original, "
        "Apache-2.0, written from specs/flirt.md)",
    }


def _original(e: fcache.Entry, args: list[str], inputs: dict[str, Path]) -> None:
    """The clean-room original on ``args`` (``python -m larmorx fsl flirt`` with
    ``LARMORX_IMPLEMENTATION=original``, this checkout's build), once per :data:`ORIG_RUNS`,
    each in ``<entry>/<label>``, in FSL's clean environment as the record runs it."""
    fp = _suite()
    env = fp._environment({})
    env["LARMORX_IMPLEMENTATION"] = "original"
    for k in ("PYTHONPATH", "VIRTUAL_ENV"):
        if k in os.environ:
            env[k] = os.environ[k]
    unset = tuple(k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_")))
    for label, mode, threads in ORIG_RUNS:
        cwd = e.dir / label
        cwd.mkdir(exist_ok=True)
        argv = []
        for a in args:
            if a.startswith("@"):
                shutil.copy(inputs[a[1:]], cwd / a[1:])
                argv.append(a[1:])
            else:
                argv.append(a)
        run_env = {**env, "LARMORX_UPSTREAM_DEFECTS": mode, "OMP_NUM_THREADS": threads}
        e.run(label, [sys.executable, "-m", "larmorx", "fsl", "flirt", *argv], env=run_env,
              unset=unset, cwd=cwd)  # fmt: skip
        for a in args:
            if a.startswith("@"):
                (cwd / a[1:]).unlink()


def _perturbed(e: fcache.Entry, args: list[str], inputs: dict[str, Path]) -> None:
    """FSL's flirt on ``args`` with its ``-in`` perturbed (:data:`BAND_SEED`), its files into
    ``<entry>/fsl-perturbed``: FSL's own variability, through the oracle-output cache in use
    mode, never cached (variation is measured fresh)."""
    import nibabel as nib

    from larmorx_validation import oracle_cache

    fp = _suite()
    i = args.index("-in")
    src = args[i + 1]
    path = inputs[src[1:]] if src.startswith("@") else Path(src)
    img = nib.load(str(path))
    data = np.asanyarray(img.dataobj).astype(np.float32)
    rng = np.random.default_rng(BAND_SEED)
    noisy = data + rng.uniform(-0.5, 0.5, data.shape).astype(np.float32)
    out = nib.Nifti1Image(noisy, img.affine, img.header)
    out.set_data_dtype(np.float32)
    name = "perturbed_input.nii"
    with tempfile.TemporaryDirectory(prefix="lx-flirt-ev-band_") as t:
        cwd = Path(t)
        nib.save(out, str(cwd / name))
        argv, copied = [], {name}
        for j, a in enumerate(args):
            if j == i + 1:
                argv.append(name)
            elif a.startswith("@"):
                shutil.copy(inputs[a[1:]], cwd / a[1:])
                argv.append(a[1:])
                copied.add(a[1:])
            else:
                argv.append(a)
        command = [str(fp.fsldir() / "bin" / "flirt"), *argv]
        p = oracle_cache.run(
            command,
            env=fp._environment({}),
            cwd=cwd,
            outputs=[cwd],
            policy=oracle_cache.Policy(
                threads=1, family="fsl", never="FSL's own variability: a perturbed input"
            ),
            runner="evidence-fsl-flirt",
            inputs=[q for q in sorted(cwd.rglob("*")) if q.is_file()],
            timeout=3600,
        )
        if p.returncode != 0:
            raise RuntimeError(f"fsl-perturbed: FSL's flirt exited {p.returncode}")
        dest = e.dir / "fsl-perturbed"
        dest.mkdir(exist_ok=True)
        for f in cwd.iterdir():
            if f.name not in copied:
                shutil.copy(f, dest / f.name)
    run = fcache.Run("fsl-perturbed", command, fp._environment({}))
    run.exit, run.seconds = p.returncode, round(p.seconds, 2)
    run.stderr_tail = p.stderr[-2000:]
    e.runs.append(run)
    e.extra.setdefault("oracle_cache", {})["fsl-perturbed"] = "ran (never cached)"
    e.extra["band"] = (
        f"FSL 6.0.7.23 on the same call with its -in perturbed by uniform noise of ±0.5 "
        f"intensity units (numpy's default_rng({BAND_SEED}), float32, as the record's band)"
    )


def _compute_orig_bbr(run: str):
    def compute(e: fcache.Entry) -> None:
        fp = _suite()
        node = _bbr_node(run)
        args = fp.fmriprep_args(run, node)
        for o in ("-in", "-ref", "-wmseg", "-init"):
            e.input(f"{o} (fMRIPrep's)", Path(_arg(args, o)))
        _original(e, args, {})
        _perturbed(e, args, {})
        e.finish(
            versions=versions_orig(),
            command=(node / "command.txt").read_text(encoding="utf-8").strip(),
            node=str(node.relative_to(common.fmriprep_run(run))),
            settings="fMRIPrep 25.2.6's BBR node of the run, its arguments as the record's case "
            "maps them, by larmorx's clean-room original (python -m larmorx fsl flirt, "
            "LARMORX_IMPLEMENTATION=original): reproduce mode at 4 and 1 threads, the default "
            "mode at 4; and FSL 6.0.7.23 on the same call with its input perturbed (FSL's own "
            "variability, through the oracle-output cache, never cached)",
        )

    return compute


def orig_bbr_entry(ctx: core.Context, run: str) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"fmriprep-{run}-original",
        f"fMRIPrep's BBR call of {run}: the clean-room original, reproduce and default modes, "
        "and FSL 6.0.7.23 on a perturbed input",
        _compute_orig_bbr(run),
    )


def _compute_orig_real(case: str):
    def compute(e: fcache.Entry) -> None:
        args, files = _real_args(case)
        for name, f in files.items():
            e.input(name, f)
        _original(e, args, files)
        if "-omat" in args:
            _perturbed(e, args, files)
        e.finish(
            versions=versions_orig(),
            command="flirt " + " ".join(a.lstrip("@") for a in args),
            settings=f"The record's case {case}, its inputs from the suite's derived inputs, by "
            "larmorx's clean-room original: reproduce mode at 4 and 1 threads, the default mode "
            "at 4"
            + (
                "; and FSL 6.0.7.23 on the same call with its input perturbed (FSL's own "
                "variability, through the oracle-output cache, never cached)"
                if "-omat" in args
                else ""
            ),
        )

    return compute


def orig_real_entry(ctx: core.Context, case: str) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        case.replace("/", "-") + "-original",
        f"The record's case {case}: the clean-room original"
        + (", and FSL 6.0.7.23 on a perturbed input" if case != "real/applyxfm-4d" else ""),
        _compute_orig_real(case),
    )


def refresh_original(ctx: core.Context, why: str) -> None:
    """After a change of the clean-room original: its runs (:data:`ORIG_RUNS`) of every
    example's entry already computed made again, in their working directories with their inputs
    (an entry not computed yet is computed whole by the export); FSL's run on a perturbed input
    (FSL's own variability) is kept: the original's change does not touch it."""
    fctx = ctx.figures_context()
    fp = _suite()
    sources = [(f"fmriprep-{run}-original", fp.fmriprep_args(run, _bbr_node(run)), {})
               for _i, run, _v, _w in BBR]  # fmt: skip
    for case in REAL.values():
        sources.append((case.replace("/", "-") + "-original", *_real_args(case)))
    for name, args, inputs in sources:
        e = fcache.Entry(fctx, TOOL, name, "")
        if not e.done:
            continue
        info = e.info()
        e.runs, e.extra = [], {}
        for label, _m, _t in ORIG_RUNS:  # the outputs being made again
            for f in (e.dir / label).iterdir():
                shutil.rmtree(f) if f.is_dir() else f.unlink()
        _original(e, args, inputs)
        fcache.merge_rerun(e, info, why)


# ------------------------------------------------------------------------- matrices


def fsl_coords(img) -> tuple[np.ndarray, np.ndarray]:
    """FLIRT's millimetres of every voxel index (index times voxel size, the first axis
    reversed for an image stored neurologically, specs/flirt.md §3) as a 4 × 4 matrix from
    voxel indices, and the voxel sizes."""
    shape = img.shape[:3]
    vox = np.array(img.header.get_zooms()[:3], dtype=np.float64)
    m = np.diag([*vox, 1.0])
    if np.linalg.det(img.affine[:3, :3]) > 0:
        m[0, 0] = -vox[0]
        m[0, 3] = (shape[0] - 1) * vox[0]
    return m, vox


def displacement(a: np.ndarray, b: np.ndarray, ref, brain: np.ndarray) -> dict:
    """How far apart two FLIRT matrices (input mm -> reference mm) put the input's points
    under every voxel of the reference's brain: |A⁻¹x − B⁻¹x| in the input's millimetres, x
    each brain voxel's FLIRT millimetres; float64 over every voxel of ``brain``."""
    s, _ = fsl_coords(ref)
    idx = np.argwhere(brain).astype(np.float64)
    x = idx @ s[:3, :3].T + s[:3, 3]
    d = np.linalg.inv(a) - np.linalg.inv(b)
    v = x @ d[:3, :3].T + d[:3, 3]
    n = np.sqrt((v * v).sum(axis=1))
    return {
        "max": float(n.max()) if n.size else 0.0,
        "mean": float(n.mean()) if n.size else 0.0,
        "p95": float(np.percentile(n, 95)) if n.size else 0.0,
        "voxels": int(n.size),
    }


def rms80(a: np.ndarray, b: np.ndarray) -> float:
    """FSL's RMS deviation of two matrices (a sphere of radius 80 mm about the origin), as
    the record reports it; 0 for two equal matrices (the product with the inverse is not
    exactly the identity in floating point)."""
    if np.array_equal(a, b):
        return 0.0
    return _suite()._rms80(a, b)


def _mat_text(path: Path) -> list[str]:
    return [" ".join(ln.split()) for ln in path.read_text(encoding="utf-8").splitlines() if ln]


def _matrix_table(
    tid: str, mats: dict[str, Path], labels: dict[str, str], note: str, other=("init",)
) -> dict:
    """The matrices as the files hold them: one column each, a row per matrix row; the last
    row says whether each file is the first one's, byte for byte (``other``: columns that are
    not a result of the same call, not compared)."""
    texts = {k: _mat_text(p) for k, p in mats.items()}
    rows = []
    for i in range(4):
        rows.append(
            {
                "head": f"row {i + 1}",
                "cells": [common.cell(texts[k][i] if i < len(texts[k]) else "") for k in mats],
            }
        )
    same = {k: mats[k].read_bytes() == mats[next(iter(mats))].read_bytes() for k in list(mats)[1:]}
    rows.append(
        {
            "head": "the file, byte for byte",
            "cells": [common.cell("the reference")]
            + [
                common.cell("–" if k in other else "identical" if same[k] else "differs")
                for k in list(mats)[1:]
            ],
        }
    )
    return common.table(
        tid,
        "The matrices, as the files hold them (input millimetres to reference millimetres, "
        "FLIRT's convention)",
        ["", *[labels[k] for k in mats]],
        rows,
        note=note,
    )


def _shift_table(tid: str, pairs: list[tuple[str, np.ndarray, np.ndarray]], ref, brain) -> dict:
    rows = []
    for head, a, b in pairs:
        d = displacement(a, b, ref, brain)
        rows.append(
            {
                "head": head,
                "cells": [
                    common.cell(d["max"], "sig"),
                    common.cell(d["mean"], "sig"),
                    common.cell(d["p95"], "sig"),
                    common.cell(rms80(a, b), "sig"),
                ],
                "verdict": None,
            }
        )
    n = displacement(pairs[0][1], pairs[0][2], ref, brain)["voxels"]
    return common.table(
        tid,
        "The displacement between two matrices over the reference's brain (mm)",
        [
            "Matrices",
            {"label": "largest", "num": True},
            {"label": "mean", "num": True},
            {"label": "95th percentile", "num": True},
            {"label": "FSL's RMS deviation (80 mm)", "num": True},
        ],
        rows,
        note=f"For every one of the {n:,} voxels of the reference's brain mask (its voxels "
        "above zero), the distance in millimetres between the two points of the input the "
        "matrices map onto it, computed in float64 from the matrices as the files hold them; "
        "the last column is FSL's own measure (rmsdiff's: the RMS deviation over a sphere of "
        "radius 80 mm about the origin).",
    )


# -------------------------------------------------------------------------- examples


PANELS = [
    ("fsl", "original", "FSL 6.0.7.23, flirt", "the conda installation, as built (the record's "
     "run, from the oracle cache)"),
    ("replica", "larmorx", "larmorx-nc replica", "reproduce mode, 4 threads"),
    ("replica-1", "larmorx", "larmorx-nc replica", "reproduce mode, 1 thread"),
    ("replica-fix", "larmorx", "larmorx-nc replica", "default mode (D13 corrections), 4 threads"),
]  # fmt: skip


def _diffs(with_init: bool) -> list[dict]:
    d = common.d
    out = [
        d("replica-fsl", "replica", "fsl", "main", "replica − FSL 6.0.7.23", common.ROLE_LX),
        d("replica1-replica", "replica-1", "replica", "check",
          "replica, 1 thread − 4 threads", common.ROLE_SELF),
        d("fix-replica", "replica-fix", "replica", "check",
          "replica, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    if with_init:
        out.append(
            d(
                "fsl-init",
                "fsl",
                "init",
                "reference",
                "FSL's result − its initial alignment",
                "what the registration changed",
            )
        )
    return out


def _verdicts(diffs: list[dict], same: dict[str, bool], what: str, mats_same: bool) -> None:
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if x["kind"] == "reference":
            x["verdict"] = None
            x["reason"] = (
                "Not a comparison of implementations: FSL's registered image against the same "
                "input resampled at the call's initial matrix (fMRIPrep's mri_coreg, converted "
                "by lta_to_fsl), the change the boundary-based optimisation made."
            )
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "replica-fsl": f"The same values, every one, on {what}"
                + ("; the matrices too, byte for byte as text." if mats_same else "."),
                "replica1-replica": "The thread count changes nothing: FLIRT's parallel steps "
                "(the boundary points, the blurs) give the same values at any thread count.",
                "fix-replica": "No correction of the register applies to this call: both modes "
                "give the same values.",
            }.get(i, "The same values, every one.")
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example (see the record)."


#: The clean-room original's panels (from its entry).
ORIG_PANELS = [
    ("orig", "larmorx", "larmorx's clean-room original",
     "reproduce mode, 4 threads (Apache-2.0, crate larmorx-fsl; the record's run)"),
    ("orig-1", "larmorx", "larmorx's clean-room original", "reproduce mode, 1 thread"),
    ("orig-fix", "larmorx", "larmorx's clean-room original",
     "default mode (D13 corrections), 4 threads"),
]  # fmt: skip


def _orig_diffs() -> list[dict]:
    d = common.d
    return [
        d("orig-fsl", "orig", "fsl", "main", "clean-room original − FSL 6.0.7.23", common.ROLE_LX),
        d("orig-replica", "orig", "replica", "check", "clean-room original − replica",
          common.ROLE_SELF),
        d("orig1-orig", "orig-1", "orig", "check", "original, 1 thread − 4 threads",
          common.ROLE_SELF),
        d("origfix-orig", "orig-fix", "orig", "check", "original, default − reproduce mode",
          common.ROLE_SELF),
    ]  # fmt: skip


#: The C library's functions FLIRT's clean-room original computes as glibc 2.35 does in its
#: reproduce mode since 2026-11-04 (the rotations, the decomposition, the blurs, the sinc
#: kernel, the mutual information), correctly rounded by default (GLIBC-07).
GLIBC_FUNCTIONS = "sinf, cosf, atan2f, cos, exp and logf"


def _named(o: fcache.Entry, label: str) -> list[str]:
    """The register entries the run ``label`` of ``o`` named on its standard error."""
    log = o.dir / f"{label}.log"
    text = log.read_text(encoding="utf-8") if log.is_file() else ""
    return sorted(set(re.findall(r"\bFSL-\d+\b", text.split("--- stderr", 1)[-1])))


def _orig_verdicts(
    diffs: list[dict],
    same: dict[str, bool],
    o: fcache.Entry,
    shift: dict | None,
    band: dict | None,
    threshold: float,
    background: float | None = None,
    fix_shift: dict | None = None,
) -> None:
    """The clean-room original's comparisons: its matrix against FSL's beside FSL's own
    variability (``shift``: the original's displacement from FSL's matrix, ``band``: FSL's on a
    perturbed input, both over the reference's brain; ``None`` for ``-applyxfm``, where both use
    the given matrix), its threads and its two modes (``fix_shift``: the default's matrix
    against the reproduce mode's); ``background``: FSL's output outside the input's field of
    view (its first voxel), to say where the images differ most."""
    for x in diffs:
        i = x["id"]
        if not i.startswith("orig"):
            continue
        x["links"] = [{"label": "record", "href": f"docs/validation/{ORIG_RECORD}.md"}]
        m = x["metrics"].get("registered", {})
        if i in ("orig-fsl", "orig-replica"):
            who = "FSL's" if i == "orig-fsl" else "the replica's (FSL's bytes)"
            if same[i]:
                x["verdict"] = "bit-identical"
                x["reason"] = f"The clean-room original gives {who} values, every one."
                continue
            edge = background is not None and any(
                abs(float(m.get(k, np.nan)) - background) <= 1e-6 * max(1.0, abs(background))
                for k in ("a_at", "b_at")
            )
            img = (
                f"the images differ in {m.get('share', 0):.1%} of their values, by at most "
                f"{m.get('max_rel_range', 0):.2g} of the range (1 − NCC "
                f"{m.get('one_minus_ncc', 0):.2g}"
                + (
                    "; the largest where a voxel at the edge of the input's field of view is "
                    "inside it for one matrix and outside, at the background value, for the other)"
                    if edge
                    else ")"
                )
            )
            if shift is None:
                x["verdict"] = "open"
                x["reason"] = (
                    f"From the same matrix (-applyxfm), {img}: the windowed-sinc interpolation "
                    "follows FSL's within the last bits of its float arithmetic, and how FSL "
                    "rounds its kernel's weights is an open question of the specification "
                    "(docs/findings/fsl-flirt-clean-room.md). The record holds such images to "
                    "1e-5 of their range."
                )
                continue
            within = band is not None and shift["p95"] <= band["p95"]
            x["verdict"] = "within-variability" if within else "open"
            x["reason"] = (
                f"A clean-room implementation, so not bit-identical by design: its matrix is "
                f"{shift['p95']:.3g} mm from {who} at the 95th percentile of the brain (at most "
                f"{shift['max']:.3g} mm)"
                + (
                    f", against FSL's own {band['p95']:.3g} mm when its input changes by noise "
                    f"of half an intensity unit (at most {band['max']:.3g} mm)"
                    if band is not None
                    else ""
                )
                + f"; {img}, the input interpolated at slightly other positions. "
                + (
                    "The difference is within FSL's own variability."
                    if within
                    else f"Within the record's threshold ({threshold:g} mm), above FSL's own "
                    "variability on this input: the cost's last bits and the optimiser's path "
                    "(docs/findings/fsl-flirt-clean-room.md)."
                )
            )
        elif i == "orig1-orig":
            x["verdict"] = "bit-identical" if same[i] else "open"
            x["reason"] = (
                "The thread count changes nothing: the original's parallel steps (the boundary "
                "points' terms, the correlation ratio's bins, the search's grid points) are "
                "summed in a fixed order, so the values are the same at any thread count."
                if same[i]
                else "The original's result should not depend on its thread count; here it does "
                "(see the record)."
            )
        elif i == "origfix-orig":
            names = _named(o, "orig-fix")
            if same[i]:
                x["verdict"] = "bit-identical"
                x["reason"] = (
                    "No correction of the register applies to this call: both modes give the "
                    "same values."
                )
            elif names:
                x["verdict"] = "d13"
                x["reason"] = (
                    "The default corrects FSL's deterministic defects and names them: "
                    + ", ".join(names)
                    + f"; it also computes {GLIBC_FUNCTIONS} correctly rounded (GLIBC-07, not "
                    "named); reproduce mode gives FSL's behaviour."
                )
                x["links"] += [{"label": n, "href": common.GLIBC07_REGISTER} for n in names]
            else:
                x["verdict"] = "d13"
                moved = (
                    f" Here the two modes' matrices part by {fix_shift['p95']:.3g} mm at the "
                    f"95th percentile of the brain (at most {fix_shift['max']:.3g} mm)"
                    + (
                        f", against FSL's own {band['p95']:.3g} mm on a perturbed input"
                        if band is not None
                        else ""
                    )
                    + ": the fit's sensitivity to the C library's last bits."
                    if fix_shift is not None
                    else " Here the two modes' images differ in "
                    f"{m.get('share', 0):.1%} of their values."
                )
                x["reason"] = common.glibc07(GLIBC_FUNCTIONS, "FLIRT", named=False) + moved
                x["links"].append({"label": "GLIBC-07", "href": common.GLIBC07_REGISTER})


def _flirt_threshold(args: list[str]) -> float:
    """The record's fixed threshold for a registration's matrix (``parity.fsl_flirt``)."""
    from larmorx_validation.parity import fsl_flirt as fo

    return fo._threshold([a.lstrip("@") for a in args])


def _note(n: int, extra: str = "") -> str:
    return (
        f"Δ is A − B, computed in float64 over all {n:,} voxels of the outputs, not only those "
        "shown. The record's threshold: every file FSL writes bit-identical (images "
        "decompressed, `descrip` left out), with FSL's exit status and messages." + extra
    )


def _brain(ref_path: Path) -> np.ndarray:
    return common.load(str(ref_path)) > 0


def _example_bbr(ctx: core.Context, ex_id: str, run: str, view: str, words: str) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = bbr_entry(ctx, run)
    o = orig_bbr_entry(ctx, run)
    info, oinfo = e.info(), o.info()
    fp = _suite()
    node = _bbr_node(run)
    args = fp.fmriprep_args(run, node)
    ref_path, wm_path = Path(_arg(args, "-ref")), Path(_arg(args, "-wmseg"))
    out_name, mat_name = _arg(args, "-out"), _arg(args, "-omat")
    ref = nib.load(str(ref_path))
    brain = _brain(ref_path)
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(ref, show)
    ex = exm.Example(ctx, grid, show, view=view)
    data = _data(run)
    ex.add(exm.Panel("t1w", "input", f"{run.split('-noreconall')[0]} T1w (the reference)",
                     "fMRIPrep's preprocessed T1w × its brain mask (-ref)",
                     {"all": common.load(str(ref_path))}))  # fmt: skip
    arrays = {
        "init": {"registered": common.load(str(e.dir / "init" / "init.nii.gz"))},
        **{pid: {"registered": common.load(str(e.dir / pid / out_name))} for pid, *_ in PANELS},
        **{
            pid: {"registered": common.load(str(o.dir / pid / out_name))} for pid, *_ in ORIG_PANELS
        },
    }
    wm = common.load(str(wm_path))
    w_init = {"window": None}
    ex.add(exm.Panel("init", "original", "Initial alignment (-init)",
                     "the BOLD reference resampled at fMRIPrep's mri_coreg matrix by FSL "
                     "6.0.7.23's -applyxfm (trilinear)", arrays["init"]))  # fmt: skip
    for pid, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    lo_hi = _window(ex, "fsl")
    w_init["window"] = _window(ex, "init")
    ex.add(exm.Panel("wm-init", "input", "White matter over the initial alignment",
                     "fMRIPrep's white-matter mask (-wmseg) drawn over the BOLD reference at "
                     "-init: the boundary BBR fits", {"all": wm},
                     overlay={"registered": {"on": "init", "window": w_init["window"]}}))  # fmt: skip
    ex.add(exm.Panel("wm-fsl", "input", "White matter over FSL's result",
                     "the same mask drawn over FSL 6.0.7.23's registered BOLD reference",
                     {"all": wm}, overlay={"registered": {"on": "fsl", "window": lo_hi}}))  # fmt: skip
    for pid, role, label, sub in ORIG_PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    diffs = _diffs(True) + _orig_diffs()
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    mats = {pid: e.dir / pid / mat_name for pid, *_ in PANELS}
    mats.update({pid: o.dir / pid / mat_name for pid, *_ in ORIG_PANELS})
    mats["fsl-perturbed"] = o.dir / "fsl-perturbed" / mat_name
    own = next(node.glob("*.mat"))
    mats_same = mats["replica"].read_bytes() == mats["fsl"].read_bytes()
    _verdicts(diffs, same, "this call", mats_same)
    m = {k: np.loadtxt(p) for k, p in {**mats, "own": own}.items()}
    m["init"] = np.loadtxt(Path(_arg(args, "-init")))
    o_shift = displacement(m["orig"], m["fsl"], ref, brain)
    o_band = displacement(m["fsl-perturbed"], m["fsl"], ref, brain)
    _orig_verdicts(diffs, same, o, o_shift, o_band, 0.1, float(arrays["fsl"]["registered"].flat[0]),
                   displacement(m["orig-fix"], m["orig"], ref, brain))  # fmt: skip
    if view == "slices":
        at = common.slice_voxel(ex, "fsl", "init", "registered")
    else:
        at, _k = common.largest_in_slice(ex, "fsl", "init", "registered", "axial")
    default = {
        "output": "registered",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1w", "wm-init", "wm-fsl", "fsl", "replica", "orig"],
        "diffs": ["replica-fsl", "orig-fsl", "origfix-orig", "fsl-init"],
        "compare": {"a": "fsl", "b": "init"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(OUTPUTS), default=default, window_panel="fsl",
        caption="the crosshair where FSL's result differs most from its initial alignment"
        + ("; the three slices through it are stored at full resolution." if view == "slices"
           else "."),
    )  # fmt: skip
    labels = {
        "fsl": "FSL 6.0.7.23",
        "replica": "replica (reproduce)",
        "replica-fix": "replica (default)",
        "orig": "clean-room original (reproduce)",
        "orig-fix": "clean-room original (default)",
        "own": "fMRIPrep's own (its image's FSL)",
        "init": "the initial matrix (-init)",
    }
    mat_files = {k: mats[k] for k in ("fsl", "replica", "replica-fix", "orig", "orig-fix")}
    mat_files["own"] = own
    mat_files["init"] = Path(_arg(args, "-init"))
    tables = [
        _matrix_table(
            f"{ex_id}-matrices",
            mat_files,
            labels,
            "Each matrix file as written (`-omat`), its numbers as text; fMRIPrep's own is the "
            "node's output in the run's working directory (FSL in fMRIPrep 25.2.6's image), "
            "the initial matrix the call's `-init` (mri_coreg's, converted by lta_to_fsl). "
            "*identical*: the same bytes as FSL 6.0.7.23's file. The clean-room original's "
            "matrices are its own result, written as FSL writes them.",
        ),
        _shift_table(
            f"{ex_id}-shift",
            [
                ("replica − FSL 6.0.7.23", m["replica"], m["fsl"]),
                ("replica (default) − FSL 6.0.7.23", m["replica-fix"], m["fsl"]),
                ("clean-room original − FSL 6.0.7.23", m["orig"], m["fsl"]),
                ("clean-room original (default) − FSL 6.0.7.23", m["orig-fix"], m["fsl"]),
                (
                    "FSL 6.0.7.23 on a perturbed input − FSL 6.0.7.23 (its own variability)",
                    m["fsl-perturbed"],
                    m["fsl"],
                ),
                ("fMRIPrep's own − FSL 6.0.7.23", m["own"], m["fsl"]),
                ("FSL 6.0.7.23 − the initial matrix (what BBR changed)", m["fsl"], m["init"]),
            ],
            ref,
            brain,
        ),
    ]
    vers = info.get("versions", {})
    served = info.get("oracle_cache", {})
    n_vox = int(np.prod(ref.shape[:3]))
    shift = displacement(m["fsl"], m["init"], ref, brain)
    title = {
        "fmriprep-bbr": "fMRIPrep's boundary-based registration of a BOLD reference",
        "panel-ds003592-older": "Data panel: an 82-year-old's BOLD reference",
        "panel-ds000228-child": "Data panel: a three-year-old's BOLD reference",
    }[ex_id]
    return {
        "id": ex_id,
        "title": title,
        "summary": (
            f"The BBR node fMRIPrep 25.2.6 ran on {words}: `flirt -in <boldref> -ref "
            "<T1w brain> -wmseg <white matter> -init <mri_coreg> -schedule bbr.sch -dof 6 "
            "-searchcost bbr -basescale 1`, given to FSL 6.0.7.23 and to the larmorx-nc replica "
            "in its reproduce and default modes. The white-matter mask is drawn over the BOLD "
            "reference at the initial alignment and over FSL's result: the boundary the cost "
            "aligns the BOLD contrast to. The matrices follow as text, with fMRIPrep's own, and "
            "the displacement each pair gives over the brain. LARMORX's clean-room original "
            "(Apache-2.0, written from the specification without FSL's source) runs the same "
            "call in its reproduce and default modes and at one thread; FSL runs it once more on "
            "the BOLD reference perturbed by noise of half an intensity unit, which measures "
            "FSL's own variability, the yardstick of a registration that is not bit-identical."
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
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): the BOLD "
                "reference and the T1w fMRIPrep computed; shown inside the T1w's brain mask, "
                "dilated by 2 voxels",
            },
            {"label": "Node", "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}`"},
            {"label": "Command", "text": "`" + _short_bbr(info["command"]) + "`"},
            {
                "label": "Programs",
                "text": f"{vers.get('fsl', '?')} ("
                + (
                    "its run served from the oracle cache, as the record ran it"
                    if served.get("fsl") == "served"
                    else "run for this page"
                )
                + f"); {vers.get('larmorx-nc', '?')} (the replica), from larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`; "
                + oinfo.get("versions", {}).get("larmorx", "the clean-room original")
                + f", from larmorx at commit `{oinfo['larmorx_commit'][:12]}`",
            },
            {
                "label": "Matrix",
                "text": "the replica's file "
                + ("byte for byte FSL's" if mats_same else "differs from FSL's")
                + "; fMRIPrep's own "
                + ("the same bytes" if own.read_bytes() == mats["fsl"].read_bytes() else "differs")
                + f"; BBR moved the brain by {shift['mean']:.2f} mm on average from the "
                f"initial alignment (at most {shift['max']:.2f} mm)",
            },
            {
                "label": "Original",
                "text": f"the clean-room original's matrix {o_shift['p95']:.3g} mm from FSL's at "
                f"the 95th percentile of the brain (at most {o_shift['max']:.3g} mm); FSL's own "
                f"on a perturbed input {o_band['p95']:.3g} mm (at most {o_band['max']:.3g} mm); "
                "its runs at 1 thread and in its default mode "
                + (
                    "give the same bytes"
                    if same["orig1-orig"] and same["origfix-orig"]
                    else "differ (the comparisons below)"
                ),
            },
            {"label": "Grid", "text": common.describe_grid(ref, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: threads, modes and its two implementations"],
            ["reference", "What the registration changed"],
        ],
        "tables": tables,
        "metrics_note": _note(n_vox),
        "provenance": [
            *common.provenance(info, f"{TOOL}/{e.case}", [data]),
            *common.provenance(oinfo, f"{TOOL}/{o.case}", [data])[1:],
        ],
    }


def _window(ex, pid: str) -> list[float]:
    from larmorx_validation.evidence import example as exm

    v = ex.values(ex.key(pid, "registered"))
    m = ex.m if v.ndim == 3 else np.broadcast_to(ex.m[..., None], v.shape)
    w = exm.windows(v[m])[0]
    return [w["lo"], w["hi"]]


def _short_bbr(command: str) -> str:
    """fMRIPrep's command with its working directory's paths cut to file names."""
    words = command.split()
    return " ".join(Path(w).name if w.startswith("/") else w for w in words)


def _example_real(ctx: core.Context, ex_id: str) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    case = REAL[ex_id]
    e = real_entry(ctx, case)
    o = orig_real_entry(ctx, case)
    info, oinfo = e.info(), o.info()
    args, files = _real_args(case)
    ref_path = files[_arg(args, "-ref").lstrip("@")]
    ref = nib.load(str(ref_path))
    brain = _brain(ref_path)
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(ref, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("t1w", "input", "ds005040 sub-002 T1w (the reference)",
                     "fMRIPrep's T1w × its brain mask, every second voxel (1.7 mm; -ref)",
                     {"all": common.load(str(ref_path))}))  # fmt: skip
    arrays = {
        pid: {"registered": common.load(str(e.dir / pid / "out.nii.gz"))} for pid, *_ in PANELS
    }
    arrays.update(
        {
            pid: {"registered": common.load(str(o.dir / pid / "out.nii.gz"))}
            for pid, *_ in ORIG_PANELS
        }
    )
    for pid, role, label, sub in PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    four_d = arrays["fsl"]["registered"].ndim == 4
    if not four_d:
        wm = common.load(str(files.get("wm.nii", _suite().inputs() / "wm.nii")))
        ex.add(exm.Panel("wm-fsl", "input", "White matter over FSL's result",
                         "fMRIPrep's white-matter mask (every second voxel) drawn over FSL "
                         "6.0.7.23's registered BOLD reference: a check of the fit",
                         {"all": wm},
                         overlay={"registered": {"on": "fsl", "window": _window(ex, "fsl")}}))  # fmt: skip
    for pid, role, label, sub in ORIG_PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    diffs = _diffs(False) + _orig_diffs()
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    tables = []
    mats_same = True
    o_shift = o_band = fix_shift = None
    if "-omat" in args:
        mats = {pid: e.dir / pid / _arg(args, "-omat") for pid, *_ in PANELS}
        mats.update({pid: o.dir / pid / _arg(args, "-omat") for pid, *_ in ORIG_PANELS})
        mats["fsl-perturbed"] = o.dir / "fsl-perturbed" / _arg(args, "-omat")
        mats_same = mats["replica"].read_bytes() == mats["fsl"].read_bytes()
        m = {k: np.loadtxt(p) for k, p in mats.items()}
        o_shift = displacement(m["orig"], m["fsl"], ref, brain)
        o_band = displacement(m["fsl-perturbed"], m["fsl"], ref, brain)
        fix_shift = displacement(m["orig-fix"], m["orig"], ref, brain)
        tables = [
            _matrix_table(
                f"{ex_id}-matrices",
                {k: mats[k] for k in ("fsl", "replica", "replica-fix", "orig", "orig-fix")},
                {
                    "fsl": "FSL 6.0.7.23",
                    "replica": "replica (reproduce)",
                    "replica-fix": "replica (default)",
                    "orig": "clean-room original (reproduce)",
                    "orig-fix": "clean-room original (default)",
                },
                "Each matrix file as written (`-omat`), its numbers as text. *identical*: the "
                "same bytes as FSL 6.0.7.23's file. The clean-room original's matrices are its "
                "own result, written as FSL writes them.",
            ),
            _shift_table(
                f"{ex_id}-shift",
                [
                    ("replica − FSL 6.0.7.23", m["replica"], m["fsl"]),
                    ("replica (default) − FSL 6.0.7.23", m["replica-fix"], m["fsl"]),
                    ("clean-room original − FSL 6.0.7.23", m["orig"], m["fsl"]),
                    ("clean-room original (default) − FSL 6.0.7.23", m["orig-fix"], m["fsl"]),
                    (
                        "FSL 6.0.7.23 on a perturbed input − FSL 6.0.7.23 (its own variability)",
                        m["fsl-perturbed"],
                        m["fsl"],
                    ),
                ],
                ref,
                brain,
            ),
        ]
    _verdicts(diffs, same, "this call", mats_same)
    _orig_verdicts(
        diffs,
        same,
        o,
        o_shift,
        o_band,
        _flirt_threshold(args),
        float(arrays["fsl"]["registered"].flat[0]),
        fix_shift,
    )
    at, _k = common.centre_view(ex)
    default = {
        "output": "registered",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1w", *(["wm-fsl"] if not four_d else []), "fsl", "replica", "orig"],
        "diffs": ["replica-fsl", "fix-replica", "orig-fsl", "origfix-orig"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(OUTPUTS), default=default, window_panel="fsl",
        caption="the crosshair at the centre of the brain.",
    )  # fmt: skip
    vers = info.get("versions", {})
    served = info.get("oracle_cache", {})
    n_vox = int(np.prod(arrays["fsl"]["registered"].shape))
    texts = {
        "dof12": (
            "A default registration: 12 degrees of freedom, the correlation ratio",
            "The record's case `real/default`: FLIRT's default (12 degrees of freedom, the "
            "correlation ratio, its search over rotations and its multi-resolution schedule) "
            "registering ds005040's BOLD reference to the subject's T1w brain at half "
            "resolution, by FSL 6.0.7.23 and the replica in its reproduce and default modes; "
            "the white-matter mask drawn over FSL's result shows the fit. LARMORX's clean-room "
            "original runs the same call in both modes and at one thread, and FSL once more on "
            "the BOLD reference perturbed by noise of half an intensity unit (its own "
            "variability).",
        ),
        "applyxfm-4d": (
            "-applyxfm on a raw BOLD series, sinc interpolation",
            "The record's case `real/applyxfm-4d`: four volumes of ds005040's raw BOLD series "
            "resampled onto the T1w's grid (every second voxel) through fMRIPrep's initial "
            "matrix, with `-applyxfm -interp sinc`, by FSL 6.0.7.23 and the replica in its "
            "reproduce and default modes; choose the volume below the panels. LARMORX's "
            "clean-room original applies the same matrix in both modes and at one thread.",
        ),
    }
    title, summary = texts[ex_id]
    return {
        "id": ex_id,
        "title": title,
        "summary": summary,
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS005040['dataset']}, {common.DS005040['subject']} "
                f"({common.DS005040['licence']}): the record's inputs derived from fMRIPrep's "
                "run (oracles/inputs/fsl-flirt-replica/); shown inside the T1w's brain mask, "
                "dilated by 2 voxels",
            },
            {"label": "Case", "text": f"`{case}` of the record (`fsl-flirt-replica`)"},
            {"label": "Command", "text": "`" + info["command"] + "`"},
            {
                "label": "Programs",
                "text": f"{vers.get('fsl', '?')} ("
                + (
                    "its run served from the oracle cache, as the record ran it"
                    if served.get("fsl") == "served"
                    else "run for this page"
                )
                + f"); {vers.get('larmorx-nc', '?')} (the replica), from larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`; "
                + oinfo.get("versions", {}).get("larmorx", "the clean-room original")
                + f", from larmorx at commit `{oinfo['larmorx_commit'][:12]}`",
            },
            *(
                [
                    {
                        "label": "Original",
                        "text": f"the clean-room original's matrix {o_shift['p95']:.3g} mm from "
                        f"FSL's at the 95th percentile of the brain (at most "
                        f"{o_shift['max']:.3g} mm); FSL's own on a perturbed input "
                        f"{o_band['p95']:.3g} mm (at most {o_band['max']:.3g} mm)",
                    }
                ]
                if o_shift is not None and o_band is not None
                else []
            ),
            {"label": "Grid", "text": common.describe_grid(ref, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: threads, modes and its two implementations"],
        ],
        "tables": tables,
        "metrics_note": _note(n_vox),
        "provenance": [
            *common.provenance(info, f"{TOOL}/{e.case}", [common.DS005040]),
            *common.provenance(oinfo, f"{TOOL}/{o.case}", [common.DS005040])[1:],
        ],
    }


# ------------------------------------------------------------------------------------ cases


def _input(a: str) -> str:
    name = a[1:]
    if a.startswith("@dt:"):
        return f"<random testing's input {name.split(':', 1)[1]}>"
    if a.startswith("@fslsch:"):
        return f"$FSLDIR/etc/flirtsch/{a.removeprefix('@fslsch:')}"
    return f"<{name}: derived from ds005040's fMRIPrep run>"


def _direct() -> dict:
    """The clean-room original's own cases (``parity.fsl_flirt._original_cases``): name -> run."""
    from larmorx_validation.parity import fsl_flirt as fo

    return {name: run for name, _desc, run in fo._original_cases()}


def _run_of(case: str):
    defs = _suite()._all_cases()
    if case in defs:
        return defs[case][2]
    return _direct().get(case)


def command(case: str) -> str:
    if case.startswith("fmriprep/"):
        run = case.split("/")[1]
        node = _bbr_node(run)
        return _short_bbr((node / "command.txt").read_text(encoding="utf-8"))
    run = _run_of(case)
    if run is None:
        return ""
    words = [_input(a) if a.startswith("@") else a for a in run.args]
    parts = [run.program, *(shlex.quote(w) if re.search(r"\s", w) or w == "" else w
                            for w in words)]  # fmt: skip
    env = " ".join(f"{k}={v}" for k, v in run.settings if v is not None)
    files = " ".join(f"(with {n} written first)" for n, _ in run.files)
    text = " ".join(x for x in (env, " ".join(parts), files) if x)
    # FSL's installation as $FSLDIR, the home directory as ~: no local paths on the site
    return text.replace(str(_suite().fsldir()), "$FSLDIR").replace(str(Path.home()), "~")


#: The register entries the replica corrects by default in FLIRT's runs.
CORRECTED = frozenset({"FSL-09", "FSL-14", "FSL-68", "FSL-111"})


def case_verdict(r: dict, desc: str) -> tuple[str, str]:
    v = common.outcome_verdict(r)
    if v is not None:
        return v
    rep = {**r.get("reported", {}), **r.get("measured", {})}
    notes = ["every file FSL writes, byte for byte, with FSL's exit status and messages"]
    own = rep.get("fMRIPrep's matrix (its container's FSL) identical as text")
    if own == "yes":
        notes.append("fMRIPrep's own matrix (its image's FSL) the same as text")
    fix = rep.get("D13 fix pass")
    named = [d for d in re.findall(r"\b(FSL-\d+)\b", desc) if d in CORRECTED]
    if named:
        notes.append(
            f"in its default mode the replica corrects {', '.join(named)} and names it"
            + (f": {fix:,} values change" if isinstance(fix, int) and fix else "")
        )
    return "bit-identical", common.clip("; ".join(notes), 600)


def _zero(x) -> bool:
    """A measured value that says "no difference": "yes" (the same text) or a difference of 0."""
    return x == "yes" or (isinstance(x, (int, float)) and not isinstance(x, bool) and x == 0)


#: Failing cases of the original's record that came from the replica's suite after the
#: original's last part, and what follows for them.
FOLLOW_UPS = {
    "programs/pointflirt-no-points": "pointflirt given two coordinate files that do not exist "
    "(no points), where FSL crashes (FSL-204, registered on 2026-11-03): since 2026-11-05 the "
    "clean-room original refuses the call in both modes, naming FSL-204 (outside the record "
    "before).",
}


def case_verdict_orig(r: dict, desc: str, band: dict | None = None) -> tuple[str, str]:
    """A case's verdict for the clean-room original: its record holds it to specs/flirt.md §13's
    thresholds (exit status and messages FSL's; files identical, matrices within a displacement,
    images from its own matrix by their correlation). Bit-identical where every value it
    measures is FSL's; within FSL's own variability where its matrix is beyond the fixed
    threshold but within FSL's on a perturbed input (the record's band), or within the band
    measured for a worked example (``band``); open otherwise."""
    from larmorx_validation.parity import fsl_flirt as fo

    v = common.outcome_verdict(r)
    if v is not None and r["case"] in FOLLOW_UPS:
        return v[0], common.clip(FOLLOW_UPS[r["case"]] + " " + v[1], 600)
    if v is not None:
        return v
    rep_ = r.get("reported", {})
    named = sorted(set(re.findall(r"\b(FSL-\d+)\b", desc)))
    fix = rep_.get("D13 fix pass")
    note = (
        " I" + common.fix_note(fix, named, GLIBC_FUNCTIONS)[1:] + "."
        if isinstance(fix, int) and fix
        else ""
    )
    differ = {
        k: x for k, x in r.get("measured", {}).items() if not (_zero(x) and "correlation" not in k)
    }
    if not differ:
        return "bit-identical", common.clip(
            "Every file FSL writes, byte for byte (images: every value and header field, "
            "`descrip` aside), with FSL's exit status and messages." + note,
            600,
        )
    parts = "; ".join(f"{k}: {x}" for k, x in list(differ.items())[:4])
    disp = [x for k, x in differ.items() if "displacement" in k and isinstance(x, (int, float))]
    run = _run_of(r["case"])
    args = ["-cost", "bbr"] if r["case"].startswith("fmriprep/") else list(run.args if run else [])
    thr = fo._threshold([a.lstrip("@") for a in args])
    if disp and max(disp) > thr:
        return "within-variability", common.clip(
            f"The matrix beyond the record's fixed threshold ({thr:g} mm) but within FSL's own "
            "variability, which the record measured on the same call with its input perturbed "
            f"by noise of half an intensity unit (a poorly conditioned registration): {parts}."
            + note,
            600,
        )
    # every displacement the record measured, a zero one included (a matrix within the record's
    # printed precision of FSL's, where only the printed digits differ)
    measured = [
        x
        for k, x in r.get("measured", {}).items()
        if "displacement" in k and isinstance(x, (int, float))
    ]
    if measured and band is not None and max(measured) <= band["p95"]:
        at = (
            "" if disp else f"; the matrix {max(measured):g} mm from FSL's (the record's precision)"
        )
        return "within-variability", common.clip(
            f"{parts}{at}"
            + f"; within FSL's own variability on this call, measured for the worked "
            f"example: {band['p95']:.3g} mm at the 95th percentile (its input perturbed by "
            "noise of half an intensity unit)." + note,
            600,
        )
    why = f"Within the record's thresholds, not bit-identical: {parts}."
    if disp:
        why += (
            " A clean-room registration: the cost's last bits and the optimiser's path "
            "(docs/findings/fsl-flirt-clean-room.md); "
            + (
                f"FSL's own variability on this call {band['p95']:.3g} mm."
                if band is not None
                else "FSL's own variability is not measured for this case."
            )
        )
    elif any("sinc" in k or "spline" in k or "1e-5" in k for k in differ):
        why += (
            " From the same matrix: the sinc and spline kernels' last float bits (an open "
            "question of the specification)."
        )
    elif any(k.startswith(("stdout", "stderr")) for k in differ):
        why += " Printed numbers differ in their last digits."
    # 610: the findings' new name (-clean-room.md) is two characters longer than the old one;
    # a reason cut short keeps every character it showed
    return "open", common.clip(why + note, 610)


def cases(
    rec,
    examples: dict[str, str],
    impl: str = "replica",
    into: list | None = None,
    bands: dict[str, dict] | None = None,
) -> list[dict]:
    """The record's cases as the case explorer's, its results as ``impl``'s; ``into``: the
    cases of another record, to which this one's results join (a case of its own where that
    record has none)."""
    fp = _suite()
    if impl == "original":
        from larmorx_validation.parity import fsl_flirt as fo

        descs = {c.id: c.description for c in fo.cases("standard")}
    else:
        descs = {c.id: c.description for c in fp.cases("full")}
    out = [] if into is None else into
    by_id = {c["id"]: c for c in out}
    for r in rec.results:
        desc = descs.get(r["case"], "")
        if r["case"].startswith("fmriprep/"):
            desc = f"fMRIPrep's BBR call ({r['case'].split('/')[1].split('-noreconall')[0]})"
        if impl == "original":
            v, why = case_verdict_orig(r, desc, (bands or {}).get(r["case"]))
        else:
            v, why = case_verdict(r, desc)
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
            "command": command(r["case"]),
            "results": {impl: result},
            **({"example": examples[r["case"]]} if r["case"] in examples else {}),
        }
        out.append(c)
        by_id[c["id"]] = c
    return out


def _bands(ctx: core.Context, examples: dict[str, str]) -> dict[str, dict]:
    """FSL's own variability measured for the worked examples (the record's case -> the
    displacement of FSL's matrix on a perturbed input from its own, over the brain)."""
    import nibabel as nib

    fp = _suite()
    out = {}
    for _i, run, _v, _w in BBR:
        node = _bbr_node(run)
        args = fp.fmriprep_args(run, node)
        e, o = bbr_entry(ctx, run), orig_bbr_entry(ctx, run)
        mat, ref_path = _arg(args, "-omat"), Path(_arg(args, "-ref"))
        cid = next(c for c, x in examples.items() if x == _i)
        out[cid] = displacement(
            np.loadtxt(o.dir / "fsl-perturbed" / mat),
            np.loadtxt(e.dir / "fsl" / mat),
            nib.load(str(ref_path)),
            _brain(ref_path),
        )
    for case in REAL.values():
        args, files = _real_args(case)
        if "-omat" not in args:
            continue
        e, o = real_entry(ctx, case), orig_real_entry(ctx, case)
        mat, ref_path = _arg(args, "-omat"), files[_arg(args, "-ref").lstrip("@")]
        out[case] = displacement(
            np.loadtxt(o.dir / "fsl-perturbed" / mat),
            np.loadtxt(e.dir / "fsl" / mat),
            nib.load(str(ref_path)),
            _brain(ref_path),
        )
    return out


#: The benchmark's series (``docs/benchmarks/fsl-flirt.json``, from
#: ``validation/investigations/cleanroom-flirt/bench.py``): FSL and the replica on one thread.
SPEED_THREADS = 14
SPEED_SERIES = [
    {"id": "FSL 6.0.7.23|1 thread", "label": "FSL 6.0.7.23 (single-threaded)", "mark": "original"},
    {"id": "replica (larmorx-nc)|1 thread", "label": "replica, 1 thread", "mark": "replica"},
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
        title="Seconds for the whole command (read, register, write), one run each, log scale; "
        f"the factor is the clean-room original's speed-up over FSL at {SPEED_THREADS} threads",
        base=(SPEED_SERIES[0]["id"], SPEED_SERIES[3]["id"]),
        note="From the benchmark report `docs/benchmarks/fsl-flirt.json` ({cpu}, {cpus} logical "
        "CPUs, shared with another agent's runs, so the times may be a little long; each program "
        "in FSL's clean environment, outputs NIfTI-1 gzipped; informal: one run each, larmorx at "
        "commit 04f5462, part 2's third step, before the specification's answers changed the "
        "arithmetic: not timed again).",
    )
    return bench, chart


#: The clean-room catch-up's trial of the original (2026-11-05), and its findings in categories
#: (``validation/investigations/cleanroom-flirt/trial_catchup_categories.py``'s ``--json`` output).
TRIAL_CATCHUP = "2026-11-05-catchup-flirt"
CATCHUP_TRIAGE = "validation/investigations/cleanroom-flirt/trial_catchup_triage.json"
#: The catch-up trial's categories: category -> (what the page says, the verdict of the row, or
#: None for a mistake fixed after the trial).
CATCHUP_TEXT: dict[str, tuple[str, str | None]] = {
    "fixed: the search's starting points of equal cost (std::sort)": (
        "Fixed after the trial (relay 10), with permanent cases: FLIRT's search kept its "
        "starting points of equal cost in the order it found them, where FSL sorts the list by "
        "cost with libstdc++'s std::sort, which is not stable, before it enters S and P. On these "
        "random pairs many costs tie (an input or a pair that carries no information at the "
        "coarse scales: every cost 1), so another starting point came first and the registration "
        "ended elsewhere, or its messages followed another path. Each of these agrees on replay "
        "with the final build and differed with the build before the fix (catchup_replay.py; "
        "trial_catchup_replay_sorted.txt, trial_catchup_replay_before_sort.txt): 755 "
        "registrations, 101 pairreg runs and 17 message counts",
        None,
    ),
    "a registration ends elsewhere": (
        "A registration ends elsewhere than FSL's (its matrix beyond the record's threshold, or "
        "only the image written from it) on the trial's random, often ill-posed pairs: small "
        "synthetic heads and blobs, odd orientations and voxel sizes, where FSL's own "
        "variability is often 0. The costs follow FSL's arithmetic within their last bits and "
        "the optimiser's path then parts; left for FLIRT's next clean-room part and the final "
        "validation phase",
        "open",
    ),
    "messages whose number follows the path": (
        "Messages whose number follows the optimiser's path, printed a different number of "
        "times: mcflirt-style COG warnings, FSL-90's 'Parameters do not form a valid axis', "
        "'Erroneous dof', Woods' warning, -verbose's lines",
        "open",
    ),
    "pairreg: its registrations' paths": (
        "pairreg's three registrations: their paths part as a registration's do",
        "open",
    ),
    "pairreg: FSL corrupts its heap (FSL-75)": (
        "pairreg where FSL itself corrupts its heap (Segmentation fault, free(): invalid size or "
        "pointer, double free: FSL-75, undefined behaviour never reproduced) and the clean-room "
        "original registers and finishes",
        "divergence",
    ),
    "fixed: -out's header, a completed sform's negative zeros": (
        "Fixed after the trial, with a permanent case: -out's header where a reference with a "
        "qform only and qfac −1 has its sform completed from the qform, whose zeros times −1 are "
        "negative zeros that FSL writes (the original wrote zeros); each agrees on replay with the "
        "final build",
        None,
    ),
    "fixed: the overall minimum on FSL's Cost minima line alone": (
        "Fixed after the trial, with a permanent case: a verbose search on a one-point grid "
        "(-nosearch) finds no local minimum, so FSL lists the overall minimum only on its Cost "
        "minima line, without a COST line; each agrees on replay with the final build",
        None,
    ),
    "a test volume without intensity range: FSL's sub-threshold costs": (
        "A test volume with no intensity range at a scale (Testvol intensity clamped between 0 "
        "and 0): FSL prints 'WARNING: Found 0 or less sub-threshold costs' and makes every grid "
        "point a minimum, where the original evaluates the fine grid's costs (corratio's cost of "
        "no variance, 1): a question for the specification (specs/flirt.md §8.1 steps 6 and 7)",
        "open",
    ),
    "a resampled image's last bits (sinc, spline)": (
        "-applyxfm and -applyisoxfm images from the same matrix: the windowed sinc's and the "
        "nearest neighbour's last bits on small images",
        "open",
    ),
    "aff2rigid's corrected matrix": (
        "aff2rigid's corrected matrix (FSL-81) against the replica's: not yet looked at",
        "open",
    ),
    "fixed: applyxfm4D's int32 padding": (
        "Fixed after the trial, with a permanent case: applyxfm4D of an int32 image of "
        "2147483647, whose padding FSL writes exactly in the type (and its interpolated voxels "
        "through single precision: FSL-09); it agrees on replay with the final build",
        None,
    ),
}


def trial_catchup(ctx: core.Context) -> tuple[list[dict], dict]:
    """The clean-room catch-up's random-testing trial of the original (its ``summary.json``)
    and its findings in categories (``trial_catchup_triage.json``): two tables, and their
    provenance."""
    path = common.oracles() / "difftest" / TRIAL_CATCHUP / "summary.json"
    raw = path.read_bytes()
    s = json.loads(raw)
    t = s["tools"]["fsl-flirt-original"]
    o = t["options"]
    impl = t["implementations"]["original"]
    others = ("aff2rigid", "alignpaxes", "avscale", "convert_xfm", "extracttxt", "img2imgcoord",
              "img2stdcoord", "makerot", "midtrans", "pointflirt", "rmsdiff", "std2imgcoord")  # fmt: skip
    costs = ("corratio", "mutualinfo", "normmi", "normcorr", "leastsq", "woods", "labeldiff", "bbr")
    notices = ", ".join(f"{k} {v['cases']}" for k, v in sorted(t.get("notices", {}).items()))
    rows = [
        ("Random cases, each compared with FSL 6.0.7.23", t["cases"]),
        ("within the record's thresholds (§13), every exact check passed", impl.get("agree", 0)),
        ("differing (each one in a category below)", impl.get("differ", 0)),
        ("a register entry named in one mode only (a notice)", impl.get("notice", 0)),
        ("flirt, applyxfm4D, pairreg", [o.get(k, 0) for k in ("flirt", "applyxfm4D", "pairreg")]),
        ("the other programs (convert_xfm, avscale, midtrans, ...)", sum(o.get(k, 0) for k in others)),
        ("each cost (" + ", ".join(costs) + ")", [o.get(f"-cost {c}", 0) for c in costs]),
        ("3, 6, 7, 9, 12 dof", [o.get(f"-dof {n}", 0) for n in (3, 6, 7, 9, 12)]),
        ("-applyxfm, -applyisoxfm", [o.get("-applyxfm", 0), o.get("-applyisoxfm", 0)]),
        ("-debug, -verbose", [o.get("-debug", 0), o.get("-verbose", 0)
                               + sum(o.get(f"-verbose {n}", 0) for n in (2, 3, 4, 5, 7))]),
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
        f"Random testing of the clean-room original after the catch-up: the trial of {s['date']}",
        ["", {"label": "Cases", "num": True}],
        out_rows,
        note=f"Differential random testing's trial of the clean-room original after the "
        f"specification's answers of 2026-11-03 (seed `{s['seed']}`: the 2026-11-02 trials' cases "
        "drawn again and further, FSL's runs served by the oracle cache where they had run), each "
        "compared as the record compares (specs/flirt.md §13: exit status and messages FSL's, "
        "files identical, matrices within a displacement over the reference's brain, images from "
        f"the original's own matrix by their correlation). The build: larmorx `{commit}`; four "
        f"classes were fixed after it, from its findings. The notices: {notices} cases. Every "
        f"case in a memory box of {s['memory']['case']} (peaks below {peaks:.2f} GiB).",
    )
    tpath = ctx.repo / CATCHUP_TRIAGE
    traw = tpath.read_bytes()
    tri = json.loads(traw)
    counts = tri["counts"]
    if sum(counts.values()) != impl.get("differ", 0):
        raise ValueError(
            f"{CATCHUP_TRIAGE}: {sum(counts.values())} findings, the trial {impl['differ']}"
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
        "The trial's differing cases, each in one category",
        ["Category", {"label": "Cases", "num": True}, "Verdict", "Reproducer"],
        trows,
        note="Every differing case of the trial in one category by its program, its failed "
        "checks and their details (`validation/investigations/cleanroom-flirt/"
        f"trial_catchup_categories.py`, its output `{CATCHUP_TRIAGE}`), each category's cause "
        "established by replaying findings (`catchup_replay.py`, with the final build and with "
        "the build before the search's sort) and by probes (`catchup_finding.py`, "
        "`catchup8_int_nn.py`; `docs/findings/fsl-flirt-clean-room.md`); the triage's entries: "
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


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, RECORD)
    rec_orig = records.load(ctx.repo, ORIG_RECORD)
    examples = {
        **{
            f"fmriprep/{run}/{_bbr_node(run).parent.parent.parent.parent.name}": ex_id
            for ex_id, run, _v, _w in BBR
        },
        **{case: ex_id for ex_id, case in REAL.items()},
    }
    cs = cases(rec, examples)
    cases(rec_orig, examples, impl="original", into=cs, bands=_bands(ctx, examples))
    entries = [bbr_entry(ctx, run) for _i, run, _v, _w in BBR]
    entries += [real_entry(ctx, case) for case in REAL.values()]
    entries += [orig_bbr_entry(ctx, run) for _i, run, _v, _w in BBR]
    entries += [orig_real_entry(ctx, case) for case in REAL.values()]
    tc, tcprov = trial_catchup(ctx)
    bench, chart = speed(ctx)
    status = "experimental"
    doc = {
        "schema": 1,
        "tool": TOOL,
        "title": "FLIRT",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source() + rec_orig.source(),
            "benchmarks": bench.source() if bench else [],
            "cache": [fcache.provenance(x) for x in entries],
            "difftest_catchup": tcprov,
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cs,
                "replica",
                "larmorx-nc replica, against FSL 6.0.7.23 (standard tier)",
                "replica",
                f"docs/validation/{RECORD}.md",
                status,
            ),
            common.implementation(
                cs,
                "original",
                "larmorx clean-room original, against FSL 6.0.7.23 (standard tier)",
                "original",
                f"docs/validation/{ORIG_RECORD}.md",
                "experimental" if rec_orig.data.get("summary", {}).get("fail") else "validated",
            ),
        ],
        "cases": cs,
        "cases_total": len(cs),
        "speed": chart,
        "tables": tc,
        "examples": [
            _example_bbr(ctx, ex_id, run, view, words) for ex_id, run, view, words in BBR[:1]
        ]
        + [_example_real(ctx, "dof12"), _example_real(ctx, "applyxfm-4d")]
        + [_example_bbr(ctx, ex_id, run, view, words) for ex_id, run, view, words in BBR[1:]],
    }
    return present(doc)


# ------------------------------------------------------------------- the writing guide's words

#: FLIRT's own rewordings, before the FSL pages' (:mod:`fsl_pages`): "the trial" of random
#: testing is a random-testing run (the guide's section 1), "relay 10" the catch-up's step 10.
TEXTS = [
    (
        r"\bFixed after the trial \(relay 10\)",
        "Fixed after the run (the clean-room catch-up, its step 10)",
    ),
    (r"\bFixed after the trial\b", "Fixed after the run"),
    (r"\bthe trial's random\b", "the run's random"),
    (r"\bthe trial's cases\b", "the run's cases"),
    (r"\bthe 2026-11-02 trials' cases\b", "the cases of the random-testing runs of 2026-11-02"),
    (r"\bDifferential random testing's trial of the clean-room original\b",
     "A random-testing run of the clean room"),
    (r"\bEvery differing case of the trial\b", "Every differing case of the run"),
    (r"\bThe reproducer is a finding of the trial\b", "The reproducer is a finding of the run"),
    # the benchmark's machine, shared: with other runs (an agent is the project's own word)
    (r"\bshared with another agent's runs\b", "shared with other runs"),
]  # fmt: skip
#: The page's examples and speed: FLIRT's `flirt`; its agreement charts and random testing cover
#: every program of FSL's FLIRT package, named as a part of FSL.
PAGE = fsl_pages.page("flirt", texts=TEXTS)
CHARTS = fsl_pages.page("", part="FLIRT programs", texts=TEXTS)
#: The clean room's random-testing tables: a bare "larmorx" there is the clean room.
TRIAL = fsl_pages.page("", part="FLIRT programs", port="clean-room", texts=TEXTS)

_MM = re.compile(r"its matrix is ([0-9.e+-]+) mm from")
_OWN = re.compile(r"FSL's own ([0-9.e+-]+) mm")
_PART = re.compile(r"matrices part by ([0-9.e+-]+) mm")
_SHARE = re.compile(r"images differ in ([0-9.]+%) of their values")


def _num(s: str) -> str:
    """A number of a reason as a sentence gives it: "4.87e-06" as 4.87 × 10⁻⁶."""
    if "e" not in s:
        return s
    m, e = s.split("e")
    sup = str.maketrans("-0123456789", "⁻⁰¹²³⁴⁵⁶⁷⁸⁹")
    return f"{m} × 10{str(int(e)).translate(sup)}"


def _reason(ex: dict, diff: str) -> str:
    return next(d for d in ex["diffs"] if d["id"] == diff)["reason"]


def _found(pat: re.Pattern, s: str) -> str:
    m = pat.search(s)
    if not m:
        raise ValueError(f"a reason without its number ({pat.pattern}): {s[:120]!r}")
    return _num(m.group(1))


def _verdict(ex: dict, diff: str) -> str:
    return next(d for d in ex["diffs"] if d["id"] == diff)["verdict"]


def _replica_bbr(ex: dict) -> str:
    """The replica on a boundary-based call: FSL's bytes, and fMRIPrep's own matrix."""
    fact = next(f["text"] for f in ex["facts"] if f["label"] == "Matrix")
    if (
        _verdict(ex, "replica-fsl") != "bit-identical"
        or "fMRIPrep's own the same bytes" not in fact
    ):
        raise ValueError(f"{ex['id']}: the replica is not FSL's and fMRIPrep's bytes")
    return (
        "the replica gives FSL's registered image, every value, and FSL's matrix byte for byte, "
        "the same bytes as fMRIPrep's own"
    )


def _shows_bbr(where: str):
    def shows(ex: dict) -> tuple[str, list[str]]:
        r = _reason(ex, "orig-fsl")
        mm, own = _found(_MM, r), _found(_OWN, r)
        v = _verdict(ex, "orig-fsl")
        head = f"{where}, {_replica_bbr(ex)}. The clean room's matrix is {mm} mm from FSL's at the 95th percentile of the brain"
        if v == "within-variability":
            tail = f", within FSL's own variability on this input ({own} mm)."
        elif v == "open" and "Within the record's threshold (0.1 mm)" in r:
            tail = (
                f": within its record's 0.1 mm, but beyond FSL's own variability on this input "
                f"({own} mm), so the comparison stays open."
            )
        else:
            raise ValueError(f"{ex['id']}: the clean room's verdict {v}")
        return head + tail, ["replica-fsl", "orig-fsl"]

    return shows


def _shows_real(what: str, fix: str):
    def shows(ex: dict) -> tuple[str, list[str]]:
        if not (_verdict(ex, "replica-fsl") == _verdict(ex, "orig-fsl") == "bit-identical"):
            raise ValueError(f"{ex['id']}: a port that is not FSL's bytes")
        r = _reason(ex, "origfix-orig")
        if fix == "matrix":
            moved = (
                f"its matrix moves by {_found(_PART, r)} mm at the 95th percentile of the brain, "
                f"more than FSL's own {_found(_OWN, r)} mm on a perturbed input"
            )
        else:
            moved = f"{_found(_SHARE, r)} of the values change"
        return (
            f"{what}, the replica and the clean room both give FSL's {'matrix and ' if fix == 'matrix' else ''}"
            f"image, every value. In `fix`, its default, the clean room computes the C "
            f"library's functions correctly rounded (GLIBC-07), and {moved}."
        ), ["replica-fsl", "orig-fsl"]

    return shows


#: "What this shows" of each worked example (the writing guide, section 4), from its own
#: verdicts, reasons and facts; beside it the verdicts of the two ports against FSL.
SHOWS = {
    "fmriprep-bbr": _shows_bbr("On fMRIPrep's own call"),
    "dof12": _shows_real("On FLIRT's default registration", "matrix"),
    "applyxfm-4d": _shows_real("From the same matrix", "values"),
    "panel-ds003592-older": _shows_bbr("On the data panel's 82-year-old"),
    "panel-ds000228-child": _shows_bbr("On the data panel's three-year-old"),
}
#: The case categories of FLIRT's records, as the page names them.
CATEGORIES = {
    "fmriprep": "fMRIPrep's calls",
    "real": "real registrations",
    "bbr": "boundary-based registration",
    "schedule": "schedules",
    "verbose": "verbose levels",
    "convert_xfm": "convert_xfm",
    "programs": "FLIRT's other programs",
    "debug": "-debug",
    "out": "-out's header",
}
#: The worked examples' matrix tables: each column's run as the panels name it.
_COLUMNS = {
    "FSL 6.0.7.23": "FSL flirt 6.0.7.23",
    "replica (reproduce)": "replica, reproduce",
    "replica (default)": "replica, fix",
    "clean-room original (reproduce)": "clean room, reproduce",
    "clean-room original (default)": "clean room, fix",
    "fMRIPrep's own (its image's FSL)": "fMRIPrep's output (FSL in its image)",
    "the initial matrix (-init)": "the initial matrix (-init)",
}
#: The displacement tables' rows, each a comparison named as the examples name it.
_ROWS = {
    "replica − FSL 6.0.7.23": "replica − FSL flirt 6.0.7.23",
    "replica (default) − FSL 6.0.7.23": "replica, fix − FSL flirt 6.0.7.23",
    "clean-room original − FSL 6.0.7.23": "clean room − FSL flirt 6.0.7.23",
    "clean-room original (default) − FSL 6.0.7.23": "clean room, fix − FSL flirt 6.0.7.23",
    "FSL 6.0.7.23 on a perturbed input − FSL 6.0.7.23 (its own variability)":
        "FSL flirt 6.0.7.23 on a perturbed input − FSL flirt 6.0.7.23 (its own variability)",
    "fMRIPrep's own − FSL 6.0.7.23": "fMRIPrep's output − FSL flirt 6.0.7.23",
    "FSL 6.0.7.23 − the initial matrix (what BBR changed)":
        "FSL flirt 6.0.7.23 − the initial matrix (what the registration changed)",
}  # fmt: skip
#: The tables' titles by the guide's pattern.
_TITLES = {
    "matrices": "The matrices as the files hold them, both ports against the original: FSL "
    "`flirt` 6.0.7.23 (input millimetres to reference millimetres, FLIRT's convention)",
    "shift": "The displacement between two matrices over the reference's brain, both ports "
    "against the original: FSL `flirt` 6.0.7.23 (mm)",
    "trial-catchup": "Random testing, the clean room against the original: FSL 6.0.7.23's FLIRT "
    "programs ({cases:,} random calls, {date})",
    "trial-catchup-triage": "The clean room's differences from the original in that run, by "
    "category",
}


def present(doc: dict) -> dict:
    """FLIRT's page in the writing guide's words and order: texts, ports and order only."""
    note = (doc.get("speed") or {}).get("note")
    fsl_pages.present(
        doc,
        PAGE,
        SHOWS,
        "Seconds for the whole command (reading, registering, writing), one run each, on a log "
        "scale. The factor is how many times faster the clean room is on 14 threads than FSL "
        "on one.",
        {
            "FSL 6.0.7.23 (single-threaded)": "FSL flirt 6.0.7.23, 1 thread",
            "replica, 1 thread": "replica, 1 thread",
            "clean-room original, 1 thread": "clean room, 1 thread",
            "clean-room original, 14 threads": "clean room, 14 threads",
        },
        categories=CATEGORIES,
        charts=CHARTS,
        diffs={
            "fsl-init": (
                "FSL flirt 6.0.7.23 − initial alignment (-init)",
                "the original against its starting point: what the registration changed",
            )
        },
        tables={"trial-catchup": TRIAL, "trial-catchup-triage": TRIAL},
    )
    if doc.get("speed"):  # its build is LARMORX's at a commit, not a port's
        doc["speed"]["note"] = present_.text(PAGE, note, port="both")
    for ex in doc["examples"]:
        for t in ex.get("tables") or []:
            kind = t["id"].rsplit("-", 1)[1]
            t["title"] = _TITLES[kind]
            for c in t["columns"]:
                c["label"] = _COLUMNS.get(c["label"], c["label"])
            for r in t["rows"]:
                if kind == "shift":
                    r["head"] = _ROWS[r["head"]]
    for t in doc.get("tables", []):
        if t["id"] == "trial-catchup":  # the run's length, from its row (the guide: its duration)
            minutes = next(r for r in t["rows"] if r["head"].startswith("length"))["cells"][0]["v"]
            t["note"] = t["note"].replace(
                "A random-testing run of the clean room after",
                f"A random-testing run of the clean room, {minutes:.0f} minutes long, after",
            )
        m = re.search(r"the trial of (\d{4}-\d\d-\d\d)", t["title"])
        cases = t["rows"][0]["cells"][0].get("v") if t["id"] == "trial-catchup" else 0
        t["title"] = _TITLES[t["id"]].format(date=m.group(1) if m else "", cases=cases)
    return doc


if __name__ == "__main__":  # compute the cache entries only (``refresh-original WHY``:
    # refresh_original)
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    if sys.argv[1:2] == ["refresh-original"]:
        refresh_original(c, " ".join(sys.argv[2:]))
        sys.exit(0)
    for _i, run, _v, _w in BBR:
        print("done", bbr_entry(c, run).dir)
    for case in REAL.values():
        print("done", real_entry(c, case).dir)
    for _i, run, _v, _w in BBR:
        print("done", orig_bbr_entry(c, run).dir)
    for case in REAL.values():
        print("done", orig_real_entry(c, case).dir)
