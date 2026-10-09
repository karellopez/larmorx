# SPDX-License-Identifier: Apache-2.0
"""AFNI-compatible tools: ``lx.afni``.

AFNI is GPL-2, so these are clean-room re-implementations: written from behaviour specs,
AFNI's documentation and black-box runs of AFNI 25.2.09, without AFNI's code. The same
programs run from the command line as ``larmorx afni <program> ...`` with their original
arguments.
"""

from larmorx.afni.tshift import METHODS, PATTERNS, fmriprep_slice_timing, tshift

__all__ = ["METHODS", "PATTERNS", "fmriprep_slice_timing", "tshift"]
