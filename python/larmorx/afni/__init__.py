# SPDX-License-Identifier: Apache-2.0
"""AFNI-compatible tools: ``lx.afni``.

AFNI is partly GPL-2 (the files of the Medical College of Wisconsin), so the tools whose code
comes from such files have a clean-room re-implementation here, written from behaviour specs,
AFNI's documentation and black-box runs of AFNI 25.2.09 and 26.3.05 (the reference since
2026-10-29), without AFNI's code. The same programs run from the command line as
``larmorx afni <program> ...`` with their original arguments.

Bit-exact replicas translated from AFNI's source are a separate, GPL-3.0-or-later package,
``larmorx-gpl`` (``pip install "larmorx[exact]"``). With ``implementation="auto"`` (the
default) a tool runs its replica's program as a separate process when it is installed
(``docs/licensing.md``); nothing of it is imported, and without it the clean-room original
runs. ``tshift``, ``automask`` and ``unifize`` have both.
"""

from larmorx.afni.automask import AutomaskResult, automask
from larmorx.afni.tshift import METHODS, PATTERNS, fmriprep_slice_timing, tshift
from larmorx.afni.unifize import UnifizeResult, fmriprep_unifize_options, unifize

__all__ = [
    "METHODS",
    "PATTERNS",
    "AutomaskResult",
    "UnifizeResult",
    "automask",
    "fmriprep_slice_timing",
    "fmriprep_unifize_options",
    "tshift",
    "unifize",
]
