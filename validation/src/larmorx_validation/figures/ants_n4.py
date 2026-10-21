# SPDX-License-Identifier: Apache-2.0
"""N4BiasFieldCorrection: the record's agreement, the differences from ANTs as built against
ANTs' own thread band, speed, and a bias field on a real T1w."""

from __future__ import annotations

from functools import cache
from pathlib import Path

import numpy as np

from larmorx_validation.figures import brain, common, records, style
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures.core import Context, Figure

TOOL = "ants-n4"
DATA_T1 = "openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz"
RUN = "ds000005-sub-01-noreconall"
MASK = f"{RUN}/out/sub-01/anat/sub-01_desc-brain_mask.nii.gz"

SPEED_LABELS = {
    "fMRIPrep's BOLD reference: -b [200] -r -w premask, boldref ds000005 (64×64×34)": (
        "BOLD reference, ds000005"
    ),
    "fMRIPrep's BOLD reference, MNI152NLin2009cAsym boldref res-02 (97×115×97)": (
        "BOLD reference, MNI template"
    ),
    "sMRIPrep before the template: ANTs' defaults, raw T1w ds000005 (160×192×192)": (
        "T1w, before the template"
    ),
    "sMRIPrep brain extraction, first N4: -b [200] -c [50x50x50x50,1e-7] -s 4, truncated T1w": (
        "T1w, brain extraction, first N4"
    ),
    "sMRIPrep brain extraction, final N4: -b [200] -c [50x50x50x50x50,1e-7] -s 4 -r -w WM, "
    "raw T1w": "T1w, brain extraction, final N4",
    "sdcflows' magnitude: -c [50x50x50x50x50,1e-7] -s 4, ds003012 magnitude1": (
        "field-map magnitude, ds003012"
    ),
}


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    b = ants_bin()
    lib = crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload is missing")
    t1 = e.input("T1w", Path(td.get(DATA_T1)))
    args = ["-d", "3", "-b", "[200]", "-c", "[50x50x50x50x50,1e-07]", "-s", "4", "-i", str(t1)]
    d = e.dir
    for label, threads, pre in (
        ("ants-1", 1, False),
        ("ants-4", 4, False),
        ("ants-1-crmath", 1, True),
    ):
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
        if pre:
            env["LD_PRELOAD"] = str(lib)
        out = f"[{d / label}.nii.gz,{d / label}-bias.nii.gz]"
        e.run(
            label,
            [b / "N4BiasFieldCorrection", *args, "-o", out],
            env=env,
            unset=("LD_PRELOAD",),
        )
    e.run(
        "larmorx",
        fcache.larmorx_cli(
            "ants",
            "N4BiasFieldCorrection",
            *args,
            "-o",
            f"[{d / 'larmorx'}.nii.gz,{d / 'larmorx'}-bias.nii.gz]",
        ),
        env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "4", "LARMORX_UPSTREAM_DEFECTS": "reproduce"},
        unset=("LD_PRELOAD",),
    )
    e.finish(
        versions={"ants": ants_version()},
        threads={"ants-1": 1, "ants-4": 4, "ants-1-crmath": 1, "larmorx": 4},
        settings="sMRIPrep's final N4 options without a weight image (-b [200] "
        "-c [50x50x50x50x50,1e-07] -s 4); larmorx with upstream_defects=reproduce, as in the "
        "record",
    )


@cache
def _images(cache_dir: str) -> dict:
    import larmorx_testdata as td

    d = Path(cache_dir)
    mask = brain.load(common.fmriprep_oracle() / MASK).data > 0
    out: dict = {"mask": mask, "show": brain.dilate(mask, 2)}
    for k in ("ants-1", "ants-4", "ants-1-crmath", "larmorx"):
        out[k] = brain.load(d / f"{k}.nii.gz")
        out[k + "-bias"] = brain.load(d / f"{k}-bias.nii.gz")
    out["t1"] = brain.load(Path(td.get(DATA_T1)))
    return out


def bias_figure(ctx: Context) -> Figure:
    state: dict = {}

    def prepare(fig: Figure) -> None:
        e = fcache.entry(ctx, TOOL, "ds000005-sub-01-t1w", "N4 on ds000005 sub-01's T1w", _compute)
        state["img"] = _images(str(e.dir))
        fig.cache = [fcache.provenance(e)]

    def draw(theme: style.Theme, width: int):
        im = state["img"]
        show = im["show"]
        plane, _ = brain.centre_planes(im["mask"])
        sp = plane.spacing(im["t1"].zooms)
        m2 = plane.take(show)
        crop = brain.bbox(m2, 2)
        t1 = plane.take(im["t1"].data)
        lx = plane.take(im["larmorx"].data)
        bias = plane.take(im["larmorx-bias"].data)
        d_lx = lx - plane.take(im["ants-1"].data)
        d_thr = plane.take(im["ants-4"].data) - plane.take(im["ants-1"].data)
        d_cm = lx - plane.take(im["ants-1-crmath"].data)
        lim = float(
            max(
                np.abs(im["larmorx"].data - im["ants-1"].data)[show].max(),
                np.abs(im["ants-4"].data - im["ants-1"].data)[show].max(),
            )
        )
        hi = np.percentile(t1[m2], 99.5)
        hi_c = np.percentile(lx[m2], 99.5)
        b_lo, b_hi = np.percentile(bias[m2], [0.5, 99.5])
        div = style.diverging(theme)
        seq = style.sequential(theme)
        same = not np.any((im["larmorx"].data - im["ants-1-crmath"].data)[show])
        v_lx = im["larmorx"].data - im["ants-1"].data
        v_thr = im["ants-4"].data - im["ants-1"].data
        v_cm = im["larmorx"].data - im["ants-1-crmath"].data
        panels = [
            brain.Panel("T1w, as acquired", t1, m2, "gray", 0, hi, sp, crop=crop),
            brain.Panel("Bias field, larmorx", bias, m2, seq, b_lo, b_hi, sp, crop=crop),
            brain.Panel("Corrected, larmorx", lx, m2, "gray", 0, hi_c, sp, crop=crop),
            brain.Panel(
                "larmorx − ANTs as built",
                d_lx,
                m2,
                div,
                -lim,
                lim,
                sp,
                crop=crop,
                outline=m2,
                subtitle=brain.largest(v_lx, show),
            ),
            brain.Panel(
                "ANTs: 4 − 1 thread",
                d_thr,
                m2,
                div,
                -lim,
                lim,
                sp,
                crop=crop,
                outline=m2,
                subtitle=brain.largest(v_thr, show),
            ),
            brain.Panel(
                "larmorx − ANTs, CORE-MATH",
                d_cm,
                m2,
                div,
                -lim,
                lim,
                sp,
                crop=crop,
                outline=m2,
                note="bit-identical" if same else "",
                subtitle=brain.largest(v_cm, show),
            ),
        ]
        bars = {
            "bias": brain.Bar(
                seq, float(b_lo), float(b_hi), "bias field (multiplicative)", fmt="{:.2f}"
            ),
            "diff": brain.Bar(
                div,
                -lim,
                lim,
                "difference in the corrected image (full range in the brain)",
                fmt="{:.2g}",
            ),
        }
        return brain.draw_grid(theme, width, brain.Grid(panels, cols=3, cols_narrow=2, bars=bars))

    return Figure(
        "bias-field",
        "A bias field on a real T1w, and the differences from ANTs",
        "Axial brain slices of OpenNeuro ds000005 sub-01's T1w: the input, larmorx's bias field "
        "and corrected image, and three difference maps on one symmetric scale: larmorx minus "
        "ANTs as built (one thread), ANTs at four threads minus ANTs at one thread, and larmorx "
        "minus ANTs with correctly rounded maths.",
        draw,
        data=[{**common.DS000005, "files": [DATA_T1, f"fMRIPrep 25.2.6's brain mask ({MASK})"]}],
        prepare=prepare,
    )


def figures(ctx: Context) -> list[Figure]:
    rec = records.load(ctx.repo, TOOL)
    differing = common.ants_differing(rec)
    n_identical = sum(
        1
        for r in rec.results
        if r["status"] == "pass" and r["case"] not in differing and r["case"] != "errors/help"
    )
    band, _ = common.band_figure(
        rec,
        original="ANTs",
        n_identical=n_identical,
        alt="For each case that differs from ANTs as built, the largest difference and one "
        "minus the correlation, divided by ANTs' own difference between one thread and 2 to 12 "
        "threads, on a log scale; values below 1 lie inside ANTs' own variability.",
    )
    figs = [
        common.agreement_figure(
            rec,
            title="Agreement with N4BiasFieldCorrection, by category",
            alt=f"Stacked bars, one per category of the N4 record ({len(rec.results)} cases): "
            "cases identical to ANTs as built, identical once ANTs uses correctly rounded "
            "maths, and rejected by both.",
            split=common.split_by_cases(differing, ("errors/help",)),
            original="ANTs",
        ),
        band,
        bias_figure(ctx),
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
