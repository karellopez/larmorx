# SPDX-License-Identifier: Apache-2.0
"""NIfTI-1/2 I/O against nibabel: the record's agreement by source of the files, and reading
and writing speed against nibabel and SimpleITK."""

from __future__ import annotations

import re
from collections import Counter

from larmorx_validation.figures import charts, records
from larmorx_validation.figures.core import Context, Figure

TOOL = "nifti-io"


def group(category: str) -> str:
    """The record's categories (one per collection) gathered by source."""
    if category.startswith("openneuro-derivatives"):
        return "fMRIPrep derivatives"
    if category.startswith("openneuro-"):
        return "OpenNeuro datasets (43)"
    if category.startswith("templateflow-"):
        return "TemplateFlow templates"
    if category == "itk-testdata":
        return "ITK test data"
    if category == "nibabel-testdata":
        return "nibabel test data"
    return category  # synthetic/<kind>


def agreement(ctx: Context) -> Figure:
    rec = records.load(ctx.repo, TOOL)
    by: dict[str, Counter] = {}
    for r in rec.results:
        by.setdefault(group(r["category"]), Counter())[r["status"]] += 1
    n_open = len({r["category"] for r in rec.results if r["category"].startswith("openneuro-ds")})
    rows, total = [], Counter()
    for name in sorted(by, key=lambda g: (g.startswith("synthetic"), g)):
        s = by[name]
        counts = {
            "identical": s["pass"],
            "expected": s["expected-divergence"],
            "fail": s["fail"],
            "both": s["both-error"],
        }
        counts = {k: v for k, v in counts.items() if v}
        label = name.replace("(43)", f"({n_open})")
        rows.append(charts.Row(label, counts))
        total.update(counts)
    rows.insert(0, charts.Row("All files", dict(total), strong=True))
    names = {"identical": "data, header and affine identical to nibabel's reading"}
    return Figure(
        "agreement",
        "Agreement with nibabel, by source of the files",
        f"Stacked bars, one per source of the NIfTI files in the record ({len(rec.results)} "
        "cases: real datasets, derivatives, templates, test suites and synthetic files): read "
        "identically to nibabel, rejected by both, and expected divergences.",
        lambda th, w: charts.agreement(th, w, rows, state_labels=names, unit="files"),
        records=rec.source(),
    )


def short(name: str) -> str:
    m = re.match(r"(\S+) \(([^,]+), (\.nii(?:\.gz)?)\)", name)
    if not m:
        return name
    stem, desc = m.group(1), m.group(2)
    if stem.startswith("tpl-"):
        kind = stem.split("_")[0].removeprefix("tpl-")
    else:
        kind = stem.split("_")[-1]
    return f"{kind}, {desc}"


def speed(ctx: Context, op: str) -> Figure | None:
    bench = records.load_bench(ctx.repo, TOOL)
    if bench is None:
        return None
    inputs = list(
        dict.fromkeys(r["input"] for r in bench.results if r["input"].endswith(".nii.gz)"))
    )
    mapping = {
        ("nibabel", op): "nibabel",
        ("SimpleITK", op): "SimpleITK",
        ("larmorx (threads=1)", op): "larmorx, 1 thread",
        ("larmorx (threads=all)", op): "larmorx, 12 threads",
    }
    legend = [
        ("nibabel", "original", "o"),
        ("SimpleITK", "other", "s"),
        ("larmorx, 1 thread", "larmorx", "o"),
        ("larmorx, 12 threads", "larmorx", "o-open"),
    ]
    rows = charts.bench_rows(
        bench, series=mapping, inputs=inputs, labels={i: short(i) for i in inputs}
    )
    verb = "Reading" if op == "read" else "Writing"
    return Figure(
        f"speed-{op}",
        f"{verb} compressed NIfTI files: speed against nibabel and SimpleITK",
        f"Median seconds to {op} each compressed (.nii.gz) file of the benchmark report with "
        "nibabel, SimpleITK and larmorx at 1 and 12 threads, on a log scale, with larmorx's "
        "speed-up over nibabel at one thread"
        + (" (each tool at its default compression level)" if op == "write" else "")
        + ".",
        lambda th, w: charts.speed(
            th, w, rows, series=legend, ratio=("nibabel", "larmorx, 1 thread")
        ),
        records=bench.source(),
    )


def figures(ctx: Context) -> list[Figure]:
    figs = [agreement(ctx)]
    for op in ("read", "write"):
        f = speed(ctx, op)
        if f is not None:
            figs.append(f)
    return figs
