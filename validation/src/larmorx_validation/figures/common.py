# SPDX-License-Identifier: Apache-2.0
"""Figures several tools share: the agreement overview built from a record, the differences
against the original's thread band, speed from a benchmark report, and a pending notice."""

from __future__ import annotations

import math
from collections.abc import Callable
from pathlib import Path

from larmorx_validation.figures import charts, records, style
from larmorx_validation.figures.core import Figure, workspace

DS000005 = {
    "dataset": "OpenNeuro ds000005 (Tom et al., 2007), snapshot 00001",
    "subject": "sub-01",
    "licence": "PDDL-1.0",
}
MNI_NOTICE = (
    "MNI ICBM 152 nonlinear asymmetric 2009c (Fonov et al., 2009, 2011), from TemplateFlow; "
    "Copyright (C) 1993-2004 Louis Collins, McConnell Brain Imaging Centre, Montreal "
    "Neurological Institute, McGill University (the site's static/hero/LICENSE-MNI.txt)"
)


def fmriprep_oracle() -> Path:
    ws = workspace()
    if ws is None:
        raise FileNotFoundError("no workspace with oracles/ (the fMRIPrep oracle runs)")
    return ws / "oracles" / "fmriprep-25.2.6"


# ---------------------------------------------------------------------------- agreement


def ants_differing(rec: records.Record) -> set[str]:
    """Cases whose output differs from ANTs as built (bit-identical with CORE-MATH), from the
    record's table of those differences."""
    t = rec.table("Case", "Output", "Difference from ANTs as built")
    return {records.strip_code(c) for c in t.column("Case")}


def split_by_cases(
    differing: set[str], not_compared: tuple[str, ...] = ()
) -> Callable[[str, list[dict]], dict[str, int]]:
    """Passing cases in ``differing`` are identical only with correctly rounded maths, those in
    ``not_compared`` have no output values; the others are identical to the original."""

    def split(category: str, passing: list[dict]) -> dict[str, int]:
        out = {"identical": 0, "identical-cm": 0, "agree": 0}
        for r in passing:
            if r["case"] in not_compared:
                out["agree"] += 1
            elif r["case"] in differing:
                out["identical-cm"] += 1
            else:
                out["identical"] += 1
        return out

    return split


def split_by_category_table(
    rec: records.Record, *, compared: str = "Compared", identical: str = "Bit-identical"
) -> Callable[[str, list[dict]], dict[str, int]]:
    """From a per-category table with a bit-identical count (passing cases beyond it are within
    the record's thresholds)."""
    t = rec.table("Category", identical)
    counts = {r["Category"]: int(records.number(r[identical])) for r in t.rows}

    def split(category: str, passing: list[dict]) -> dict[str, int]:
        n = counts.get(category, 0)
        n = min(n, len(passing))
        return {"identical": n, "within": len(passing) - n}

    del compared
    return split


def agreement_figure(
    rec: records.Record,
    *,
    title: str,
    alt: str,
    split=None,
    original: str = "the original",
    state_labels: dict[str, str] | None = None,
    headline_split: dict[str, int] | None = None,
    labels: dict[str, str] | None = None,
    order: list[str] | None = None,
    note: str | None = None,
    name: str = "agreement",
) -> Figure:
    rows = charts.rows_from_record(
        rec, split, labels=labels, order=order, headline_split=headline_split
    )
    names = {
        "identical": f"identical to {original} as built",
        "identical-cm": f"identical once {original} uses correctly rounded maths",
        "agree": "agrees (no output values compared)",
        **(state_labels or {}),
    }
    return Figure(
        name,
        title,
        alt,
        lambda th, w: charts.agreement(th, w, rows, state_labels=names, note=note),
        records=rec.source(),
    )


# ---------------------------------------------------------------------------- band


def band_points(rec: records.Record) -> list[charts.BandPoint]:
    """The ratios to the thread band from an ANTs record's table of differences."""
    t = rec.table("Case", "Output", "max |diff| / band")
    pts = []
    for row in t.rows:
        case = records.strip_code(row["Case"])
        pts.append(charts.BandPoint(case, records.number(row["max |diff| / band"]), "max"))
        pts.append(charts.BandPoint(case, records.number(row["(1 - NCC) / band"]), "ncc"))
    return pts


def band_figure(
    rec: records.Record, *, original: str, alt: str, n_identical: int, note: str = ""
) -> tuple[Figure, list[charts.BandPoint]]:
    pts = band_points(rec)
    shown = [p for p in pts if math.isfinite(p.value)]
    fig = Figure(
        "band",
        f"Differences from {original} as built, against its own thread band",
        alt,
        lambda th, w: charts.band_strip(
            th,
            w,
            shown,
            groups=["max", "ncc"],
            group_labels={"max": "largest |difference|", "ncc": "1 − correlation"},
            zeros={"max": n_identical, "ncc": n_identical},
            xlabel=f"larmorx's difference from {original} (1 thread) ÷ "
            f"{style.possessive(original)} own thread band (log scale)",
            band_label=f"inside {style.possessive(original)} own thread-to-thread variability",
        ),
        records=rec.source(),
        notes=note,
    )
    return fig, pts


# ---------------------------------------------------------------------------- speed

ANTS_SERIES = (
    {
        ("ANTs 2.6.5", "1"): "ANTs, 1 thread",
        ("ANTs 2.6.5", "12"): "ANTs, 12 threads",
        ("larmorx", "1"): "larmorx, 1 thread",
        ("larmorx", "12"): "larmorx, 12 threads",
    },
    [
        ("ANTs, 1 thread", "original", "o"),
        ("ANTs, 12 threads", "original", "o-open"),
        ("larmorx, 1 thread", "larmorx", "o"),
        ("larmorx, 12 threads", "larmorx", "o-open"),
    ],
    ("ANTs, 1 thread", "larmorx, 1 thread"),
)


def speed_figure(
    bench: records.Bench,
    *,
    alt: str,
    series=ANTS_SERIES,
    labels: dict[str, str] | None = None,
    inputs: list[str] | None = None,
    title: str = "Speed against the original",
    name: str = "speed",
    xlabel: str = "seconds, median of repeated runs (log scale)",
) -> Figure:
    mapping, legend, ratio = series
    rows = charts.bench_rows(bench, series=mapping, inputs=inputs, labels=labels)
    return Figure(
        name,
        title,
        alt,
        lambda th, w: charts.speed(th, w, rows, series=legend, ratio=ratio, xlabel=xlabel),
        records=bench.source(),
    )
