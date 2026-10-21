# SPDX-License-Identifier: Apache-2.0
"""3dTshift: the agreement of the clean-room original and of the GPL replica with AFNI, a real
BOLD run's time series through fMRIPrep's slice-timing command, and speed.

The replica (``larmorx-gpl``, GPL-3.0-or-later) runs here as a separate program, as everywhere
in the validation package (Apache-2.0)."""

from __future__ import annotations

import json
from functools import cache
from pathlib import Path

import numpy as np

from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import common, records, style
from larmorx_validation.figures.canvas import Canvas, text_width
from larmorx_validation.figures.core import Context, Figure

TOOL = "afni-tshift"
BOLD = "openneuro/ds005040/sub-002/func/sub-002_task-clip_run-1_bold.nii.gz"
RUN = "ds005040-sub-002-noreconall"
MASK = f"{RUN}/out/sub-002/func/sub-002_task-clip_run-1_desc-brain_mask.nii.gz"
TR, TZERO = 2.0, 0.975
DS005040 = {
    "dataset": "OpenNeuro ds005040, sub-002, task clip run 1",
    "subject": "sub-002",
    "licence": "CC0-1.0",
}
SPEED_LABELS = {
    "ds000210 sub-02 cuedSGT echo 1: 64×64×33, 260 volumes, int16": "ds000210: 64×64×33, 260 vol.",
    "ds006010 sub-206 category: 96×96×69, 113 volumes, uint16 (read as float32)": (
        "ds006010: 96×96×69, 113 vol."
    ),
    "ds005454 sub-16 rest: 160×160×96, 280 volumes, int16 (multiband 4)": (
        "ds005454: 160×160×96, 280 vol."
    ),
}


def agreement(rec: records.Record, which: str) -> Figure:
    t = rec.table("Category", "Compared", "Bit-identical")
    n_cmp = sum(int(records.number(r["Compared"])) for r in t.rows)
    n_id = sum(int(records.number(r["Bit-identical"])) for r in t.rows)
    return common.agreement_figure(
        rec,
        name="agreement" if which == "original" else "agreement-replica",
        title=f"Agreement of the {which} with 3dTshift, by category",
        alt=f"Stacked bars, one per category of the 3dTshift record of the {which} "
        f"({len(rec.results)} cases): {n_id} of {n_cmp} compared cases bit-identical to AFNI's "
        "output, the rest within the record's thresholds, rejected by both, and expected "
        "divergences.",
        split=common.split_by_category_table(rec),
        state_labels={
            "identical": "bit-identical to AFNI 25.2.09",
            "within": "within the record's thresholds (last float bits; integers by 1)",
        },
    )


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.afni_tshift import (
        afni_binary,
        afni_version,
        replica_binary,
        replica_version,
    )

    afni, gpl = afni_binary(), replica_binary()
    if afni is None or gpl is None:
        raise FileNotFoundError("the AFNI oracle or the larmorx-gpl replica is missing")
    bold = e.input("BOLD", Path(td.get(BOLD)))
    side = json.loads(Path(td.get(BOLD.replace(".nii.gz", ".json"))).read_text(encoding="utf-8"))
    times = [float(t) for t in side["SliceTiming"]]
    (e.dir / "slice_timing.1D").write_text("\t".join(map(str, times)), encoding="utf-8")
    args = [
        "-ignore",
        "0",
        "-tpattern",
        f"@{e.dir / 'slice_timing.1D'}",
        "-TR",
        "2s",
        "-tzero",
        str(TZERO),
    ]
    e.run("afni", [afni, *args, "-prefix", str(e.dir / "afni.nii.gz"), str(bold)])
    common_env = {"LARMORX_UPSTREAM_DEFECTS": "reproduce"}
    e.run(
        "larmorx-original",
        fcache.larmorx_cli(
            "afni", "3dTshift", *args, "-prefix", str(e.dir / "larmorx-original.nii.gz"), str(bold)
        ),
        env={**common_env, "LARMORX_IMPLEMENTATION": "original"},
    )
    e.run(
        "larmorx-replica",
        [
            gpl,
            "afni",
            "3dTshift",
            *args,
            "-prefix",
            str(e.dir / "larmorx-replica.nii.gz"),
            str(bold),
        ],
        env=common_env,
    )
    e.finish(
        versions={"afni": afni_version(), "larmorx-gpl": replica_version()},
        settings="fMRIPrep 25.2.6's 3dTshift arguments for this run (-ignore 0 -tpattern "
        "@slice_timing.1D -TR 2s -tzero 0.975; the slice times from the BIDS sidecar) on the raw "
        "BOLD; larmorx in reproduce mode",
        slice_timing=times,
    )


@cache
def _series(cache_dir: str) -> dict:
    import larmorx_testdata as td

    d = Path(cache_dir)

    import nibabel as nib

    def raw(p: Path) -> np.ndarray:
        return np.asanyarray(nib.load(str(p)).dataobj).astype(np.float64)

    out = {
        "input": raw(Path(td.get(BOLD))),
        "afni": raw(d / "afni.nii.gz"),
        "original": raw(d / "larmorx-original.nii.gz"),
        "replica": raw(d / "larmorx-replica.nii.gz"),
        "times": json.loads((d / "RUN_INFO.json").read_text(encoding="utf-8"))["slice_timing"],
    }
    out["mask"] = np.asanyarray(nib.load(str(common.fmriprep_oracle() / MASK)).dataobj) > 0
    afni = nib.load(str(d / "afni.nii.gz"))
    out["dtype"] = str(afni.get_data_dtype())
    # the stored integers' scale factor: a difference of one stored unit
    out["unit"] = float(getattr(afni.dataobj, "slope", 1.0) or 1.0)
    return out


def series_figure(ctx: Context) -> Figure:
    state: dict = {}

    def prepare(fig: Figure) -> None:
        e = fcache.entry(
            ctx,
            TOOL,
            "ds005040-sub-002-clip-1",
            "fMRIPrep's slice-timing correction of ds005040 sub-002's BOLD",
            _compute,
        )
        state["s"] = _series(str(e.dir))
        fig.cache = [fcache.provenance(e)]

    def draw(theme: style.Theme, width: int):
        s = state["s"]
        mask = s["mask"]
        nz = s["input"].shape[2]
        # a voxel of the latest slice acquired with a good share of brain (shifted back the
        # most): the brain voxel there whose series varies most
        counts = mask.sum(axis=(0, 1))
        k = max((t, kk) for kk, t in enumerate(s["times"]) if counts[kk] >= 0.5 * counts.max())[1]
        sl = s["input"][:, :, k, :]
        m = mask[:, :, k]
        var = np.where(m, sl.std(axis=-1), -1)
        i, j = np.unravel_index(int(np.argmax(var)), var.shape)
        n = s["input"].shape[-1]
        vols = np.arange(n)
        t_in = vols * TR + s["times"][k]
        t_out = vols * TR + TZERO
        narrow = width < 500
        h = 470 if not narrow else 500
        c = Canvas(theme, width, h)
        legend = [
            ("input, at its acquisition times", theme.both, "line"),
            ("AFNI 3dTshift", theme.original, "o"),
            ("larmorx, clean-room original", theme.larmorx, "o-open"),
            ("larmorx-gpl, replica", theme.replica, "x"),
        ]
        x, y = 0.0, 4.0
        for label, col, mk in legend:
            w = 18 + text_width(label, style.SANS, 11.5) + 20
            if x + w > width and x > 0:
                x, y = 0.0, y + 20
            if mk == "line":
                c.ax.plot([x, x + 12], [y + 7, y + 7], color=col, lw=1.2)
            else:
                _mark(c.ax, x + 6, y + 7, col, mk, theme, s=34)
            c.text(x + 18, y + 11.5, label, size=11.5, color=theme.ink2)
            x += w
        top = y + 34
        ax = c.axes(64, top, width - 70, 190)
        a, b = 8, min(n, 8 + (22 if narrow else 34))
        ax.plot(t_in[a:b], s["input"][i, j, k, a:b], color=theme.both, lw=1.0, zorder=1)
        ax.scatter(t_in[a:b], s["input"][i, j, k, a:b], s=8, color=theme.both, zorder=2)
        for key, col, mk in (
            ("afni", theme.original, "o"),
            ("original", theme.larmorx, "o-open"),
            ("replica", theme.replica, "x"),
        ):
            _mark(ax, t_out[a:b], s[key][i, j, k, a:b], col, mk, theme, s=30 if mk != "x" else 22)
        ax.plot(
            t_out[a:b], s["afni"][i, j, k, a:b], color=theme.original, lw=0.9, zorder=1, alpha=0.6
        )
        style.style_axes(ax, theme, grid="y")
        for t in [*ax.get_xticklabels(), *ax.get_yticklabels()]:
            t.set_fontfamily(style.MONO)
        ax.set_xlabel("time (s)", fontsize=style.pt(11.5))
        ax.set_ylabel("signal", fontsize=style.pt(11.5))
        c.text(
            64,
            top - 8,
            f"one voxel of slice {k + 1} of {nz} (acquired {s['times'][k]:.2f} s "
            f"into each TR), shifted to {TZERO} s",
            size=11,
            color=theme.ink2,
        )
        # bottom: voxels differing from AFNI, per volume
        top2 = top + 190 + 80
        ax2 = c.axes(64, top2, width - 70, h - top2 - 40)
        nb = int(mask.sum())
        d_o = np.array(
            [np.count_nonzero((s["original"][..., v] != s["afni"][..., v])[mask]) for v in range(n)]
        )
        d_r = np.array(
            [np.count_nonzero((s["replica"][..., v] != s["afni"][..., v])[mask]) for v in range(n)]
        )
        ax2.bar(vols, d_o, width=0.8, color=theme.larmorx, lw=0, label="clean-room original")
        ax2.plot(vols, d_r, color=theme.replica, lw=1.6, label="replica")
        lo = float(np.abs(s["original"] - s["afni"])[mask].max()) / s["unit"] if d_o.any() else 0
        style.style_axes(ax2, theme, grid="y")
        for t in [*ax2.get_xticklabels(), *ax2.get_yticklabels()]:
            t.set_fontfamily(style.MONO)
        ax2.set_xlim(-1, n)
        ax2.set_ylim(0, max(1, d_o.max()) * 1.25)
        from matplotlib.ticker import MaxNLocator

        ax2.yaxis.set_major_locator(MaxNLocator(integer=True))
        ax2.set_xlabel("volume", fontsize=style.pt(11.5))
        c.text(
            64,
            top2 - 24,
            f"brain voxels differing from AFNI's output, per volume, of {nb:,}",
            size=11,
            color=theme.ink2,
        )
        c.text(
            64,
            top2 - 8,
            f"original: {int(d_o.sum()):,} values, by {lo:g} stored {s['dtype']} unit"
            f"{'s' if lo != 1 else ''} · replica: {int(d_r.sum())}",
            family=style.MONO,
            size=10,
            color=theme.ink3,
        )
        return c.fig

    return Figure(
        "time-series",
        "Slice-timing correction of a real BOLD run: AFNI, the original and the replica",
        "Top: one voxel's signal over time in the last slice acquired of OpenNeuro ds005040 "
        "sub-002's BOLD run, at its acquisition times and shifted by AFNI 3dTshift, larmorx's "
        "clean-room original and the larmorx-gpl replica. Bottom: per volume, the number of "
        "brain voxels whose output differs from AFNI's.",
        draw,
        data=[{**DS005040, "files": [BOLD, f"fMRIPrep 25.2.6's BOLD brain mask ({MASK})"]}],
        prepare=prepare,
    )


def _mark(ax, x, y, color, mk, theme, s=30):
    if mk == "x":
        ax.scatter(x, y, s=s, marker="x", color=color, linewidths=1.3, zorder=4)
    elif mk == "o-open":
        ax.scatter(x, y, s=s * 1.6, facecolors="none", edgecolors=color, linewidths=1.4, zorder=3)
    else:
        ax.scatter(x, y, s=s, color=color, edgecolors=theme.paper, linewidths=0.8, zorder=3)


def figures(ctx: Context) -> list[Figure]:
    rec = records.load(ctx.repo, TOOL)
    rep = records.load(ctx.repo, "afni-tshift-replica")
    figs = [agreement(rec, "original"), agreement(rep, "replica"), series_figure(ctx)]
    bench = records.load_bench(ctx.repo, TOOL)
    if bench is not None:
        series = (
            {
                ("AFNI 25.2.09", "1 thread"): "AFNI",
                ("larmorx (CLI)", "1 thread"): "original, 1 thread",
                ("larmorx (CLI)", "12 threads"): "original, 12 threads",
                ("larmorx-gpl (replica, CLI)", "1 thread"): "replica, 1 thread",
                ("larmorx-gpl (replica, CLI)", "12 threads"): "replica, 12 threads",
            },
            [
                ("AFNI", "original", "o"),
                ("original, 1 thread", "larmorx", "o"),
                ("original, 12 threads", "larmorx", "o-open"),
                ("replica, 1 thread", "replica", "o"),
                ("replica, 12 threads", "replica", "o-open"),
            ],
            ("AFNI", "original, 1 thread"),
        )
        figs.append(
            common.speed_figure(
                bench,
                series=series,
                labels=SPEED_LABELS,
                title="Speed against AFNI",
                alt="Median seconds per BOLD run for AFNI's 3dTshift, larmorx's clean-room original "
                "and the larmorx-gpl replica (command lines), at 1 and 12 threads, on a log scale, "
                "with the original's speed-up over AFNI at one thread.",
            )
        )
    return figs
