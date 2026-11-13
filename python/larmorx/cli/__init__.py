# SPDX-License-Identifier: Apache-2.0
"""The ``larmorx`` command line and its alias ``lx``.

``larmorx [--n-threads N] [--implementation auto|replica|clean-room] [--upstream-defects
fix|reproduce] <family> <tool> <the original's arguments>``. Arguments are parsed once, in Rust
(crate ``larmorx-cli``), so the console scripts behave exactly like the standalone ``larmorx``
binary: larmorx's own options stand before the family, with the Python API's names, and each one
given wins over the environment variable it replaces (``OMP_NUM_THREADS``,
``LARMORX_IMPLEMENTATION``, ``LARMORX_UPSTREAM_DEFECTS``). Tools with a bit-exact replica run its
program as a separate process when it is found (``docs/licensing.md``); it is also looked for in
this Python environment's scripts directory. A tool's messages appear while it runs, as from the
standalone program.

Python programs are run here, after Rust has read the options: ``larmorx sdcflows sdcflows ...``
is sdcflows' BIDS app (:func:`larmorx.sdcflows.run_app`) and ``larmorx nipype run ...`` nipype's
interface runner (:func:`larmorx.nipype.run`), with the options in their environment; the
standalone binary lists them and says they need this package.
"""

import sys
import sysconfig
from collections.abc import Sequence

from larmorx import _core


def _search_dirs() -> list[str]:
    """:func:`larmorx._replica.search_dirs`, without importing :mod:`larmorx._replica` when it
    is not loaded yet: its imports (``subprocess``, ``dataclasses``, ``typing``, ...) cost the
    command line about 2.3 MiB of its peak memory and nothing else (2026-10-30,
    ``validation/investigations/specs-memory/entry_memory.py``). Once it is loaded (the Python
    API, the tests), its function is the one asked."""
    replica = sys.modules.get("larmorx._replica")
    if replica is not None:
        return replica.search_dirs()
    scripts = sysconfig.get_path("scripts")
    return [scripts] if scripts else []


def run(argv: Sequence[str], *, implementation: str | None = None) -> int:
    """Run the command line with ``argv`` (program name first) and return the exit code; the
    tool's output goes to ``sys.stdout`` and ``sys.stderr`` while it runs.

    ``implementation`` (``"auto"``, ``"replica"`` or ``"clean-room"``; ``"original"``, its old
    spelling, with a ``DeprecationWarning``) chooses how tools with a replica run; ``None`` reads
    ``LARMORX_IMPLEMENTATION`` (default ``auto``). ``--implementation`` in ``argv`` wins over it.
    """
    argv = list(argv)
    if implementation == "original":
        from larmorx import _replica

        implementation = _replica.implementation_name(implementation)
    program = _core.cli_python_program(argv)
    if program is not None:
        family, tool, args, environment = program
        return _python_program(family, tool, args, implementation, environment, _program_name(argv))
    code, _, _ = _core.cli_main(argv, _search_dirs(), implementation, stream=True)
    return code


def _program_name(argv: Sequence[str]) -> str:
    """The name the command line was invoked as (``larmorx`` or ``lx``), as Rust's
    ``program_name``: the file's stem."""
    import os

    stem = os.path.splitext(os.path.basename(argv[0]))[0] if argv else ""
    return stem or "larmorx"


def _python_program(
    family: str,
    tool: str,
    args: list[str],
    implementation: str | None,
    environment: list[tuple[str, str | None]],
    prog: str,
) -> int:
    """sdcflows' BIDS app or nipype's interface runner, with ``implementation`` as its
    ``LARMORX_IMPLEMENTATION`` and the changes larmorx's own options make to its environment
    (``environment``: a value to set, or ``None`` to remove the variable; they win over
    ``implementation``)."""
    import os

    env = dict(os.environ)
    if implementation is not None:
        env["LARMORX_IMPLEMENTATION"] = implementation
    for name, value in environment:
        if value is None:
            env.pop(name, None)
        else:
            env[name] = value
    if (family, tool) == ("nipype", "run"):
        from larmorx.nipype import run as nipype_run

        return nipype_run(args, prog=f"{prog} {family} {tool}", env=env)
    from larmorx.sdcflows.app import run_app

    return run_app(args, env=env)


def main() -> int:
    """Console-script entry point of ``larmorx`` and ``lx``."""
    return run(sys.argv)
