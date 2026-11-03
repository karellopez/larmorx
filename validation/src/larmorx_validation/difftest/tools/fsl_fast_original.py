# SPDX-License-Identifier: Apache-2.0
"""FAST (``fast``), the clean-room original (``larmorx fsl fast``, crate larmorx-fsl, written
from ``specs/fast.md``) against FSL 6.0.7.23: the replica's draw (:mod:`.fsl_fast`: fMRIPrep's
call; one channel of a T1-, T2- or PD-like brain with two to four classes and FAST's options;
one-slice brains; two to four channels; FSL's standard priors with ``-a`` and ``-P``; the usual
stored types, geometries and output types; realistic cases only), each case run through the
original's suite (``larmorx_validation.parity.fsl_fast``) and judged by its thresholds
(``specs/fast.md`` §11: the exit status, the files, both streams with numbers to 4 significant
digits, the segmentations' Dice and identical share, the partial volumes, probabilities, bias
field and restored image within their bounds, FSL's last-bit band with correctly rounded maths,
the D13 fix pass), not by bit-identity."""

from __future__ import annotations

from larmorx_validation.difftest.core import Tool
from larmorx_validation.difftest.tools import fsl_fast


def _run(impl: str):
    from larmorx_validation.parity import fsl_fast as fo

    def run(case, checks) -> None:
        fo.run_case(case, checks)

    return run


def _unavailable() -> str:
    from larmorx_validation.parity import fsl_versions

    if not fsl_versions.installed(fsl_versions.NEWEST):
        return f"FSL {fsl_versions.NEWEST} (~/fsl-{fsl_versions.NEWEST}) was not found"
    return ""


#: FAST's clean-room original through its suite: the record's thresholds, not bit-identity.
TOOL = Tool(
    name="fsl-fast-original",
    title="FAST (clean-room original)",
    suite="fsl-fast",
    implementations=("original",),
    draw=fsl_fast.draw,
    case=fsl_fast.case,
    run=_run,
    exact=lambda impl: False,
    exact_checks=(),
    timeout=600,
    weight=1.0,
    unavailable=_unavailable,
    family="fsl",
)
