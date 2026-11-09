# SPDX-License-Identifier: Apache-2.0 AND MIT
"""sdcflows' BIDS app (the ``sdcflows`` program): the field maps of a BIDS dataset estimated and
written as derivatives, run as ``larmorx sdcflows sdcflows <bids_dir> <output_dir> participant
[options]`` (a Python program: the ``larmorx`` command of the Python package runs it; the
standalone Rust binary says so and stops).

Ported from sdcflows 2.17.1 (Apache-2.0, the NiPreps developers): ``cli/parser.py`` (every
option, its help, default and checks, through Python's own :mod:`argparse` as sdcflows uses it,
so the usage, help and error messages are argparse's of the Python that runs it, as sdcflows'
are), ``cli/main.py`` (the run, ``--dry-run``'s listing, the exit statuses), ``cli/workflow.py``
(``build_workflow``'s message), ``config.py`` (the run's settings: :class:`AppSettings`, one
object per run instead of sdcflows' module-wide singletons, written to the working directory as
sdcflows writes them, with toml 0.10.2's ``dumps``: :func:`toml_dumps`, ported from toml 0.10.2's
``encoder.py``, MIT, Copyright 2013-2019 William Pearson, 2015-2016 Julien Enselme, 2016 Google
Inc., 2017 Samuel Vasko, 2017 Nate Prewitt, 2017 Jack Evans, 2019 Filippo Broggini) and
``workflows/fit/base.py`` (``init_sdcflows_wf``: :func:`sdcflows_workflow`); niworkflows 1.15.4's
``utils/bids.py`` (:func:`larmorx.niworkflows.collect_participants`) and ``utils/debug.py``
(``setup_exceptionhook``, ``--pdb``).

What has no counterpart, on purpose: nipype (its plugins, scheduler, node messages, crash files,
``--use-plugin``'s file, which is checked but not read, and ``--mem-gb``'s memory limit:
recorded, unused), pybids' database (``--bids-database-dir`` must exist and is not read; the
dataset is indexed again in memory and no database is written), the telemetry (``--notrack`` is
recorded; larmorx never sends anything), the reports' figures (larmorprepx's reports part:
decision D7). The estimators run one after the other with ``--omp-nthreads`` threads each
(nipype runs ``--nprocs`` processes at once: the results are the same). sdcflows replaces
Python's ``warnings.warn`` for the whole run and its warnings go nowhere; here the ported
libraries' warnings are hidden the same way, and larmorx's notices of corrected or reproduced
defects (:class:`larmorx.UpstreamDefectWarning`, D13) are shown on standard error. An exception
sdcflows does not catch (a Python traceback, exit status 1) is written as the traceback's first
and last lines (the exception with the original's module name), exit status 1; ``--pdb`` prints
the whole traceback and opens Python's debugger when the terminal is interactive, as
niworkflows' hook does.

Corrected by default (``LARMORX_UPSTREAM_DEFECTS=reproduce``, ``upstream_defects="reproduce"``,
gives sdcflows' behaviour):

- SDC-29 (rule B): the app builds fieldmap-less (SyN) estimators, which ``find_estimators`` gives
  a subject without field maps unless ``--no-fmapless``, without their inputs: their
  preprocessing needs an anatomical preprocessing the app does not have (fMRIPrep wires it), so
  they fail when run, after the other estimators' work has started. Here the run stops before any
  work with a clear message naming them and ``--no-fmapless``.
- SDC-30 (rule A): ``--session-label`` is recorded and never used: every session is estimated.
  Here only the sessions given.
- SDC-31 (rule A): ``--bids-filter-file`` is read and never used. Here its ``fmap`` entry
  restricts the field maps (``find_estimators``' ``bids_filters``), as fMRIPrep applies the same
  file's ``fmap`` entry to the same search.
- SDC-32 (rule A): ``--sloppy`` and ``--debug`` are recorded and every estimator is built with
  ``sloppy=False, debug=False``. Here they reach the estimators' workflows.
- SDC-33 (rule B): option values the app accepts and then fails on with a Python traceback
  (exit status 1) instead of argparse's usage error (exit status 2): a ``--mem-gb`` unit
  sdcflows' ``_to_gb`` does not know (``16g``, ``1.5G``, ``16 GB``: ``KeyError``), a
  ``--bids-filter-file`` that does not exist or is not JSON, a ``--use-plugin`` file that does
  not exist. Here argparse's error.
- SDC-35 (rule A): ``--use-plugin`` always stops the app: sdcflows reads the file with PyYAML's
  ``load`` without the ``Loader`` argument PyYAML 6 requires (``TypeError``, exit status 1).
  Here the file is accepted (it must exist; nipype's plugins have no counterpart, so it changes
  nothing).

- SDC-36 (rule A): every participant's estimators are recorded in one registry, so two
  participants that share a ``B0FieldIdentifier`` (BIDS asks it to be unique only within a
  participant) stop the app with ``KeyError``. Here each participant has its own (the automatic
  names ``auto_NNNNN`` stay sdcflows').

Not reproduced (nondeterminism, SDC-34): argparse names a non-integer ``--nprocs`` or
``--omp-nthreads`` value's converter by its ``functools.partial`` representation, which holds a
memory address (``invalid _min_one value`` here); participant labels not found are listed in a
Python ``set``'s order, and the settings file's ``analysis_level`` too (both sorted here).
"""

from __future__ import annotations

import contextlib
import json
import logging
import os
import re
import sys
import tempfile
import time
import traceback
import uuid
import warnings
from argparse import Action, ArgumentDefaultsHelpFormatter, ArgumentParser
from collections.abc import Iterator, Mapping, Sequence
from dataclasses import dataclass, field, fields
from functools import partial
from pathlib import Path
from typing import Any, ClassVar

from larmorx import _defects
from larmorx._defects import UpstreamDefectsName

__all__ = [
    "PROG",
    "SDCFLOWS_VERSION",
    "AppSettings",
    "FieldmaplessError",
    "SdcflowsWorkflow",
    "build_parser",
    "parse_args",
    "run_app",
    "sdcflows_workflow",
    "toml_dumps",
]

#: The sdcflows release whose command line this is.
SDCFLOWS_VERSION = "2.17.1"
#: The program's name in its messages (sdcflows' console script).
PROG = "sdcflows"
#: ``os.EX_USAGE`` and ``os.EX_SOFTWARE`` (the numbers, also where ``os`` lacks them).
EX_USAGE = 64
EX_SOFTWARE = 70
#: The environment variable that chooses the numpy and SciPy releases whose arithmetic the
#: estimators reproduce: ``newest`` (numpy 2.5.4, SciPy 1.18.1, the default) or ``shared``
#: (numpy 2.3.5, SciPy 1.15.3), numpy's LAPACK in nibabel's ``set_qform`` included (3.12.1 and
#: 3.12.0: :func:`larmorx.io.nibabel_header.use_lapack`).
LIBRARIES_ENV = "LARMORX_SDCFLOWS_LIBRARIES"
_LIBRARIES = {"newest": ("2.5.4", "1.18.1"), "shared": ("2.3.5", "1.15.3")}
#: The LAPACK release of each library set's numpy (OpenBLAS 0.3.34's and 0.3.30's).
_LAPACK = {"newest": "3.12.1", "shared": "3.12.0"}
_IMPLEMENTATION_ENV = "LARMORX_IMPLEMENTATION"

# The modules of the originals' exceptions (a traceback's last line names them).
_ORIGINAL_MODULES = {
    "larmorx.bids.layout": "bids.exceptions",
    "larmorx.niworkflows.participants": "niworkflows.utils.bids",
    "larmorx.sdcflows.estimators": "sdcflows.fieldmaps",
}


class FieldmaplessError(ValueError):
    """SDC-29: fieldmap-less estimators the app cannot run (they need the SyN preprocessing's
    inputs, which sdcflows' app does not compute)."""


# --- the command line (sdcflows' cli/parser.py) ------------------------------------------------


def _parse_participant_labels(value: str) -> list[str]:
    """Drop ``sub-`` prefixes of participant labels (sorted, without duplicates)."""
    return sorted({item.removeprefix("sub-") for item in value.split()})


def build_parser(*, upstream_defects: UpstreamDefectsName = "fix") -> ArgumentParser:
    """sdcflows' ``_parser()``: every option with its help and default, ``prog`` ``sdcflows``.
    With ``upstream_defects="fix"`` a ``--mem-gb`` unit sdcflows does not know is argparse's
    error (SDC-33); the converters of ``--nprocs``, ``--omp-nthreads`` and the paths are named
    ``_min_one`` and ``_path_exists`` in argparse's messages (SDC-34)."""
    mode = _defects.check(upstream_defects)

    class ParticipantLabelAction(Action):
        def __call__(self, parser, namespace, values, option_string=None):  # type: ignore[no-untyped-def]
            setattr(namespace, self.dest, _parse_participant_labels(" ".join(values)))

    def _path_exists(path: Any, parser: ArgumentParser) -> Path:
        """Ensure a given path exists."""
        if path is None or not Path(path).exists():
            raise parser.error(f"Path does not exist: <{path}>.")  # type: ignore[func-returns-value]
        return Path(path).expanduser().absolute()

    def _min_one(value: Any, parser: ArgumentParser) -> int:
        """Ensure an argument is not lower than 1."""
        value = int(value)
        if value < 1:
            raise parser.error("Argument can't be less than one.")  # type: ignore[func-returns-value]
        return value  # type: ignore[no-any-return]

    def _to_gb(value: str) -> float:
        scale = {"G": 1, "T": 10**3, "M": 1e-3, "K": 1e-6, "B": 1e-9}
        digits = "".join([c for c in value if c.isdigit()])
        n_digits = len(digits)
        units = value[n_digits:] or "G"
        if mode == "fix":
            n = int(digits)
            if units[0] not in scale:  # SDC-33: sdcflows' KeyError
                raise ValueError(units)
            return n * scale[units[0]]
        return int(digits) * scale[units[0]]

    parser = ArgumentParser(
        prog=PROG,
        description=f"""\
SDCFlows {SDCFLOWS_VERSION}

Estimate fieldmaps available in a BIDS-compliant MRI dataset.""",
        formatter_class=ArgumentDefaultsHelpFormatter,
    )
    PathExists = partial(_path_exists, parser=parser)
    PositiveInt = partial(_min_one, parser=parser)
    # SDC-34: argparse names a converter without __name__ by its repr (a memory address)
    PathExists.__name__ = "_path_exists"  # type: ignore[attr-defined]
    PositiveInt.__name__ = "_min_one"  # type: ignore[attr-defined]

    parser.add_argument(
        "bids_dir",
        action="store",
        type=PathExists,
        help="The root folder of a BIDS valid dataset (sub-XXXXX folders should "
        "be found at the top level in this folder).",
    )
    parser.add_argument(
        "output_dir",
        action="store",
        type=Path,
        help="The directory where the output files "
        "should be stored. If you are running group level analysis "
        "this folder should be prepopulated with the results of the "
        "participant level analysis.",
    )
    parser.add_argument(
        "analysis_level",
        action="store",
        nargs="+",
        help="Level of the analysis that will be performed. "
        "Multiple participant level analyses can be run independently "
        "(in parallel) using the same output_dir.",
        choices=["participant", "group"],
    )

    # optional arguments
    parser.add_argument("--version", action="version", version=f"SDCFlows {SDCFLOWS_VERSION}")
    parser.add_argument(
        "-v",
        "--verbose",
        dest="verbose_count",
        action="count",
        default=0,
        help="Increases log verbosity for each occurrence, debug level is -vvv.",
    )

    # main options
    g_bids = parser.add_argument_group("Options related to BIDS")
    g_bids.add_argument(
        "--participant-label",
        action=ParticipantLabelAction,
        nargs="+",
        help="A space delimited list of participant identifiers or a single "
        "identifier (the sub- prefix can be removed).",
    )
    g_bids.add_argument(
        "--session-label",
        action="store",
        nargs="*",
        type=str,
        help="Filter input dataset by session label.",
    )
    g_bids.add_argument(
        "--bids-filter-file",
        action="store",
        type=Path,
        metavar="PATH",
        help="a JSON file describing custom BIDS input filter using pybids "
        "{<suffix>:{<entity>:<filter>,...},...} "
        "(https://github.com/bids-standard/pybids/blob/master/bids/layout/config/bids.json)",
    )
    g_bids.add_argument(
        "--bids-database-dir",
        metavar="PATH",
        type=PathExists,
        help="Path to an existing PyBIDS database folder, for faster indexing "
        "(especially useful for large datasets).",
    )
    g_bids.add_argument(
        "--bids-database-wipe",
        action="store_true",
        default=False,
        help="Wipe out previously existing BIDS indexing caches, forcing re-indexing.",
    )

    # General performance
    g_perfm = parser.add_argument_group("Options to handle performance")
    g_perfm.add_argument(
        "--nprocs",
        action="store",
        type=PositiveInt,
        help="""\
Maximum number of simultaneously running parallel processes executed by *SDCFlows* \
(e.g., several instances of ANTs' registration). \
However, when ``--nprocs`` is greater or equal to the ``--omp-nthreads`` option, \
it also sets the maximum number of threads that simultaneously running processes \
may aggregate (meaning, with ``--nprocs 16 --omp-nthreads 8`` a maximum of two \
8-CPU-threaded processes will be running at a given time). \
Under this mode of operation, ``--nprocs`` sets the maximum number of processors \
that can be assigned work within an *SDCFlows* job, which includes all the processors \
used by currently running single- and multi-threaded processes. \
If ``None``, the number of CPUs available will be automatically assigned (which may \
not be what you want in, e.g., shared systems like a HPC cluster.""",
    )
    g_perfm.add_argument(
        "--omp-nthreads",
        action="store",
        type=PositiveInt,
        help="""\
Maximum number of threads that multi-threaded processes executed by *SDCFlows* \
(e.g., ANTs' registration) can use. \
If ``None``, the number of CPUs available will be automatically assigned (which may \
not be what you want in, e.g., shared systems like a HPC cluster.""",
    )
    g_perfm.add_argument(
        "--mem-gb",
        dest="memory_gb",
        action="store",
        type=_to_gb,
        help="Upper bound memory limit for SDCFlows processes.",
    )
    g_perfm.add_argument(
        "--debug",
        action="store_true",
        default=False,
        help="Enable changes to processing to aid in debugging",
    )
    g_perfm.add_argument(
        "--pdb",
        dest="pdb",
        action="store_true",
        default=False,
        help="Open Python debugger (pdb) on exceptions.",
    )
    g_perfm.add_argument(
        "--sloppy",
        action="store_true",
        default=False,
        help="Use sloppy mode for minimal footprint.",
    )

    # Control instruments
    g_outputs = parser.add_argument_group("Instrumental options")
    g_outputs.add_argument(
        "-w",
        "--work-dir",
        action="store",
        type=Path,
        default=Path("work").absolute(),
        help="Path where intermediate results should be stored.",
    )
    g_outputs.add_argument(
        "-n",
        "--dry-run",
        action="store_true",
        default=False,
        help="only find estimable fieldmaps (that is, estimation is not triggered)",
    )
    g_outputs.add_argument(
        "--no-fmapless",
        action="store_false",
        dest="fmapless",
        default=True,
        help="Allow fieldmap-less estimation",
    )
    g_outputs.add_argument(
        "--use-plugin",
        action="store",
        default=None,
        type=Path,
        help="Nipype plugin configuration file.",
    )
    g_outputs.add_argument(
        "--notrack",
        action="store_true",
        help="Opt-out of sending tracking information of this run to the NiPreps developers. "
        "This information helps to improve SDCFlows and provides an indicator of "
        "real world usage for obtaining funding.",
    )

    return parser


# --- the settings (sdcflows' config.py) ------------------------------------------------------


def _meminfo() -> dict[str, float]:
    out: dict[str, float] = {}
    with contextlib.suppress(OSError, ValueError, IndexError):
        for line in Path("/proc/meminfo").read_text(encoding="utf-8").splitlines():
            name, _, rest = line.partition(":")
            out[name] = float(rest.split()[0])
    return out


def _overcommit() -> tuple[str, str]:
    limit, policy = "n/a", "n/a"
    with contextlib.suppress(Exception):
        oc = Path("/proc/sys/vm/overcommit_memory")
        if oc.exists():
            policy = {"0": "heuristic", "1": "always", "2": "never"}.get(
                oc.read_text(encoding="utf-8").strip(), "unknown"
            )
            if policy != "never":
                kb = Path("/proc/sys/vm/overcommit_kbytes")
                if kb.exists():
                    limit = kb.read_text(encoding="utf-8").strip()
                ratio = Path("/proc/sys/vm/overcommit_ratio")
                if limit in ("0", "n/a") and ratio.exists():
                    limit = "{}%".format(ratio.read_text(encoding="utf-8").strip())
    return policy, limit


def _total_memory() -> float | None:
    with contextlib.suppress(Exception):
        if "linux" in sys.platform:
            return _meminfo()["MemTotal"] / (1024.0**2)
        if "darwin" in sys.platform:
            import subprocess

            text = subprocess.run(
                ["sysctl", "hw.memsize"], capture_output=True, text=True, check=False
            ).stdout
            return float(text.strip().split(" ")[-1]) / (1024.0**3)
    return None


@dataclass
class EnvironmentSettings:
    """sdcflows' ``config.environment``: the platform (written to the settings file only).
    nipype's and TemplateFlow's versions have no counterpart; ``larmorx_version`` is added.
    ``free_mem`` is ``MemFree`` (psutil's ``free`` on Linux; elsewhere not known)."""

    cpu_count: int | None = None
    exec_docker_version: str | None = None
    exec_env: str = os.name
    free_mem: float | None = None
    overcommit_policy: str = "n/a"
    overcommit_limit: str = "n/a"
    total_memory: float | None = None
    version: str = SDCFLOWS_VERSION
    larmorx_version: str = ""

    @classmethod
    def detect(cls, env: Mapping[str, str]) -> EnvironmentSettings:
        """The settings of this machine and process environment, as sdcflows reads them."""
        from larmorx import __version__

        exec_env, docker = os.name, None
        if env.get("IS_DOCKER_8395080871"):
            exec_env = "singularity"
            cgroup = Path("/proc/1/cgroup")
            with contextlib.suppress(OSError):
                if cgroup.exists() and "docker" in cgroup.read_text(encoding="utf-8"):
                    docker = env.get("DOCKER_VERSION_8395080871")
                    exec_env = "docker"
        free = _meminfo().get("MemFree")
        policy, limit = _overcommit()
        return cls(
            cpu_count=os.cpu_count(),
            exec_docker_version=docker,
            exec_env=exec_env,
            free_mem=None if free is None else round(free * 1024 / 1024**3, 1),
            overcommit_policy=policy,
            overcommit_limit=limit,
            total_memory=_total_memory(),
            larmorx_version=__version__,
        )


@dataclass
class ExecutionSettings:
    """sdcflows' ``config.execution``: the run's folders, the dataset, the participants and the
    switches of the command line. ``bids_database_dir`` is recorded only when given (no
    database is written); ``layout`` is the dataset's :class:`larmorx.bids.BIDSLayout`."""

    ants_float: bool = False
    bids_dir: Path | None = None
    bids_database_dir: Path | None = None
    bids_database_wipe: bool = False
    bids_description_hash: str | None = None
    bids_filters: dict[str, Any] | None = None
    cwd: str = ""
    debug: bool = False
    dry_run: bool = False
    layout: Any = None
    log_dir: Path | None = None
    log_level: int = 25
    notrack: bool = False
    output_dir: Path | None = None
    participant_label: list[str] | None = None
    pdb: bool = False
    run_uuid: str = ""
    session_label: list[str] | None = None
    sloppy: bool = False
    templateflow_home: Path | None = None
    work_dir: Path = field(default_factory=lambda: Path("work").absolute())
    write_graph: bool = False

    _paths = (
        "bids_dir",
        "bids_database_dir",
        "layout",
        "log_dir",
        "output_dir",
        "templateflow_home",
        "work_dir",
    )


@dataclass
class WorkflowSettings:
    """sdcflows' ``config.workflow``."""

    analysis_level: list[str] = field(default_factory=lambda: ["participant"])
    fmapless: bool = False
    species: str = "human"
    template_id: str = "MNI152NLin2009cAsym"

    _paths = ()


@dataclass
class NipypeSettings:
    """sdcflows' ``config.nipype`` (nipype's settings: recorded; ``nprocs`` and
    ``omp_nthreads`` are the run's processes and threads)."""

    crashfile_format: str = "txt"
    get_linked_libs: bool = False
    local_hash_check: bool = True
    memory_gb: float | None = None
    nprocs: int | None = None
    omp_nthreads: int = 1
    plugin: str = "MultiProc"
    plugin_args: dict[str, Any] = field(
        default_factory=lambda: {"maxtasksperchild": 1, "raise_insufficient": False}
    )
    remove_node_directories: bool = False
    resource_monitor: bool = False
    stop_on_first_crash: bool = True

    _paths = ()


def _section(obj: Any) -> dict[str, Any]:
    """sdcflows' ``_Config.get()``: the settings that are not ``None``, paths as text."""
    out: dict[str, Any] = {}
    for f in fields(obj):
        v = getattr(obj, f.name)
        if v is None:
            continue
        if f.name in getattr(obj, "_paths", ()):
            v = str(v)
        out[f.name] = v
    return out


@dataclass
class AppSettings:
    """One run's settings: sdcflows' ``config`` module's four sections (no global state) and
    the choices larmorx adds (``upstream_defects``, ``implementation``, ``libraries``)."""

    environment: EnvironmentSettings
    execution: ExecutionSettings
    workflow: WorkflowSettings
    nipype: NipypeSettings
    upstream_defects: UpstreamDefectsName = "fix"
    implementation: str = "auto"
    libraries: str = "newest"
    use_plugin: Path | None = None

    def get(self) -> dict[str, dict[str, Any]]:
        """sdcflows' ``config.get()``: the sections as dictionaries, in its order."""
        return {
            "environment": _section(self.environment),
            "execution": _section(self.execution),
            "workflow": _section(self.workflow),
            "nipype": _section(self.nipype),
        }

    def dumps(self) -> str:
        """sdcflows' ``config.dumps()``: the settings as TOML text (:func:`toml_dumps`)."""
        return toml_dumps(self.get())


# --- toml 0.10.2's dumps (MIT; the settings file's text) -------------------------------------


def _toml_str(v: Any) -> str:
    v = f"{v!r}"
    if v[0] == "u":
        v = v[1:]
    singlequote = v.startswith("'")
    if singlequote or v.startswith('"'):
        v = v[1:-1]
    if singlequote:
        v = v.replace("\\'", "'")
        v = v.replace('"', '\\"')
    parts = v.split("\\x")
    while len(parts) > 1:
        i = -1
        if not parts[0]:
            parts = parts[1:]
        parts[0] = parts[0].replace("\\\\", "\\")
        joinx = parts[0][i] != "\\"
        while parts[0][:i] and parts[0][i] == "\\":
            joinx = not joinx
            i -= 1
        joiner = "x" if joinx else "u00"
        parts = [parts[0] + joiner + parts[1], *parts[2:]]
    return '"' + parts[0] + '"'


def _toml_float(v: float) -> str:
    return f"{v}".replace("e+0", "e+").replace("e-0", "e-")


def _toml_value(v: Any) -> str:
    t = type(v)
    if t is str:
        return _toml_str(v)
    if t is bool:
        return str(v).lower()
    if t is int:
        return str(v)
    if t is float:
        return _toml_float(v)
    if t is list or hasattr(v, "__iter__"):
        return "[" + "".join(" " + _toml_value(u) + "," for u in v) + "]"
    return _toml_str(v)


def _toml_sections(o: Mapping[str, Any], sup: str) -> tuple[str, dict[str, Any]]:
    retstr = ""
    if sup != "" and sup[-1] != ".":
        sup += "."
    retdict: dict[str, Any] = {}
    arraystr = ""
    for section in o:
        section = str(section)
        qsection = section
        if not re.match(r"^[A-Za-z0-9_-]+$", section):
            qsection = _toml_str(section)
        value = o[section]
        if not isinstance(value, dict):
            arrayoftables = isinstance(value, list) and any(isinstance(a, dict) for a in value)
            if arrayoftables:
                for a in value:
                    arraytabstr = "\n"
                    arraystr += "[[" + sup + qsection + "]]\n"
                    s, d = _toml_sections(a, sup + qsection)
                    if s:
                        if s[0] == "[":
                            arraytabstr += s
                        else:
                            arraystr += s
                    while d:
                        newd: dict[str, Any] = {}
                        for dsec in d:
                            s1, d1 = _toml_sections(d[dsec], sup + qsection + "." + dsec)
                            if s1:
                                arraytabstr += "[" + sup + qsection + "." + dsec + "]\n"
                                arraytabstr += s1
                            for s2 in d1:
                                newd[dsec + "." + s2] = d1[s2]
                        d = newd
                    arraystr += arraytabstr
            elif value is not None:
                retstr += qsection + " = " + _toml_value(value) + "\n"
        else:
            retdict[qsection] = value
    retstr += arraystr
    return retstr, retdict


def toml_dumps(o: Mapping[str, Any]) -> str:
    """toml 0.10.2's ``dumps`` (the text sdcflows writes its settings with): tables, values of
    the types JSON and the settings hold (text, numbers, booleans, lists, tables, arrays of
    tables; ``None`` values left out), toml's quoting and number formats."""
    retval, sections = _toml_sections(o, "")
    outer = [id(o)]
    while sections:
        ids = [id(s) for s in sections.values()]
        if any(x in ids for x in outer):
            raise ValueError("Circular reference detected")
        outer += ids
        newsections: dict[str, Any] = {}
        for section in sections:
            add, addsections = _toml_sections(sections[section], section)
            if add or (not add and not addsections):
                if retval and retval[-2:] != "\n\n":
                    retval += "\n"
                retval += "[" + section + "]\n"
                if add:
                    retval += add
            for s in addsections:
                newsections[section + "." + s] = addsections[s]
        sections = newsections
    return retval


# --- parsing and its checks (sdcflows' parse_args) -------------------------------------------


def _layout(execution: ExecutionSettings) -> Any:
    """sdcflows' ``execution.init()``: the dataset indexed with its app's ignore patterns (only
    ``sub-<label>`` folders, without ``beh``, ``pet``, ``perf``, ``meg``, ``eeg``, ``ieeg``,
    ``micr``, ``nirs``), the BIDS validator off."""
    from larmorx.bids import BIDSLayout, BIDSLayoutIndexer

    if execution.bids_filters is None:
        execution.bids_filters = {}
    ignore_paths = [
        re.compile(r"^(?!/sub-[a-zA-Z0-9]+)"),
        re.compile(r"^/sub-[a-zA-Z0-9]+(/ses-[a-zA-Z0-9]+)?/(beh|pet|perf|meg|eeg|ieeg|micr|nirs)"),
    ]
    if execution.participant_label:
        ignore_paths[0] = re.compile(r"^(?!/sub-(" + "|".join(execution.participant_label) + "))")
    indexer = BIDSLayoutIndexer(validate=False, ignore=ignore_paths)
    return BIDSLayout(str(execution.bids_dir), indexer=indexer)


def parse_args(
    args: Sequence[str] | None = None,
    *,
    upstream_defects: UpstreamDefectsName = "fix",
    env: Mapping[str, str] | None = None,
) -> AppSettings:
    """sdcflows' ``parse_args``: the command line parsed and checked, the dataset indexed, the
    output, log and working folders made, the participants checked; the run's settings.

    argparse's errors and ``--help``/``--version`` raise ``SystemExit`` (argparse's), as in
    sdcflows; a dataset without ``dataset_description.json`` raises pybids' error.
    ``env`` is the process environment the settings read (``OMP_NUM_THREADS``,
    ``TEMPLATEFLOW_HOME``; by default ``os.environ``)."""
    mode = _defects.check(upstream_defects)
    env = os.environ if env is None else env
    parser = build_parser(upstream_defects=mode)
    opts = parser.parse_args(args)

    omp_default = int(env.get("OMP_NUM_THREADS", os.cpu_count()))  # type: ignore[arg-type]
    tf_home = Path(
        env.get("TEMPLATEFLOW_HOME", os.path.join(Path.home(), ".cache", "templateflow"))
    )
    execution = ExecutionSettings(
        cwd=os.getcwd(),
        run_uuid="{}_{}".format(time.strftime("%Y%m%d-%H%M%S"), uuid.uuid4()),
        templateflow_home=tf_home.absolute(),
    )
    workflow = WorkflowSettings()
    nipype = NipypeSettings(nprocs=os.cpu_count(), omp_nthreads=omp_default)
    execution.log_level = int(max(25 - 5 * opts.verbose_count, logging.DEBUG))

    # config.from_dict(vars(opts)): the settings sdcflows' sections have
    values = vars(opts)
    for section in (execution, workflow, nipype):
        names = {f.name for f in fields(section)}
        for k, v in values.items():
            if v is None or k not in names:
                continue
            if k in section._paths:
                v = Path(v).absolute()
            setattr(section, k, v)
    layout = _layout(execution)  # execution.init(), inside sdcflows' from_dict

    implementation = env.get(_IMPLEMENTATION_ENV, "") or "auto"
    libraries = env.get(LIBRARIES_ENV, "") or "newest"
    if libraries not in _LIBRARIES:
        parser.error(f"{LIBRARIES_ENV} must be one of {sorted(_LIBRARIES)}, not {libraries!r}")
    settings = AppSettings(
        EnvironmentSettings.detect(env),
        execution,
        workflow,
        nipype,
        upstream_defects=mode,
        implementation=implementation,
        libraries=libraries,
        use_plugin=opts.use_plugin,
    )

    # Load base plugin_settings from file if --use-plugin (checked, not read: no nipype)
    if opts.use_plugin is not None:
        if mode == "fix" and not Path(opts.use_plugin).is_file():  # SDC-33
            parser.error(f"Path does not exist: <{opts.use_plugin}>.")
        with open(opts.use_plugin, encoding="utf-8"):
            if mode == "reproduce":  # SDC-35: PyYAML 6's load needs its Loader argument
                raise TypeError("load() missing 1 required positional argument: 'Loader'")

    # Load BIDS filters
    if opts.bids_filter_file:
        path = opts.bids_filter_file
        if mode == "fix":  # SDC-33
            if not Path(path).is_file():
                parser.error(f"Path does not exist: <{path}>.")
            try:
                execution.bids_filters = json.loads(Path(path).read_text())
            except (json.JSONDecodeError, UnicodeDecodeError):
                parser.error(f"JSON syntax error in: <{path}>.")
        else:
            execution.bids_filters = json.loads(Path(path).read_text())

    bids_dir = execution.bids_dir
    output_dir = execution.output_dir
    work_dir = execution.work_dir
    assert bids_dir is not None and output_dir is not None

    # Ensure input and output folders are not the same
    if output_dir == bids_dir:
        suggested_path = bids_dir / "derivatives" / f"sdcflows_{SDCFLOWS_VERSION.split('+')[0]}"
        parser.error(
            "The selected output folder is the same as the input BIDS folder. "
            f"Please modify the output path (suggestion: {suggested_path})."
        )

    if bids_dir in work_dir.parents:
        parser.error(
            "The selected working directory is a subdirectory of the input BIDS folder. "
            "Please modify the output path."
        )

    # Setup directories
    execution.log_dir = output_dir / "logs"
    execution.log_dir.mkdir(exist_ok=True, parents=True)
    output_dir.mkdir(exist_ok=True, parents=True)
    work_dir.mkdir(exist_ok=True, parents=True)
    execution.layout = layout

    participant_label = layout.get_subjects()
    if execution.participant_label is not None:
        selected_label = set(execution.participant_label)
        missing_subjects = selected_label - set(participant_label)
        if missing_subjects:
            parser.error(  # SDC-34: sorted, not a set's order
                "One or more participant labels were not found in the BIDS directory: "
                f"{', '.join(sorted(missing_subjects))}."
            )
        participant_label = selected_label

    execution.participant_label = sorted(participant_label)

    # Handle analysis_level (SDC-34: sorted, not a set's order)
    workflow.analysis_level = sorted(set(workflow.analysis_level))
    return settings


# --- the workflow (sdcflows' init_sdcflows_wf) -----------------------------------------------


@dataclass(frozen=True)
class SdcflowsWorkflow:
    """``init_sdcflows_wf``'s graph, built and not yet run (:meth:`run`): the participants, the
    estimators of each one (``estimators``, in sdcflows' order), every estimator's workflow
    (``workflows``, in the same order: :class:`larmorx.sdcflows.EstimatorWorkflow`), and where
    the derivatives (``output_dir/sdcflows``) and the nodes' files (``work_dir/sdcflows_wf``)
    go."""

    subjects: tuple[str, ...]
    estimators: dict[str, tuple[Any, ...]]
    workflows: tuple[Any, ...]
    output_dir: Path
    work_dir: Path | None
    upstream_defects: UpstreamDefectsName = "fix"
    n_threads: int = 1

    @property
    def all_estimators(self) -> tuple[Any, ...]:
        """Every participant's estimators, in order (one per workflow)."""
        return tuple(e for sub in self.subjects for e in self.estimators[sub])

    def run(self) -> tuple[Any, ...]:
        """Run every estimator's workflow, then its derivatives (``fmap_derivatives_<id>``: the
        field map, reference, mask and coefficients under ``output_dir/sdcflows``, named by all
        its sources), one after the other; the first failure stops the run (nipype's
        ``stop_on_first_crash``). One :class:`larmorx.sdcflows.EstimatorRun` per estimator."""
        from larmorx.sdcflows.base import EstimatorRun
        from larmorx.sdcflows.outputs import fmap_derivatives_workflow

        tmp = None
        if self.work_dir is None:
            tmp = tempfile.TemporaryDirectory(prefix="larmorx-sdcflows-")
            root = Path(tmp.name) / "sdcflows_wf"
        else:
            root = Path(self.work_dir) / "sdcflows_wf"
        try:
            runs = []
            used: set[str] = set()
            subjects = [sub for sub in self.subjects for _ in self.estimators[sub]]
            for sub, estimator, wf in zip(
                subjects, self.all_estimators, self.workflows, strict=True
            ):
                # SDC-36: an identifier two participants share gets its own folder
                folder = root if wf.name not in used else root / f"sub_{sub}"
                used.add(wf.name)
                result = wf.run(folder)
                derivs = fmap_derivatives_workflow(
                    [str(Path(f.path).absolute()) for f in estimator.sources],
                    os.fspath(result.fmap),
                    os.fspath(result.fmap_ref),
                    [os.fspath(c) for c in result.fmap_coeff],
                    os.fspath(result.fmap_mask),
                    [f.metadata for f in estimator.sources],
                    output_dir=self.output_dir,
                    bids_fmap_id=estimator.bids_id,
                    write_coeff=True,
                    write_mask=True,
                    out_dir=folder / f"fmap_derivatives_{estimator.sanitized_id}",
                    upstream_defects=self.upstream_defects,
                    n_threads=self.n_threads,
                )
                runs.append(EstimatorRun(estimator, wf, result, derivs))
        finally:
            if tmp is not None:
                tmp.cleanup()
        return tuple(runs)


def _find_all(
    layout: Any,
    subjects: Sequence[str],
    *,
    sessions: list[str] | None,
    fmapless: bool,
    logger: logging.Logger | None,
    bids_filters: Mapping[str, Any] | None,
    mode: UpstreamDefectsName,
) -> dict[str, tuple[Any, ...]]:
    """Every participant's estimators (``find_estimators``), in order. sdcflows records them all
    in one registry (its globals), so two participants that share a ``B0FieldIdentifier`` (BIDS
    asks it to be unique only within a participant) stop it with ``KeyError`` (SDC-36, rule A).
    Corrected: each participant has a registry of its own, which also holds the earlier
    participants' automatic names (``auto_NNNNN``), so the names are sdcflows' wherever sdcflows
    runs; ``reproduce``: one registry."""
    from larmorx.sdcflows.estimators import FieldmapRegistry
    from larmorx.sdcflows.wrangler import find_estimators

    shared = FieldmapRegistry()
    autos: dict[str, Any] = {}
    seen: dict[str, str] = {}
    record: dict[str, tuple[Any, ...]] = {}
    for subject in subjects:
        if mode == "reproduce":
            reg = shared
        else:
            reg = FieldmapRegistry()
            for k, v in autos.items():
                reg.estimators[k] = v
        record[subject] = tuple(
            find_estimators(
                layout=layout,
                subject=subject,
                sessions=sessions,
                fmapless=fmapless,
                logger=logger,
                bids_filters=bids_filters,
                registry=reg,
                upstream_defects=mode,
            )
        )
        if mode == "reproduce":
            continue
        for e in record[subject]:
            if e.bids_id.startswith("auto_"):
                autos[e.bids_id] = reg.estimators[e.bids_id]
            elif e.bids_id in seen:
                warnings.warn(
                    _defects.UpstreamDefectWarning(
                        "SDC-36",
                        "sdcflows",
                        "fix",
                        f"sub-{seen[e.bids_id]} and sub-{subject} share the B0FieldIdentifier "
                        f"{e.bids_id!r}: sdcflows' app stops with KeyError",
                    ),
                    stacklevel=3,
                )
            else:
                seen[e.bids_id] = subject
    return record


def sdcflows_workflow(
    layout: Any,
    *,
    output_dir: str | os.PathLike[str],
    participant_label: Sequence[str] | None = None,
    work_dir: str | os.PathLike[str] | None = None,
    fmapless: bool = True,
    sessions: Sequence[str] | None = None,
    bids_filters: Mapping[str, Any] | None = None,
    omp_nthreads: int = 1,
    sloppy: bool = False,
    debug: bool = False,
    logger: logging.Logger | None = None,
    upstream_defects: UpstreamDefectsName = "fix",
    n_threads: int = 1,
    **workflow_options: Any,
) -> SdcflowsWorkflow:
    """sdcflows' ``init_sdcflows_wf`` (its BIDS app's graph), built: the participants of
    ``layout`` (:func:`larmorx.niworkflows.collect_participants` with ``participant_label``),
    each one's estimators (:func:`larmorx.sdcflows.find_estimators` with ``fmapless``,
    ``sessions``, ``bids_filters`` and ``logger``; recorded in new registries, as a new sdcflows
    process starts with empty globals: one per participant, SDC-36) and each estimator's workflow (:func:`larmorx.sdcflows.estimator_workflow` with
    ``omp_nthreads``, ``sloppy``, ``debug`` and ``workflow_options``: the libraries' versions,
    the implementations, ``fsldir``, ...). sdcflows' app gives ``sessions``, ``bids_filters``,
    ``sloppy`` and ``debug`` their defaults (SDC-30 to SDC-32 in :mod:`larmorx.sdcflows.app`).

    SDC-29: fieldmap-less estimators cannot run here (their inputs come from fMRIPrep's SyN
    preprocessing): :class:`FieldmaplessError` before any work; ``upstream_defects="reproduce"``
    builds them as sdcflows does, and :meth:`SdcflowsWorkflow.run` then fails on the first."""
    from larmorx.niworkflows import collect_participants
    from larmorx.sdcflows.base import estimator_workflow
    from larmorx.sdcflows.estimators import EstimatorType

    mode = _defects.check(upstream_defects)
    subjects = collect_participants(layout, participant_label)
    record = _find_all(
        layout,
        subjects,
        sessions=None if sessions is None else list(sessions),
        fmapless=fmapless,
        logger=logger,
        bids_filters=bids_filters,
        mode=mode,
    )
    anat = [
        (sub, e.bids_id) for sub in subjects for e in record[sub] if e.method == EstimatorType.ANAT
    ]
    if anat and mode == "fix":
        listed = "; ".join(
            f"sub-{sub}: {', '.join(i for s, i in anat if s == sub)}"
            for sub in dict.fromkeys(s for s, _ in anat)
        )
        raise FieldmaplessError(
            "SDC-29: sdcflows' command line cannot estimate fieldmap-less (SyN) estimators: "
            "their inputs come from an anatomical preprocessing it does not run (sdcflows "
            f"builds them without inputs and they fail when run). Found: {listed}. Run with "
            "--no-fmapless (fmapless=False) to estimate the others, or estimate them with "
            "larmorx.sdcflows.syn_preprocessing_workflow and syn_sdc_workflow (fMRIPrep's wiring)"
        )
    workflows = []
    for sub in subjects:
        for estim in record[sub]:
            workflows.append(
                estimator_workflow(
                    estim,
                    omp_nthreads=omp_nthreads,
                    sloppy=sloppy,
                    debug=debug,
                    upstream_defects=mode,
                    n_threads=n_threads,
                    **workflow_options,
                )
            )
    return SdcflowsWorkflow(
        tuple(subjects),
        record,
        tuple(workflows),
        Path(output_dir),
        None if work_dir is None else Path(work_dir),
        mode,
        n_threads,
    )


# --- the run (sdcflows' cli/main.py) ---------------------------------------------------------


class _Formatter(logging.Formatter):
    """sdcflows' log format, with its level names 25 ``IMPORTANT`` and 15 ``VERBOSE`` (sdcflows
    adds them to :mod:`logging` for the whole process; here only its own lines carry them)."""

    _NAMES: ClassVar[dict[int, str]] = {25: "IMPORTANT", 15: "VERBOSE"}

    def format(self, record: logging.LogRecord) -> str:
        if record.levelno in self._NAMES and record.levelname.startswith("Level "):
            record.levelname = self._NAMES[record.levelno]
        return super().format(record)


_FMT = "%(asctime)s,%(msecs)d %(name)-2s %(levelname)-2s:\n\t %(message)s"
_DATEFMT = "%y%m%d-%H:%M:%S"


def _cli_logger(level: int, stream: Any | None) -> logging.Logger:
    """sdcflows' ``cli`` logger, of this run only: its lines on ``stream`` (standard output)
    with sdcflows' format, or, without a stream, as Python's last-resort handler prints them
    (standard error, warnings and above: sdcflows' parent process before its loggers start)."""
    logger = logging.Logger("cli", level)
    if stream is not None:
        handler = logging.StreamHandler(stream=stream)
        handler.setFormatter(_Formatter(fmt=_FMT, datefmt=_DATEFMT))
        logger.addHandler(handler)
    return logger


def _exception_lines(e: BaseException) -> str:
    """A traceback's first and last lines: the exception named with the original's module."""
    t = type(e)
    module = _ORIGINAL_MODULES.get(t.__module__, t.__module__)
    name = t.__qualname__ if module in ("builtins", "__main__") else f"{module}.{t.__qualname__}"
    text = str(e)
    return "Traceback (most recent call last):\n" + (f"{name}: {text}" if text else name) + "\n"


def _interactive() -> bool:
    return sys.stdin.isatty() and sys.stdout.isatty() and sys.stderr.isatty()


@contextlib.contextmanager
def _warnings_as_sdcflows() -> Iterator[None]:
    """sdcflows' ``_warnings`` module: Python's warnings go to a logger without output; larmorx's
    notices of corrected or reproduced defects are shown."""
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        warnings.filterwarnings("default", category=_defects.UpstreamDefectWarning)
        yield


def _workflow_options(settings: AppSettings, env: Mapping[str, str]) -> dict[str, Any]:
    np_v, sp_v = _LIBRARIES[settings.libraries]
    impl = settings.implementation
    return {
        "numpy_version": np_v,
        "scipy_version": sp_v,
        "prelude_implementation": impl,
        "topup_implementation": impl,
        "volreg_implementation": impl,
        "fsldir": env.get("FSLDIR") or None,
    }


def _dry_run(settings: AppSettings, logger: logging.Logger) -> int:
    from larmorx.niworkflows import collect_participants

    ex = settings.execution
    sessions, filters = _app_filters(settings)
    subjects = collect_participants(ex.layout, ex.participant_label)
    estimators_record = _find_all(
        ex.layout,
        subjects,
        sessions=sessions,
        fmapless=settings.workflow.fmapless,
        logger=logger,
        bids_filters=filters,
        mode=settings.upstream_defects,
    )
    bids_dir = Path(ex.bids_dir)  # type: ignore[arg-type]
    print(f"Estimation for <{bids_dir}> complete. Found:")
    for subject, estimators in estimators_record.items():
        print(f"\tsub-{subject}")
        if not estimators:
            print("\t\tNo estimators found")
            continue
        for estimator in estimators:
            print(f"\t\t{estimator}")
            for fl in estimator.sources:
                fl_relpath = Path(fl.path).relative_to(bids_dir / f"sub-{subject}")
                pe_dir = fl.metadata.get("PhaseEncodingDirection")
                print(f"\t\t\t{pe_dir}\t{fl_relpath}")
    return 0


def _app_filters(settings: AppSettings) -> tuple[list[str] | None, dict[str, Any] | None]:
    """The sessions and BIDS filters of the estimators' search: SDC-30 and SDC-31 (sdcflows'
    app passes neither)."""
    if settings.upstream_defects == "reproduce":
        return None, None
    ex = settings.execution
    sessions = list(ex.session_label) if ex.session_label else None
    filters = (ex.bids_filters or {}).get("fmap") or None
    return sessions, filters


def _run(settings: AppSettings, env: Mapping[str, str]) -> int:
    ex, wfs, ny = settings.execution, settings.workflow, settings.nipype
    mode = settings.upstream_defects
    logger = _cli_logger(ex.log_level, sys.stdout)
    logger.log(
        25,
        f"""\
Running SDCFlows {SDCFLOWS_VERSION}:
             * BIDS dataset path: {ex.bids_dir}.
             * Output folder: {ex.output_dir}.
             * Analysis levels: {wfs.analysis_level}.
""",
    )
    sessions, filters = _app_filters(settings)
    fix = mode == "fix"
    wf = sdcflows_workflow(
        ex.layout,
        output_dir=ex.output_dir,  # type: ignore[arg-type]
        participant_label=ex.participant_label,
        work_dir=ex.work_dir,
        fmapless=wfs.fmapless,
        sessions=sessions,
        bids_filters=filters,
        omp_nthreads=ny.omp_nthreads,
        sloppy=ex.sloppy and fix,  # SDC-32
        debug=ex.debug and fix,
        logger=logger,
        upstream_defects=mode,
        n_threads=ny.omp_nthreads,
        **_workflow_options(settings, env),
    )
    if not wf.workflows:
        # sdcflows' parent process logs it before its loggers start: Python's last resort
        sys.stderr.write(
            "Workflow did not generate any jobs. Please check your inputs are valid.\n"
        )
        return EX_USAGE
    nprocs = int(ny.nprocs or 1)
    if ny.plugin in ("MultiProc", "LegacyMultiProc") and 1 < nprocs < ny.omp_nthreads:
        logger.warning(
            "Per-process threads (--omp-nthreads=%d) exceed total threads (--nthreads/--n_cpus=%d)",
            ny.omp_nthreads,
            nprocs,
        )
    _run_workflow(wf, settings.libraries)
    logger.log(25, "Finished all workload")
    return 0


def _run_workflow(wf: Any, libraries: str) -> None:
    """``wf.run()`` with nibabel's ``set_qform`` arithmetic on the library set's LAPACK
    (``use_lapack``, a context variable: set in a copy of the caller's context, so it does not
    outlive the run)."""
    import contextvars

    from larmorx.io.nibabel_header import use_lapack

    def run() -> None:
        use_lapack(_LAPACK[libraries])
        wf.run()

    contextvars.copy_context().run(run)


def _write_settings(settings: AppSettings) -> Path:
    """The settings file sdcflows writes into the working folder (``tempfile.mktemp``'s name,
    ``.sdcflows.<8 characters>.toml``, created as a plain file is)."""
    import random

    chars = "abcdefghijklmnopqrstuvwxyz0123456789_"
    rng = random.Random()
    text = settings.dumps()
    while True:
        name = ".sdcflows." + "".join(rng.choice(chars) for _ in range(8)) + ".toml"
        path = Path(settings.execution.work_dir) / name
        try:
            with open(path, "x", encoding="utf-8") as f:
                f.write(text)
        except FileExistsError:
            continue
        return path


def _main(
    argv: Sequence[str] | None,
    mode: UpstreamDefectsName,
    env: Mapping[str, str],
    state: dict[str, Any],
) -> int:
    settings = parse_args(argv, upstream_defects=mode, env=env)
    ex = settings.execution
    if ex.pdb:  # niworkflows' setup_exceptionhook
        state["pdb"] = True
        settings.nipype.plugin = "Linear"
    _write_settings(settings)
    if settings.workflow.analysis_level != ["participant"]:
        raise ValueError("Analysis level can only be 'participant'")
    if ex.dry_run:
        return _dry_run(settings, _cli_logger(logging.WARNING, None))
    return _run(settings, env)


def run_app(
    argv: Sequence[str] | None = None,
    *,
    upstream_defects: UpstreamDefectsName | None = None,
    env: Mapping[str, str] | None = None,
) -> int:
    """Run sdcflows' command line with ``argv`` (its arguments, without the program's name;
    by default ``sys.argv[1:]``) and return its exit status: 0, argparse's 2, 64 when no
    estimator was found (``os.EX_USAGE``), 1 for an error (its message on standard error).
    ``upstream_defects`` defaults to ``LARMORX_UPSTREAM_DEFECTS`` (else ``"fix"``); ``env`` is
    the environment the run reads (by default ``os.environ``: ``OMP_NUM_THREADS``,
    ``TEMPLATEFLOW_HOME``, ``FSLDIR``, ``LARMORX_IMPLEMENTATION``, ``LARMORX_SDCFLOWS_LIBRARIES``).
    """
    env = os.environ if env is None else env
    mode = upstream_defects or env.get(_defects.ENV, "") or "fix"
    if mode not in _defects.UPSTREAM_DEFECTS:
        sys.stderr.write(f"error: {_defects.ENV} must be 'fix' or 'reproduce', not {mode!r}\n")
        return 2
    state: dict[str, Any] = {"pdb": False}
    try:
        with _warnings_as_sdcflows():
            return _main(argv, mode, env, state)  # type: ignore[arg-type]
    except SystemExit as e:
        return _exit_status(e.code)
    except FieldmaplessError as e:
        sys.stderr.write(f"{PROG}: error: {e}\n")
        return 1
    except Exception as e:
        sys.stdout.flush()
        if state["pdb"]:
            traceback.print_exception(e)
            print()
            if _interactive():
                import pdb

                pdb.post_mortem(e.__traceback__)
        else:
            sys.stderr.write(_exception_lines(e))
        return 1


def _exit_status(code: Any) -> int:
    """``sys.exit``'s status: ``None`` 0, an integer itself, anything else printed and 1."""
    if code is None:
        return 0
    if isinstance(code, int):
        return code
    sys.stderr.write(f"{code}\n")
    return 1
