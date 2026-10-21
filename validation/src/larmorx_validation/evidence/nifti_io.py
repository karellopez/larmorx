# SPDX-License-Identifier: Apache-2.0
"""NIfTI I/O's evidence (``lx.load``, ``lx.save`` against nibabel): what is compared, field by
field, on real files (a raw OpenNeuro BOLD run and fMRIPrep's preprocessed output of it); the
record's cases over the files of 41 OpenNeuro datasets, fMRIPrep's derivatives, templates and
two test suites, classified from each case's measured values; speed from the benchmark
report. Everything is computed in this process (nibabel and larmorx), no cache entry."""

from __future__ import annotations

import hashlib
import tempfile
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import records

TOOL = "nifti-io"
FILES = [
    (
        "openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz",
        "ds000005 sub-01 BOLD, run 1 (raw, as published)",
    ),
    (
        "openneuro-derivatives/ds000005-fmriprep/sub-01/func/"
        "sub-01_task-mixedgamblestask_run-1_desc-preproc_bold.nii.gz",
        "fMRIPrep's preprocessed BOLD of the same run (written by nibabel)",
    ),
]


def _fmt(v) -> str:
    if isinstance(v, bytes):
        return "“" + v.decode(errors="replace") + "”"
    if isinstance(v, tuple | list):
        return ", ".join(_fmt(x) for x in v)
    if isinstance(v, float):
        return repr(v) if v == v else "NaN"
    return str(v)


def _file_table(rel: str, label: str, i: int) -> tuple[dict, dict]:
    import larmorx_testdata as td
    import nibabel as nib

    import larmorx as lx
    from larmorx_validation.parity.nifti_io import FIELDS, _head, _ours, _theirs

    path = Path(td.get(rel))
    ref = nib.load(str(path))
    img = lx.load(str(path))
    h = img.header
    size = 348 if h.version == 1 else 540
    rh = type(ref.header)(_head(path, size))
    rows, same_all = [], True
    for name in FIELDS:
        if name not in rh:
            continue
        a = getattr(h, name)
        if name == "regular":
            a = bytes([a])
        a, b = _ours(a), _theirs(rh, name)
        same = _fmt(a) == _fmt(b)
        same_all &= same
        rows.append(
            {
                "head": f"`{name}`",
                "cells": [
                    {"text": _fmt(b)},
                    {"text": "the same" if same else _fmt(a)},
                    {"text": "" if same else "differs"},
                ],
            }
        )
    data_nib = np.asanyarray(ref.dataobj)
    data_lx = np.asarray(img.data)
    # bit for bit: the same type (byte order aside, larmorx returns native order) and bytes
    same_data = (
        data_nib.shape == data_lx.shape
        and data_nib.dtype.newbyteorder("=") == data_lx.dtype.newbyteorder("=")
        and np.ascontiguousarray(data_nib).astype(data_lx.dtype).tobytes()
        == np.ascontiguousarray(data_lx).tobytes()
    )
    sha = hashlib.sha256(np.ascontiguousarray(data_lx).tobytes()).hexdigest()[:12]
    aff = float(np.abs(np.asarray(img.affine) - ref.affine).max())
    q = float(np.abs(np.asarray(h.qform) - rh.get_qform()).max()) if h.qform is not None else 0.0
    s = float(np.abs(np.asarray(h.sform) - rh.get_sform()).max())
    stored = lx.io.read_header(str(path))
    reser = stored.to_bytes() == _head(path, size)
    with tempfile.TemporaryDirectory() as t:
        out = Path(t) / ("out" + "".join(path.suffixes[-2:]))
        lx.save(img, str(out))
        back = nib.load(str(out))
        back_same = np.array_equal(np.asanyarray(back.dataobj), data_nib)
        back_aff = float(np.abs(back.affine - ref.affine).max())
    tail = [
        {
            "head": f"the {len(rows)} header fields",
            "cells": [
                {"text": "as stored"},
                {"text": "every one the same" if same_all else "some differ"},
                {"verdict": "bit-identical" if same_all else "open"},
            ],
        },
        {
            "head": "data (each reader's default)",
            "cells": [
                {"text": f"{data_nib.dtype}, {'×'.join(map(str, data_nib.shape))}"},
                {
                    "text": f"the same values, bit for bit (SHA-256 `{sha}`)"
                    if same_data
                    else "differ"
                },
                {"verdict": "bit-identical" if same_data else "open"},
            ],
        },
        {
            "head": "affine (the one each reader uses)",
            "cells": [
                {"text": "qform/sform code " + f"{int(rh['qform_code'])}/{int(rh['sform_code'])}"},
                {"text": "max |Δ| " + ("0" if aff == 0 else f"{aff:.3g}") + " mm"},
                {"verdict": "bit-identical" if aff == 0 else "within-variability"},
            ],
        },
        {
            "head": "qform and sform matrices",
            "cells": [
                {"text": "from the quaternion; as stored"},
                {"text": f"max |Δ| {q:.3g} / {s:.3g} mm" if q or s else "equal"},
                {"verdict": "bit-identical" if q == 0 and s == 0 else "within-variability"},
            ],
        },
        {
            "head": "the header, written back",
            "cells": [
                {"text": f"{size} bytes"},
                {"text": "byte for byte" if reser else "differs"},
                {"verdict": "bit-identical" if reser else "open"},
            ],
        },
        {
            "head": "larmorx writes the image; nibabel reads it",
            "cells": [
                {"text": "data and affine"},
                {
                    "text": "the same data"
                    + (", affine equal" if back_aff == 0 else f", affine within {back_aff:.2g} mm")
                    if back_same
                    else "differs"
                },
                {
                    "verdict": "bit-identical"
                    if back_same and back_aff == 0
                    else "within-variability"
                    if back_same
                    else "open"
                },
            ],
        },
    ]
    t = common.table(
        f"file-{i}",
        f"{label}: every header field and the data, as nibabel 5.4.2 and larmorx read them",
        ["Field", "nibabel", "larmorx", "Verdict"],
        rows
        + [
            {**r, "class": "is-sep is-summary" if j == 0 else "is-summary"}
            for j, r in enumerate(tail)
        ],
        note=f"`{rel.split('/')[-1]}` ({core.short_path(str(path))}). The header fields as stored "
        "(nibabel's header class on the file's first bytes); the data in nibabel's default "
        "read; the affine is the matrix each library gives (nibabel computes a qform in "
        "numpy's long double, 80-bit on Linux x86-64, 64-bit on arm64 and Windows; larmorx "
        "in double everywhere).",
    )
    return t, {"fields": same_all, "data": same_data, "affine": aff}


def _group(category: str) -> str:
    if category.startswith("openneuro-derivatives"):
        return "fMRIPrep derivatives"
    if category.startswith("openneuro-"):
        return "OpenNeuro datasets"
    if category.startswith("templateflow-"):
        return "TemplateFlow templates"
    if category == "itk-testdata":
        return "ITK test data"
    if category == "nibabel-testdata":
        return "nibabel test data"
    return category


def _double_qform_same(case: str) -> bool | None:
    """Whether larmorx's matrices equal nibabel's when nibabel computes the qform in double
    precision, as it does where ``numpy.longdouble`` is 64 bits (arm64, Windows): nibabel's
    ``quaternions.MAX_FLOAT`` set to float64 for the call. None when the file is not at hand."""
    import larmorx_testdata as td
    import nibabel as nib
    from nibabel import quaternions

    import larmorx as lx

    try:
        path = Path(td.get(case))
    except Exception:  # not a catalog path
        return None
    saved = quaternions.MAX_FLOAT
    quaternions.MAX_FLOAT = np.float64
    try:
        ref = nib.load(str(path))
        hdr = ref.header
        ours = lx.load(str(path)).header
        same = np.array_equal(np.asarray(lx.load(str(path)).affine), ref.affine)
        if ours.qform is not None and int(hdr["qform_code"]):
            same &= np.array_equal(np.asarray(ours.qform), hdr.get_qform())
        return bool(same)
    finally:
        quaternions.MAX_FLOAT = saved


def _cases(ctx: core.Context, rec) -> list[dict]:
    descs = {}
    for t in rec.tables:
        if "Case" in t.columns and "What it tests" in t.columns:
            for r in t.rows:
                descs[records.strip_code(r["Case"])] = r["What it tests"]
    out = []
    for r in rec.results:
        v = common.outcome_verdict(r)
        m = r.get("measured", {})
        if v is None:
            aff = [
                float(x)
                for k, x in m.items()
                if k in ("affine", "qform matrix", "sform matrix") and isinstance(x, (int, float))
            ]
            worst = max(aff, default=0.0)
            if worst == 0:
                v = (
                    "bit-identical",
                    "voxel data in every read mode, every header field, the "
                    "extensions and the matrices equal; the written file read back alike",
                )
            elif _double_qform_same(r["case"]):
                v = (
                    "within-variability",
                    f"data and every header field equal; the matrices within {worst:.2g} mm: "
                    "nibabel computes the qform in numpy's long double, 80-bit on Linux x86-64; "
                    "computed in double, as nibabel does where long double is 64 bits (arm64, "
                    "Windows), its matrices are larmorx's, bit for bit (NIB-04)",
                )
            else:
                v = (
                    "open",
                    f"data and every header field equal; the matrices within {worst:.2g} mm, "
                    "not accounted for by nibabel's long double",
                )
        out.append(
            {
                "id": r["case"],
                "category": _group(r["category"]),
                "description": descs.get(r["case"], ""),
                "command": "",
                "results": {
                    "port": {
                        "outcome": r["status"],
                        "verdict": v[0],
                        "checks": f"{r['checks_passed']}/{r['checks_total']}",
                        "reason": common.clip(v[1], 400),
                    }
                },
            }
        )
    return out


def _speed(bench) -> dict | None:
    if bench is None:
        return None
    rows: dict[str, dict] = {}
    series = [
        {"id": "nibabel|read", "label": "nibabel, read", "mark": "original"},
        {"id": "larmorx (threads=1)|read", "label": "larmorx, read, 1 thread", "mark": "larmorx"},
        {
            "id": "larmorx (threads=all)|read",
            "label": "larmorx, read, all threads",
            "mark": "larmorx-open",
        },
        {"id": "SimpleITK|read", "label": "SimpleITK, read", "mark": "original-open"},
    ]
    inputs = sorted({r["input"] for r in bench.results if ".nii.gz)" in r["input"]})
    labels = {i: i.replace(", .nii.gz)", ")") for i in inputs}
    s = common.speed(
        bench,
        series,
        labels,
        title="",
        base=("nibabel|read", "larmorx (threads=1)|read"),
        note="",
        inputs=inputs,
    )
    if not s:
        return None
    for r in s["rows"]:
        rows[r["input"]] = r
    s["title"] = (
        "Seconds to read a gzipped file into memory, median of repeated runs, log scale; "
        "the factor is larmorx's speed-up over nibabel at one thread"
    )
    env = bench.data.get("environment", {})
    s["note"] = (
        f"From the benchmark report `docs/benchmarks/nifti-io.json` ({env.get('cpu', '?')}, "
        f"{env.get('logical_cpus', '?')} logical CPUs); uncompressed files and writing "
        "are in the report."
    )
    return s


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, TOOL)
    cases = _cases(ctx, rec)
    bench = records.load_bench(ctx.repo, TOOL)
    tables = []
    for i, (rel, label) in enumerate(FILES):
        t, _ = _file_table(rel, label, i)
        tables.append(t)
    cats = [
        "OpenNeuro datasets",
        "fMRIPrep derivatives",
        "TemplateFlow templates",
        "ITK test data",
        "nibabel test data",
    ]
    cats += sorted({c["category"] for c in cases} - set(cats))
    impl = common.implementation(
        cases,
        "port",
        "larmorx (own implementation)",
        "larmorx",
        f"docs/validation/{TOOL}.md",
        "validated",
    )
    impl["agreement"] = core.agreement(common.flat_verdicts(cases, "port"), cats)
    n_open = len({r["category"] for r in rec.results if r["category"].startswith("openneuro-ds")})
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "NIfTI I/O",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": bench.source() if bench else [],
            "cache": [],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [impl],
        "cases": cases,
        "cases_total": len(cases),
        "openneuro_datasets": n_open,
        "speed": _speed(bench),
        "tables": tables,
        "examples": [],
    }
