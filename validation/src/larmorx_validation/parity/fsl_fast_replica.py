# SPDX-License-Identifier: Apache-2.0
"""Parity of the FAST replica (``larmorx-nc fsl fast``, FSL Licence) with FSL 6.0.7.23's
``fast``, file by file and bit by bit.

The replica runs as a separate program (``LARMORX_NC_BIN``, else ``crates-nc/target/release``
or ``debug``, else ``larmorx-nc`` on the ``PATH``); this module imports nothing from it. Kinds of
cases:

- **fmriprep**: fMRIPrep 25.2.6's own segmentation calls (``anat_fit_wf/fast``: ``fast -I 0 -N -p
  -g -S 1 <T1w brain>``) of the oracle runs, their input taken from the runs' working
  directories and copied into the run directory, as fMRIPrep's node holds it; the standard runs
  in the standard tier, every call in the full tier; fMRIPrep's own outputs (its container's
  FSL) compared too, as a report;
- **real**: fMRIPrep's call on derived images (the smoke tier's half-resolution T1w).

Every case compares, with the replica in ``reproduce`` mode (D13): the exit status, standard
output and error (the run directory and FSL's program path replaced, the replica's
``larmorx-nc:`` lines left out), the set of files written and every file's bytes (images
decompressed, ``descrip`` left out). A second pass runs the default ``fix`` mode and checks D13's
rule. Every original run goes through the oracle-output cache (:func:`oracle_run`): FAST draws
its random numbers from the C library's ``rand`` with a fixed seed, so a run is served once it
ran.
"""

from __future__ import annotations

import functools
import shutil
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity import defects, fsl_versions
from larmorx_validation.parity.fsl_flirt_replica import (
    _agree_on_errors,
    _environment,
    _save,
    _strided,
    compare,
    fix_pass,
    fsldir,
    run_replica,
)
from larmorx_validation.parity.fsl_mcflirt_replica import (
    Completed,
    _differing_values,
    _same_streams,
    replica_binary,
    workspace,
)
from larmorx_validation.parity.harness import Case, Check, CheckList, Outcome, Suite

__all__ = ["replica_binary", "suite"]

SUITE = "fsl-fast-replica"
PROGRAM = "fast"
FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
#: The fMRIPrep runs of the standard tier.
STANDARD_RUNS = ("ds000005-sub-01-noreconall",)
#: The run the derived inputs come from.
SOURCE_RUN = "ds000005-sub-01-noreconall"


# ------------------------------------------------------------------------------------------------
# The programs


def oracle_run(args: list[str], *, cwd: Path, settings: dict[str, str | None]):
    """FSL's ``fast`` with ``args`` in ``cwd``, through the oracle-output cache: FAST is
    single-threaded and seeds the C library's ``rand`` with a constant, so a run is served once
    it ran; every file it writes is in ``cwd`` and every file there counts by its content."""
    from larmorx_validation import oracle_cache

    argv = [str(fsldir() / "bin" / PROGRAM), *args]
    return oracle_cache.run(
        argv,
        env=_environment(settings),
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner=SUITE,
        inputs=[p for p in sorted(cwd.rglob("*")) if p.is_file()],
        timeout=7200,
    )


def run_oracle(args: list[str], *, cwd: Path, settings: dict[str, str | None]) -> Completed:
    p = oracle_run(args, cwd=cwd, settings=settings)
    return Completed(p.returncode, p.stdout, p.stderr)


# ------------------------------------------------------------------------------------------------
# Inputs


def fmriprep_calls() -> list[tuple[str, Path]]:
    """``(run, fast node directory)`` of every FAST call of the fMRIPrep oracle runs."""
    ws = workspace()
    if ws is None:
        return []
    out = []
    for cmd in sorted((ws / FMRIPREP_RUNS).glob("*/work/*/*/anat_fit_wf/fast/command.txt")):
        run = cmd.relative_to(ws / FMRIPREP_RUNS).parts[0]
        out.append((run, cmd.parent))
    return out


def fmriprep_args(node: Path) -> list[str]:
    """fMRIPrep's call as the suite runs it: the input named by its file name (the suite copies
    it into the run directory, where fMRIPrep's node held it)."""
    argv = (node / "command.txt").read_text(encoding="utf-8").split()[1:]
    return [*argv[:-1], Path(argv[-1]).name]


@functools.cache
def fmriprep_input(run: str, node: str) -> Path | None:
    """The T1w brain fMRIPrep segmented: the file of that name in the run's working directory
    (the node itself keeps only its outputs)."""
    name = fmriprep_args(Path(node))[-1]
    work = Path(node).parents[3]
    for p in sorted(work.rglob(name)):
        if Path(node) not in p.parents and p.is_file():
            return p
    return None


@functools.cache
def inputs() -> Path:
    """The derived inputs (``oracles/inputs/fsl-fast-replica/``), made once, deterministically,
    from ds000005's fMRIPrep run: its T1w brain at half resolution."""
    import nibabel as nib

    ws = workspace()
    assert ws is not None
    out = ws / "oracles" / "inputs" / SUITE
    stamp = out / "DONE-v1"
    if stamp.is_file():
        return out
    out.mkdir(parents=True, exist_ok=True)
    call = next((n for r, n in fmriprep_calls() if r == SOURCE_RUN), None)
    assert call is not None, f"{SOURCE_RUN}'s FAST node is missing"
    src = fmriprep_input(SOURCE_RUN, str(call))
    assert src is not None
    t1 = nib.load(str(src))
    t, ta = _strided(t1, 2)
    _save(out / "t1_half.nii.gz", np.asarray(t, np.float32), ta)
    stamp.write_text("derived inputs of the fsl-fast-replica suite, version 1\n", encoding="utf-8")
    return out


# ------------------------------------------------------------------------------------------------
# Cases


@dataclass(frozen=True)
class Run:
    """A case: the arguments (``@name`` names a derived input copied into the run directory),
    the environment."""

    args: tuple[str, ...]
    settings: tuple[tuple[str, str | None], ...] = ()
    fix: bool = True


def _prepare(run: Run, tmp: Path) -> list[str]:
    out = []
    for a in run.args:
        if a.startswith("@"):
            src = inputs() / a[1:]
            shutil.copy(src, tmp / src.name)
            out.append(src.name)
        else:
            out.append(a)
    return out


def _run_case(run: Run, checks: CheckList) -> None:
    settings = dict(run.settings)
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-fast-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-fast-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = _prepare(run, a)
        _prepare(run, b)
        oracle = run_oracle(args, cwd=a, settings=settings)
        replica = run_replica(PROGRAM, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
        if run.fix and replica.code == 0:
            fix_pass(checks, PROGRAM, lambda t: _prepare(run, t), args, settings, b, replica)
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node = payload
    src = fmriprep_input(run, node)
    if src is None:
        raise Outcome("skipped", f"{run}: the input of fMRIPrep's FAST node is not available")
    args = fmriprep_args(Path(node))
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-fast-fp-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-fast-fp-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        shutil.copy(src, a / args[-1])
        shutil.copy(src, b / args[-1])
        oracle = run_oracle(args, cwd=a, settings={})
        replica = run_replica(PROGRAM, args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
        _fmriprep_outputs(checks, Path(node), b)
        if oracle.code == 0 and replica.code == 0:

            def prepare(t: Path) -> None:
                shutil.copy(src, t / args[-1])

            fix_pass(checks, PROGRAM, prepare, args, {}, b, replica)
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


def _fmriprep_outputs(checks: CheckList, node: Path, replica_dir: Path) -> None:
    """fMRIPrep's own outputs (its container's FSL), as a report."""
    theirs = sorted(node.glob("*_pve*.nii.gz"))
    same, total, notes = 0, 0, []
    for t in theirs:
        ours = replica_dir / t.name
        if not ours.is_file():
            continue
        total += 1
        if _same_streams(t, ours, blank_descrip=True):
            same += 1
        else:
            notes.append(f"{t.name}: {_differing_values(t, ours)}")
    checks.add(
        Check(
            "fMRIPrep's outputs (its container's FSL) identical",
            True,
            metric="files",
            value=f"{same}/{total}",
            threshold="reported",
            detail="; ".join(notes[:4]),
        )
    )


#: The real cases: name -> (tier, description, run).
def _real_cases() -> dict[str, tuple[str, str, Run]]:
    c: dict[str, tuple[str, str, Run]] = {}

    def add(name: str, tier: str, desc: str, *args: str, **kw: Any) -> None:
        c[name] = (tier, desc, Run(tuple(args), **kw))

    add(
        "real/fmriprep-call-half",
        "smoke",
        "fMRIPrep's call (-I 0 -N -p -g -S 1) on ds000005's T1w brain at half resolution",
        "-I",
        "0",
        "-N",
        "-p",
        "-g",
        "-S",
        "1",
        "@t1_half.nii.gz",
    )
    return c


def _all_cases() -> dict[str, tuple[str, str, Run]]:
    return {**_real_cases()}


def cases(tier: str) -> list[Case]:
    from larmorx_validation.parity.harness import in_tier

    out = []
    for run, node in fmriprep_calls():
        t = "standard" if run in STANDARD_RUNS else "full"
        if in_tier(t, tier):
            out.append(
                Case(
                    f"fmriprep/{run}",
                    "fmriprep",
                    "fMRIPrep's segmentation call",
                    ("fmriprep", (run, str(node))),
                )
            )
    for name, (t, desc, _run) in _all_cases().items():
        if in_tier(t, tier):
            out.append(Case(name, name.split("/")[0], desc, ("run", name)))
    return out


def run_case(case: Case, checks: CheckList) -> None:
    kind, payload = case.payload
    if kind == "fmriprep":
        _fmriprep_case(payload, checks)
    elif kind == "direct":
        # a Run given whole (differential random testing's cases)
        _run_case(payload, checks)
    else:
        _run_case(_all_cases()[payload][2], checks)


def suite() -> Suite:
    return Suite(
        name=SUITE,
        title="FAST replica (bit-exact, FSL Licence)",
        tool="`larmorx-nc fsl fast` (crate larmorx-nc-fast, translated from FSL "
        f"{fsl_versions.NEWEST}'s source)",
        reference=f"fast from FSL {fsl_versions.NEWEST} (fsl-fast4 2111.3, fsl-newimage 2601.0, "
        "fsl-miscmaths 2412.6, fsl-utils 2412.6; the conda build at `~/fsl-6.0.7.23`, as built: "
        "glibc 2.35's maths and random numbers)",
        thresholds=[
            ("Exit status", "the same"),
            (
                "Standard output and error",
                "identical (the run directory and FSL's program path replaced; the replica's "
                "`larmorx-nc:` lines left out)",
            ),
            ("Files written", "the same names"),
            ("Every file", "bit-identical (images decompressed; `descrip` left out)"),
            ("D13 fix pass", "a defect named exactly when the default's outputs differ"),
        ],
        cases=cases,
        run_case=run_case,
        packages=("nibabel",),
        notes=(
            "The replica runs as a separate program, the FSL replica package's (`larmorx-nc`), "
            "compared with FSL in `reproduce` mode (D13); the default `fix` pass is checked "
            "against the reproduced run. fMRIPrep's own outputs come from its container's FSL "
            "and are reported only. The derived inputs are made once from ds000005's fMRIPrep "
            "run (`oracles/inputs/fsl-fast-replica/`)."
        ),
    )
