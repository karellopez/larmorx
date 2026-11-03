# SPDX-License-Identifier: Apache-2.0
"""Parity of the topup replica (``larmorx-nc fsl topup``, FSL Licence) with FSL 6.0.7.23's
``topup``, file by file and bit by bit.

The replica runs as a separate program (``LARMORX_NC_BIN``, else ``crates-nc/target/release``
or ``debug``, else ``larmorx-nc`` on the ``PATH``); this module imports nothing from it. Kinds of
cases:

- **fmriprep**: fMRIPrep 25.2.6's own field-map estimations (sdcflows' PEPOLAR workflow,
  ``fmap_preproc_wf/<estimator>/topup``: ``topup --config=b02b0.cnf --datain=<encfile>
  --imain=<merged b0s> --out=<base> --iout --fout --jacout=jac --logout --rbmout=xfm
  --dfout=warpfield``) of the oracle runs: the merged images from the runs' working
  directories, sdcflows' ``b02b0.cnf`` (the workspace's sdcflows clone, the same bytes as
  fMRIPrep 25.2.6's container holds) and the encoding file nipype writes (``%d %d %d %.8f``
  per volume, from the node's recorded directions and read-out times), each copied into the
  run directory under the name fMRIPrep's node gave it; fMRIPrep's own outputs (its
  container's FSL) compared too, as a report;
- **debug**: the same images with one short level and ``--debug`` and ``-v`` (every debug file
  of every cost-function evaluation: the corrected images, Jacobians, fields, masks, masked
  differences, gradients and the Hessian's weights), without and with the movements.

Every case compares, with the replica in ``reproduce`` mode (D13): the exit status, standard
output and error (the run directory and FSL's program path replaced, the replica's
``larmorx-nc:`` lines left out), the set of files written and every file's bytes (images
decompressed, ``descrip`` left out). A second pass runs the default ``fix`` mode and checks D13's
rule. Every original run goes through the oracle-output cache (:func:`oracle_run`): topup at
``--nthr=1`` is deterministic, and it runs for minutes, so each run is made once.
"""

from __future__ import annotations

import json
import shutil
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from larmorx_validation.parity import defects, fsl_versions
from larmorx_validation.parity.fsl_flirt_replica import (
    _agree_on_errors,
    _environment,
    compare,
    fix_pass,
    fsldir,
    run_replica,
)
from larmorx_validation.parity.fsl_mcflirt_replica import (
    Completed,
    _differing_values,
    _same_bytes,
    _same_streams,
    replica_binary,
    workspace,
)
from larmorx_validation.parity.harness import Case, Check, CheckList, Outcome, Suite

__all__ = ["replica_binary", "suite"]

SUITE = "fsl-topup-replica"
PROGRAM = "topup"
FMRIPREP_RUNS = "oracles/fmriprep-25.2.6"
#: The fMRIPrep runs of the standard tier: ds005040 (Philips 3 T, a PEPOLAR pair along y,
#: 64 x 64 x 40 voxels of 3 x 3 x 3.3 mm).
STANDARD_RUNS = ("ds005040-sub-002-noreconall",)
#: The run the debug cases' images come from.
SOURCE_RUN = "ds005040-sub-002-noreconall"
#: sdcflows' topup configuration, in the workspace's sdcflows clone (2.15.2; byte-identical to
#: fMRIPrep 25.2.6's ``/app/.pixi/envs/fmriprep/lib/python3.12/site-packages/sdcflows/data/
#: flirtsch/b02b0.cnf``, MD5 025014fc26cf49e82840f6971d35cd11).
SDCFLOWS_CONFIG = "sdcflows/sdcflows/data/flirtsch/b02b0.cnf"


# ------------------------------------------------------------------------------------------------
# The programs


def oracle_run(args: list[str], *, cwd: Path, settings: dict[str, str | None], never: str = ""):
    """FSL's ``topup`` with ``args`` in ``cwd``, through the oracle-output cache: topup at one
    thread is deterministic, so a run is served once it ran (unless ``never`` says why not);
    every file it writes is in ``cwd`` and every file there counts by its content."""
    from larmorx_validation import oracle_cache

    argv = [str(fsldir() / "bin" / PROGRAM), *args]
    return oracle_cache.run(
        argv,
        env=_environment(settings),
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="fsl").but_never(never),
        runner=SUITE,
        inputs=[p for p in sorted(cwd.rglob("*")) if p.is_file()],
        timeout=36000,
    )


def run_oracle(
    args: list[str], *, cwd: Path, settings: dict[str, str | None], never: str = ""
) -> Completed:
    p = oracle_run(args, cwd=cwd, settings=settings, never=never)
    return Completed(p.returncode, p.stdout, p.stderr)


# ------------------------------------------------------------------------------------------------
# Inputs


def fmriprep_calls() -> list[tuple[str, Path]]:
    """``(run, topup node directory)`` of every topup call of the fMRIPrep oracle runs."""
    ws = workspace()
    if ws is None:
        return []
    out = []
    pattern = "*/work/*/*/fmap_preproc_wf/*/topup/command.txt"
    for cmd in sorted((ws / FMRIPREP_RUNS).glob(pattern)):
        run = cmd.relative_to(ws / FMRIPREP_RUNS).parts[0]
        out.append((run, cmd.parent))
    return out


def _node_inputs(node: Path) -> dict[str, Any]:
    """The node's recorded inputs (nipype's ``_0x<hash>.json``: a list of name, value pairs)."""
    record = next(iter(sorted(node.glob("_0x*.json"))))
    return {k: v for k, v in json.loads(record.read_text(encoding="utf-8"))}


def fmriprep_args(node: Path) -> list[str]:
    """fMRIPrep's call as the suite runs it: the configuration, encoding file and images named
    by their file names (the suite copies them into the run directory)."""
    argv = (node / "command.txt").read_text(encoding="utf-8").split()[1:]
    out = []
    for a in argv:
        key, sep, value = a.partition("=")
        if sep and key in ("--config", "--datain", "--imain"):
            a = f"{key}={Path(value).name}"
        out.append(a)
    return out


def _encfile(node: Path) -> str:
    """nipype's ``TOPUP._generate_encfile``: per volume the direction's three components and the
    read-out time, ``np.savetxt(fmt="%d %d %d %.8f")``."""
    rec = _node_inputs(node)
    lines = []
    for d, t in zip(rec["encoding_direction"], rec["readout_times"], strict=True):
        sign = -1 if d.endswith("-") else 1
        comp = [sign * int(axis == d[0]) for axis in "xyz"]
        lines.append(f"{comp[0]} {comp[1]} {comp[2]} {float(t):.8f}\n")
    return "".join(lines)


def _fmriprep_image(node: Path) -> Path | None:
    """The merged reversed-phase-encoding images the node estimated from (the setwise
    average's output in the run's working directory)."""
    in_file = Path(_node_inputs(node)["in_file"][0])
    work = node.parents[4]
    p = work / in_file.relative_to("/work")
    return p if p.is_file() else None


def _prepare_fmriprep(node: Path, d: Path) -> bool:
    """The three inputs of fMRIPrep's call, under their names, in ``d``."""
    ws = workspace()
    img = _fmriprep_image(node)
    if ws is None or img is None:
        return False
    args = fmriprep_args(node)
    names = {a.partition("=")[0]: a.partition("=")[2] for a in args if "=" in a}
    shutil.copy(img, d / names["--imain"])
    shutil.copy(ws / SDCFLOWS_CONFIG, d / names["--config"])
    (d / names["--datain"]).write_text(_encfile(node), encoding="utf-8")
    return True


# ------------------------------------------------------------------------------------------------
# Cases


@dataclass(frozen=True)
class Run:
    """A case on the source run's fMRIPrep inputs: the arguments (``{imain}`` and ``{datain}``
    are the images' and the encoding file's names), the environment."""

    args: tuple[str, ...]
    settings: tuple[tuple[str, str | None], ...] = ()
    fix: bool = True


def _source_node() -> Path | None:
    return next((n for r, n in fmriprep_calls() if r == SOURCE_RUN), None)


def _run_args(run: Run, node: Path) -> list[str]:
    args = fmriprep_args(node)
    names = {a.partition("=")[0]: a.partition("=")[2] for a in args if "=" in a}
    return [a.format(imain=names["--imain"], datain=names["--datain"]) for a in run.args]


def _run_case(run: Run, checks: CheckList) -> None:
    node = _source_node()
    if node is None:
        raise Outcome("skipped", f"{SOURCE_RUN}'s topup node is not available")
    settings = dict(run.settings)
    args = _run_args(run, node)
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-topup-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-topup-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        if not (_prepare_fmriprep(node, a) and _prepare_fmriprep(node, b)):
            raise Outcome("skipped", f"{SOURCE_RUN}'s topup inputs are not available")
        oracle = run_oracle(args, cwd=a, settings=settings)
        replica = run_replica(PROGRAM, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
        if run.fix and replica.code == 0:
            fix_pass(
                checks,
                PROGRAM,
                lambda t: _prepare_fmriprep(node, t),
                args,
                settings,
                b,
                replica,
            )
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


def _fmriprep_case(payload: tuple[str, str], checks: CheckList) -> None:
    run, node_s = payload
    node = Path(node_s)
    args = fmriprep_args(node)
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-topup-fp-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-topup-fp-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        if not (_prepare_fmriprep(node, a) and _prepare_fmriprep(node, b)):
            raise Outcome("skipped", f"{run}: the inputs of fMRIPrep's topup node are missing")
        oracle = run_oracle(args, cwd=a, settings={})
        replica = run_replica(PROGRAM, args, cwd=b, settings={}, mode=defects.REPRODUCE)
        compare(checks, PROGRAM, oracle, replica, a, b)
        _fmriprep_outputs(checks, node, b)
        if oracle.code == 0 and replica.code == 0:
            fix_pass(checks, PROGRAM, lambda t: _prepare_fmriprep(node, t), args, {}, b, replica)
    _agree_on_errors(checks, PROGRAM, oracle.code, oracle.stderr)


def _fmriprep_outputs(checks: CheckList, node: Path, replica_dir: Path) -> None:
    """fMRIPrep's own outputs (its container's FSL, an older topup), as a report."""
    same, total, notes = 0, 0, []
    for t in sorted(node.iterdir()):
        if not (t.name.endswith((".nii.gz", ".mat")) and (replica_dir / t.name).is_file()):
            continue
        ours = replica_dir / t.name
        total += 1
        ok = (
            _same_streams(t, ours, blank_descrip=True)
            if t.name.endswith(".nii.gz")
            else _same_bytes(t, ours)
        )
        if ok:
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


#: The cases on the source run's inputs: name -> (tier, description, run).
def _all_cases() -> dict[str, tuple[str, str, Run]]:
    c: dict[str, tuple[str, str, Run]] = {}

    def add(name: str, tier: str, desc: str, *args: str, **kw: Any) -> None:
        c[name] = (tier, desc, Run(tuple(args), **kw))

    add(
        "debug/one-level",
        "smoke",
        "one level (subsampling 2, 3 Levenberg-Marquardt iterations) with --debug=2 and -v: "
        "every cost-function evaluation's debug images, gradients and the Hessian's weights",
        "--imain={imain}",
        "--datain={datain}",
        "--out=dbg",
        "--warpres=14",
        "--subsamp=2",
        "--fwhm=6",
        "--miter=3",
        "--lambda=0.001",
        "--ssqlambda=1",
        "--regmod=bending_energy",
        "--estmov=0",
        "--minmet=0",
        "--splineorder=3",
        "--numprec=double",
        "--interp=spline",
        "--scale=1",
        "--debug=2",
        "--iout=dbg_corr",
        "--fout=dbg_field",
        "-v",
    )
    add(
        "debug/movements",
        "smoke",
        "one level with the movements estimated (five parameters of the reversed scan: the "
        "translation along the phase encoding left out) by Levenberg-Marquardt, --debug=1 and "
        "-v; the rigid-body matrices",
        "--imain={imain}",
        "--datain={datain}",
        "--out=mov",
        "--warpres=14",
        "--subsamp=2",
        "--fwhm=6",
        "--miter=3",
        "--lambda=0.001",
        "--estmov=1",
        "--minmet=0",
        "--debug=1",
        "--iout=mov_corr",
        "--rbmout=mov_xfm",
        "-v",
    )
    return c


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
                    "fMRIPrep's field-map estimation call (sdcflows' b02b0.cnf)",
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
        _run_case(payload, checks)
    else:
        _run_case(_all_cases()[payload][2], checks)


def suite() -> Suite:
    return Suite(
        name=SUITE,
        title="topup replica (bit-exact, FSL Licence)",
        tool="`larmorx-nc fsl topup` (crate larmorx-nc-topup, translated from FSL "
        f"{fsl_versions.NEWEST}'s source)",
        reference=f"topup from FSL {fsl_versions.NEWEST} (fsl-topup 2203.5, fsl-basisfield "
        "2203.4, fsl-warpfns 2501.0, fsl-newimage 2601.0, fsl-miscmaths 2412.6, fsl-utils "
        "2412.6, OpenBLAS 0.3.33 on one thread; the conda build at `~/fsl-6.0.7.23`, as built)",
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
            "(an older topup) and are reported only. The inputs are fMRIPrep's: the merged "
            "images of its working directory, sdcflows' configuration and the encoding file "
            "nipype writes."
        ),
    )
