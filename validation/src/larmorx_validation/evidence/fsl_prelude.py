# SPDX-License-Identifier: Apache-2.0
"""PRELUDE's evidence (the larmorx-nc replica, FSL Licence; the clean-room original is the next
part, written from ``specs/prelude.md``): fMRIPrep's seven phase-unwrapping calls (sdcflows'
phase-difference workflow, ``fmap_preproc_wf/<estimator>/phdiff_wf/prelude``: ``prelude
--abs=<magnitude> --mask=<brain mask> --phase=<phase difference, 0 to 2 pi rad>
--unwrap=<out>``) on the four oracle runs that have one (ds005454, ds002603, ds002422 twice,
ds001600 three times): the wrapped phase difference, the magnitude and the mask prelude is given,
the unwrapped phase of FSL 6.0.7.23, of fMRIPrep's own run (its container's FSL) and of the
replica in its reproduce and default modes; two corrections by default on real phase maps
(FSL-272: a NaN first phase value, on ds002603's map; FSL-261: ``--removeramps`` on ds002422's
second map, whose brain reaches the last slice); a synthetic phase whose truth is known; every
case of the record with its verdict; random testing's trial.

Every run of FSL goes through the oracle-output cache (``use`` mode) with the parity record's
arguments, environment and inputs (``parity.fsl_prelude_replica``), so the record's runs are
served, never computed twice; the two derived real calls run through it too. The replica runs as
a separate program; nothing here imports it. Every real image is shown inside sdcflows' brain
mask (prelude's ``--mask``), dilated by two voxels: no face can show.

    python -m larmorx_validation.evidence.fsl_prelude [fmriprep|fsl272|ramps|truth|named]
"""

from __future__ import annotations

import copy
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

TOOL = "fsl-prelude"
RECORD = "fsl-prelude-replica"
#: Random testing's trial of this part (``<workspace>/oracles/difftest/<trial>/summary.json``).
TRIAL = "2026-11-06-prelude-replica"
TWO_PI = 2.0 * np.pi
#: fMRIPrep's calls shown as examples: example id, the fMRIPrep run, the estimator (sdcflows'
#: workflow name), what the field map is.
FMRIPREP = [
    ("fmriprep-ds005454", "ds005454-sub-16-noreconall", "wf_gre16",
     "a GRE phase difference (Siemens, 3 T) of a multiband study"),
    ("fmriprep-ds002603", "ds002603-sub-01-noreconall", "wf_auto_00000",
     "a Philips phase difference at 2 mm (`acq-Mm2`), 128 × 128 × 128 voxels"),
    ("fmriprep-ds002422-run01", "ds002422-sub-02-noreconall", "wf_auto_00000",
     "the first of two phase differences (1.5 T, `run-01`)"),
    ("fmriprep-ds002422-run02", "ds002422-sub-02-noreconall", "wf_auto_00001",
     "the second phase difference (1.5 T, `run-02`), on another grid"),
    ("fmriprep-ds001600-v1", "ds001600-sub-1-noreconall", "wf_auto_00000",
     "two phase images (`acq-v1`, phase1 and phase2): sdcflows subtracts them"),
    ("fmriprep-ds001600-v2", "ds001600-sub-1-noreconall", "wf_auto_00001",
     "two phase images of a second protocol (`acq-v2`)"),
    ("fmriprep-ds001600-v4", "ds001600-sub-1-noreconall", "wf_auto_00002",
     "a phase difference (`acq-v4`)"),
]  # fmt: skip
#: The replica's runs: label, D13 mode.
REPLICA_RUNS = [("replica", "reproduce"), ("replica-fix", "fix")]
#: A notice of the replica's (``larmorx-nc: upstream defect FSL-xx (...)``).
NOTICE = re.compile(r"upstream defect ((?:FSL|GLIBC)-\d+)")
#: FSL-272's example: fMRIPrep's call whose phase is given with its first values NaN.
FSL272_RUN = ("ds002603-sub-01-noreconall", "wf_auto_00000")
#: FSL-261's example: fMRIPrep's call given ``--removeramps`` (its mask reaches the last slice).
RAMPS_RUN = ("ds002422-sub-02-noreconall", "wf_auto_00001")
#: The synthetic example whose true phase is known: the record's case.
TRUTH_CASE = "opt/default"


def _suite():
    from larmorx_validation.parity import fsl_prelude_replica as P

    return P


def _data(run: str) -> dict:
    from larmorx_validation.evidence.ants_panel import data

    return data(run.split("-sub-")[0])


def versions() -> dict[str, str]:
    from larmorx_validation.parity import fsl_versions
    from larmorx_validation.parity.fsl_flirt_replica import fsldir
    from larmorx_validation.parity.fsl_mcflirt_replica import replica_version

    return {
        "fsl": f"FSL {fsl_versions.version_of(fsldir())} (fsl-fugue 2603.0; ~/fsl-6.0.7.23, "
        "never on the PATH)",
        "larmorx-nc": replica_version(),
    }


# --------------------------------------------------------------------- running the programs


def _copier(files: dict[str, Path]):
    def put(d: Path) -> None:
        for name, src in files.items():
            shutil.copy(src, d / name)

    return put


def _oracle(e: fcache.Entry, label: str, args: list[str], put_inputs, settings: dict) -> None:
    """FSL's prelude with ``args`` through the oracle-output cache, its working directory
    holding the inputs ``put_inputs(dir)`` copies there (as the record's case does); the files
    it writes, the inputs left out, into ``<entry>/<label>``; whether the run was served from
    the cache is kept in the entry."""
    from larmorx_validation.parity.fsl_flirt_replica import _environment, fsldir

    P = _suite()
    with tempfile.TemporaryDirectory(prefix="lx-nc-prelude-ev_") as t:
        cwd = Path(t)
        put_inputs(cwd)
        given = {p.name for p in cwd.iterdir()}
        p = P.oracle_run(args, cwd=cwd, settings=settings)
        if p.returncode != 0:
            raise RuntimeError(f"{label}: FSL's prelude exited {p.returncode}: {p.stderr[-800:]}")
        out = e.dir / label
        out.mkdir(exist_ok=True)
        for f in cwd.iterdir():
            if f.name not in given and f.is_file():
                shutil.copy(f, out / f.name)
        (e.dir / f"{label}.stdout.txt").write_text(p.stdout, encoding="utf-8")
    run = fcache.Run(label, [str(fsldir() / "bin" / "prelude"), *args], _environment(settings))
    run.exit, run.seconds = p.returncode, round(p.seconds, 2)
    run.stderr_tail = p.stderr[-2000:]
    e.runs.append(run)
    e.extra.setdefault("oracle_cache", {})[label] = "served" if p.cached else "ran"


def _replica(e: fcache.Entry, label: str, mode: str, args: list[str], put_inputs, settings: dict):
    """The replica on ``args`` in ``mode``, in ``<entry>/<label>`` (it writes its outputs where
    FSL does: its working directory), in the record's environment; the inputs removed after,
    the register entries its notices name kept."""
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    exe = _suite().replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    unset = tuple(k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_", "OMP_")))
    cwd = e.dir / label
    cwd.mkdir(exist_ok=True)
    put_inputs(cwd)
    given = list(cwd.iterdir())
    env = {**_environment(settings), "LARMORX_UPSTREAM_DEFECTS": mode, "OMP_NUM_THREADS": "4"}
    e.run(label, [str(exe), "fsl", "prelude", *args], env=env, unset=unset, cwd=cwd)
    log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
    e.extra.setdefault("named", {})[label] = sorted(set(NOTICE.findall(log)))
    for p in given:
        if p.is_file():
            p.unlink()


def _node(run: str, est: str) -> Path:
    for r, n in _suite().fmriprep_calls():
        if r == run and n.parent.parent.name == est:
            return n
    raise FileNotFoundError(f"fMRIPrep's prelude node {est} of {run}")


def _roles(args: list[str]) -> dict[str, str]:
    """``--phase`` / ``--abs`` / ``--mask`` / ``--unwrap`` -> the file name in ``args``."""
    return {k[2:]: v for k, _s, v in (a.partition("=") for a in args) if _s}


def _compute_fmriprep(run: str, est: str):
    def compute(e: fcache.Entry) -> None:
        P = _suite()
        node = _node(run, est)
        args, files = P.fmriprep_args(run, node)
        for name, src in files.items():
            e.input(f"{name} (fMRIPrep's)", src)
        put = _copier(files)
        _oracle(e, "fsl", args, put, {})
        for label, mode in REPLICA_RUNS:
            _replica(e, label, mode, args, put, {})
        e.finish(
            versions=versions(),
            node=str(node.relative_to(common.fmriprep_run(run))),
            args=args,
            settings="fMRIPrep 25.2.6's prelude node (sdcflows' phase-difference workflow) of "
            "the run: its arguments from command.txt, its inputs from the run's working "
            "directory copied into the run directory under their names, as the record's case "
            "does; FSL 6.0.7.23 through the oracle-output cache (the record's own run) and the "
            "larmorx-nc replica in its reproduce and default modes",
        )

    return compute


def fmriprep_entry(ctx: core.Context, run: str, est: str) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"fmriprep-{run}-{est}",
        f"fMRIPrep's prelude call {est} of {run}: FSL 6.0.7.23 (the record's run, from the oracle "
        "cache) and the replica, reproduce and default modes",
        _compute_fmriprep(run, est),
    )


def _save_like(img, data: np.ndarray, path: Path) -> None:
    """``data`` as float32 with ``img``'s header (no scaling): fsl272_measure.py's recipe."""
    import nibabel as nib

    h = img.header.copy()
    h.set_data_dtype(np.float32)
    h.set_slope_inter(1.0, 0.0)
    nib.save(nib.Nifti1Image(data.astype(np.float32), img.affine, h), str(path))


def _fsl272_inputs(
    e: fcache.Entry, make: bool = False
) -> tuple[list[str], dict[str, Path], dict[str, Path], str]:
    """FSL-272's two phase maps from fMRIPrep's call (``validation/investigations/prelude-spec/
    fsl272_measure.py``'s recipe): the phase moved to [0, 2 pi) as a scanner's phase difference
    in radians comes, as it is (``nonan``) and with its first values in FSL's order NaN (both x
    ends of the first row: background); the call's arguments with ``--savemask``. ``make``
    writes the two maps into the entry (when it is computed; an export only reads them)."""
    import nibabel as nib

    P = _suite()
    run, est = FSL272_RUN
    args, files = P.fmriprep_args(run, _node(run, est))
    pname = _roles(args)["phase"]
    d = e.dir / "inputs"
    if make:
        img = nib.load(str(files[pname]))
        phase = np.mod(np.asarray(img.dataobj, np.float32).astype(np.float64), TWO_PI)
        phase = phase.astype(np.float32)
        d.mkdir(exist_ok=True)
        nan = phase.copy()
        nan[0, 0, 0] = nan[-1, 0, 0] = np.nan
        _save_like(img, phase, d / f"nonan_{pname}")
        _save_like(img, nan, d / f"nan_{pname}")
    others = {k: v for k, v in files.items() if k != pname}
    keep = [a for a in args if not a.startswith(("--unwrap=", "--phase="))]
    call = [*keep, f"--phase={pname}", "--unwrap=uw", "--savemask=mk"]
    return (
        call,
        {**others, pname: d / f"nan_{pname}"},
        {**others, pname: d / f"nonan_{pname}"},
        pname,
    )


def _compute_fsl272(e: fcache.Entry) -> None:
    call, with_nan, without, pname = _fsl272_inputs(e, make=True)
    for name, src in with_nan.items():
        what = "its first values NaN" if name == pname else "fMRIPrep's"
        e.input(f"{name} ({what})", src)
    e.input(f"{pname} (moved to [0, 2π), no NaN)", without[pname])
    put = _copier(with_nan)
    _oracle(e, "fsl", call, put, {})
    for label, mode in REPLICA_RUNS:
        _replica(e, label, mode, call, put, {})
    _oracle(e, "fsl-nonan", call, _copier(without), {})
    run, est = FSL272_RUN
    e.finish(
        versions=versions(),
        args=call,
        source=f"{run} {est}",
        settings="fMRIPrep 25.2.6's prelude call of ds002603 (sdcflows' phase-difference "
        "workflow), its phase moved to [0, 2π) as a scanner's phase difference in radians comes "
        "(numpy's mod in double precision, stored as float32) and its first values in FSL's "
        "order (both x ends of the first row, background) set to NaN, as "
        "validation/investigations/prelude-spec/fsl272_measure.py makes it; FSL 6.0.7.23 through "
        "the oracle-output cache on that map and on the same map without the NaN, and the "
        "larmorx-nc replica in its reproduce and default modes on the map with the NaN",
    )


def fsl272_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "fsl272-ds002603",
        "FSL-272 on fMRIPrep's ds002603 phase map with its first values NaN: FSL 6.0.7.23 with "
        "and without the NaN (through the oracle cache) and the replica, reproduce and default",
        _compute_fsl272,
    )  # fmt: skip


def _ramps_call() -> tuple[list[str], dict[str, Path]]:
    P = _suite()
    run, est = RAMPS_RUN
    args, files = P.fmriprep_args(run, _node(run, est))
    return [*args, "--removeramps"], files


def _compute_ramps(e: fcache.Entry) -> None:
    call, files = _ramps_call()
    for name, src in files.items():
        e.input(f"{name} (fMRIPrep's)", src)
    put = _copier(files)
    _oracle(e, "fsl", call, put, {})
    for label, mode in REPLICA_RUNS:
        _replica(e, label, mode, call, put, {})
    run, est = RAMPS_RUN
    e.finish(
        versions=versions(),
        args=call,
        source=f"{run} {est}",
        settings="fMRIPrep 25.2.6's prelude call of ds002422's second phase difference with "
        "--removeramps added (its brain mask reaches the last slice), FSL 6.0.7.23 through the "
        "oracle-output cache and the larmorx-nc replica in its reproduce and default modes",
    )


def ramps_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "ramps-ds002422-run02",
        "FSL-261: fMRIPrep's ds002422 run-02 call with --removeramps: FSL 6.0.7.23 (through the "
        "oracle cache) and the replica, reproduce and default modes", _compute_ramps,
    )  # fmt: skip


def _record_run(case: str):
    return _suite()._all_cases()[case][2]


def _compute_record_case(case: str):
    def compute(e: fcache.Entry) -> None:
        P = _suite()
        run = _record_run(case)
        with tempfile.TemporaryDirectory(prefix="lx-nc-prelude-ev-args_") as t:
            args = P._prepare(run, Path(t))
            for f in sorted(Path(t).iterdir()):
                e.input(f.name, P.inputs() / f.name)

        def put(d: Path) -> None:
            P._prepare(run, d)

        settings = dict(run.settings)
        _oracle(e, "fsl", args, put, settings)
        for label, mode in REPLICA_RUNS:
            _replica(e, label, mode, args, put, settings)
        e.finish(
            versions=versions(),
            args=args,
            settings=f"The record's case {case}, its inputs the suite's derived inputs "
            "(oracles/inputs/fsl-prelude-replica/), by FSL 6.0.7.23 through the oracle-output "
            "cache (the record's own run) and by the larmorx-nc replica in its reproduce and "
            "default modes",
        )

    return compute


def truth_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, TRUTH_CASE.replace("/", "-"),
        f"The record's case {TRUTH_CASE}: FSL 6.0.7.23 (from the oracle cache) and the replica",
        _compute_record_case(TRUTH_CASE),
    )  # fmt: skip


def case_table() -> dict[str, tuple[str, object]]:
    """Every case of the suite and its permanent random-testing cases: id -> (description, its
    ``Run``; None for fMRIPrep's calls)."""
    from larmorx_validation.difftest import permanent

    P = _suite()
    out: dict[str, tuple[str, object]] = {}
    for c in P.cases("full"):
        kind, payload = c.payload
        out[c.id] = (c.description, P._all_cases()[payload][2] if kind == "run" else None)
    for c in permanent.cases("fsl-prelude", "full", "replica"):
        out[c.id] = (c.description, c.payload[1])
    return out


def _compute_named(e: fcache.Entry) -> None:
    """The replica's default mode on every case of the record (no FSL run): its exit status and
    the register entries it names, for the case explorer's reasons."""
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    P = _suite()
    rec = records.load(e.ctx.repo, RECORD)
    defs = case_table()
    nodes = {f"fmriprep/{r}/{n.parent.parent.name}": (r, n) for r, n in P.fmriprep_calls()}
    exe = P.replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    unset = tuple(k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_", "OMP_")))
    default: dict[str, dict] = {}
    for r in rec.results:
        case = r["case"]
        with tempfile.TemporaryDirectory(prefix="lx-nc-prelude-ev-named_") as t:
            d = Path(t)
            if case in nodes:
                args, files = P.fmriprep_args(*nodes[case])
                _copier(files)(d)
                settings: dict = {}
            else:
                run = defs[case][1]
                args = P._prepare(run, d)
                settings = dict(run.settings)
            env = {**_environment(settings), "LARMORX_UPSTREAM_DEFECTS": "fix",
                   "OMP_NUM_THREADS": "4"}  # fmt: skip
            label = case.replace("/", "-")
            x = e.run(label, [str(exe), "fsl", "prelude", *args], env=env, unset=unset, cwd=d,
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
        ctx.figures_context(), TOOL, "cases-default-mode",
        "The replica's default mode on every case of the record: its exit status and the "
        "entries it names", _compute_named,
    )  # fmt: skip


# -------------------------------------------------------------------------- the examples


def _unwrapped(run_dir: Path, name: str) -> np.ndarray:
    return common.load(str(run_dir / name))


def _out_name(args: list[str], run_dir: Path) -> str:
    """The unwrapped phase's file name (``--unwrap``, ``-o`` or ``-u``; FSLOUTPUTTYPE's
    extension)."""
    base = _roles(args).get("unwrap")
    if base is None:
        it = iter(args)
        for a in it:
            if a in ("-o", "-u", "--out"):
                base = next(it)
    stem = base.split(".nii")[0]
    hit = sorted(run_dir.glob(f"{stem}.nii*"))
    if not hit:
        raise FileNotFoundError(f"{run_dir}: no {stem}.nii*")
    return hit[0].name


def _cycles(unwrapped: np.ndarray, wrapped: np.ndarray, mask: np.ndarray) -> dict:
    """The whole cycles prelude added inside ``mask``: (unwrapped − wrapped) / 2π rounded, the
    wrapped phase taken in [−π, π) as FSL rewraps a phase from 0 to 2π; its range and how many
    voxels got a nonzero number, and the largest departure from a whole number of cycles."""
    w = wrapped.astype(np.float64)
    w = w - TWO_PI * np.round(w / TWO_PI)
    d = (unwrapped.astype(np.float64) - w)[mask]
    k = np.round(d / TWO_PI)
    return {
        "min": int(k.min()),
        "max": int(k.max()),
        "moved": int(np.count_nonzero(k)),
        "voxels": int(mask.sum()),
        "residue": float(np.abs(d - TWO_PI * k).max()),
        "values": {int(a): int(b) for a, b in zip(*np.unique(k, return_counts=True), strict=True)},
    }


def _diffs(fmriprep: bool) -> list[dict]:
    d = common.d
    out = [d("replica-fsl", "replica", "fsl", "main", "replica − FSL 6.0.7.23", common.ROLE_LX)]
    if fmriprep:
        out.append(d("replica-own", "replica", "own", "fmriprep",
                     "replica − fMRIPrep's own output", common.ROLE_FMRIPREP))  # fmt: skip
    out.append(d("fix-replica", "replica-fix", "replica", "check",
                 "replica, default − reproduce mode", common.ROLE_SELF))  # fmt: skip
    return out


#: Why the default mode differs, per register entry (its notices name them).
FIX_REASONS = {
    "FSL-261": "FSL-261, corrected by default with a notice: with --removeramps, FSL estimates, "
    "removes and restores the linear phase ramps over every column, row and slice but the last "
    "(its loops stop one short), so the last slice, here holding part of the brain, is unwrapped "
    "with its ramp while the rest is unwrapped without it; the default takes every column, row "
    "and slice. The reproduce mode gives FSL's bytes.",
    "FSL-272": "FSL-272, corrected by default with a notice: FSL takes the phase's maximum and "
    "minimum from its first value on, so a NaN first value makes both NaN: the phase from 0 to "
    "2π is not rewrapped to [−π, π] (nor would a range above 3π be refused), and it is "
    "unwrapped in other intervals, whole cycles wrong in some regions; the default takes the "
    "extrema over the values that are numbers. The reproduce mode gives FSL's bytes.",
}


def _verdicts(diffs: list[dict], same: dict[str, bool], info: dict) -> None:
    named = [n for n in info.get("named", {}).get("replica-fix", []) if n.startswith("FSL-")]
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if x["kind"] in ("defect", "truth"):
            continue  # their reasons are the example's own
        if same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "replica-fsl": "The same values, every one, in the unwrapped phase (the record "
                "compares every file prelude writes, its messages and its exit status too).",
                "replica-own": "fMRIPrep's own unwrapped phase (prelude in fMRIPrep 25.2.6's "
                "image, FSL's earlier build) holds the same values.",
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


def _note(n: int, extra: str = "") -> str:
    return (
        f"Δ is A − B in radians, computed in float64 over all {n:,} voxels of the outputs, not "
        "only those shown. The record's threshold: every file prelude writes bit-identical "
        "(images decompressed, `descrip` left out), with FSL's exit status and messages." + extra
    )


def _programs(info: dict) -> str:
    vers = info.get("versions", {})
    served = info.get("oracle_cache", {})
    how = (
        "its run served from the oracle cache, as the record ran it"
        if served.get("fsl") == "served"
        else "run for this page through the oracle cache"
    )
    return (
        f"{vers.get('fsl', '?')} ({how}); {vers.get('larmorx-nc', '?')} (the replica), from "
        f"larmorx at commit `{info['larmorx_commit'][:12]}`"
    )


def _show(mask: np.ndarray, iterations: int) -> np.ndarray:
    from scipy import ndimage

    return ndimage.binary_dilation(mask > 0, iterations=iterations)


def _input_panels(ex, files: dict[str, Path], roles: dict[str, str], what: dict[str, str]) -> None:
    """The phase, magnitude and mask prelude was given (stored as in their files)."""
    from larmorx_validation.evidence import example as exm

    ph, ph_s, ph_i = common.stored(files[roles["phase"]])
    ex.add(exm.Panel("phase", "input", what["phase"], what["phase_sub"], {"all": ph},
                     scale=ph_s, inter=ph_i))  # fmt: skip
    mag, mag_s, mag_i = common.stored(files[roles["abs"]])
    ex.add(exm.Panel("mag", "input", what["mag"], what["mag_sub"], {"all": mag},
                     scale=mag_s, inter=mag_i))  # fmt: skip
    if "mask" in roles:
        mk, mk_s, mk_i = common.stored(files[roles["mask"]])
        from larmorx_validation.evidence.fsl_bet import window_of

        ex.add(
            exm.Panel(
                "mask",
                "input",
                what["mask"],
                what["mask_sub"],
                {"all": mk},
                scale=mk_s,
                inter=mk_i,
                overlay={"unwrapped": {"on": "mag", "window": window_of(ex, "mag")}},
            )
        )


def _example_fmriprep(ctx: core.Context, ex_id: str, run: str, est: str, what: str) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    P = _suite()
    e = fmriprep_entry(ctx, run, est)
    info = e.info()
    node = _node(run, est)
    args, files = P.fmriprep_args(run, node)
    roles = _roles(args)
    name = _out_name(args, e.dir / "fsl")
    pimg = nib.load(str(files[roles["phase"]]))
    mask = common.load(str(files[roles["mask"]]))
    show = _show(mask, 2)
    grid = core.Grid.around(pimg, show)
    ex = exm.Example(ctx, grid, show)
    ds = run.split("-noreconall")[0]
    two = "phase1" in roles["phase"]
    _input_panels(ex, files, roles, {
        "phase": f"{ds}: phase difference (rad)",
        "phase_sub": "prelude's --phase in fMRIPrep: "
        + ("sdcflows' difference of the two phase images, each rescaled to radians"
           if two else "sdcflows' au2rads of the scanner's phase difference")
        + ", 0 to 2π (float32), wrapped",
        "mag": f"{ds}: magnitude",
        "mag_sub": "prelude's --abs: sdcflows' clipped magnitude (int16, 0 to 255)",
        "mask": f"{ds}: brain mask",
        "mask_sub": "prelude's --mask: sdcflows' brain mask of the magnitude, over the magnitude",
    })  # fmt: skip
    arrays = {
        lab: {"unwrapped": _unwrapped(e.dir / lab, name)}
        for lab in ("fsl", "replica", "replica-fix")
    }
    own = {"unwrapped": _unwrapped(node, name)}
    ex.add(exm.Panel("fsl", "original", "FSL 6.0.7.23, prelude",
                     "the conda installation, as built (the record's run, from the oracle cache)",
                     arrays["fsl"]))  # fmt: skip
    ex.add(exm.Panel("own", "fmriprep", "fMRIPrep 25.2.6's own output",
                     "its container's prelude (FSL's earlier build), the node's file",
                     own))  # fmt: skip
    ex.add(exm.Panel("replica", "larmorx", "larmorx-nc replica", "reproduce mode",
                     arrays["replica"]))  # fmt: skip
    ex.add(exm.Panel("replica-fix", "larmorx", "larmorx-nc replica",
                     "default mode (D13 corrections)", arrays["replica-fix"]))  # fmt: skip
    diffs = _diffs(True)
    outputs = [{"id": "unwrapped", "label": "Unwrapped phase (rad)", "unit": "rad"}]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    _verdicts(diffs, same, info)
    at, _k = common.largest_in_slice(ex, "fsl", "phase", "unwrapped", "axial")
    default = {
        "output": "unwrapped",
        "plane": "axial",
        "voxel": at,
        "panels": ["phase", "mag", "fsl", "replica"],
        "diffs": ["replica-fsl", "replica-own"],
        "compare": {"a": "fsl", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="fsl",
        caption="the crosshair where FSL's unwrapped phase departs most from the wrapped "
        "phase it was given in this slice (the most cycles added); the replica's output equals "
        "FSL's everywhere.",
    )  # fmt: skip
    m = mask > 0
    cyc = _cycles(arrays["fsl"]["unwrapped"], common.load(str(files[roles["phase"]])), m)
    data = _data(run)
    n_vox = int(np.prod(pimg.shape[:3]))
    vox_ml = float(np.prod(pimg.header.get_zooms()[:3])) / 1000.0
    return {
        "id": ex_id,
        "title": f"fMRIPrep's call on {ds}: {what.split(' (')[0].split(':')[0]}",
        "summary": f"The prelude node fMRIPrep 25.2.6 ran on {ds} ({what}; sdcflows' "
        f"`{est}` estimator), given to FSL 6.0.7.23 and to the larmorx-nc replica in its "
        "reproduce and default modes, with fMRIPrep's own output beside them. The input panels "
        "are what prelude is given: the phase difference wrapped into 0 to 2π, the magnitude, "
        "and the brain mask (drawn over the magnitude). Unwrapping adds whole cycles of 2π, "
        "region by region, so the phase varies smoothly inside the mask; sdcflows then turns "
        "it into the field in Hz.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): the field "
                "map's phase difference and magnitude as sdcflows prepared them; shown inside "
                "sdcflows' brain mask (prelude's --mask), dilated by 2 voxels "
                f"({float(np.count_nonzero(m)) * vox_ml:,.0f} ml in the mask), so no face shows",
            },
            {"label": "Node", "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}`"},
            {"label": "Command", "text": "`" + command_fmriprep(args) + "`"},
            {
                "label": "Cycles added",
                "text": f"{cyc['moved']:,} of the {cyc['voxels']:,} voxels in the mask moved by "
                f"whole cycles of 2π, from {cyc['min']} to {cyc['max']} (FSL's output against "
                "the wrapped phase rewrapped to [−π, π]; every one a whole number of cycles to "
                f"{cyc['residue']:.2g} rad)",
            },
            {"label": "Programs", "text": _programs(info)},
            {"label": "Grid", "text": common.describe_grid(pimg, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["fmriprep", "LARMORX against fMRIPrep's own output"],
            ["check", "LARMORX against itself: the modes"],
        ],
        "metrics_note": _note(n_vox),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
        "cycles": cyc,
    }


def _example_fsl272(ctx: core.Context) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    e = fsl272_entry(ctx)
    info = e.info()
    call, with_nan, _without, pname = _fsl272_inputs(e)
    roles = _roles(call)
    pimg = nib.load(str(with_nan[pname]))
    mask = common.load(str(with_nan[roles["mask"]]))
    show = _show(mask, 2)
    grid = core.Grid.around(pimg, show)
    ex = exm.Example(ctx, grid, show)
    run = FSL272_RUN[0]
    ds = run.split("-noreconall")[0]
    _input_panels(ex, with_nan, roles, {
        "phase": f"{ds}: phase difference, first values NaN",
        "phase_sub": "fMRIPrep's --phase moved to [0, 2π) (float32), its two first values in "
        "FSL's order (background corners, outside the view) NaN",
        "mag": f"{ds}: magnitude",
        "mag_sub": "prelude's --abs in fMRIPrep: sdcflows' clipped magnitude",
        "mask": f"{ds}: brain mask",
        "mask_sub": "prelude's --mask in fMRIPrep, over the magnitude",
    })  # fmt: skip
    name = _out_name(call, e.dir / "fsl")
    arrays = {lab: {"unwrapped": _unwrapped(e.dir / lab, name)}
              for lab in ("fsl", "replica", "replica-fix", "fsl-nonan")}  # fmt: skip
    ex.add(exm.Panel("fsl", "original", "FSL 6.0.7.23, prelude, the NaN map",
                     "as built, through the oracle cache: neither rewrapped nor range-checked",
                     arrays["fsl"]))  # fmt: skip
    ex.add(exm.Panel("replica", "larmorx", "larmorx-nc replica", "reproduce mode",
                     arrays["replica"]))  # fmt: skip
    ex.add(exm.Panel("replica-fix", "larmorx", "larmorx-nc replica",
                     "default mode (FSL-272 corrected)", arrays["replica-fix"]))  # fmt: skip
    ex.add(
        exm.Panel(
            "fsl-nonan",
            "reference",
            "FSL 6.0.7.23, the same map without the NaN",
            "what FSL gives when the first value is a number",
            arrays["fsl-nonan"],
        )
    )
    d = common.d
    diffs = [
        d("replica-fsl", "replica", "fsl", "main", "replica (reproduce) − FSL 6.0.7.23",
          common.ROLE_LX),
        d("fix-nonan", "replica-fix", "fsl-nonan", "main",
          "replica (default) − FSL without the NaN", common.ROLE_LX),
        d("fix-replica", "replica-fix", "replica", "check", "replica, default − reproduce mode",
          common.ROLE_SELF),
        d("nan-nonan", "fsl", "fsl-nonan", "defect", "FSL with the NaN − FSL without it",
          "the original against itself"),
    ]  # fmt: skip
    outputs = [{"id": "unwrapped", "label": "Unwrapped phase (rad)", "unit": "rad"}]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    _verdicts(diffs, same, info)
    m = (mask > 0) & np.isfinite(common.load(str(with_nan[pname])))
    k = np.round((arrays["fsl"]["unwrapped"].astype(np.float64)
                  - arrays["fsl-nonan"]["unwrapped"].astype(np.float64))[m] / TWO_PI)  # fmt: skip
    u, c = np.unique(k.astype(int), return_counts=True)
    common_k = int(u[int(np.argmax(c))])
    wrong = int(np.count_nonzero(k != common_k))
    cycles = ", ".join(f"{int(a):+d} cycles in {int(b):,}" for a, b in zip(u, c, strict=True))
    for x in diffs:
        if x["id"] == "nan-nonan":
            x["verdict"] = None
            x["reason"] = (
                "The defect's effect, FSL against itself (no verdict): with the NaN first value "
                "FSL keeps the phase in 0 to 2π and unwraps it in other intervals than the "
                f"rewrapped phase, so of the {int(m.sum()):,} mask voxels {cycles} voxels "
                f"differ from its own result without the NaN: {wrong:,} voxels "
                f"({100 * wrong / max(1, int(m.sum())):.1f} %) by another number of cycles than "
                "the rest, regions unwrapped whole cycles wrong relative to their neighbours."
            )
            x["links"] = [{"label": "FSL-272", "href": "docs/findings/upstream-defects.md"}]
        elif x["id"] == "fix-nonan":
            mt = x["metrics"]["unwrapped"]
            if same["fix-nonan"]:
                x["reason"] = (
                    "The default gives FSL's own result for the same map without the NaN, bit "
                    "for bit: it takes the extrema over the values that are numbers, so it "
                    "rewraps the phase as FSL does when the first value is one."
                )
            else:
                x["verdict"] = "d13"
                x["reason"] = (
                    f"The default rewraps the phase as FSL does without the NaN: {mt['differ']:,} "
                    f"values differ from FSL's result there, by at most {mt['max_abs']:.2g} rad "
                    "(no whole cycle)."
                )
            x["links"] = [{"label": "FSL-272", "href": "docs/findings/upstream-defects.md"}]
    at, _k = common.largest_in_slice(ex, "fsl", "fsl-nonan", "unwrapped", "axial")
    default = {
        "output": "unwrapped",
        "plane": "axial",
        "voxel": at,
        "panels": ["phase", "fsl", "replica-fix", "fsl-nonan"],
        "diffs": ["nan-nonan", "fix-nonan"],
        "compare": {"a": "fsl", "b": "fsl-nonan"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="fsl-nonan",
        caption="the crosshair where FSL's output with the NaN departs most from its output "
        "without it in this slice.",
    )  # fmt: skip
    data = _data(run)
    return {
        "id": "fsl272",
        "title": "A NaN first value, and a correction by default (FSL-272)",
        "summary": "fMRIPrep's call on ds002603's phase map, the phase moved to [0, 2π) as a "
        "scanner's phase difference in radians comes and its first two values in FSL's order "
        "(background corners) set to NaN, as a masked or divided phase map can hold. FSL takes "
        "the phase's maximum and minimum from its first value on, so both are NaN: the phase is "
        "not rewrapped to [−π, π] and is unwrapped in other intervals. The replica's reproduce "
        "mode gives FSL's bytes; its default takes the extrema over the values that are numbers "
        "and gives what FSL gives for the same map without the NaN. The difference *FSL with the "
        "NaN − FSL without it* shows the defect: whole cycles, and other numbers of them in some "
        "regions. fMRIPrep's own calls are not affected: sdcflows rescales the phase from its "
        "minimum and maximum, so a NaN anywhere would make every value NaN.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): fMRIPrep's "
                "phase difference of the field map, changed as described; shown inside "
                "sdcflows' brain mask, dilated by 2 voxels, so no face shows",
            },
            {"label": "Command", "text": "`" + command_fmriprep(call) + "`"},
            {"label": "Programs", "text": _programs(info)},
            {"label": "Grid", "text": common.describe_grid(pimg, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: the modes"],
            ["defect", "FSL against itself: what the NaN changes (the defect)"],
        ],
        "metrics_note": _note(int(np.prod(pimg.shape[:3]))),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data],
                                        ["Recipe: validation/investigations/prelude-spec/"
                                         "fsl272_measure.py (FSL-272's measurement)"]),
    }  # fmt: skip


def _fix_counts(rec, prefix: str) -> list[tuple[str, int]]:
    """The record's cases whose id starts with ``prefix``: (case, values its default mode
    changes, the record's D13 fix pass)."""
    out = []
    for r in rec.results:
        if r["case"].startswith(prefix):
            n = r.get("reported", {}).get("D13 fix pass")
            out.append((r["case"], int(n) if isinstance(n, int) else 0))
    return out


def _example_ramps(ctx: core.Context, rec) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    e = ramps_entry(ctx)
    info = e.info()
    call, files = _ramps_call()
    roles = _roles(call)
    pimg = nib.load(str(files[roles["phase"]]))
    mask = common.load(str(files[roles["mask"]]))
    show = _show(mask, 2)
    grid = core.Grid.around(pimg, show)
    ex = exm.Example(ctx, grid, show)
    run = RAMPS_RUN[0]
    ds = run.split("-noreconall")[0]
    _input_panels(ex, files, roles, {
        "phase": f"{ds} run-02: phase difference (rad)",
        "phase_sub": "prelude's --phase in fMRIPrep (sdcflows' au2rads, 0 to 2π, float32)",
        "mag": f"{ds} run-02: magnitude",
        "mag_sub": "prelude's --abs in fMRIPrep: sdcflows' clipped magnitude",
        "mask": f"{ds} run-02: brain mask",
        "mask_sub": "prelude's --mask in fMRIPrep, over the magnitude: it reaches the last slice",
    })  # fmt: skip
    name = _out_name(call, e.dir / "fsl")
    arrays = {lab: {"unwrapped": _unwrapped(e.dir / lab, name)}
              for lab in ("fsl", "replica", "replica-fix")}  # fmt: skip
    ex.add(exm.Panel("fsl", "original", "FSL 6.0.7.23, prelude --removeramps",
                     "as built, through the oracle cache", arrays["fsl"]))  # fmt: skip
    ex.add(exm.Panel("replica", "larmorx", "larmorx-nc replica", "reproduce mode",
                     arrays["replica"]))  # fmt: skip
    ex.add(exm.Panel("replica-fix", "larmorx", "larmorx-nc replica",
                     "default mode (FSL-261 corrected)", arrays["replica-fix"]))  # fmt: skip
    diffs = _diffs(False)
    outputs = [{"id": "unwrapped", "label": "Unwrapped phase (rad)", "unit": "rad"}]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    _verdicts(diffs, same, info)
    dv = np.abs(arrays["replica-fix"]["unwrapped"].astype(np.float64)
                - arrays["replica"]["unwrapped"].astype(np.float64))  # fmt: skip
    m = mask > 0
    nz = int(pimg.shape[2])
    last = int(np.count_nonzero(dv[:, :, nz - 1][m[:, :, nz - 1]]))
    other = int(np.count_nonzero(dv[:, :, : nz - 1][m[:, :, : nz - 1]]))
    for x in diffs:
        if x["id"] == "fix-replica" and x.get("verdict") == "d13":
            mt = x["metrics"]["unwrapped"]
            synth = ", ".join(f"`{c}` {n:,}" for c, n in _fix_counts(rec, "opt/removeramps") if n)
            x["reason"] += (
                f" Here the brain mask holds {int(m[:, :, nz - 1].sum()):,} voxels of the last "
                f"slice: {last:,} of them change, and {other:,} voxels of the other slices (the "
                f"ramps estimated over every slice), by at most {mt['max_abs']:.2g} rad (RMS "
                f"{mt['rms']:.2g}): the same whole cycles everywhere, only the last bits of the "
                "ramps' removal and restoration move. Not important on this map; the correction "
                "matters where the last column, row or slice holds more of the image, or where "
                "the image has one slice (FSL then removes no ramp at all): the values the "
                f"record's synthetic cases change, {synth}."
            )
    at, _k = common.largest_in_slice(ex, "replica-fix", "replica", "unwrapped", "coronal")
    default = {
        "output": "unwrapped",
        "plane": "coronal",
        "voxel": at,
        "panels": ["phase", "fsl", "replica", "replica-fix"],
        "diffs": ["replica-fsl", "fix-replica"],
        "compare": {"a": "replica-fix", "b": "replica"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="fsl",
        caption="the crosshair where the default mode differs most from the reproduce mode "
        "(FSL's bytes) in this coronal slice; the top slice is the image's last.",
    )  # fmt: skip
    data = _data(run)
    return {
        "id": "ramps",
        "title": "--removeramps, and a correction by default (FSL-261)",
        "summary": "fMRIPrep's call on ds002422's second phase difference with `--removeramps` "
        "added: prelude estimates the phase's linear ramp along each axis, removes it before "
        "unwrapping and restores it after. FSL's loops stop one column, row and slice short, so "
        "the last slice, which this brain mask reaches, keeps its ramp through the unwrapping "
        "while the rest loses it. The replica's reproduce mode gives FSL's bytes; its default "
        "takes every column, row and slice (FSL-261), so the ramps are estimated over the whole "
        "mask and removed and restored everywhere. fMRIPrep does not ask for the ramps' removal.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): fMRIPrep's "
                "inputs of the call; shown inside sdcflows' brain mask, dilated by 2 voxels, so "
                "no face shows",
            },
            {"label": "Command", "text": "`" + command_fmriprep(call) + "`"},
            {"label": "Programs", "text": _programs(info)},
            {"label": "Grid", "text": common.describe_grid(pimg, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: the modes"],
        ],
        "metrics_note": _note(int(np.prod(pimg.shape[:3]))),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
    }


def _example_truth(ctx: core.Context) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    P = _suite()
    e = truth_entry(ctx)
    info = e.info()
    args = info["args"]
    base = P.inputs()
    ph_name = args[args.index("-p") + 1]
    mag_name = args[args.index("-a") + 1]
    pimg = nib.load(str(base / ph_name))
    shape = tuple(pimg.shape[:3])
    name = _out_name(args, e.dir / "fsl")
    arrays = {lab: {"unwrapped": _unwrapped(e.dir / lab, name)}
              for lab in ("fsl", "replica", "replica-fix")}  # fmt: skip
    # prelude writes 0 outside its mask: the truth is shown and compared inside it
    inside = arrays["fsl"]["unwrapped"] != 0
    truth = np.where(inside, P._true_phase(shape, 1), 0.0).astype(np.float32)
    wrapped = common.load(str(base / ph_name))
    mag = common.load(str(base / mag_name))
    show = common.unmasked(wrapped, mag, truth, *(a["unwrapped"] for a in arrays.values()))
    grid = core.Grid.around(pimg, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("truth", "input", "the true phase (rad), inside prelude's mask",
                     "the suite's smooth phase: a ramp and a bowl spanning several cycles, with "
                     "noise (float64, shown in float32); 0 outside the mask, as prelude writes",
                     {"all": truth}))  # fmt: skip
    ex.add(exm.Panel("phase", "input", "the wrapped phase (rad)",
                     "the true phase wrapped into [−π, π) (float32): prelude's -p",
                     {"all": wrapped}))  # fmt: skip
    ex.add(exm.Panel("mag", "input", "the magnitude", "a head-like bright ellipsoid over "
                     "background noise (float32): prelude's -a", {"all": mag}))  # fmt: skip
    ex.add(
        exm.Panel(
            "fsl",
            "original",
            "FSL 6.0.7.23, prelude",
            "as built (the record's run, from the oracle cache)",
            arrays["fsl"],
        )
    )
    ex.add(exm.Panel("replica", "larmorx", "larmorx-nc replica", "reproduce mode",
                     arrays["replica"]))  # fmt: skip
    ex.add(exm.Panel("replica-fix", "larmorx", "larmorx-nc replica",
                     "default mode (D13 corrections)", arrays["replica-fix"]))  # fmt: skip
    diffs = _diffs(False)
    diffs.append(common.d("fsl-truth", "fsl", "truth", "truth", "FSL 6.0.7.23 − the true phase",
                          "the original against the truth"))  # fmt: skip
    outputs = [{"id": "unwrapped", "label": "Unwrapped phase (rad)", "unit": "rad"}]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    _verdicts(diffs, same, info)
    u = arrays["fsl"]["unwrapped"].astype(np.float64)
    m = u != 0
    dt = (u - truth.astype(np.float64))[m]
    k = np.round(dt / TWO_PI)
    vals, counts = np.unique(k.astype(int), return_counts=True)
    resid = np.abs(dt - TWO_PI * k)
    for x in diffs:
        if x["id"] == "fsl-truth":
            x["verdict"] = None
            plus = (
                f"plus {int(vals[0])} × 2π in every one of them" if len(vals) == 1
                else "plus " + ", ".join(f"{int(a):+d} × 2π in {int(b):,} voxels"
                                         for a, b in zip(vals, counts, strict=True))
            )  # fmt: skip
            x["reason"] = (
                "What unwrapping recovers (no verdict: the truth is not a program's output): "
                f"inside prelude's mask ({int(m.sum()):,} voxels) FSL's unwrapped phase is the "
                f"true phase {plus}, to {float(resid.max()):.2g} rad (the wrapped phase's float32 "
                "rounding): every wrap undone. The one constant whole number of cycles that no "
                "unwrapping can know is set by the reference region prelude starts from."
            )
    at, _k = common.largest_in_slice(ex, "fsl", "phase", "unwrapped", "axial")
    default = {
        "output": "unwrapped",
        "plane": "axial",
        "voxel": at,
        "panels": ["truth", "phase", "fsl", "replica"],
        "diffs": ["replica-fsl", "fsl-truth"],
        "compare": {"a": "fsl", "b": "truth"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="fsl",
        caption="the crosshair where FSL's unwrapped phase departs most from the wrapped phase "
        "in this slice; the whole field of view is shown (a synthetic image).",
    )  # fmt: skip
    return {
        "id": "truth",
        "title": "A phase whose truth is known",
        "summary": "The record's case `opt/default` on the suite's synthetic phase map (40 × 36 "
        "× 12 voxels): a smooth phase spanning several cycles, with noise, wrapped into [−π, π), "
        "and a magnitude from which prelude makes its own mask (no `--mask`). The true phase is "
        "an input panel: FSL's unwrapped phase and the replica's in both modes are the true "
        "phase plus one constant number of whole cycles inside the mask (the comparison *FSL − "
        "the true phase*).",
        "facts": [
            {
                "label": "Data",
                "text": "synthetic, generated by the parity suite with seeded random numbers "
                "(`parity.fsl_prelude_replica._true_phase`, `_magnitude`; "
                "oracles/inputs/fsl-prelude-replica/v4); no subject",
            },
            {"label": "Case", "text": f"`{TRUTH_CASE}` of the record (`{RECORD}`)"},
            {"label": "Command", "text": "`" + command(TRUTH_CASE, case_table()) + "`"},
            {"label": "Programs", "text": _programs(info)},
            {"label": "Grid", "text": common.describe_grid(pimg, grid.shape).replace(
                "around the brain", "the whole field of view")},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: the modes"],
            ["truth", "FSL against the true phase"],
        ],
        "metrics_note": _note(int(np.prod(shape))),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", []),
    }  # fmt: skip


def _calls_table(examples: list[dict]) -> dict:
    """fMRIPrep's seven calls in one table: grid, mask, cycles added, and the comparisons."""
    rows = []
    for ex in examples:
        if not ex["id"].startswith("fmriprep-"):
            continue
        by = {d["id"]: d for d in ex["diffs"]}
        cyc = ex["cycles"]
        grid = next(f["text"] for f in ex["facts"] if f["label"] == "Grid").split(";")[0]
        rows.append({
            "head": ex["title"].split(": ", 1)[0].replace("fMRIPrep's call on ", ""),
            "sub": ex["title"].split(": ", 1)[1] if ": " in ex["title"] else "",
            "cells": [
                common.cell(grid),
                common.cell(cyc["voxels"], "count"),
                common.cell(f"{cyc['min']} to {cyc['max']}"),
                common.cell(cyc["moved"], "count"),
                common.cell("identical" if by["replica-fsl"]["verdict"] == "bit-identical"
                            else "differs"),
                common.cell("identical" if by["replica-own"]["verdict"] == "bit-identical"
                            else "differs"),
                common.cell("identical" if by["fix-replica"]["verdict"] == "bit-identical"
                            else "differs"),
            ],
            "verdict": "bit-identical" if all(by[i]["verdict"] == "bit-identical" for i in
                                              ("replica-fsl", "replica-own")) else "open",
        })  # fmt: skip
    return common.table(
        "fmriprep-calls",
        "fMRIPrep's seven prelude calls, against FSL 6.0.7.23",
        ["Run", "Grid", {"label": "Voxels in the mask", "num": True}, "Whole cycles added",
         {"label": "Voxels moved", "num": True}, "Replica (reproduce) against FSL",
         "Replica against fMRIPrep's own", "Replica, default against reproduce"],
        rows,
        note="Each call's unwrapped phase compared byte for byte (the image decompressed, "
        "`descrip` left out): *identical*, every value. *Whole cycles added*: FSL's output "
        "minus the wrapped phase rewrapped to [−π, π], in cycles of 2π, inside the mask; *voxels "
        "moved*: those with a nonzero number. The record compares every file, the messages and "
        "the exit status too.",
    )  # fmt: skip


# ------------------------------------------------------------------------------------ cases


def _input(name: str) -> str:
    if name.startswith("dt:"):
        return f"<random testing's input {name.split(':', 1)[1]}>"
    if name.startswith("panel/"):
        ds = name.split("/")[1].split("_")[0]
        what = (
            "in radians (sdcflows' au2rads)"
            if "rads" in name
            else (
                "magnitude" if "magnitude" in name else "phase difference as the scanner stores it"
            )
        )
        return f"<{ds}'s {what if 'magnitude' not in what else 'first magnitude'}>"
    kind = {
        "s": "synthetic 40 × 36 × 12",
        "b": "synthetic 72 × 70 × 8",
        "o": "synthetic, one slice",
        "n": "synthetic, neurological",
        "t": "synthetic, three volumes",
    }.get(name[:1], "synthetic")
    return f"<{name}: {kind}>"  # fmt: skip


def _arg(a: str) -> str:
    key, sep, value = a.partition("=")
    if not sep:
        key, value = "", a
    if not value.startswith("@"):
        return shlex.quote(a) if re.search(r"\s", a) or a == "" else a
    return f"{key}{sep}{_input(value[1:])}"


def command_fmriprep(args: list[str]) -> str:
    """fMRIPrep's call as the record runs it, its files named by what they are."""
    subs = {"--abs": "<sdcflows' clipped magnitude>", "--mask": "<sdcflows' brain mask>",
            "--phase": "<the phase difference in rad>"}  # fmt: skip
    out = []
    for a in args:
        k, sep, v = a.partition("=")
        if sep and k == "--phase" and v.startswith("nan_"):
            out.append(f"{k}=<the phase difference, its first values NaN>")
        else:
            out.append(f"{k}={subs[k]}" if sep and k in subs else a)
    return "prelude " + " ".join(out)


def command(case: str, defs: dict[str, tuple[str, object]]) -> str:
    if case.startswith("fmriprep/"):
        _f, run, est = case.split("/")
        return command_fmriprep(_suite().fmriprep_args(run, _node(run, est))[0])
    if case not in defs or defs[case][1] is None:
        return ""
    run = defs[case][1]
    parts = ["prelude", *(_arg(a) for a in run.args)]
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
            why += f" The replica's default mode ends with exit status {code}, naming " + (
                f"{', '.join(named)}."
            )
        return verdict, common.clip(why, 600)
    rep = {**r.get("reported", {}), **r.get("measured", {})}
    files = str(rep.get("every file bit-identical", ""))
    fix = rep.get("D13 fix pass")
    if "every file bit-identical" in r.get("reported", {}) and files:
        a, _s, b = files.partition("/")
        if a != b:
            why = (
                f"FSL's values come from memory it never wrote ({', '.join(named) or 'FSL-265'}: "
                f"a complex128 image read with no width): {files} files bit-identical, the "
                "others' values reported, not compared; the exit status, messages and file "
                "names are FSL's"
            )
            if isinstance(fix, int) and fix:
                why += f"; the default reads the image correctly ({fix:,} values change)"
            return "divergence", common.clip(why, 600)
    notes = [
        f"every file FSL writes{f' ({files})' if files else ''} byte for byte, with FSL's exit "
        "status and messages"
    ]
    own = rep.get("fMRIPrep's own output (its container's FSL) against the replica's (reported)")
    if own == "yes":
        notes.append("fMRIPrep's own unwrapped phase (its image's prelude) the same")
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


def cases(rec, examples: dict[str, str], default: dict[str, dict]) -> list[dict]:
    defs = case_table()
    out = []
    for r in rec.results:
        desc, _run = defs.get(r["case"], ("", None))
        if r["case"].startswith("fmriprep/"):
            _f, run, est = r["case"].split("/")
            desc = (
                f"fMRIPrep's phase-unwrapping call ({run.split('-noreconall')[0]}, sdcflows' {est})"
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


# ------------------------------------------------------------------------------ the trial


def trial(ctx: core.Context) -> tuple[dict, dict]:
    """Random testing's trial of this part, from its ``summary.json``: a table, and its
    provenance."""
    path = common.oracles() / "difftest" / TRIAL / "summary.json"
    raw = path.read_bytes()
    s = json.loads(raw)
    t = s["tools"]["fsl-prelude"]
    o = t["options"]
    agree = t["implementations"]["replica"].get("agree", 0)
    types = sum(v for k, v in o.items() if k.startswith("FSLOUTPUTTYPE="))
    rows = [
        ("Random cases, each compared with FSL 6.0.7.23", t["cases"]),
        ("bit-identical (every file, the messages, the exit status, the default's notices)",
         agree),
        ("differing: the random cases' own mistake, drawn again and agreeing (below)",
         t["implementations"]["replica"].get("differ", 0)),
        ("fMRIPrep's call (`--abs`, `--mask`, `--phase`, `--unwrap`)", o.get("fMRIPrep's call", 0)),
        ("phase and magnitude, complex (`-c`)", [o.get("phase and magnitude", 0),
                                                o.get("complex", 0)]),
        ("one, two or three volumes", [t["cases"] - o.get("2 volumes", 0) - o.get("3 volumes", 0),
                                       o.get("2 volumes", 0), o.get("3 volumes", 0)]),
        ("a phase in [−π, π), in [0, 2π), unwrapped already",
         [sum(v for k, v in o.items() if k.startswith(f"phase {w} ")) for w in ("pi", "twopi",
                                                                               "none")]),
        ("`-s`, `--labelslices`, `-f`", [o.get("-s", 0), o.get("--labelslices", 0),
                                         o.get("-f", 0)]),
        ("`-m` or `--mask`, `-t`, `-n`", [o.get("-m", 0) + o.get("--mask", 0), o.get("-t", 0),
                                          o.get("-n", 0)]),
        ("`--removeramps`, `-v`", [o.get("--removeramps", 0), o.get("-v", 0)]),
        ("`--savemask`, `-l`, `-r`", [o.get("savemask", 0), o.get("labels", 0),
                                      o.get("rawphase", 0)]),
        ("`--start`, `--end`", [o.get("--start", 0), o.get("--end", 0)]),
        ("a magnitude or mask of another size (FSL-271)", o.get("FSL-271 other size", 0)),
        ("an output type other than NIFTI_GZ", types - o.get("FSLOUTPUTTYPE=NIFTI_GZ", 0)),
        ("distinct options and settings drawn", t["options_covered"]),
        ("length (minutes; 12 jobs at one thread each)", round(s["elapsed_s"] / 60, 1)),
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
    table = common.table(
        "trial",
        f"Random testing: the trial of {s['date']}",
        ["", {"label": "Cases", "num": True}],
        out_rows,
        note=f"Differential random testing's trial of this part (seed `{s['seed']}`, larmorx "
        f"`{s['larmorx_commit']}`): a smooth wrapped phase over a head phantom (or its complex "
        "image: the magnitude times a power of a rational unit complex number), random "
        "background, 1 to 3 volumes; float32, float64, int16, uint16, complex64 and complex128 "
        "images; FSL's output types; FSL 6.0.7.23 through the oracle cache (`use` mode), the "
        "replica in reproduce mode compared file by file, its default mode checked by D13's "
        "rule. The nine differing cases were the random cases' own mistake (a magnitude of "
        "'another size' drawn on a one-slice image by shortening its one-voxel axis, which "
        "stayed one voxel, so the replica's default rightly did not refuse it): the draw was "
        "fixed and the nine, drawn again, agree (validation/difftest/triage.toml). Every case "
        f"in a memory box of {s['memory']['case']} (peaks below {peaks:.2f} GiB).",
    )
    prov = {
        "trial": f"oracles/difftest/{TRIAL}/summary.json",
        "sha256": hashlib.sha256(raw).hexdigest(),
    }
    return table, prov


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, RECORD)
    examples_by_case = {f"fmriprep/{run}/{est}": ex_id for ex_id, run, est, _w in FMRIPREP}
    examples_by_case[TRUTH_CASE] = "truth"
    ne = named_entry(ctx)
    cs = cases(rec, examples_by_case, ne.info().get("default_mode", {}))
    exs = [_example_fmriprep(ctx, *x) for x in FMRIPREP]
    calls = _calls_table(exs)
    for ex in exs:
        ex.pop("cycles", None)
    exs += [_example_fsl272(ctx), _example_ramps(ctx, rec), _example_truth(ctx)]
    entries = [fmriprep_entry(ctx, run, est) for _i, run, est, _w in FMRIPREP]
    entries += [fsl272_entry(ctx), ramps_entry(ctx), truth_entry(ctx), ne]
    tt, tprov = trial(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "prelude",
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
        "tables": [calls, tt],
        "examples": exs,
    }


if __name__ == "__main__":  # compute the cache entries only: [fmriprep|fsl272|ramps|truth|named]
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    what = sys.argv[1:] or ["fmriprep", "fsl272", "ramps", "truth", "named"]
    if "fmriprep" in what:
        for _i, run, est, _w in FMRIPREP:
            print("done", fmriprep_entry(c, run, est).dir, flush=True)
    for w, fn in (("fsl272", fsl272_entry), ("ramps", ramps_entry), ("truth", truth_entry),
                  ("named", named_entry)):  # fmt: skip
        if w in what:
            print("done", fn(c).dir, flush=True)
