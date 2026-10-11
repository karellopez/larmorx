# SPDX-License-Identifier: Apache-2.0
"""Comparing one node: larmorx on fMRIPrep's inputs and parameters, against fMRIPrep's output.

Three kinds of node:

- **cli**: a program larmorx runs with the original program's arguments (ANTs' programs,
  AFNI's 3dTshift). larmorx runs ``command.txt``'s arguments. Where its output is not
  bit-identical to fMRIPrep's, the difference is taken apart along a chain of reference runs,
  each differing from the previous one in a single respect:

  1. fMRIPrep's output, as it ran (its thread count);
  2. the same program in fMRIPrep's container at **one thread** (thread effects; with
     fMRIPrep's own thread count, re-run, it shows whether the program reproduces itself);
  3. the same, with **CORE-MATH**'s correctly rounded functions preloaded (glibc's maths;
     the image has the same glibc as this machine);
  4. the ANTs **2.6.5** oracle build, one thread, CORE-MATH preloaded (the version and the
     build: fMRIPrep 25.2.6 ships ANTs 2.6.2 from conda-forge; larmorx replicates 2.6.5's
     source, which the repository's oracle, ``scripts/build_ants_oracle.sh``, honours);
  5. larmorx, which must be bit-identical to 4.

  ANTs 2.6.5 as built (glibc's maths) is run too, for the headline of the tool's own records.

- **hmc**: ``mcflirt``, replaced by ``larmorx mri hmc`` with the same arguments; the
  matrices, the ITK transforms fMRIPrep derives from them and the motion confounds
  (fMRIPrep's own code, run in its container on larmorx's matrices) are compared.
- **resample**: fMRIPrep's ``ResampleSeries`` (Python), replaced by
  ``lx.transforms.resample_series`` with the node's inputs.
"""

from __future__ import annotations

import ast
import os
import shutil
import time
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.stages import compare as cmp
from larmorx_validation.stages import runners as rn
from larmorx_validation.stages.catalogue import Stage, normalise
from larmorx_validation.stages.oracle import Node

ANTS_PROGRAMS = {
    "antsApplyTransforms",
    "DenoiseImage",
    "ImageMath",
    "MultiplyImages",
    "N4BiasFieldCorrection",
    "ResampleImage",
    "ResampleImageBySpacing",
    "SmoothImage",
    "ThresholdImage",
}


def _base(node: Node, stage: Stage) -> dict[str, Any]:
    norm, keys = normalise(node.path)
    return {
        "stage": stage.id,
        "node": node.path,
        "name": norm.split("/")[-1],
        "keys": keys,
        "program": node.program or node.interface,
        "fmriprep_threads": node.threads,
        "fmriprep_seconds": node.duration,
        "command": node.command,
    }


def _label(program: str | None, outputs: list[str], i: int) -> str:
    if program == "N4BiasFieldCorrection" and len(outputs) == 2:
        return ("corrected", "bias field")[i]
    return os.path.basename(outputs[i])


# ------------------------------------------------------------------------------------------------
# Programs run with their own arguments


def compare_cli(s: rn.Settings, node: Node, stage: Stage) -> dict[str, Any]:
    res = _base(node, stage)
    argv = node.argv
    names = rn.output_names(argv)
    refs = rn.reference_outputs(s.run, node, names)
    if not names:
        res.update(status="error", note="cannot tell the outputs from the command line")
        return res
    if missing := [str(p) for p in refs if not p.exists()]:
        res.update(status="skipped", note=f"fMRIPrep's output is gone: {missing[0]}")
        return res
    base = s.compare / node.path
    threads = s.threads or node.threads
    lx = rn.run_larmorx(s, node, argv, base / "larmorx", threads)
    res["larmorx_threads"] = threads
    res["larmorx_seconds"] = lx.seconds
    if not lx.ok:
        res.update(status="larmorx-error", note=lx.stderr.strip()[-300:])
        return res
    derivatives = s.run.derivatives()
    outputs = []
    for i, (ref, out) in enumerate(zip(refs, lx.outputs, strict=True)):
        d = cmp.compare_files(ref, out)
        entry: dict[str, Any] = {"output": _label(node.program, names, i), "vs_fmriprep": d}
        if (deriv := derivatives.get(str(ref))) and Path(deriv).is_file():
            entry["derivative"] = os.path.relpath(deriv, s.run.root)
            entry["vs_derivative"] = cmp.compare_files(deriv, out)
        outputs.append(entry)
    res["outputs"] = outputs

    # fMRIPrep's program again, in its container, with fMRIPrep's thread count: the time on
    # this machine, and whether the program reproduces fMRIPrep's own output.
    cn = None
    if s.container:
        cn = rn.run_container(
            s, node, argv, base / f"container-{node.threads}t", node.threads, timed=True
        )
        res["container_seconds"] = cn.seconds
        if cn.ok:
            for entry, ref, out in zip(outputs, refs, cn.outputs, strict=True):
                entry["rerun_vs_fmriprep"] = cmp.compare_files(ref, out)
    if all(e["vs_fmriprep"].identical for e in outputs):
        res["status"] = "identical"
        return res

    # Take the difference apart.
    chain: list[tuple[str, list[Path] | None]] = [("fmriprep", refs)]
    if s.container:
        c1 = cn if node.threads == 1 else rn.run_container(s, node, argv, base / "container-1t", 1)
        chain.append(("container-1t", c1.outputs if c1 and c1.ok else None))
        cr = rn.run_container(s, node, argv, base / "container-1t-cr", 1, correctly_rounded=True)
        chain.append(("container-1t-cr", cr.outputs if cr and cr.ok else None))
    if s.ants_oracle and node.program in ANTS_PROGRAMS:
        r = rn.run_ants_oracle(s, argv, base / "ants-2.6.5-1t-cr", correctly_rounded=True)
        chain.append(("ants-2.6.5-1t-cr", r.outputs if r and r.ok else None))
        # ANTs 2.6.5 as built (glibc's maths), as the tool's own records compare
        a = rn.run_ants_oracle(s, argv, base / "ants-2.6.5-1t", correctly_rounded=False)
        if a is not None and a.ok:
            for entry, ants_out, out in zip(outputs, a.outputs, lx.outputs, strict=True):
                entry["vs_ants_as_built"] = cmp.compare_files(ants_out, out)
    chain.append(("larmorx", lx.outputs))
    for i, entry in enumerate(outputs):
        links = []
        previous = chain[0]
        for name, files in chain[1:]:
            if files is None:
                links.append({"from": previous[0], "to": name, "diff": None})
                continue
            prev_files = previous[1]
            assert prev_files is not None
            links.append(
                {
                    "from": previous[0],
                    "to": name,
                    "diff": cmp.compare_files(prev_files[i], files[i]),
                }
            )
            previous = (name, files)
        entry["chain"] = links
        entry["cause"] = explain(links, node.threads, entry.get("rerun_vs_fmriprep"))
    _check_float_pipeline(s, node, argv, base, outputs, lx.outputs)
    if all(e["vs_fmriprep"].identical or e["cause"]["explained"] for e in outputs):
        res["status"] = "explained"
    elif all(
        e["vs_fmriprep"].identical or e["cause"]["explained"] or e["cause"].get("known")
        for e in outputs
    ):
        res["status"] = "known-divergence"
    else:
        res["status"] = "unexplained"
    return res


#: larmorx's documented divergences, by the cause :func:`_check_float_pipeline` confirms.
KNOWN = {
    "float pipeline": (
        "larmorx computed `antsApplyTransforms --float 1` in double precision (rounding the "
        "input and output to float32), where ANTs runs its whole pipeline, transforms and "
        "interpolator included, in single precision. Since 2026-10-11 larmorx computes it in "
        "single precision as ANTs does (docs/findings/ants-float.md), so this check should "
        "no longer find anything"
    ),
}


def _check_float_pipeline(
    s: rn.Settings,
    node: Node,
    argv: list[str],
    base: Path,
    outputs: list[dict[str, Any]],
    lx_outputs: list[Path],
) -> None:
    """For ``antsApplyTransforms --float 1`` outputs that larmorx does not reproduce: run the
    ANTs 2.6.5 reference (correctly rounded maths) in double precision (``--float 0``) and
    round its output to float32. If that is larmorx's output, bit for bit, larmorx computed in
    double (what it did before 2026-10-11, when it learnt ANTs' single-precision pipeline)."""
    if node.program != "antsApplyTransforms" or not s.ants_oracle:
        return
    try:
        i = argv.index("--float")
    except ValueError:
        return
    if i + 1 >= len(argv) or argv[i + 1] != "1":
        return
    pending = [e for e in outputs if not e["vs_fmriprep"].identical and not e["cause"]["explained"]]
    if not pending:
        return
    double = list(argv)
    double[i + 1] = "0"
    r = rn.run_ants_oracle(s, double, base / "ants-2.6.5-1t-cr-double", correctly_rounded=True)
    if r is None or not r.ok:
        return
    for entry, ants_out, lx_out in zip(outputs, r.outputs, lx_outputs, strict=True):
        if entry not in pending:
            continue
        a, aa, _ = cmp.load(ants_out)
        b, ab, _ = cmp.load(lx_out)
        d = cmp.compare_arrays(
            np.asarray(a, dtype=np.float32), b, affine_diff=float(np.abs(aa - ab).max())
        )
        entry["double_check"] = d
        if d.identical:
            entry["cause"]["causes"] = [c for c in entry["cause"]["causes"] if c != "larmorx"] + [
                "float pipeline"
            ]
            entry["cause"]["known"] = "float pipeline"


#: What each link of the chain changes.
LINK_CAUSES = {
    "container-1t": "threads",
    "container-1t-cr": "glibc maths",
    "ants-2.6.5-1t-cr": "version",
    "larmorx": "larmorx",
}


def explain(
    links: list[dict[str, Any]], threads: int, rerun: cmp.ImageDiff | None
) -> dict[str, Any]:
    """Which links of the chain change the output, and whether that explains larmorx's
    difference from fMRIPrep (it does when larmorx equals the last reference)."""
    causes = []
    for link in links:
        d = link["diff"]
        if d is None or d.identical:
            continue
        cause = LINK_CAUSES.get(link["to"], link["to"])
        if cause == "threads" and threads == 1:
            cause = "run-to-run"
        causes.append(cause)
    last = links[-1]["diff"] if links else None
    compared = [link for link in links if link["diff"] is not None]
    reached = bool(compared) and compared[-1]["to"] == "larmorx"
    explained = last is not None and last.identical and len(compared) == len(links)
    if rerun is not None and not rerun.identical:
        causes.insert(0, "run-to-run")
    return {
        "causes": causes,
        "explained": explained,
        "complete": reached and len(compared) == len(links),
    }


# ------------------------------------------------------------------------------------------------
# Head-motion correction


_HMC_SCRIPT = r'''
"""fMRIPrep 25.2.6's own code on a set of mcflirt-style matrices: the ITK transforms
(niworkflows' MCFLIRT2ITK) and the motion confounds (FSLMotionParams, FramewiseDisplacement,
FSLRMSDeviation), exactly as fMRIPrep's HMC and confounds workflows compute them."""
import json
import os
import sys
from pathlib import Path

mats_dir, source, reference = sys.argv[1:4]
from niworkflows.interfaces.itk import MCFLIRT2ITK
from fmriprep.interfaces.confounds import (
    FramewiseDisplacement,
    FSLMotionParams,
    FSLRMSDeviation,
)

mats = sorted(Path(mats_dir).glob("MAT_*"), key=lambda p: int(p.name[4:]))
root = Path.cwd()
out = {}


def run(name, interface):
    # each interface in its own directory, as nipype runs each node (FSLMotionParams and
    # FSLRMSDeviation write files of the same name)
    d = root / name
    d.mkdir()
    os.chdir(d)
    try:
        return interface.run().outputs.out_file
    finally:
        os.chdir(root)


out["itk"] = run(
    "fsl2itk", MCFLIRT2ITK(in_files=[str(m) for m in mats], in_source=source, in_reference=reference)
)
out["motion"] = run("motion_params", FSLMotionParams(xfm_file=out["itk"], boldref_file=reference))
out["fd"] = run("fdisp", FramewiseDisplacement(in_file=out["motion"]))
out["rmsd"] = run("rmsd", FSLRMSDeviation(xfm_file=out["itk"], boldref_file=reference))
(root / "confounds.json").write_text(json.dumps(out))
'''


def _confounds(s: rn.Settings, directory: Path, mats: Path, source: str, reference: str) -> dict:
    """fMRIPrep's ITK transforms and motion confounds from ``mats``, computed in its image."""
    if directory.exists():
        shutil.rmtree(directory)
    directory.mkdir(parents=True)
    shutil.copytree(mats, directory / "mats")
    p = rn.run_container_python(s, directory, _HMC_SCRIPT, ["/cmp/mats", source, reference])
    result = directory / "confounds.json"
    if p.returncode != 0 or not result.is_file():
        raise RuntimeError(f"fMRIPrep's confound code failed: {p.stderr[-500:]}")
    import json

    files = json.loads(result.read_text())

    def host(path: str) -> Path:
        return directory / os.path.relpath(path, "/cmp")

    import pandas as pd

    motion = pd.read_csv(host(files["motion"]), sep="\t")
    motion["framewise_displacement"] = pd.read_csv(host(files["fd"]), sep="\t").iloc[:, 0]
    motion["rmsd"] = pd.read_csv(host(files["rmsd"]), sep="\t").iloc[:, 0]
    return {"itk": host(files["itk"]), "table": motion}


MOTION_COLUMNS = ("trans_x", "trans_y", "trans_z", "rot_x", "rot_y", "rot_z")


def _confound_diff(ref, test) -> dict[str, float]:
    """Motion parameters (mm, degrees), FD and RMSD of two confound tables."""
    out: dict[str, float] = {}
    trans = np.abs(
        ref[list(MOTION_COLUMNS[:3])].to_numpy() - test[list(MOTION_COLUMNS[:3])].to_numpy()
    )
    rot = np.degrees(
        np.abs(ref[list(MOTION_COLUMNS[3:])].to_numpy() - test[list(MOTION_COLUMNS[3:])].to_numpy())
    )
    out["median_mm"] = float(np.median(trans))
    out["max_mm"] = float(trans.max())
    out["median_deg"] = float(np.median(rot))
    out["max_deg"] = float(rot.max())
    fa = ref["framewise_displacement"].to_numpy(dtype=float)[1:]
    fb = test["framewise_displacement"].to_numpy(dtype=float)[1:]
    out["fd_r"] = float(np.corrcoef(fa, fb)[0, 1]) if fa.std() > 0 and fb.std() > 0 else 1.0
    out["fd_max_diff"] = float(np.abs(fa - fb).max())
    out["fd_mean_ref"] = float(fa.mean())
    out["fd_mean_test"] = float(fb.mean())
    ra = ref["rmsd"].to_numpy(dtype=float)[1:]
    rb = test["rmsd"].to_numpy(dtype=float)[1:]
    out["rmsd_max_diff"] = float(np.abs(ra - rb).max())
    out["identical"] = bool(
        np.array_equal(ref[list(MOTION_COLUMNS)].to_numpy(), test[list(MOTION_COLUMNS)].to_numpy())
    )
    return out


def _fov_centre(path: str) -> np.ndarray:
    import nibabel as nib

    h = nib.load(path).header
    shape = np.array(h.get_data_shape()[:3], dtype=float)
    zooms = np.abs(np.array(h["pixdim"][1:4], dtype=float))
    return 0.5 * (shape - 1) * zooms


def compare_hmc(s: rn.Settings, node: Node, stage: Stage) -> dict[str, Any]:
    import pandas as pd

    import larmorx as lx

    res = _base(node, stage)
    argv = node.argv
    out_name = rn.output_names(argv)[0]
    ref_mats = Path(s.run.host(out_name) + ".mat")
    if not ref_mats.is_dir():
        res.update(status="skipped", note=f"mcflirt's matrices are gone: {ref_mats}")
        return res
    source = rn._option(argv, ("-in",)) or ""
    reference = rn._option(argv, ("-reffile", "-r")) or ""
    base = s.compare / node.path
    threads = s.threads or node.threads
    lx_run = rn.run_larmorx(s, node, argv, base / "larmorx", threads)
    res["larmorx_threads"] = threads
    res["larmorx_seconds"] = lx_run.seconds
    if not lx_run.ok:
        res.update(status="larmorx-error", note=lx_run.stderr.strip()[-300:])
        return res
    lx_mats = base / "larmorx" / (os.path.basename(out_name) + ".mat")
    centre = _fov_centre(s.run.host(source))
    f_m = cmp.read_mats(ref_mats)
    l_m = cmp.read_mats(lx_mats)
    res["matrices"] = cmp.motion_diff(np.array(f_m), np.array(l_m), centre)

    # What fMRIPrep needs, through the Python API: the matrices only, no resampling.
    t0 = time.perf_counter()
    # D13: fMRIPrep as it ran, so mcflirt's defects reproduced.
    rep = {"upstream_defects": "reproduce"}
    api = lx.mri.hmc(
        s.run.host(source),
        reference=s.run.host(reference),
        resample=False,
        n_threads=threads,
        **rep,
    )
    res["api_seconds"] = time.perf_counter() - t0
    res["api_threads"] = threads
    t0 = time.perf_counter()
    lx.mri.hmc(
        s.run.host(source), reference=s.run.host(reference), resample=False, n_threads=0, **rep
    )
    res["api_seconds_all"] = time.perf_counter() - t0
    res["api_threads_all"] = os.cpu_count()

    # fMRIPrep's transforms and confounds.
    tsv = _confounds_tsv(s, node)
    if s.container and tsv is not None:
        table = pd.read_csv(tsv, sep="\t")
        sanity = _confounds(s, base / "confounds-fmriprep", ref_mats, source, reference)
        mine = _confounds(s, base / "confounds-larmorx", lx_mats, source, reference)
        hmc_xfm = _hmc_xfm(s, node)
        res["confounds"] = {
            "tsv": os.path.relpath(tsv, s.run.root),
            "check": _confound_diff(table, sanity["table"]),
            "larmorx": _confound_diff(table, mine["table"]),
        }
        res["fd_max_fmriprep"] = float(
            table["framewise_displacement"].to_numpy(dtype=float)[1:].max()
        )
        band = _mcflirt_band(s)
        if band:
            res["band"] = band
        if hmc_xfm is not None:
            fx = cmp.read_itk_affines(hmc_xfm)
            res["itk"] = {
                "derivative": os.path.relpath(hmc_xfm, s.run.root),
                "check_identical": bool(np.array_equal(fx, cmp.read_itk_affines(sanity["itk"]))),
                "larmorx_max_abs": float(np.abs(fx - cmp.read_itk_affines(mine["itk"])).max()),
                # larmorx's own ITK output against nitransforms' conversion of its matrices
                "own_vs_nitransforms_max_abs": float(
                    np.abs(_itk_from_api(api) - cmp.read_itk_affines(mine["itk"])).max()
                ),
            }
    # References: mcflirt again in fMRIPrep's image (reproducibility, time on this machine),
    # and FSL 6.0.7's mcflirt on this machine (another FSL build).
    if s.container:
        cn = rn.run_container(s, node, argv, base / "container-1t", 1, timed=True)
        res["container_seconds"] = cn.seconds
        c_mats = base / "container-1t" / (os.path.basename(out_name) + ".mat")
        if cn.code == 0 and c_mats.is_dir():
            res["rerun"] = cmp.motion_diff(np.array(f_m), np.array(cmp.read_mats(c_mats)), centre)
    h = rn.run_host_mcflirt(s, argv, base / "fsl-6.0.7")
    if h is not None and h.code == 0 and h.outputs[0].is_dir():
        res["host_fsl"] = cmp.motion_diff(
            np.array(f_m), np.array(cmp.read_mats(h.outputs[0])), centre
        )
        res["host_fsl"]["version"] = rn.fsl_version(rn.host_mcflirt())
        res["host_fsl"]["larmorx_vs"] = cmp.motion_diff(
            np.array(cmp.read_mats(h.outputs[0])), np.array(l_m), centre
        )
    res["status"] = "compared"
    return res


def _mcflirt_band(s: rn.Settings) -> dict[str, float] | None:
    """mcflirt's own variability on this dataset with fMRIPrep's command, from the mri-hmc
    band runs (``larmorx_validation.parity.mri_hmc_band``), if there are any."""
    from larmorx_validation.parity.mri_hmc_band import band_for

    dataset = Path(s.run.info.get("BIDS input", "")).name
    if not dataset:
        return None
    try:
        return band_for(f"real/{dataset}/fmriprep")
    except (OSError, ValueError, KeyError):
        return None


def _itk_from_api(result) -> np.ndarray:
    return np.asarray(result.itk)


def _confounds_tsv(s: rn.Settings, node: Node) -> Path | None:
    _, keys = normalise(node.path)
    run = keys.get("run", "")
    for p in sorted((s.run.root / "out").rglob("*_desc-confounds_timeseries.tsv")):
        if _bids_run(p.name) == run:
            return p
    return None


def _hmc_xfm(s: rn.Settings, node: Node) -> Path | None:
    _, keys = normalise(node.path)
    for p in sorted((s.run.root / "out").rglob("*_desc-hmc_xfm.txt")):
        if _bids_run(p.name) == keys.get("run", ""):
            return p
    return None


def _bids_run(filename: str) -> str:
    """``sub-01_task-x_run-01_desc-...`` -> ``task_x_run_01`` (as nipype names the workflow)."""
    parts = [
        p
        for p in filename.split("_")
        if p.split("-")[0] in ("task", "acq", "ce", "dir", "rec", "run", "echo", "part", "ses")
    ]
    return "_".join(p.replace("-", "_") for p in parts if not p.startswith("ses-"))


# ------------------------------------------------------------------------------------------------
# One-shot resampling


def _value(text: str | None) -> Any:
    if text is None or text == "<undefined>":
        return None
    try:
        return ast.literal_eval(text)
    except (ValueError, SyntaxError):
        return text


def compare_resample(s: rn.Settings, node: Node, stage: Stage) -> dict[str, Any]:
    import larmorx as lx

    res = _base(node, stage)
    i = {k: _value(v) for k, v in node.inputs.items()}
    out_file = node.outputs.get("out_file")
    if not out_file or not Path(s.run.host(out_file)).is_file():
        res.update(status="skipped", note="fMRIPrep's output is gone")
        return res
    threads = s.threads or node.threads
    transforms = [s.run.host(t) for t in (i.get("transforms") or [])]
    kwargs = dict(
        inverse=i.get("inverse") or False,
        fieldmap=s.run.host(i["fieldmap"]) if i.get("fieldmap") else None,
        pe_dir=i.get("pe_dir"),
        ro_time=i.get("ro_time"),
        jacobian=bool(i.get("jacobian", True)),
        order=int(i.get("order", 3)),
        mode=str(i.get("mode", "grid-constant")),
        cval=float(i.get("cval", 0.0)),
        prefilter=bool(i.get("prefilter", True)),
        output_dtype=str(i.get("output_data_type", "float32")),
    )
    res["inputs"] = {
        "in_file": i.get("in_file"),
        "ref_file": i.get("ref_file"),
        "transforms": i.get("transforms"),
        **{k: v for k, v in kwargs.items() if k != "fieldmap"},
    }
    t0 = time.perf_counter()
    # D13: fMRIPrep as it ran, so its Jacobian axis (PIPE-03) and nitransforms' composite
    # order (PIPE-04) reproduced.
    img = lx.transforms.resample_series(
        s.run.host(i["in_file"]),
        s.run.host(i["ref_file"]),
        transforms,
        n_threads=threads,
        upstream_defects="reproduce",
        **kwargs,
    )
    res["larmorx_seconds"] = time.perf_counter() - t0
    res["larmorx_threads"] = threads
    # fMRIPrep's node also writes its output (gzip); so does larmorx here, for the same work.
    directory = s.compare / node.path / "larmorx"
    directory.mkdir(parents=True, exist_ok=True)
    target = directory / os.path.basename(s.run.host(out_file))
    t0 = time.perf_counter()
    lx.save(img, target, n_threads=threads)
    res["larmorx_write_seconds"] = time.perf_counter() - t0

    import nibabel as nib

    ref_img = nib.load(s.run.host(out_file))
    ref = np.asanyarray(ref_img.dataobj)
    data = np.asarray(img.data)
    d = cmp.compare_arrays(ref, data, affine_diff=float(np.abs(ref_img.affine - img.affine).max()))
    entry: dict[str, Any] = {"output": "preprocessed BOLD", "vs_fmriprep": d}
    deriv = s.run.derivatives().get(s.run.host(out_file))
    if deriv and Path(deriv).is_file():
        entry["derivative"] = os.path.relpath(deriv, s.run.root)
        dimg = nib.load(deriv)
        entry["vs_derivative"] = cmp.compare_arrays(
            np.asanyarray(dimg.dataobj),
            data,
            affine_diff=float(np.abs(dimg.affine - img.affine).max()),
        )
        entry["derivative_dtype"] = str(dimg.get_data_dtype())
    del ref
    res["outputs"] = [entry]
    res["status"] = "identical" if d.identical else "different"
    if s.container:
        res.update(_rerun_resample(s, node, out_file))
    return res


_RESAMPLE_SCRIPT = r'''
"""fMRIPrep 25.2.6's ResampleSeries again, with the node's inputs (timing on this machine)."""
import json
import sys
import time

from fmriprep.interfaces.resampling import ResampleSeries

inputs = json.loads(sys.argv[1])
t0 = time.perf_counter()
result = ResampleSeries(**inputs).run()
seconds = time.perf_counter() - t0
open("rerun.json", "w").write(json.dumps({"seconds": seconds, "out_file": result.outputs.out_file}))
'''


def _rerun_resample(s: rn.Settings, node: Node, out_file: str) -> dict[str, Any]:
    """fMRIPrep's ``ResampleSeries`` again in its image, with the node's inputs and thread
    count: its time on this machine, and whether it reproduces its own output (timed, so
    never reused)."""
    import json

    directory = s.compare / node.path / f"container-{node.threads}t"
    if directory.exists():
        shutil.rmtree(directory)
    directory.mkdir(parents=True)
    inputs = {k: v for k, v in ((k, _value(v)) for k, v in node.inputs.items()) if v is not None}
    # The node's own num_threads (in ``inputs``), and OMP_NUM_THREADS=1 as fMRIPrep's image
    # sets it: with more, the concurrent volumes' OpenBLAS calls race and corrupt the output
    # (docs/findings/fmriprep-oracle.md).
    p = rn.run_container_python(s, directory, _RESAMPLE_SCRIPT, [json.dumps(inputs)], threads=1)
    record = directory / "rerun.json"
    if p.returncode != 0 or not record.is_file():
        return {"container_error": p.stderr[-300:]}
    rec = json.loads(record.read_text())
    out: dict[str, Any] = {"container_seconds": rec["seconds"]}
    produced = directory / os.path.relpath(rec["out_file"], "/cmp")
    if produced.is_file():
        out["rerun_identical"] = cmp.compare_files(s.run.host(out_file), produced).identical
        produced.unlink()  # a copy of fMRIPrep's output (up to 1 GB); only its time matters
    return out


def compare_node(s: rn.Settings, node: Node, stage: Stage) -> dict[str, Any]:
    """Compare one node; a failure of the harness itself is recorded, not raised."""
    t0 = time.perf_counter()
    try:
        if stage.kind == "hmc":
            res = compare_hmc(s, node, stage)
        elif stage.kind == "resample":
            res = compare_resample(s, node, stage)
        else:
            res = compare_cli(s, node, stage)
    except Exception as e:
        import traceback

        res = _base(node, stage)
        res.update(
            status="harness-error",
            note=f"{type(e).__name__}: {e}",
            trace=traceback.format_exc()[-1500:],
        )
    res["harness_seconds"] = time.perf_counter() - t0
    return res


__all__ = ["compare_cli", "compare_hmc", "compare_node", "compare_resample", "explain"]
