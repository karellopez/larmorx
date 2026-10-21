# SPDX-License-Identifier: Apache-2.0
"""The figures every tool shares: the agreement overview of a validation record, the
differences against the original's own variability, and speed from a benchmark report."""

from __future__ import annotations

import math
from collections import Counter
from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass, field

from larmorx_validation.figures import style
from larmorx_validation.figures.canvas import Canvas, text_width, wrap
from larmorx_validation.figures.records import Bench, Record

# ----------------------------------------------------------------------------- agreement


@dataclass
class Row:
    label: str
    counts: dict[str, int]
    strong: bool = False  # the headline row

    @property
    def total(self) -> int:
        return sum(self.counts.values())


#: A record's status -> agreement state, for the cases that did not pass.
STATUS_STATE = {"both-error": "both", "expected-divergence": "expected", "fail": "fail"}


def rows_from_record(
    rec: Record,
    split: Callable[[str, list[dict]], dict[str, int]] | None = None,
    *,
    order: Sequence[str] | None = None,
    labels: dict[str, str] | None = None,
    pass_state: str = "agree",
    headline: str = "All cases",
    headline_split: dict[str, int] | None = None,
) -> list[Row]:
    """One row per category of ``rec`` (and a headline row for all cases). ``split(category,
    passing results)`` says how a category's passing cases divide among ``identical``,
    ``identical-cm`` and ``within``; without it they are ``pass_state``. ``headline_split``
    gives the split of all passing cases where the record states it only in total."""
    by_cat: dict[str, list[dict]] = {}
    for r in rec.results:
        by_cat.setdefault(r["category"], []).append(r)
    cats = list(order) if order else sorted(by_cat)
    rows = []
    for cat in cats:
        results = by_cat.get(cat, [])
        if not results:
            continue
        counts: Counter = Counter()
        passing = [r for r in results if r["status"] == "pass"]
        if passing:
            if split:
                counts.update(split(cat, passing))
            else:
                counts[pass_state] += len(passing)
        for r in results:
            if r["status"] in STATUS_STATE:
                counts[STATUS_STATE[r["status"]]] += 1
        rows.append(Row((labels or {}).get(cat, cat), dict(counts)))
    total: Counter = Counter()
    for row in rows:
        total.update(row.counts)
    if headline_split:
        n_pass = total.pop(pass_state, 0)
        assert sum(headline_split.values()) == n_pass, (headline_split, n_pass)
        total.update(headline_split)
    return [Row(headline, dict(total), strong=True), *rows]


def agreement(
    theme: style.Theme,
    width: int,
    rows: list[Row],
    *,
    unit: str = "cases",
    note: str | None = None,
    state_labels: dict[str, str] | None = None,
    label_max: float = 200,
) -> object:
    """Stacked bars, one per row, each case's state as a share of the row; the count at the
    right. The headline row first, then a hairline, then the categories."""
    states = [s for s in style.STATE_ORDER if any(r.counts.get(s) for r in rows)]
    names = {**style.STATE_LABELS, **(state_labels or {})}
    narrow = width < 500
    pad = 0
    label_w = (
        0 if narrow else min(label_max, max(text_width(r.label, style.MONO, 11) for r in rows) + 16)
    )
    count_w = 74
    right = width - 4  # text set flush right keeps a margin
    bar_x = pad + label_w
    bar_w = width - bar_x - count_w
    # legend: wrap the swatches into lines
    legend_lines: list[list[str]] = [[]]
    x = 0.0
    for s in states:
        w = 11 + 6 + text_width(names[s], style.SANS, 11.5) + 18
        if legend_lines[-1] and x + w > width:
            legend_lines.append([])
            x = 0.0
        legend_lines[-1].append(s)
        x += w
    y = 4.0
    legend_h = 20 * len(legend_lines)
    row_h_strong, row_h = (40, 34) if narrow else (34, 26)
    bar_h_strong, bar_h = 14, 10
    note_lines = wrap(note, width, style.SANS, 11.5) if note else []
    height = (
        y
        + legend_h
        + 16
        + row_h_strong
        + 14
        + row_h * (len(rows) - 1)
        + (10 + 17 * len(note_lines) if note_lines else 0)
        + 6
    )
    c = Canvas(theme, width, height)
    st = theme.states
    for line in legend_lines:
        x = 0.0
        for s in line:
            c.swatch(x, y + 2, st[s], 11)
            c.text(x + 17, y + 11.5, names[s], size=11.5, color=theme.ink2)
            x += 11 + 6 + text_width(names[s], style.SANS, 11.5) + 18
        y += 20
    y += 16
    for row in rows:
        rh, bh = (row_h_strong, bar_h_strong) if row.strong else (row_h, bar_h)
        if narrow:
            c.text(
                0,
                y + 11,
                row.label,
                family=style.MONO if not row.strong else style.SANS,
                size=11 if not row.strong else 12.5,
                color=theme.ink if row.strong else theme.ink2,
                weight=600 if row.strong else 400,
            )
            by = y + 16
        else:
            by = y + (rh - bh) / 2
            c.text(
                bar_x - 12,
                by + bh / 2,
                row.label,
                family=style.MONO if not row.strong else style.SANS,
                size=11 if not row.strong else 12.5,
                color=theme.ink if row.strong else theme.ink2,
                weight=600 if row.strong else 400,
                ha="right",
                va="center_baseline",
            )
        total = row.total
        x = bar_x
        segs = [(s, row.counts.get(s, 0)) for s in states if row.counts.get(s, 0)]
        gap = 2.0
        usable = bar_w - gap * (len(segs) - 1)
        for s, n in segs:
            w = usable * n / total
            c.rect(x, by, w, bh, st[s]["color"], hatch=st[s].get("hatch"), edge=st[s].get("edge"))
            if (
                row.strong
                and not st[s].get("hatch")
                and w >= text_width(str(n), style.MONO, 10) + 10
            ):
                c.text(
                    x + w / 2,
                    by + bh / 2,
                    str(n),
                    family=style.MONO,
                    size=10,
                    color=_on(st[s]["color"], theme),
                    ha="center",
                    va="center_baseline",
                )
            x += w + gap
        c.text(
            right,
            by + bh / 2,
            f"{total} {unit if row.strong else ''}".strip(),
            family=style.MONO,
            size=11,
            color=theme.ink2,
            ha="right",
            va="center_baseline",
        )
        y += rh
        if row.strong:
            c.hline(0, width, y + 6)
            y += 14
    if note_lines:
        y += 10
        for line in note_lines:
            y += 17
            c.text(0, y - 4, line, size=11.5, color=theme.ink3)
    return c.fig


def _on(fill: str, theme: style.Theme) -> str:
    """Ink or paper, whichever reads on ``fill``."""
    from matplotlib.colors import to_rgb

    r, g, b = to_rgb(fill)
    lum = 0.2126 * r + 0.7152 * g + 0.0722 * b
    return "#16130f" if lum > 0.36 else "#f6f2ea"


# ----------------------------------------------------------------------------- band ratio


@dataclass
class BandPoint:
    case: str
    value: float  # the difference divided by the original's own band
    group: str = ""  # a row of the strip (e.g. an output, or a metric)
    label: bool = False


def band_strip(
    theme: style.Theme,
    width: int,
    points: list[BandPoint],
    *,
    groups: Sequence[str],
    group_labels: dict[str, str] | None = None,
    xlabel: str = "difference ÷ the original's own variability (log scale)",
    floor: float = 1e-8,
    band_label: str = "inside the original's own variability",
    zero_label: str = "bit-identical",
    zeros: dict[str, int] | None = None,
) -> object:
    """One strip per group: each case's difference from the original, divided by the
    original's own variability (its thread band), on a log scale; 1 is the band's edge.
    Points at zero (bit-identical) are counted at the left rather than drawn."""
    import numpy as np

    narrow = width < 500
    label_w = 0 if narrow else 168
    top = 24
    row_h = 54 if narrow else 40
    height = top + row_h * len(groups) + 50
    c = Canvas(theme, width, height)
    plot_x = label_w
    plot_w = width - plot_x - 6
    plot_y = top
    plot_h = row_h * len(groups)
    ax = c.axes(plot_x, plot_y, plot_w, plot_h)
    finite = [p.value for p in points if p.value > 0 and math.isfinite(p.value)]
    lo = max(min([*finite, 1e-3]) / 3, floor)
    hi = max(max([*finite, 1.0]) * 3, 10)
    ax.set_xscale("log")
    ax.set_xlim(lo, hi)
    ax.set_ylim(len(groups) - 0.5, -0.5)
    ax.axvspan(
        lo, 1.0, color=theme.larmorx_wash, alpha=0.35 if theme.name == "light" else 0.45, lw=0
    )
    ax.axvline(1.0, color=theme.ink3, lw=0.9)
    c.text(
        plot_x + plot_w * (math.log(1.0 / lo) / math.log(hi / lo)) + 6,
        top - 6,
        "band edge",
        family=style.MONO,
        size=10,
        color=theme.ink3,
    )
    c.text(plot_x + 6, top - 6, band_label, size=11, color=theme.ink2)
    rng = np.random.default_rng(7)
    for gi, g in enumerate(groups):
        vals = [p for p in points if p.group == g]
        xs = [min(max(p.value, lo * 1.05), hi / 1.05) for p in vals if p.value > 0]
        ys = gi + (0.12 if narrow else 0) + rng.uniform(-0.17, 0.17, size=len(xs))
        ax.scatter(
            xs, ys, s=30, color=theme.larmorx, edgecolors=theme.paper, linewidths=1.2, zorder=3
        )
        for _ in [p for p in vals if not math.isfinite(p.value)]:
            ax.annotate("∞", (hi / 1.05, gi), color=theme.ink2, ha="right", va="center")
        name = (group_labels or {}).get(g, g)
        nz = (zeros or {}).get(g, 0)
        cy = plot_y + row_h * (gi + 0.5)
        if narrow:
            c.text(
                plot_x + 6,
                plot_y + row_h * gi + 13,
                name + (f" · {nz} {zero_label}" if nz else ""),
                family=style.MONO,
                size=10,
                color=theme.ink2,
            )
        else:
            c.text(
                label_w - 12, cy - 3, name, family=style.MONO, size=11, color=theme.ink2, ha="right"
            )
            if nz:
                c.text(
                    label_w - 12,
                    cy + 12,
                    f"+ {nz} {zero_label}",
                    family=style.MONO,
                    size=10,
                    color=theme.ink3,
                    ha="right",
                )
    style.style_axes(ax, theme, grid="x")
    ax.spines["left"].set_visible(False)
    ax.set_yticks([])
    for t in ax.get_xticklabels():
        t.set_fontfamily(style.MONO)
    ax.set_xlabel(xlabel, fontsize=style.pt(11.5), color=theme.ink2)
    return c.fig


# ----------------------------------------------------------------------------- speed


@dataclass
class SpeedRow:
    label: str
    marks: list[tuple[str, float, str]] = field(default_factory=list)  # (series, seconds, role)


def speed(
    theme: style.Theme,
    width: int,
    rows: list[SpeedRow],
    *,
    series: list[tuple[str, str, str]],  # (name, role colour key, marker)
    xlabel: str = "seconds, median of repeated runs (log scale)",
    ratio: tuple[str, str] | None = None,  # (slower series, faster series): the factor at right
) -> object:
    """A dot plot: one row per input, one mark per program and thread count, seconds on a log
    scale; the speed-up factor between two series at the right."""
    narrow = width < 500
    label_w = 0 if narrow else 220
    ratio_w = 62 if ratio else 0
    legend_h = 22 * (2 if narrow and len(series) > 2 else 1)
    top = legend_h + (26 if ratio else 10)
    row_h = 46 if narrow else 30
    height = top + row_h * len(rows) + 50
    c = Canvas(theme, width, height)
    colours = {
        "original": theme.original,
        "larmorx": theme.larmorx,
        "replica": theme.replica,
        "other": theme.both,  # a second reference (e.g. SimpleITK beside nibabel)
    }
    x = 0.0
    y = 4.0
    for name, role, marker in series:
        w = 14 + text_width(name, style.SANS, 11.5) + 20
        if x + w > width and x > 0:
            x, y = 0.0, y + 22
        _legend_marker(c, x + 5, y + 7, colours[role], marker, theme)
        c.text(x + 14, y + 11.5, name, size=11.5, color=theme.ink2)
        x += w
    plot_x = label_w
    plot_w = width - plot_x - ratio_w - 4
    ax = c.axes(plot_x, top, plot_w, row_h * len(rows))
    vals = [s for r in rows for _, s, _ in r.marks if s > 0]
    lo, hi = min(vals), max(vals)
    ax.set_xscale("log")
    ax.set_xlim(lo / 1.8, hi * 1.8)
    ax.set_ylim(len(rows) - 0.5, -0.5)
    marker_of = {name: (role, marker) for name, role, marker in series}
    for i, r in enumerate(rows):
        xs = [s for _, s, _ in r.marks]
        if xs:
            ax.plot([min(xs), max(xs)], [i, i], color=theme.line, lw=1.2, zorder=1)
        for name, s, _ in r.marks:
            role, marker = marker_of[name]
            _marker(ax, s, i, colours[role], marker, theme)
        if narrow:
            ax.text(
                lo / 1.7,
                i - 0.42,
                r.label,
                fontfamily=style.SANS,
                fontsize=style.pt(10.5),
                color=theme.ink2,
                va="bottom",
            )
        else:
            lines = wrap(r.label, label_w - 14, style.SANS, 11)
            for k, line in enumerate(lines[:2]):
                c.text(
                    label_w - 12,
                    top + row_h * (i + 0.5) + (k - (min(len(lines), 2) - 1) / 2) * 13,
                    line + ("…" if k == 1 and len(lines) > 2 else ""),
                    size=11,
                    color=theme.ink2,
                    ha="right",
                    va="center_baseline",
                )
        if ratio:
            d = {name: s for name, s, _ in r.marks}
            if ratio[0] in d and ratio[1] in d and d[ratio[1]] > 0:
                f = d[ratio[0]] / d[ratio[1]]
                c.text(
                    width - 4,
                    top + row_h * (i + 0.5),
                    f"{f:.1f}×" if f < 10 else f"{f:.0f}×",
                    family=style.SERIF,
                    size=17,
                    color=theme.ink,
                    ha="right",
                    va="center_baseline",
                )
    style.style_axes(ax, theme, grid="x")
    ax.spines["left"].set_visible(False)
    ax.set_yticks([])
    for t in ax.get_xticklabels():
        t.set_fontfamily(style.MONO)
    ax.set_xlabel(xlabel, fontsize=style.pt(11.5), color=theme.ink2)
    if ratio:
        c.text(
            width - 4, top - 6, "speed-up", family=style.MONO, size=10, color=theme.ink3, ha="right"
        )
    return c.fig


def _marker(ax, x, y, color, marker, theme):
    hollow = marker.endswith("-open")
    m = marker.replace("-open", "")
    ax.scatter(
        [x],
        [y],
        s=52 if m == "o" else 58,
        marker=m,
        facecolors=theme.paper if hollow else color,
        edgecolors=color if hollow else theme.paper,
        linewidths=1.6 if hollow else 1.2,
        zorder=3,
    )


def _legend_marker(c: Canvas, x, y, color, marker, theme):
    hollow = marker.endswith("-open")
    m = marker.replace("-open", "")
    c.ax.scatter(
        [x],
        [y],
        s=46,
        marker=m,
        facecolors=theme.paper if hollow else color,
        edgecolors=color if hollow else theme.paper,
        linewidths=1.5 if hollow else 1.0,
    )


def bench_rows(
    bench: Bench,
    *,
    series: dict[tuple[str, str], str],  # (tool, operation) -> series name
    inputs: Iterable[str] | None = None,
    labels: dict[str, str] | None = None,
) -> list[SpeedRow]:
    """Rows for :func:`speed` from a benchmark report: one per input, one mark per
    (tool, operation) named in ``series``."""
    order: list[str] = []
    marks: dict[str, list] = {}
    for r in bench.results:
        key = (r["tool"], r["operation"])
        if key not in series:
            continue
        if inputs is not None and r["input"] not in inputs:
            continue
        if r["input"] not in marks:
            order.append(r["input"])
            marks[r["input"]] = []
        marks[r["input"]].append((series[key], r["median"], ""))
    if inputs is not None:
        order = [i for i in inputs if i in marks]
    return [SpeedRow((labels or {}).get(i, i), marks[i]) for i in order]
