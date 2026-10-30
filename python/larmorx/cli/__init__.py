# SPDX-License-Identifier: Apache-2.0
"""The ``larmorx`` command line and its alias ``lx``.

Arguments are parsed once, in Rust (crate ``larmorx-cli``), so the console scripts behave
exactly like the standalone ``larmorx`` binary. Tools with a bit-exact replica run its program
as a separate process when it is found (``LARMORX_IMPLEMENTATION``, ``docs/licensing.md``);
it is also looked for in this Python environment's scripts directory.
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
    """Run the command line with ``argv`` (program name first) and return the exit code.

    ``implementation`` (``"auto"``, ``"replica"`` or ``"original"``) chooses how tools with a
    replica run; ``None`` reads ``LARMORX_IMPLEMENTATION`` (default ``auto``).
    """
    code, out, err = _core.cli_main(list(argv), _search_dirs(), implementation)
    sys.stdout.write(out)
    sys.stderr.write(err)
    return code


def main() -> int:
    """Console-script entry point of ``larmorx`` and ``lx``."""
    return run(sys.argv)
