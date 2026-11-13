# SPDX-License-Identifier: Apache-2.0
"""One run of the confounds' evidence, in the Python environment that runs this module (the
part's ``e1``: ``<workspace>/oracles/python-sdcflows-2.17``, numpy 2.5.4 with OpenBLAS 0.3.34
and ``OPENBLAS_L2_SIZE=512``, SciPy 1.18.1, pandas 3.0.6, nipype 1.12.0), as the confounds'
``check_workflow.py --sections real`` runs fMRIPrep's workflows (its helpers are imported from
``validation/investigations/confounds``).

Usage::

    python -m larmorx_validation.evidence.confounds_run workflow <workflow> <out_dir> \\
        [--keep] [--threads-check]

``<workflow>``: a confounds workflow of fMRIPrep 25.2.6's runs, relative to
``<workspace>/oracles/fmriprep-25.2.6`` (``<run>/work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/
bold_confounds_wf``). LARMORX's ``bold_confounds`` runs from the workflow's own inputs (its
nodes' pickled inputs, ``check_workflow.workflow_inputs``) with ``ds_confounds`` writing the
derivatives:

- ``e3``: ``reproduce`` with fMRIPrep 25.2.6's releases (fMRIPrep 25.2.6, niworkflows 1.15.2,
  numpy 2.2.6, SciPy 1.15.2, pandas 2.2.3: LARMORX reproduces their arithmetic and texts in any
  environment), at 8 threads: the run compared with fMRIPrep's own files;
- ``sp``: the same with fMRIPrep's own motion table in place of LARMORX's (only where ``e3``'s
  table differs from fMRIPrep's: glibc's ``atan2``, D4), which must give fMRIPrep's every
  column;
- ``new``: ``reproduce`` with the newest releases (fMRIPrep 25.2.7, niworkflows 1.15.4, numpy
  2.5.4, SciPy 1.18.1, pandas 3.0.6), at 8 threads: the originals' own step (each node of it is
  the newest originals' bit for bit: the part's records);
- ``fix``: LARMORX's default (the D13 corrections) with the newest releases, at 8 threads;
- ``fix-1`` (``--threads-check``): the default at one thread, compared file by file with ``fix``.

``<out_dir>/stats.json``: per mode the confounds table compared column by column (by name: the
columns' order is niworkflows' set order, PIPE-43) with fMRIPrep's file (``e3``, ``sp``), with
``e3`` (``new``: the step), with ``new`` (``fix``: the default), the JSON sidecars, every node file
of ``e3`` against fMRIPrep's node folder (``check_workflow.compare_nodes``), the metadata
dictionaries, the notices LARMORX gave, the seconds. Without ``--keep`` only the derivatives of
each mode are kept (the node folders go).
"""

from __future__ import annotations

import argparse
import gzip
import json
import math
import os
import shutil
import sys
import time
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve()
#: The confounds' investigation (its check scripts' helpers are imported from there).
SCRIPTS = HERE.parents[3] / "investigations" / "confounds"

#: fMRIPrep 25.2.6's releases, as LARMORX's ``bold_confounds`` names them.
E3 = {
    "fmriprep_version": "25.2.6",
    "niworkflows_version": "1.15.2",
    "numpy_version": "2.2.6",
    "scipy_version": "1.15.2",
    "pandas_version": "2.2.3",
}
#: The metadata dictionaries compared with the nodes' result files.
DICTS = (
    ("tcc_metadata_fmt", "output"),
    ("acc_metadata_fmt", "output"),
    ("crowncc_metadata_fmt", "output"),
    ("merge_confound_metadata2", "out_dict"),
)


def releases() -> dict[str, str]:
    import importlib.metadata as md

    names = ("nipype", "niworkflows", "nitime", "numpy", "scipy", "pandas", "nibabel")
    out = {n: md.version(n) for n in names}
    for k in ("OPENBLAS_CORETYPE", "OPENBLAS_L2_SIZE", "OPENBLAS_NUM_THREADS"):
        out[k] = os.environ.get(k, "")
    return out


def _cw():
    if str(SCRIPTS) not in sys.path:
        sys.path.insert(0, str(SCRIPTS))
    import check_workflow as cw

    return cw


def _content(p: Path) -> bytes:
    data = p.read_bytes()
    return gzip.decompress(data) if p.name.endswith(".gz") else data


def _value(s: str) -> float | None:
    """A cell's number (pandas writes NaN as ``n/a``)."""
    try:
        return float(s)
    except ValueError:
        return None


def _negated(x: str, y: str) -> bool:
    """Whether the cell texts ``x`` and ``y`` are the same number with the other sign."""
    return x != y and (x == "-" + y or y == "-" + x)


def columns(a: Path, b: Path) -> dict[str, Any]:
    """Table ``a`` against table ``b``, column by column by name: per column the cells, those
    whose texts differ, the largest |a − b| and |a − b| ÷ the largest |b| over the column, and
    whether every differing cell is the other's number negated (a sign flip); the columns in one
    table only; whether the names come in the same order."""
    cw = _cw()
    ha, ca = cw.table(a)
    hb, cb = cw.table(b)
    out: dict[str, Any] = {
        "order_same": ha == hb,
        "only_a": [h for h in ha if h not in cb],
        "only_b": [h for h in hb if h not in ca],
        "rows": [len(next(iter(ca.values()), [])), len(next(iter(cb.values()), []))],
        "columns": {},
    }
    for h in hb:
        if h not in ca:
            continue
        xa, xb = ca[h], cb[h]
        differ = sum(x != y for x, y in zip(xa, xb, strict=False)) + abs(len(xa) - len(xb))
        rec: dict[str, Any] = {"cells": len(xb), "differ": differ}
        if differ:
            big = rel_den = 0.0
            na = 0
            for x, y in zip(xa, xb, strict=False):
                vb = _value(y)
                if vb is not None and math.isfinite(vb):
                    rel_den = max(rel_den, abs(vb))
                if x == y:
                    continue
                va = _value(x)
                if va is None or vb is None or not (math.isfinite(va) and math.isfinite(vb)):
                    na += 1
                    continue
                big = max(big, abs(va - vb))
            rec["max_abs"] = big
            rec["max_rel"] = big / rel_den if rel_den else None
            rec["na"] = na
            rec["sign"] = all(
                _negated(x, y) for x, y in zip(xa, xb, strict=False) if x != y
            ) and len(xa) == len(xb)
        out["columns"][h] = rec
    return out


def _meta(a: Path, b: Path) -> dict[str, Any]:
    ja = json.loads(a.read_text(encoding="utf-8"))
    jb = json.loads(b.read_text(encoding="utf-8"))
    return {
        "bytes_same": a.read_bytes() == b.read_bytes(),
        "added": sorted(set(ja) - set(jb)),
        "removed": sorted(set(jb) - set(ja)),
        "changed": sorted(k for k in ja if k in jb and ja[k] != jb[k]),
    }


def _files_same(a: Path, b: Path) -> dict[str, Any]:
    """Every file below ``a`` against the file of the same relative path below ``b``, decompressed."""
    same, differ, missing = 0, [], []
    for p in sorted(a.rglob("*")):
        if not p.is_file():
            continue
        q = b / p.relative_to(a)
        if not q.is_file():
            missing.append(str(p.relative_to(a)))
        elif _content(p) == _content(q):
            same += 1
        else:
            differ.append(str(p.relative_to(a)))
    return {"same": same, "differ": differ, "missing": missing}


def _bold(path: str) -> dict[str, Any]:
    import nibabel as nb

    img = nb.load(path)
    return {
        "shape": [int(x) for x in img.shape],
        "zooms": [float(z) for z in img.header.get_zooms()],
        "dtype": str(img.get_data_dtype()),
    }


def _workflow(rel: str, out: Path, keep: bool, threads_check: bool) -> dict[str, Any]:
    import larmorx
    from larmorx.pipelines.larmorprepx import confounds as pc

    cw = _cw()
    wf = cw.RUNS / rel
    run = Path(rel).parts[0]
    tag = f"{run}/{wf.parts[-2]}"
    args, opts = cw.workflow_inputs(wf)
    out.mkdir(parents=True, exist_ok=True)
    rec: dict[str, Any] = {
        "workflow": rel,
        "tag": tag,
        "run": run,
        "bold_run": wf.parts[-2],
        "bold": _bold(args["bold"]),
        "skip_vols": args["skip_vols"],
        "metadata": {k: v for k, v in opts["metadata"].items()},
        "options": {k: v for k, v in opts.items() if k not in ("metadata", "source_file")},
        "larmorx": larmorx.__version__,
        "modes": {},
    }

    def go(label: str, threads: int, **kw: Any):
        d = out / label
        if d.exists():
            shutil.rmtree(d)
        t0 = time.perf_counter()
        res, notes = cw.quiet(
            pc.bold_confounds,
            *args.values(),
            out_dir=d / "bold_confounds_wf",
            fmriprep_dir=d / "out",
            n_threads=threads,
            **opts,
            **kw,
        )
        tsv = Path(res.derivatives.out_file[0])
        rec["modes"][label] = {
            "threads": threads,
            "upstream_defects": kw.get("upstream_defects", "fix"),
            "releases": "fMRIPrep 25.2.6's" if "fmriprep_version" in kw else "the newest",
            "seconds": round(time.perf_counter() - t0, 2),
            "notices": dict(sorted(notes.items())),
            "tsv": str(tsv.relative_to(d)),
            "json": str(Path(res.derivatives.out_meta).relative_to(d)),
        }
        return res, tsv, Path(res.derivatives.out_meta)

    res_e3, tsv_e3, js_e3 = go("e3", 8, upstream_defects="reproduce", **E3)
    theirs = cw.derivative(wf, tsv_e3.name)
    rec["fmriprep"] = {
        "tsv": str(theirs.relative_to(cw.RUNS / run)),
        "name_same": tsv_e3.relative_to(out / "e3" / "out")
        == theirs.relative_to(cw.RUNS / run / "out"),
    }
    rec["e3_vs_fmriprep"] = columns(tsv_e3, theirs)
    rec["e3_vs_fmriprep"]["json"] = _meta(js_e3, theirs.with_suffix(".json"))
    # every node file of e3 against fMRIPrep's node folder, and the metadata dictionaries
    cw.COUNTS.clear()
    cw.INFO.clear()
    cw.FAILS.clear()
    cw.compare_nodes(out / "e3" / "bold_confounds_wf", wf, tag)
    for node, key in DICTS:
        cw.compare_dicts(res_e3.nodes[node], wf, node, key, tag)
    rec["nodes"] = {
        "counts": {k: list(v) for k, v in cw.COUNTS.items()},
        "info": {k: dict(v) for k, v in cw.INFO.items()},
        "fails": list(cw.FAILS),
    }
    differ = sum(c["differ"] for c in rec["e3_vs_fmriprep"]["columns"].values())
    if differ or rec["e3_vs_fmriprep"]["only_a"] or rec["e3_vs_fmriprep"]["only_b"]:
        with cw.spliced(wf / "motion_params"):
            _r, tsv_sp, _j = go("sp", 8, upstream_defects="reproduce", **E3)
        rec["sp_vs_fmriprep"] = columns(tsv_sp, theirs)
    _r, tsv_new, js_new = go("new", 8, upstream_defects="reproduce")
    rec["new_vs_e3"] = columns(tsv_new, tsv_e3)
    rec["new_vs_e3"]["json"] = _meta(js_new, js_e3)
    _r, tsv_fix, js_fix = go("fix", 8)
    rec["fix_vs_new"] = columns(tsv_fix, tsv_new)
    rec["fix_vs_new"]["json"] = _meta(js_fix, js_new)
    if threads_check:
        go("fix-1", 1)
        rec["fix1_vs_fix"] = _files_same(out / "fix-1", out / "fix")
    if not keep:
        for label in rec["modes"]:
            shutil.rmtree(out / label / "bold_confounds_wf", ignore_errors=True)
    rec["kept"] = keep
    return rec


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("task", choices=("workflow",))
    ap.add_argument("arg")
    ap.add_argument("out", type=Path)
    ap.add_argument("--keep", action="store_true")
    ap.add_argument("--threads-check", action="store_true")
    a = ap.parse_args()
    import logging
    import warnings

    logging.getLogger("nipype.workflow").setLevel(logging.ERROR)
    logging.getLogger("nipype.interface").setLevel(logging.ERROR)
    warnings.filterwarnings("ignore", category=DeprecationWarning)
    t0 = time.perf_counter()
    rec = _workflow(a.arg, a.out, a.keep, a.threads_check)
    rec["seconds"] = round(time.perf_counter() - t0, 2)
    rec["releases"] = releases()
    (a.out / "stats.json").write_text(json.dumps(rec, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
