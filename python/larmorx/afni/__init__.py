# SPDX-License-Identifier: Apache-2.0
"""AFNI-compatible tools: ``lx.afni``.

Each tool has two implementations (``docs/licensing.md``): a bit-exact replica translated
from AFNI 26.3.05's source (Apache-2.0, with CC BY 4.0's attribution for the code of the
Medical College of Wisconsin, which AFNI 26.3.05 releases under CC BY 4.0; part of larmorx
since 2026-10-29, until then the separate GPL package ``larmorx-gpl``), and a clean-room
original built for speed, written from behaviour specs, AFNI's documentation and black-box runs
of AFNI, without AFNI's code. ``implementation="auto"`` (the default) runs the replica;
``"replica"`` and ``"original"`` force one. The same programs run from the command line as
``larmorx afni <program> ...`` with their original arguments (``LARMORX_IMPLEMENTATION``
chooses). ``tshift``, ``automask`` and ``unifize`` have both.
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
