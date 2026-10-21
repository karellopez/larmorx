# SPDX-License-Identifier: Apache-2.0
"""DenoiseImage: the record's agreement, the differences from ANTs as built against ANTs' own
thread band, ANTs' dependence on its thread count (on sMRIPrep's command), and speed."""

from __future__ import annotations

from functools import cache
from pathlib import Path

import numpy as np

from larmorx_validation.figures import brain, common, records, style
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures.canvas import Canvas
from larmorx_validation.figures.core import Context, Figure

TOOL = "ants-denoise"
DATA_T1 = "openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz"
MASK = "ds000005-sub-01-noreconall/out/sub-01/anat/sub-01_desc-brain_mask.nii.gz"
ANTS_THREADS = (1, 2, 4, 8, 12)
LX_THREADS = (1, 12)

SPEED_LABELS = {
    "sMRIPrep: -n Rician -s 1, raw T1w ds000005 (160×192×192)": "sMRIPrep's call, T1w ds000005",
    "sMRIPrep: -n Rician -s 1, T1w ds000210 (256×256×176)": "sMRIPrep's call, T1w ds000210",
    "-n Rician, MNI152NLin2009cAsym T1w res-02 (97×115×97)": "Rician, MNI template 2 mm",
    "-n Gaussian (ANTs' default model), raw T1w ds000005": "Gaussian, T1w ds000005",
    "-n Rician, BOLD reference ds000005 (64×64×34)": "Rician, BOLD reference",
    "-n Rician, fieldmap magnitude ds003012 (64×64×36)": "Rician, field-map magnitude",
}


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    b, lib = ants_bin(), crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload is missing")
    t1 = e.input("T1w", Path(td.get(DATA_T1)))
    args = ["-i", str(t1), "-n", "Rician", "-s", "1"]  # sMRIPrep's command line
    for n in ANTS_THREADS:
        e.run(
            f"ants-{n}",
            [b / "DenoiseImage", *args, "-o", str(e.dir / f"ants-{n}.nii.gz")],
            env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(n)},
            unset=("LD_PRELOAD", "NSLOTS"),
        )
    e.run(
        "ants-1-crmath",
        [b / "DenoiseImage", *args, "-o", str(e.dir / "ants-1-crmath.nii.gz")],
        env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1", "LD_PRELOAD": str(lib)},
        unset=("NSLOTS",),
    )
    for n in LX_THREADS:
        e.run(
            f"larmorx-{n}",
            fcache.larmorx_cli(
                "ants", "DenoiseImage", *args, "-o", str(e.dir / f"larmorx-{n}.nii.gz")
            ),
            env={
                "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(n),
                "LARMORX_UPSTREAM_DEFECTS": "reproduce",
            },
            unset=("LD_PRELOAD", "NSLOTS"),
        )
    e.finish(
        versions={"ants": ants_version()},
        threads={
            **{f"ants-{n}": n for n in ANTS_THREADS},
            "ants-1-crmath": 1,
            **{f"larmorx-{n}": n for n in LX_THREADS},
        },
        settings="sMRIPrep's command line (-n Rician -s 1; patch 1, search 2), each program once "
        "per thread count; larmorx with upstream_defects=reproduce, as in the record",
    )


@cache
def _images(cache_dir: str) -> dict:
    d = Path(cache_dir)
    mask = brain.load(common.fmriprep_oracle() / MASK).data > 0
    out: dict = {"mask": mask, "show": brain.dilate(mask, 2)}
    for n in ANTS_THREADS:
        out[f"ants-{n}"] = brain.load(d / f"ants-{n}.nii.gz").data
    out["ants-1-crmath"] = brain.load(d / "ants-1-crmath.nii.gz").data
    for n in LX_THREADS:
        out[f"larmorx-{n}"] = brain.load(d / f"larmorx-{n}.nii.gz").data
    v = brain.load(d / "ants-1.nii.gz")
    out["zooms"] = v.zooms
    # the fraction of voxels (the whole image) that differ from ANTs at one thread
    ref = out["ants-1"]
    out["frac"] = {
        **{f"ants-{n}": float(np.mean(out[f"ants-{n}"] != ref)) for n in ANTS_THREADS},
        **{f"larmorx-{n}": float(np.mean(out[f"larmorx-{n}"] != ref)) for n in LX_THREADS},
        "larmorx-crmath": float(np.mean(out["larmorx-1"] != out["ants-1-crmath"])),
        "larmorx-threads": float(np.mean(out["larmorx-1"] != out["larmorx-12"])),
    }
    out["max"] = {
        **{f"ants-{n}": float(np.abs(out[f"ants-{n}"] - ref).max()) for n in ANTS_THREADS},
        **{f"larmorx-{n}": float(np.abs(out[f"larmorx-{n}"] - ref).max()) for n in LX_THREADS},
    }
    return out


def threads_figure(ctx: Context) -> tuple[Figure, Figure]:
    state: dict = {}

    def prepare(fig: Figure) -> None:
        e = fcache.entry(
            ctx,
            TOOL,
            "ds000005-sub-01-threads",
            "DenoiseImage on ds000005 sub-01's T1w at several thread counts",
            _compute,
        )
        state["img"] = _images(str(e.dir))
        fig.cache = [fcache.provenance(e)]

    def draw_chart(theme: style.Theme, width: int):
        im = state["img"]
        fr = im["frac"]
        narrow = width < 500
        h = 300 if narrow else 280
        c = Canvas(theme, width, h)
        legend = [
            ("ANTs n threads vs ANTs 1 thread", theme.original, "o"),
            ("larmorx n threads vs ANTs 1 thread", theme.larmorx, "o"),
        ]
        x = 0.0
        y = 4.0
        from larmorx_validation.figures.canvas import text_width

        for label, col, _m in legend:
            w = 16 + text_width(label, style.SANS, 11.5) + 22
            if x + w > width and x > 0:
                x, y = 0.0, y + 20
            c.ax.scatter([x + 5], [y + 7], s=40, color=col, edgecolors=theme.paper, linewidths=1)
            c.text(x + 14, y + 11.5, label, size=11.5, color=theme.ink2)
            x += w
        top = y + 34
        ax = c.axes(48, top, width - 48 - 12, h - top - 44)
        xs = list(ANTS_THREADS)
        ys = [100 * fr[f"ants-{n}"] for n in xs]
        ax.plot(xs, ys, color=theme.original, lw=1.5, zorder=2)
        ax.scatter(
            xs, ys, s=40, color=theme.original, edgecolors=theme.paper, linewidths=1.2, zorder=3
        )
        lx = [100 * fr[f"larmorx-{n}"] for n in LX_THREADS]
        ax.plot(list(LX_THREADS), lx, color=theme.larmorx, lw=1.5, zorder=2)
        ax.scatter(
            list(LX_THREADS),
            lx,
            s=40,
            color=theme.larmorx,
            edgecolors=theme.paper,
            linewidths=1.2,
            zorder=3,
        )
        for n, v in zip(xs, ys, strict=True):
            if n > 1:
                ax.annotate(
                    f"{v:.1f} %",
                    (n, v),
                    xytext=(0, 9),
                    textcoords="offset points",
                    ha="center",
                    fontfamily=style.MONO,
                    fontsize=style.pt(10.5),
                    color=theme.ink2,
                )
        ax.annotate(
            f"{100 * fr['larmorx-12']:.2f} % (glibc's expf; 0 with CORE-MATH)",
            (12, lx[-1]),
            xytext=(-4, 8),
            textcoords="offset points",
            ha="right",
            fontfamily=style.MONO,
            fontsize=style.pt(10.5),
            color=theme.ink2,
        )
        ax.set_xscale("log", base=2)
        ax.set_xticks(xs)
        ax.set_xticklabels([str(n) for n in xs])
        ax.set_xlim(0.85, 14)
        ax.set_ylim(0, 100)
        ax.set_yticks([0, 25, 50, 75, 100])
        ax.set_yticklabels(["0", "25", "50", "75", "100 %"])
        style.style_axes(ax, theme, grid="y")
        for t in [*ax.get_xticklabels(), *ax.get_yticklabels()]:
            t.set_fontfamily(style.MONO)
        ax.set_xlabel("threads (ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS)", fontsize=style.pt(11.5))
        ax.set_ylabel("voxels that differ", fontsize=style.pt(11.5))
        return c.fig

    def draw_maps(theme: style.Theme, width: int):
        im = state["img"]
        show = im["show"]
        _, plane = brain.centre_planes(im["mask"])  # coronal: ITK's slabs run along z
        sp = plane.spacing(im["zooms"])
        m2 = plane.take(show)
        crop = brain.bbox(m2, 2)
        seq = style.sequential(theme)
        lo, hi = -4.0, 2.5  # log10 |difference|: 1e-4 to about 300

        def logdiff(a, b):
            d = np.abs(plane.take(a) - plane.take(b))
            with np.errstate(divide="ignore"):
                out = np.log10(d)
            out[d == 0] = np.nan  # identical voxels: the paper
            return out

        def sub(a, b):
            v = (a != b)[show]
            m = float(np.abs(a - b)[show].max())
            return (
                "identical"
                if m == 0
                else (f"{100 * v.mean():.2g} % of brain voxels · largest {m:.2g}")
            )

        same = not np.any(im["larmorx-12"] != im["ants-1-crmath"])
        panels = [
            brain.Panel(
                "ANTs: 2 vs 1 thread",
                logdiff(im["ants-2"], im["ants-1"]),
                m2,
                seq,
                lo,
                hi,
                sp,
                crop=crop,
                outline=m2,
                subtitle=sub(im["ants-2"], im["ants-1"]),
            ),
            brain.Panel(
                "ANTs: 12 vs 1 thread",
                logdiff(im["ants-12"], im["ants-1"]),
                m2,
                seq,
                lo,
                hi,
                sp,
                crop=crop,
                outline=m2,
                subtitle=sub(im["ants-12"], im["ants-1"]),
            ),
            brain.Panel(
                "larmorx (12) vs ANTs (1)",
                logdiff(im["larmorx-12"], im["ants-1"]),
                m2,
                seq,
                lo,
                hi,
                sp,
                crop=crop,
                outline=m2,
                subtitle=sub(im["larmorx-12"], im["ants-1"]),
            ),
            brain.Panel(
                "larmorx vs ANTs, CORE-MATH",
                logdiff(im["larmorx-12"], im["ants-1-crmath"]),
                m2,
                seq,
                lo,
                hi,
                sp,
                crop=crop,
                outline=m2,
                note="bit-identical" if same else "",
                subtitle=sub(im["larmorx-12"], im["ants-1-crmath"]),
            ),
        ]
        bars = {
            "diff": brain.Bar(
                seq,
                lo,
                hi,
                "|difference| in the denoised image (log scale; identical voxels left blank)",
                ticks=[-4, -2, 0, 2],
                fmt="1e{:.0f}",
            )
        }
        return brain.draw_grid(theme, width, brain.Grid(panels, cols=2, cols_narrow=2, bars=bars))

    data = [{**common.DS000005, "files": [DATA_T1, f"fMRIPrep 25.2.6's brain mask ({MASK})"]}]
    chart = Figure(
        "threads",
        "DenoiseImage's result depends on ANTs' thread count; larmorx's does not",
        "A line chart of the share of the T1w's voxels that differ from ANTs' one-thread result: "
        "ANTs at 2, 4, 8 and 12 threads, and larmorx at 1 and 12 threads.",
        draw_chart,
        data=data,
        prepare=prepare,
    )
    maps = Figure(
        "thread-maps",
        "Where ANTs' threads change the result",
        "Coronal brain slices of the differences from ANTs at one thread on one symmetric scale: "
        "ANTs at 2 and at 12 threads, larmorx at 12 threads, and larmorx against ANTs with "
        "correctly rounded maths.",
        draw_maps,
        data=data,
        prepare=prepare,
    )
    return chart, maps


def figures(ctx: Context) -> list[Figure]:
    rec = records.load(ctx.repo, TOOL)
    differing = common.ants_differing(rec)
    n_identical = sum(
        1
        for r in rec.results
        if r["status"] == "pass" and r["case"] not in differing and r["case"] != "errors/help"
    )
    band, pts = common.band_figure(
        rec,
        original="ANTs",
        n_identical=n_identical,
        alt="For each case that differs from ANTs as built, the largest difference and one "
        "minus the correlation, divided by ANTs' own difference between one thread and 2, 4 "
        "and 12 threads, on a log scale; every value is far below 1.",
        note="Cases where ANTs' threads change nothing (a band of 0, ratio infinite) are not "
        "drawn: "
        + ", ".join(sorted({p.case for p in common.band_points(rec) if not np.isfinite(p.value)})),
    )
    del pts
    figs = [
        common.agreement_figure(
            rec,
            title="Agreement with DenoiseImage, by category",
            alt=f"Stacked bars, one per category of the DenoiseImage record "
            f"({len(rec.results)} cases): identical to ANTs as built, identical once ANTs uses "
            "correctly rounded maths, and rejected by both.",
            split=common.split_by_cases(differing, ("errors/help",)),
            original="ANTs",
        ),
        band,
        *threads_figure(ctx),
    ]
    bench = records.load_bench(ctx.repo, TOOL)
    if bench is not None:
        figs.append(
            common.speed_figure(
                bench,
                labels=SPEED_LABELS,
                title="Speed against ANTs",
                alt="Median seconds per input for ANTs and larmorx at 1 and 12 threads, on a "
                "log scale, with larmorx's speed-up at one thread.",
            )
        )
    return figs
