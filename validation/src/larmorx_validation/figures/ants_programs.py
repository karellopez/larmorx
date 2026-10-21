# SPDX-License-Identifier: Apache-2.0
"""ImageMath, ThresholdImage, MultiplyImages, SmoothImage, ResampleImage and
ResampleImageBySpacing: an overview of their records, ImageMath by category, and speed on
fMRIPrep's, sMRIPrep's and sdcflows' calls."""

from __future__ import annotations

import re
from collections import Counter

from larmorx_validation.figures import charts, common, records
from larmorx_validation.figures.core import Context, Figure

PROGRAMS = {
    "ants-image-math": "ImageMath",
    "ants-threshold-image": "ThresholdImage",
    "ants-multiply-images": "MultiplyImages",
    "ants-smooth-image": "SmoothImage",
    "ants-resample-image": "ResampleImage",
    "ants-resample-image-by-spacing": "ResampleImageBySpacing",
}
NAMES = {
    "identical": "bit-identical to ANTs 2.6.5 (values and header)",
    "within": "within the declared tolerance",
    "agree": "passes; values not compared (ANTs reads memory it never wrote, or no image)",
}
#: Benchmark inputs shown (the pipelines' own calls first), by report.
SPEED = {
    "ants-programs": {
        "TruncateImageIntensity 0.01 0.999 256 (fMRIPrep's call), raw T1w ds000005 (176×256×256 "
        "int16)": "TruncateImageIntensity (fMRIPrep)",
        "ThresholdImage Otsu 3 with a brain mask, fMRIPrep T1w": "ThresholdImage Otsu 3",
        "MultiplyImages T1w × brain mask (fMRIPrep derivatives)": "MultiplyImages",
        "ImageMath 4 m 0.5, fMRIPrep BOLD series (4D, 28 MB gzipped)": "ImageMath m, 4D BOLD",
    },
    "ants-gaussian": {
        "ImageMath Laplacian 1.5 1 (fMRIPrep's call), raw T1w ds000005 (160×192×192 int16)": (
            "Laplacian (fMRIPrep), T1w"
        ),
        "ImageMath G 2 (discrete Gaussian, sigma 2 mm), raw T1w": "G 2 (Gaussian), T1w",
        "SmoothImage 3 sigma 1 voxel, raw T1w": "SmoothImage, T1w",
        "ResampleImageBySpacing 3 to 2 mm (smoothed, linear), raw T1w": "ResampleImageBySpacing",
        "ResampleImage 3 to 1 mm, B-spline order 3 as unsigned short, raw T1w": (
            "ResampleImage, B-spline 3"
        ),
    },
    "ants-morphology": {
        "ImageMath MD 2 (sMRIPrep), T1w brain mask ds000005 (160×192×192)": "MD 2 (sMRIPrep)",
        "ImageMath ME 10 (sMRIPrep's CSF step), T1w brain mask": "ME 10 (sMRIPrep)",
        "ImageMath FillHoles 2 (sMRIPrep), T1w brain mask": "FillHoles (sMRIPrep)",
        "ImageMath GetLargestComponent (sMRIPrep), T1w brain mask": "GetLargestComponent",
        "ImageMath MaurerDistance, T1w brain mask": "MaurerDistance",
    },
}


def identity(rec: records.Record) -> tuple[int, int]:
    """(bit-identical, compared) from the record's "Bit-identical: X of Y passing cases"."""
    m = re.search(r"\*\*Bit-identical: (\d+) of (\d+) passing cases\*\*", rec.text)
    if not m:
        raise LookupError(f"{rec.path_md.name}: no bit-identical summary")
    return int(m.group(1)), int(m.group(2))


def overview(ctx: Context) -> Figure:
    rows: list[charts.Row] = []
    sources: list[dict] = []
    total: Counter = Counter()
    for name, program in PROGRAMS.items():
        rec = records.load(ctx.repo, name)
        sources += rec.source()
        same, compared = identity(rec)
        s = Counter(r["status"] for r in rec.results)
        counts = {
            "identical": same,
            "within": compared - same,
            "agree": s["pass"] - compared,
            "expected": s["expected-divergence"],
            "fail": s["fail"],
            "both": s["both-error"],
        }
        counts = {k: v for k, v in counts.items() if v}
        rows.append(charts.Row(program, counts))
        total.update(counts)
    rows.insert(0, charts.Row("All six programs", dict(total), strong=True))
    return Figure(
        "overview",
        "Agreement of ImageMath and the ANTs filters with ANTs 2.6.5",
        "Stacked bars, one per program (ImageMath, ThresholdImage, MultiplyImages, "
        "SmoothImage, ResampleImage, ResampleImageBySpacing), of their validation records' "
        "cases: bit-identical to ANTs, within the declared tolerance, passing without values "
        "to compare, expected divergences and rejected by both.",
        lambda th, w: charts.agreement(th, w, rows, state_labels=NAMES),
        records=sources,
    )


def image_math(ctx: Context) -> Figure:
    rec = records.load(ctx.repo, "ants-image-math")
    t = rec.table("Category", "Bit-identical", "Pass")
    same = {r["Category"]: int(records.number(r["Bit-identical"])) for r in t.rows}
    # the cases the record names as within their tolerance; the other passing cases that are
    # not bit-identical had no values to compare
    within = set(_cases_after(rec, "Not bit-identical"))

    def split(cat: str, passing: list[dict]) -> dict[str, int]:
        n_within = sum(1 for r in passing if r["case"] in within)
        n_same = same.get(cat, 0)
        return {
            "identical": n_same,
            "within": n_within,
            "agree": len(passing) - n_same - n_within,
        }

    return common.agreement_figure(
        rec,
        name="image-math",
        title="ImageMath, by category",
        alt=f"Stacked bars, one per category of the ImageMath record ({len(rec.results)} "
        "cases): bit-identical to ANTs, within the declared tolerance, values not compared, "
        "and rejected by both.",
        split=split,
        state_labels=NAMES,
    )


def _cases_after(rec: records.Record, prefix: str) -> list[str]:
    for line in rec.text.splitlines():
        if line.startswith(prefix):
            return records.case_list(line)
    return []


def speed(ctx: Context) -> Figure | None:
    rows: list[charts.SpeedRow] = []
    sources: list[dict] = []
    mapping, legend, ratio = common.ANTS_SERIES
    for name, labels in SPEED.items():
        bench = records.load_bench(ctx.repo, name)
        if bench is None:
            continue
        sources += bench.source()
        rows += charts.bench_rows(bench, series=mapping, inputs=list(labels), labels=labels)
    if not rows:
        return None
    return Figure(
        "speed",
        "Speed against ANTs on the pipelines' calls",
        "Median seconds per call for ANTs and larmorx at 1 and 12 threads, on a log scale, "
        "for a selection of the benchmark reports' inputs (the pipelines' own calls first), "
        "with larmorx's speed-up at one thread.",
        lambda th, w: charts.speed(th, w, rows, series=legend, ratio=ratio),
        records=sources,
    )


def figures(ctx: Context) -> list[Figure]:
    figs = [overview(ctx), image_math(ctx)]
    s = speed(ctx)
    if s is not None:
        figs.append(s)
    return figs
