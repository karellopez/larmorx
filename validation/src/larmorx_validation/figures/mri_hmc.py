# SPDX-License-Identifier: Apache-2.0
"""Head-motion correction (``lx.mri.hmc``, clean-room, experimental): the record's agreement
with mcflirt, a real run's motion traces and framewise displacement next to mcflirt's, the
deviation from mcflirt next to mcflirt's own variability, and speed."""

from __future__ import annotations

from functools import cache
from pathlib import Path

import numpy as np

from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import common, records, style
from larmorx_validation.figures.canvas import Canvas, text_width
from larmorx_validation.figures.core import Context, Figure

TOOL = "mri-hmc"
CASE = "real/ds000005/fmriprep-reports"
BAND_CASE = "real/ds000005/fmriprep"
PERTURBATIONS = ("noise", "noise2", "noise3", "reverse")
SPEED_LABELS = {
    "ds000005 sub-01 run 1: 64×64×34, 240 volumes, int16": "ds000005: 64×64×34, 240 vol.",
    "ds006010 sub-206: 96×96×69, 113 volumes, uint16 (multiband)": "ds006010: 96×96×69, 113 vol.",
    "ds000258 sub-21262 echo 4: 64×64×30, 239 volumes, int16 (big-endian)": (
        "ds000258: 64×64×30, 239 vol."
    ),
}


def _oracle() -> Path:
    from larmorx_validation.parity.mri_hmc import oracle_dir

    root = oracle_dir()
    if root is None:
        raise FileNotFoundError("the recorded mcflirt runs (oracles/fsl-6.0.7/mcflirt) are missing")
    return root


def _compute(e: fcache.Entry) -> None:
    import json

    run = json.loads((_oracle() / "runs" / CASE / "run.json").read_text(encoding="utf-8"))
    argv = run["steps"][0]["argv"]
    src, ref = run["inputs"]["in"]["path"], run["inputs"]["ref"]["path"]
    e.input("BOLD", Path(src))
    e.input("reference", Path(ref))
    opts = [a for a in argv[1:] if a.startswith("-") and a not in ("-in", "-out", "-reffile")]
    for mode in ("reproduce", "fix"):
        out = e.dir / mode
        out.mkdir(exist_ok=True)
        e.run(
            mode,
            fcache.larmorx_cli(
                "mri",
                "hmc",
                "-in",
                src,
                "-out",
                str(out / "out_mcf.nii.gz"),
                "-reffile",
                ref,
                *opts,
            ),
            env={"LARMORX_UPSTREAM_DEFECTS": mode, "FSLOUTPUTTYPE": "NIFTI_GZ"},
        )
    e.finish(
        settings=f"fMRIPrep's mcflirt command as the recorded run {CASE} ran it ({' '.join(opts)}"
        "); larmorx in reproduce mode (as the record compares) and in its default mode (D13 "
        "corrections)",
        oracle=f"recorded mcflirt run oracles/fsl-6.0.7/mcflirt/runs/{CASE} (FSL 6.0.7.17)",
    )


@cache
def _traces(cache_dir: str) -> dict:
    from larmorx_validation.parity.mri_hmc import (
        _fov_centre,
        _read_mats,
        decompose,
        framewise_displacement,
        rms_deviation,
    )

    d = Path(cache_dir)
    root = _oracle()
    ref_mats, _ = _read_mats(root / "runs" / CASE / "out_mcf.nii.gz.mat")
    centre = _fov_centre(root / "runs" / CASE / "out_mcf.nii.gz")
    out: dict = {"n": len(ref_mats)}

    def params(mats):
        p = np.array([decompose(m, centre) for m in mats])
        return p

    out["mcflirt"] = params(ref_mats)
    for mode in ("reproduce", "fix"):
        mats, _ = _read_mats(d / mode / "out_mcf.nii.gz.mat")
        out[mode] = params(mats)
        out[f"{mode}-rms"] = np.array(
            [rms_deviation(a, b, centre) for a, b in zip(mats, ref_mats, strict=True)]
        )
    out["band"] = {}
    for kind in PERTURBATIONS:
        p = root / "band" / BAND_CASE / kind / "out_mcf.nii.gz.mat"
        if p.is_dir():
            mats, _ = _read_mats(p)
            if kind == "reverse":
                mats = mats[::-1]
            if len(mats) == len(ref_mats):
                out["band"][kind] = np.array(
                    [rms_deviation(a, b, centre) for a, b in zip(mats, ref_mats, strict=True)]
                )
    for k in ("mcflirt", "reproduce", "fix"):
        out[f"{k}-fd"] = framewise_displacement(out[k])
    return out


def motion_figure(ctx: Context) -> tuple[Figure, Figure]:
    state: dict = {}

    def prepare(fig: Figure) -> None:
        e = fcache.entry(
            ctx,
            TOOL,
            "ds000005-sub-01-run-1",
            "fMRIPrep's head-motion correction of ds000005 sub-01's first BOLD run",
            _compute,
        )
        state["t"] = _traces(str(e.dir))
        fig.cache = [fcache.provenance(e)]

    def legend(c: Canvas, theme: style.Theme, width: int, items, y=4.0) -> float:
        x = 0.0
        for label, col, ls in items:
            w = 22 + text_width(label, style.SANS, 11.5) + 20
            if x + w > width and x > 0:
                x, y = 0.0, y + 20
            c.ax.plot([x, x + 16], [y + 7, y + 7], color=col, lw=1.8, ls=ls)
            c.text(x + 22, y + 11.5, label, size=11.5, color=theme.ink2)
            x += w
        return y + 20

    def draw_traces(theme: style.Theme, width: int):
        t = state["t"]
        narrow = width < 500
        cols = 1 if narrow else 3
        rows = 6 // cols
        cell_h = 92
        top0 = 30 if not narrow else 50
        h = top0 + rows * (cell_h + 50) + 190
        c = Canvas(theme, width, h)
        y = legend(
            c,
            theme,
            width,
            [
                ("mcflirt (FSL 6.0.7)", theme.original, "-"),
                ("larmorx, as compared (reproduce)", theme.larmorx, "-"),
            ],
        )
        top = max(top0, y + 8)
        names = [
            "rotation x (°)",
            "rotation y (°)",
            "rotation z (°)",
            "translation x (mm)",
            "translation y (mm)",
            "translation z (mm)",
        ]
        gap = 26
        cw = (width - 66 - gap * (cols - 1)) / cols
        vols = np.arange(t["n"])
        for q in range(6):
            r, k = divmod(q, cols)
            x0 = 58 + k * (cw + gap)
            y0 = top + r * (cell_h + 50) + 22
            ax = c.axes(x0, y0, cw, cell_h)
            scale = 180 / np.pi if q < 3 else 1.0
            ax.plot(vols, t["mcflirt"][:, q] * scale, color=theme.original, lw=1.6)
            ax.plot(vols, t["reproduce"][:, q] * scale, color=theme.larmorx, lw=1.0)
            style.style_axes(ax, theme, grid="y")
            ax.tick_params(labelsize=style.pt(9.5))
            for tl in [*ax.get_xticklabels(), *ax.get_yticklabels()]:
                tl.set_fontfamily(style.MONO)
            ax.set_xlim(0, t["n"] - 1)
            c.text(x0, y0 - 8, names[q], family=style.MONO, size=10.5, color=theme.ink2)
        # framewise displacement
        y0 = top + rows * (cell_h + 50) + 26
        ax = c.axes(58, y0, width - 66, 100)
        ax.plot(vols, t["mcflirt-fd"], color=theme.original, lw=1.6)
        ax.plot(vols, t["reproduce-fd"], color=theme.larmorx, lw=1.0)
        style.style_axes(ax, theme, grid="y")
        for tl in [*ax.get_xticklabels(), *ax.get_yticklabels()]:
            tl.set_fontfamily(style.MONO)
        ax.set_xlim(0, t["n"] - 1)
        ax.set_xlabel("volume", fontsize=style.pt(11))
        r_fd = float(np.corrcoef(t["mcflirt-fd"][1:], t["reproduce-fd"][1:])[0, 1])
        c.text(
            58,
            y0 - 8,
            f"framewise displacement (mm, Power; r = {r_fd:.3f})",
            family=style.MONO,
            size=10.5,
            color=theme.ink2,
        )
        return c.fig

    def draw_band(theme: style.Theme, width: int):
        t = state["t"]
        h = 318
        c = Canvas(theme, width, h)
        fx = t["fix-rms"]
        changed = np.flatnonzero(fx != t["reproduce-rms"])
        items = [
            ("larmorx, as compared (reproduce)", theme.larmorx, "-"),
            ("larmorx's default (D13 corrections)", theme.larmorx, ":"),
            ("mcflirt on slightly perturbed input (its own band)", theme.both, "-"),
        ]
        y = legend(c, theme, width, items)
        if changed.size:
            note = (
                f"the default differs in {changed.size} volume{'s' if changed.size > 1 else ''}"
                f" (volume {int(changed[-1])}: {fx[changed[-1]]:.2g} mm)"
            )
            c.text(0, y + 12, note, family=style.MONO, size=10, color=theme.ink3)
            y += 18
        c.text(
            52,
            y + 18,
            "RMS deviation from mcflirt (mm, 80 mm sphere)",
            family=style.MONO,
            size=10.5,
            color=theme.ink2,
        )
        ax = c.axes(52, y + 30, width - 60, h - y - 76)
        vols = np.arange(t["n"])
        for kind, v in t["band"].items():
            ax.plot(vols, v, color=theme.both, lw=0.9, alpha=0.9, zorder=1)
            del kind
        ax.plot(vols, t["reproduce-rms"], color=theme.larmorx, lw=1.5, zorder=3)
        ax.plot(vols, t["fix-rms"], color=theme.larmorx, lw=1.3, ls=":", zorder=2)
        top = max([t["reproduce-rms"].max(), *[v.max() for v in t["band"].values()]])
        ax.set_ylim(0, top * 1.15)
        style.style_axes(ax, theme, grid="y")
        for tl in [*ax.get_xticklabels(), *ax.get_yticklabels()]:
            tl.set_fontfamily(style.MONO)
        ax.set_xlim(0, t["n"] - 1)
        ax.set_xlabel("volume", fontsize=style.pt(11))
        return c.fig

    data = [
        {
            **common.DS000005,
            "files": [
                "openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz",
                "fMRIPrep 21.0.1's published boldref (openneuro-derivatives ds000005-fmriprep)",
            ],
        }
    ]
    traces = Figure(
        "motion",
        "Motion estimates on a real run: mcflirt and larmorx",
        "Six small line charts of the rotations and translations estimated for each of the 240 "
        "volumes of OpenNeuro ds000005 sub-01's first BOLD run by mcflirt and by larmorx, and "
        "the framewise displacement of both.",
        draw_traces,
        data=data,
        prepare=prepare,
    )
    band = Figure(
        "deviation",
        "larmorx's deviation from mcflirt against mcflirt's own variability",
        "A line chart, per volume of the same run, of the RMS deviation (80 mm sphere) of "
        "larmorx's matrices from mcflirt's, in reproduce mode and in its default mode, and of "
        "mcflirt's own matrices when its input is slightly perturbed.",
        draw_band,
        data=data,
        prepare=prepare,
    )
    return traces, band


def figures(ctx: Context) -> list[Figure]:
    rec = records.load(ctx.repo, TOOL)
    figs = [_agreement(rec), *motion_figure(ctx)]
    bench = records.load_bench(ctx.repo, TOOL)
    if bench is not None:
        series = (
            {
                ("mcflirt (FSL 6.0.7)", "1 thread"): "mcflirt",
                ("larmorx (CLI)", "1 thread"): "larmorx, 1 thread",
                ("larmorx (CLI)", "12 threads"): "larmorx, 12 threads",
            },
            [
                ("mcflirt", "original", "o"),
                ("larmorx, 1 thread", "larmorx", "o"),
                ("larmorx, 12 threads", "larmorx", "o-open"),
            ],
            ("mcflirt", "larmorx, 1 thread"),
        )
        figs.append(
            common.speed_figure(
                bench,
                series=series,
                labels=SPEED_LABELS,
                title="Speed against mcflirt",
                alt="Median seconds per BOLD run for mcflirt and larmorx's command line at 1 and 12 "
                "threads, on a log scale, with larmorx's speed-up at one thread.",
            )
        )
    return figs


def _agreement(rec: records.Record) -> Figure:
    from larmorx_validation.figures import charts

    rows = charts.rows_from_record(rec, None, pass_state="within")
    names = {"within": "within the thresholds (mcflirt's band where wider)"}
    return Figure(
        "agreement",
        "Agreement with mcflirt, by category",
        f"Stacked bars, one per category of the head-motion record ({len(rec.results)} cases): "
        "within the record's thresholds (or mcflirt's own band where wider), one failure, "
        "rejected by both, and an expected divergence.",
        lambda th, w: charts.agreement(th, w, rows, state_labels=names),
        records=rec.source(),
    )
