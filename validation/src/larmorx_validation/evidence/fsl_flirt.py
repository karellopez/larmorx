# SPDX-License-Identifier: Apache-2.0
"""FLIRT's evidence (the larmorx-nc replica, FSL Licence; the clean-room original is still to be
written): fMRIPrep's boundary-based registration call (``fsl_bbr_wf/flt_bbr``) on three oracle
runs, a default 12-dof registration and ``-applyxfm`` on four volumes of a raw BOLD series, by
FSL 6.0.7.23 and by the replica in its ``reproduce`` and default modes (and at one thread); the
matrices as text with fMRIPrep's own, the displacement each pair of matrices gives over the
reference's brain, and every case of the record.

Every run of FSL is a run of the parity record (``parity.fsl_flirt_replica``), with the same
arguments, environment and inputs, through the oracle-output cache: the page computes no
original result of its own but one, the BOLD reference resampled at fMRIPrep's initial
alignment (``-applyxfm`` with the call's ``-init``), which shows what BBR changed. The replica
runs as a separate program; nothing here imports it. Everything shown lies inside the
reference's brain mask (fMRIPrep's masked T1w), dilated by two voxels: no face can show.
"""

from __future__ import annotations

import copy
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
    info = e.info()
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
    diffs = _diffs(True)
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    mats = {pid: e.dir / pid / mat_name for pid, *_ in PANELS}
    own = next(node.glob("*.mat"))
    mats_same = mats["replica"].read_bytes() == mats["fsl"].read_bytes()
    _verdicts(diffs, same, "this call", mats_same)
    if view == "slices":
        at = common.slice_voxel(ex, "fsl", "init", "registered")
    else:
        at, _k = common.largest_in_slice(ex, "fsl", "init", "registered", "axial")
    default = {
        "output": "registered",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1w", "wm-init", "wm-fsl", "fsl", "replica"],
        "diffs": ["replica-fsl", "fsl-init"],
        "compare": {"a": "fsl", "b": "init"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(OUTPUTS), default=default, window_panel="fsl",
        caption="the crosshair where FSL's result differs most from its initial alignment"
        + ("; the three slices through it are stored at full resolution." if view == "slices"
           else "."),
    )  # fmt: skip
    m = {k: np.loadtxt(p) for k, p in {**mats, "own": own}.items()}
    m["init"] = np.loadtxt(Path(_arg(args, "-init")))
    labels = {
        "fsl": "FSL 6.0.7.23",
        "replica": "replica (reproduce)",
        "replica-fix": "replica (default)",
        "own": "fMRIPrep's own (its image's FSL)",
        "init": "the initial matrix (-init)",
    }
    mat_files = {k: mats[k] for k in ("fsl", "replica", "replica-fix")}
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
            "*identical*: the same bytes as FSL 6.0.7.23's file.",
        ),
        _shift_table(
            f"{ex_id}-shift",
            [
                ("replica − FSL 6.0.7.23", m["replica"], m["fsl"]),
                ("replica (default) − FSL 6.0.7.23", m["replica-fix"], m["fsl"]),
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
            "the displacement each pair gives over the brain."
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
                f"`{info['larmorx_commit'][:12]}`",
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
            {"label": "Grid", "text": common.describe_grid(ref, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: threads and modes"],
            ["reference", "What the registration changed"],
        ],
        "tables": tables,
        "metrics_note": _note(n_vox),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
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
    info = e.info()
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
    diffs = _diffs(False)
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    tables = []
    mats_same = True
    if "-omat" in args:
        mats = {pid: e.dir / pid / _arg(args, "-omat") for pid, *_ in PANELS}
        mats_same = mats["replica"].read_bytes() == mats["fsl"].read_bytes()
        m = {k: np.loadtxt(p) for k, p in mats.items()}
        tables = [
            _matrix_table(
                f"{ex_id}-matrices",
                {k: mats[k] for k in ("fsl", "replica", "replica-fix")},
                {
                    "fsl": "FSL 6.0.7.23",
                    "replica": "replica (reproduce)",
                    "replica-fix": "replica (default)",
                },
                "Each matrix file as written (`-omat`), its numbers as text. *identical*: the "
                "same bytes as FSL 6.0.7.23's file.",
            ),
            _shift_table(
                f"{ex_id}-shift",
                [
                    ("replica − FSL 6.0.7.23", m["replica"], m["fsl"]),
                    ("replica (default) − FSL 6.0.7.23", m["replica-fix"], m["fsl"]),
                ],
                ref,
                brain,
            ),
        ]
    _verdicts(diffs, same, "this call", mats_same)
    at, _k = common.centre_view(ex)
    default = {
        "output": "registered",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1w", *(["wm-fsl"] if not four_d else []), "fsl", "replica"],
        "diffs": ["replica-fsl", "fix-replica"],
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
            "the white-matter mask drawn over FSL's result shows the fit.",
        ),
        "applyxfm-4d": (
            "-applyxfm on a raw BOLD series, sinc interpolation",
            "The record's case `real/applyxfm-4d`: four volumes of ds005040's raw BOLD series "
            "resampled onto the T1w's grid (every second voxel) through fMRIPrep's initial "
            "matrix, with `-applyxfm -interp sinc`, by FSL 6.0.7.23 and the replica in its "
            "reproduce and default modes; choose the volume below the panels.",
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
                f"`{info['larmorx_commit'][:12]}`",
            },
            {"label": "Grid", "text": common.describe_grid(ref, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: threads and modes"],
        ],
        "tables": tables,
        "metrics_note": _note(n_vox),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [common.DS005040]),
    }


# ------------------------------------------------------------------------------------ cases


def _input(a: str) -> str:
    name = a[1:]
    if a.startswith("@dt:"):
        return f"<random testing's input {name.split(':', 1)[1]}>"
    if a.startswith("@fslsch:"):
        return f"$FSLDIR/etc/flirtsch/{a.removeprefix('@fslsch:')}"
    return f"<{name}: derived from ds005040's fMRIPrep run>"


def command(case: str) -> str:
    fp = _suite()
    if case.startswith("fmriprep/"):
        run = case.split("/")[1]
        node = _bbr_node(run)
        return _short_bbr((node / "command.txt").read_text(encoding="utf-8"))
    defs = fp._all_cases()
    if case not in defs:
        return ""
    run = defs[case][2]
    words = [_input(a) if a.startswith("@") else a for a in run.args]
    parts = [run.program, *(shlex.quote(w) if re.search(r"\s", w) or w == "" else w
                            for w in words)]  # fmt: skip
    env = " ".join(f"{k}={v}" for k, v in run.settings if v is not None)
    files = " ".join(f"(with {n} written first)" for n, _ in run.files)
    return " ".join(x for x in (env, " ".join(parts), files) if x)


#: The register entries the replica corrects by default in FLIRT's runs.
CORRECTED = frozenset({"FSL-09", "FSL-14", "FSL-68"})


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


def cases(rec, examples: dict[str, str]) -> list[dict]:
    fp = _suite()
    descs = {c.id: c.description for c in fp.cases("full")}
    out = []
    for r in rec.results:
        desc = descs.get(r["case"], "")
        if r["case"].startswith("fmriprep/"):
            desc = f"fMRIPrep's BBR call ({r['case'].split('/')[1].split('-noreconall')[0]})"
        v, why = case_verdict(r, desc)
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": desc,
                "command": command(r["case"]),
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
        **{
            f"fmriprep/{run}/{_bbr_node(run).parent.parent.parent.parent.name}": ex_id
            for ex_id, run, _v, _w in BBR
        },
        **{case: ex_id for ex_id, case in REAL.items()},
    }
    cs = cases(rec, examples)
    entries = [bbr_entry(ctx, run) for _i, run, _v, _w in BBR]
    entries += [real_entry(ctx, case) for case in REAL.values()]
    status = "experimental"
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "FLIRT",
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
                status,
            ),
        ],
        "cases": cs,
        "cases_total": len(cs),
        "examples": [
            _example_bbr(ctx, ex_id, run, view, words) for ex_id, run, view, words in BBR[:1]
        ]
        + [_example_real(ctx, "dof12"), _example_real(ctx, "applyxfm-4d")]
        + [_example_bbr(ctx, ex_id, run, view, words) for ex_id, run, view, words in BBR[1:]],
    }


if __name__ == "__main__":  # compute the cache entries only
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    for _i, run, _v, _w in BBR:
        print("done", bbr_entry(c, run).dir)
    for case in REAL.values():
        print("done", real_entry(c, case).dir)
