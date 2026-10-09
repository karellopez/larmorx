"""The ``larmorx`` command line and its alias ``lx``.

Arguments are parsed once, in Rust (crate ``larmorx-cli``), so the console scripts behave
exactly like the standalone ``larmorx`` binary.
"""

import sys
from collections.abc import Sequence

from larmorx import _core


def run(argv: Sequence[str]) -> int:
    """Run the command line with ``argv`` (program name first) and return the exit code."""
    code, out, err = _core.cli_main(list(argv))
    sys.stdout.write(out)
    sys.stderr.write(err)
    return code


def main() -> int:
    """Console-script entry point of ``larmorx`` and ``lx``."""
    return run(sys.argv)
