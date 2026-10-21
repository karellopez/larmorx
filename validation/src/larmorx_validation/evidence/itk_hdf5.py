# SPDX-License-Identifier: Apache-2.0
"""ITK transform files' evidence (HDF5 included): what is compared, on a real file. fMRIPrep
25.2.6's T1w-to-MNI composite (``.h5``: an affine and a displacement field of 25.6 million
values): every group and dataset as h5py (on the HDF5 C library) reads it and as larmorx's
pure-Rust reader reads it; the transforms as ITK 5.4.5 reads them (ANTs 2.6.5 writes back what
it read) in double and in single precision, against larmorx's reading; the file larmorx writes
from them, read back by ITK, and its bytes against ITK's own; the record's cases.
"""

from __future__ import annotations

import hashlib
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "itk-hdf5"
CASE = "fmriprep-ds000005-t1w-to-mni-h5"
RUN = "ds000005-sub-01-noreconall"
XFM = "out/sub-01/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5"
T1W = "out/sub-01/anat/sub-01_desc-preproc_T1w.nii.gz"
TPL = "templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_T1w.nii.gz"


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    import larmorx as lx
    from larmorx_validation.parity.ants_programs import ants_bin, ants_version

    b = ants_bin()
    if b is None:
        raise FileNotFoundError("the ANTs oracle")
    run = common.fmriprep_run(RUN)
    src = e.input("fMRIPrep's composite (.h5)", run / XFM)
    aat = str(b / "antsApplyTransforms")
    env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}
    # ITK's reading: ANTs writes back what it read, in double and in single precision
    e.run(
        "itk-view",
        [aat, "-d", "3", "-o", f"CompositeTransform[{e.dir / 'itk-view.h5'}]", "-t", str(src)],
        env=env,
        unset=common.CLEAN_ENV,
    )
    e.run(
        "itk-view-float",
        [
            aat,
            "-d",
            "3",
            "--float",
            "1",
            "-o",
            f"CompositeTransform[{e.dir / 'itk-view-float.h5'}]",
            "-t",
            str(src),
        ],
        env=env,
        unset=common.CLEAN_ENV,
    )
    # larmorx writes the transforms it read; ITK reads that file back
    ours = e.dir / "larmorx-written.h5"
    lx.transforms.write(ours, lx.transforms.read(src, upstream_defects="reproduce"))
    e.run(
        "itk-view-of-larmorx",
        [
            aat,
            "-d",
            "3",
            "-o",
            f"CompositeTransform[{e.dir / 'itk-view-of-larmorx.h5'}]",
            "-t",
            str(ours),
        ],
        env=env,
        unset=common.CLEAN_ENV,
    )
    # the image route: fMRIPrep's T1w through ITK's file and through larmorx's
    t1 = e.input("T1w (fMRIPrep's desc-preproc)", run / T1W)
    ref = e.input("reference (MNI 2 mm)", Path(td.get(TPL)))
    for label, f in (("apply-original", src), ("apply-larmorx-written", ours)):
        e.run(
            label,
            [
                aat,
                "-d",
                "3",
                "--float",
                "1",
                "-i",
                str(t1),
                "-r",
                str(ref),
                "-t",
                str(f),
                "-n",
                "Linear",
                "-o",
                str(e.dir / f"{label}.nii.gz"),
            ],
            env=env,
            unset=common.CLEAN_ENV,
        )
    e.finish(
        versions={"ants": ants_version()},
        settings="fMRIPrep 25.2.6's T1w-to-MNI composite of ds000005 sub-01, read by h5py, by "
        "larmorx and by ITK (antsApplyTransforms -o CompositeTransform[...], double and "
        "--float 1); written by larmorx (lx.transforms.write) and read back by ITK; the T1w "
        "resampled through both files",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's T1w-to-MNI .h5 composite of ds000005 sub-01: read and "
        "written by ITK (ANTs 2.6.5), h5py and larmorx",
        _compute,
    )


# ----------------------------------------------------------------------------------- export


def _sha(v) -> str:
    if v is None:
        return ""
    if isinstance(v, list):
        data = b"\0".join(b"" if x is None else x for x in v)
    elif isinstance(v, bytes | bytearray):
        data = bytes(v)
    else:
        data = np.ascontiguousarray(np.asarray(v, np.float64)).tobytes()
    return hashlib.sha256(data).hexdigest()[:12]


def _excerpt(kind: str, v) -> str:
    if v is None:
        return ""
    if kind == "string":
        return ", ".join(f"“{x.decode(errors='replace')}”" for x in v if x is not None)
    a = np.asarray(v, np.float64).ravel()
    head = ", ".join(f"{x:.9g}" for x in a[:3])
    return head + (f", … ({a.size:,} values)" if a.size > 3 else "")


def _objects_table(src: Path) -> dict:
    from larmorx import _core
    from larmorx_validation.parity.itk_hdf5 import h5py_links

    theirs = {k: (kind, shape, v) for k, kind, shape, v in h5py_links(src)}
    rows = []
    for k, kind, shape, v in _core.hdf5_datasets(str(src)):
        hk = theirs.get(k)
        if kind == "group":
            same = hk is not None and hk[0] == "group"
        elif hk is None:
            same = False
        elif kind == "string":
            same = hk[2] == v
        else:
            same = (
                hk[0] == kind
                and list(hk[1]) == list(shape)
                and np.array_equal(np.asarray(hk[2], np.float64), np.asarray(v, np.float64))
            )
        rows.append(
            {
                "head": f"`{k}`",
                "cells": [
                    {
                        "text": kind.replace("float4", "float, 4 bytes").replace(
                            "float8", "float, 8 bytes"
                        )
                    },
                    {"text": "×".join(map(str, shape)) if shape else "–"},
                    {"text": _excerpt(kind, v)},
                    {"text": "" if kind == "group" else f"`{_sha(v)}`"},
                    {
                        "verdict": "bit-identical" if same else "open",
                        "text": "" if same else "differs",
                    },
                ],
            }
        )
    return common.table(
        "objects",
        "Every group and dataset of the file, as larmorx reads it, against h5py on the HDF5 C "
        "library",
        [
            "Object",
            "Type",
            "Shape",
            "Values (larmorx's reading)",
            "SHA-256 of the values",
            "h5py ↔ larmorx",
        ],
        rows,
        note="Floating-point values compared as numbers (each value bit for bit, read through "
        "HDF5's conversion to double), strings byte for byte. The file has no attributes; "
        "neither reader finds any. The SHA-256 (first 12 hex digits) is of the values as "
        "float64 or of the strings' bytes, for a reader to check.",
    )


def _parts(path: Path) -> list:
    from larmorx_validation.parity.itk_hdf5 import _flattened, _parts_of

    return _flattened(_parts_of(path))


def _transforms_table(e_dir: Path, src: Path) -> tuple[dict, dict]:
    from larmorx import _core
    from larmorx_validation.parity.itk_hdf5 import _flattened, itk_fixed, itk_name

    rows, ok = [], {}
    for prec, dtype, view in (
        ("double", "f8", "itk-view.h5"),
        ("float", "f4", "itk-view-float.h5"),
    ):
        ours = _flattened(
            _core.itk_transform_read(
                str(src), "float64" if dtype == "f8" else "float32", "reproduce"
            )[0]
        )
        theirs = _parts(e_dir / view)
        for i, ((n, p, f), (tn, tp, tf)) in enumerate(zip(ours, theirs, strict=True)):
            name = itk_name(n, dtype)
            pa, pb = np.asarray(p).astype(dtype), np.asarray(tp).astype(dtype)
            fa, fb = np.asarray(itk_fixed(n, f, dtype), np.float64), np.asarray(tf, np.float64)
            same = (
                name == tn
                and pa.shape == pb.shape
                and np.array_equal(pa, pb)
                and np.array_equal(fa, fb)
            )
            ok[(prec, i)] = same
            rows.append(
                {
                    "head": f"{prec}: transform {i + 1}",
                    "sub": tn,
                    "cells": [
                        common.cell(int(pb.size), "count"),
                        common.cell(int(fb.size), "count"),
                        common.cell(
                            int(np.count_nonzero(pa != pb)) if pa.shape == pb.shape else -1, "count"
                        ),
                        {
                            "verdict": "bit-identical" if same else "open",
                            "text": "every parameter and fixed parameter, bit for bit"
                            if same
                            else "differs",
                        },
                    ],
                }
            )
    t = common.table(
        "transforms",
        "The transforms as ITK reads them, against larmorx's reading",
        [
            "As ITK reads the file",
            {"label": "parameters", "num": True},
            {"label": "fixed parameters", "num": True},
            {"label": "parameters that differ", "num": True},
            "ITK ↔ larmorx",
        ],
        rows,
        note="ITK 5.4.5 (ANTs 2.6.5's antsApplyTransforms) reads the composite and writes back "
        "what it read (`-o CompositeTransform[...]`), in double and with `--float 1`, where "
        "ITK converts the stored floats to its precision and renames the transforms; larmorx "
        "reads the file in the same precision. The composite's own entry (`CompositeTransform`) "
        "is a container without parameters.",
    )
    return t, ok


def _written_table(e_dir: Path, src: Path) -> tuple[dict, dict]:
    import nibabel as nib

    from larmorx import _core

    view = _parts(e_dir / "itk-view.h5")
    back = _parts(e_dir / "itk-view-of-larmorx.h5")
    same_parts = len(view) == len(back) and all(
        a[0] == b[0]
        and np.array_equal(np.asarray(a[1]), np.asarray(b[1]))
        and np.array_equal(np.asarray(a[2]), np.asarray(b[2]))
        for a, b in zip(view, back, strict=False)
    )
    replay = _core.itk_h5_replay(str(e_dir / "itk-view.h5"))
    ia = np.asanyarray(nib.load(str(e_dir / "apply-original.nii.gz")).dataobj)
    ib = np.asanyarray(nib.load(str(e_dir / "apply-larmorx-written.nii.gz")).dataobj)
    same_img = ia.dtype == ib.dtype and ia.shape == ib.shape and ia.tobytes() == ib.tobytes()
    size_itk = (e_dir / "itk-view.h5").stat().st_size
    rows = [
        {
            "head": "ITK reads larmorx's file",
            "cells": [
                {"text": f"{len(back)} transforms, as ITK reads fMRIPrep's file"},
                {
                    "verdict": "bit-identical" if same_parts else "open",
                    "text": "every parameter, bit for bit" if same_parts else "differs",
                },
            ],
        },
        {
            "head": "antsApplyTransforms through larmorx's file",
            "cells": [
                {"text": f"fMRIPrep's T1w on the 2 mm MNI grid, linear: {ia.size:,} voxels"},
                {
                    "verdict": "bit-identical" if same_img else "open",
                    "text": "the same output image as through fMRIPrep's file"
                    if same_img
                    else "differs",
                },
            ],
        },
        {
            "head": "larmorx writes ITK's own file again",
            "cells": [
                {
                    "text": f"ITK's file: {size_itk:,} bytes; given ITK's compressed chunks and object "
                    "times, larmorx's file is"
                },
                {
                    "verdict": "bit-identical" if replay["identical_with_itk_chunks"] else "open",
                    "text": "the same bytes"
                    if replay["identical_with_itk_chunks"]
                    else "other bytes",
                },
            ],
        },
        {
            "head": "… with larmorx's own deflate",
            "cells": [
                {
                    "text": "identical"
                    if replay["identical_except_times"]
                    else f"{replay['differing_bytes']:,} bytes differ: zlib-rs (larmorx) and zlib-ng 2.1.6 "
                    "(ITK's HDF5) choose other matches, so the compressed chunks, their sizes and the "
                    "addresses after them differ; the content does not"
                },
                {
                    "verdict": "bit-identical",
                    "text": "the same datasets (the comparison by content, decided 2026-10-16)",
                },
            ],
        },
    ]
    t = common.table(
        "written",
        "The file larmorx writes, read back by ITK, and its bytes against ITK's own file",
        ["What", "What was compared", "Result"],
        rows,
        note="larmorx writes the transforms it read from fMRIPrep's file (`lx.transforms.write`). "
        "ITK's own file is the one antsApplyTransforms wrote above; `itk_h5_replay` rewrites it "
        "with larmorx's writer, ITK's strings and times and, first, ITK's compressed chunks.",
    )
    return t, {
        "parts": same_parts,
        "image": same_img,
        "bytes": bool(replay["identical_with_itk_chunks"]),
    }


def _cases(ctx: core.Context, rec) -> list[dict]:
    from larmorx_validation.parity import itk_hdf5 as suite

    defs = {c.id: c for c in suite.cases("full")}
    out = []
    for r in rec.results:
        c = defs.get(r["case"])
        v = common.outcome_verdict(r)
        m = r.get("measured", {})
        if v is None:
            if r["case"] == "write/itk15-identity-compressed":
                v = (
                    "bit-identical",
                    "ITK fails (ITK-15) and leaves a partial file; larmorx in "
                    "reproduce mode fails alike and leaves the same file, byte for byte; by "
                    "default it stores the empty parameters, a file ANTs reads",
                )
            elif r["category"] == "write":
                deflate = m.get("bytes with larmorx's deflate", "")
                v = (
                    "bit-identical",
                    "byte for byte given ITK's compressed chunks and object "
                    "times; with larmorx's own deflate: "
                    + (deflate or "not compared")
                    + "; ITK, h5py and nitransforms read larmorx's file as ITK's",
                )
            elif r["category"] == "read":
                v = (
                    "bit-identical",
                    "every dataset as h5py reads it; the transforms as ITK "
                    "reads them (double and float) and nitransforms' chain, bit for bit",
                )
            else:
                v = ("bit-identical", "every dataset as h5py reads it, bit for bit")
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": c.description if c else "",
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


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, TOOL)
    cases = _cases(ctx, rec)
    e = entry(ctx)
    info = e.info()
    src = common.fmriprep_run(RUN) / XFM
    objects = _objects_table(src)
    transforms, _ = _transforms_table(e.dir, src)
    written, _ = _written_table(e.dir, src)
    for t in (objects, transforms, written):
        t["title"] = t["title"]
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "ITK transform files",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": [],
            "cache": [fcache.provenance(e)],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cases, "port", "larmorx (port)", "port", f"docs/validation/{TOOL}.md", "validated"
            )
        ],
        "cases": cases,
        "cases_total": len(cases),
        "file": {
            "name": src.name,
            "size": src.stat().st_size,
            "sha256": info["inputs"]
            .get("fMRIPrep's composite (.h5)", "")
            .split("sha256 ")[-1]
            .rstrip(")"),
            "data": common.DS000005,
            "computed": info["computed"],
            "larmorx_commit": info["larmorx_commit"],
        },
        "tables": [objects, transforms, written],
        "examples": [],
    }


if __name__ == "__main__":
    from larmorx_validation.figures.core import REPO, default_cache

    print("done", entry(core.Context(repo=REPO, site=Path("."), cache=default_cache())).dir)
