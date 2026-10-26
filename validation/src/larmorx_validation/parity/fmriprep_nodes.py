# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's own calls of a program in every finished oracle run, to replay exactly.

The data panel's re-validation (``docs/next-steps.md``, item 6) compares every node of the
fourteen fMRIPrep 25.2.6 oracle runs (``oracles/fmriprep-25.2.6/<run>``) that ran a program
larmorx has: the node's own ``command.txt`` (fMRIPrep's arguments, as nipype wrote them), its
inputs from the run's working directory, its outputs redirected to the case's directory. Only
finished runs are read (:func:`larmorx_validation.stages.oracle.run_finished`): a run in
progress has an incomplete working directory that changes while it is read.

The calls are found by their ``command.txt`` alone (the first word names the program), so
enumerating a program over the fourteen runs reads a few hundred small files, not nipype's
reports. Container paths (``/work``, ``/out``, ``/data``, ``/templateflow``) are mapped to this
machine by :class:`~larmorx_validation.stages.oracle.OracleRun`. Two calls with the same
arguments (the template's Laplacian, computed once per run from the same file) are compared
once: :func:`calls` keeps the first and lists the others as its duplicates.
"""

from __future__ import annotations

import functools
import os
import re
import shlex
from dataclasses import dataclass, field
from pathlib import Path

#: The oracle runs, relative to the workspace.
ORACLE_RUNS = "oracles/fmriprep-25.2.6"
#: fMRIPrep's version, as the runs' working directories name it.
WORKFLOW = "fmriprep_25_2_wf"


@functools.cache
def root() -> Path | None:
    """``oracles/fmriprep-25.2.6`` of the workspace (``LARMORX_FMRIPREP_RUNS`` overrides)."""
    if env := os.environ.get("LARMORX_FMRIPREP_RUNS"):
        return Path(env)
    for parent in Path(__file__).resolve().parents:
        if (parent / ORACLE_RUNS).is_dir():
            return parent / ORACLE_RUNS
    return None


@functools.cache
def finished_runs() -> tuple[str, ...]:
    """The names of the finished oracle runs, in name order (``()`` when there are none)."""
    from larmorx_validation.stages.oracle import run_finished

    r = root()
    if r is None:
        return ()
    return tuple(sorted(p.name for p in r.iterdir() if (p / "work").is_dir() and run_finished(p)))


@functools.cache
def oracle_run(name: str):
    """The :class:`~larmorx_validation.stages.oracle.OracleRun` ``name``."""
    from larmorx_validation.stages.oracle import OracleRun

    return OracleRun(root() / name)


def short(run: str) -> str:
    """``ds000228-sub-pixar008-noreconall`` -> ``ds000228-sub-pixar008``."""
    return run.removesuffix("-noreconall")


def label(path: str) -> str:
    """A node's path, readable: the workflow and subject left out, ``_wf`` suffixes dropped,
    the BOLD run, the space, the echo and the map-node item kept as ``[...]``.

    ``fmriprep_25_2_wf/sub_01_wf/bold_task_x_run_01_wf/bold_fit_wf/enhance_and_skullstrip_bold_wf/
    n4_correct`` -> ``bold[task_x_run_01]/bold_fit/enhance_and_skullstrip_bold/n4_correct``."""
    parts = path.split("/")
    out: list[str] = []
    for i, p in enumerate(parts):
        if i == 0 and re.fullmatch(r"fmriprep_\w+_wf", p):
            continue
        if i <= 1 and re.fullmatch(r"sub_\w+_wf", p):
            continue
        m = re.fullmatch(r"bold_(\w+)_wf", p)
        if m and ("run" in p or "task" in p):
            out.append(f"bold[{m.group(1)}]")
            continue
        if m := re.fullmatch(r"_(?:in_tuple|template)_(.+)", p):
            out[-1:] = [f"{out[-1]}[{m.group(1)}]"] if out else [f"[{m.group(1)}]"]
            continue
        if m := re.fullmatch(r"_echoidx_(\d+)", p):
            out.append(f"echo[{m.group(1)}]")
            continue
        if p == "mapflow":
            continue
        if i > 0 and parts[i - 1] == "mapflow":
            if m := re.search(r"(\d+)$", p):
                out[-1] = f"{out[-1]}[{m.group(1)}]"
            continue
        out.append(p.removesuffix("_wf"))
    return "/".join(out)


@dataclass(frozen=True)
class NodeCall:
    """One node of an oracle run that ran ``program``."""

    #: The run directory's name (``ds000228-sub-pixar008-noreconall``).
    run: str
    #: The node directory, relative to the run's ``work/`` (posix).
    node: str
    program: str
    #: The command's arguments after the program, as nipype's shell splits them, every
    #: container path mapped to this machine; the outputs as ``{out:<name>}`` placeholders.
    args: tuple[str, ...]
    #: The outputs, as placeholders (``{out:<name>}``), in the order the command names them.
    outputs: tuple[str, ...]
    #: The catalogue's normalised path (subject, run, space, item removed).
    norm: str
    #: Other nodes (``run:node``) whose command is the same, word for word, after mapping.
    duplicates: tuple[str, ...] = field(default=(), compare=False)

    @property
    def id(self) -> str:
        """``<run>/<label>``, unique within the program."""
        return f"{short(self.run)}/{label(self.node)}"

    @property
    def work(self) -> Path:
        return root() / self.run / "work"

    @property
    def directory(self) -> Path:
        """The node's directory in the run's working directory."""
        return self.work / self.node

    def command(self) -> str:
        """``command.txt`` as fMRIPrep wrote it (container paths)."""
        return (self.directory / "command.txt").read_text(encoding="utf-8").strip()

    def nslots(self) -> int | None:
        """The threads fMRIPrep gave the node (``NSLOTS`` in nipype's report)."""
        report = self.directory / "_report" / "report.rst"
        if not report.is_file():
            return None
        m = re.search(r"^\* NSLOTS : (\d+)", report.read_text(errors="replace"), re.M)
        return int(m.group(1)) if m else None

    def missing(self) -> list[str]:
        """The input files the command names that the working directory no longer holds."""
        out = []
        for a in self.args:
            for t in re.split(r"[\[\],\s]+", a):
                if t.startswith("/") and not Path(t).exists():
                    out.append(t)
        return out


def _program_of(command: str) -> str:
    words = command.split(None, 1)
    return os.path.basename(words[0]) if words else ""


@functools.cache
def _commands(run: str) -> tuple[tuple[str, str], ...]:
    """Every ``(node path, command)`` of ``run``'s working directory."""
    work = root() / run / "work"
    out = []
    for f in sorted(work.rglob("command.txt")):
        try:
            text = f.read_text(encoding="utf-8", errors="replace").strip()
        except OSError:
            continue
        if text:
            out.append((f.parent.relative_to(work).as_posix(), text))
    return tuple(out)


def _output_words(program: str, words: list[str]) -> list[str]:
    """The output names of a command (as its words name them)."""
    from larmorx_validation.stages.runners import output_names

    if program == "antsRegistration":
        return []  # the registration suite places its outputs itself (--output prefix)
    if program == "antsAI":
        i = _index(words, ("-o", "--output"))
        return [words[i + 1]] if i is not None else []
    if program == "Atropos":
        return []  # the Atropos suite places its outputs itself (--output [seg, post%02d])
    return output_names([program, *words])


def _index(words: list[str], names: tuple[str, ...]) -> int | None:
    for i, w in enumerate(words):
        if w in names and i + 1 < len(words):
            return i
    return None


def _placeholder(name: str) -> str:
    return "{out:" + os.path.basename(name) + "}"


def _mapped(run: str, node: str, program: str, command: str) -> tuple[list[str], list[str]]:
    """``command``'s arguments with container paths mapped and the outputs as placeholders."""
    r = oracle_run(run)
    words = shlex.split(command)[1:]
    outputs = _output_words(program, words)
    mapped = []
    for w in words:
        for name in outputs:
            # A word, or a bracketed part of one, that is exactly the output's name.
            pattern = rf"(?<![^\s\[,]){re.escape(name)}(?=[\s,\]]|$)"
            w = re.sub(pattern, lambda _m, n=name: _placeholder(n), w)
        mapped.append(r.host_text(w))
    return mapped, [_placeholder(n) for n in outputs]


@functools.cache
def calls(program: str) -> tuple[NodeCall, ...]:
    """Every node of every finished run that ran ``program``, in run and path order; calls
    whose arguments are the same word for word are kept once, the others named in
    ``duplicates``."""
    from larmorx_validation.stages.catalogue import normalise

    found: list[NodeCall] = []
    seen: dict[tuple[str, ...], int] = {}
    dups: dict[int, list[str]] = {}
    for run in finished_runs():
        for node, command in _commands(run):
            if _program_of(command) != program:
                continue
            args, outputs = _mapped(run, node, program, command)
            key = tuple(args)
            if key in seen:
                dups.setdefault(seen[key], []).append(f"{short(run)}/{label(node)}")
                continue
            seen[key] = len(found)
            found.append(
                NodeCall(run, node, program, tuple(args), tuple(outputs), normalise(node)[0])
            )
    return tuple(
        NodeCall(c.run, c.node, c.program, c.args, c.outputs, c.norm, tuple(dups.get(i, ())))
        for i, c in enumerate(found)
    )


def all_nodes(program: str) -> list[tuple[str, str]]:
    """Every ``(run, node)`` that ran ``program``, duplicates included."""
    return [
        (run, node)
        for run in finished_runs()
        for node, command in _commands(run)
        if _program_of(command) == program
    ]


def describe(call: NodeCall) -> str:
    """A case's description: the node, the run, and the calls it stands for."""
    text = (
        f"fMRIPrep 25.2.6's {call.program} node `{label(call.node)}` of the oracle run "
        f"{short(call.run)} ({call.norm}): its command.txt replayed, its inputs from the run's "
        "working directory"
    )
    if call.duplicates:
        text += f"; the same call as {len(call.duplicates)} other node(s): " + ", ".join(
            call.duplicates
        )
    return text


#: The case id prefix and category of fMRIPrep's calls in the panel tier.
PREFIX = "panel/fmriprep/"
CATEGORY = "panel-fmriprep"


def ants_cases(program: str, *, band_once: bool = False, **kw) -> list:
    """Every distinct call of ``program`` as an
    :class:`~larmorx_validation.parity.ants_programs.AntsCase` of the panel tier (the programs
    whose outputs the command names: ImageMath, ThresholdImage, MultiplyImages,
    N4BiasFieldCorrection, DenoiseImage, antsApplyTransforms); ``kw`` are further fields. With
    ``band_once``, ANTs' thread band (``AntsCase.band``) runs for the first call of each kind
    of node (its normalised path) only."""
    from larmorx_validation.parity.ants_programs import AntsCase

    out = []
    banded: set[str] = set()
    for c in calls(program):
        extra = dict(kw)
        if band_once:
            extra["band"] = c.norm not in banded
            banded.add(c.norm)
        out.append(
            AntsCase(
                PREFIX + c.id,
                CATEGORY,
                describe(c),
                program,
                c.args,
                tier="panel",
                outputs=c.outputs,
                **extra,
            )
        )
    return out


def record_lines(program: str, results, why_not: dict[str, str] | None = None) -> list[str]:
    """The inventory for a record (:func:`inventory_lines`) from a suite's results: nothing
    when the results hold no call of fMRIPrep's."""
    ids = {c.id for c in calls(program)}
    compared = {
        r.case[len(PREFIX) :]: r.status
        for r in results
        if r.case.startswith(PREFIX) and r.case[len(PREFIX) :] in ids
    }
    if not compared:
        return []
    return ["", *inventory_lines(program, compared, why_not or {})]


def inventory_lines(program: str, compared: dict[str, str], why_not: dict[str, str]) -> list[str]:
    """The record's inventory of ``program``'s nodes in the finished runs, grouped by the
    catalogue's normalised path: how many nodes, in how many runs, how many compared (their
    cases, ``compared``: node id -> case status) and why the others are not (``why_not``: node
    id or normalised path -> reason)."""
    from larmorx_validation.report import table
    from larmorx_validation.stages.catalogue import normalise

    groups: dict[str, list[tuple[str, str]]] = {}
    for run, node in all_nodes(program):
        groups.setdefault(normalise(node)[0], []).append((run, node))
    by_id = {c.id: c for c in calls(program)}
    dup_of = {d: c.id for c in calls(program) for d in c.duplicates}
    rows = []
    for norm, members in sorted(groups.items()):
        runs = sorted({r for r, _ in members})
        ids = [f"{short(r)}/{label(n)}" for r, n in members]
        n_cases = sum(1 for i in ids if i in by_id and i in compared)
        n_dup = sum(1 for i in ids if i in dup_of and dup_of[i] in compared)
        statuses: dict[str, int] = {}
        for i in ids:
            if i in compared:
                statuses[compared[i]] = statuses.get(compared[i], 0) + 1
        reasons = sorted(
            {
                why_not.get(i) or why_not.get(norm, "")
                for i in ids
                if i not in compared and i not in dup_of
            }
            - {""}
        )
        rows.append(
            (
                f"`{norm}`",
                len(members),
                len(runs),
                n_cases,
                n_dup,
                ", ".join(f"{v} {k}" for k, v in sorted(statuses.items())) or "–",
                "; ".join(reasons) or "–",
            )
        )
    total = len(all_nodes(program))
    n_runs = len({r for r, _ in all_nodes(program)})
    return [
        f"**fMRIPrep's {program} nodes in the {len(finished_runs())} finished oracle runs:** "
        f"{total} nodes in {n_runs} runs, {len(calls(program))} distinct calls (the others repeat "
        "one of them word for word and are compared through it).",
        "",
        table(
            (
                "Node (normalised path)",
                "Nodes",
                "Runs",
                "Compared",
                "Same call as a compared one",
                "Statuses",
                "Not compared: why",
            ),
            rows,
        ),
    ]


__all__ = [
    "CATEGORY",
    "ORACLE_RUNS",
    "PREFIX",
    "NodeCall",
    "all_nodes",
    "ants_cases",
    "calls",
    "describe",
    "finished_runs",
    "inventory_lines",
    "label",
    "oracle_run",
    "record_lines",
    "root",
    "short",
]
