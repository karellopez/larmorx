# SPDX-License-Identifier: Apache-2.0
"""Choosing between a tool's clean-room original and its bit-exact replica.

Some tools have two implementations (``docs/licensing.md``): the clean-room original in this
Apache-2.0 package, and a replica translated from the upstream's source, in a package of the
upstream's licence family (``larmorx-gpl``). larmorx never imports, loads or links a replica.
It runs the replica package's program as a separate process and exchanges files with it, so
the licences stay separate. This module knows the program's name and arguments, nothing more.

The registry of tools with a replica and the search for the program are shared with the
command line (``larmorx_cli::replica`` in Rust, through :mod:`larmorx._core`). The search
order is the program's environment variable (``LARMORX_GPL_BIN``), then this Python
environment's scripts directory, then ``PATH``. It is done once per call; nothing is cached.

A tool's wrapper uses it in three steps (see ``larmorx.afni.tshift``)::

    selected = _replica.select("afni", "3dTshift", implementation)  # None: run the original
    if selected is not None:
        process = _replica.run(selected, args, cwd=tmp, n_threads=n_threads)
        ...  # read the output files; the result records selected.implementation()
"""

from __future__ import annotations

import os
import subprocess
import sysconfig
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Literal

from larmorx import _core

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
        ``"original"``: larmorx's own code, in this process; ``"replica"``: the bit-exact
        replica, run as a separate program.
    package
        The distribution that provides it: ``"larmorx"``, or the replica's (``"larmorx-gpl"``).
    version
        That distribution's version (for a replica, what its program reports).
    licence
        Its licence, as an SPDX expression.
    program
        The replica's program, or ``None`` for the original.
    """

    kind: Literal["original", "replica"]
    package: str
    version: str
    licence: str
    program: str | None = None


#: larmorx's own implementation.
ORIGINAL = Implementation("original", "larmorx", _core.__version__, "Apache-2.0")


class ReplicaNotFoundError(RuntimeError):
    """``implementation="replica"`` was asked for, and the replica's program was not found."""


class ReplicaError(RuntimeError):
    """The replica's program was found, but it failed.

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
    """A tool with a replica: ``<program> <family> <tool> <the tool's original arguments>``."""

    family: str
    tool: str
    package: str
    program: str
    env: str
    licence: str
    install: str


def registry() -> dict[tuple[str, str], Replica]:
    """Every tool with a replica, by ``(family, tool)`` (``larmorx_cli::replica::REPLICAS``)."""
    return {(r["family"], r["tool"]): Replica(**r) for r in _core.replica_registry()}


def search_dirs() -> list[str]:
    """Where replica programs are looked for after their environment variable and before
    ``PATH``: this Python environment's scripts directory, where ``pip`` puts them."""
    scripts = sysconfig.get_path("scripts")
    return [scripts] if scripts else []


def find(family: str, tool: str) -> str | None:
    """The path of the replica program of ``family tool``, or ``None``. ``ValueError`` if the
    program's environment variable names something that is not a file."""
    found = _core.replica_find(family, tool, search_dirs())
    return None if found is None else os.fspath(found)


@dataclass(frozen=True)
class Selected:
    """A replica chosen to run, and the path of its program."""

    replica: Replica
    program: str

    def implementation(self) -> Implementation:
        """What :attr:`larmorx.Image.implementation` records: the package and the version its
        program reports (``<package> <version>`` from ``--version``)."""
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
        r = self.replica
        return Implementation("replica", r.package, words[1], r.licence, self.program)


def select(family: str, tool: str, implementation: str) -> Selected | None:
    """The replica of ``family tool`` to run, or ``None`` to run the original.

    ``"auto"``: the replica if its program is found, else ``None``; ``"replica"``: the replica,
    or :class:`ReplicaNotFoundError` with the command that installs it; ``"original"``:
    ``None``.
    """
    if implementation not in IMPLEMENTATIONS:
        raise ValueError(
            f"implementation must be 'auto', 'replica' or 'original', not {implementation!r}"
        )
    if implementation == "original":
        return None
    replica = registry()[(family, tool)]
    program = find(family, tool)
    if program is not None:
        return Selected(replica, program)
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
) -> subprocess.CompletedProcess[str]:
    """Run ``<program> <family> <tool> args...`` in ``cwd``; :class:`ReplicaError` unless it
    exits with 0.

    Replica programs take their thread count from ``OMP_NUM_THREADS`` (AFNI's variable): it is
    set to ``n_threads``, or removed for 0 (all logical CPUs). Their results do not depend on
    it.
    """
    r = selected.replica
    command = [selected.program, r.family, r.tool, *args]
    env = dict(os.environ)
    if n_threads > 0:
        env["OMP_NUM_THREADS"] = str(n_threads)
    else:
        env.pop("OMP_NUM_THREADS", None)
    process = _run(command, cwd=cwd, env=env)
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
