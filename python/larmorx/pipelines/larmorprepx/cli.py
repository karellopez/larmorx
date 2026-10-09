# SPDX-License-Identifier: Apache-2.0
"""The ``larmorprepx`` command (stub: only ``--version`` and ``--help`` work)."""

import argparse
import importlib.util
import sys
from collections.abc import Sequence

from larmorx import __version__

#: Top-level modules provided by the ``[prep]`` extra (PLAN.md §2.2).
PREP_MODULES = ("templateflow", "matplotlib", "nilearn", "pandas", "jinja2")


def missing_prep_modules() -> list[str]:
    """Return the ``[prep]`` modules that are not installed."""
    return [name for name in PREP_MODULES if importlib.util.find_spec(name) is None]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="larmorprepx",
        description="fMRI preprocessing with larmorx (not implemented yet).",
    )
    parser.add_argument("-V", "--version", action="version", version=f"larmorprepx {__version__}")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Console-script entry point of ``larmorprepx``; returns the exit code."""
    build_parser().parse_args(argv)
    missing = missing_prep_modules()
    if missing:
        print(
            f"larmorprepx needs the [prep] extra (missing: {', '.join(missing)}).\n"
            'Install it with: pip install "larmorx[prep]"',
            file=sys.stderr,
        )
        return 1
    print("larmorprepx: the pipeline is not implemented yet.", file=sys.stderr)
    return 1
