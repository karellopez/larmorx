# SPDX-License-Identifier: Apache-2.0
"""ITK transform files and HDF5 (``lx.transforms``, crates ``larmorx-hdf5``, ``larmorx-io``):
the record's agreement by category and its checks, file by file. No benchmark report yet:
the page says so."""

from __future__ import annotations

import re

from larmorx_validation.figures import charts, common, records
from larmorx_validation.figures.core import Context, Figure

TOOL = "itk-hdf5"

#: The record's summary lines -> (label, how the rest of the files count).
CHECKS = [
    ("Datasets as h5py reads them, bit for bit", "datasets as h5py reads them", None),
    ("HDF5 storage features", "storage features, as h5py reads them", None),
    (
        "Transforms as ITK reads them (double), bit for bit",
        "transforms as ITK reads them (double)",
        None,
    ),
    (
        "Transforms as ITK reads them (float), bit for bit",
        "transforms as ITK reads them (float)",
        None,
    ),
    ("nitransforms' chain, bit for bit", "nitransforms' chain", None),
    ("Files ITK refuses", "files ITK refuses", "both"),
    ("Files written:", "files written (ITK's compressed chunks)", "within"),
    ("Files written with larmorx's deflate", "files written (larmorx's own deflate)", "within"),
]


def checks(ctx: Context) -> Figure:
    rec = records.load(ctx.repo, TOOL)
    rows = []
    for prefix, label, rest in CHECKS:
        line = next(ln for ln in rec.text.splitlines() if ln.startswith(prefix))
        m = re.search(r"(\d+) of (\d+)", line)
        if not m:
            raise LookupError(f"{rec.path_md.name}: no count in {line!r}")
        k, n = int(m.group(1)), int(m.group(2))
        if rest == "both":
            rows.append(charts.Row(label, {"both": k, "fail": n - k}))
        else:
            rows.append(charts.Row(label, {"identical": k, (rest or "fail"): n - k}))
    refused = next(ln for ln in rec.text.splitlines() if ln.startswith("Files ITK reads and"))
    n_ref = int(re.search(r": (\d+)", refused).group(1))
    rows.append(charts.Row("files ITK reads and larmorx refuses", {"expected": n_ref}))
    rows = [charts.Row(r.label, {k: v for k, v in r.counts.items() if v}) for r in rows]
    names = {
        "identical": "bit for bit (written files: byte for byte)",
        "within": "the same content; other compressed bytes",
        "both": "refused alike (exit status and messages)",
        "expected": "expected divergence (refused by larmorx)",
    }
    return Figure(
        "checks",
        "ITK transform files: what was compared, file by file",
        "Stacked bars, one per check of the record: HDF5 datasets and storage features as h5py "
        "reads them, transforms as ITK reads them in double and float, nitransforms' chain, "
        "files ITK refuses, files written (byte-identical or with the same content), and the "
        "files larmorx refuses where ITK reads them.",
        lambda th, w: charts.agreement(
            th, w, rows, state_labels=names, unit="files", label_max=290
        ),
        records=rec.source(),
    )


def figures(ctx: Context) -> list[Figure]:
    rec = records.load(ctx.repo, TOOL)
    written = next(ln for ln in rec.text.splitlines() if ln.startswith("Files written:"))
    k = int(re.search(r"(\d+) of \d+", written).group(1))

    def split(cat: str, passing: list[dict]) -> dict[str, int]:
        if cat == "write":
            return {"identical": k, "within": len(passing) - k}
        return {"identical": len(passing)}

    return [
        common.agreement_figure(
            rec,
            title="Agreement with ITK, h5py and nitransforms, by category",
            alt=f"Stacked bars, one per category of the ITK transform files record "
            f"({len(rec.results)} cases): read bit for bit, written byte for byte or with the "
            "same content, rejected by both, and expected divergences.",
            split=split,
            state_labels={
                "identical": "bit for bit (written files: byte for byte)",
                "within": "written with the same content; other compressed bytes",
            },
        ),
        checks(ctx),
    ]
