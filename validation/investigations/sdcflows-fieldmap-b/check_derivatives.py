# SPDX-License-Identifier: Apache-2.0
"""The field-map derivatives: niworkflows' ``DerivativesDataSink`` and sdcflows'
``init_fmap_derivatives_wf`` and ``init_fmap_preproc_wf`` against larmorx's ports.

- ``nodes``: every ``ds_*`` node of the field-map derivatives in the fMRIPrep oracle runs
  (``fmap_preproc_wf/fmap_derivatives_wf_*``, fMRIPrep 25.2.6's image: niworkflows 1.15.4, pybids
  0.20.0, ``out_path_base = ''``) and in sdcflows 2.17.1's own BIDS-app runs
  (``fmap_derivatives_*``, ``out_path_base = 'sdcflows'``): its inputs (``_inputs.pklz``) given to
  :func:`larmorx.niworkflows.derivatives_data_sink`, the files it writes against the run's own
  derivatives: the names, the uncompressed bytes (header, extensions, data), the gzip bytes and
  the JSON sidecars byte for byte.
- ``workflow``: each estimator of those runs, built by larmorx (fMRIPrep's choice for the
  fMRIPrep runs, sdcflows' app's ``find_estimators`` for the app runs), its estimation outputs
  taken from the run's work folder, through :func:`larmorx.sdcflows.fmap_derivatives_workflow`:
  every derivative file of the estimator against the run's.
- ``synthetic``: niworkflows' own ``DerivativesDataSink`` (this environment's) against larmorx's
  on edge cases made with nibabel: copies and (de)compression, the header fixes (xform codes,
  oblique affines, units, ``RepetitionTime``), spaces and resolutions (TemplateFlow's metadata),
  the data type coercions (masks, ``dseg``, ``source``, scaled data), big-endian files, extensions,
  NIfTI-2, magic strings, files that are not NIfTI, several files and sources, the errors; the
  defects' cases with ``upstream_defects="reproduce"`` (the corrected results printed).
- ``preproc``: sdcflows 2.17.1's own ``init_fmap_preproc_wf`` (nipype, in this environment; its
  programs through the oracle cache: ANTs' N4 at one thread with CORE-MATH, FSL 6.0.7.23's
  ``prelude`` and ``topup``, AFNI 26.3.05's ``3dvolreg``) against
  :func:`larmorx.sdcflows.fmap_preproc_workflow` on the estimators of chosen datasets
  (``--datasets``): every derivative file.

Usage: ``LARMORX_WORKSPACE=<workspace> python -P check_derivatives.py [--sections ...]
[--datasets ...] --scratch <folder>``.
"""

from __future__ import annotations

import argparse
import gzip
import os
import re
import sys
import time
import warnings
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_estimators as ce  # noqa: E402
import check_workflows as cw  # noqa: E402

W = ce.W
FMRIPREP = ce.ORACLES
SDC = ce.SDC_RUNS


def load_inputs(node: Path) -> dict[str, Any]:
    from nipype.utils.filemanip import loadpkl

    return dict(loadpkl(node / "_inputs.pklz"))


def run_info(run: Path) -> dict[str, str]:
    text = (run / "RUN_INFO.txt").read_text(encoding="utf-8")
    return {
        "root": re.search(r"^BIDS input: (\S+)", text, re.M).group(1),  # type: ignore[union-attr]
        "sub": re.search(r"^participant: (\S+)", text, re.M).group(1),  # type: ignore[union-attr]
        "done": "yes" if re.search(r"^exit status: 0$", text, re.M) else "",
    }


def runs() -> list[tuple[str, Path, str]]:
    """(family, run folder, out_path_base) of every finished run with field-map derivatives."""
    out = []
    for run in sorted(FMRIPREP.iterdir()):
        if not (run / "RUN_INFO.txt").is_file() or not run_info(run)["done"]:
            continue
        if any(run.glob("work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/fmap_derivatives_wf_*")):
            out.append(("fmriprep", run, ""))
    for run in sorted(SDC.iterdir()):
        if any(run.glob("work/sdcflows_wf/fmap_derivatives_*")):
            out.append(("sdcflows", run, "sdcflows"))
    return out


def host(run: Path, family: str, p: str) -> str:
    """A path of the run's nodes on this machine (fMRIPrep's ran in its image)."""
    if family != "fmriprep":
        return p
    if p.startswith("/data/"):
        return str(Path(run_info(run)["root"]) / p[len("/data/") :])
    if p.startswith(("/work/", "/out/", "/out")):
        return str(run / p.lstrip("/"))
    return p


def ds_nodes(run: Path, family: str) -> list[Path]:
    pat = (
        "work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/fmap_derivatives_wf_*"
        if family == "fmriprep"
        else "work/sdcflows_wf/fmap_derivatives_*"
    )
    nodes = []
    for wf in sorted(run.glob(pat)):
        for node in sorted(wf.glob("ds_*")):
            if (node / "mapflow").is_dir():
                nodes += sorted(
                    (node / "mapflow").glob("_ds_*"),
                    key=lambda p: int(re.search(r"(\d+)$", p.name).group(1)),
                )
            elif (node / "_inputs.pklz").is_file():
                nodes.append(node)
    return nodes


def _flat(value: Any) -> list[Any]:
    """A MapNode's list input (its items may be one-item lists)."""
    out: list[Any] = []
    for v in value if isinstance(value, list) else [value]:
        out += _flat(v) if isinstance(v, list) else [v]
    return out


def raw(path: Path) -> bytes:
    with gzip.open(path, "rb") if str(path).endswith(".gz") else open(path, "rb") as f:
        return f.read()  # type: ignore[no-any-return]


def sink_call(inp: dict[str, Any], run: Path, family: str, base: Path, out_path_base: str):
    """The node's inputs as :func:`derivatives_data_sink`'s arguments."""
    from larmorx.niworkflows.bids import _CONFIG_ENTITIES, STATIC_INPUTS

    kw: dict[str, Any] = {"entities": {}, "metadata": {}}
    for k, v in inp.items():
        if k in STATIC_INPUTS:
            continue
        if k in _CONFIG_ENTITIES:
            kw["entities"][k] = v
        else:
            kw["metadata"][k] = v
    args = {
        "in_file": [host(run, family, f) for f in inp["in_file"]],
        "source_file": list(inp["source_file"]),
        "base_directory": str(base),
        "out_path_base": out_path_base,
        "check_hdr": inp.get("check_hdr", True),
        "compress": inp.get("compress", []),
        "data_dtype": inp.get("data_dtype") or None,
        "dismiss_entities": inp.get("dismiss_entities", []),
        "meta_dict": inp.get("meta_dict") or None,
        **kw,
    }
    return args


def compare_file(t: cw.Tally, what: str, label: str, ref: Path, mine: Path) -> None:
    t.add(f"{what}: names", mine.exists(), f"{label}: {mine.name}")
    if not mine.exists():
        return
    if ref.suffix == ".json":
        t.add(f"{what}: JSON bytes", ref.read_bytes() == mine.read_bytes(), f"{label}: {ref.name}")
        return
    t.add(f"{what}: contents (uncompressed bytes)", raw(ref) == raw(mine), f"{label}: {ref.name}")
    t.add(f"{what}: gzip bytes", ref.read_bytes() == mine.read_bytes(), f"{label}: {ref.name}")


def sec_nodes(t: cw.Tally, scratch: Path) -> None:
    from larmorx.io.nibabel_header import use_lapack
    from larmorx.niworkflows import derivatives_data_sink

    for family, run, opb in runs():
        for node in ds_nodes(run, family):
            inp = load_inputs(node)
            label = f"{run.name}/{node.parent.name if node.parent.name != 'mapflow' else node.parents[1].name}/{node.name}"
            base = scratch / "nodes" / family / run.name
            args = sink_call(inp, run, family, base, opb)
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                # fMRIPrep's image: numpy 2.2.6 on LAPACK 3.12.0 (nibabel's set_qform)
                use_lapack("3.12.0" if family == "fmriprep" else "3.12.1")
                res = derivatives_data_sink(**args)
            t.add(f"nodes {family}: sinks run (information)", True, "")
            if any(res.fixed_hdr):
                t.add(f"nodes {family}: sinks that fixed the header (information)", True, "")
            ref_base = Path(host(run, family, inp["base_directory"])) / opb
            for mine in res.out_file:
                rel = Path(mine).relative_to(base / opb)
                compare_file(t, f"nodes {family}", label, ref_base / rel, Path(mine))
            if res.out_meta:
                rel = Path(res.out_meta).relative_to(base / opb)
                compare_file(t, f"nodes {family}", label, ref_base / rel, Path(res.out_meta))
            # the oracle's sidecar exists exactly when larmorx writes one
            want_meta = (ref_base / Path(res.out_file[0]).relative_to(base / opb)).with_name(
                Path(res.out_file[0]).name.split(".", 1)[0] + ".json"
            )
            t.add(
                f"nodes {family}: sidecar written as the original",
                want_meta.exists() == (res.out_meta is not None) or len(res.out_file) > 1,
                label,
            )


def _fmriprep_estimators(run: Path) -> list[Any]:
    from larmorx.pipelines.larmorprepx import estimators as le

    for name, root, sub, opts in ce.oracle_datasets():
        if name != run.name:
            continue
        lay = le.fmriprep_layout(root, [sub], pybids_version="0.20.0")
        subwf = sorted((run / "work" / "fmriprep_25_2_wf").glob(f"sub_{sub}*_wf"))
        ses = re.search(r"_ses_(.+)_wf$", subwf[0].name) if subwf else None
        session = ses.group(1) if ses else None
        bold = [b for b in ce._bold_data(lay, sub) if session is None or f"_ses-{session}_" in b[0]]
        ests, _ = le.map_fieldmap_estimation(
            lay, sub, session, bold, opts["ignore_fieldmaps"], opts["use_syn"], opts["force_syn"],
            None, sdcflows_version="2.15", upstream_defects="reproduce",
        )  # fmt: skip
        return list(ests)
    return []


def _app_estimators(run: Path) -> list[Any]:
    import larmorx.sdcflows as ls
    from larmorx.bids import BIDSLayout, BIDSLayoutIndexer

    info = run_info(run)
    lay = BIDSLayout(
        info["root"],
        indexer=BIDSLayoutIndexer(validate=False, **ce.indexers(info["sub"])["sdcflows_app"]),
    )
    return list(
        ls.find_estimators(
            layout=lay, subject=info["sub"], fmapless=False, upstream_defects="reproduce"
        )
    )


def sec_workflow(t: cw.Tally, scratch: Path) -> None:
    import larmorx.sdcflows as ls
    from larmorx.io.nibabel_header import use_lapack

    for family, run, opb in runs():
        use_lapack("3.12.0" if family == "fmriprep" else "3.12.1")
        ls.clear_registry()
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ests = _fmriprep_estimators(run) if family == "fmriprep" else _app_estimators(run)
        info = run_info(run)
        for est in ests:
            pat = (
                f"work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/fmap_derivatives_wf_{est.sanitized_id}"
                if family == "fmriprep"
                else f"work/sdcflows_wf/fmap_derivatives_{est.sanitized_id}"
            )
            found = sorted(run.glob(pat))
            label = f"{run.name}/{est.bids_id}"
            t.add(f"workflow {family}: the estimator's derivatives folder", len(found) == 1, label)
            if len(found) != 1:
                continue
            wf = found[0]
            merge = load_inputs(wf / "merge_fmap")
            ref = load_inputs(wf / "ds_reference")
            mask = load_inputs(wf / "ds_mask")
            coeff = load_inputs(wf / "ds_coeff")
            if family == "fmriprep":
                sources = [str(f.path) for f in est.sources if f.suffix not in ("T1w", "T2w")]
            else:
                sources = [str(Path(f.path).absolute()) for f in est.sources]
            base = scratch / "workflow" / family / run.name
            fieldmap = merge["in_files"]
            res = ls.fmap_derivatives_workflow(
                sources,
                [
                    host(run, family, f)
                    for f in (fieldmap if isinstance(fieldmap, list) else [fieldmap])
                ],
                host(run, family, ref["in_file"][0]),
                [host(run, family, f) for f in _flat(coeff["in_file"])],
                host(run, family, mask["in_file"][0]),
                [f.metadata for f in est.sources],
                output_dir=base,
                bids_fmap_id=est.bids_id,
                write_coeff=True,
                write_mask=True,
                out_path_base=opb,
                out_dir=scratch / "workflow-work" / family / run.name / est.sanitized_id,
            )
            ref_base = run / "out" / opb
            mine_files = [res.fieldmap, res.fmap_ref, res.fmap_mask, *(res.fmap_coeff or ())]
            mine_files.append(
                str(Path(res.fieldmap).with_name(Path(res.fieldmap).name.split(".")[0] + ".json"))
            )
            fmapid = re.sub(r"[^a-zA-Z0-9]", "", est.bids_id)
            want = sorted(
                p.relative_to(ref_base)
                for p in ref_base.rglob(f"*_fmapid-{fmapid}_*")
                if "figures" not in p.parts
            )
            got = sorted(Path(m).relative_to(base / opb) for m in mine_files)
            t.add(f"workflow {family}: the file names", want == got, f"{label}: {want} vs {got}")
            for rel in got:
                mine = base / opb / rel
                if family == "fmriprep" and rel.suffix == ".json":
                    # RawSources: fMRIPrep's image saw the dataset at /data
                    text = mine.read_text(encoding="utf-8").replace(
                        info["root"].rstrip("/") + "/", "/data/"
                    )
                    mine = scratch / "workflow-json" / rel.name
                    mine.parent.mkdir(parents=True, exist_ok=True)
                    mine.write_text(text, encoding="utf-8")
                compare_file(t, f"workflow {family}", label, ref_base / rel, mine)


def _sdcflows_preproc(ds_root: str, sub: str, base: Path) -> Path:
    """sdcflows 2.17.1's ``init_fmap_preproc_wf`` in nipype on the dataset's estimators (its own
    ``find_estimators``, as its BIDS app finds them), PEPOLAR inputs set as fMRIPrep sets them."""
    import check_pepolar as cp
    from bids.layout import BIDSLayout, BIDSLayoutIndexer
    from sdcflows.fieldmaps import EstimatorType, clear_registry
    from sdcflows.utils.wrangler import find_estimators
    from sdcflows.workflows.base import init_fmap_preproc_wf

    shims = base / "shims"
    shims.mkdir(parents=True, exist_ok=True)
    cw.install_shims(shims)
    cp.install_shims(shims)
    clear_registry()
    lay = BIDSLayout(
        ds_root, indexer=BIDSLayoutIndexer(validate=False, **ce.indexers(sub)["sdcflows_app"])
    )
    ests = find_estimators(layout=lay, subject=sub, fmapless=False)
    wf = init_fmap_preproc_wf(
        estimators=ests, omp_nthreads=1, output_dir=str(base / "out"), subject=sub
    )
    for e in ests:
        if e.method == EstimatorType.PEPOLAR:
            inputs = getattr(wf.inputs, f"in_{e.sanitized_id}")
            inputs.in_data = [str(s.path) for s in e.sources]
            inputs.metadata = [s.metadata for s in e.sources]
    cw.run_nipype(wf, base / "work")
    return base / "out"


def sec_preproc(
    t: cw.Tally, scratch: Path, datasets: list[str], nt: int, times: list[float]
) -> None:
    import check_pepolar as cp

    import larmorx.sdcflows as ls
    from larmorx.bids import BIDSLayout, BIDSLayoutIndexer

    os.environ.setdefault("FSLDIR", cp.fsldir())
    for name in datasets:
        run = SDC / name
        info = run_info(run)
        base = scratch / "preproc" / name
        t0 = time.time()
        ref_out = _sdcflows_preproc(info["root"], info["sub"], base / "sdcflows")
        times.append(time.time() - t0)
        print(f"INFO {name}: sdcflows' init_fmap_preproc_wf {times[-1]:.0f} s (one thread)")
        ls.clear_registry()
        lay = BIDSLayout(
            info["root"],
            indexer=BIDSLayoutIndexer(validate=False, **ce.indexers(info["sub"])["sdcflows_app"]),
        )
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ests = ls.find_estimators(layout=lay, subject=info["sub"], fmapless=False)
            t0 = time.time()
            res = ls.fmap_preproc_workflow(
                ests,
                omp_nthreads=1,
                output_dir=base / "larmorx" / "out",
                subject=info["sub"],
                work_dir=base / "larmorx" / "work",
                n_threads=nt,
                upstream_defects="reproduce",
                numpy_version=cw.NP,
                scipy_version=cw.SP,
                fsldir=cp.fsldir(),
                prelude_implementation="replica",
                topup_implementation="replica",
                volreg_implementation="replica",
            )
        print(
            f"INFO {name}: larmorx's fmap_preproc_workflow {time.time() - t0:.0f} s ({nt} threads)"
        )
        want = sorted(
            p.relative_to(ref_out)
            for p in ref_out.rglob("*")
            if p.is_file() and "figures" not in p.parts
        )
        mine_out = base / "larmorx" / "out"
        got = sorted(p.relative_to(mine_out) for p in mine_out.rglob("*") if p.is_file())
        t.add("preproc: the derivative files", want == got, f"{name}: {set(want) ^ set(got)}")
        for rel in want:
            compare_file(t, "preproc", name, ref_out / rel, mine_out / rel)
        t.add("preproc: one output per estimator", len(res.fmap) == len(ests), name)


def _img(
    path: Path,
    data: Any,
    *,
    codes=(1, 1),
    units=("mm", None),
    affine=None,
    zooms=None,
    version: int = 1,
    slope=None,
    big=False,
    ext: bytes | None = None,
    magic=None,
) -> str:
    """A synthetic input written by nibabel (the original's own library)."""
    import nibabel as nb
    import numpy as np

    data = np.asarray(data)
    aff = np.diag([2.0, 2.5, 3.0, 1.0]) if affine is None else np.asarray(affine, dtype=float)
    cls = nb.Nifti1Image if version == 1 else nb.Nifti2Image
    hdr = cls.header_class(endianness=">" if big else "<")
    hdr.set_data_dtype(data.dtype)
    img = cls(data, aff, hdr)
    img.header.set_qform(aff, codes[0])
    img.header.set_sform(aff, codes[1])
    img.header.set_xyzt_units(*units)
    if zooms is not None:
        img.header.set_zooms(zooms)
    if ext is not None:
        img.header.extensions.append(nb.nifti1.Nifti1Extension(6, ext))
    path.parent.mkdir(parents=True, exist_ok=True)
    if slope is not None:
        # stored integers with a scaling nibabel's writer would not keep
        from niworkflows.utils.images import unsafe_write_nifti_header_and_data

        hdr2 = img.header.copy()
        hdr2.set_data_dtype(data.dtype)
        hdr2.set_slope_inter(*slope)
        unsafe_write_nifti_header_and_data(str(path), hdr2, data)
    else:
        nb.save(img, str(path))
    if magic is not None:
        opener = gzip.open if str(path).endswith(".gz") else open
        with opener(path, "rb") as f:
            b = bytearray(f.read())
        b[344:348] = magic
        with opener(path, "wb") as f:
            f.write(bytes(b))
    return str(path)


def synthetic_cases(root: Path) -> list[dict[str, Any]]:
    import json

    import numpy as np

    rng = np.random.default_rng(12)
    inp = root / "inputs"
    f32 = rng.random((4, 5, 6)).astype(np.float32)
    f4d = rng.random((4, 5, 6, 3)).astype(np.float32)
    fm = {"suffix": "fieldmap", "datatype": "fmap", "desc": "preproc"}
    oblique = np.array(
        [[1.9, 0.3, 0.1, -10.0], [-0.2, 2.4, 0.4, 12.0], [0.05, -0.5, 2.9, 3.5], [0, 0, 0, 1]]
    )
    cases: list[dict[str, Any]] = [
        {
            "name": "copy_gz",
            "in_file": _img(inp / "a.nii.gz", f32),
            "entities": fm,
            "compress": [True],
        },
        {
            "name": "copy_nii_to_gz",
            "in_file": _img(inp / "b.nii", f32),
            "entities": fm,
            "compress": [True],
        },
        {
            "name": "copy_gz_to_nii",
            "in_file": _img(inp / "c.nii.gz", f32),
            "entities": fm,
            "compress": [False],
        },
        {
            "name": "copy_nii_nii_mat",
            "in_file": _img(inp / "d.nii", f32),
            "entities": fm,
            "mat": True,
        },
        {
            "name": "fix_codes_00",
            "in_file": _img(inp / "e.nii.gz", f32, codes=(0, 0)),
            "entities": fm,
        },
        {
            "name": "fix_codes_22_oblique",
            "in_file": _img(inp / "f.nii.gz", f32, codes=(2, 2), affine=oblique),
            "entities": fm,
        },
        {
            "name": "fix_codes_q1_s0_oblique",
            "in_file": _img(inp / "f2.nii.gz", f32, codes=(1, 0), affine=oblique),
            "entities": fm,
        },
        {
            "name": "fix_time_units",
            "in_file": _img(inp / "g.nii.gz", f4d, units=("mm", "sec")),
            "entities": fm,
        },
        {
            "name": "fix_space_unit_unknown",
            "in_file": _img(inp / "g2.nii.gz", f32, units=("unknown", None)),
            "entities": fm,
        },
        {
            "name": "fix_space_unit_meter_kept",
            "in_file": _img(inp / "g3.nii.gz", f32, units=("meter", None), codes=(0, 1)),
            "entities": fm,
        },
        {
            "name": "bold_units",
            "in_file": _img(inp / "h.nii.gz", f4d),
            "source": "/bids/sub-01/func/sub-01_task-x_bold.nii.gz",
            "entities": {"desc": "preproc", "suffix": "bold"},
        },
        {
            "name": "rt_input",
            "in_file": _img(inp / "i.nii.gz", f4d, units=("mm", "sec"), zooms=(2.0, 2.5, 3.0, 2.0)),
            "source": "/bids/sub-01/func/sub-01_task-x_bold.nii.gz",
            "entities": {"desc": "preproc", "suffix": "bold"},
            "metadata": {"RepetitionTime": 2.5},
        },
        {
            "name": "rt_meta_dict",
            "in_file": _img(inp / "j.nii.gz", f4d, units=("mm", "sec"), zooms=(2.0, 2.5, 3.0, 2.0)),
            "source": "/bids/sub-01/func/sub-01_task-x_bold.nii.gz",
            "entities": {"desc": "preproc", "suffix": "bold"},
            "meta_dict": {"RepetitionTime": 2.5, "Z": "val"},
            "init_metadata": {"SkullStripped": True},
        },
        {
            "name": "rt_3d_pipe30",
            "in_file": _img(inp / "k.nii.gz", f32),
            "source": "/bids/sub-01/func/sub-01_task-x_bold.nii.gz",
            "entities": {"desc": "preproc", "suffix": "boldref"},
            "metadata": {"RepetitionTime": 2.0},
            "reproduce": True,
        },
        {
            "name": "space_standard_res",
            "in_file": _img(inp / "l.nii.gz", f32),
            "source": "/bids/sub-01/func/sub-01_task-x_bold.nii.gz",
            "entities": {
                "desc": "preproc",
                "suffix": "bold",
                "space": "MNI152NLin2009cAsym",
                "resolution": "2",
            },
        },
        {
            "name": "space_native_res",
            "in_file": _img(inp / "m.nii.gz", f32),
            "source": "/bids/sub-01/func/sub-01_task-x_bold.nii.gz",
            "entities": {
                "desc": "preproc",
                "suffix": "bold",
                "space": "T1w",
                "resolution": "native",
            },
        },
        {
            "name": "space_nonstandard_res",
            "in_file": _img(inp / "n.nii.gz", f32),
            "source": "/bids/sub-01/func/sub-01_task-x_bold.nii.gz",
            "entities": {
                "desc": "preproc",
                "suffix": "bold",
                "space": "fsnative",
                "resolution": "2",
            },
        },
        {
            "name": "mask_float",
            "in_file": _img(inp / "o.nii.gz", (f32 > 0.5).astype(np.float32)),
            "entities": {"desc": "brain", "suffix": "mask", "datatype": "fmap"},
        },
        {
            "name": "mask_int16_pipe29",
            "in_file": _img(inp / "p.nii.gz", np.array([[[300, -5], [1, 0]]], dtype=np.int16)),
            "entities": {"desc": "brain", "suffix": "mask", "datatype": "fmap"},
            "reproduce": True,
        },
        {
            "name": "dseg_from_float",
            "in_file": _img(inp / "q.nii.gz", (f32 * 7).astype(np.float64)),
            "source": "/bids/sub-01/anat/sub-01_T1w.nii.gz",
            "entities": {"suffix": "dseg"},
        },
        {
            "name": "dtype_source",
            "in_file": _img(inp / "r.nii.gz", f32 * 100),
            "source": _img(
                root / "bids" / "sub-01" / "anat" / "sub-01_T1w.nii.gz", (f32 * 10).astype(np.int16)
            ),
            "entities": {"desc": "preproc", "suffix": "T1w"},
            "data_dtype": "source",
        },
        {
            "name": "dtype_float32_of_scaled",
            "in_file": _img(
                inp / "s.nii.gz", np.arange(120, dtype=np.int16).reshape(4, 5, 6), slope=(2.0, 1.0)
            ),
            "entities": fm,
            "data_dtype": "float32",
        },
        {
            "name": "scaled_fix_path",
            "in_file": _img(
                inp / "t.nii.gz",
                np.arange(120, dtype=np.int16).reshape(4, 5, 6),
                slope=(0.5, -3.0),
                codes=(0, 0),
            ),
            "entities": fm,
        },
        {
            "name": "big_endian_fix",
            "in_file": _img(
                inp / "u.nii.gz",
                np.arange(120, dtype=np.int16).reshape(4, 5, 6),
                codes=(0, 2),
                big=True,
            ),
            "entities": fm,
        },
        {
            "name": "extensions_fix",
            "in_file": _img(inp / "v.nii.gz", f32, codes=(0, 0), ext=b"a comment"),
            "entities": fm,
        },
        {
            "name": "nifti2_fix",
            "in_file": _img(inp / "w.nii", f32, codes=(0, 0), version=2),
            "entities": fm,
            "compress": [True],
        },
        {
            "name": "ni1_magic",
            "in_file": _img(inp / "x.nii", f32, codes=(0, 0), magic=b"ni1\x00"),
            "entities": fm,
        },
        {
            "name": "bad_magic_copied",
            "in_file": _img(inp / "y.nii", f32, codes=(0, 0), magic=b"xyz\x00"),
            "entities": fm,
        },
        {
            "name": "json_file",
            "in_file": "JSON",
            "entities": {
                "desc": "preproc",
                "suffix": "fieldmap",
                "datatype": "fmap",
                "extension": None,
            },
        },
        {
            "name": "two_files_list_entity",
            "in_file": [_img(inp / "z1.nii.gz", f32), _img(inp / "z2.nii.gz", f32)],
            "entities": {**fm, "custom": [1, 2]},
            "allowed": ("custom",),
            "metadata": {"Units": "Hz"},
        },
        {
            "name": "two_sources_sessions",
            "in_file": _img(inp / "aa.nii.gz", f32),
            "source": [
                "/bids/sub-01/ses-A/fmap/sub-01_ses-A_acq-a_fieldmap.nii.gz",
                "/bids/sub-01/ses-B/fmap/sub-01_ses-B_acq-a_fieldmap.nii.gz",
            ],
            "entities": fm,
        },
        {
            "name": "dismiss",
            "in_file": _img(inp / "bb.nii.gz", f32),
            "entities": fm,
            "dismiss": ["acquisition", "run"],
        },
        {
            "name": "custom_fmapid",
            "in_file": _img(inp / "cc.nii.gz", f32),
            "entities": {**fm, "fmapid": "auto00001", "label": "x"},
            "metadata": {"B0FieldIdentifier": "auto_00001", "IntendedFor": ["func/a.nii.gz"]},
        },
        {
            "name": "compress_list",
            "in_file": [_img(inp / "dd1.nii.gz", f32), _img(inp / "dd2.nii", f32)],
            "entities": {**fm, "custom": ["a", "b"]},
            "allowed": ("custom",),
            "compress": [False, True],
        },
        {
            "name": "compress_short_pipe28",
            "in_file": [_img(inp / f"ee{i}.nii.gz", f32) for i in range(3)],
            "entities": {**fm, "custom": ["a", "b", "c"]},
            "allowed": ("custom",),
            "compress": [False, True],
            "reproduce": True,
        },
        {
            "name": "no_path",
            "in_file": _img(inp / "ff.nii.gz", f32),
            "entities": {"suffix": "nothingmatches", "datatype": "fmap"},
        },
        {
            "name": "not_matched",
            "in_file": [_img(inp / "gg1.nii.gz", f32), _img(inp / "gg2.nii.gz", f32)],
            "entities": fm,
        },
        {
            "name": "check_hdr_off",
            "in_file": _img(inp / "hh.nii.gz", f32, codes=(0, 0)),
            "entities": fm,
            "check_hdr": False,
        },
    ]
    jf = inp / "sidecar_like.json"
    jf.write_text(json.dumps({"a": 1}), encoding="utf-8")
    for c in cases:
        if c["in_file"] == "JSON":
            c["in_file"] = str(jf)
    return cases


def _run_original(c: dict[str, Any], base: Path, cwd: Path) -> Any:
    from niworkflows.interfaces.bids import DerivativesDataSink

    cwd.mkdir(parents=True, exist_ok=True)
    old = os.getcwd()
    os.chdir(cwd)
    try:
        kw = dict(c.get("init_metadata", {}))
        ds = DerivativesDataSink(
            base_directory=str(base), allowed_entities=c.get("allowed", ()), **kw
        )
        if "check_hdr" in c:
            ds.inputs.check_hdr = c["check_hdr"]
        ds.inputs.in_file = c["in_file"]
        ds.inputs.source_file = c.get(
            "source", "/bids/sub-01/fmap/sub-01_acq-a_run-01_fieldmap.nii.gz"
        )
        if "compress" in c:
            ds.inputs.compress = c["compress"]
        if "data_dtype" in c:
            ds.inputs.data_dtype = c["data_dtype"]
        if "dismiss" in c:
            ds.inputs.dismiss_entities = c["dismiss"]
        if "meta_dict" in c:
            ds.inputs.meta_dict = dict(c["meta_dict"])
        for k, v in c.get("entities", {}).items():
            if v is not None:
                setattr(ds.inputs, k, v)
        for k, v in c.get("metadata", {}).items():
            setattr(ds.inputs, k, v)
        res = ds.run()
        out = res.outputs.out_file
        return ([out] if isinstance(out, str) else list(out)), res.outputs.out_meta
    except Exception as e:
        return e
    finally:
        os.chdir(old)


def _run_mine(c: dict[str, Any], base: Path, mode: str) -> Any:
    import json

    from larmorx.niworkflows import derivatives_data_sink

    meta = {}
    tf_meta = {}
    tpl = (
        Path.home()
        / ".cache"
        / "templateflow"
        / "tpl-MNI152NLin2009cAsym"
        / "template_description.json"
    )
    if tpl.exists():
        tf_meta = {"MNI152NLin2009cAsym": json.loads(tpl.read_text(encoding="utf-8"))}
    meta.update(c.get("metadata", {}))
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            r = derivatives_data_sink(
                c["in_file"],
                c.get("source", "/bids/sub-01/fmap/sub-01_acq-a_run-01_fieldmap.nii.gz"),
                base_directory=base,
                check_hdr=c.get("check_hdr", True),
                compress=c.get("compress", ()),
                data_dtype=c.get("data_dtype"),
                dismiss_entities=c.get("dismiss", ()),
                meta_dict=c.get("meta_dict"),
                allowed_entities=c.get("allowed", ()),
                entities={k: v for k, v in c.get("entities", {}).items() if v is not None},
                metadata=meta,
                init_metadata=c.get("init_metadata"),
                template_metadata=tf_meta,
                templateflow_version="25.1.2",
                upstream_defects=mode,
            )
        return list(r.out_file), r.out_meta
    except Exception as e:
        return e


def sec_synthetic(t: cw.Tally, scratch: Path) -> None:
    root = scratch / "synthetic"
    cases = synthetic_cases(root)
    for c in cases:
        if c["name"] == "copy_nii_nii_mat":
            Path(c["in_file"][:-4] + ".mat").write_text("a related file\n", encoding="utf-8")
        ref_base = root / "ref" / c["name"]
        mine_base = root / "mine" / c["name"]
        ref = _run_original(c, ref_base, root / "cwd" / c["name"])
        mode = "reproduce" if c.get("reproduce") else "fix"
        mine = _run_mine(c, mine_base, mode)
        label = c["name"]
        if isinstance(ref, Exception) or isinstance(mine, Exception):
            same = (
                isinstance(ref, Exception)
                and isinstance(mine, Exception)
                and type(ref).__name__ == type(mine).__name__
                and str(mine).split("\n")[0] in str(ref)
            )
            t.add("synthetic: the same error", same, f"{label}: {ref!r} vs {mine!r}")
            if c.get("reproduce"):
                fixed = _run_mine(c, root / "mine-fix" / c["name"], "fix")
                print(f"INFO {label} with upstream_defects='fix': {fixed!r}"[:300])
            continue
        r_files, r_meta = ref
        m_files, m_meta = mine
        rel_r = [str(Path(f).relative_to(ref_base)) for f in r_files]
        rel_m = [str(Path(f).relative_to(mine_base)) for f in m_files]
        t.add("synthetic: out_file", rel_r == rel_m, f"{label}: {rel_r} vs {rel_m}")
        t.add("synthetic: out_meta", bool(r_meta) == bool(m_meta), label)
        tree_r = sorted(str(p.relative_to(ref_base)) for p in ref_base.rglob("*") if p.is_file())
        tree_m = sorted(str(p.relative_to(mine_base)) for p in mine_base.rglob("*") if p.is_file())
        t.add("synthetic: every file written", tree_r == tree_m, f"{label}: {tree_r} vs {tree_m}")
        for rel in tree_r:
            if (mine_base / rel).exists():
                if rel.endswith(".json") or rel.endswith(".mat"):
                    t.add(
                        "synthetic: JSON and other bytes",
                        (ref_base / rel).read_bytes() == (mine_base / rel).read_bytes(),
                        f"{label}: {rel}",
                    )
                else:
                    compare_file(t, "synthetic", label, ref_base / rel, mine_base / rel)
        if c.get("reproduce"):
            fixed = _run_mine(c, root / "mine-fix" / c["name"], "fix")
            print(f"INFO {label} with upstream_defects='fix': {fixed!r}"[:300])


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", nargs="*", default=["nodes", "workflow"])
    ap.add_argument("--datasets", nargs="*", default=["ds005752-sub-ON01016-direct"])
    ap.add_argument("--scratch", required=True)
    ap.add_argument("--threads", type=int, default=8)
    args = ap.parse_args()
    scratch = Path(args.scratch)
    scratch.mkdir(parents=True, exist_ok=True)
    t = cw.Tally()
    times: list[float] = []
    if "nodes" in args.sections:
        sec_nodes(t, scratch)
    if "workflow" in args.sections:
        sec_workflow(t, scratch)
    if "synthetic" in args.sections:
        sec_synthetic(t, scratch)
    if "preproc" in args.sections:
        sec_preproc(t, scratch, args.datasets, args.threads, times)
    bad = t.report()
    print("ALL OK" if not bad else f"DIFFERENCES: {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
