# SPDX-License-Identifier: Apache-2.0
"""Fixtures shared by the Python tests."""

from __future__ import annotations

import shutil
from pathlib import Path

import pytest

from larmorx import _core, _replica


@pytest.fixture
def no_replica(monkeypatch, tmp_path_factory):
    """No separate replica program can be found: neither through its environment variable, nor
    in the Python environment's scripts directory, nor on ``PATH``. So ``implementation="auto"``
    and the command line run the clean-room original of a tool whose replica is a separate
    program (FSL's), whatever this machine has installed. The AFNI tools' replicas are built in
    (``clean_room`` chooses their originals)."""
    monkeypatch.delenv("LARMORX_GPL_BIN", raising=False)
    monkeypatch.delenv("LARMORX_NC_BIN", raising=False)
    monkeypatch.delenv("LARMORX_IMPLEMENTATION", raising=False)
    monkeypatch.setattr(_replica, "search_dirs", list)
    monkeypatch.setenv("PATH", str(tmp_path_factory.mktemp("empty-path")))


@pytest.fixture
def clean_room(monkeypatch, no_replica):
    """The clean-room originals run: ``implementation="auto"`` chooses the original of a tool
    whose replica is built in (AFNI's 3dTshift, 3dAutomask, 3dUnifize), and the command line's
    ``LARMORX_IMPLEMENTATION`` is ``original``. For the tests written about the originals."""
    select = _replica.select

    def choose(family: str, tool: str, implementation: str) -> _replica.Selected | None:
        if implementation == "auto" and _replica.registry()[(family, tool)].builtin:
            implementation = "original"
        return select(family, tool, implementation)

    monkeypatch.setattr(_replica, "select", choose)
    monkeypatch.setenv("LARMORX_IMPLEMENTATION", "original")


class BuiltinStub:
    """A stand-in for the built-in replicas (``larmorx._core.replica_run_builtin``): it records
    each call (the arguments with the family and tool first, the thread count, D13's choice,
    AFNI's variables of the environment it is given, the directory), writes its standard error,
    and with exit status 0 copies its input to each output option it is given."""

    def __init__(self) -> None:
        self.calls: list[dict] = []
        self.exit = 0
        self.stdout = ""
        self.stderr = ""

    def __call__(
        self,
        family: str,
        tool: str,
        args: list[str],
        env: list[tuple[str, str]],
        cwd: str | None = None,
    ) -> tuple[int, str, str]:
        environment = dict(env)
        self.calls.append(
            {
                "args": [family, tool, *args],
                "threads": environment.get("OMP_NUM_THREADS"),
                "defects": environment.get("LARMORX_UPSTREAM_DEFECTS"),
                "afni": {
                    k: v
                    for k, v in environment.items()
                    if k.upper().startswith(("AFNI_", "NIML_")) or k.upper() == "IDCODE_PREFIX"
                },
                "cwd": cwd,
            }
        )
        if self.exit == 0:
            base = Path(cwd or ".")
            source = args[args.index("-input") + 1] if "-input" in args else args[-1]
            for option in ("-prefix", "-apply_prefix", "-depth", "-ssave", "-amsave"):
                if option in args:
                    shutil.copyfile(base / source, base / args[args.index(option) + 1])
        return self.exit, self.stdout, self.stderr


@pytest.fixture
def builtin_stub(monkeypatch) -> BuiltinStub:
    """:class:`BuiltinStub` in place of the built-in replicas, for the Python wrappers' runs."""
    stub = BuiltinStub()
    monkeypatch.setattr(_core, "replica_run_builtin", stub)
    return stub
