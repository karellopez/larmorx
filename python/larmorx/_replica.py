# SPDX-License-Identifier: Apache-2.0
"""Choosing between a tool's clean-room original and its bit-exact replica.

Some tools have two implementations (``docs/licensing.md``): the clean-room original, and a
replica translated from the upstream's source. Where the upstream's licence allows it, the
replica is part of this Apache-2.0 package and runs in this process, through
:mod:`larmorx._core` (*built in*: AFNI's 3dTshift, 3dAutomask and 3dUnifize since 2026-10-29,
when AFNI 26.3.05 released the Medical College of Wisconsin's code under CC BY 4.0). Otherwise
it is in a package of the upstream's licence family (``larmorx-nc``), which larmorx never
imports, loads or links: it runs that package's program as a separate process and exchanges
files with it, so the licences stay separate, and this module knows the program's name and
arguments, nothing more.

The registry of tools with a replica and the search for a separate program are shared with the
command line (``larmorx_cli::replica`` in Rust, through :mod:`larmorx._core`). The search
order is the program's environment variable (``LARMORX_NC_BIN``), then this Python
environment's scripts directory, then ``PATH``. It is done once per call; nothing is cached.

A tool's wrapper uses it in three steps, the same for both kinds (see ``larmorx.afni.tshift``)::

    selected = _replica.select("afni", "3dTshift", implementation)  # None: run the original
    if selected is not None:
        process = _replica.run(selected, args, cwd=tmp, n_threads=n_threads)
        ...  # read the output files; the result records selected.implementation()

A built-in replica runs with the arguments, the directory and the environment a separate
program would get (the environment as a table, never this process's own), and gives the same
files, messages and exit status.
"""

from __future__ import annotations

import os
import subprocess
import sysconfig
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from typing import Literal

from larmorx import _core, _defects

__all__ = [
    "IMPLEMENTATIONS",
    "ORIGINAL",
    "Implementation",
    "ImplementationName",
    "Replica",
    "ReplicaError",
    "ReplicaNotFoundError",
    "Selected",
    "find",
    "registry",
    "run",
    "search_dirs",
    "select",
]

#: The values of a wrapper's ``implementation`` argument.
IMPLEMENTATIONS = ("auto", "replica", "original")

ImplementationName = Literal["auto", "replica", "original"]


@dataclass(frozen=True)
class Implementation:
    """The implementation that computed a result (:attr:`larmorx.Image.implementation`).

    Attributes
    ----------
    kind
        ``"original"``: larmorx's clean-room code; ``"replica"``: the bit-exact replica, built in
        (in this process) or run as a separate program.
    package
        The distribution that provides it: ``"larmorx"`` (the originals and the built-in
        replicas), or the replica's (``"larmorx-nc"``).
    version
        That distribution's version (for a separate replica, what its program reports).
    licence
        Its code's licence, as an SPDX expression (a built-in AFNI replica:
        ``"Apache-2.0 AND CC-BY-4.0"``).
    program
        A separate replica's program, or ``None`` (the original, a built-in replica).
    """

    kind: Literal["original", "replica"]
    package: str
    version: str
    licence: str
    program: str | None = None


#: larmorx's own implementation.
ORIGINAL = Implementation("original", "larmorx", _core.__version__, "Apache-2.0")


class ReplicaNotFoundError(RuntimeError):
    """``implementation="replica"`` was asked for, and the separate replica's program was not
    found."""


class ReplicaError(RuntimeError):
    """The replica ran (built in, or its program was found), but it failed.

    larmorx never falls back to the original then: the two differ in the last bits, so a
    silent fallback would change results without notice.

    Attributes
    ----------
    command
        The program and its arguments.
    returncode
        The program's exit code.
    stderr
        What it wrote to standard error.
    """

    def __init__(self, message: str, *, command: Sequence[str], returncode: int, stderr: str):
        super().__init__(message)
        self.command = list(command)
        self.returncode = returncode
        self.stderr = stderr


@dataclass(frozen=True)
class Replica:
    """A tool with a replica: built in (``builtin``: in this process, ``program``, ``env`` and
    ``install`` ``None``), or ``<program> <family> <tool> <the tool's original arguments>``.
    ``original`` is false for a tool whose clean-room original is still to be written: the
    replica is then its only implementation."""

    family: str
    tool: str
    package: str
    program: str | None
    env: str | None
    licence: str
    install: str | None
    original: bool = True
    builtin: bool = False


def registry() -> dict[tuple[str, str], Replica]:
    """Every tool with a replica, by ``(family, tool)`` (``larmorx_cli::replica::REPLICAS``)."""
    return {(r["family"], r["tool"]): Replica(**r) for r in _core.replica_registry()}


def search_dirs() -> list[str]:
    """Where replica programs are looked for after their environment variable and before
    ``PATH``: this Python environment's scripts directory, where ``pip`` puts them."""
    scripts = sysconfig.get_path("scripts")
    return [scripts] if scripts else []


def find(family: str, tool: str) -> str | None:
    """The path of the replica program of ``family tool``, or ``None`` (not found, or built in).
    ``ValueError`` if the program's environment variable names something that is not a file."""
    found = _core.replica_find(family, tool, search_dirs())
    return None if found is None else os.fspath(found)


@dataclass(frozen=True)
class Selected:
    """A replica chosen to run, and the path of its program (``None``: built in)."""

    replica: Replica
    program: str | None

    def implementation(self) -> Implementation:
        """What :attr:`larmorx.Image.implementation` records: for a built-in replica, larmorx
        and its version; else the package and the version its program reports
        (``<package> <version>`` from ``--version``)."""
        r = self.replica
        if self.program is None:
            return Implementation("replica", "larmorx", _core.__version__, r.licence)
        command = [self.program, "--version"]
        process = _run(command)
        words = process.stdout.split()
        if process.returncode != 0 or len(words) != 2 or words[0] != self.replica.package:
            raise ReplicaError(
                f"{self.program} --version did not report {self.replica.package}'s version "
                f"(exit code {process.returncode}): {(process.stdout + process.stderr).strip()!r}",
                command=command,
                returncode=process.returncode,
                stderr=process.stderr,
            )
        return Implementation("replica", r.package, words[1], r.licence, self.program)


def select(family: str, tool: str, implementation: str) -> Selected | None:
    """The replica of ``family tool`` to run, or ``None`` to run the original.

    ``"auto"``: the replica if it is built in or its program is found, else ``None``;
    ``"replica"``: the replica, or :class:`ReplicaNotFoundError` with the command that installs
    it; ``"original"``: ``None``. A tool without a clean-room original
    (:attr:`Replica.original` false) has only its replica: ``"auto"`` without its program raises
    :class:`ReplicaNotFoundError`, and ``"original"`` raises ``NotImplementedError``.
    """
    if implementation not in IMPLEMENTATIONS:
        raise ValueError(
            f"implementation must be 'auto', 'replica' or 'original', not {implementation!r}"
        )
    replica = registry()[(family, tool)]
    if implementation == "original":
        if not replica.original:
            where = (
                "is built in"
                if replica.builtin
                else f"is in the {replica.package} package ({replica.licence}): install it "
                f"with {replica.install}"
            )
            raise NotImplementedError(
                f"larmorx has no clean-room original of {family} {tool} yet (it is to be "
                f"written from its spec in specs/); its bit-exact replica {where} and use "
                f"implementation='auto' or 'replica'"
            )
        return None
    if replica.builtin:
        return Selected(replica, None)
    program = find(family, tool)
    if program is not None:
        return Selected(replica, program)
    if not replica.original:
        raise ReplicaNotFoundError(
            f"larmorx has no clean-room original of {family} {tool} yet, and its replica's "
            f"{replica.program} program was not found ({replica.env}, "
            f"{', '.join(search_dirs()) or 'no scripts directory'}, PATH). Install the "
            f"{replica.package} package ({replica.licence}) with: {replica.install}"
        )
    if implementation == "replica":
        raise ReplicaNotFoundError(
            f"implementation='replica' needs the {replica.package} package ({replica.licence}), "
            f"whose {replica.program} program was not found ({replica.env}, "
            f"{', '.join(search_dirs()) or 'no scripts directory'}, PATH). Install it with: "
            f"{replica.install}"
        )
    return None


def run(
    selected: Selected,
    args: Sequence[str],
    *,
    cwd: str | os.PathLike[str],
    n_threads: int = 1,
    upstream_defects: str | None = None,
    env: Mapping[str, str | None] | None = None,
    exclude: Callable[[str], bool] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run ``<program> <family> <tool> args...`` in ``cwd`` (a built-in replica: the same, in this
    process, with that environment and directory given to it); :class:`ReplicaError` unless it
    exits with 0.

    The program gets this process's environment without the variables ``exclude`` names
    (``exclude(name)`` is true), with the changes in ``env`` (a value of ``None`` removes the
    variable). A wrapper whose replica reads the upstream's own variables excludes them all and
    passes its settings in ``env``, so that its result never depends on the caller's
    environment (CLAUDE.md rule 6). Replica programs take their thread count from
    ``OMP_NUM_THREADS`` (AFNI's variable): it is set to ``n_threads``, or removed for 0 (all
    logical CPUs). Their results do not depend on it. They take D13's choice (``"fix"`` or
    ``"reproduce"``, ``docs/api/upstream-defects.md``) from ``LARMORX_UPSTREAM_DEFECTS``, as the
    original's command line does: it is set to ``upstream_defects`` unless that is ``None``.
    """
    r = selected.replica
    command = [selected.program, r.family, r.tool, *args]
    child_env = {k: v for k, v in os.environ.items() if exclude is None or not exclude(k)}
    for name, value in (env or {}).items():
        if value is None:
            child_env.pop(name, None)
        else:
            child_env[name] = value
    if n_threads > 0:
        child_env["OMP_NUM_THREADS"] = str(n_threads)
    else:
        child_env.pop("OMP_NUM_THREADS", None)
    if upstream_defects is not None:
        child_env[_defects.ENV] = upstream_defects
    if selected.program is None:
        process = _run_builtin(r, list(args), cwd=cwd, env=child_env)
    else:
        process = _run(command, cwd=cwd, env=child_env)
    if process.returncode != 0:
        tail = "\n".join(process.stderr.strip().splitlines()[-20:])
        raise ReplicaError(
            f"the replica {r.package} {r.family} {r.tool} failed with exit code "
            f"{process.returncode}:\n{tail}",
            command=command,
            returncode=process.returncode,
            stderr=process.stderr,
        )
    return process


def _run_builtin(
    replica: Replica,
    args: list[str],
    *,
    cwd: str | os.PathLike[str],
    env: dict[str, str],
) -> subprocess.CompletedProcess[str]:
    """A built-in replica's run, as :func:`subprocess.run` would report a separate program's:
    in this process (without the GIL), with ``env`` as its environment and ``cwd`` as the
    directory relative names start in."""
    code, out, err = _core.replica_run_builtin(
        replica.family, replica.tool, args, sorted(env.items()), os.fspath(cwd)
    )
    command = ["larmorx", replica.family, replica.tool, *args]
    return subprocess.CompletedProcess(command, code, out, err)


def _run(
    command: list[str],
    *,
    cwd: str | os.PathLike[str] | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            command,
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
        )
    except OSError as e:
        raise ReplicaError(
            f"cannot run the replica program {command[0]}: {e}",
            command=command,
            returncode=-1,
            stderr="",
        ) from e
