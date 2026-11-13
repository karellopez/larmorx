# SPDX-License-Identifier: Apache-2.0
"""PRELUDE, the clean-room original (``larmorx fsl prelude``, crate larmorx-fsl, written from
``specs/prelude.md``) against FSL 6.0.7.23: the replica descriptor's draw and cases
(:mod:`.fsl_prelude`, used as a black box: its ``TOOL``'s ``draw`` and ``case``), each case run
through the original's suite (``larmorx_validation.parity.fsl_prelude``), which runs the replica
suite's check of it with larmorx's original standing in for the replica's program, FSL's runs from
the oracle cache.

**The same cases as the replica's** (``fsl-prelude``): case ``n`` of a seed is drawn from the
replica tool's stream for that seed and number (``fsl-prelude:<seed>:<n>``, the extreme-share draw
consumed as the replica's is), so a trial with the seed of the replica's trial finds FSL's runs of
every case it drew in the oracle cache: no original result is computed twice, and only the
original runs."""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool

#: The descriptor whose stream the cases are drawn from.
DRAWN_AS = "fsl-prelude"
NAME = "fsl-prelude-original"


def _replica_tool() -> Tool:
    from larmorx_validation.difftest.tools import fsl_prelude

    return fsl_prelude.TOOL


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
    return _replica_tool().draw(r)


def _case(recipe: dict, impl: str, **kw):
    """The replica descriptor's case, run through the original's suite."""
    from larmorx_validation.parity.harness import Case

    rep = _replica_tool()
    c = rep.case(recipe, rep.implementations[0], **kw)
    return Case(c.id, c.category, c.description, ("replica-suite", c))


def _run(impl: str):
    from larmorx_validation.parity import fsl_prelude as fo

    return fo.run_case


def _unavailable() -> str:
    from larmorx_validation.parity import fsl_versions

    if not fsl_versions.installed(fsl_versions.NEWEST):
        return f"FSL {fsl_versions.NEWEST} (~/fsl-{fsl_versions.NEWEST}) was not found"
    return ""


#: PRELUDE's clean-room original through its suite: the replica suite's checks of each case.
TOOL = Tool(
    name=NAME,
    title="PRELUDE (clean-room original)",
    suite="fsl-prelude",
    implementations=("original",),
    draw=draw,
    case=_case,
    run=_run,
    exact=lambda impl: False,
    exact_checks=(),
    timeout=600,
    weight=1.0,
    unavailable=_unavailable,
    family="fsl",
)
