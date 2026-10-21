# SPDX-License-Identifier: Apache-2.0
"""antsApplyTransforms: the record's agreement (double and ``--float 1``), and fMRIPrep's
T1w-to-MNI composite applied to a real T1w with Lanczos interpolation, by ANTs and larmorx."""

from __future__ import annotations

from functools import cache
from pathlib import Path

import numpy as np

from larmorx_validation.figures import brain, common, records, style
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures.core import Context, Figure

TOOL = "ants-apply-transforms"
RUN = "ds000005-sub-01-noreconall"
ANAT = f"{RUN}/out/sub-01/anat"
T1 = f"{ANAT}/sub-01_desc-preproc_T1w.nii.gz"
XFM = f"{ANAT}/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5"
TPL = "templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_"

SPEED_LABELS = {
    "boldref → T1w 1 mm, Linear": "BOLD reference → T1w, linear",
    "boldref → T1w 1 mm, Lanczos": "BOLD reference → T1w, Lanczos",
    "T1w → MNI 2 mm (.h5 affine + warp), Linear": "T1w → MNI (.h5), linear",
    "T1w → MNI 2 mm (.h5 affine + warp), Lanczos": "T1w → MNI (.h5), Lanczos",
    "aseg → boldref, GenericLabel": "labels → BOLD reference",
    "BOLD 240 volumes → T1w grid, Lanczos (-e 3)": "240-volume BOLD → T1w, Lanczos",
}


def glibc_cases(rec: records.Record) -> set[str]:
    """The passing cases the record lists as differing from ANTs as built only through glibc."""
    for line in rec.text.splitlines():
        if line.startswith("Differing from ANTs as built only through glibc"):
            return set(records.case_list(line))
    raise LookupError(f"{rec.path_md.name}: no list of the cases glibc's maths changes")


def split(rec: records.Record):
    cm = glibc_cases(rec)

    def f(category: str, passing: list[dict]) -> dict[str, int]:
        out = {"identical": 0, "identical-cm": 0, "within": 0}
        for r in passing:
            if category == "transform-output":
                out["within"] += 1
            elif r["case"] in cm:
                out["identical-cm"] += 1
            else:
                out["identical"] += 1
        return out

    return f


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    b, lib = ants_bin(), crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload is missing")
    root = common.fmriprep_oracle()
    t1 = e.input("T1w", root / T1)
    xfm = e.input("T1w-to-MNI", root / XFM)
    ref = e.input("reference", Path(td.get(TPL + "T1w.nii.gz")))
    base = ["-d", "3", "-i", str(t1), "-r", str(ref), "-t", str(xfm), "-n", "LanczosWindowedSinc"]
    env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "4"}
    for prec, extra in (("double", []), ("float", ["--float", "1"])):
        e.run(
            f"ants-{prec}",
            [b / "antsApplyTransforms", *base, *extra, "-o", str(e.dir / f"ants-{prec}.nii.gz")],
            env=env,
            unset=("LD_PRELOAD",),
        )
        e.run(
            f"ants-{prec}-crmath",
            [
                b / "antsApplyTransforms",
                *base,
                *extra,
                "-o",
                str(e.dir / f"ants-{prec}-crmath.nii.gz"),
            ],
            env={**env, "LD_PRELOAD": str(lib)},
        )
        e.run(
            f"larmorx-{prec}",
            fcache.larmorx_cli(
                "ants",
                "antsApplyTransforms",
                *base,
                *extra,
                "-o",
                str(e.dir / f"larmorx-{prec}.nii.gz"),
            ),
            env={**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"},
            unset=("LD_PRELOAD",),
        )
    e.finish(
        versions={"ants": ants_version()},
        threads={"all": 4},
        settings="fMRIPrep's T1w-to-MNI152NLin2009cAsym composite (affine and SyN warp, .h5) "
        "applied to its preprocessed T1w on the template's 2 mm grid, LanczosWindowedSinc, in "
        "double and with --float 1; larmorx with upstream_defects=reproduce",
    )


@cache
def _images(cache_dir: str) -> dict:
    import larmorx_testdata as td

    d = Path(cache_dir)
    out = {
        k: brain.load(d / f"{k}.nii.gz")
        for k in (
            "ants-double",
            "ants-double-crmath",
            "larmorx-double",
            "ants-float",
            "ants-float-crmath",
            "larmorx-float",
        )
    }
    out["mask"] = brain.load(Path(td.get(TPL + "desc-brain_mask.nii.gz"))).data > 0
    return out


def warp_figure(ctx: Context) -> Figure:
    state: dict = {}

    def prepare(fig: Figure) -> None:
        e = fcache.entry(
            ctx,
            TOOL,
            "ds000005-sub-01-t1w-to-mni",
            "fMRIPrep's T1w-to-MNI composite applied to ds000005 sub-01's T1w",
            _compute,
        )
        state["img"] = _images(str(e.dir))
        fig.cache = [fcache.provenance(e)]

    def draw(theme: style.Theme, width: int):
        im = state["img"]
        mask = im["mask"]
        show = brain.dilate(mask, 1)
        plane, _ = brain.centre_planes(mask)
        sp = plane.spacing(im["larmorx-double"].zooms)
        m2 = plane.take(show)
        crop = brain.bbox(m2, 1)
        lx = plane.take(im["larmorx-double"].data)
        hi = float(np.percentile(lx[m2], 99.5))
        diffs = {
            "dd": im["larmorx-double"].data - im["ants-double"].data,
            "df": im["larmorx-float"].data - im["ants-float"].data,
            "dcm": im["larmorx-double"].data - im["ants-double-crmath"].data,
            "fcm": im["larmorx-float"].data - im["ants-float-crmath"].data,
        }
        lim = max(float(np.abs(v[show]).max()) for v in diffs.values()) or 1.0
        div = style.diverging(theme)

        def dp(title, key, sub=""):
            v = diffs[key]
            same = not np.any(v[show])
            return brain.Panel(
                title,
                plane.take(v),
                m2,
                div,
                -lim,
                lim,
                sp,
                crop=crop,
                outline=m2,
                note="bit-identical" if same else "",
                subtitle=brain.largest(v, show) + (f" · {sub}" if sub else ""),
            )

        cm_same = not (np.any(diffs["dcm"][show]) or np.any(diffs["fcm"][show]))
        cm = dp("larmorx − ANTs, CORE-MATH", "dcm", "double")
        if cm_same:
            cm.subtitle = "identical · double and --float 1"
        panels = [
            brain.Panel(
                "T1w in MNI space, larmorx",
                lx,
                m2,
                "gray",
                0,
                hi,
                sp,
                crop=crop,
                subtitle="Lanczos, double",
            ),
            dp("larmorx − ANTs as built", "dd", "double"),
            dp("larmorx − ANTs as built", "df", "--float 1"),
            cm,
        ]
        bars = {
            "d": brain.Bar(div, -lim, lim, "difference (full range in the brain)", fmt="{:.2g}")
        }
        return brain.draw_grid(
            theme, width, brain.Grid(panels, cols=4, cols_narrow=2, bars=bars, gap=12)
        )

    return Figure(
        "t1w-to-mni",
        "fMRIPrep's T1w-to-MNI transform applied by ANTs and larmorx",
        "Axial slices on the MNI152NLin2009cAsym 2 mm grid: ds000005 sub-01's T1w warped by "
        "larmorx with Lanczos interpolation, and the differences from ANTs as built and from "
        "ANTs with correctly rounded maths, in double precision and with --float 1, on one "
        "symmetric scale.",
        draw,
        data=[
            {
                **common.DS000005,
                "files": [f"fMRIPrep 25.2.6's desc-preproc_T1w and T1w-to-MNI .h5 ({RUN})"],
            },
            {
                "dataset": "MNI152NLin2009cAsym (TemplateFlow)",
                "licence": "MNI ICBM 152 notice",
                "notice": common.MNI_NOTICE,
            },
        ],
        prepare=prepare,
    )


def figures(ctx: Context) -> list[Figure]:
    rec = records.load(ctx.repo, TOOL)
    figs = [
        common.agreement_figure(
            rec,
            title="Agreement with antsApplyTransforms, by category",
            alt=f"Stacked bars, one per category of the antsApplyTransforms record "
            f"({len(rec.results)} cases): values identical to ANTs as built, identical once "
            "ANTs uses correctly rounded maths, transform files with ANTs' content, rejected by "
            "both, and expected divergences.",
            split=split(rec),
            original="ANTs",
            state_labels={"within": "transform files with ANTs' content (HDF5 by dataset)"},
            labels={"float": "float (--float 1)"},
        ),
        warp_figure(ctx),
    ]
    bench = records.load_bench(ctx.repo, TOOL)
    if bench is not None:
        series = (
            {
                ("ANTs 2.6.5", "1 thread"): "ANTs, 1 thread",
                ("ANTs 2.6.5", "12 threads"): "ANTs, 12 threads",
                ("larmorx (CLI)", "1 thread"): "larmorx, 1 thread",
                ("larmorx (CLI)", "12 threads"): "larmorx, 12 threads",
            },
            [
                ("ANTs, 1 thread", "original", "o"),
                ("ANTs, 12 threads", "original", "o-open"),
                ("larmorx, 1 thread", "larmorx", "o"),
                ("larmorx, 12 threads", "larmorx", "o-open"),
            ],
            ("ANTs, 1 thread", "larmorx, 1 thread"),
        )
        figs.append(
            common.speed_figure(
                bench,
                series=series,
                labels=SPEED_LABELS,
                title="Speed against ANTs",
                alt="Median seconds per input for ANTs (through ANTsPy) and larmorx's command line "
                "at 1 and 12 threads, on a log scale, with larmorx's speed-up at one thread.",
            )
        )
    return figs
