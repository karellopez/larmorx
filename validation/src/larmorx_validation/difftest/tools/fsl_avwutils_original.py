# SPDX-License-Identifier: Apache-2.0
"""fslmaths and fslstats, the clean-room originals (``larmorx fsl fslmaths`` and ``fsl
fslstats``, crate larmorx-fsl, written from ``specs/fslmaths.md`` and ``specs/fslstats.md``)
against FSL 6.0.7.23: the replica descriptor's draws (:mod:`.fsl_avwutils`: random images of six
kinds and realistic chains of fslmaths' operations; realistic fslstats option lists with ``-t``,
``-K`` and ``-json``; fMRIPrep's and BET's forms; realistic cases only), each case run through
the original's suite (``larmorx_validation.parity.fsl_maths`` and ``fsl_stats``) and judged by
its thresholds (``specs/fslmaths.md`` §21: the exit status, both streams, the files, the headers,
the values bit for bit but where §21 bounds the sums, the double functions within one ulp, the
D13 fix pass; ``specs/fslstats.md`` §10: the text and JSON with §10's bound), not by
bit-identity.

**The same cases as the replica's** (``fsl-maths`` and ``fsl-stats``): case ``n`` of a seed is
drawn from the replica tool's stream for that seed and number (``fsl-maths:<seed>:<n>``, the
extreme-share draw consumed as the replica's is), so a trial with the seed of the replica's trial
(``fsl-maths-trial``, ``LARMORX_DIFFTEST_CACHE_MODE=use``) finds FSL's runs of the cases it drew
in the oracle cache: no original result is computed twice, and only the original runs."""

from __future__ import annotations

import dataclasses
import os

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool
from larmorx_validation.difftest.tools import fsl_avwutils

#: original tool -> the replica descriptor whose stream its cases are drawn from.
DRAWN_AS = {"fsl-maths-original": "fsl-maths", "fsl-stats-original": "fsl-stats"}


def _draw_as(name: str, fn):
    def draw(d: gen.Draw) -> dict:
        """The replica descriptor's case of the same seed and number (``d.key`` is
        ``<tool>:<seed>:<n>``)."""
        key = d.key
        if key.startswith(name + ":"):
            key = DRAWN_AS[name] + key[len(name) :]
        r = gen.Draw(key)
        # the night's draw of its extreme share, consumed first (core.make_recipe)
        r.rng.random()
        r.chosen = d.chosen
        r.extreme = d.extreme
        return fn(r)

    return draw


def _threads(module) -> None:
    # the worker's thread count (difftest's --threads, OMP_NUM_THREADS) for every run of the
    # original
    module.LX_THREADS = max(1, int(os.environ.get("OMP_NUM_THREADS") or 1))


def _run_maths(impl: str):
    from larmorx_validation.parity import fsl_maths as fo

    def run(case, checks) -> None:
        _threads(fo)
        fo.run_case(case, checks)

    return run


def _run_stats(impl: str):
    from larmorx_validation.parity import fsl_stats as so

    def run(case, checks) -> None:
        _threads(so)
        so.run_case(case, checks)

    return run


def _case_maths(recipe: dict, impl: str, *, expect: str = "pass", reason: str = "", **kw):
    """The replica descriptor's case; a permanent case expected to diverge (``expect``
    ``divergence``: only §21's share of voxels within 4 ulp may fail, for ``reason``) carries
    its reason in its payload, which the suite's
    :func:`~larmorx_validation.parity.fsl_maths.run_case` reads."""
    c = fsl_avwutils.case_maths(recipe, impl, **kw)
    if expect == "divergence":
        kind, payload = c.payload
        c = dataclasses.replace(c, payload=(kind, payload, reason or "an expected divergence"))
    return c


def _case_stats(recipe: dict, impl: str, *, expect: str = "pass", reason: str = "", **kw):
    return fsl_avwutils.case_stats(recipe, impl, **kw)


def _unavailable() -> str:
    from larmorx_validation.parity import fsl_versions

    if not fsl_versions.installed(fsl_versions.NEWEST):
        return f"FSL {fsl_versions.NEWEST} (~/fsl-{fsl_versions.NEWEST}) was not found"
    return ""


#: fslmaths' clean-room original through its suite: the record's thresholds, not bit-identity.
MATHS = Tool(
    name="fsl-maths-original",
    title="fslmaths (clean-room original)",
    suite="fsl-maths",
    implementations=("original",),
    draw=_draw_as("fsl-maths-original", fsl_avwutils.draw_maths),
    case=_case_maths,
    run=_run_maths,
    exact=lambda impl: False,
    exact_checks=(),
    timeout=300,
    weight=1.0,
    unavailable=_unavailable,
    family="fsl",
)

#: fslstats' clean-room original through its suite: §10's thresholds.
STATS_TOOL = Tool(
    name="fsl-stats-original",
    title="fslstats (clean-room original)",
    suite="fsl-stats",
    implementations=("original",),
    draw=_draw_as("fsl-stats-original", fsl_avwutils.draw_stats),
    case=_case_stats,
    run=_run_stats,
    exact=lambda impl: False,
    exact_checks=(),
    timeout=300,
    weight=0.6,
    unavailable=_unavailable,
    family="fsl",
)
