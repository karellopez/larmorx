# SPDX-License-Identifier: Apache-2.0
"""The ``larmorx`` command line and its alias ``lx``.

Arguments are parsed once, in Rust (crate ``larmorx-cli``), so the console scripts behave
exactly like the standalone ``larmorx`` binary. Tools with a bit-exact replica run its program
as a separate process when it is found (``LARMORX_IMPLEMENTATION``, ``docs/licensing.md``);
it is also looked for in this Python environment's scripts directory.
"""

import sys
from collections.abc import Sequence

from larmorx import _core, _replica


def run(argv: Sequence[str], *, implementation: str | None = None) -> int:
    """Run the command line with ``argv`` (program name first) and return the exit code.

    ``implementation`` (``"auto"``, ``"replica"`` or ``"original"``) chooses how tools with a
    replica run; ``None`` reads ``LARMORX_IMPLEMENTATION`` (default ``auto``).
    """
    code, out, err = _core.cli_main(list(argv), _replica.search_dirs(), implementation)
    sys.stdout.write(out)
    sys.stderr.write(err)
    return code


def main() -> int:
    """Console-script entry point of ``larmorx`` and ``lx``."""
    return run(sys.argv)
