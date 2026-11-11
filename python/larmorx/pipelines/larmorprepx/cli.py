# SPDX-License-Identifier: Apache-2.0
"""The ``larmorprepx`` command: a preview, not ready yet (only ``--version`` and ``--help`` work).

In larmorx 0.1 the pipeline's field-map stages exist as Python functions
(:mod:`larmorx.pipelines.larmorprepx.fieldmaps`); the anatomical (FreeSurfer) and BOLD stages, the
confounds and the final validation against fMRIPrep come in later releases (:data:`ROADMAP`).
"""

import argparse
import importlib.util
import sys
from collections.abc import Sequence

from larmorx import __version__

#: Where the pipeline's plan is.
ROADMAP = "https://github.com/karellopez/larmorx/blob/main/docs/next-steps.md"

#: What the command says it is (its help and its message).
NOT_READY = (
    f"larmorprepx is not ready yet in larmorx {__version__}: its field-map stages exist as "
    "Python functions (larmorx.pipelines.larmorprepx.fieldmaps); the anatomical (FreeSurfer) and "
    f"BOLD stages, the confounds and the final validation come in later releases ({ROADMAP})."
)

#: Top-level modules provided by the ``[prep]`` extra (PLAN.md §2.2).
PREP_MODULES = ("templateflow", "matplotlib", "nilearn", "pandas", "jinja2")


def missing_prep_modules() -> list[str]:
    """Return the ``[prep]`` modules that are not installed."""
    return [name for name in PREP_MODULES if importlib.util.find_spec(name) is None]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="larmorprepx",
        description=f"fMRI preprocessing with larmorx (a preview). {NOT_READY}",
    )
    parser.add_argument("-V", "--version", action="version", version=f"larmorprepx {__version__}")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """Console-script entry point of ``larmorprepx``; returns the exit code."""
    build_parser().parse_args(argv)
    print(NOT_READY, file=sys.stderr)
    missing = missing_prep_modules()
    if missing:
        print(
            f"It will need the [prep] extra (missing: {', '.join(missing)}).\n"
            'Install it with: pip install "larmorx[prep]"',
            file=sys.stderr,
        )
    return 1
