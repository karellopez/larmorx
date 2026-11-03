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
  differences, gradients and the Hessian's weights), without and with the movements;
- **opt**, **cfg**, **err**, **thr**: every option of topup, FSL's configurations, the messages
  and failures, the thread counts, on the half-resolution pair;
- **at**: applytopup (``larmorx-nc fsl applytopup``) on topup's own outputs (FSL's, from the
  cache): least-squares restoration and Jacobian modulation, every option, message and failure.

Every case compares, with the replica in ``reproduce`` mode (D13): the exit status, standard
output and error (the run directory and FSL's program path replaced, the replica's
``larmorx-nc:`` lines left out), the set of files written and every file's bytes (images
decompressed, ``descrip`` left out). A second pass runs the default ``fix`` mode and checks D13's
rule. Every original run goes through the oracle-output cache (:func:`oracle_run`): topup at
``--nthr=1`` is deterministic, and it runs for minutes, so each run is made once.
"""

from __future__ import annotations

import json
import os
import re
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


def oracle_run(
    args: list[str],
    *,
    cwd: Path,
    settings: dict[str, str | None],
    never: str = "",
    threads: int = 1,
    timeout: int = 36000,
    program: str = PROGRAM,
):
    """FSL's ``program`` (``topup`` or ``applytopup``) with ``args`` in ``cwd``, through the oracle-output cache: topup at one
    thread is deterministic, so a run is served once it ran (unless ``never`` says why not);
    every file it writes is in ``cwd`` and every file there counts by its content."""
    from larmorx_validation import oracle_cache

    argv = [str(fsldir() / "bin" / program), *args]
    return oracle_cache.run(
        argv,
        env=_environment(settings),
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=threads, family="fsl").but_never(never),
        runner=SUITE,
        inputs=[p for p in sorted(cwd.rglob("*")) if p.is_file()],
        timeout=timeout,
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


def _source_node() -> Path | None:
    return next((n for r, n in fmriprep_calls() if r == SOURCE_RUN), None)


# ------------------------------------------------------------------------------------------------
# Derived inputs

#: The derived inputs' version: a new version makes them again.
INPUTS_VERSION = 1
#: ds005040's read-out time (s), as nipype's encoding file writes it.
RT = "0.01379510"
#: The text inputs: acquisition-parameter files (one row per volume: the phase-encoding vector
#: and the read-out time) and configuration files.
TEXTS = {
    "acq.txt": f"0 1 0 {RT}\n0 -1 0 {RT}\n",
    "acq_x.txt": f"1 0 0 {RT}\n-1 0 0 {RT}\n",
    "acq_four.txt": f"0 1 0 {RT}\n0 -1 0 {RT}\n1 0 0 {RT}\n-1 0 0 {RT}\n",
    "acq_t.txt": f"0 0\n1 -1\n0 0\n{RT} {RT}\n",
    "acq_oblique.txt": f"0.6 0.8 0 {RT}\n-0.6 -0.8 0 {RT}\n",
    "acq_z.txt": f"0 0 1 {RT}\n0 0 -1 {RT}\n",
    "acq_same.txt": f"0 1 0 {RT}\n0 1 0 {RT}\n",
    "acq_rts.txt": f"0 1 0 {RT}\n0 -1 0 0.02000000\n",
    "acq_zero.txt": f"0 1 0 {RT}\n0 -1 0 0\n",
    "acq_two_zero.txt": "0 1 0 0\n0 -1 0 0\n",
    "acq_long_rt.txt": "0 1 0 0.5\n0 -1 0 0.5\n",
    "acq_norm.txt": f"0 0.9 0 {RT}\n0 -1 0 {RT}\n",
    "acq_three.txt": f"0 1 0 {RT}\n0 -1 0 {RT}\n0 1 0 {RT}\n",
    "acq_five.txt": f"0 1 0 {RT} 1\n0 -1 0 {RT} 1\n",
    "acq_text.txt": "zero one zero time\nzero minus zero time\n",
    "acq_one.txt": f"0 1 0 {RT}\n",
    "short.cnf": (
        "# two levels: Levenberg-Marquardt with the movements, then scaled conjugate gradients\n"
        "--warpres=20,14\n--subsamp=2,1\n--fwhm=8,4\n--miter=3,2\n--lambda=0.005,0.0001\n"
        "--ssqlambda=1\n--regmod=bending_energy\n--estmov=1,0\n--minmet=0,1\n"
        "--splineorder=3\n--numprec=double\n--interp=spline\n--scale=1\n"
    ),
    "bad_option.cnf": "--warpres=20\n--bogus=1\n",
    # a comment line of more than 1023 characters after its first word, then two settings
    "long_line.cnf": "# " + "word " * 250 + "\n--warpres=20\n--miter=2\n",
}


def _save(path: Path, data: np.ndarray, affine: np.ndarray, dtype: Any = np.float32) -> Any:
    import nibabel as nib

    img = nib.Nifti1Image(np.asarray(data, dtype=dtype), affine)
    img.header.set_xyzt_units("mm", "sec")
    img.set_qform(affine, 1)
    img.set_sform(affine, 1)
    nib.save(img, str(path))
    return img


def inputs() -> Path:
    """The derived inputs (``oracles/inputs/fsl-topup-replica/``), made once, deterministically,
    from ds005040's merged pair (64 x 64 x 40, phase encoding along y): ``pair`` (the pair as
    fMRIPrep merged it), ``half`` (every second voxel: 32 x 32 x 20 of 6 x 6 x 6.6 mm, the
    option cases' image), ``xhalf`` (x and y exchanged: phase encoding along x), ``four`` (both
    pairs: four volumes), ``half_i16`` (16-bit integers, a quarter of the values), ``crop``
    (30 x 30 x 18: divisible by 2 and 3), ``one`` (one volume, 3D), ``half_intent`` (intent
    parameters in the header); and :data:`TEXTS`."""
    ws = workspace()
    assert ws is not None
    out = ws / "oracles" / "inputs" / SUITE
    stamp = out / f"DONE-v{INPUTS_VERSION}"
    if stamp.is_file():
        return out
    import nibabel as nib

    out.mkdir(parents=True, exist_ok=True)
    node = _source_node()
    assert node is not None, f"{SOURCE_RUN}'s topup node is missing"
    img = _fmriprep_image(node)
    assert img is not None
    src = nib.load(str(img))
    full = np.asarray(src.dataobj, np.float32)
    _save(out / "pair.nii.gz", full, src.affine)
    half = full[::2, ::2, ::2]
    aff = src.affine.copy()
    aff[:3, :3] = aff[:3, :3] * 2
    _save(out / "half.nii.gz", half, aff)
    xhalf = np.ascontiguousarray(np.transpose(half, (1, 0, 2, 3)))
    _save(out / "xhalf.nii.gz", xhalf, aff)
    _save(out / "four.nii.gz", np.concatenate([half, xhalf], axis=3), aff)
    _save(out / "half_i16.nii.gz", np.round(half * 0.25), aff, np.int16)
    _save(out / "crop.nii.gz", half[1:31, 1:31, 1:19], aff)
    _save(out / "one.nii.gz", half[..., 0], aff)
    intent = nib.Nifti1Image(half, aff)
    intent.header.set_xyzt_units("mm", "sec")
    intent.set_qform(aff, 1)
    intent.set_sform(aff, 1)
    intent.header.set_intent("estimate", (), name="")
    intent.header["intent_p1"] = 1.5
    intent.header["intent_p2"] = 2.5
    intent.header["intent_p3"] = 3.5
    nib.save(intent, str(out / "half_intent.nii.gz"))
    for name, text in TEXTS.items():
        (out / name).write_text(text, encoding="utf-8")
    stamp.write_text(
        f"derived inputs of the {SUITE} suite, version {INPUTS_VERSION}\n", encoding="utf-8"
    )
    return out


# ------------------------------------------------------------------------------------------------
# Cases


@dataclass(frozen=True)
class Run:
    """A case: the arguments (``{imain}`` and ``{datain}`` name fMRIPrep's images and encoding
    file on the source run, copied into the run directory with sdcflows' configuration;
    ``@name`` a derived input, copied too), the environment."""

    args: tuple[str, ...]
    settings: tuple[tuple[str, str | None], ...] = ()
    fix: bool = True
    #: Derived inputs copied into the run directory although no argument names them.
    files: tuple[str, ...] = ()
    #: Why FSL's run is never cached.
    never: str = ""
    #: FSL's thread count (``--nthr``, ``FSL_NUM_THREADS``, OpenBLAS's) for the cache's policy.
    threads: int = 1
    #: ``--profile``: the profile's durations (the clock's) are masked before the files are
    #: compared.
    profile: bool = False
    #: A thread-dependence case: the replica's run is compared with FSL's run of these
    #: arguments and settings at one thread (D13 class (b)); FSL's own run of :attr:`args` is
    #: measured against it.
    single: tuple[tuple[str, ...], tuple[tuple[str, str | None], ...]] | None = None
    #: A case FSL runs or stops on that the default refuses (rules B and A): the replica's
    #: ``fix`` run exits 1 with a refusal naming this entry.
    fix_refuses: str = ""
    #: A case FSL runs or stops on that the default corrects (rule A): the replica's ``fix``
    #: run finishes (exit 0), naming this entry.
    fix_finishes: str = ""
    #: A case whose default run prints this text on standard error (a corrected message).
    fix_says: str = ""
    #: A case the replica refuses in both modes, naming this entry, where FSL needs more memory
    #: than the machine holds: FSL is not run (its run was measured once; the reason).
    refused: str = ""
    refused_reason: str = ""
    #: Seconds before FSL's run is stopped.
    timeout: int = 36000
    #: The program: ``topup`` or ``applytopup``.
    program: str = PROGRAM
    #: applytopup's cases: the topup run whose outputs (FSL's, from the cache) are copied in as
    #: ``tu_fieldcoef.nii.gz``, ``tu_movpar.txt`` and ``tu_field.nii.gz`` (:data:`TOPUP_SOURCES`).
    source: str = ""
    #: Files of :attr:`source` left out.
    drop: tuple[str, ...] = ()


#: The entries the ``fix`` pass may name without a visible change: applytopup's FSL-132 (the
#: first volume's interpolation) and FSL-133 (the lines' wrap) change values that the
#: restoration and the output's single precision can round back to the same numbers.
SILENT: tuple[str, ...] = ("FSL-132", "FSL-133")
#: How a profile's durations are masked.
_DURATION = re.compile(r"Duration: \S+")
#: The profile's name after ``--out``'s base name (``"_" + __FILE__ + "_" + __func__ + ".txt"``).
PROFILE_SUFFIX = ".topup_profile_topup.cpp_main.txt"


def _prepare(run: Run, d: Path, node: Path | None) -> list[str]:
    """``run``'s inputs in ``d``; its arguments as the programs get them."""
    if any("{" in a for a in run.args) and (node is None or not _prepare_fmriprep(node, d)):
        raise Outcome("skipped", f"{SOURCE_RUN}'s topup inputs are not available")
    if run.program == "applytopup":
        applytopup_inputs()
    out = []
    for a in run.args:
        if node is not None:
            names = {k: v for k, _, v in (x.partition("=") for x in fmriprep_args(node))}
            a = a.format(imain=names["--imain"], datain=names["--datain"])
        key, sep, value = a.partition("=")
        if sep and value.startswith("@"):
            # a comma-separated list (applytopup's --imain): each @name copied
            parts = []
            for part in value.split(","):
                if part.startswith("@"):
                    shutil.copy(inputs() / part[1:], d / part[1:])
                    part = part[1:]
                parts.append(part)
            a = f"{key}={','.join(parts)}"
        elif a.startswith("@"):
            shutil.copy(inputs() / a[1:], d / a[1:])
            a = a[1:]
        out.append(a)
    for f in run.files:
        shutil.copy(inputs() / f, d / f)
    if run.source:
        src = topup_outputs(run.source)
        for f in sorted(src.iterdir()):
            if f.name.startswith("tu_") and f.name not in run.drop:
                shutil.copy(f, d / f.name)
    return out


def _mask_profiles(checks: CheckList, a: Path, b: Path) -> None:
    """``--profile``'s files with their durations masked: FSL's lines have the form the
    replica writes, the clock decides the numbers."""
    for d, who in ((a, "FSL"), (b, "the replica")):
        for p in sorted(d.glob(f"*{PROFILE_SUFFIX}")):
            text = p.read_text(encoding="utf-8")
            lines = text.splitlines()
            ok = all(re.fullmatch(r"Entry: .* Duration: [0-9.e+-]+", ln) for ln in lines)
            checks(f"{who}'s profile lines (Entry: ... Duration: <seconds>)", ok, detail=text[:200])
            p.write_text(_DURATION.sub("Duration: <t>", text), encoding="utf-8")


def _oracle(run: Run, args: list[str], d: Path, settings: dict[str, str | None]) -> Completed:
    never = run.never or ("--profile writes the clock's durations" if run.profile else "")
    p = oracle_run(
        args,
        cwd=d,
        settings=settings,
        never=never,
        threads=run.threads,
        timeout=run.timeout,
        program=run.program,
    )
    code = -9 if p.returncode is None else p.returncode
    return Completed(code, p.stdout, p.stderr)


def _fix_refuses(run: Run, node: Path | None, checks: CheckList) -> None:
    """:attr:`Run.fix_refuses`: the default run refuses."""
    settings = dict(run.settings)
    with tempfile.TemporaryDirectory(prefix="lx-nc-topup-fix-") as b_:
        b = Path(b_)
        args = _prepare(run, b, node)
        r = run_replica(run.program, args, cwd=b, settings=settings, mode=defects.FIX)
        refusal = next(
            (ln for ln in r.stderr.splitlines() if ln.startswith("larmorx-nc: error:")), ""
        )
        checks(
            f"the default run refuses, naming {run.fix_refuses}",
            r.code == 1 and run.fix_refuses in refusal,
            detail=f"exit {r.code}: {refusal[:200]}",
        )


def _fix_finishes(run: Run, node: Path | None, checks: CheckList) -> None:
    """:attr:`Run.fix_finishes`: the default run finishes where FSL stops."""
    with tempfile.TemporaryDirectory(prefix="lx-nc-topup-fix-") as b_:
        b = Path(b_)
        args = _prepare(run, b, node)
        r = run_replica(run.program, args, cwd=b, settings=dict(run.settings), mode=defects.FIX)
        named = defects.notices_in(r.stderr)
        checks(
            f"the default run finishes and names {run.fix_finishes}",
            r.code == 0 and run.fix_finishes in named,
            detail=f"exit {r.code}: {', '.join(named) or 'nothing named'}",
        )


def _fix_says(run: Run, node: Path | None, checks: CheckList) -> None:
    """:attr:`Run.fix_says`: the default run's corrected message."""
    with tempfile.TemporaryDirectory(prefix="lx-nc-topup-fix-") as b_:
        b = Path(b_)
        args = _prepare(run, b, node)
        r = run_replica(run.program, args, cwd=b, settings=dict(run.settings), mode=defects.FIX)
        named = defects.notices_in(r.stderr)
        checks(
            f"the default run's message ({', '.join(named) or 'nothing named'})",
            run.fix_says in r.stderr and bool(named),
            detail=" ".join(r.stderr.split())[-200:],
        )


def _refused_case(run: Run, node: Path | None, checks: CheckList) -> None:
    """:attr:`Run.refused`: the replica refuses in both modes; FSL is not run."""
    from larmorx_validation.parity.harness import EXPECTED

    for mode in (defects.REPRODUCE, defects.FIX):
        with tempfile.TemporaryDirectory(prefix="lx-nc-topup-") as b_:
            b = Path(b_)
            args = _prepare(run, b, node)
            before = {q.name for q in b.iterdir()}
            r = run_replica(run.program, args, cwd=b, settings=dict(run.settings), mode=mode)
            refusal = next(
                (ln for ln in r.stderr.splitlines() if ln.startswith("larmorx-nc: error:")), ""
            )
            written = sorted(q.name for q in b.iterdir() if q.name not in before)
            checks(
                f"the replica refuses ({mode})",
                r.code == 1 and run.refused in refusal and not written,
                detail=f"exit {r.code}: {refusal[:200]}",
            )
    if all(c.passed for c in checks.checks):
        raise Outcome(EXPECTED, f"{run.refused}: refused; FSL is not run: {run.refused_reason}")


def _thread_case(run: Run, node: Path | None, checks: CheckList) -> None:
    """D13 class (b): the replica at :attr:`Run.args`' thread count gives FSL's one-thread
    result (:attr:`Run.single`); FSL's own run at that thread count is measured against it."""
    assert run.single is not None
    s_args, s_settings = run.single
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-topup-t1-") as a1_,
        tempfile.TemporaryDirectory(prefix="lx-nc-topup-tn-") as an_,
        tempfile.TemporaryDirectory(prefix="lx-nc-topup-lx-") as b_,
    ):
        a1, an, b = Path(a1_), Path(an_), Path(b_)
        one = Run(s_args, s_settings, timeout=run.timeout)
        oracle1 = _oracle(one, _prepare(one, a1, node), a1, dict(s_settings))
        args = _prepare(run, an, node)
        oraclen = _oracle(run, args, an, dict(run.settings))
        _prepare(run, b, node)
        replica = run_replica(
            PROGRAM, args, cwd=b, settings=dict(run.settings), mode=defects.REPRODUCE
        )
        # the log records the options as given: FSL's at this thread count
        for log in an.glob("*.topup_log"):
            shutil.copy(log, a1 / log.name)
        compare(checks, PROGRAM, oracle1, replica, a1, b)
        checks(
            f"FSL at {run.threads} threads exits as at one",
            oraclen.code == oracle1.code,
            detail=f"{oraclen.code} vs {oracle1.code}",
        )
        same, total, notes = 0, 0, []
        for p in sorted(an.iterdir()):
            q = a1 / p.name
            if not q.is_file() or p.name.endswith(".topup_log"):
                continue
            total += 1
            if p.name.endswith(".nii.gz"):
                ok = _same_streams(p, q, blank_descrip=True)
                if not ok:
                    notes.append(f"{p.name}: {_differing_values(p, q)}")
            else:
                ok = _same_bytes(p, q)
                if not ok:
                    notes.append(p.name)
            same += ok
        checks.add(
            Check(
                f"FSL at {run.threads} threads against FSL at one (its own thread band)",
                True,
                metric="files identical",
                value=f"{same}/{total}",
                threshold="reported",
                detail="; ".join(notes[:4]),
            )
        )


def _run_case(run: Run, checks: CheckList) -> None:
    node = _source_node()
    if run.single is not None:
        _thread_case(run, node, checks)
        return
    if run.refused:
        _refused_case(run, node, checks)
        return
    settings = dict(run.settings)
    with (
        tempfile.TemporaryDirectory(prefix="lx-nc-topup-") as a_,
        tempfile.TemporaryDirectory(prefix="lx-nc-topup-") as b_,
    ):
        a, b = Path(a_), Path(b_)
        args = _prepare(run, a, node)
        _prepare(run, b, node)
        oracle = _oracle(run, args, a, settings)
        replica = run_replica(run.program, args, cwd=b, settings=settings, mode=defects.REPRODUCE)
        if run.profile:
            _mask_profiles(checks, a, b)
        compare(checks, run.program, oracle, replica, a, b)
        if run.fix and replica.code == 0 and not run.profile:
            fix_pass(
                checks,
                run.program,
                lambda t: _prepare(run, t, node),
                args,
                settings,
                b,
                replica,
                silent_ok=SILENT,
            )
    if run.fix_refuses:
        _fix_refuses(run, node, checks)
    if run.fix_finishes:
        _fix_finishes(run, node, checks)
    if run.fix_says:
        _fix_says(run, node, checks)
    _agree_on_errors(checks, run.program, oracle.code, oracle.stderr)


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


#: The cases' common arguments: the half-resolution pair, its acquisition parameters, the
#: output's base name; a short estimation (one level, three iterations).
HALF = ("--imain=@half.nii.gz", "--datain=@acq.txt", "--out=o")
SHORT = ("--warpres=20", "--fwhm=8", "--miter=3", "--lambda=0.005")
LEVELS = (
    "--warpres=20,14",
    "--subsamp=2,1",
    "--fwhm=8,4",
    "--miter=3,2",
    "--lambda=0.005,0.0001",
    "--estmov=1,0",
    "--minmet=0,1",
)


def _debug_cases() -> dict[str, tuple[str, str, Run]]:
    """``--debug`` on fMRIPrep's own inputs (ds005040's pair at full resolution)."""
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
    add(
        "debug/hessian",
        "standard",
        "--debug=3 on the half-resolution pair: one iteration's every debug file, the Hessian's "
        "text included",
        *HALF,
        "--warpres=20",
        "--miter=1",
        "--estmov=0",
        "--debug=3",
    )
    add(
        "debug/trace",
        "standard",
        "--trace: every routine's entry and exit printed, two levels",
        *HALF,
        *LEVELS,
        "--trace",
    )
    add(
        "debug/profile",
        "standard",
        "--profile: the levels' durations in <out>.topup_profile_topup.cpp_main.txt (the "
        "clock's numbers masked)",
        *HALF,
        *LEVELS,
        "--profile",
        profile=True,
    )
    return c


def _option_cases() -> dict[str, tuple[str, str, Run]]:
    """Every option on the half-resolution pair (32 x 32 x 20), one case per path."""
    c: dict[str, tuple[str, str, Run]] = {}
    st = "standard"

    def add(name: str, desc: str, *args: str, tier: str = st, **kw: Any) -> None:
        c[name] = (tier, desc, Run(tuple(args), **kw))

    add("opt/defaults", "every default (warpres 10, fwhm 8, five LM iterations, movements)", *HALF)
    add(
        "opt/levels",
        "two levels: subsampling 2 with Levenberg-Marquardt and the movements, then full "
        "resolution with scaled conjugate gradients; every output",
        *HALF,
        *LEVELS,
        "--iout=c",
        "--fout=f",
        "--dfout=d",
        "--rbmout=r",
        "--jacout=j",
        "--featout=feat",
        "-v",
        tier="smoke",
    )
    add("opt/numprec-float", "--numprec=float", *HALF, *SHORT, "--numprec=float", "--fout=f")
    add("opt/interp-linear", "--interp=linear", *HALF, *SHORT, "--interp=linear", "--iout=c")
    add(
        "opt/membrane-lm",
        "--regmod=membrane_energy with Levenberg-Marquardt (the energy's Hessian)",
        *HALF,
        *SHORT,
        "--regmod=membrane_energy",
        "--fout=f",
    )
    add(
        "opt/membrane-scg",
        "--regmod=membrane_energy with scaled conjugate gradients (the energy's gradient)",
        *HALF,
        *SHORT,
        "--regmod=membrane_energy",
        "--minmet=1",
        "--estmov=0",
        "--fout=f",
    )
    add("opt/splineorder-2", "--splineorder=2", *HALF, *SHORT, "--splineorder=2", "--fout=f")
    add(
        "opt/scale",
        "--scale=1: each volume scaled to the mean",
        *HALF,
        *SHORT,
        "--scale=1",
        "--iout=c",
    )
    add(
        "opt/ssqlambda-0",
        "--ssqlambda=0: lambda as given",
        *HALF,
        "--warpres=20",
        "--miter=3",
        "--lambda=50",
        "--ssqlambda=0",
        "--fout=f",
    )
    add(
        "opt/regrid-0",
        "--regrid=0: the image's own grid, two levels",
        *HALF,
        *LEVELS,
        "--regrid=0",
        "--fout=f",
    )
    add("opt/scg", "--minmet=1 without movements", *HALF, *SHORT, "--minmet=1", "--estmov=0")
    add(
        "opt/scg-movements",
        "--minmet=1 with the movements: FSL's warning",
        *HALF,
        *SHORT,
        "--minmet=1",
        "--estmov=1",
        "--rbmout=r",
    )
    add(
        "opt/acqp",
        "--acqp instead of --datain",
        "--imain=@half.nii.gz",
        "--acqp=@acq.txt",
        "--out=o",
        *SHORT,
    )
    add(
        "opt/datain-transposed",
        "an acquisition-parameter file of four rows",
        "--imain=@half.nii.gz",
        "--datain=@acq_t.txt",
        "--out=o",
        *SHORT,
    )
    add(
        "opt/x-phase-encoding",
        "phase encoding along x (the translation along x left out), the movements",
        "--imain=@xhalf.nii.gz",
        "--datain=@acq_x.txt",
        "--out=o",
        *SHORT,
        "--iout=c",
        "--rbmout=r",
        "--jacout=j",
        "--dfout=d",
    )
    add(
        "opt/four-volumes",
        "four volumes along x and y: six movement parameters for each moving scan",
        "--imain=@four.nii.gz",
        "--datain=@acq_four.txt",
        "--out=o",
        *SHORT,
        "--iout=c",
        "--rbmout=r",
        "--jacout=j",
        "--dfout=d",
    )
    add(
        "opt/oblique-phase-encoding",
        "a phase-encoding vector along x and y (the Jacobian of both displacements)",
        "--imain=@half.nii.gz",
        "--datain=@acq_oblique.txt",
        "--out=o",
        *SHORT,
        "--jacout=j",
        "--dfout=d",
    )
    add(
        "opt/readout-times",
        "different read-out times",
        "--imain=@half.nii.gz",
        "--datain=@acq_rts.txt",
        "--out=o",
        *SHORT,
        "--fout=f",
    )
    add(
        "opt/zero-readout",
        "one read-out time of 0 (a synthesised undistorted image)",
        "--imain=@half.nii.gz",
        "--datain=@acq_zero.txt",
        "--out=o",
        *SHORT,
        "--fout=f",
        "--iout=c",
    )
    add(
        "opt/same-direction",
        "both volumes with the same phase encoding",
        "--imain=@half.nii.gz",
        "--datain=@acq_same.txt",
        "--out=o",
        *SHORT,
        "--fout=f",
    )
    add(
        "opt/z-phase-encoding",
        "phase encoding along z",
        "--imain=@half.nii.gz",
        "--datain=@acq_z.txt",
        "--out=o",
        *SHORT,
    )
    add(
        "opt/int16",
        "a 16-bit integer image",
        "--imain=@half_i16.nii.gz",
        "--datain=@acq.txt",
        "--out=o",
        *SHORT,
        "--iout=c",
    )
    add(
        "opt/one-volume",
        "a single 3D volume",
        "--imain=@one.nii.gz",
        "--datain=@acq_one.txt",
        "--out=o",
        *SHORT,
    )
    add(
        "opt/no-out",
        "no --out: the base name from --imain",
        "--imain=@half.nii.gz",
        "--datain=@acq.txt",
        *SHORT,
    )
    add(
        "opt/intent",
        "an image with intent parameters: the field's header",
        "--imain=@half_intent.nii.gz",
        "--datain=@acq.txt",
        "--out=o",
        *SHORT,
        "--fout=f",
        "--featout=feat",
    )
    add(
        "opt/nifti",
        "FSLOUTPUTTYPE=NIFTI",
        *HALF,
        *SHORT,
        "--fout=f",
        settings=(("FSLOUTPUTTYPE", "NIFTI"),),
    )
    add(
        "opt/nifti2",
        "FSLOUTPUTTYPE=NIFTI2_GZ",
        *HALF,
        *SHORT,
        "--fout=f",
        "--iout=c",
        settings=(("FSLOUTPUTTYPE", "NIFTI2_GZ"),),
    )
    add(
        "opt/nifti-pair",
        "FSLOUTPUTTYPE=NIFTI_PAIR",
        *HALF,
        *SHORT,
        "--fout=f",
        settings=(("FSLOUTPUTTYPE", "NIFTI_PAIR"),),
    )
    add(
        "opt/iout-extension",
        "--iout with .nii.gz: the mean as <name>.nii.gz_mean",
        *HALF,
        *SHORT,
        "--iout=c.nii.gz",
    )
    add(
        "opt/nthr-0",
        "--nthr=0: the default thread count (FSL_NUM_THREADS unset: one)",
        *HALF,
        *SHORT,
        "--nthr=0",
    )
    add(
        "opt/warpres-0",
        "--warpres=0: a knot every voxel",
        *HALF,
        "--warpres=0",
        "--miter=2",
        "--estmov=0",
        "--minmet=1",
    )
    add(
        "opt/lambda-negative",
        "a negative --lambda",
        *HALF,
        "--warpres=20",
        "--miter=3",
        "--lambda=-0.005",
        "--fout=f",
        fix_refuses="FSL-126",
    )
    add(
        "opt/fwhm-negative",
        "a negative --fwhm",
        *HALF,
        "--warpres=20",
        "--miter=3",
        "--fwhm=-8",
        "--fout=f",
        fix_refuses="FSL-125",
    )
    add(
        "opt/warpres-negative",
        "a negative --warpres",
        *HALF,
        "--warpres=-20",
        "--miter=2",
        "--fout=f",
        refused="FSL-124",
        refused_reason="its knot spacing of about 4e9 voxels needs more memory than the machine holds (2026-11-03: killed in a 24 GiB memory box)",
    )
    return c


def _config_cases() -> dict[str, tuple[str, str, Run]]:
    """``--config``: FSL's shipped configurations by name (in ``$FSLDIR/etc/flirtsch``, with
    one iteration per level from the command line), local files, the failures."""
    c: dict[str, tuple[str, str, Run]] = {}
    st = "standard"

    def add(name: str, desc: str, *args: str, tier: str = st, **kw: Any) -> None:
        c[name] = (tier, desc, Run(tuple(args), **kw))

    for cnf in ("b02b0", "b02b0_1", "b02b0_2", "b02b0_4", "b02b0_7T", "test_acqp"):
        add(
            f"cfg/{cnf}",
            f"FSL's {cnf}.cnf by name, one iteration per level",
            *HALF,
            f"--config={cnf}",
            "--miter=1",
            "--iout=c",
            "--fout=f",
        )
    add(
        "cfg/b02b0-extension",
        "FSL's b02b0.cnf by name with its extension",
        *HALF,
        "--config=b02b0.cnf",
        "--miter=1",
    )
    add("cfg/local", "a local configuration file", *HALF, "--config=@short.cnf", "--fout=f")
    add(
        "cfg/local-no-extension",
        "a local configuration named without .cnf",
        *HALF,
        "--config=short",
        "--fout=f",
        files=("short.cnf",),
    )
    add(
        "cfg/command-line-wins",
        "options on the command line take precedence",
        *HALF,
        "--config=@short.cnf",
        "--warpres=30,20",
        "--fout=f",
    )
    add(
        "cfg/directory",
        "a directory as the configuration (read as empty)",
        *HALF,
        "--config=.",
        "--miter=2",
    )
    add("cfg/not-found", "a configuration that does not exist", *HALF, "--config=nothere")
    add(
        "cfg/not-found-path",
        "a configuration with a path that does not exist",
        *HALF,
        "--config=./nothere",
    )
    add(
        "cfg/fsldir-unset",
        "FSLDIR unset and a name to look up there",
        *HALF,
        "--config=b02b0",
        settings=(("FSLDIR", None),),
        fix_refuses="FSL-121",
    )
    add(
        "cfg/bad-option",
        "an unknown option in the configuration",
        *HALF,
        "--config=@bad_option.cnf",
    )
    add(
        "cfg/long-line",
        "a comment line of more than 1023 characters before two settings",
        *HALF,
        "--config=@long_line.cnf",
        "--fout=f",
    )
    return c


def _error_cases() -> dict[str, tuple[str, str, Run]]:
    """The messages, exit statuses and failure cases."""
    c: dict[str, tuple[str, str, Run]] = {}
    st = "standard"

    def add(name: str, desc: str, *args: str, tier: str = st, **kw: Any) -> None:
        c[name] = (tier, desc, Run(tuple(args), **kw))

    i, d = "--imain=@half.nii.gz", "--datain=@acq.txt"
    add("err/no-arguments", "no arguments: the usage")
    add("err/help", "-h: the usage", "-h")
    add("err/help-long", "--help with the compulsory options set", i, d, "--help")
    add("err/unknown-option", "an unknown option", i, d, "--bogus=1")
    add("err/missing-value", "an option without its value", "--imain")
    add("err/no-imain", "no --imain", d, tier="smoke")
    add("err/imain-missing", "an --imain that does not exist", "--imain=nothere.nii.gz", d)
    add("err/imain-text", "an --imain that is not an image", "--imain=@acq.txt", d)
    add("err/datain-and-acqp", "both --datain and --acqp", i, d, "--acqp=@acq.txt")
    add("err/no-datain", "neither --datain nor --acqp", i)
    add("err/datain-missing", "a --datain that does not exist", i, "--datain=nothere.txt")
    add("err/datain-five-columns", "a --datain of five columns", i, "--datain=@acq_five.txt")
    add("err/datain-text", "a --datain of words", i, "--datain=@acq_text.txt")
    add(
        "err/datain-norm", "a phase-encoding vector not of unit length", i, "--datain=@acq_norm.txt"
    )
    add("err/datain-rows", "three rows for two volumes", i, "--datain=@acq_three.txt")
    add(
        "err/acqp-rows",
        "three rows for two volumes, given with --acqp",
        i,
        "--acqp=@acq_three.txt",
        fix_says="Mismatch between half.nii.gz and acq_three.txt",
    )
    add("err/readout-long", "a read-out time above 0.2 s", i, "--datain=@acq_long_rt.txt")
    add("err/readout-two-zero", "two read-out times of 0", i, "--datain=@acq_two_zero.txt")
    add(
        "err/levels-subsamp",
        "--subsamp of three levels against two",
        *HALF,
        "--warpres=20,14",
        "--subsamp=2,1,1",
    )
    add(
        "err/levels-minmet",
        "--minmet of three levels against two",
        *HALF,
        "--warpres=20,14",
        "--minmet=0,1,1",
    )
    add("err/minmet-2", "--minmet=2", *HALF, "--minmet=2")
    add("err/subsamp-incompatible", "--subsamp=3 on a 32-voxel axis", *HALF, "--subsamp=3")
    add("err/subsamp-0", "--subsamp=0", *HALF, "--subsamp=0")
    add(
        "err/subsamp-mixed",
        "subsamplings 3 and 2 on a 30 x 30 x 18 image (each divides it; the regridded grid of "
        "33 does not divide by 2)",
        "--imain=@crop.nii.gz",
        d,
        "--out=o",
        "--warpres=20,14",
        "--subsamp=3,2",
        "--miter=2",
        fix_finishes="FSL-118",
    )
    add("err/numprec", "an unknown --numprec", *HALF, "--numprec=single")
    add("err/regmod", "an unknown --regmod", *HALF, "--regmod=sobolev")
    add("err/interp", "an unknown --interp", *HALF, "--interp=cubic")
    add("err/splineorder-4", "--splineorder=4", *HALF, "--splineorder=4")
    add("err/splineorder-negative", "--splineorder=-1", *HALF, "--splineorder=-1")
    add("err/debug-4", "--debug=4", *HALF, *SHORT, "--debug=4")
    add("err/debug-negative", "--debug=-1", *HALF, *SHORT, "--debug=-1")
    add(
        "err/miter-negative",
        "--miter=-1 (about 4e9 iterations)",
        *HALF,
        "--warpres=20",
        "--estmov=0",
        "--miter=-1",
        timeout=900,
        fix_refuses="FSL-123",
    )
    add(
        "err/out-no-directory",
        "--out in a directory that does not exist",
        *HALF[:2],
        "--out=nodir/o",
        *SHORT,
    )
    add(
        "err/iout-no-directory",
        "--iout in a directory that does not exist",
        *HALF,
        *SHORT,
        "--iout=nodir/c",
    )
    add(
        "err/logout-no-directory",
        "--logout in a directory that does not exist",
        *HALF,
        *SHORT,
        "--logout=nodir/log",
    )
    add(
        "err/rbmout-no-directory",
        "--rbmout in a directory that does not exist",
        *HALF,
        *SHORT,
        "--rbmout=nodir/r",
    )
    add(
        "err/profile-no-directory",
        "--profile with --out in a directory that does not exist",
        *HALF[:2],
        "--out=nodir/o",
        *SHORT,
        "--profile",
        profile=True,
    )
    add(
        "err/outputtype-invalid",
        "FSLOUTPUTTYPE=BOGUS",
        *HALF,
        *SHORT,
        "--fout=f",
        settings=(("FSLOUTPUTTYPE", "BOGUS"),),
    )
    add(
        "err/outputtype-unset",
        "FSLOUTPUTTYPE unset",
        *HALF,
        *SHORT,
        "--fout=f",
        settings=(("FSLOUTPUTTYPE", None),),
    )
    return c


def _thread_cases() -> dict[str, tuple[str, str, Run]]:
    """The thread dependence (D13 class (b)): the replica gives FSL's one-thread result at
    any thread count; FSL's own runs at four threads are measured against it."""
    c: dict[str, tuple[str, str, Run]] = {}
    st = "standard"
    base = (*HALF, *LEVELS, "--fout=f", "--iout=c")
    lm = (*HALF, *SHORT, "--fout=f", "--iout=c")
    scg = (
        *HALF,
        "--warpres=4",
        "--fwhm=4",
        "--miter=2",
        "--lambda=0.005",
        "--estmov=0",
        "--minmet=1",
        "--fout=f",
        "--iout=c",
    )
    c["thr/nthr-4"] = (
        st,
        "--nthr=4 (the spline field's threaded products and the Hessians' threaded sparse "
        "products) against --nthr=1",
        Run((*base, "--nthr=4"), threads=4, single=((*base, "--nthr=1"), ())),
    )
    c["thr/fsl-num-threads"] = (
        st,
        "FSL_NUM_THREADS=4 with --nthr=1 (the default thread count of the Hessian's sparse "
        "matrices) against it unset",
        Run(lm, (("FSL_NUM_THREADS", "4"),), threads=4, single=(lm, ())),
    )
    c["thr/openblas"] = (
        st,
        "FSL_SKIP_GLOBAL=1 and OPENBLAS_NUM_THREADS=4 (OpenBLAS's own threads: its dot product "
        "of more than 10,000 values) against OpenBLAS on one thread, with scaled conjugate "
        "gradients on a knot every voxel (about 31,000 coefficients)",
        Run(
            scg,
            (("FSL_SKIP_GLOBAL", "1"), ("OPENBLAS_NUM_THREADS", "4")),
            threads=4,
            single=(scg, ()),
        ),
    )
    return c


# ------------------------------------------------------------------------------------------------
# applytopup


#: applytopup's derived inputs' version (made beside the topup cases' own, which they leave
#: as they are): a new version makes them again.
AT_VERSION = 2
#: The topup runs whose outputs applytopup's cases apply (FSL's outputs, served from the
#: oracle cache; the replica's topup gives the same files bit for bit, as the topup cases
#: check): a topup case's name, or a run of its own.
TOPUP_SOURCES: dict[str, str | Run] = {
    "y": "opt/levels",
    "x": "opt/x-phase-encoding",
    "four": "opt/four-volumes",
    "quad": "opt/splineorder-2",
    "crop": Run(("--imain=@crop.nii.gz", "--datain=@acq.txt", "--out=o", *SHORT, "--fout=f")),
}


def _atomic_dir(target: Path, make: Any) -> Path:
    """``target`` made once by ``make(tmp)`` in a private directory renamed into place (the
    suite's workers may ask at the same time)."""
    if (target / "DONE").is_file():
        return target
    target.parent.mkdir(parents=True, exist_ok=True)
    tmp = Path(tempfile.mkdtemp(prefix=f".{target.name}-", dir=target.parent))
    make(tmp)
    (tmp / "DONE").write_text("made by the fsl-topup-replica suite\n", encoding="utf-8")
    try:
        tmp.rename(target)
    except OSError:
        shutil.rmtree(tmp, ignore_errors=True)
    return target


def applytopup_inputs() -> Path:
    """applytopup's images (``oracles/inputs/fsl-topup-replica/applytopup-v2/images/``, linked
    into the inputs' directory by name): each volume of the half-resolution pair as its own
    image (``half_up``, ``half_dn``), two volumes of each (``dwi_up``, ``dwi_dn``: the second
    half the first), the pair along x (``xhalf_up``, ``xhalf_dn``), the cropped pair
    (``crop_up``, ``crop_dn``: 30 voxels along the phase encoding), the four volumes along y
    and x (``four_0`` to ``four_3``), 16-bit and 8-bit integers (``half_up_i16``,
    ``half_up_i8``), the first volume with 4 mm voxels along y (``half_up_aniso``) and in a
    grid of 64 x 16 x 20 (``half_up_64x16``)."""
    import nibabel as nib

    base = inputs()

    def make(d: Path) -> None:
        half = nib.load(str(base / "half.nii.gz"))
        h = np.asarray(half.dataobj, np.float32)
        aff = half.affine
        xh = np.asarray(nib.load(str(base / "xhalf.nii.gz")).dataobj, np.float32)
        crop = np.asarray(nib.load(str(base / "crop.nii.gz")).dataobj, np.float32)
        four = np.asarray(nib.load(str(base / "four.nii.gz")).dataobj, np.float32)
        imgs = {
            "half_up": h[..., 0],
            "half_dn": h[..., 1],
            "dwi_up": np.stack([h[..., 0], h[..., 0] * np.float32(0.5)], axis=3),
            "dwi_dn": np.stack([h[..., 1], h[..., 1] * np.float32(0.5)], axis=3),
            "xhalf_up": xh[..., 0],
            "xhalf_dn": xh[..., 1],
            "crop_up": crop[..., 0],
            "crop_dn": crop[..., 1],
        }
        for k in range(4):
            imgs[f"four_{k}"] = four[..., k]
        for name, data in imgs.items():
            _save(d / f"{name}.nii.gz", data, aff)
        _save(d / "half_up_i16.nii.gz", np.round(h[..., 0] * 0.25), aff, np.int16)
        _save(
            d / "half_up_i8.nii.gz",
            np.clip(np.round(h[..., 0] * 0.05), -128, 127),
            aff,
            np.int8,
        )
        # the first volume's values in a grid of 64 x 16 x 20 (as many voxels, another shape)
        _save(d / "half_up_64x16.nii.gz", h[..., 0].reshape((64, 16, 20), order="F"), aff)
        aniso = aff.copy()
        aniso[:3, 1] = aniso[:3, 1] * (4.0 / 6.0)
        _save(d / "half_up_aniso.nii.gz", h[..., 0], aniso)

    images = _atomic_dir(base / f"applytopup-v{AT_VERSION}" / "images", make)
    for f in sorted(images.glob("*.nii.gz")):
        link = base / f.name
        if not link.exists():
            part = base / f".{f.name}.{os.getpid()}"
            shutil.copy(f, part)
            os.replace(part, link)
    return images


def topup_outputs(source: str) -> Path:
    """FSL's topup outputs for :data:`TOPUP_SOURCES` ``source`` (its coefficients, movements
    and --fout field as ``tu_fieldcoef.nii.gz``, ``tu_movpar.txt``, ``tu_field.nii.gz``),
    from a run through the oracle cache, kept in ``applytopup-v2/<source>/``."""
    spec = TOPUP_SOURCES[source]
    run = _option_cases()[spec][2] if isinstance(spec, str) else spec

    def make(d: Path) -> None:
        with tempfile.TemporaryDirectory(prefix="lx-nc-topup-src-") as t_:
            t = Path(t_)
            args = _prepare(run, t, _source_node())
            r = _oracle(run, args, t, dict(run.settings))
            assert r.code == 0, f"topup for {source} exits {r.code}: {r.stderr[-300:]}"
            for name, new in (
                ("o_fieldcoef.nii.gz", "tu_fieldcoef.nii.gz"),
                ("o_movpar.txt", "tu_movpar.txt"),
                ("f.nii.gz", "tu_field.nii.gz"),
            ):
                if (t / name).is_file():
                    shutil.copy(t / name, d / new)

    return _atomic_dir(inputs() / f"applytopup-v{AT_VERSION}" / source, make)


def _applytopup_cases() -> dict[str, tuple[str, str, Run]]:
    """applytopup: least-squares restoration and Jacobian modulation on topup's own outputs,
    every option, message and failure case, one case per path."""
    c: dict[str, tuple[str, str, Run]] = {}
    st = "standard"

    def add(name: str, desc: str, *args: str, tier: str = st, **kw: Any) -> None:
        kw.setdefault("source", "y")
        c[f"at/{name}"] = (tier, desc, Run(tuple(args), program="applytopup", **kw))

    pair = ("--imain=@half_up.nii.gz,@half_dn.nii.gz", "--inindex=1,2")
    acq, tu, out = "--datain=@acq.txt", "--topup=tu", "--out=hifi"
    up = ("--imain=@half_up.nii.gz", "--inindex=1")
    jac = "--method=jac"
    add(
        "lsr-y",
        "least-squares restoration of the pair along y (the default method, spline "
        "interpolation, the movements)",
        *pair,
        acq,
        tu,
        out,
        tier="smoke",
    )
    add(
        "lsr-4d",
        "least-squares restoration of two volumes per phase-encoding direction",
        "--imain=@dwi_up.nii.gz,@dwi_dn.nii.gz",
        "--inindex=1,2",
        acq,
        tu,
        out,
    )
    add(
        "lsr-x",
        "least-squares restoration along x",
        "--imain=@xhalf_up.nii.gz,@xhalf_dn.nii.gz",
        "--inindex=1,2",
        "--datain=@acq_x.txt",
        tu,
        out,
        source="x",
    )
    add(
        "lsr-four",
        "least-squares restoration of two collections (along y and along x)",
        "--imain=@four_0.nii.gz,@four_1.nii.gz,@four_2.nii.gz,@four_3.nii.gz",
        "--inindex=1,2,3,4",
        "--datain=@acq_four.txt",
        tu,
        out,
        source="four",
    )
    add(
        "lsr-three",
        "least-squares restoration of three images in one collection",
        "--imain=@half_up.nii.gz,@half_dn.nii.gz,@half_up.nii.gz",
        "--inindex=1,2,1",
        acq,
        tu,
        out,
    )
    add("lsr-trilinear", "--interp=trilinear", *pair, acq, tu, out, "--interp=trilinear")
    add(
        "lsr-crop",
        "least-squares restoration of lines of 30 voxels (a length that does not divide 2^32)",
        "--imain=@crop_up.nii.gz,@crop_dn.nii.gz",
        "--inindex=1,2",
        acq,
        tu,
        out,
        source="crop",
    )
    add("jac-y", "Jacobian modulation of one image", *up, acq, tu, out, jac)
    add("jac-pair", "Jacobian modulation of the pair, averaged", *pair, acq, tu, out, jac)
    add(
        "jac-4d",
        "Jacobian modulation of two volumes",
        "--imain=@dwi_up.nii.gz",
        "--inindex=1",
        acq,
        tu,
        out,
        jac,
    )
    add(
        "jac-trilinear",
        "--method=jac --interp=trilinear",
        *up,
        acq,
        tu,
        out,
        jac,
        "-n",
        "trilinear",
    )
    add(
        "jac-x",
        "Jacobian modulation along x",
        "--imain=@xhalf_dn.nii.gz",
        "--inindex=2",
        "--datain=@acq_x.txt",
        tu,
        out,
        jac,
        source="x",
    )
    add("jac-quadratic", "a field of quadratic splines", *up, acq, tu, out, jac, source="quad")
    add(
        "jac-field",
        "--topup naming a field image (topup's --fout): no movements",
        *up,
        acq,
        "--topup=tu_field",
        out,
        jac,
    )
    add(
        "jac-field-aniso",
        "a field image and voxels of 6 x 4 mm in plane (the Jacobian of the y displacements)",
        "--imain=@half_up_aniso.nii.gz",
        "--inindex=1",
        acq,
        "--topup=tu_field",
        out,
        jac,
    )
    add(
        "jac-no-movpar",
        "coefficients without their _movpar.txt (no movements)",
        *up,
        acq,
        tu,
        out,
        jac,
        drop=("tu_movpar.txt",),
    )
    add(
        "jac-oblique",
        "a phase-encoding vector along x and y",
        *up,
        "--datain=@acq_oblique.txt",
        tu,
        out,
        jac,
    )
    add("jac-z", "phase encoding along z", *up, "--datain=@acq_z.txt", tu, out, jac)
    for dtn in ("preserve", "char", "short", "int", "float", "double"):
        add(f"datatype-{dtn}", f"--datatype={dtn}", *up, acq, tu, out, jac, f"--datatype={dtn}")
    add(
        "datatype-preserve-int16",
        "--datatype=preserve of a 16-bit image",
        "--imain=@half_up_i16.nii.gz",
        "--inindex=1",
        acq,
        tu,
        out,
        jac,
        "-d",
        "preserve",
    )
    add(
        "datatype-preserve-int8",
        "--datatype=preserve of an 8-bit signed image",
        "--imain=@half_up_i8.nii.gz",
        "--inindex=1",
        acq,
        tu,
        out,
        jac,
        "--datatype=preserve",
    )
    add(
        "short-options",
        "the short options",
        "-i",
        "half_up.nii.gz,half_dn.nii.gz",
        "-x",
        "1,2",
        "-a",
        "acq.txt",
        "-t",
        "tu",
        "-o",
        "hifi",
        "-m",
        "lsr",
        "-v",
        files=("half_up.nii.gz", "half_dn.nii.gz", "acq.txt"),
    )
    add(
        "nifti",
        "FSLOUTPUTTYPE=NIFTI",
        *up,
        acq,
        tu,
        out,
        jac,
        settings=(("FSLOUTPUTTYPE", "NIFTI"),),
    )
    # messages and failures
    add("no-arguments", "no arguments: the compulsory options and the usage")
    add("help", "-h: the usage", "-h")
    add("unknown-option", "an unknown option", *pair, acq, tu, out, "--bogus=1")
    add("invert", "--invert (declared, never added to the parser)", *pair, acq, tu, out, "--invert")
    add("missing-out", "no --out", *pair, acq, tu)
    add("interp-unknown", "an unknown --interp", *pair, acq, tu, out, "--interp=cubic")
    add(
        "lists-mismatch",
        "two images and one index",
        "--imain=@half_up.nii.gz,@half_dn.nii.gz",
        "--inindex=1",
        acq,
        tu,
        out,
    )
    add("datatype-unknown", "an unknown --datatype", *up, acq, tu, out, jac, "--datatype=long")
    add(
        "datatype-preserve-missing",
        "--datatype=preserve of an image that does not exist",
        "--imain=nothere.nii.gz",
        "--inindex=1",
        acq,
        tu,
        out,
        "--datatype=preserve",
    )
    add(
        "imain-missing",
        "an image that does not exist",
        "--imain=nothere.nii.gz",
        "--inindex=1",
        acq,
        tu,
        out,
    )
    add(
        "images-mismatch",
        "images of different sizes",
        "--imain=@half_up.nii.gz,@crop_dn.nii.gz",
        "--inindex=1,2",
        acq,
        tu,
        out,
    )
    add("datain-missing", "a --datain that does not exist", *pair, "--datain=nothere.txt", tu, out)
    add(
        "datain-norm",
        "a phase-encoding vector not of unit length",
        *pair,
        "--datain=@acq_norm.txt",
        tu,
        out,
    )
    add("inindex-zero", "an index of 0", "--imain=@half_up.nii.gz", "--inindex=0", acq, tu, out)
    add(
        "inindex-beyond",
        "an index beyond --datain's rows",
        "--imain=@half_up.nii.gz",
        "--inindex=3",
        acq,
        tu,
        out,
    )
    add("topup-extension", "--topup with an extension", *pair, acq, "--topup=tu.nii.gz", out)
    add("topup-missing", "a --topup that does not exist", *pair, acq, "--topup=nothere", out)
    add(
        "lsr-oblique",
        "--method=lsr with an oblique phase encoding",
        *pair,
        "--datain=@acq_oblique.txt",
        tu,
        out,
    )
    add(
        "lsr-same",
        "--method=lsr of two images with the same phase encoding",
        *pair,
        "--datain=@acq_same.txt",
        tu,
        out,
    )
    add("lsr-one", "--method=lsr of one image", *up, acq, tu, out)
    add(
        "outputtype-invalid",
        "FSLOUTPUTTYPE=BOGUS",
        *up,
        acq,
        tu,
        out,
        jac,
        settings=(("FSLOUTPUTTYPE", "BOGUS"),),
    )
    # the defects
    add(
        "method-unknown",
        "an unknown --method (FSL-135: FSL writes nothing and exits 0)",
        *pair,
        acq,
        tu,
        out,
        "--method=foo",
        fix_refuses="FSL-135",
    )
    add(
        "lsr-no-movements",
        "--method=lsr with a field image, which has no movements (FSL-135)",
        *pair,
        acq,
        "--topup=tu_field",
        out,
        fix_refuses="FSL-135",
    )
    add(
        "jac-other-grid",
        "--method=jac with a field on another grid (FSL-134: another number of voxels)",
        "--imain=@crop_up.nii.gz",
        "--inindex=1",
        acq,
        tu,
        out,
        jac,
        fix_refuses="FSL-134",
    )
    add(
        "jac-other-shape",
        "--method=jac with a field of as many voxels as the images in another shape (FSL-134)",
        "--imain=@half_up_64x16.nii.gz",
        "--inindex=1",
        acq,
        tu,
        out,
        jac,
        fix_refuses="FSL-134",
    )
    add(
        "lsr-other-grid",
        "--method=lsr with a field on another grid (FSL-134)",
        "--imain=@crop_up.nii.gz,@crop_dn.nii.gz",
        "--inindex=1,2",
        acq,
        tu,
        out,
        refused="FSL-134",
        refused_reason="FSL reads the field's lines past their end (undefined behaviour)",
    )
    return c


def _all_cases() -> dict[str, tuple[str, str, Run]]:
    return {
        **_debug_cases(),
        **_option_cases(),
        **_config_cases(),
        **_error_cases(),
        **_thread_cases(),
        **_applytopup_cases(),
    }


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
    else:
        _run_case(_all_cases()[payload][2], checks)


def suite() -> Suite:
    return Suite(
        name=SUITE,
        title="topup and applytopup replicas (bit-exact, FSL Licence)",
        tool="`larmorx-nc fsl topup` and `fsl applytopup` (crate larmorx-nc-topup, translated from FSL "
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
