# SPDX-License-Identifier: Apache-2.0
"""topup and applytopup, the clean-room original (``larmorx fsl topup`` and ``fsl applytopup``,
crate larmorx-fsl, written from ``specs/topup.md``) against FSL 6.0.7.23: the replica's draw
(:mod:`.fsl_topup`: reversed-phase-encoding acquisitions of an EPI-like phantom head distorted
by one off-resonance field, small movements; topup's per-level and global options, its
configurations and outputs, ``-v``, ``--debug``, ``--trace``; fMRIPrep's call; applytopup's five
methods on FSL's topup run of the case's own pair; realistic cases only), each case run through
the original's suite (``larmorx_validation.parity.fsl_topup``) and judged by its thresholds
(``specs/topup.md`` §12: the exit status, the files, both streams with numbers to 4 significant
digits, the headers and the log, the field, movements, corrected images, Jacobians and
displacement fields within their bounds, the D13 fix pass), not by bit-identity.

**The same cases as the replica's** (``fsl-topup``): case ``n`` of a seed is drawn from the
replica tool's stream for that seed and number (``fsl-topup:<seed>:<n>``, the extreme-share draw
consumed as the replica's is), so a trial with the seed of the replica's trial
(``topup-trial``, ``LARMORX_DIFFTEST_CACHE_MODE=use``) finds FSL's runs of every case it drew in
the oracle cache: no original result is computed twice, and only the original runs."""

from __future__ import annotations

import os

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool
from larmorx_validation.difftest.tools import fsl_topup

#: The descriptor whose stream the cases are drawn from.
DRAWN_AS = "fsl-topup"
NAME = "fsl-topup-original"


def draw(d: gen.Draw) -> dict:
    """The replica descriptor's case of the same seed and number (``d.key`` is
    ``<tool>:<seed>:<n>``)."""
    key = d.key
    if key.startswith(NAME + ":"):
        key = DRAWN_AS + key[len(NAME) :]
    r = gen.Draw(key)
    # the night's draw of its extreme share, consumed first (core.make_recipe)
    r.rng.random()
    r.chosen = d.chosen
    r.extreme = d.extreme
    return fsl_topup.draw(r)


def _run(impl: str):
    from larmorx_validation.parity import fsl_topup as fo

    def run(case, checks) -> None:
        # the worker's thread count (difftest's --threads, OMP_NUM_THREADS) for every run of the
        # original
        fo.LX_THREADS = max(1, int(os.environ.get("OMP_NUM_THREADS") or 1))
        fo.run_case(case, checks)

    return run


def _unavailable() -> str:
    from larmorx_validation.parity import fsl_versions

    if not fsl_versions.installed(fsl_versions.NEWEST):
        return f"FSL {fsl_versions.NEWEST} (~/fsl-{fsl_versions.NEWEST}) was not found"
    return ""


#: topup's and applytopup's clean-room original through its suite: the record's thresholds, not
#: bit-identity.
TOOL = Tool(
    name=NAME,
    title="topup and applytopup (clean-room original)",
    suite="fsl-topup",
    implementations=("original",),
    draw=draw,
    case=fsl_topup.case,
    run=_run,
    exact=lambda impl: False,
    exact_checks=(),
    timeout=600,
    weight=1.0,
    unavailable=_unavailable,
    family="fsl",
)
