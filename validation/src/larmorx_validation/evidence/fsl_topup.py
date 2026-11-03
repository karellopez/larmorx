# SPDX-License-Identifier: Apache-2.0
"""topup's and applytopup's evidence (the larmorx-nc replicas, FSL Licence, and LARMORX's
clean-room originals, crate larmorx-fsl, Apache-2.0): fMRIPrep's field-map estimation call
(sdcflows' PEPOLAR workflow, ``fmap_preproc_wf/<estimator>/topup``: ``topup --config=b02b0.cnf
--datain=<encoding file> --imain=<merged pair> --out --iout --fout --jacout --logout --rbmout
--dfout``) on its two oracle runs (ds005040 and ds001600): the reversed-phase-encoding pair
before and after the correction, the field, the Jacobians and the displacements, by FSL 6.0.7.23
(at one and at four threads: its own thread band), by the replica in its reproduce and default
modes (and at four threads) and by the clean-room original in its reproduce and default modes at
14 threads (and at one); a correction by default inside topup (FSL-117: membrane energy with
Levenberg-Marquardt); applytopup's least-squares restoration, Jacobian modulation and vb3D on
topup's own result (FSL-132, FSL-142 and FSL-143 corrected by default); every case of both
records with its verdict (the original's classified from its files against FSL's, and against
FSL's own thread band where the record measures it); random testing's two trials, the original's
with its triage; the speed.

Every run of FSL is a run of the parity records (``parity.fsl_topup_replica``, whose keys the
original's ``parity.fsl_topup`` shares), with the same arguments, environment and inputs,
through the oracle-output cache (``use`` mode): the page computes no original result of its own,
apart from FSL at four threads on fMRIPrep's calls (through the cache too). The replica runs as a
separate program; nothing here imports it. Everything shown lies inside sdcflows' brain mask of
the corrected reference (dilated): no face can show.
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
    "movements": "opt/levels",
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
#: The clean-room original's record (``parity.fsl_topup``), and its runs on the examples:
#: label, D13 mode, threads (its result is the same bits at any thread count).
RECORD_ORIG = "fsl-topup"
ORIGINAL_RUNS = [("original", "reproduce", 14), ("original-1", "reproduce", 1),
                 ("original-fix", "fix", 14)]  # fmt: skip
ORIGINAL_PANELS = [
    ("original", "larmorx", "LARMORX clean-room original", "reproduce mode, 14 threads"),
    ("original-1", "larmorx", "LARMORX clean-room original", "reproduce mode, 1 thread"),
    ("original-fix", "larmorx", "LARMORX clean-room original",
     "default mode (D13 corrections), 14 threads"),
]  # fmt: skip
#: The clean-room original's random-testing trial, and its triage (findings 36: every finding in
#: one category, ``validation/investigations/cleanroom-topup/triage_counts.py``).
TRIAL_ORIG = "2026-11-03-topup-original"
TRIAGE = "validation/investigations/cleanroom-topup/trial_triage.json"
FINDINGS = "docs/findings/fsl-topup-original.md"
REGISTER = "docs/findings/upstream-defects.md"


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


def _real_source(case: str):
    """``(args, put_inputs, settings, program)`` of the record's case ``case``, as the record
    runs it."""
    fs = _suite()
    run = _real_run(case)
    node = fs._source_node()
    with tempfile.TemporaryDirectory(prefix="lx-nc-topup-ev-args_") as t:
        args = fs._prepare(run, Path(t), node)

    def put(d: Path) -> None:
        fs._prepare(run, d, node)

    return args, put, dict(run.settings), run.program


def _fmriprep_source(run: str):
    """``(args, put_inputs)`` of fMRIPrep's topup call of ``run``, as the record runs it."""
    fs = _suite()
    node = _node(run)

    def put(d: Path) -> None:
        fs._prepare_fmriprep(node, d)

    return fs.fmriprep_args(node), put


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


def refresh_replica(ctx: core.Context, why: str) -> None:
    """After a change of the replica's default mode: its default-mode runs of every example's
    entry already computed made again (in their working directories, with their inputs; an
    entry not computed yet is computed whole by the export), and its default mode on every case
    of the record computed again (the named entry); FSL's runs and the reproduce mode's are
    kept (the record shows the reproduce mode unchanged)."""
    fctx = ctx.figures_context()
    label, mode, extra = REPLICA_RUNS[2]
    sources = []
    for _i, run, _f in FMRIPREP:
        args, put = _fmriprep_source(run)
        sources.append((f"fmriprep-{run}", [*args, *extra], put, {}, "topup"))
    for case in REAL.values():
        args, put, settings, program = _real_source(case)
        sources.append((case.replace("/", "-"), args, put, settings, program))
    for name, args, put, settings, program in sources:
        e = fcache.Entry(fctx, TOOL, name, "")
        if not e.done:
            continue
        info = e.info()
        for f in (e.dir / label).iterdir():  # the outputs being made again
            f.unlink()
        _replica(e, label, mode, args, put, settings, program)
        fcache.merge_rerun(e, info, why)
    fcache.recompute(fctx, TOOL, "cases-default-mode", NAMED_TEXT, _compute_named)


NAMED_TEXT = (
    "The replica's default mode on every case of the record: its exit status and the entries "
    "it names"
)


def named_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(ctx.figures_context(), TOOL, "cases-default-mode", NAMED_TEXT,
                        _compute_named)  # fmt: skip


# ------------------------------------------------------------------ the clean-room original


def _orig():
    from larmorx_validation.parity import fsl_topup as orig

    return orig


def versions_original() -> dict[str, str]:
    """FSL's version and the clean-room original's."""
    import larmorx
    from larmorx_validation.parity import fsl_versions
    from larmorx_validation.parity.fsl_flirt_replica import fsldir

    return {
        "fsl": f"FSL {fsl_versions.version_of(fsldir())} (fsl-topup 2203.5; ~/fsl-6.0.7.23, "
        "never on the PATH)",
        "larmorx": f"larmorx {larmorx.__version__} (the clean-room original, crate larmorx-fsl, "
        "Apache-2.0)",
    }


def _original_run(
    e: fcache.Entry,
    label: str,
    args: list[str],
    put_inputs,
    settings: dict[str, str | None],
    mode: str,
    n: int,
    program: str = "topup",
) -> None:
    """The clean-room original on ``args`` in ``<entry>/<label>`` (it writes its outputs where
    FSL does: its working directory), as its record runs it; the inputs removed after."""
    import time

    from larmorx_validation.parity import defects

    orig = _orig()
    cwd = e.dir / label
    cwd.mkdir(exist_ok=True)
    put_inputs(cwd)
    given = [p for p in cwd.iterdir() if p.is_file()]
    e.ctx.log(f"  {e.tool}/{e.case}: {label}")
    t0 = time.perf_counter()
    p = orig.run_original(args, cwd=cwd, settings=settings, mode=mode, threads=n, program=program)
    seconds = round(time.perf_counter() - t0, 2)
    for f in given:
        f.unlink()
    env = {defects.ENV: mode, "OMP_NUM_THREADS": str(n), "LARMORX_IMPLEMENTATION": "original"}
    run = fcache.Run(label, [*orig.original_command(), "fsl", program, *args], env)
    run.exit, run.seconds, run.stderr_tail = p.code, seconds, p.stderr[-2000:]
    e.runs.append(run)
    (e.dir / f"{label}.log").write_text(
        f"$ {shlex.join(run.argv)}\n# env: {env}\n# exit {p.code}, {seconds} s\n--- stdout\n"
        f"{p.stdout}\n--- stderr\n{p.stderr}",
        encoding="utf-8",
    )
    e.extra.setdefault("named", {})[label] = defects.notices_in(p.stderr)
    if p.code != 0:
        raise RuntimeError(f"{label}: the original exited {p.code}: {p.stderr[-800:]}")


def _original_source(source: str):
    """``(args, put_inputs, settings, program)`` of an fMRIPrep run's call or a record case,
    as the records run them."""
    fs = _suite()
    if source.startswith("ds"):
        node = _node(source)

        def put(d: Path) -> None:
            fs._prepare_fmriprep(node, d)

        return fs.fmriprep_args(node), put, {}, "topup"
    run = _real_run(source)
    node = fs._source_node()
    with tempfile.TemporaryDirectory(prefix="lx-topup-ev-args_") as t:
        args = fs._prepare(run, Path(t), node)

    def put_real(d: Path) -> None:
        fs._prepare(run, d, node)

    return args, put_real, dict(run.settings), run.program


def _compute_original(source: str):
    def compute(e: fcache.Entry) -> None:
        args, put, settings, program = _original_source(source)
        for label, mode, n in ORIGINAL_RUNS:
            _original_run(e, label, args, put, settings, mode, n, program)
        what = (
            "fMRIPrep 25.2.6's topup node of " if source.startswith("ds") else "The record's case "
        )
        e.finish(
            versions=versions_original(),
            args=args,
            settings=f"{what}{source}, its inputs as the records prepare them: LARMORX's "
            "clean-room original as its record runs it (reproduce mode at 14 threads and at one, "
            "the default mode at 14); FSL 6.0.7.23 and the replica are the replica entry's runs "
            "of the same call (FSL through the oracle-output cache)",
        )

    return compute


def original_entry(ctx: core.Context, source: str) -> fcache.Entry:
    """The clean-room original's runs of an fMRIPrep run's call (``source`` a run) or a record
    case (``source`` a case)."""
    name = f"fmriprep-{source}" if source.startswith("ds") else source.replace("/", "-")
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"original-{name}",
        f"{source}: LARMORX's clean-room original, reproduce mode at 14 threads and at one, "
        "the default mode",
        _compute_original(source),
    )


def case_table_original() -> dict[str, tuple[str, object]]:
    """Every case of the clean-room original's suite (``parity.fsl_topup``: the replica suite's
    cases and the original's own permanent cases): id -> (description, its ``Run``: for a
    permanent case its applytopup run, else its topup run; None for fMRIPrep's calls)."""
    from larmorx_validation.difftest import permanent

    s = _orig().suite()
    permanent.attach(s)
    out: dict[str, tuple[str, object]] = {}
    for c in s.cases("full"):
        kind, payload, *_ = c.payload
        if kind == "fmriprep":
            run = None
        elif kind == "difftest":
            run = payload[1] if payload[1] is not None else payload[0]
        else:
            run = _suite()._all_cases()[payload][2]
        out.setdefault(c.id, (c.description, run))
    return out


def _differ(a: np.ndarray, b: np.ndarray) -> tuple[int, float]:
    """How many values differ (NaN equal to NaN) and the largest finite |a − b|."""
    if a.shape != b.shape:
        return int(max(a.size, b.size)), float("inf")
    x, y = a.astype(np.float64), b.astype(np.float64)
    same = (x == y) | (np.isnan(x) & np.isnan(y))
    n = int(np.count_nonzero(~same))
    d = np.abs(x - y)[~same]
    d = d[np.isfinite(d)]
    return n, float(d.max()) if d.size else (float("inf") if n else 0.0)


def _exact_files(a: Path, b: Path, before: set[str]) -> dict[str, dict]:
    """Every file FSL wrote in ``a`` against the original's in ``b``, value by value (images:
    how many values differ and the largest |Δ|; text files: whether they are the same, the run
    directory's name aside). ``--debug``'s files (each implementation's own intermediate values)
    and ``--profile``'s (the clock's durations) are left out, as the record leaves them out."""
    orig = _orig()
    out: dict[str, dict] = {}
    for name, pa in orig._files(a, before).items():
        if name.startswith("TopupDebug") or name.endswith(_suite().PROFILE_SUFFIX):
            continue
        if name.endswith((".img", ".img.gz")):
            continue  # an ANALYZE pair is read through its .hdr
        pb = b / name
        if not pb.is_file():
            out[name] = {"kind": "missing", "differ": 1, "max_abs": float("inf"), "values": 0}
            continue
        if orig._is_image(name):
            x, y = orig._data(orig._load(pa)), orig._data(orig._load(pb))
            n, mx = _differ(x, y)
            out[name] = {"kind": "image", "differ": n, "max_abs": mx, "values": int(x.size)}
        else:
            ta = pa.read_bytes().replace(str(a).encode(), b"{cwd}")
            tb = pb.read_bytes().replace(str(b).encode(), b"{cwd}")
            out[name] = {"kind": "text", "differ": int(ta != tb), "max_abs": 0.0, "values": 1}
    return out


def _measured(ms) -> dict[str, list]:
    """The record's metrics as ``name -> [value, threshold, higher is better]``."""
    return {m.name: [float(m.value), float(m.threshold), bool(m.higher_is_better)] for m in ms}


def _classify(case, kind: str, payload, status: str, want_band: bool) -> dict:
    """One case of the original's record run again as its suite runs it (FSL served from the
    oracle cache), recording on the way every file of FSL's run against the original's
    (reproduce mode), the record's metrics (§12) of the original and, where the record measures
    it (fMRIPrep's calls, the thread cases), of FSL's own run at another thread count against
    its run at one, and every default-mode run's exit status and named entries. A case the
    record did not pass runs only the original's default mode."""
    import threading

    from larmorx_validation.parity import defects
    from larmorx_validation.parity.harness import CheckList

    orig = _orig()
    fs = _suite()
    out: dict = {}
    if status != "pass":
        if kind != "run":
            return out
        run = fs._all_cases()[payload][2]
        if run.refused:
            return out
        with tempfile.TemporaryDirectory(prefix="lx-topup-ev-fix_") as t:
            d = Path(t)
            try:
                args = fs._prepare(run, d, fs._source_node())
            except Exception:  # an input the case cannot make: no default run
                return out
            p = orig.run_original(args, cwd=d, settings=dict(run.settings), mode=defects.FIX,
                                  program=run.program)  # fmt: skip
            line = orig._refusal(p.stderr)
            named = defects.notices_in(p.stderr)
            named += [n for n in re.findall(r"\b(FSL-\d+)\b", line) if n not in named]
            out["default"] = {"exit": p.code, "named": named}
        return out
    local = _RECORDED.setdefault(threading.get_ident(), {})
    local.clear()
    local["want_band"] = want_band
    checks = CheckList()
    try:
        orig.run_case(case, checks)
    except Exception as x:  # an Outcome: the case ends otherwise than in the record
        out["outcome"] = str(x)[:300]
    out["passed"] = all(c.passed for c in checks.checks)
    for k in ("files", "metrics", "band"):
        if k in local:
            out[k] = local[k]
    fixes = local.get("fix", [])
    if fixes:
        out["default"] = fixes[0]
    return out


#: Per worker thread (its identifier), what the wrapped suite functions record of the case it
#: runs.
_RECORDED: dict[int, dict] = {}


def _recording():
    """The suite's ``compare`` and ``run_original`` wrapped to record what :func:`_classify`
    keeps, and a function that restores them."""
    import threading

    from larmorx_validation.parity import defects

    orig = _orig()
    real_compare, real_run = orig.compare, orig.run_original

    def rec() -> dict:
        return _RECORDED.setdefault(threading.get_ident(), {})

    def compare(checks, oracle, ours, a, b, args, before, band=None, program="topup", **kw):
        r = rec()
        r["files"] = _exact_files(a, b, set(before))
        eff = orig.effective_args(a, args)
        fa = orig._files(a, set(before))
        mask = orig.brain_mask(a, eff, fa, program)
        got, _exact = orig.measures(a, b, eff, fa, mask, program)
        r["metrics"] = _measured(got)
        if band is not None and r.get("want_band"):
            banded = band()
            if banded is not None:
                bm, _ = orig.measures(a, banded[1], eff, fa, mask, program)
                r["band"] = {m.name: float(m.value) for m in bm}
        return real_compare(checks, oracle, ours, a, b, args, before, band, program=program, **kw)

    def run_original(args, *, cwd, settings, mode, threads=None, program="topup"):
        p = real_run(args, cwd=cwd, settings=settings, mode=mode, threads=threads, program=program)
        if mode == defects.FIX:
            line = orig._refusal(p.stderr)
            named = defects.notices_in(p.stderr)
            named += [n for n in re.findall(r"\b(FSL-\d+)\b", line) if n not in named]
            rec().setdefault("fix", []).append({"exit": p.code, "named": named})
        return p

    orig.compare, orig.run_original = compare, run_original

    def restore() -> None:
        orig.compare, orig.run_original = real_compare, real_run

    return restore


def _compute_cases_original(e: fcache.Entry) -> None:
    from concurrent.futures import ThreadPoolExecutor

    from larmorx_validation.difftest import permanent

    rec = records.load(e.ctx.repo, RECORD_ORIG)
    s = _orig().suite()
    permanent.attach(s)
    by_id = {c.id: c for c in s.cases("full")}
    fs = _suite()

    def one(r: dict) -> tuple[str, dict]:
        c = by_id[r["case"]]
        kind, payload, *_ = c.payload
        single = kind == "run" and fs._all_cases()[payload][2].single is not None
        return r["case"], _classify(c, kind, payload, r["status"], kind == "fmriprep" or single)

    jobs = int(os.environ.get("LARMORX_EVIDENCE_JOBS", "3"))
    restore = _recording()
    try:
        with ThreadPoolExecutor(jobs) as pool:
            done = dict(pool.map(one, rec.results))
    finally:
        restore()
    e.finish(
        versions=versions_original(),
        cases={k: done[k] for k in sorted(done)},
        settings="Every case of the clean-room original's record (fsl-topup) run again as its "
        "suite runs it, in fresh directories with the case's inputs (FSL 6.0.7.23 through the "
        "oracle-output cache: the record's runs served, the never-cached policies rerun; the "
        "original at 4 threads, as the record runs it): for each case the record passed, every "
        "file FSL writes against the original's (reproduce mode), value by value, the record's "
        "metrics (specs/topup.md §12) and, on fMRIPrep's calls and the thread cases, the same "
        "metrics of FSL's own run at another thread count against its run at one (served); for "
        "every case the original's default mode (its exit status and the register entries it "
        "names)",
    )


def cases_original_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        "cases-original",
        "Every case of the clean-room original's record: its default mode, and each passing "
        "case's files against FSL's, with FSL's own thread band where the record measures it",
        _compute_cases_original,
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


def _counted_cell(f: Path, other: Path) -> dict:
    """A clean-room original's file against FSL's: *identical* (header and values, as for the
    replica), else how many of its values differ (images) or *differs* (text files)."""
    if not other.is_file():
        return common.cell("–")
    data, head = _same_file(f, other)
    if data:
        return common.cell("identical" if head else "data identical")
    if ".nii" not in f.name:
        return common.cell("differs")
    n, _mx = _differ(common.load(str(f)), common.load(str(other)))
    size = int(np.prod(common.load(str(f)).shape))
    return common.cell(f"{n:,} of {size:,} values differ")


def _files_table(
    tid: str,
    fsl_dir: Path,
    columns: dict[str, Path | None],
    note: str,
    counted: dict[str, Path] | None = None,
) -> dict:
    """Every file the program wrote, FSL's against each run: the data and the header
    (``descrip``, FSL's build string, aside), byte for byte; for the clean-room original's runs
    (``counted``), how many values differ where they are not the same."""
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
        for d in (counted or {}).values():
            cells.append(_counted_cell(f, d / f.name))
        rows.append({"head": f"`{f.name}`", "cells": cells})
    extra = (
        " For the clean-room original, where the values are not the same, how many of the "
        "file's values differ from FSL's (its record holds it to specs/topup.md §12's "
        "thresholds; the comparisons above give the sizes)."
        if counted
        else ""
    )
    return common.table(
        tid,
        "Every file written, against FSL 6.0.7.23's at one thread",
        ["File", "Type", "Size", *columns, *(counted or {})],
        rows,
        note="Each image decompressed and compared byte for byte with FSL's: *identical*, the "
        "header (its `descrip`, FSL's build string, aside) and every value; *data identical*, "
        "every value but not the header; *differs*, values that differ (see the comparisons "
        "above); text files byte for byte; – where the run keeps no such file. " + note + extra,
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
    "wraps a negative index along a line by 2³² modulo the line's length; the default wraps "
    "it around the line.",
    "FSL-142": "FSL-142, corrected by default with a notice: vb3D and vb4D count two data "
    "points per line whatever the collection holds; the default counts every image's.",
    "FSL-143": "FSL-143, corrected by default with a notice: vb3D's and vb4D's prior couples "
    "neighbouring slices with half the weight its own construction gives them, so the "
    "precision it builds is not positive definite and the iterations diverge (FSL's output "
    "here spans about ±4 × 10¹⁰ for an input of 0 to 1.1 × 10⁵); the default gives the "
    "prior its full weight, and the restoration stays in the input's range, as vb2D's does.",
    "FSL-144": "FSL-144, corrected by default with a notice: applytopup --method=jac with a "
    "phase encoding along z, which topup does not estimate, leaves the image uncorrected "
    "without a word; the default refuses the call with the reason. The reproduce mode gives "
    "FSL's bytes.",
    "FSL-145": "FSL-145, corrected by default with a notice: with --splineorder=2, an axis "
    "shorter than two knot spacings gets two coefficients, or one, which force the field "
    "towards 0 at its far end; the default lays the general count. The reproduce mode gives "
    "FSL's bytes.",
    "FSL-146": "FSL-146, corrected by default with a notice: with volumes encoded along "
    "different axes, FSL's gradient leaves out of each axis' term the volumes not encoded "
    "along it, so it is not the cost's gradient; the default minimises with the cost's own "
    "gradient. The reproduce mode gives FSL's bytes.",
    "FSL-201": "FSL-201, corrected by default with a notice (since 2026-11-03): with the "
    "movements estimated, FSL's derivatives of the cost with respect to the rotations are "
    "forward differences (steps of 10⁻⁵ rad) of rotation matrices built from a "
    "single-precision angle, cosine and sine, so the rounding divided by the step makes its "
    "gradient and its Gauss-Newton Hessian of the movements other than the cost's (their "
    "rotation entries 0.25 to 39 % off on ds005040's pair at half resolution) and the "
    "minimisers end elsewhere than the cost's minimum; the default computes the exact "
    "derivatives. Measured on 22 runs that estimate the movements, the default's final cost "
    "was lower or equal in 18 (the other four, runs cut short, 0.004 to 0.13 % higher), lower "
    "in all 8 random cases whose volumes moved (by 0.01 to 44 %), and 38 % lower after 60 "
    "iterations of scaled conjugate gradients on the half-resolution pair, where FSL's stall; "
    "fMRIPrep's calls estimate no movements and stay the same. The reproduce mode gives FSL's "
    "bytes.",
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


def _diffs_original(fmriprep: bool) -> list[dict]:
    d = common.d
    out = [d("original-fsl", "original", "fsl", "main", "clean-room original − FSL 6.0.7.23",
             common.ROLE_LX)]  # fmt: skip
    if fmriprep:
        out.append(d("original-own", "original", "own", "fmriprep",
                     "clean-room original − fMRIPrep's own output", common.ROLE_FMRIPREP))  # fmt: skip
    out += [
        d("original1-original", "original-1", "original", "check",
          "clean-room original, 1 thread − 14 threads", common.ROLE_SELF),
        d("ofix-original", "original-fix", "original", "check",
          "clean-room original, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    return out


def _worse(m: list, band: float) -> bool:
    """Whether the original's metric ``m`` (value, threshold, higher is better) is further
    from FSL than FSL's own band value."""
    v, _t, hib = m
    return v < band if hib else v > band


def _beyond_band(c: dict) -> list[str]:
    """The record's metrics of a classified case in which the original is further from FSL
    than FSL's own run at another thread count is."""
    band = c.get("band") or {}
    return [k for k, m in c.get("metrics", {}).items() if k in band and _worse(m, band[k])]


def _ranked(c: dict) -> list[str]:
    """The record's metrics of a classified case, the closest to its threshold first."""

    def share(m: list) -> float:
        v, t, hib = m
        if hib:
            return (1.0 - v) / max(1.0 - t, 1e-300)
        return v / t if t > 0 else (0.0 if v == 0 else float("inf"))

    ms = c.get("metrics", {})
    return sorted(ms, key=lambda k: -share(ms[k]))


def _metric_words(c: dict, names: list[str] | None = None, n: int = 3) -> str:
    """The original's values of the record's metrics ``names`` (default: the closest to their
    thresholds), each with its threshold and FSL's own band value where measured."""
    ms, band = c.get("metrics", {}), c.get("band") or {}
    parts = []
    for k in (names if names is not None else _ranked(c))[:n]:
        v, t, hib = ms[k]
        f = ".6g" if hib else ".3g"  # a correlation near 1: its digits
        s = f"{k} {v:{f}} (§12: {t:.3g}"
        if k in band:
            s += f"; FSL's own {band[k]:{f}}"
        parts.append(s + ")")
    return "; ".join(parts)


def _worst_file(files: dict[str, dict]) -> str:
    k, x = max(files.items(), key=lambda kv: (kv[1]["kind"] == "image", kv[1]["differ"]))
    if x["kind"] != "image":
        return f"`{k}`"
    return f"`{k}`: {x['differ']:,} of {x['values']:,} values, at most {x['max_abs']:.3g}"


def original_verdict(c: dict, what: str) -> tuple[str, str]:
    """The clean-room original's verdict on a case the record passed, from its classification
    (``cases-original``): every file's values against FSL's, the record's metrics (§12) and
    FSL's own thread band where the record measures it."""
    files = c.get("files")
    if files is None:
        return "open", "Not classified (the case's run again did not reach its comparison)."
    diff = {k: x for k, x in files.items() if x["differ"]}
    if not diff:
        if not files:
            return "bit-identical", (
                f"FSL writes no file on {what}; the exit status and messages are FSL's."
            )
        return "bit-identical", (
            f"The same values, every one, in all {len(files)} files FSL writes on {what}, with "
            "FSL's exit status and messages (the record checks the headers and the log)."
        )
    if c.get("band"):
        beyond = _beyond_band(c)
        if not beyond:
            return "within-variability", (
                "In every metric of the record (specs/topup.md §12, inside the record's brain) "
                "the clean-room original is no further from FSL at one thread than FSL at "
                f"another thread count is (its own thread band, FSL-116): {_metric_words(c)}."
            )
        return "open", (
            "Within the record's thresholds (specs/topup.md §12), but further from FSL than "
            f"FSL's own thread band in {len(beyond)} metric(s): {_metric_words(c, beyond)}."
        )
    return "open", (
        f"Within the record's thresholds (specs/topup.md §12): {_metric_words(c)}. "
        f"{len(diff)} of {len(files)} files differ in their values (most: {_worst_file(diff)}): "
        "the clean-room original computes in its own order of operations (written from the "
        "specification, not from FSL's code). FSL's own band is not measured on this case (its "
        "thread band is, on fMRIPrep's calls)."
        if c.get("metrics")
        else f"{len(diff)} of {len(files)} files differ in their values (most: "
        f"{_worst_file(diff)}); the record's exact checks pass."
    )


#: The corrections the clean-room original's default mode names, as it makes them.
ORIGINAL_FIX = {
    k: v.replace("The reproduce mode gives FSL's bytes.", "Its reproduce mode follows FSL.")
    .replace(" (the final cost is never higher: −0.00 to −1.66 % on the measured cases)", "")
    .replace("the replica's default", "the default")
    .replace("the replica gives", "the original gives")
    for k, v in FIX_REASONS.items()
}
# the clean-room original computes the exact rotation derivatives in both modes (no notice)
ORIGINAL_FIX.pop("FSL-201")
ORIGINAL_FIX["FSL-145"] = (
    "FSL-145, corrected by default with a notice: with --splineorder=2, an axis shorter than two "
    "knot spacings gets two coefficients, which force the field towards 0 at its far end; the "
    "default lays the general count. Its reproduce mode lays FSL's two."
)
ORIGINAL_FIX["FSL-146"] = (
    "FSL-146, corrected by default with a notice: with volumes encoded along different axes, "
    "FSL's gradient leaves out of each axis' term the volumes not encoded along it, so it is not "
    "the cost's gradient; the default minimises with the cost's own gradient. Its reproduce mode "
    "follows FSL's."
)


def _verdicts_original(diffs: list[dict], same: dict[str, bool], c: dict, info: dict,
                       what: str) -> None:  # fmt: skip
    """The clean-room original's comparisons: against FSL (from the case's classification), with
    itself (threads, modes)."""
    named = [n for n in info.get("named", {}).get("original-fix", []) if n.startswith("FSL-")]
    verdict, why = original_verdict(c, what)
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if i == "original-fsl":
            x["verdict"], x["reason"] = verdict, why
            if verdict != "bit-identical":
                x["links"].append({"label": "findings", "href": FINDINGS})
            if verdict == "within-variability":
                x["links"].append({"label": "FSL-116", "href": REGISTER})
        elif i == "original-own":
            if same.get("replica-own"):
                x["verdict"] = verdict
                x["reason"] = "fMRIPrep's own files are FSL's (the replica's row): " + (
                    why[:1].lower() + why[1:]
                )
            else:
                x["verdict"] = "open"
                x["reason"] = "fMRIPrep's own output is not FSL's here (see the replica's row)."
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "original1-original": "The thread count changes nothing: the clean-room "
                "original's parallel loops (fixed blocks of lines, planes and coefficients, "
                "sums in a fixed order) give its one-thread result bit for bit at 14 threads.",
                "ofix-original": "No correction of the register applies to this call: both "
                "modes give the same values."
                if not named
                else "No correction of the register changes this call's values: the default "
                f"mode names {', '.join(named)}, with the same values in both modes.",
            }.get(i, "The same values, every one.")
        elif i == "ofix-original" and named:
            x["verdict"] = "d13"
            x["reason"] = " ".join(ORIGINAL_FIX.get(n, f"{n}, corrected by default.")
                                   for n in named)  # fmt: skip
            x["links"] += [{"label": n, "href": REGISTER} for n in named]
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example (see the record)."


def _band_table_original(tid: str, c: dict) -> dict:
    """The record's metrics (§12) of fMRIPrep's call: the clean-room original against FSL at
    one thread, beside FSL's own thread band (FSL at --nthr=4 against one) and the threshold."""
    ms, band = c.get("metrics", {}), c.get("band") or {}
    rows = []
    for k in ms:
        v, t, hib = ms[k]
        b = band.get(k)
        where = (
            "–"
            if b is None
            else "within FSL's band"
            if not _worse(ms[k], b)
            else "within the threshold, beyond FSL's band"
            if (v >= t if hib else v <= t)
            else "beyond both"
        )
        f = "sig6" if hib else "sig"  # a correlation near 1: its digits
        rows.append({
            "head": k,
            "cells": [common.cell(v, f), common.cell(b, f), common.cell(t, "sig"),
                      common.cell(where)],
        })  # fmt: skip
    return common.table(
        tid,
        "The clean-room original against FSL's own thread band: the record's metrics",
        [
            "Metric (specs/topup.md §12)",
            {"label": "Original − FSL", "num": True},
            {"label": "FSL --nthr=4 − FSL", "num": True},
            {"label": "§12's threshold", "num": True},
            "",
        ],
        rows,
        note="Each metric as the record computes it (parity.fsl_topup's `measures`), over the "
        "record's brain (FSL's corrected mean above 10 % of its 99th percentile): the clean-room "
        "original (reproduce mode, 4 threads; its result is the same at any thread count) "
        "against FSL 6.0.7.23 at one thread, and FSL at --nthr=4 against FSL at one (its own "
        "thread band, FSL-116; the run the replica's evidence made, served by the oracle "
        "cache); a correlation is better when higher, every other metric when lower.",
    )


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
        "decompressed, `descrip` left out), with FSL's exit status and messages."
        + extra
        + " The clean-room original's record (fsl-topup) holds it to specs/topup.md §12's "
        "thresholds instead: FSL's exit status, files and messages (numbers to 4 significant "
        "digits), the headers and the log identical; inside the record's brain the field within "
        "0.5 Hz at the 99th percentile and 0.1 Hz on average (correlation at least 0.999), the "
        "corrected images within 1e-3 relative at the 99th percentile and 1e-3 of the mean "
        "intensity on average, the movements within 0.05 mm and 0.001 rad, the Jacobians and "
        "displacements within the field's bounds times the read-out time; a metric beyond its "
        "threshold passes within FSL's own band where the record measures it."
    )


def _programs(info: dict, oinfo: dict | None = None) -> str:
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
    extra = ""
    if oinfo is not None:
        extra = (
            f"; {oinfo.get('versions', {}).get('larmorx', '?')}, from larmorx at commit "
            f"`{oinfo['larmorx_commit'][:12]}`"
        )
    return (
        f"{vers.get('fsl', '?')} ({how}{four}); {vers.get('larmorx-nc', '?')} (the replica), "
        f"from larmorx at commit `{info['larmorx_commit'][:12]}`" + extra
    )


def _provenance(info: dict, e: fcache.Entry, oinfo: dict, o: fcache.Entry, data: list) -> list:
    """Both entries' provenance: the replica's and FSL's runs, and the original's."""
    first = common.provenance(info, f"{TOOL}/{e.case}", data)
    second = common.provenance(oinfo, f"{TOOL}/{o.case}", data)
    return first + [line for line in second if line.startswith("Runs:")]


def _show(mask: np.ndarray, iterations: int) -> np.ndarray:
    from scipy import ndimage

    return ndimage.binary_dilation(mask > 0, iterations=iterations)


def _example_fmriprep(
    ctx: core.Context, ex_id: str, run: str, first: str, classified: dict[str, dict]
) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    fs = _suite()
    e = fmriprep_entry(ctx, run)
    info = e.info()
    o = original_entry(ctx, run)
    oinfo = o.info()
    c = classified.get(f"fmriprep/{run}", {})
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
    arrays.update({pid: _arrays(o.dir / pid, outs) for pid, *_ in ORIGINAL_PANELS})
    for pid, role, label, sub in [*PANELS, *ORIGINAL_PANELS]:
        ex.add(exm.Panel(pid, role, label.format(program="topup"), sub, arrays[pid]))
    own_dirs = {k: v for k, v in outs.items() if (node / v[0]).is_file()}
    own = _arrays(node, own_dirs)
    rdiffs, odiffs = _diffs(True), _diffs_original(True)
    diffs = rdiffs + odiffs
    outputs = [{"id": k, "label": v[2], "unit": v[3]} for k, v in outs.items()]
    common.compare_all(ex, diffs, outputs, full={"own": own})
    same = common.identical(diffs)
    _verdicts(rdiffs, same, info, "topup")
    _verdicts_original(odiffs, same, c, oinfo, "this call")
    at, _k = common.largest_in_slice(ex, "original", "fsl", first, "axial")
    default = {
        "output": first,
        "plane": "axial",
        "voxel": at,
        "panels": ["in1", "in2", "fsl", "replica", "original"],
        "diffs": ["replica-fsl", "original-fsl", "fsl4-fsl"],
        "compare": {"a": "fsl", "b": "original"},
    }
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="fsl",
        caption="the crosshair where the clean-room original differs most from FSL at one "
        "thread in this slice, beside FSL at four threads against one, its own band, on the "
        "same scale (the replica's output equals FSL's everywhere).",
    )  # fmt: skip
    cols = {
        "replica (reproduce)": e.dir / "replica",
        "replica --nthr=4": e.dir / "replica-4",
        "replica (default)": e.dir / "replica-fix",
        "FSL --nthr=4": e.dir / "fsl-4",
        "fMRIPrep's own": node,
    }
    counted = {"original (reproduce)": o.dir / "original",
               "original (default)": o.dir / "original-fix"}  # fmt: skip
    tables = [
        _files_table(
            f"{ex_id}-files",
            e.dir / "fsl",
            cols,
            "fMRIPrep's own files are the node's outputs in the run's working directory (topup "
            "in fMRIPrep 25.2.6's image); it keeps no log or movement parameters. The log "
            "records the options as given, so the --nthr=4 runs' logs differ by that option.",
            counted,
        ),
        _band_table(f"{ex_id}-band", arrays["fsl-4"], arrays["fsl"], mask, outs),
    ]
    if c.get("metrics"):
        tables.append(_band_table_original(f"{ex_id}-original-band", c))
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
        "thread (as fMRIPrep runs it) and at four, to the larmorx-nc replica in its "
        "reproduce and default modes and at four threads, and to LARMORX's clean-room original "
        "in its reproduce and default modes at 14 threads and at one. The two input panels are "
        "the pair before the correction, distorted in opposite directions along the phase "
        "encoding; choose an output above the panels: each volume corrected, the field in Hz, "
        "each volume's Jacobian and its displacement along the phase encoding. Below the "
        "viewer, every file topup writes against FSL's, with fMRIPrep's own, and the record's "
        "metrics of the clean-room original beside FSL's own thread band."
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
            {"label": "Programs", "text": _programs(info, oinfo)},
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
        "provenance": _provenance(info, e, oinfo, o, [data]),
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
    "movements": (
        "Movements estimated, and a correction by default",
        "The record's case `opt/levels`: two levels on the half-resolution pair of ds005040, "
        "the first at subsampling 2 by Levenberg-Marquardt with the movements estimated (the "
        "second volume's rigid-body movement against the first; fMRIPrep's configuration "
        "estimates none, FSL's own b02b0.cnf does in its first five levels), the second at full "
        "resolution by scaled conjugate gradients; every output written. FSL 6.0.7.23 and the "
        "replica's reproduce mode give the same bytes; since 2026-11-03 the replica's default "
        "corrects FSL-201 (FSL's rotation derivatives are not its cost's) and so takes other "
        "steps: the difference *replica, default − reproduce mode* shows what that changes, and "
        "the movement table below the viewer the movements each run estimated. The clean-room "
        "original computes the cost's own derivatives in both modes, so its record holds this "
        "case as an expected divergence from FSL.",
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


def _movements_table(tid: str, runs: dict[str, Path], second: str, name: str) -> dict:
    """The movements each run estimated (``--out``'s ``_movpar.txt``: per volume, the
    translations in mm and the rotations in radians), every moving volume, against FSL's (the
    first run) and against the run ``second`` (``name`` in the columns' labels)."""

    def read(d: Path) -> np.ndarray | None:
        f = next(iter(sorted(d.glob("*_movpar.txt"))), None)
        if f is None:
            return None
        return np.array([[float(v) for v in ln.split()] for ln in
                         f.read_text(encoding="utf-8").splitlines() if ln.strip()])  # fmt: skip

    ref, ref2 = read(next(iter(runs.values()))), read(runs[second])
    rows = []
    for head, d in runs.items():
        m = read(d)
        if m is None:
            continue
        for k in range(1, m.shape[0]):
            diffs = [float(np.max(np.abs(m[k, a:b] - r[k, a:b])))
                     for r in (ref, ref2) for a, b in ((0, 3), (3, 6))]  # fmt: skip
            rows.append({
                "head": head if m.shape[0] == 2 else f"{head}, volume {k + 1}",
                "cells": [*(common.cell(float(v), "sig6") for v in m[k, :6]),
                          *(common.cell(x, "sig") for x in diffs)],
            })  # fmt: skip
    return common.table(
        tid,
        "The movements estimated: the moving volume against the first",
        ["Run", *[{"label": f"{a} (mm)", "num": True} for a in ("x", "y", "z")],
         *[{"label": f"rotation {a} (rad)", "num": True} for a in ("x", "y", "z")],
         {"label": "|Δ| translation vs FSL", "num": True},
         {"label": "|Δ| rotation vs FSL", "num": True},
         {"label": f"|Δ| translation vs {name}", "num": True},
         {"label": f"|Δ| rotation vs {name}", "num": True}],
        rows,
        note="Each run's `--out` movement file (`_movpar.txt`: one line per volume, the first "
        "volume the reference), as written; the last four columns the largest difference of "
        f"the translations (mm) and of the rotations (rad) from FSL's and from {name}'s.",
    )  # fmt: skip


#: The derived inputs' labels: file name -> (label, sub).
INPUT_TEXT = {
    "half.nii.gz": "ds005040 pair, half resolution",
    "half_up.nii.gz": "ds005040, +y image, half resolution",
    "half_dn.nii.gz": "ds005040, −y image, half resolution",
}


def _example_real(ctx: core.Context, ex_id: str, classified: dict[str, dict]) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    fs = _suite()
    case = REAL[ex_id]
    e = real_entry(ctx, case)
    info = e.info()
    o = original_entry(ctx, case)
    oinfo = o.info()
    c = classified.get(case, {})
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
    arrays.update({pid: _arrays(o.dir / pid, outs) for pid, *_ in ORIGINAL_PANELS})
    for pid, role, label, sub in [*panels, *ORIGINAL_PANELS]:
        sub = sub.replace(" (the record's run, from the oracle cache)", ", the record's run")
        ex.add(exm.Panel(pid, role, label.format(program=run.program), sub, arrays[pid]))
    rdiffs, odiffs = _diffs(False), _diffs_original(False)
    diffs = rdiffs + odiffs
    outputs = [{"id": k, "label": v[2], "unit": v[3]} for k, v in outs.items()]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    _verdicts(rdiffs, same, info, run.program)
    _verdicts_original(odiffs, same, c, oinfo, "this case")
    # a case the original's record does not pass (an expected divergence): the record's verdict
    row = next((r for r in records.load(ctx.repo, RECORD_ORIG).results if r["case"] == case), None)
    held = common.outcome_verdict(row) if row is not None else None
    if held is not None:
        for x in odiffs:
            if x["id"] == "original-fsl":
                x["verdict"], x["reason"] = held
                x["links"] = [{"label": "record", "href": f"docs/validation/{RECORD_ORIG}.md"},
                              {"label": "findings", "href": FINDINGS}]  # fmt: skip
                if "FSL-201" in (info.get("named", {}).get("replica-fix") or []):
                    x["reason"] = common.clip(
                        x["reason"].rstrip(".") + ". Since 2026-11-03 the specification states "
                        "what FSL computes (FSL-201, specs/topup.md §4.6): the original's "
                        "derivatives are the cost's, as the replica's default mode now computes "
                        "them; its reproduce mode is to follow FSL's.",
                        700,
                    )
    out_id = next(iter(outs))
    at, _k = common.largest_in_slice(ex, "replica-fix", "replica", out_id, "axial")
    default = {
        "output": out_id,
        "plane": "axial",
        "voxel": at,
        "panels": [*[p.id for p in ex.panels if p.role == "input"][:3], "fsl", "original",
                   "replica-fix"],
        # the comparisons as before the original joined: its difference from FSL (larger than
        # the correction's) would set their shared scale and hide the correction
        "diffs": ["replica-fsl", "fix-replica"],
        "compare": {"a": "replica-fix", "b": "replica"},
    }  # fmt: skip
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default,
        window_panel="replica-fix" if ex_id == "vb3d" else "fsl",
        caption="the crosshair where the default mode differs most from the reproduce mode "
        "(FSL's bytes) in this slice.",
    )  # fmt: skip
    cols = {"replica (reproduce)": e.dir / "replica", "replica (default)": e.dir / "replica-fix"}
    counted = {"original (reproduce)": o.dir / "original",
               "original (default)": o.dir / "original-fix"}  # fmt: skip
    tables = [_files_table(f"{ex_id}-files", e.dir / "fsl", cols, "", counted)]
    if ex_id == "movements":
        runs = {"FSL 6.0.7.23": e.dir / "fsl", **cols, **counted}
        tables.append(
            _movements_table(
                f"{ex_id}-movements", runs, "replica (default)", "the replica's default"
            )
        )
    title, summary = REAL_TEXT[ex_id]
    summary += (
        " LARMORX's clean-room original runs the same call in its reproduce mode (at 14 threads "
        "and at one) and its default mode"
        + (", the same derivatives in both." if ex_id == "movements" else
           ", which makes the same correction.")
    )  # fmt: skip
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
            {"label": "Case", "text": f"`{case}` of the records (`{RECORD}`, `{RECORD_ORIG}`)"},
            {"label": "Command", "text": "`" + command(case, case_table()) + "`"},
            {"label": "Programs", "text": _programs(info, oinfo)},
            {"label": "Grid", "text": common.describe_grid(first, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: threads and modes"],
        ],
        "tables": tables,
        "metrics_note": _note(int(np.prod(first.shape[:3]))),
        "provenance": _provenance(info, e, oinfo, o, [data]),
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


def case_verdict_original(r: dict, c: dict) -> tuple[str, str]:
    """A case's verdict for the clean-room original: the record holds it to specs/topup.md
    §12's thresholds; a passing case is classified from its files against FSL's and FSL's own
    thread band where measured (``c``: the case in the ``cases-original`` entry, with the
    original's default mode on it)."""
    dm = c.get("default", {})
    named, code = dm.get("named", []), dm.get("exit")
    v = common.outcome_verdict(r)
    if v is not None:
        verdict, why = v
        if verdict == "both-reject" and named:
            why += (
                f" The clean-room original's default mode finishes instead (exit status 0), "
                f"naming {', '.join(named)}."
                if code == 0
                else f" The clean-room original's default mode ends with exit status {code}, "
                f"naming {', '.join(named)}."
            )
        return verdict, common.clip(why, 700)
    verdict, why = original_verdict(c, "this case")
    fix = r.get("reported", {}).get("D13 fix pass")
    notes = []
    if isinstance(fix, int) and fix:
        notes.append(
            f"in its default mode the clean-room original corrects "
            f"{', '.join(named) or 'a register entry'} and names it: {fix:,} values change"
        )
    elif named and code not in (0, None):
        notes.append(f"its default mode refuses the call (exit {code}), naming {', '.join(named)}")
    elif named:
        notes.append(f"its default mode names {', '.join(named)}, with the same values")
    if notes:
        why = why.rstrip(".") + "; " + "; ".join(notes) + "."
    return verdict, common.clip(why, 700)


def add_original(cs: list[dict], rec, examples: dict[str, str], classified: dict) -> None:
    """The clean-room original's results joined to the cases (a case of its own where the
    replica's record has none: its permanent cases)."""
    defs = case_table_original()
    by = {c["id"]: c for c in cs}
    for r in rec.results:
        v, why = case_verdict_original(r, classified.get(r["case"], {}))
        result = {
            "outcome": r["status"],
            "verdict": v,
            "checks": f"{r['checks_passed']}/{r['checks_total']}",
            "reason": why,
        }
        if r["case"] in by:
            by[r["case"]]["results"]["original"] = result
            continue
        cs.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": defs.get(r["case"], ("", None))[0],
                "command": command(r["case"], defs),
                "results": {"original": result},
                **({"example": examples[r["case"]]} if r["case"] in examples else {}),
            }
        )


# ------------------------------------------------------------------------------- speed

#: The benchmark's series (``docs/benchmarks/fsl-topup.json``, from
#: ``validation/investigations/cleanroom-topup/bench.py``).
SPEED_SERIES = [
    {"id": "FSL 6.0.7.23|1 thread", "label": "FSL 6.0.7.23, one thread (as fMRIPrep runs it)",
     "mark": "original"},
    {"id": "FSL 6.0.7.23|14 threads", "label": "FSL 6.0.7.23, --nthr=14", "mark": "original-open"},
    {"id": "replica (larmorx-nc)|1 thread", "label": "replica (one thread)", "mark": "replica"},
    {"id": "original (larmorx)|1 thread", "label": "clean-room original, 1 thread",
     "mark": "larmorx"},
    {"id": "original (larmorx)|14 threads", "label": "clean-room original, 14 threads",
     "mark": "larmorx-open"},
]  # fmt: skip
SPEED_LABELS = {
    "fmriprep/ds005040-sub-002-noreconall": "fMRIPrep's topup call: ds005040's pair",
    "fmriprep/ds001600-sub-1-noreconall": "fMRIPrep's topup call: ds001600's pair",
    "applytopup/lsr ds005040": "applytopup --method=lsr: ds005040's pair, FSL's topup result",
}


def speed(ctx: core.Context) -> tuple[list[dict], dict | None]:
    """The speed chart from the benchmark report (one run each, informal: its own format)."""
    import types

    p = ctx.repo / "docs" / "benchmarks" / f"{TOOL}.json"
    if not p.is_file():
        return [], None
    data = json.loads(p.read_text(encoding="utf-8"))
    md = p.with_suffix(".md").read_text(encoding="utf-8")
    m = re.search(r"the main machine: ([^,]+),\s+(\d+)\s+cores\s+/\s+(\d+)\s+threads", md)
    cpu, cpus = (m.group(1), int(m.group(3))) if m else ("?", "?")
    results = []
    for row in data["rows"]:
        call = row["call"]
        for t, sec in row["fsl_seconds"].items():
            op = "1 thread" if t == "1" else f"{t} threads"
            results.append({"tool": "FSL 6.0.7.23", "operation": op, "input": call,
                            "median": sec})  # fmt: skip
        results.append({"tool": "replica (larmorx-nc)", "operation": "1 thread", "input": call,
                        "median": row["replica_seconds"]})  # fmt: skip
        for t, sec in row["original_seconds"].items():
            op = "1 thread" if t == "1" else f"{t} threads"
            results.append({"tool": "original (larmorx)", "operation": op, "input": call,
                            "median": sec})  # fmt: skip
    bench = types.SimpleNamespace(
        results=results, data={"environment": {"cpu": cpu, "logical_cpus": cpus}}
    )
    chart = common.speed(
        bench,
        SPEED_SERIES,
        SPEED_LABELS,
        title="Seconds for the whole call (read the images, estimate or restore, write every "
        "output), one run each, log scale; the factor is the clean-room original's speed-up "
        "over FSL, both at one thread",
        base=(SPEED_SERIES[0]["id"], SPEED_SERIES[3]["id"]),
        note="From the benchmark report `docs/benchmarks/fsl-topup.json` ({cpu}, {cpus} logical "
        "CPUs, shared with another agent's work: informal, one run each). The replica's files "
        "are FSL's one-thread files byte for byte; the clean-room original's are within its "
        "record's thresholds and the same bits at 1 and 14 threads. FSL at --nthr=14 changes its "
        "result (FSL-116).",
        inputs=[row["call"] for row in data["rows"]],
    )
    source = [
        {
            "path": p.relative_to(ctx.repo).as_posix(),
            "sha256": hashlib.sha256(p.read_bytes()).hexdigest(),
            "larmorx_commit": data.get("commit"),
            "cpu": cpu,
        }
    ]
    return source, chart


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


#: The triage's categories (findings 36, ``triage_counts.py``): the trial's category -> (what
#: the page says, the verdict of the row or None, the reproducer the findings name).
TRIAGE_TEXT: dict[str, tuple[str, str | None, int]] = {
    "explained: finding 14 (FSL's movement derivatives)": (
        "explained: FSL's movement derivatives are not its cost's (findings 14; the record's "
        "expected divergences), in cases that estimate the movements; registered since "
        "2026-11-03 as FSL-201 (rule A), the exact derivatives the default of both "
        "implementations, the original's reproduce mode to follow FSL's", None, 6),
    "FSL-146 (rule A; reproduce agrees) and, with movements, finding 14": (
        "a defect of FSL, registered: FSL-146, volumes encoded along different axes (rule A: "
        "corrected by default; the reproduce mode now gives FSL's path), with movements also "
        "findings 14", "d13", 10280),
    "explained: FSL-133 (lines whose length does not divide 2^32)": (
        "explained: FSL-133, applytopup's lines whose length does not divide 2^32 (FSL's wrap, "
        "not reproduced; the specification states it since 2026-11-03)", None, 5),
    "larmorx mistake, fixed: the quadratic B-splines' count": (
        "a LARMORX mistake, fixed: the quadratic B-splines' count (a permanent case; FSL-145 "
        "found with it)", None, 11095),
    "harness, fixed: the transposed acquisition file": (
        "the suite's own mistake, fixed: an acquisition file of four rows", None, 27),
    "fixed in part (the SCG weight) / explained: the path of ill-determined fits": (
        "in part a LARMORX convention, fixed (the scaled conjugate gradients' weight, a "
        "permanent case); the rest explained: other paths of fits the data hardly determine, "
        "some just past a threshold", None, 12118),
    "harness, fixed: outputs named by a configuration file": (
        "the suite's own mistake, fixed: outputs named by a configuration file", None, 12),
    "explained: FSL-143 (vb3D/vb4D's indefinite prior, reproduced; its divergence)": (
        "explained: FSL-143, vb3D's and vb4D's indefinite prior (reproduced: its divergence "
        "amplifies last bits)", None, 275),
    "open: quadratic splines' regulariser Hessian (k even)": (
        "open: the quadratic splines' regulariser Hessian at an even knot spacing (the "
        "specification's answer of 2026-11-03, §4.6, for the original's next part)", "open",
        10753),
    "harness, fixed: --trace with -v": (
        "the suite's own mistake, fixed: --trace with -v", None, 1142),
    "explained: --debug's attempt files (the minimisation's path)": (
        "explained: --debug's attempt files (how many Levenberg-Marquardt attempts a level "
        "makes: the minimisation's path)", None, 2150),
    "open: applytopup lsr on lines that divide 2^32": (
        "open: applytopup --method=lsr on lines whose length divides 2^32 (99th percentile "
        "1.4e-3 to 0.13; the specification's answer of 2026-11-03, §7.4, for the original's next "
        "part)", "open", 8089),
    "explained: integer outputs (float values a rounding apart land on neighbouring integers)": (
        "explained: integer outputs (values a rounding apart land on neighbouring integers)",
        None, 7321),
    "explained: vb2D's transients (the record's at/vb2d-aniso)": (
        "explained: vb2D's transients (as the record's at/vb2d-aniso)", None, 13346),
}  # fmt: skip


def trial_original(ctx: core.Context) -> tuple[list[dict], dict]:
    """The clean-room original's random-testing trial (its ``summary.json``) and its triage
    (``trial_triage.json``): two tables, and their provenance."""
    path = common.oracles() / "difftest" / TRIAL_ORIG / "summary.json"
    raw = path.read_bytes()
    s = json.loads(raw)
    t = s["tools"]["fsl-topup-original"]
    o = t["options"]
    impl = t["implementations"]["original"]
    rows = [
        ("Random cases, each compared with FSL 6.0.7.23", t["cases"]),
        ("within the record's thresholds (§12), every exact check passed", impl.get("agree", 0)),
        ("differing (each one triaged below)", impl.get("differ", 0)),
        ("fMRIPrep's call (sdcflows' b02b0.cnf, every output)", o.get("fMRIPrep's call", 0)),
        ("a configuration file (`--config`)", o.get("--config", 0)),
        ("one, two or three levels", [o.get(f"{n} levels", 0) for n in (1, 2, 3)]),
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
    commit = s["larmorx_commit"].removesuffix("-dirty")
    summary = common.table(
        "trial-original",
        f"Random testing of the clean-room original: the trial of {s['date']}",
        ["", {"label": "Cases", "num": True}],
        out_rows,
        note=f"Differential random testing's trial of the clean-room original (seed "
        f"`{s['seed']}`: the replica trial's cases drawn again, so every FSL run was served by "
        "the oracle cache): the same EPI-like phantom pairs, applytopup calls and options as the "
        "replica's trial above, each compared as the record compares (specs/topup.md §12's "
        "thresholds, the exact checks, FSL's band of a last-bit change of its read-out times "
        f"where a metric is beyond its threshold). The build: larmorx `{commit}` with the "
        "changes committed as `f1288d8` (the trial started before the commit). Every case in a "
        f"memory box of {s['memory']['case']} (peaks below {peaks:.2f} GiB).",
    )
    tpath = ctx.repo / TRIAGE
    traw = tpath.read_bytes()
    tri = json.loads(traw)
    counts = tri["counts"]
    if sum(counts.values()) != impl.get("differ", 0):
        raise ValueError(f"{TRIAGE}: {sum(counts.values())} findings, the trial {impl['differ']}")
    trows = []
    for cat, n in sorted(counts.items(), key=lambda kv: -kv[1]):
        text, verdict, case = TRIAGE_TEXT[cat]
        row = {"head": text, "cells": [common.cell(int(n), "count"),
                                       common.cell(f"{s['seed']}-{case:05d}-original")]}  # fmt: skip
        if verdict:
            row["verdict"] = verdict
        trows.append(row)
    triage = common.table(
        "trial-original-triage",
        "The trial's differing cases, each in one category",
        ["Category", {"label": "Cases", "num": True}, "Reproducer"],
        trows,
        note="Every differing case of the trial in one category by its recipe and its first "
        "failed check (`validation/investigations/cleanroom-topup/triage_counts.py`, its output "
        f"`{TRIAGE}`), each category's cause established by replaying samples with each fix and "
        "by probes (`docs/findings/fsl-topup-original.md`, findings 29 to 36); the mistakes of "
        "LARMORX and of the suite are fixed, with permanent cases in the record; the reproducer "
        "is a finding of the trial (`difftest --replay`).",
    )
    prov = {
        "trial": f"oracles/difftest/{TRIAL_ORIG}/summary.json",
        "sha256": hashlib.sha256(raw).hexdigest(),
        "triage": TRIAGE,
        "triage_sha256": hashlib.sha256(traw).hexdigest(),
    }
    return [summary, triage], prov


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, RECORD)
    examples = {
        **{f"fmriprep/{run}": ex_id for ex_id, run, _f in FMRIPREP},
        **{case: ex_id for ex_id, case in REAL.items()},
    }
    ne = named_entry(ctx)
    cs = cases(rec, examples, ne.info().get("default_mode", {}))
    rec_orig = records.load(ctx.repo, RECORD_ORIG)
    co = cases_original_entry(ctx)
    classified = co.info().get("cases", {})
    add_original(cs, rec_orig, examples, classified)
    entries = [fmriprep_entry(ctx, run) for _i, run, _f in FMRIPREP]
    entries += [real_entry(ctx, case) for case in REAL.values()]
    entries.append(ne)
    entries += [original_entry(ctx, run) for _i, run, _f in FMRIPREP]
    entries += [original_entry(ctx, case) for case in REAL.values()]
    entries.append(co)
    tt, tprov = trial(ctx)
    to, toprov = trial_original(ctx)
    bench, chart = speed(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "topup and applytopup",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source() + rec_orig.source(),
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
            common.implementation(
                cs,
                "original",
                "LARMORX clean-room original, against FSL 6.0.7.23 (standard tier)",
                "original",
                f"docs/validation/{RECORD_ORIG}.md",
                "experimental",
            ),
        ],
        "cases": cs,
        "cases_total": len(cs),
        "speed": chart,
        "tables": [tt, *to],
        "examples": [
            _example_fmriprep(ctx, *FMRIPREP[0], classified),
            _example_fmriprep(ctx, *FMRIPREP[1], classified),
        ]
        + [_example_real(ctx, ex_id, classified) for ex_id in REAL],
    }


if __name__ == "__main__":  # compute the cache entries only:
    # [fmriprep RUN | real CASE | named | original [SOURCE] | cases-original]
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    what = sys.argv[1:] or ["all"]
    if what[0] == "refresh":  # refresh_replica, the reason given
        refresh_replica(c, " ".join(what[1:]))
        sys.exit(0)
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
    if what[0] in ("original", "all"):
        for source in [run for _i, run, _f in FMRIPREP] + list(REAL.values()):
            if len(what) < 2 or what[1] == source:
                print("done", original_entry(c, source).dir, flush=True)
    if what[0] in ("cases-original", "all"):
        print("done", cases_original_entry(c).dir, flush=True)
