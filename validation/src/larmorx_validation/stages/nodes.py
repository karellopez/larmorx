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
import re
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
    argv, files = rn.prepare_inputs(s.run, node, node.argv)
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
    lx = rn.run_larmorx(s, node, argv, base / "larmorx", threads, files=files)
    res["larmorx_threads"] = threads
    res["larmorx_seconds"] = lx.seconds
    if not lx.ok:
        res.update(status="larmorx-error", note=lx.stderr.strip()[-300:])
        return res
    # what nipype did to the program's outputs (N4's header copy), done to every run's
    post = _NipypePost(s, node, argv, names)
    if post.active:
        res["nipype_copy_header"] = post.describe()
    lx_out = post(lx)
    assert lx_out is not None
    derivatives = s.run.derivatives()
    outputs = []
    for i, (ref, out) in enumerate(zip(refs, lx_out, strict=True)):
        d = cmp.compare_files(ref, out)
        entry: dict[str, Any] = {"output": _label(node.program, names, i), "vs_fmriprep": d}
        if post.active:
            # larmorx's own output (float32) against fMRIPrep's re-quantized one: nipype's share
            entry["raw_vs_fmriprep"] = cmp.compare_files(ref, lx.outputs[i])
            entry["raw_stored"] = cmp.stored_diff(ref, lx.outputs[i])
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
            s,
            node,
            argv,
            base / f"container-{node.threads}t",
            node.threads,
            timed=True,
            files=files,
        )
        res["container_seconds"] = cn.seconds
        cn_out = post(cn)
        if cn_out is not None:
            for entry, ref, out in zip(outputs, refs, cn_out, strict=True):
                entry["rerun_vs_fmriprep"] = cmp.compare_files(ref, out)
    if all(e["vs_fmriprep"].identical for e in outputs):
        res["status"] = "identical"
        return res

    # Take the difference apart.
    chain: list[tuple[str, list[Path] | None]] = [("fmriprep", refs)]
    if s.container:
        c1 = (
            cn
            if node.threads == 1
            else rn.run_container(s, node, argv, base / "container-1t", 1, files=files)
        )
        chain.append(("container-1t", post(c1)))
        cr = rn.run_container(
            s, node, argv, base / "container-1t-cr", 1, correctly_rounded=True, files=files
        )
        chain.append(("container-1t-cr", post(cr)))
    if s.ants_oracle and node.program in ANTS_PROGRAMS:
        r = rn.run_ants_oracle(s, argv, base / "ants-2.6.5-1t-cr", correctly_rounded=True)
        chain.append(("ants-2.6.5-1t-cr", post(r)))
        # ANTs 2.6.5 as built (glibc's maths), as the tool's own records compare
        a = rn.run_ants_oracle(s, argv, base / "ants-2.6.5-1t", correctly_rounded=False)
        if a is not None and a.ok:
            for entry, ants_out, out in zip(outputs, a.outputs, lx.outputs, strict=True):
                entry["vs_ants_as_built"] = cmp.compare_files(ants_out, out)
    chain.append(("larmorx", lx_out))
    for i, entry in enumerate(outputs):
        links = []
        previous = chain[0]
        for name, produced in chain[1:]:
            if produced is None:
                links.append({"from": previous[0], "to": name, "diff": None})
                continue
            prev_files = previous[1]
            assert prev_files is not None
            links.append(
                {
                    "from": previous[0],
                    "to": name,
                    "diff": cmp.compare_files(prev_files[i], produced[i]),
                }
            )
            previous = (name, produced)
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


_COPY_HEADER_SCRIPT = r'''
"""nipype 1.12.0's header copy after a program (CopyHeaderInterface._post_run_hook with the
interface's _copy_header_map), done on copies of a run's outputs, in fMRIPrep's image."""
import shutil
import sys

from nipype.utils.imagemanip import copy_header

source = sys.argv[1]
for spec in sys.argv[2:]:
    name, target, keep = spec.split(":")
    shutil.copyfile(name, target)
    copy_header(source, target, keep_dtype=keep == "1")
'''


class _NipypePost:
    """What nipype does to a program's outputs after the run, done again to every run's
    outputs, so that they compare with fMRIPrep's node output.

    nipype's ``N4BiasFieldCorrection`` with ``copy_header=True`` (niworkflows' brain
    extraction, sdcflows' field-map reference) copies the input's header onto the outputs
    (``nipype/interfaces/ants/segmentation.py``, ``_copy_header_map``): the corrected image
    **with the input's data type** (``keep_dtype=False``), the bias field with its own. When the
    input is an integer image (sdcflows' clipped field-map reference is int16), nibabel
    re-quantizes N4's float32 output to that type with a scale factor (PIPE-16). The copy runs
    nipype's own function in fMRIPrep's image on a copy of each output (``nipype-<name>``);
    runs whose input is float32 are left as they are (their data do not change)."""

    def __init__(self, s: rn.Settings, node: Node, argv: list[str], names: list[str]):
        self.s = s
        self.names = [os.path.basename(n) for n in names]
        self.source = rn._option(argv, ("-i", "--input-image")) or ""
        self.dtype = ""
        self.active = False
        if node.program == "N4BiasFieldCorrection" and node.inputs.get("copy_header") == "True":
            import nibabel as nib

            try:
                self.dtype = str(nib.load(s.run.host(self.source)).get_data_dtype())
            except (OSError, ValueError):
                self.dtype = ""
            self.active = bool(self.dtype) and self.dtype != "float32" and s.run.image is not None

    def describe(self) -> dict[str, Any]:
        return {"input": self.source, "input_dtype": self.dtype, "outputs": self.names}

    def __call__(self, run: rn.RunResult | None) -> list[Path] | None:
        if run is None or not run.ok:
            return None
        if not self.active:
            return run.outputs
        targets = [run.directory / f"nipype-{n}" for n in self.names]
        specs = [
            f"/cmp/{n}:/cmp/nipype-{n}:{'0' if i == 0 else '1'}" for i, n in enumerate(self.names)
        ]
        p = rn.run_container_python(
            self.s, run.directory, _COPY_HEADER_SCRIPT, [self.source, *specs]
        )
        if p.returncode != 0 or not all(t.is_file() for t in targets):
            raise RuntimeError(f"nipype's copy_header failed: {p.stderr[-300:]}")
        return targets


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
# Slice timing: 3dTshift, both implementations

#: 3dTshift's implementations in larmorx (docs/licensing.md): the bit-exact replica, a
#: translation of AFNI's source (built into larmorx since 2026-10-29; until then the
#: ``larmorx-gpl`` program), and the clean-room original, written from ``specs/3dTshift.md``.
TSHIFT_IMPLEMENTATIONS = ("replica", "original")

_NOTICE = re.compile(r"\b(?:AFNI|ANTS|ITK|FSL|PIPE|SCI)-\d+\b")


def tshift_within(d: cmp.ImageDiff, stored: dict[str, Any] | None = None) -> tuple[bool, str]:
    """docs/validation/afni-tshift.md's thresholds for one output: the same stored type and
    scale factor; float32 values within 1e-5 of the largest |AFNI output|; integers within 1
    stored unit (``stored``: :func:`~larmorx_validation.stages.compare.stored_diff`)."""
    if d.identical:
        return True, "bit-identical"
    if d.note:
        return False, d.note
    if stored and stored["dtype"][0] != stored["dtype"][1]:
        return False, f"stored types differ: {stored['dtype'][0]}, {stored['dtype'][1]}"
    if stored and "max_abs" in stored:
        same = stored["dtype"][0] == stored["dtype"][1] and np.array_equal(
            np.array(stored["scl_slope"]), np.array(stored["scl_slope"][::-1]), equal_nan=True
        )
        text = (
            f"{stored['dtype'][0]}: {stored['differing']:,} of {stored['values']:,} stored "
            f"values differ, max |diff| {stored['max_abs']} unit"
        )
        if not same:
            text += f"; types or scale factors differ ({stored['dtype']}, {stored['scl_slope']})"
        return same and stored["max_abs"] <= 1, text
    return d.max_rel <= 1e-5, f"max rel {d.max_rel:.2g}"


def compare_tshift(s: rn.Settings, node: Node, stage: Stage) -> dict[str, Any]:
    """3dTshift as fMRIPrep ran it, with both larmorx implementations in ``reproduce`` mode
    (fMRIPrep's run) and in their default (``fix``) mode; references: fMRIPrep's program
    again in its image, and the repository's AFNI 25.2.09 build."""
    res = _base(node, stage)
    argv, files = rn.prepare_inputs(s.run, node, node.argv)
    res["argv"] = argv
    res["files"] = files
    names = rn.output_names(argv)
    refs = rn.reference_outputs(s.run, node, names)
    if not names:
        res.update(status="error", note="cannot tell the outputs from the command line")
        return res
    if missing := [str(p) for p in refs if not p.exists()]:
        res.update(status="skipped", note=f"fMRIPrep's output is gone: {missing[0]}")
        return res
    ref = refs[0]
    base = s.compare / node.path
    threads = s.threads or node.threads
    res["larmorx_threads"] = threads
    outputs: list[dict[str, Any]] = []
    produced: dict[str, Path] = {}
    for impl in TSHIFT_IMPLEMENTATIONS:
        entry: dict[str, Any] = {"output": os.path.basename(names[0]), "implementation": impl}
        outputs.append(entry)
        try:
            r = rn.run_larmorx(
                s, node, argv, base / f"larmorx-{impl}", threads, implementation=impl, files=files
            )
        except FileNotFoundError as e:
            entry["error"] = str(e)
            continue
        res[f"larmorx_{impl}_seconds"] = r.seconds
        if not r.ok:
            entry["error"] = r.stderr.strip()[-300:]
            continue
        produced[impl] = r.outputs[0]
        entry["vs_fmriprep"] = cmp.compare_files(ref, r.outputs[0])
        entry["header_vs_fmriprep"] = cmp.header_diff(ref, r.outputs[0])
        entry["stored_vs_fmriprep"] = cmp.stored_diff(ref, r.outputs[0])
        entry["within"] = tshift_within(entry["vs_fmriprep"], entry["stored_vs_fmriprep"])
        # larmorx's default: AFNI's defects corrected (D13)
        f = rn.run_larmorx(
            s,
            node,
            argv,
            base / f"larmorx-{impl}-fix",
            threads,
            mode="fix",
            implementation=impl,
            files=files,
        )
        if f.ok:
            entry["fix"] = {
                "vs_reproduce": cmp.compare_files(r.outputs[0], f.outputs[0]),
                "header": cmp.header_diff(r.outputs[0], f.outputs[0]),
                "notices": sorted(set(_NOTICE.findall(f.stderr))),
            }
        else:
            entry["fix"] = {"error": f.stderr.strip()[-300:]}
    res["larmorx_seconds"] = res.get("larmorx_replica_seconds", res.get("larmorx_original_seconds"))
    res["outputs"] = outputs

    # fMRIPrep's program again, in its image, as it ran (one thread): the time on this machine
    # and whether AFNI reproduces fMRIPrep's own output.
    if s.container:
        cn = rn.run_container(
            s,
            node,
            argv,
            base / f"container-{node.threads}t",
            node.threads,
            timed=True,
            files=files,
        )
        res["container_seconds"] = cn.seconds if cn else None
        if cn is not None and cn.ok:
            res["rerun_vs_fmriprep"] = cmp.compare_files(ref, cn.outputs[0])
    # The repository's AFNI 25.2.09 build (the parity suites' oracle)
    a = rn.run_afni_oracle(s, argv, base / "afni-25.2.09", files)
    if a is not None and a.ok:
        res["afni_oracle_vs_fmriprep"] = cmp.compare_files(ref, a.outputs[0])
        res["afni_oracle_header_vs_fmriprep"] = cmp.header_diff(ref, a.outputs[0])
        for entry in outputs:
            if entry["implementation"] in produced:
                entry["vs_afni_oracle"] = cmp.compare_files(
                    a.outputs[0], produced[entry["implementation"]]
                )
    if "replica" in produced and "original" in produced:
        res["original_vs_replica"] = cmp.compare_files(produced["replica"], produced["original"])

    by = {e["implementation"]: e for e in outputs}
    rep, orig = by["replica"], by["original"]
    if "error" in rep or "error" in orig:
        res["status"] = "larmorx-error"
        res["note"] = rep.get("error") or orig.get("error")
    elif not rep["vs_fmriprep"].identical:
        afni = res.get("afni_oracle_vs_fmriprep")
        rep_afni = rep.get("vs_afni_oracle")
        # explained when the replica is AFNI's oracle build bit for bit and fMRIPrep's own
        # program differs from that build
        res["status"] = (
            "explained" if rep_afni is not None and rep_afni.identical and afni else "unexplained"
        )
    elif not orig["within"][0]:
        res["status"] = "unexplained"
    else:
        res["status"] = "identical" if orig["vs_fmriprep"].identical else "explained"
    return res


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
        # Where PLAN.md §11.3's FD correlation applies and larmorx misses it, mcflirt's own
        # band on this very run decides (porting guide §3, §7).
        if (
            res["fd_max_fmriprep"] >= HMC_FD_MIN_MM
            and res["confounds"]["larmorx"]["fd_r"] <= HMC_FD_R
        ):
            res["node_band"] = _node_band(s, node, base, source, reference, table)
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


#: PLAN.md §11.3's FD correlation, required where fMRIPrep's largest FD reaches 0.5 mm
#: (specs/mcflirt.md §14; record.FD_MIN_MM, record.FD_R).
HMC_FD_MIN_MM = 0.5
HMC_FD_R = 0.99

#: The perturbations of the node band: the threshold perturbations of the mri-hmc band
#: (``larmorx_validation.parity.mri_hmc_band``), which leave the geometry unchanged.
NODE_BAND_PERTURBATIONS = ("noise", "noise2", "noise3", "reverse")


def _node_band(
    s: rn.Settings, node: Node, base: Path, source: str, reference: str, table
) -> dict[str, Any]:
    """mcflirt's own variability on this node: FSL 6.0.7's mcflirt (which reproduces
    fMRIPrep's matrices bit for bit), black-box runs with fMRIPrep's arguments on the series
    perturbed as the mri-hmc band perturbs it (noise of ±0.5 of a stored unit, three
    realisations; the volumes reversed), and fMRIPrep's own confound code on each, against
    fMRIPrep's confounds file, as larmorx's matrices are compared."""
    import subprocess

    from larmorx_validation.parity.mri_hmc_band import SEED, _perturb

    exe = rn.host_mcflirt()
    if exe is None:
        return {"error": "FSL 6.0.7's mcflirt was not found"}
    env = {
        "HOME": os.environ.get("HOME", ""),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(exe.resolve().parents[1]),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        "OMP_NUM_THREADS": "1",
    }
    out: dict[str, Any] = {"mcflirt": rn.fsl_version(exe), "seed": SEED, "perturbations": {}}
    for kind in NODE_BAND_PERTURBATIONS:
        d = base / f"band-{kind}"
        if d.exists():
            shutil.rmtree(d)
        d.mkdir(parents=True)
        made = _perturb(kind, Path(s.run.host(source)), Path(s.run.host(reference)), d)
        if made is None:
            continue
        series, ref = made
        p = subprocess.run(
            [str(exe), "-in", str(series), "-out", str(d / "out_mcf.nii.gz")]
            + (["-reffile", str(ref)] if ref is not None else [])
            + ["-mats"],
            env=env,
            capture_output=True,
            text=True,
        )
        mats = d / "out_mcf.nii.gz.mat"
        if p.returncode != 0 or not mats.is_dir():
            out["perturbations"][kind] = {"error": p.stderr[-300:]}
            continue
        if kind == "reverse":
            # MAT_i of the reversed series is volume n-1-i
            names = sorted(mats.glob("MAT_*"), key=lambda q: int(q.name[4:]))
            ordered = d / "ordered.mat"
            ordered.mkdir()
            for i, q in enumerate(names):
                shutil.copyfile(q, ordered / f"MAT_{len(names) - 1 - i:04d}")
            mats = ordered
        conf = _confounds(s, d / "confounds", mats, source, reference)
        out["perturbations"][kind] = _confound_diff(table, conf["table"])
    fd = [v["fd_r"] for v in out["perturbations"].values() if "fd_r" in v]
    if fd:
        out["fd_r"] = min(fd)
    return out


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
        if _same_run(_bids_run(p.name), run):
            return p
    return None


def _hmc_xfm(s: rn.Settings, node: Node) -> Path | None:
    _, keys = normalise(node.path)
    for p in sorted((s.run.root / "out").rglob("*_desc-hmc_xfm.txt")):
        if _same_run(_bids_run(p.name), keys.get("run", "")):
            return p
    return None


def _bids_run(filename: str) -> str:
    """``sub-01_ses-1_task-x_run-01_desc-...`` -> ``ses_1_task_x_run_01``, as fMRIPrep names the
    run's workflow (``get_wf_name``: every entity but the subject; the session too)."""
    parts = [
        p
        for p in filename.split("_")
        if p.split("-")[0] in ("ses", "task", "acq", "ce", "rec", "dir", "run", "echo", "part")
    ]
    return "_".join(p.replace("-", "_") for p in parts)


def _same_run(derivative: str, workflow: str) -> bool:
    """A derivative's run key (:func:`_bids_run`) names the run of a workflow's key. A
    multi-echo run's workflow is named after its first echo (``..._echo_1``) while its
    derivatives combine the echoes and drop the entity, so the workflow's key is also tried
    without it."""
    return derivative == workflow or derivative == re.sub(r"_echo_\d+", "", workflow)


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
    if kwargs["fieldmap"] is not None and kwargs["pe_dir"] and kwargs["ro_time"]:
        res["fieldmap"] = i.get("fieldmap")
        res["jacobian_fix"] = _jacobian_fix_pass(
            s, i, transforms, kwargs, data, img.affine, threads
        )
    if s.container:
        res.update(_rerun_resample(s, node, out_file))
    return res


def _brain_mask(s: rn.Settings, shape: tuple[int, ...], affine: np.ndarray) -> tuple:
    """fMRIPrep's BOLD brain mask on this grid: the ``desc-brain_mask`` derivative of the run
    (any space) whose shape and affine are the grid's. ``(mask, its file)``, or ``(None, "")``."""
    import nibabel as nib

    for p in sorted((s.run.root / "out").rglob("*_desc-brain_mask.nii.gz")):
        if "/func/" not in p.as_posix():
            continue
        m = nib.load(p)
        if m.shape[:3] == tuple(shape[:3]) and np.allclose(m.affine, affine, atol=1e-4):
            return np.asanyarray(m.dataobj) > 0, os.path.relpath(p, s.run.root)
    return None, ""


def _relative_change(
    rep: np.ndarray, fix: np.ndarray, mask: np.ndarray | None, affine: np.ndarray
) -> dict[str, Any]:
    """How much larmorx's default (``fix``) changes fMRIPrep's output (``rep``) per brain
    voxel: the relative change of the voxel's temporal mean, ``(fix - rep) / |rep|`` (with
    the Jacobian, the ratio of the two factors), its distribution, and where it is largest
    (the largest clusters of voxels changed by more than 1 %, 6-connected, in world mm)."""
    from scipy import ndimage as ndi

    a = rep.mean(axis=3, dtype=np.float64) if rep.ndim == 4 else rep.astype(np.float64)
    b = fix.mean(axis=3, dtype=np.float64) if fix.ndim == 4 else fix.astype(np.float64)
    if mask is None:
        mask = a > 0.1 * a.max()
    valid = mask & (np.abs(a) > 0)
    rel = np.zeros(a.shape)
    rel[valid] = (b[valid] - a[valid]) / np.abs(a[valid])
    r = rel[valid]
    ar = np.abs(r)
    big = np.abs(rel) > 0.01
    labels, n = ndi.label(big)
    clusters = []
    if n:
        sizes = ndi.sum_labels(np.ones(a.shape), labels, index=np.arange(1, n + 1))
        for k in np.argsort(sizes)[::-1][:3]:
            idx = np.argwhere(labels == k + 1)
            centre = affine[:3, :3] @ idx.mean(axis=0) + affine[:3, 3]
            clusters.append(
                {
                    "voxels": int(sizes[k]),
                    "centroid_mm": [round(float(x), 1) for x in centre],
                    "max_abs": float(np.abs(rel[labels == k + 1]).max()),
                }
            )
    at = np.unravel_index(int(np.argmax(np.abs(rel))), rel.shape)
    return {
        "brain_voxels": int(valid.sum()),
        "over": {f"{t:g}": float(np.mean(ar > t)) for t in (1e-3, 1e-2, 5e-2, 0.2)},
        "abs_percentiles": {f"{p:g}": float(np.percentile(ar, p)) for p in (50, 90, 99, 99.9)},
        "max_abs": float(ar.max()),
        "signed_range": [float(r.min()), float(r.max())],
        "signed_median": float(np.median(r)),
        "max_at_mm": [round(float(x), 1) for x in affine[:3, :3] @ np.array(at) + affine[:3, 3]],
        "clusters_over_1pct": clusters,
        "clusters_count": int(n),
    }


def _jacobian_fix_pass(
    s: rn.Settings,
    i: dict[str, Any],
    transforms: list[str],
    kwargs: dict[str, Any],
    reproduced: np.ndarray,
    affine: np.ndarray,
    threads: int,
) -> dict[str, Any]:
    """PIPE-03 on fMRIPrep's own field map: larmorx's default (``fix``) against fMRIPrep's
    result (``reproduce``, compared with fMRIPrep's output above), and the two Jacobian factors
    themselves. Where fMRIPrep ran without the Jacobian (its default for field maps other than
    PEPOLAR, ``fmriprep/workflows/base.py``, ``_jacobian_for``), larmorx's default must leave
    the output unchanged, and both are run again with the Jacobian on, as fMRIPrep's
    ``--force fmap-jacobian`` would run them."""
    import warnings

    import nibabel as nib

    import larmorx as lx

    def resample(source, xfms, defects: str, **over) -> tuple[np.ndarray, list[str]]:
        kw = {**kwargs, **over}
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            out = lx.transforms.resample_series(
                source,
                s.run.host(i["ref_file"]),
                xfms,
                n_threads=threads,
                upstream_defects=defects,
                **kw,
            )
        notices = sorted({m for w in caught for m in _NOTICE.findall(str(w.message))})
        return np.asarray(out.data), notices

    out: dict[str, Any] = {"jacobian": bool(kwargs["jacobian"])}
    mask, mask_file = _brain_mask(s, reproduced.shape, affine)
    out["mask"] = mask_file or "temporal mean above 10 % of its maximum"
    source = s.run.host(i["in_file"])
    t0 = time.perf_counter()
    fixed, notices = resample(source, transforms, "fix")
    out["seconds"] = time.perf_counter() - t0
    out["notices"] = notices
    out["fix_vs_reproduce"] = cmp.compare_arrays(reproduced, fixed)
    if kwargs["jacobian"]:
        if not out["fix_vs_reproduce"].identical:
            out["change"] = _relative_change(reproduced, fixed, mask, affine)
        rep_forced = reproduced
    else:
        # the Jacobian forced on: fMRIPrep's factor (reproduce) against larmorx's (fix)
        del fixed
        rep_forced, _ = resample(source, transforms, "reproduce", jacobian=True)
        fix_forced, forced_notices = resample(source, transforms, "fix", jacobian=True)
        out["forced"] = {
            "notices": forced_notices,
            "fix_vs_reproduce": cmp.compare_arrays(rep_forced, fix_forced),
        }
        if not out["forced"]["fix_vs_reproduce"].identical:
            out["forced"]["change"] = _relative_change(rep_forced, fix_forced, mask, affine)
        del fix_forced
    del rep_forced
    # The factors themselves: a volume of ones on the source's grid, through the transforms
    # without head motion (both factors are computed before it), with the field map.
    src = nib.load(source)
    ones = (np.ones(src.shape[:3], np.float32), src.affine)
    keep = [k for k, t in enumerate(transforms) if "desc-hmc_xfm" not in os.path.basename(t)]
    inverse = kwargs["inverse"]
    if isinstance(inverse, list | tuple) and len(inverse) == len(transforms):
        inverse = [inverse[k] for k in keep]
    factors = {}
    for defects in ("reproduce", "fix"):
        factors[defects], _ = resample(
            ones,
            [transforms[k] for k in keep],
            defects,
            jacobian=True,
            inverse=inverse or False,
            order=1,
            mode="nearest",
        )
    inside = mask if mask is not None else np.ones(factors["fix"].shape, bool)
    fr, ff = (
        factors["reproduce"][inside].astype(np.float64),
        factors["fix"][inside].astype(np.float64),
    )
    gap = np.abs(ff - fr)
    out["factors"] = {
        "fmriprep_range": [float(fr.min()), float(fr.max())],
        "larmorx_range": [float(ff.min()), float(ff.max())],
        "over": {f"{t:g}": float(np.mean(gap > t)) for t in (0.001, 0.01, 0.05, 0.2)},
        "median_gap": float(np.median(gap)),
        "max_gap": float(gap.max()),
    }
    return out


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
    # an empty list (``inverse : []`` of a node without transforms) is left to the interface's
    # default: nipype turns an empty list given to a multi-object input into Undefined
    inputs = {
        k: v
        for k, v in ((k, _value(v)) for k, v in node.inputs.items())
        if v is not None and v != []
    }
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
        elif stage.kind == "tshift":
            res = compare_tshift(s, node, stage)
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


__all__ = [
    "compare_cli",
    "compare_hmc",
    "compare_node",
    "compare_resample",
    "compare_tshift",
    "explain",
    "tshift_within",
]
