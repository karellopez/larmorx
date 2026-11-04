# SPDX-License-Identifier: Apache-2.0
"""FAST's evidence (the larmorx-nc replica, FSL Licence, and LARMORX's clean-room original,
Apache-2.0): fMRIPrep's tissue segmentation call (``anat_fit_wf/fast``: ``fast -I 0 -N -p -g -S 1
<T1w brain>``) on three oracle runs (an adult, and the data panel's 3.5-year-old and glioma
patient), the bias field FAST estimates by default, FSL's tissue priors (``-a``, where both
implementations' default corrects FSL-09 inside FLIRT's resampling of the priors) and a
three-channel segmentation, by FSL 6.0.7.23 (as built, and with correctly rounded maths: the
original's own last-bit band), by the replica in its ``reproduce`` and default modes (and at one
thread) and by the clean-room original in its ``reproduce`` and default modes at 14 threads and
one; every file FAST writes, the tissue volumes the partial-volume maps give, fMRIPrep's own
outputs, every case of both records with its verdict (the original's classified against FSL with
correctly rounded maths), and the benchmark.

Every run of FSL goes through the oracle-output cache (``use`` mode) with the parity suites'
keys (``parity.fsl_fast_replica``), so the runs the records made are served; FSL with the
CORE-MATH preload is the clean-room suite's band run (``parity.fsl_fast``). The replica runs as a
separate program; nothing here imports it. Everything shown lies inside the brain: fMRIPrep's
skull-stripped T1w (dilated by two voxels), or the template's brain mask: no face can show.
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
#: The clean-room original's record, and its runs: label, D13 mode, threads (its result is the
#: same at any thread count: docs/benchmarks/fsl-fast.md).
RECORD_ORIG = "fsl-fast"
ORIGINAL_RUNS = [("original", "reproduce", 14), ("original-1", "reproduce", 1),
                 ("original-fix", "fix", 14)]  # fmt: skip
#: The panels of the original's entries: FSL with correctly rounded maths first (the closest
#: reference of the original), then the original's runs.
ORIGINAL_PANELS = [
    ("fsl-cr", "original", "FSL 6.0.7.23, correctly rounded maths",
     "the same program with CORE-MATH's exp, log, expf and logf preloaded, through the oracle "
     "cache"),
    ("original", "larmorx", "LARMORX clean-room original", "reproduce mode, 14 threads"),
    ("original-1", "larmorx", "LARMORX clean-room original", "reproduce mode, 1 thread"),
    ("original-fix", "larmorx", "LARMORX clean-room original",
     "default mode (D13 corrections), 14 threads"),
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


def _replica(
    e: fcache.Entry,
    args: list[str],
    put_inputs,
    settings: dict[str, str | None],
    runs: list[tuple[str, str, str]] = REPLICA_RUNS,
) -> None:
    """The replica on ``args``, once per run of ``runs`` (default :data:`REPLICA_RUNS`), each in
    ``<entry>/<label>`` (it writes its outputs where FSL does: its working directory); the inputs
    removed after."""
    from larmorx_validation.parity.fsl_flirt_replica import _environment

    fs = _suite()
    exe = fs.replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    env = _environment(settings)
    unset = tuple(k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_", "OMP_")))
    for label, mode, threads in runs:
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


def _fmriprep_source(run: str):
    """``(args, put_inputs, input)`` of fMRIPrep's FAST call of ``run``, as the record runs it."""
    fs = _suite()
    node = _node(run)
    args = fs.fmriprep_args(node)
    src = fs.fmriprep_input(run, str(node))
    if src is None:
        raise FileNotFoundError(f"the input of fMRIPrep's FAST node of {run}")

    def put(d: Path) -> None:
        shutil.copy(src, d / args[-1])

    return args, put, src


def _compute_fmriprep(run: str):
    def compute(e: fcache.Entry) -> None:
        node = _node(run)
        args, put, src = _fmriprep_source(run)
        e.input("T1w brain (fMRIPrep's)", src)
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


def _real_source(case: str):
    """``(args, put_inputs, settings)`` of the record's case ``case``, as the record runs it."""
    fs = _suite()
    run = _real_run(case)
    # the arguments as the record's case gives them (an input by its file name)
    args = [Path(a[1:]).name if a.startswith("@") else a for a in run.args]

    def put(d: Path) -> None:
        fs._prepare(run, d)

    return args, put, dict(run.settings)


def _compute_real(case: str):
    def compute(e: fcache.Entry) -> None:
        fs = _suite()
        run = _real_run(case)
        for a in run.args:
            if a.startswith("@"):
                e.input(a[1:], fs.inputs() / a[1:])
        args, put, settings = _real_source(case)
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


NAMED_TEXT = (
    "The replica's default mode on every case of the record: its exit status and the entries "
    "it names"
)


def named_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(ctx.figures_context(), TOOL, "cases-default-mode", NAMED_TEXT,
                        _compute_named)  # fmt: skip


def refresh_replica(ctx: core.Context, why: str) -> None:
    """After a change of the replica's default mode: its default-mode runs of every example's
    entry made again (in their working directories, with their inputs), and its default mode on
    every case of the record computed again (the named entry); FSL's runs and the reproduce
    mode's are kept (the record shows the reproduce mode unchanged)."""
    fix = [r for r in REPLICA_RUNS if r[1] == "fix"]
    sources = [(fmriprep_entry(ctx, run), _fmriprep_source(run)[:2], {})
               for _i, run, _v, _w in FMRIPREP]  # fmt: skip
    for case in REAL.values():
        args, put, settings = _real_source(case)
        sources.append((real_entry(ctx, case), (args, put), settings))
    for e, (args, put), settings in sources:
        info = e.info()
        e.runs, e.extra = [], {}
        for label, _m, _t in fix:  # the outputs being made again (the run writes the same files)
            for f in (e.dir / label).iterdir():
                f.unlink()
        _replica(e, args, put, settings, runs=fix)
        fcache.merge_rerun(e, info, why)
    fcache.recompute(ctx.figures_context(), TOOL, "cases-default-mode", NAMED_TEXT,
                     _compute_named)  # fmt: skip


# ------------------------------------------------------------------ the clean-room original


def _orig():
    from larmorx_validation.parity import fsl_fast as orig

    return orig


def versions_original() -> dict[str, str]:
    """FSL's version, the original's and the CORE-MATH preload's (by its content)."""
    import larmorx
    from larmorx_validation.figures.core import sha256

    lib = _orig().crmath_preload()
    return {
        "fsl": _fsl_version(),
        "larmorx": f"larmorx {larmorx.__version__} (the clean-room original, crate larmorx-fsl, "
        "Apache-2.0)",
        "core-math": "CORE-MATH's correctly rounded exp, log, expf, logf (the workspace's "
        f"preload, sha256 {sha256(lib)[:16]})"
        if lib
        else "CORE-MATH preload not found",
    }


def _fsl_version() -> str:
    from larmorx_validation.parity import fsl_versions
    from larmorx_validation.parity.fsl_flirt_replica import fsldir

    return (
        f"FSL {fsl_versions.version_of(fsldir())} (fsl-fast4 2111.3; ~/fsl-6.0.7.23, never on "
        "the PATH)"
    )


def _original_run(
    e: fcache.Entry, label: str, args: list[str], put_inputs, settings: dict, mode: str, n: int
) -> None:
    """The clean-room original on ``args`` in ``<entry>/<label>`` (it writes its outputs where
    FSL does: its working directory), as its record runs it; the inputs removed after."""
    import time

    from larmorx_validation.parity import defects

    orig = _orig()
    cwd = e.dir / label
    cwd.mkdir(exist_ok=True)
    put_inputs(cwd)
    given = list(cwd.iterdir())
    e.ctx.log(f"  {e.tool}/{e.case}: {label}")
    t0 = time.perf_counter()
    p = orig.run_original(args, cwd=cwd, settings=settings, mode=mode, threads=n)
    seconds = round(time.perf_counter() - t0, 2)
    for f in given:
        f.unlink()
    env = {defects.ENV: mode, "OMP_NUM_THREADS": str(n), "LARMORX_IMPLEMENTATION": "original"}
    run = fcache.Run(label, [*orig.original_command(), "fsl", "fast", *args], env)
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


def _source(source: str):
    """``(args, put_inputs, settings, never, inputs)`` of an fMRIPrep run or a record case."""
    fs = _suite()
    if source.startswith("ds"):
        node = _node(source)
        args = fs.fmriprep_args(node)
        src = fs.fmriprep_input(source, str(node))
        if src is None:
            raise FileNotFoundError(f"the input of fMRIPrep's FAST node of {source}")

        def put(d: Path) -> None:
            shutil.copy(src, d / args[-1])

        return args, put, {}, "", {"T1w brain (fMRIPrep's)": src}
    run = _real_run(source)
    ins = {a[1:]: fs.inputs() / a[1:] for a in run.args if a.startswith("@")}
    args = [Path(a[1:]).name if a.startswith("@") else a for a in run.args]

    def put_real(d: Path) -> None:
        fs._prepare(run, d)

    return args, put_real, dict(run.settings), run.never, ins


def _compute_original(source: str):
    def compute(e: fcache.Entry) -> None:
        args, put, settings, never, ins = _source(source)
        for label, path in ins.items():
            e.input(label, path)
        lib = _orig().crmath_preload()
        if lib is None:
            raise FileNotFoundError("the CORE-MATH preload (oracles/core-math-preload)")
        # FSL with correctly rounded maths: the clean-room suite's band run (served when the
        # record made it), the original's closest reference
        _oracle(e, "fsl-cr", args, put, {**settings, "LD_PRELOAD": str(lib)}, never=never)
        for label, mode, n in ORIGINAL_RUNS:
            _original_run(e, label, args, put, settings, mode, n)
        what = (
            "fMRIPrep 25.2.6's FAST node of " if source.startswith("ds") else "The record's case "
        )
        e.finish(
            versions=versions_original(),
            command="fast " + " ".join(args),
            settings=f"{what}{source}: FSL 6.0.7.23 with the CORE-MATH preload through the "
            "oracle-output cache "
            "(the clean-room suite's band run) and LARMORX's clean-room original as its record "
            "runs it (reproduce mode at 14 and 1 threads, the default mode at 14); FSL as built "
            "and the replica are the replica entry's runs of the same call",
        )

    return compute


def original_entry(ctx: core.Context, source: str) -> fcache.Entry:
    """The clean-room original's runs, and FSL's with correctly rounded maths, of an fMRIPrep
    run's call (``source`` a run) or a record case (``source`` a case)."""
    name = f"fmriprep-{source}" if source.startswith("ds") else source.replace("/", "-")
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"original-{name}",
        f"{source}: FSL 6.0.7.23 with correctly rounded maths (from the oracle cache) and "
        "LARMORX's clean-room original, reproduce and default modes",
        _compute_original(source),
    )


def case_table_original() -> dict[str, tuple[str, object]]:
    """Every case of the clean-room original's suite (``parity.fsl_fast``: the replica suite's
    cases and the original's own permanent cases): id -> (description, its ``Run``; None for
    fMRIPrep's calls)."""
    from larmorx_validation.difftest import permanent

    s = _orig().suite()
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


def _values(path: Path) -> np.ndarray:
    return np.asarray(_orig()._load(path)[1])


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


def _compare_dirs(fsl: Path, ours: Path, cr: Path, before: set[str]) -> dict[str, dict]:
    """Every image FSL wrote (``fsl``) against the original's (``ours``) and FSL's with
    correctly rounded maths (``cr``): per file, (values that differ, largest |Δ|) of original −
    FSL, FSL with CR − FSL, original − FSL with CR."""
    from larmorx_validation.parity.fsl_mcflirt_replica import _files, _is_image

    out = {}
    fa, fb, fc = _files(fsl, set()), _files(ours, set()), _files(cr, set())
    for key in sorted(set(fa) - before):
        if not _is_image(key) or key not in fb or key not in fc:
            continue
        if key.endswith(".img") or ".img." in key:
            continue  # an ANALYZE pair is read through its .hdr
        a, b, c = _values(fa[key]), _values(fb[key]), _values(fc[key])
        out[key] = {"kind": _orig()._kind(key), "orig_fsl": _differ(b, a),
                    "cr_fsl": _differ(c, a), "orig_cr": _differ(b, c), "values": int(a.size)}  # fmt: skip
    return out


def _classify_case(r: dict, payloads: dict, nodes: dict, lib: Path) -> tuple[str, dict]:
    """One case of the original's record: its default mode (exit status, the entries named)
    and, for a case that passed, every file of FSL, FSL with correctly rounded maths and the
    original (reproduce mode) compared, with the suite's own treatment of the memory FSL never
    writes (FSL-96, FSL-97)."""
    from larmorx_validation.parity.harness import CheckList

    fs, orig = _suite(), _orig()
    case = r["case"]
    if case.startswith("fmriprep/"):
        node = nodes[case]
        args0 = fs.fmriprep_args(node)
        src = fs.fmriprep_input(case.split("/")[1], str(node))

        def prepare(d: Path) -> list[str]:
            shutil.copy(src, d / args0[-1])
            return args0

        settings: dict[str, str | None] = {}
        never, unwritten, no_bias = "", (), False
    else:
        kind, payload = payloads[case]
        run = payload if kind == "direct" else fs._all_cases()[payload][2]

        def prepare(d: Path) -> list[str]:
            return fs._prepare(run, d)

        settings = dict(run.settings)
        never, unwritten, no_bias = run.never, run.unwritten, run.no_bias_field
    out: dict = {}
    with tempfile.TemporaryDirectory(prefix="lx-fast-ev-orig_") as t:
        t = Path(t)
        a, b, c, f = (t / x for x in ("fsl", "orig", "cr", "fix"))
        for x in (a, b, c, f):
            x.mkdir()
        args = prepare(f)
        p = orig.run_original(args, cwd=f, settings=settings, mode="fix", threads=4)
        from larmorx_validation.parity import defects

        out["default"] = {"exit": p.code, "named": defects.notices_in(p.stderr)}
        if r["status"] != "pass":
            return case, out
        for x in (a, b, c):
            prepare(x)
        before = set(_files_of(a))
        pa = fs.oracle_run(args, cwd=a, settings=settings, never=never)
        pc = fs.oracle_run(args, cwd=c, settings={**settings, "LD_PRELOAD": str(lib)}, never=never)
        po = orig.run_original(args, cwd=b, settings=settings, mode="reproduce", threads=4)
        for d in (a, c):
            if unwritten:
                fs._clear_unwritten(CheckList(), d, unwritten)
            if no_bias:
                fs._undefined_bias_field(CheckList(), d, args)
        out["served"] = {"fsl": bool(pa.cached), "fsl-cr": bool(pc.cached)}
        out["exit"] = [pa.returncode, pc.returncode, po.code]
        out["files"] = _compare_dirs(a, b, c, before)
    return case, out


def _files_of(d: Path) -> list[str]:
    from larmorx_validation.parity.fsl_mcflirt_replica import _files

    return list(_files(d, set()))


def _compute_cases_original(e: fcache.Entry) -> None:
    from concurrent.futures import ThreadPoolExecutor

    from larmorx_validation.difftest import permanent

    rec = records.load(e.ctx.repo, RECORD_ORIG)
    s = _orig().suite()
    permanent.attach(s)
    payloads = {c.id: c.payload for c in s.cases("full")}
    nodes = {f"fmriprep/{r}": n for r, n in _suite().fmriprep_calls()}
    lib = _orig().crmath_preload()
    if lib is None:
        raise FileNotFoundError("the CORE-MATH preload (oracles/core-math-preload)")
    jobs = int(os.environ.get("LARMORX_EVIDENCE_JOBS", "3"))
    with ThreadPoolExecutor(jobs) as pool:
        done = dict(pool.map(lambda r: _classify_case(r, payloads, nodes, lib), rec.results))
    e.finish(
        versions=versions_original(),
        cases={k: done[k] for k in sorted(done)},
        settings="Every case of the clean-room original's record (fsl-fast), in fresh "
        "directories with the case's inputs: the original's default mode (4 threads: its exit "
        "status and the register entries its notices name); for each case that passed, FSL "
        "6.0.7.23 as built and with the CORE-MATH preload through the oracle-output cache (the "
        "record's runs served, the never-cached policies rerun) and the original in reproduce "
        "mode, every image compared value by value (FSL's memory it never writes, FSL-96 and "
        "FSL-97, treated as the record treats it)",
    )


CASES_ORIGINAL_TEXT = (
    "Every case of the clean-room original's record: its default mode, and each passing case's "
    "files against FSL and FSL with correctly rounded maths"
)


def cases_original_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "cases-original", CASES_ORIGINAL_TEXT,
        _compute_cases_original,
    )  # fmt: skip


def _emptied(d: Path) -> None:
    """The outputs of a run directory being made again (the run writes the same files)."""
    for f in d.iterdir():
        shutil.rmtree(f) if f.is_dir() else f.unlink()


def refresh_original(ctx: core.Context, why: str) -> None:
    """After a change of the clean-room original: its runs (:data:`ORIGINAL_RUNS`) of every
    example's entry already computed made again, in their working directories with their
    inputs (FSL's run with correctly rounded maths kept: the original's change does not touch
    it), and every case of its record classified again (the ``cases-original`` entry, FSL's runs
    served by the oracle-output cache)."""
    fctx = ctx.figures_context()
    for source in [run for _i, run, _v, _w in FMRIPREP] + list(REAL.values()):
        name = f"fmriprep-{source}" if source.startswith("ds") else source.replace("/", "-")
        e = fcache.Entry(fctx, TOOL, f"original-{name}", "")
        if not e.done:
            continue
        info = e.info()
        args, put, settings, _never, _ins = _source(source)
        e.runs, e.extra = [], {}
        for label, mode, n in ORIGINAL_RUNS:
            _emptied(e.dir / label)
            _original_run(e, label, args, put, settings, mode, n)
        fcache.merge_rerun(e, info, why)
    fcache.recompute(fctx, TOOL, "cases-original", CASES_ORIGINAL_TEXT, _compute_cases_original,
                     why)  # fmt: skip


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


def _files_table(
    tid: str,
    files: dict[str, Path],
    run_dir,
    labels: dict[str, str],
    counted: dict[str, tuple[Path, Path]] | None = None,
) -> dict:
    """Every image FAST wrote, FSL's against each run: the data and the header (``descrip``,
    FSL's build string, aside), byte for byte; ``counted`` (column -> (run directory, reference
    directory), the clean-room original's columns): the values of the run's file that differ
    from the reference's when the data are not the same."""
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
        for run_d, ref_d in (counted or {}).values():
            data, head = _same_file(ref_d / f.name, run_d / f.name)
            if data:
                cells.append(common.cell("identical" if head else "data identical"))
            else:
                n, _m = _differ(_values(run_d / f.name), _values(ref_d / f.name))
                cells.append(common.cell(f"{n:,} values differ"))
        rows.append({"head": f"`{f.name}`", "cells": cells})
    note = (
        "Each file decompressed and compared byte for byte with FSL's: *identical*, the header "
        "(its `descrip`, FSL's build string, aside) and every value; *data identical*, every "
        "value but not the header; *differs*, values that differ (see the comparisons above)."
    )
    if counted:
        note += (
            " For the clean-room original, where the values are not the same, how many of the "
            "file's values differ: from FSL's file, and in the last two columns from the file of "
            "FSL with correctly rounded maths (the record holds them to specs/fast.md §11's "
            "thresholds, not to FSL's bytes; since 2026-11-04 its reproduce mode computes exp, "
            "log, expf and logf as glibc 2.35 does and its default correctly rounded, GLIBC-07)."
        )
    return common.table(
        tid,
        "Every image FAST writes, against FSL 6.0.7.23's",
        ["File", "Type", "Voxels", *labels.values(), *(counted or {})],
        rows,
        note=note,
    )


def _volumes_table(tid: str, pve: dict[str, dict[str, np.ndarray]], vox_ml: float, note: str):
    """The tissue volumes the partial-volume maps give (ml): sum of each map × the voxel."""
    rows = []
    ref = None
    for head, maps in pve.items():
        vals = [float(np.sum(maps[f"pve_{k}"], dtype=np.float64)) * vox_ml for k in range(3)]
        ref = ref or vals
        rel = max(abs(v - r) / abs(r) if r else 0.0 for v, r in zip(vals, ref, strict=True))
        same = "the same" if vals == ref else f"differs (at most {100 * rel:.2g} %)"
        rows.append({"head": head, "cells": [common.cell(v, "sig6") for v in vals]
                     + [common.cell(same)]})  # fmt: skip
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
                **REPLICA_FIX,
            }
            x["reason"] = " ".join(reasons.get(n, f"{n}, corrected by default.") for n in fix_named
                                   if n not in ("FSL-96", "FSL-97"))  # fmt: skip
            for n in fix_named:
                x["links"].append({"label": n, "href": "docs/findings/upstream-defects.md"})
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example (see the record)."


#: The corrections of 2026-11-03 the replica's default mode names (the spec answers' part).
REPLICA_FIX = {
    "FSL-202": "FSL-202, corrected by default with a notice (since 2026-11-03): with one "
    "channel, FAST takes each class's variance in the logarithm of the intensities as the mean "
    "of the squares minus the square of the mean, that square of a single-precision mean "
    "rounded to single precision, so most of its digits cancel; the default squares the mean "
    "of a double-precision quotient in double precision. Measured on seven cases, fMRIPrep's "
    "three calls among them, the default's segmentation and mixel types lie 0 or 1 voxel from "
    "FAST computed with its class statistics in double precision throughout, where FSL's lie 1 "
    "to 844 voxels from it, and its own last-bit band is 0 voxels where FSL's reaches 3 to 807: "
    "most of FSL's own band is this cancellation. The reproduce mode gives FSL's bytes (the "
    "row above).",
    "FSL-203": "FSL-203, corrected by default with a notice (since 2026-11-03): when the "
    "image's first voxel is NaN, FAST's intensity extrema become NaN, its rule for negative "
    "values is skipped and its brain mask is empty, so it stops with exit status 255; the "
    "default takes the extrema over the values that are numbers.",
}


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


def _diffs_original(own: bool) -> list[dict]:
    d = common.d
    out = [
        d("original-fsl", "original", "fsl", "main", "clean-room original − FSL 6.0.7.23",
          common.ROLE_LX),
        d("original-cr", "original", "fsl-cr", "main",
          "clean-room original − FSL, correctly rounded maths", common.ROLE_LX),
        d("ofix-cr", "original-fix", "fsl-cr", "main",
          "clean-room original, default − FSL, correctly rounded maths", common.ROLE_LX),
        d("original1-original", "original-1", "original", "check",
          "clean-room original, 1 thread − 14 threads", common.ROLE_SELF),
        d("ofix-original", "original-fix", "original", "check",
          "clean-room original, default − reproduce mode", common.ROLE_SELF),
        d("cr-fsl", "fsl-cr", "fsl", "variability",
          "FSL, correctly rounded maths − FSL as built", common.ROLE_ORIG),
    ]  # fmt: skip
    if own:
        out.insert(2, d("original-own", "original", "own", "fmriprep",
                        "clean-room original − fMRIPrep's own output", common.ROLE_FMRIPREP))  # fmt: skip
    return out


def _beyond(x: dict, y: dict) -> list[str]:
    """The outputs where comparison ``x`` differs in more values, or by more, than ``y``."""
    out = []
    for o, m in x["metrics"].items():
        n = y["metrics"].get(o)
        if n is not None and (m["differ"] > n["differ"] or m["max_abs"] > n["max_abs"]):
            out.append(o)
    return out


def _counts(x: dict, outputs: list[str], labels: dict[str, str], y: dict | None = None) -> str:
    """The differences of comparison ``x`` in ``outputs`` (and of ``y``, the yardstick)."""
    parts = []
    for o in outputs[:3]:
        m = x["metrics"][o]
        t = f"{labels.get(o, o)}: {m['differ']:,} values differ (at most {m['max_abs']:.3g})"
        if y is not None and o in y["metrics"]:
            n = y["metrics"][o]
            t += f", FSL's own {n['differ']:,} (at most {n['max_abs']:.3g})"
        parts.append(t)
    more = len(outputs) - 3
    return "; ".join(parts) + (f"; {more} more" if more > 0 else "")


FINDINGS = "docs/findings/fsl-fast-original.md"
REGISTER = "docs/findings/upstream-defects.md"
#: The corrections the clean-room original's default mode names, as it makes them.
ORIGINAL_FIX = {
    "FSL-09": "FSL-09, corrected by default with a notice: FAST registers FSL's tissue priors "
    "(8-bit integers) onto the image with FLIRT, which writes its resampled values in that "
    "integer type truncated toward zero; the clean-room original's default (FLIRT's clean-room "
    "original, run in the same process) rounds them to the nearest integer, so the priors FAST "
    "starts from, and its result, differ. Its reproduce mode follows FSL (the rows above).",
    "FSL-98": "FSL-98, corrected by default with a notice: with several channels, sorting the "
    "classes gives each class another class's covariance and posteriors when the classes move "
    "in a cycle; the default gives each class its own.",
    "FSL-103": "FSL-103, corrected by default with a notice: the brain mask leaves out the voxels "
    "equal to the maximum when it is 2^24 or more.",
}


def _verdicts_original(
    diffs: list[dict], same: dict[str, bool], what: str, oinfo: dict, labels: dict[str, str]
):
    """The clean-room original's comparisons: verdicts from the metrics, against FSL as built,
    FSL with correctly rounded maths and FSL's own last-bit band (``cr-fsl``). Since 2026-11-04
    its reproduce mode computes exp, log, expf and logf as glibc 2.35 does and its default
    correctly rounded (GLIBC-07, not named): the reproduce mode is held to FSL as built, the
    default to FSL with correctly rounded maths."""
    by = {x["id"]: x for x in diffs}
    named = [n for n in oinfo.get("named", {}).get("original-fix", []) if n.startswith("FSL-")]
    shown = [n for n in named if n not in ("FSL-96", "FSL-97")]
    band = by["cr-fsl"]
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if x["kind"] == "variability":
            x["verdict"] = None
            x["reason"] = (
                f"FSL's result on {what} does not move when exp, log, expf and logf are "
                "correctly rounded."
                if same[i]
                else "The yardstick: FSL against itself with CORE-MATH's correctly rounded exp, "
                "log, expf and logf preloaded into the same unmodified program, so how far FSL's "
                "result moves when only those functions' last bits change. FAST's single-channel "
                "variance in the log domain cancels most digits, so such a change moves a few "
                "voxels of the smallest classes and mixel types."
            )
            x["links"].append({"label": "FSL's own last-bit band", "href": FINDINGS})
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "original-fsl": f"The same values, every one, in every image FAST writes on "
                f"{what}.",
                "original-cr": "With correctly rounded maths FSL computes exactly the clean-room "
                f"original's values in every image FAST writes on {what}: the original follows "
                "FAST's arithmetic, with correctly rounded exp and log.",
                "ofix-cr": "With correctly rounded maths FSL computes exactly the clean-room "
                f"original's default values in every image FAST writes on {what}: the default "
                "follows FAST's arithmetic with exp, log, expf and logf correctly rounded on "
                "every platform"
                + (
                    ", and no correction of the register applies to this call."
                    if not shown
                    else "."
                ),
                "original1-original": "The thread count changes nothing: the clean-room "
                "original's parallel sweeps (a pipeline of planes in the serial order), blocked "
                "sums and sorts give its one-thread result bit for bit at any thread count.",
                "ofix-original": _fix_same(named),
            }.get(i, "The same values, every one.")
        elif i in ("ofix-original", "ofix-cr") and shown:
            x["verdict"] = "d13"
            x["reason"] = " ".join(
                ORIGINAL_FIX.get(n, f"{n}, corrected by default.") for n in shown
            ) + (
                " It also computes exp, log, expf and logf correctly rounded (GLIBC-07, not named)."
                if i == "ofix-original"
                else ""
            )
            x["links"] += [{"label": n, "href": REGISTER} for n in shown]
        elif i == "ofix-original":
            x["verdict"] = "d13"
            x["reason"] = common.glibc07("exp, log, expf and logf", "FAST", named=False) + (
                " The default gives FSL's values with correctly rounded maths bit for bit and "
                "the reproduce mode FSL's as built (the rows above), so this row is FSL's own "
                "last-bit band, value for value (the yardstick row)."
                if same.get("ofix-cr") and same.get("original-fsl")
                else " "
                + _counts(
                    x, [o for o, m in x["metrics"].items() if not m["identical"]], labels, band
                )
                + "."
            )
            x["links"].append({"label": "GLIBC-07", "href": REGISTER})
        elif i == "ofix-cr":
            beyond = _beyond(x, band)
            diff_out = [o for o, m in x["metrics"].items() if not m["identical"]]
            x["verdict"] = "open" if beyond else "within-variability"
            x["reason"] = (
                "The default against FSL with correctly rounded maths: "
                + _counts(x, beyond or diff_out, labels, band)
                + (
                    "; further than FSL as built is from it."
                    if beyond
                    else "; no further, in any output, than FSL as built is from it (FSL's own "
                    "last-bit band)."
                )
            )
            x["links"].append({"label": "findings", "href": FINDINGS})
        elif i == "original-own" and not same.get("replica-own"):
            x["verdict"] = "open"
            x["reason"] = "fMRIPrep's own output is not FSL's here (see the replica's row)."
        elif i in ("original-fsl", "original-own"):
            pre = (
                "fMRIPrep's own files are FSL's (the replica's row): "
                if i == "original-own"
                else ""
            )
            beyond = _beyond(by["original-fsl"], band)
            if same["original-cr"]:
                x["verdict"] = "identical-cr"
                x["reason"] = pre + (
                    "With CORE-MATH's correctly rounded exp, log, expf and logf preloaded into the "
                    "same unmodified FSL, FSL gives the clean-room original's values in every image "
                    f"FAST writes on {what}: the differences from FSL as built come from glibc's "
                    "last bits in those functions alone."
                )
            elif not beyond:
                x["verdict"] = "within-variability"
                diff_out = [
                    o for o, m in by["original-fsl"]["metrics"].items() if not m["identical"]
                ]
                x["reason"] = pre + (
                    "In every output the clean-room original differs from FSL in no more values, "
                    "and by no more, than FSL itself does when only its maths functions' last "
                    "bits change (FSL with correctly rounded maths − FSL, the yardstick row): "
                    + _counts(by["original-fsl"], diff_out, labels, band)
                    + "."
                )
                x["links"].append({"label": "FSL's own last-bit band", "href": FINDINGS})
            elif not _beyond(by["original-cr"], band):
                x["verdict"] = "within-variability"
                near = [o for o, m in by["original-cr"]["metrics"].items() if not m["identical"]]
                x["reason"] = pre + (
                    "In every output the clean-room original is no further from FSL with "
                    "correctly rounded maths than FSL as built is (the yardstick row): FSL's own "
                    "last-bit band. Against FSL with correctly rounded maths: "
                    + _counts(by["original-cr"], near, labels, band)
                    + "; the partial volumes that differ are each one fraction step (0.01) away, "
                    "near ties decided by last bits."
                    if near
                    else ""
                )
                x["links"].append({"label": "FSL's own last-bit band", "href": FINDINGS})
            else:
                x["verdict"] = "open"
                x["reason"] = pre + (
                    "Within the record's thresholds (specs/fast.md §11), but in "
                    f"{len(beyond)} output(s) the clean-room original differs from FSL, and from "
                    "FSL with correctly rounded maths, more than the two differ from each other: "
                    + _counts(by["original-fsl"], beyond, labels, band)
                    + ". FAST's single-channel variance in the log domain cancels most digits, "
                    "so a last-bit change of a posterior moves a few voxels of the smallest "
                    "classes; it can still be improved."
                )
                x["links"].append({"label": "findings", "href": FINDINGS})
        elif i == "original-cr" and same["original-fsl"]:
            x["verdict"] = "within-variability"
            x["reason"] = (
                "The reproduce mode computes exp, log, expf and logf as glibc 2.35 does, as FSL's "
                "build, so it gives FSL's values as built, every one (the row above); against "
                "FSL with correctly rounded maths its differences are therefore FSL's own "
                "last-bit band, value for value (the yardstick row)."
            )
            x["links"].append({"label": "FSL's own last-bit band", "href": FINDINGS})
        elif i == "original-cr":
            # the reproduce mode computes glibc's functions (since 2026-11-04), so it is held to
            # FSL as built: from FSL with correctly rounded maths it may differ by FSL's own band
            # and its own differences from FSL as built together
            of = by["original-fsl"]["metrics"]
            beyond = [
                o
                for o, m in x["metrics"].items()
                if o in band["metrics"]
                and m["differ"] > band["metrics"][o]["differ"] + of.get(o, {}).get("differ", 0)
            ]
            diff_out = [o for o, m in x["metrics"].items() if not m["identical"]]
            if not beyond:
                x["verdict"] = "within-variability"
                x["reason"] = (
                    "The reproduce mode computes exp, log, expf and logf as glibc 2.35 does, as "
                    "FSL's build (since 2026-11-04), so it is held to FSL as built (the row "
                    "above); from FSL with correctly rounded maths it differs by no more than "
                    "FSL's own last-bit band (the yardstick row) and its own differences from FSL "
                    "as built together: " + _counts(x, diff_out, labels, band) + "."
                )
            else:
                x["verdict"] = "open"
                x["reason"] = (
                    "Further from FSL with correctly rounded maths than FSL's own last-bit band "
                    "and the original's differences from FSL as built together, in "
                    f"{len(beyond)} output(s): " + _counts(x, beyond, labels, band) + "."
                )
            x["links"].append({"label": "findings", "href": FINDINGS})
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example (see the record)."


def _note(n: int, extra: str = "") -> str:
    return (
        f"Δ is A − B, computed in float64 over all {n:,} voxels of the outputs, not only those "
        "shown. The record's threshold: every file FSL writes bit-identical (images "
        "decompressed, `descrip` left out), with FSL's exit status and messages."
        + extra
        + " The clean-room original's record (fsl-fast) holds it to specs/fast.md §11's "
        "thresholds instead: FSL's exit status, files and messages (numbers to 4 significant "
        "digits); the label images per-class Dice ≥ 0.999 with ≥ 99.9 % of the mask's voxels "
        "identical; the partial volumes' mean |Δ| ≤ 0.001, 99.9th percentile ≤ 0.02, totals "
        "within 0.1 %; the probabilities' mean |Δ| ≤ 0.001; the bias field and corrected image "
        "within 1e-4 relative at the 99.9th percentile."
    )


def _load(run_dir: Path, files: dict[str, Path]) -> dict[str, np.ndarray]:
    return {k: common.load(str(run_dir / f.name)) for k, f in files.items()}


def _programs(info: dict, oinfo: dict | None = None) -> str:
    vers = info.get("versions", {})
    served = info.get("oracle_cache", {})
    extra = ""
    if oinfo is not None:
        ov = oinfo.get("versions", {})
        cr = oinfo.get("oracle_cache", {}).get("fsl-cr")
        extra = (
            f"; the same FSL with {ov.get('core-math', 'CORE-MATH')} ("
            + ("served from the oracle cache" if cr == "served" else "run through the oracle cache")
            + f"); {ov.get('larmorx', '?')}, from larmorx at commit "
            f"`{oinfo['larmorx_commit'][:12]}`"
        )
    return (
        f"{vers.get('fsl', '?')} ("
        + (
            "its run served from the oracle cache, as the record ran it"
            if served.get("fsl") == "served"
            else "run for this page through the oracle cache"
        )
        + f"); {vers.get('larmorx-nc', '?')} (the replica), from larmorx at commit "
        f"`{info['larmorx_commit'][:12]}`" + extra
    )


def _differs_at(ex, a: str, b: str, out: str, at: list[int]) -> bool:
    """Whether panels ``a`` and ``b`` differ in ``out`` at the display voxel ``at``."""
    va, vb = ex.values(ex.key(a, out)), ex.values(ex.key(b, out))
    return bool(va[tuple(at)] != vb[tuple(at)])


def _priors_same(pri: list[Path], run_dir: Path) -> str:
    """How a run's registered priors compare with FSL's."""
    res = [_same_file(p, run_dir / p.name) for p in pri if (run_dir / p.name).is_file()]
    if len(res) != len(pri):
        return "not all written"
    if all(d and h for d, h in res):
        return "FSL's bytes"
    if all(d for d, _h in res):
        return "FSL's values"
    n = sum(_differ(_values(p), _values(run_dir / p.name))[0] for p in pri)
    return f"{n:,} values other than FSL's (FSL-09: rounded)"


def _panel_order() -> list[tuple[str, str, str, str]]:
    """FSL, FSL with correctly rounded maths, the replica's runs, the original's."""
    return [PANELS[0], ORIGINAL_PANELS[0], *PANELS[1:], *ORIGINAL_PANELS[1:]]


def _counted(e: fcache.Entry, o: fcache.Entry) -> dict[str, tuple[Path, Path]]:
    """The files table's columns of the clean-room original."""
    return {
        "original (reproduce)": (o.dir / "original", e.dir / "fsl"),
        "original (default)": (o.dir / "original-fix", e.dir / "fsl"),
        "original − FSL, correctly rounded maths": (o.dir / "original", o.dir / "fsl-cr"),
        "original (default) − FSL, correctly rounded maths": (o.dir / "original-fix",
                                                               o.dir / "fsl-cr"),
    }  # fmt: skip


def _provenance(info: dict, e: fcache.Entry, oinfo: dict, o: fcache.Entry, data: list) -> list:
    """Both entries' provenance: the replica's and the original's runs."""
    first = common.provenance(info, f"{TOOL}/{e.case}", data)
    second = common.provenance(oinfo, f"{TOOL}/{o.case}", data)
    return first + [line for line in second if line.startswith("Runs:")]


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
    o = original_entry(ctx, run)
    oinfo = o.info()
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
    arrays.update({pid: _load(o.dir / pid, files) for pid, *_ in ORIGINAL_PANELS})
    for pid, role, label, sub in _panel_order():
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    own_files = {k: node / f.name for k, f in files.items() if (node / f.name).is_file()}
    own = {k: common.load(str(p)) for k, p in own_files.items()}
    rdiffs, odiffs = _diffs(True), _diffs_original(True)
    diffs = rdiffs + odiffs
    outputs = _outputs(files)
    common.compare_all(ex, diffs, outputs, full={"own": own})
    same = common.identical(diffs)
    _verdicts(rdiffs, same, "this call", info, bias=False)
    _verdicts_original(odiffs, same, "this call", oinfo, {x["id"]: x["label"] for x in outputs})
    if view == "slices":
        # the three slices stay where they were before the original joined the example
        at = _default_view(ex, view, "fsl", "replica", "pve_1")
        where = "the brain's centre"
    else:
        at = _default_view(ex, view, "fsl", "original", "pve_1")
        where = (
            "the voxel of the slice with the most brain where the clean-room original's grey "
            "matter differs most from FSL's"
            if _differs_at(ex, "fsl", "original", "pve_1", at)
            else "the brain's centre (no voxel of the slice with the most brain differs between "
            "FSL and the clean-room original)"
        )
    if run in VIEW_AT:
        at = grid.display_index(VIEW_AT[run])
        where = "the tumour"
    default = {
        "output": "pve_1",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1w", "fsl", "replica", "original"],
        "diffs": ["replica-fsl", "original-fsl"],
        "compare": {"a": "fsl", "b": "original"},
    }
    agree = (
        "the replica agrees with FSL everywhere; the clean-room original's differences are "
        "sought in them"
        if view == "slices"
        else "the replica agrees with FSL everywhere"
    )
    out = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="fsl",
        caption=(f"the three slices through {where} are stored at full resolution ({agree})."
                 if view == "slices" else f"the crosshair at {where} ({agree})."),
    )  # fmt: skip
    vox_ml = float(np.prod(img.header.get_zooms()[:3])) / 1000.0
    pve = {
        "FSL 6.0.7.23": arrays["fsl"],
        "replica (reproduce)": arrays["replica"],
        "replica (default)": arrays["replica-fix"],
        "fMRIPrep's own (its image's FSL)": own,
        "FSL 6.0.7.23, correctly rounded maths": arrays["fsl-cr"],
        "clean-room original (reproduce)": arrays["original"],
        "clean-room original (default)": arrays["original-fix"],
    }
    run_dirs = {"replica": e.dir / "replica", "replica-fix": e.dir / "replica-fix"}
    tables = [
        _files_table(
            f"{ex_id}-files",
            files,
            lambda pid: run_dirs[pid],
            {"replica": "replica (reproduce)", "replica-fix": "replica (default)"},
            _counted(e, o),
        ),
        _volumes_table(
            f"{ex_id}-volumes",
            pve,
            vox_ml,
            "Each partial-volume map summed over every voxel in float64, times the voxel's "
            f"volume ({vox_ml * 1000:.4g} mm³); fMRIPrep's own maps are the node's outputs in "
            "the run's working directory (FSL in fMRIPrep 25.2.6's image). *the same*: the "
            "three volumes equal FSL's to the last digit of float64; otherwise the largest "
            "relative difference of the three.",
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
            "probabilities and the binary class images written too), given to FSL 6.0.7.23 (as built, "
            "and with correctly rounded maths), to the larmorx-nc replica in its reproduce and "
            "default modes and at one thread, and to LARMORX's clean-room original in its "
            "reproduce and default modes at 14 threads and at one. "
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
            {"label": "Programs", "text": _programs(info, oinfo)},
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: threads and modes"],
            ["fmriprep", "LARMORX against fMRIPrep's own output"],
            ["variability", "FSL against itself: the yardstick"],
        ],
        "tables": tables,
        "metrics_note": _note(n_vox),
        "provenance": _provenance(info, e, oinfo, o, [data]),
    }


#: The real examples' texts: title, summary, the input panels (argument -> label, sub), data.
REAL_TEXT = {
    "bias-field": (
        "The bias field: FAST's default",
        "The record's case `opt/bias-outputs`: FAST's default segmentation, which estimates "
        "a smooth bias field along with the classes and divides it out at each iteration "
        "(`-b` writes the field, `-B` the corrected image, `-g` the binary classes), on "
        "ds000005's skull-stripped T1w at half resolution, by FSL 6.0.7.23 (as built and with "
        "correctly rounded maths), the replica and the clean-room original, each in its "
        "reproduce and default modes; choose the bias field above the panels.",
    ),
    "priors": (
        "FSL's tissue priors (-a), and a correction by default",
        "The record's case `priors/a-verbose`: `-a` with a standard-to-input matrix brings "
        "FSL's tissue priors (avg152T1's CSF, grey and white matter) onto the image with FLIRT "
        "and starts the segmentation from them, here on MNI152NLin2009cAsym's 2 mm T1w inside "
        "its brain mask; `-v` prints every step. FSL 6.0.7.23 and the replica's reproduce mode "
        "give the same bytes; the replica's default corrects FSL-09 in FLIRT's resampling of "
        "the priors, and so starts from other priors: the difference *replica, default − "
        "reproduce mode* shows what that changes. The clean-room original registers the priors "
        "with FLIRT's clean-room original in the same process, in both modes the same way.",
    ),
    "multi-channel": (
        "Three channels: T1w, T2w and BOLD reference",
        "The record's case `multi/s3`: `-S 3` segments three registered images together "
        "(MNI152NLin2009cAsym's 2 mm T1w, T2w and BOLD reference inside its brain mask; the "
        "last image named, the T1w, is channel 1, which names the outputs and orders the "
        "classes), each class a Gaussian in three dimensions with its 3 × 3 covariance, a bias "
        "field per channel (`-b`, `-B`); by FSL 6.0.7.23 (as built and with correctly rounded "
        "maths), the replica and the clean-room original, each in its reproduce and default "
        "modes. The matrix arithmetic need not follow LAPACK's bits in the clean-room original "
        "(specs/fast.md §8.6): its own LU decomposition.",
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
    o = original_entry(ctx, case)
    oinfo = o.info()
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
    arrays.update({pid: _load(o.dir / pid, files) for pid, *_ in ORIGINAL_PANELS})
    for pid, role, label, sub in _panel_order():
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    rdiffs, odiffs = _diffs(False), _diffs_original(False)
    diffs = rdiffs + odiffs
    outputs = _outputs(files, channels if len(images) > 1 else None)
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    bias = "-N" not in run.args
    _verdicts(rdiffs, same, "this case", info, bias=bias)
    _verdicts_original(odiffs, same, "this case", oinfo, {x["id"]: x["label"] for x in outputs})
    if ex_id == "priors":
        out_id, a, b = "pve_1", "replica-fix", "replica"
        panels = ["in1", "fsl", "replica", "replica-fix", "original-fix"]
        cap = (
            "the crosshair where the replica's default grey matter differs most from FSL's "
            "(the clean-room original's default makes the same correction)."
        )
        shown_diffs = ["replica-fsl", "fix-replica", "ofix-original"]
    elif ex_id == "bias-field":
        out_id, a, b = "bias", "fsl", "original"
        panels = ["in1", "fsl", "replica", "original"]
        what_differs = "bias field"
        shown_diffs = ["replica-fsl", "original-fsl"]
    else:
        out_id, a, b = "seg", "fsl", "original"
        panels = ["in1", "in2", "in3", "fsl", "replica", "original"]
        what_differs = "classes"
        shown_diffs = ["replica-fsl", "original-fsl"]
    at = _default_view(ex, "volume", a, b, out_id)
    if ex_id != "priors":
        cap = (
            f"the crosshair where the clean-room original's {what_differs} differs most from "
            "FSL's in the slice with the most brain (the replica agrees everywhere)."
            if _differs_at(ex, a, b, out_id, at)
            else "the crosshair at the brain's centre: no voxel of the slice with the most brain "
            f"differs in the {what_differs} between FSL and the clean-room original (the "
            "replica agrees everywhere)."
        )
    default = {
        "output": out_id,
        "plane": "axial",
        "voxel": at,
        "panels": panels,
        "diffs": shown_diffs,
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
            _counted(e, o),
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
                + "; the clean-room original's (FLIRT's clean-room original in the same "
                "process): reproduce mode "
                + _priors_same(pri, o.dir / "original")
                + ", default mode "
                + _priors_same(pri, o.dir / "original-fix")
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
            {"label": "Case", "text": f"`{case}` of both records (`{RECORD}`, `{RECORD_ORIG}`)"},
            {"label": "Command", "text": "`" + info["command"] + "`"},
            {"label": "Programs", "text": _programs(info, oinfo)},
            *facts_extra,
            {"label": "Grid", "text": common.describe_grid(first, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against FSL"],
            ["check", "LARMORX against itself: threads and modes"],
            ["variability", "FSL against itself: the yardstick"],
        ],
        "tables": tables,
        "metrics_note": _note(n_vox),
        "provenance": _provenance(info, e, oinfo, o, data),
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


def _worst(files: dict[str, dict], key: str) -> str:
    """The file whose comparison ``key`` differs in the most values, with its count."""
    k, x = max(files.items(), key=lambda kv: kv[1][key][0])
    return f"`{k}`: {x[key][0]:,} of {x['values']:,} values (at most {x[key][1]:.3g})"


def case_verdict_original(r: dict, c: dict) -> tuple[str, str]:
    """A case's verdict for the clean-room original: the record holds it to specs/fast.md
    §11's thresholds; a passing case is classified from its files against FSL as built, FSL with
    correctly rounded maths and FSL's own difference between the two (``c``: the case in the
    ``cases-original`` entry, with the original's default mode on it)."""
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
        if r["case"] == "opt/nan-first":
            why = (
                "Outside the record, a follow-up of the clean-room original: an image whose first "
                "voxel is NaN, whose extrema FAST takes as NaN and whose brain mask is then empty, "
                "so its k-means stops with exit status 255 (FSL-203, registered with the "
                "specification's answers of 2026-11-03); the original passes the case's "
                "other checks against FSL (5 of 6), and its default finishes (exit status 0) but "
                "names FSL-103 where the record asks for FSL-203 (FSL-203 follows in the "
                "original's next part). " + why
            )
        if r["case"] == "multi/s5":
            why = (
                "Outside §11's thresholds, explained: five channels of which the first is nearly "
                "a combination of the last two, so each class's 5 × 5 covariance is nearly "
                "singular and its inverse amplifies the matrix arithmetic's last bits; the "
                "original against itself with one rounding changed in the covariance's products "
                "moves further (_mixeltype Dice 0.865) than it lands from FSL (0.907), and FSL "
                "with correctly rounded maths stays at FSL's (0.99993). Following LAPACK's and "
                "OpenBLAS's operation order is the replica's job (specs/fast.md §8.6). " + why
            )
        return verdict, common.clip(why, 700)
    files = c.get("files") or {}
    rep = {**r.get("reported", {}), **r.get("measured", {})}
    notes = []
    fix = rep.get("D13 fix pass")
    shown = [n for n in named if n not in ("FSL-96", "FSL-97")]
    if isinstance(fix, int) and fix:
        notes.append(common.fix_note(fix, shown or named, "exp, log, expf and logf"))
    elif named and code not in (0, None):
        notes.append(
            f"its default mode refuses the call (exit {code}), naming {', '.join(named)}: an "
            "argument without meaning, which FSL uses as given"
        )
    elif named:
        notes.append(f"its default mode names {', '.join(named)}, with the same values")
    tail = ("; " + "; ".join(notes)) if notes else ""
    if not files:
        return "open", common.clip(
            "Within the record's thresholds (specs/fast.md §11); no image to classify" + tail, 700
        )
    n = len(files)
    of = {k: x for k, x in files.items() if x["orig_fsl"][0]}
    oc = {k: x for k, x in files.items() if x["orig_cr"][0]}
    beyond = {
        k: x
        for k, x in files.items()
        if x["orig_fsl"][0] > x["cr_fsl"][0] or x["orig_fsl"][1] > x["cr_fsl"][1]
    }
    # the original's distance from FSL with correctly rounded maths against FSL's own
    beyond_cr = {
        k: x
        for k, x in files.items()
        if x["orig_cr"][0] > x["cr_fsl"][0] or x["orig_cr"][1] > x["cr_fsl"][1]
    }
    if not of:
        cr = [k for k, x in files.items() if x["cr_fsl"][0]]
        return "bit-identical", common.clip(
            f"Every value of all {n} images FSL writes, with FSL's exit status and messages (the "
            "record checks the headers' grid, forms and data types)"
            + (
                f"; FSL with correctly rounded maths differs in {len(cr)} of them, so glibc's "
                "last bits matter here: the reproduce mode computes exp, log, expf and logf as "
                "glibc 2.35 does (since 2026-11-04)"
                if cr
                else ""
            )
            + tail,
            700,
        )
    if not oc:
        return "identical-cr", common.clip(
            "FSL with correctly rounded maths (CORE-MATH's exp, log, expf, logf preloaded into "
            f"the same program) gives the clean-room original's values in all {n} images; FSL "
            f"as built differs in {len(of)} (most: {_worst(of, 'orig_fsl')}), within the "
            "record's thresholds" + tail,
            700,
        )
    near = _near_fsl(of, "multi" in r["case"])
    if not beyond:
        return "within-variability", common.clip(
            f"In each of the {n} images the clean-room original differs from FSL in no more "
            "values, and by no more, than FSL with correctly rounded maths does (FSL's own "
            f"last-bit band): most {_worst(of, 'orig_fsl')}, FSL's own in that file "
            f"{_band_of(of)}; {near}" + tail,
            700,
        )
    if not beyond_cr:
        return "within-variability", common.clip(
            f"In each of the {n} images the clean-room original is no further from FSL with "
            "correctly rounded maths than FSL as built is (FSL's own last-bit band): "
            f"{near}; against FSL as built most {_worst(of, 'orig_fsl')}" + tail,
            700,
        )
    return "open", common.clip(
        f"Within the record's thresholds (specs/fast.md §11), but in {len(beyond)} of {n} "
        "images the clean-room original differs from FSL, and from FSL with correctly rounded "
        "maths, more than the two differ from each other: most "
        f"{_worst(beyond, 'orig_fsl')} against FSL as built, FSL's own in that file "
        f"{_band_of(beyond)}; {near}" + tail,
        700,
    )


def _near_fsl(of: dict[str, dict], multi: bool) -> str:
    """What the original's differences from FSL as built are (its reproduce mode computes
    glibc's exp, log, expf and logf since 2026-11-04, so FSL as built is its reference)."""
    total = sum(x["orig_fsl"][0] for x in of.values())
    text = f"it equals FSL as built but in {total:,} values of {len(of)} image(s)"
    if all(x["kind"] == "pve" and x["orig_fsl"][1] <= 0.0100001 for x in of.values()):
        text += (
            ", each a partial volume one fraction step (0.01) away: near ties between "
            "neighbouring fractions, decided by last bits"
            + (
                " (with several channels the clean-room original's own matrix arithmetic, not "
                "LAPACK's: specs/fast.md §8.6)"
                if multi
                else ""
            )
        )
    return text


def _band_of(files: dict[str, dict]) -> str:
    _k, x = max(files.items(), key=lambda kv: kv[1]["orig_fsl"][0])
    return f"{x['cr_fsl'][0]:,} values (at most {x['cr_fsl'][1]:.3g})"


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


#: The benchmark's series (``docs/benchmarks/fsl-fast.json``, from
#: ``validation/investigations/cleanroom-fast/bench.py``): FSL and the replica single-threaded.
SPEED_SERIES = [
    {"id": "FSL 6.0.7.23|1 thread", "label": "FSL 6.0.7.23 (single-threaded)", "mark": "original"},
    {"id": "replica (larmorx-nc)|1 thread", "label": "replica (single-threaded)", "mark": "replica"},
    {"id": "original (larmorx)|1 thread", "label": "clean-room original, 1 thread",
     "mark": "larmorx"},
    {"id": "original (larmorx)|14 threads", "label": "clean-room original, 14 threads",
     "mark": "larmorx-open"},
]  # fmt: skip


def speed(ctx: core.Context) -> tuple[list[dict], dict | None]:
    """The speed chart from the benchmark report (one run each, informal: its own format)."""
    import hashlib
    import json
    import types

    p = ctx.repo / "docs" / "benchmarks" / f"{TOOL}.json"
    if not p.is_file():
        return [], None
    data = json.loads(p.read_text(encoding="utf-8"))
    md = p.with_suffix(".md").read_text(encoding="utf-8")
    m = re.search(r"the main machine: ([^,]+),\s+(\d+)\s+cores\s+/\s+(\d+)\s+threads", md)
    cpu, cpus = (m.group(1), int(m.group(3))) if m else ("?", "?")
    results, labels = [], {}
    words = {run: w for _i, run, _v, w in FMRIPREP}
    for row in data["rows"]:
        run = row["run"]
        labels[run] = f"{words.get(run, run)}: {row['voxels']:,} voxels in the mask"
        results.append({"tool": "FSL 6.0.7.23", "operation": "1 thread", "input": run,
                        "median": row["fsl_seconds"]})  # fmt: skip
        results.append({"tool": "replica (larmorx-nc)", "operation": "1 thread", "input": run,
                        "median": row["replica_seconds"]})  # fmt: skip
        for t, sec in row["original_seconds"].items():
            op = "1 thread" if t == "1" else f"{t} threads"
            results.append({"tool": "original (larmorx)", "operation": op, "input": run,
                            "median": sec})  # fmt: skip
    bench = types.SimpleNamespace(
        results=results, data={"environment": {"cpu": cpu, "logical_cpus": cpus}}
    )
    chart = common.speed(
        bench,
        SPEED_SERIES,
        labels,
        title="Seconds for fMRIPrep's whole call (read the T1w brain, segment, write its twelve "
        "images), one run each, log scale; the factor is the clean-room original's speed-up "
        "over FSL at one thread",
        base=(SPEED_SERIES[0]["id"], SPEED_SERIES[2]["id"]),
        note="From the benchmark report `docs/benchmarks/fsl-fast.json` ({cpu}, {cpus} logical "
        "CPUs, shared with other work: informal, one run each). FSL's time is its run "
        "the oracle cache stored, not re-run; the replica's images are FSL's byte for byte, the "
        "clean-room original's within its record's thresholds and the same at any thread count.",
        inputs=[run for _i, run, _v, _w in FMRIPREP],
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


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, RECORD)
    examples = {
        **{f"fmriprep/{run}": ex_id for ex_id, run, _v, _w in FMRIPREP},
        **{case: ex_id for ex_id, case in REAL.items()},
    }
    ne = named_entry(ctx)
    cs = cases(rec, examples, ne.info().get("default_mode", {}))
    rec_orig = records.load(ctx.repo, RECORD_ORIG)
    co = cases_original_entry(ctx)
    add_original(cs, rec_orig, examples, co.info().get("cases", {}))
    entries = [fmriprep_entry(ctx, run) for _i, run, _v, _w in FMRIPREP]
    entries += [real_entry(ctx, case) for case in REAL.values()]
    entries.append(ne)
    entries += [original_entry(ctx, run) for _i, run, _v, _w in FMRIPREP]
    entries += [original_entry(ctx, case) for case in REAL.values()]
    entries.append(co)
    bench, chart = speed(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "FAST",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source() + rec_orig.source(),
            "benchmarks": bench,
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
        "examples": [_example_fmriprep(ctx, *FMRIPREP[0])]
        + [_example_real(ctx, ex_id) for ex_id in REAL]
        + [_example_fmriprep(ctx, *x) for x in FMRIPREP[1:]],
    }


if __name__ == "__main__":  # compute the cache entries only (``refresh WHY``: refresh_replica;
    # ``refresh-original WHY``: refresh_original)
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    if sys.argv[1:2] == ["refresh"]:
        refresh_replica(c, " ".join(sys.argv[2:]))
        sys.exit(0)
    if sys.argv[1:2] == ["refresh-original"]:
        refresh_original(c, " ".join(sys.argv[2:]))
        sys.exit(0)
    for _i, run, _v, _w in FMRIPREP:
        print("done", fmriprep_entry(c, run).dir)
    for case in REAL.values():
        print("done", real_entry(c, case).dir)
    print("done", named_entry(c).dir)
    for _i, run, _v, _w in FMRIPREP:
        print("done", original_entry(c, run).dir, flush=True)
    for case in REAL.values():
        print("done", original_entry(c, case).dir, flush=True)
    print("done", cases_original_entry(c).dir, flush=True)
