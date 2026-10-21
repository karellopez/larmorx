# SPDX-License-Identifier: Apache-2.0
"""antsAI and antsAffineInitializer: their records' agreement and ANTs' own thread band.
No benchmark report yet: the page says so."""

from __future__ import annotations

from larmorx_validation.figures import ants_registration as reg
from larmorx_validation.figures import records, style
from larmorx_validation.figures.core import Context, Figure

LABELS = {"real": "real (fMRIPrep's)"}


def _agreement(rec: records.Record, program: str, name: str) -> Figure:
    from larmorx_validation.figures import common

    split = reg.headline_split(rec)
    return common.agreement_figure(
        rec,
        name=name,
        title=f"Agreement with {program}, by category",
        alt=f"Stacked bars, one per category of the {program} record ({len(rec.results)} "
        f"cases): {split['identical']} compared cases identical to ANTs as built"
        + (
            f", {split['identical-cm']} once ANTs uses correctly rounded maths"
            if split["identical-cm"]
            else ""
        )
        + ", rejected by both (ANTs' messages and exit statuses compared), and expected "
        "divergences.",
        original="ANTs",
        headline_split=split,
        labels=LABELS,
        state_labels={"agree": "identical, as built or with CORE-MATH (split in the first bar)"},
    )


def _band(ai: records.Record, init: records.Record) -> Figure:
    pts = [(f"antsAI: {c}", v) for c, v in reg.thread_band(ai)] + [
        (f"antsAffineInitializer: {c}", v) for c, v in reg.thread_band(init)
    ]

    def draw(theme: style.Theme, width: int):
        return reg._band_chart(theme, width, pts, "ANTs")

    return Figure(
        "thread-band",
        "ANTs' own variability: antsAI and antsAffineInitializer at 2, 4 and 8 threads",
        "For each case measured, the largest change in a transform parameter between ANTs at "
        "one thread and ANTs at 2, 4 and 8 threads, on a log scale; cases the threads do not "
        "change are counted above. larmorx gives the one-thread result at any thread count.",
        draw,
        records=ai.source() + init.source(),
    )


def figures(ctx: Context) -> list[Figure]:
    ai = records.load(ctx.repo, "ants-ai")
    init = records.load(ctx.repo, "ants-affine-initializer")
    return [
        _agreement(ai, "antsAI", "agreement-ai"),
        _agreement(init, "antsAffineInitializer", "agreement-initializer"),
        _band(ai, init),
    ]
