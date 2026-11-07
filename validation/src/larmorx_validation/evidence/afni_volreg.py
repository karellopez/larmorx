# SPDX-License-Identifier: Apache-2.0
"""3dvolreg's evidence: fMRIPrep's real call (niworkflows' ``RobustAverage``: ``-Fourier
-twopass -zpad 4``, the parameters, matrices and maximum displacements written) on the BOLD
reference volumes of OpenNeuro ds000005 sub-01's first run, given to AFNI 26.3.05 (the
replica's reference), AFNI 25.2.09 (the version step), fMRIPrep's build of AFNI in its image
and the larmorx replica (default mode at 1 and 6 threads, ``reproduce``): the raw series, the
volumes 3dvolreg is given, the registered series and its motion parameters; RobustAverage's
median of the registered volumes beside fMRIPrep's own BOLD reference of the run; AFNI-99 (the
maximum displacements of a mirrored position) on the same volumes stored a second way; the
record's cases; informal timings.

Only the replica exists so far: the clean-room original's panels come with its part. The
cache entry runs the programs directly (AFNI by its oracle builds, fMRIPrep's AFNI through
Apptainer, the replica through ``python -m larmorx`` with ``LARMORX_IMPLEMENTATION=replica``);
the volumes given to 3dvolreg are the parity suite's (``parity.afni_volreg._sliced``: re-made by
fMRIPrep's own Python in its image).
"""

from __future__ import annotations

import copy
import gzip
import re
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import afni_automask as am
from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "afni-volreg"
PROGRAM = "3dvolreg"
LABEL = "ds000005-run-01"  # the parity suite's fMRIPrep call (parity.afni_volreg.CALLS)
CASE = "fmriprep-ds000005-run-01"
RUN = "ds000005-sub-01-noreconall"
BOLD = "openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz"
MASK = "out/sub-01/func/sub-01_task-mixedgamblestask_run-01_desc-brain_mask.nii.gz"
DATA = common.DS000005
#: RobustAverage's 3dvolreg call, as the record's suite gives it (``{out}``: a run's stem).
CALL = ["-Fourier", "-twopass", "-zpad", "4", "-prefix", "{out}.nii.gz", "-1Dfile", "{out}.1D",
        "-1Dmatrix_save", "{out}.aff12.1D", "-maxdisp1D", "{out}_md.1D"]  # fmt: skip
#: The runs: label, program (afni, image: fMRIPrep's AFNI, replica), AFNI version, D13 mode,
#: thread count. ``lps-``: the same volumes stored with their second axis reversed (AFNI-99).
RUNNERS = [
    ("afni26", "afni", "26.3.05", None, "1"),
    ("afni25", "afni", "25.2.09", None, "1"),
    ("image", "image", None, None, "1"),
    ("replica-1", "replica", None, "fix", "1"),
    ("replica-6", "replica", None, "fix", "6"),
    ("replica-reproduce", "replica", None, "reproduce", "1"),
    ("lps-afni26", "afni", "26.3.05", None, "1"),
    ("lps-replica", "replica", None, "fix", "1"),
    ("lps-replica-reproduce", "replica", None, "reproduce", "1"),
]
#: The series whose RobustAverage median is computed (``<label>-average.nii.gz``).
AVERAGED = ("afni26", "afni25", "image", "replica-1", "replica-reproduce")
LPS_INPUT = "input-lps.nii.gz"


# --------------------------------------------------------------------- running the programs


def _run_all(e: fcache.Entry, sliced: Path, lps: Path) -> None:
    from larmorx_validation.parity import afni_replica as ar
    from larmorx_validation.parity import afni_versions

    add, unset = am.environment()
    home = e.dir / ".home"
    home.mkdir(exist_ok=True)
    sif = ar.fmriprep_image()
    for label, kind, version, mode, threads in RUNNERS:
        inp = lps if label.startswith("lps-") else sliced
        argv = [a.replace("{out}", str(e.dir / label)) for a in CALL] + [str(inp)]
        env = {**add, "HOME": str(home), "OMP_NUM_THREADS": threads}
        if kind == "afni":
            exe = afni_versions.binary(PROGRAM, version)
            if exe is None:
                raise FileNotFoundError(f"AFNI {version}'s {PROGRAM} (oracles/afni-{version})")
            e.run(label, [exe, *argv], env=env, unset=unset)
        elif kind == "image":
            if sif is None:
                raise FileNotFoundError("fMRIPrep's image (or apptainer)")
            cmd = ["apptainer", "exec", "--cleanenv", "--no-home"]
            for d in sorted({str(e.dir), str(inp.parent)}):
                cmd += ["--bind", f"{d}:{d}"]
            for k, v in env.items():
                cmd += ["--env", f"{k}={v}"]
            e.run(label, [*cmd, str(sif), PROGRAM, *argv], unset=unset)
        else:
            env["LARMORX_UPSTREAM_DEFECTS"] = mode
            env.update(ar.REPLICA_ENV)
            e.run(label, [*ar.replica_command(), "afni", PROGRAM, *argv], env=env, unset=unset)


def _store_lps(src: Path, out: Path) -> None:
    """The same volumes stored with the second axis reversed (posterior to anterior becomes
    anterior to posterior: LAS → LPS, every axis then positive in AFNI's DICOM order): the
    stored integers, the scale factor and every other header field as they are, only the
    voxel order and the qform/sform changed to keep every voxel where it was."""
    import nibabel as nib

    img = nib.load(str(src))
    if img.header.extensions:
        raise AssertionError(f"{src}: an extension (the sliced volumes have none)")
    raw = np.asanyarray(img.dataobj.get_unscaled())
    flip = np.eye(4)
    flip[1, 1], flip[1, 3] = -1.0, raw.shape[1] - 1
    aff = img.affine @ flip
    # the header as the file stores it (nibabel's loaded header has its scale factor and
    # data offset reset: they live in the data object)
    with gzip.open(src, "rb") as f:
        hdr = nib.Nifti1Header.from_fileobj(f)
    hdr.set_qform(aff, code=int(img.header["qform_code"]))
    hdr.set_sform(aff, code=int(img.header["sform_code"]))
    off = int(hdr["vox_offset"])
    data = np.ascontiguousarray(raw[:, ::-1]).astype(hdr.get_data_dtype())
    with open(out, "wb") as raw_file, gzip.GzipFile(fileobj=raw_file, mode="wb", mtime=0) as f:
        f.write(hdr.binaryblock + b"\0" * (off - len(hdr.binaryblock)))
        f.write(data.tobytes(order="F"))
    back = nib.load(str(out))
    if not np.array_equal(np.asanyarray(back.dataobj)[:, ::-1], np.asanyarray(img.dataobj)):
        raise AssertionError(f"{out}: the re-stored values are not the input's")
    if nib.aff2axcodes(back.affine) != ("L", "P", "S"):
        raise AssertionError(f"{out}: stored {nib.aff2axcodes(back.affine)}, not LPS")


def _average(series: Path, sliced: Path, nonneg: bool, out: Path) -> None:
    """RobustAverage's last step (niworkflows' ``interfaces/images.py``, as the parity suite's
    ``_average``): the registered values clipped (at 0 below with ``nonnegative``), their
    median over the volumes, written with the sliced volumes' header."""
    import nibabel as nib

    s = nib.load(str(sliced))
    data = nib.load(str(series)).get_fdata(dtype="float32")
    data = np.clip(data, a_min=0.0 if nonneg else data.min(), a_max=data.max())
    s.__class__(np.median(data, axis=3), s.affine, s.header).to_filename(str(out))


def _compute(e: fcache.Entry) -> None:
    from larmorx_validation.parity import afni_replica as ar
    from larmorx_validation.parity import afni_volreg as pv

    node = pv._node(LABEL)
    src, t_mask, nonneg = pv._node_inputs(node)
    sliced = e.input("RobustAverage's sliced volumes (3dvolreg's input)", pv._sliced(LABEL))
    e.input("the raw BOLD run", src)
    e.input("fMRIPrep's BOLD reference (the node's output)", pv._fmriprep_average(LABEL))
    lps = e.dir / LPS_INPUT
    _store_lps(sliced, lps)
    e.input("the sliced volumes stored LPS (AFNI-99)", lps)
    _run_all(e, sliced, lps)
    for label in AVERAGED:
        _average(e.dir / f"{label}.nii.gz", sliced, nonneg, e.dir / f"{label}-average.nii.gz")
    e.finish(
        versions={
            "afni_26": ar.afni_build(PROGRAM, version="26.3.05"),
            "afni_25": ar.afni_build(PROGRAM, version="25.2.09"),
            "afni_in_fmriprep_image": am.image_version(PROGRAM),
            "larmorx-replica": ar.replica_version(),
        },
        command=" ".join([PROGRAM, *[a.replace("{out}", "o") for a in CALL], "sliced.nii.gz"]),
        t_mask=None if t_mask is None else [int(t) for t in t_mask],
        nonnegative=nonneg,
        threads={"afni": ["1"], "replica": ["1", "6"]},
        settings="fMRIPrep 25.2.6's RobustAverage (gen_avg, hmc_boldref_wf) of ds000005 sub-01 "
        "task mixedgamblestask run 1: its 3dvolreg call on the sliced volumes (re-made by "
        "fMRIPrep's Python in its image) by AFNI 26.3.05 and 25.2.09 (oracle builds), "
        "fMRIPrep's AFNI (in its image) and the replica (default mode at 1 and 6 threads, "
        "reproduce at 1); the same call on the volumes stored LPS (AFNI 26.3.05, the replica "
        "in both modes); RobustAverage's median of each registered series",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's RobustAverage 3dvolreg call on ds000005 sub-01 run 1: AFNI 26.3.05 and "
        "25.2.09, fMRIPrep's AFNI, the replica in both modes; the volumes stored LPS (AFNI-99); "
        "the medians",
        _compute,
    )


# ------------------------------------------------------------------------------ text files


def text_values(path: Path) -> np.ndarray:
    """A 1D file's numbers, one row per line, comment lines left out."""
    rows = [
        [float(v) for v in line.split()]
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    return np.array(rows, dtype=np.float64)


def text_body(path: Path) -> str:
    """A 1D file without its comment lines (the command line it names differs by run)."""
    return "\n".join(
        x for x in path.read_text(encoding="utf-8").splitlines() if not x.lstrip().startswith("#")
    )


def _same_text(d: Path, a: str, b: str, suffix: str) -> bool:
    return text_body(d / f"{a}{suffix}") == text_body(d / f"{b}{suffix}")


#: The -1Dfile columns (AFNI's order and units).
PARAMS = ["roll (°, about I-S)", "pitch (°, about R-L)", "yaw (°, about A-P)",
          "dS (mm)", "dL (mm)", "dP (mm)"]  # fmt: skip


def _round(v) -> list[float]:
    return [round(float(x), 6) for x in np.ravel(v)]


# ----------------------------------------------------------------------------------- export

SERIES = [{"id": "series", "label": "Registered series", "unit": ""}]
AVERAGE = [{"id": "average", "label": "BOLD reference (RobustAverage's median)", "unit": ""}]
NOTE = (
    "Δ is A − B, computed in float64 over every value of the files ({n}), not only those shown; "
    "the range is B's maximum minus its minimum. The record's threshold: every output "
    "bit-identical to AFNI 26.3.05's (the registered series, -1Dfile, -1Dmatrix_save, "
    "-maxdisp1D), with its exit status and messages. The rows *the original against itself* are "
    "AFNI's own variability on this input: its previous release and fMRIPrep's build."
)


def _panel(e: fcache.Entry, pid: str, stem: str, role: str, label: str, sub: str, out: str,
           flip: bool = False):  # fmt: skip
    from larmorx_validation.evidence import example as exm

    arr, scale, inter = common.stored(e.dir / f"{stem}.nii.gz")
    if flip:
        arr = np.ascontiguousarray(arr[:, ::-1])
    return exm.Panel(pid, role, label, sub, {out: arr}, scale=scale, inter=inter)


def _verdicts(diffs: list[dict], same: dict[str, bool], reasons: dict[str, str]) -> None:
    for x in diffs:
        i = x["id"]
        x.setdefault("links", [])
        if x["kind"] == "variability":
            x["verdict"] = None
            x["reason"] = reasons.get(i, "The same bytes." if same[i] else "They differ.")
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = reasons.get(i, "The same values, every one.")
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example."


def _series_example(ctx: core.Context, e: fcache.Entry, reg: dict) -> dict:
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.parity import afni_volreg as pv

    info = e.info()
    d = e.dir
    run = common.fmriprep_run(RUN)
    sliced = pv._sliced(LABEL)
    img = nib.load(str(sliced))
    brain = common.load(str(run / MASK)) > 0
    show = ndimage.binary_dilation(brain, iterations=1)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    t_mask = info.get("t_mask")
    raw, rs, ri = common.stored(td.get(BOLD))
    keep = np.arange(raw.shape[3]) if t_mask is None else np.flatnonzero(t_mask)
    ex.add(exm.Panel("bold", "input", "ds000005 sub-01 BOLD, run 1, as stored",
                     f"int16; the {len(keep)} volumes RobustAverage takes (file volumes "
                     f"{int(keep[0])} to {int(keep[-1])} of {raw.shape[3]})",
                     {"all": np.ascontiguousarray(raw[..., keep])}, scale=rs, inter=ri))  # fmt: skip
    sv, ss, si = common.stored(sliced)
    ex.add(exm.Panel("sliced", "input", "3dvolreg's input: RobustAverage's sliced volumes",
                     "clipped to the 99.8th percentile and divided by the global signal's drift "
                     "(fMRIPrep's Python in its image); int16 with a scale factor",
                     {"all": sv}, scale=ss, inter=si))  # fmt: skip
    for pid, role, label, sub in [
        ("afni26", "original", "AFNI 26.3.05, 3dvolreg", "the repository's build; the replica's reference"),
        ("afni25", "original", "AFNI 25.2.09, 3dvolreg", "the repository's build; the previous release"),
        ("image", "original", "AFNI in fMRIPrep's image", f"fMRIPrep 25.2.6's build ({info['versions'].get('afni_in_fmriprep_image', '?')})"),
        ("replica-1", "larmorx", "larmorx replica", "default mode, 1 thread"),
        ("replica-6", "larmorx", "larmorx replica", "default mode, 6 threads"),
        ("replica-reproduce", "larmorx", "larmorx replica", "reproduce mode, 1 thread"),
    ]:  # fmt: skip
        ex.add(_panel(e, pid, pid, role, label, sub, "series"))
    dd = common.d
    diffs = [
        dd("replica-afni26", "replica-1", "afni26", "main", "replica (default) − AFNI 26.3.05", common.ROLE_LX),
        dd("repro-afni26", "replica-reproduce", "afni26", "main", "replica (reproduce) − AFNI 26.3.05", common.ROLE_LX),
        dd("afni25-afni26", "afni25", "afni26", "variability", "AFNI 25.2.09 − 26.3.05",
           "the original against itself: the version step"),
        dd("image-afni26", "image", "afni26", "variability", "fMRIPrep's AFNI − AFNI 26.3.05", common.ROLE_ORIG),
        dd("replica6-replica1", "replica-6", "replica-1", "check", "replica, 6 threads − 1 thread", common.ROLE_SELF),
        dd("replica-repro", "replica-1", "replica-reproduce", "check", "replica, default − reproduce mode",
           common.ROLE_SELF),
    ]  # fmt: skip
    common.compare_all(ex, diffs, SERIES)
    same = common.identical(diffs)
    # the text outputs, compared without their comment line (it names each run's own files)
    texts = {
        x["id"]: {
            s: _same_text(d, x["a"], x["b"], s)
            for s in (".1D", ".aff12.1D", "_md.1D", "_md_delt.1D")
        }
        for x in diffs
    }
    md_fix = not texts["replica-repro"]["_md.1D"]
    link = [_link(reg, "AFNI-99")]
    reasons = {
        "replica-afni26": "The same values, every one: the registered series, and the parameters "
        "and matrices as text"
        + (
            "; the maximum displacements differ (AFNI-99, corrected by default: the chart below)."
            if md_fix
            else "."
        ),
        "repro-afni26": "The same values, every one, and every text file AFNI writes."
        if all(texts["repro-afni26"].values())
        else "The registered series is AFNI's; a text file differs.",
        "afni25-afni26": "AFNI 25.2.09 gives AFNI 26.3.05's bytes on this call (the series and "
        "every text file): 3dvolreg did not change between the two releases."
        if same["afni25-afni26"] and all(texts["afni25-afni26"].values())
        else "AFNI 25.2.09 and 26.3.05 differ on this call.",
        "image-afni26": "fMRIPrep's build of AFNI (in its image) gives the repository's AFNI "
        "26.3.05 bytes, the series and every text file."
        if same["image-afni26"] and all(texts["image-afni26"].values())
        else "fMRIPrep's build of AFNI differs from the repository's on this call.",
        "replica6-replica1": "The same bytes on any number of threads (AFNI's 3dvolreg has no "
        "parallel loop on this path; neither has the replica's registration).",
        "replica-repro": "The registered volumes are the same bytes in both modes: AFNI-99 changes "
        "only the maximum displacements, which the default computes for the voxels themselves "
        "(the chart below)."
        if md_fix
        else "No correction applies to this call: both modes give the same bytes.",
    }
    _verdicts(diffs, same, reasons)
    for x in diffs:
        if x["id"] in ("replica-afni26", "replica-repro") and md_fix:
            x["links"] = link
    at, _k = common.centre_view(ex)
    # the volume that moved most from the base (AFNI's own maximum displacements)
    t_most = int(np.argmax(text_values(d / "afni26_md.1D")[:, 0]))
    default = {
        "output": "series",
        "plane": "axial",
        "voxel": at,
        "t": t_most,
        "panels": ["sliced", "afni26", "replica-1", "replica-reproduce"],
        "diffs": ["replica-afni26", "afni25-afni26", "image-afni26"],
        "compare": {"a": "sliced", "b": "afni26"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(SERIES), default=default, window_panel="afni26",
                        caption=f"file volume {int(keep[t_most])}, the one AFNI finds farthest "
                        "from the base (the first volume given).")  # fmt: skip
    out["grid"]["t_index"] = [int(t) for t in keep]

    # the parameters and the maximum displacements over the volumes
    par = {k: text_values(d / f"{k}.1D") for k in ("afni26", "replica-1", "replica-reproduce")}
    md = {
        k: text_values(d / f"{k}_md.1D")[:, 0] for k in ("afni26", "replica-1", "replica-reproduce")
    }
    delt = {
        k: text_values(d / f"{k}_md_delt.1D")[:, 0]
        for k in ("afni26", "replica-1", "replica-reproduce")
    }
    n = len(par["afni26"])

    def three(values: dict) -> list[dict]:
        return [
            {"label": "AFNI 26.3.05", "mark": "original", "y": _round(values["afni26"])},
            {"label": "replica, default", "mark": "larmorx", "y": _round(values["replica-1"])},
            {"label": "replica, reproduce", "mark": "replica", "y": _round(values["replica-reproduce"])},
        ]  # fmt: skip

    panels = [
        {"label": PARAMS[j], "series": three({k: v[:, j] for k, v in par.items()})}
        for j in range(6)
    ]
    panels.append({"label": "maximum displacement (mm, -maxdisp1D)", "series": three(md)})
    panels.append({"label": "maximum displacement from the previous volume (mm, _delt)",
                   "series": three(delt)})  # fmt: skip

    def row(label: str, a: str, b: str, cls: str = "", sub: str = "") -> dict:
        pa, pb = text_values(d / f"{a}.1D"), text_values(d / f"{b}.1D")
        ma, mb = text_values(d / f"{a}_md.1D")[:, 0], text_values(d / f"{b}_md.1D")[:, 0]
        ident = [_same_text(d, a, b, s) for s in (".1D", ".aff12.1D", "_md.1D", "_md_delt.1D")]
        return {
            "head": label,
            **({"sub": sub} if sub else {}),
            **({"class": cls} if cls else {}),
            "cells": [
                common.cell(float(np.max(np.abs(pa[:, :3] - pb[:, :3])))),
                common.cell(float(np.max(np.abs(pa[:, 3:] - pb[:, 3:])))),
                common.cell(float(np.max(np.abs(ma - mb)))),
                common.cell(", ".join(
                    n_ for n_, ok in zip(("-1Dfile", "matrices", "-maxdisp1D", "_delt"), ident, strict=True)
                    if ok) or "none"),
            ],
        }  # fmt: skip

    stats = common.table(
        "volreg-params",
        "The text outputs compared, over every volume (the files without their comment line)",
        ["Comparison", {"label": "largest |Δ| rotation (°)", "num": True},
         {"label": "largest |Δ| shift (mm)", "num": True},
         {"label": "largest |Δ| max displacement (mm)", "num": True}, "identical as text"],
        [
            row("replica (default) − AFNI 26.3.05", "replica-1", "afni26",
                sub="AFNI-99 corrected" if md_fix else ""),
            row("replica (reproduce) − AFNI 26.3.05", "replica-reproduce", "afni26"),
            row("replica: default − reproduce", "replica-1", "replica-reproduce"),
            row("replica: 6 threads − 1 thread", "replica-6", "replica-1"),
            row("AFNI 25.2.09 − 26.3.05", "afni25", "afni26", "is-sep is-yardstick", sub="the version step"),
            row("fMRIPrep's AFNI − AFNI 26.3.05", "image", "afni26", "is-yardstick"),
        ],
        note="Values as the files write them (-1Dfile: 4 decimals; -maxdisp1D: 3). The shaded rows "
        "are AFNI against itself. AFNI-99: the maximum displacement AFNI writes is that of the "
        "voxel mirrored along every axis stored posterior to anterior (this run's second axis); "
        "the default computes it for the voxel itself, reproduce gives AFNI's numbers.",
    )  # fmt: skip
    lines = {
        "params": {
            "title": f"The six motion parameters and the maximum displacements over the {n} volumes",
            "x_label": "volume",
            "x": list(range(n)),
            "key": [
                {"label": "AFNI 26.3.05", "mark": "original"},
                {"label": "larmorx replica, default mode", "mark": "larmorx"},
                {"label": "larmorx replica, reproduce mode (AFNI's bytes)", "mark": "replica"},
            ],
            "panels": panels,
            "note": "3dvolreg's own parameters (-1Dfile): rotations in degrees about AFNI's axes, "
            "shifts in mm (S, L, P positive), from the base (the first volume) to each volume. "
            "Lines that coincide are the same values. Move the pointer over a chart to read "
            "every value at one volume.",
            "stats": stats,
        }
    }
    n_vals = int(np.prod(img.shape))
    vers = info.get("versions", {})
    return {
        "id": "fmriprep-volreg",
        "title": "fMRIPrep's registration of a real BOLD run's reference volumes",
        "summary": (
            "fMRIPrep's BOLD reference is the median of some of a run's volumes, aligned first: "
            "niworkflows' RobustAverage (the gen_avg node of fMRIPrep 25.2.6's hmc_boldref_wf) "
            f"takes {len(keep)} volumes of OpenNeuro ds000005 sub-01's first run (file volumes "
            f"{int(keep[0])} to {int(keep[-1])} of {raw.shape[3]}), clips them and "
            "divides them by the global signal's drift, and gives them to 3dvolreg with "
            "-Fourier -twopass -zpad 4. That call, given to AFNI 26.3.05, AFNI 25.2.09, "
            "fMRIPrep's build of AFNI and the larmorx replica in both modes. Choose a volume "
            "with the slider; the input panels show the run before registration."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{DATA['dataset']}, sub-01, task mixedgamblestask run 1 ({DATA['licence']}); "
                "shown inside fMRIPrep's BOLD brain mask, dilated by one voxel",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `bold_fit_wf/hmc_boldref_wf/gen_avg` "
                "(niworkflows' RobustAverage)",
            },
            {"label": "Command", "text": f"`{info['command']}`"},
            {
                "label": "Programs",
                "text": f"{vers.get('afni_26', '?')} and {vers.get('afni_25', '?')} (the repository's "
                f"oracle builds); AFNI {vers.get('afni_in_fmriprep_image', '?')} in fMRIPrep's image; "
                f"{vers.get('larmorx-replica', '?')} at commit `{info['larmorx_commit'][:12]}`",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "lines": lines,
        "chain": None,
        "metrics_note": NOTE.format(n=f"{n_vals:,}"),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [DATA]),
    }


def _average_example(ctx: core.Context, e: fcache.Entry) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.parity import afni_volreg as pv

    info = e.info()
    run = common.fmriprep_run(RUN)
    own = pv._fmriprep_average(LABEL)
    img = nib.load(str(own))
    brain = common.load(str(run / MASK)) > 0
    show = ndimage.binary_dilation(brain, iterations=1)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    sliced = nib.load(str(pv._sliced(LABEL)))
    data = sliced.get_fdata(dtype="float32")
    nonneg = bool(info.get("nonnegative", True))
    data = np.clip(data, a_min=0.0 if nonneg else data.min(), a_max=data.max())
    ex.add(exm.Panel("unregistered", "input", "the sliced volumes' median, not registered",
                     "what the median would be without 3dvolreg (for comparison; not a step of "
                     "fMRIPrep)", {"all": np.median(data, axis=3).astype(np.float32)}))  # fmt: skip
    ex.add(exm.Panel("fmriprep", "fmriprep", "fMRIPrep 25.2.6, as it ran",
                     "the node's BOLD reference (AFNI in its image)",
                     {"average": np.asanyarray(img.dataobj)}))  # fmt: skip
    for pid, role, label, sub in [
        ("afni26", "original", "AFNI 26.3.05's series, median", "RobustAverage's median of AFNI's registered volumes"),
        ("afni25", "original", "AFNI 25.2.09's series, median", "the previous release"),
        ("replica-1", "larmorx", "larmorx replica's series, median", "default mode, 1 thread"),
        ("replica-reproduce", "larmorx", "larmorx replica's series, median", "reproduce mode"),
    ]:  # fmt: skip
        a = np.asanyarray(nib.load(str(e.dir / f"{pid}-average.nii.gz")).dataobj)
        ex.add(exm.Panel(pid, role, label, sub, {"average": a}))
    dd = common.d
    diffs = [
        dd("replica-fmriprep", "replica-1", "fmriprep", "fmriprep", "replica − fMRIPrep's own reference",
           common.ROLE_FMRIPREP),
        dd("replica-afni26", "replica-1", "afni26", "main", "replica (default) − AFNI 26.3.05", common.ROLE_LX),
        dd("repro-afni26", "replica-reproduce", "afni26", "main", "replica (reproduce) − AFNI 26.3.05",
           common.ROLE_LX),
        dd("afni26-fmriprep", "afni26", "fmriprep", "variability", "AFNI 26.3.05 − fMRIPrep's own",
           "the original against itself: fMRIPrep's AFNI 25.2.09 in its image"),
        dd("afni25-afni26", "afni25", "afni26", "variability", "AFNI 25.2.09 − 26.3.05",
           "the original against itself: the version step"),
    ]  # fmt: skip
    common.compare_all(ex, diffs, AVERAGE)
    same = common.identical(diffs)
    reasons = {
        "replica-fmriprep": "fMRIPrep's BOLD reference of this run, every value: the replica's "
        "registered volumes give the median fMRIPrep computed from AFNI's.",
        "afni26-fmriprep": "AFNI 26.3.05 gives the reference fMRIPrep computed with its own AFNI, "
        "every value."
        if same["afni26-fmriprep"]
        else "AFNI 26.3.05's median differs from fMRIPrep's own.",
        "afni25-afni26": "The same bytes: 3dvolreg did not change between the two releases."
        if same["afni25-afni26"]
        else "The two releases differ here.",
    }
    _verdicts(diffs, same, reasons)
    at, _k = common.centre_view(ex)
    default = {
        "output": "average",
        "plane": "axial",
        "voxel": at,
        "panels": ["unregistered", "fmriprep", "replica-1"],
        "diffs": ["replica-fmriprep", "afni26-fmriprep"],
        "compare": {"a": "unregistered", "b": "replica-1"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(AVERAGE), default=default, window_panel="fmriprep",
                        caption="the crosshair at the brain's centre.")  # fmt: skip
    n_vals = int(np.prod(img.shape))
    return {
        "id": "fmriprep-boldref",
        "title": "The BOLD reference fMRIPrep makes from the registered volumes",
        "summary": (
            "RobustAverage's last step: the median, voxel by voxel, of the registered volumes of "
            "the first example. fMRIPrep's own reference of the run (the node's output, from "
            "AFNI in its image) beside the median of AFNI 26.3.05's, AFNI 25.2.09's and the "
            "replica's registered series, computed as RobustAverage computes it. The first "
            "panel is the median of the volumes before registration: choose *compare* to see "
            "what aligning them changes."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{DATA['dataset']}, sub-01, run 1 ({DATA['licence']}); "
                "shown inside fMRIPrep's BOLD brain mask, dilated by one voxel",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `bold_fit_wf/hmc_boldref_wf/gen_avg`: "
                "its output, `…_bold_average.nii.gz`",
            },
            {
                "label": "Median",
                "text": "niworkflows' RobustAverage: the registered values clipped "
                f"({'at 0 below' if nonneg else 'to their range'}), `np.median` over the volumes, "
                "written with the sliced volumes' header (float32)",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "metrics_note": NOTE.format(n=f"{n_vals:,}").replace(
            "(the registered series, -1Dfile, -1Dmatrix_save, -maxdisp1D)",
            "(the registered series and the text files; here their median, which the record "
            "also compares with fMRIPrep's own)",
        ),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [DATA]),
    }


def _afni99_example(ctx: core.Context, e: fcache.Entry, reg: dict) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.parity import afni_volreg as pv

    info = e.info()
    d = e.dir
    run = common.fmriprep_run(RUN)
    sliced = pv._sliced(LABEL)
    img = nib.load(str(sliced))
    brain = common.load(str(run / MASK)) > 0
    show = ndimage.binary_dilation(brain, iterations=1)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    sv, ss, si = common.stored(sliced)
    ex.add(exm.Panel("sliced", "input", "3dvolreg's input, stored LAS", "as fMRIPrep gives it "
                     "(the first example's input)", {"all": sv}, scale=ss, inter=si))  # fmt: skip
    for pid, stem, role, label, sub, flip in [
        ("afni26", "afni26", "original", "AFNI 26.3.05, stored LAS", "as fMRIPrep gives it", False),
        ("lps-afni26", "lps-afni26", "original", "AFNI 26.3.05, stored LPS",
         "the same volumes, the second axis reversed; shown back in place", True),
        ("lps-replica", "lps-replica", "larmorx", "larmorx replica, stored LPS", "default mode", True),
        ("lps-replica-reproduce", "lps-replica-reproduce", "larmorx", "larmorx replica, stored LPS",
         "reproduce mode", True),
    ]:  # fmt: skip
        ex.add(_panel(e, pid, stem, role, label, sub, "series", flip=flip))
    dd = common.d
    diffs = [
        dd("lps-las", "lps-afni26", "afni26", "variability", "AFNI 26.3.05: stored LPS − LAS",
           "the original against itself: the same volumes stored another way"),
        dd("lps-replica-afni", "lps-replica", "lps-afni26", "main", "replica (default) − AFNI, LPS", common.ROLE_LX),
        dd("lps-repro-afni", "lps-replica-reproduce", "lps-afni26", "main", "replica (reproduce) − AFNI, LPS",
           common.ROLE_LX),
    ]  # fmt: skip
    common.compare_all(ex, diffs, SERIES)
    same = common.identical(diffs)
    m = next(x for x in diffs if x["id"] == "lps-las")["metrics"]["series"]
    reasons = {
        "lps-las": "The same registration in the other storage order: each volume is resampled "
        "along the second axis in the other direction, so the parameters differ in their last "
        f"digits and {m['differ']:,} of {m['voxels']:,} values by up to {m['max_abs']:.3g} "
        f"({m['max_rel_range']:.2g} of the range)."
        if not same["lps-las"]
        else "The same bytes in either storage order.",
        "lps-replica-afni": "The same values, every one: AFNI-99 changes no registered volume.",
    }
    _verdicts(diffs, same, reasons)
    link = [_link(reg, "AFNI-99")]
    for x in diffs:
        x["links"] = link
    t_last = int(img.shape[3]) - 1
    if same["lps-las"]:
        at, _k = common.centre_view(ex)
    else:
        at, _k = common.largest_in_slice(ex, "lps-afni26", "afni26", "series", "axial", t=t_last)
    default = {
        "output": "series",
        "plane": "axial",
        "voxel": at,
        "t": t_last,
        "panels": ["afni26", "lps-afni26", "lps-replica"],
        "diffs": ["lps-las", "lps-replica-afni"],
        "compare": {"a": "afni26", "b": "lps-afni26"},
    }
    out = common.finish(ctx, ex, diffs, copy.deepcopy(SERIES), default=default, window_panel="afni26",
                        caption="the last volume.")  # fmt: skip
    # the maximum displacements: AFNI and the replica's default on both storages
    md = {k: text_values(d / f"{k}_md.1D")[:, 0] for k in
          ("afni26", "lps-afni26", "replica-1", "lps-replica", "replica-reproduce", "lps-replica-reproduce")}  # fmt: skip
    n = len(md["afni26"])
    panels = [
        {"label": "stored LAS, as fMRIPrep gives it (mm)", "series": [
            {"label": "AFNI 26.3.05", "mark": "original", "y": _round(md["afni26"])},
            {"label": "replica, default", "mark": "larmorx", "y": _round(md["replica-1"])}]},
        {"label": "the same volumes stored LPS (mm)", "series": [
            {"label": "AFNI 26.3.05", "mark": "original", "y": _round(md["lps-afni26"])},
            {"label": "replica, default", "mark": "larmorx", "y": _round(md["lps-replica"])}]},
        {"label": "LAS − LPS (mm)", "series": [
            {"label": "AFNI 26.3.05", "mark": "original", "y": _round(md["afni26"] - md["lps-afni26"])},
            {"label": "replica, default", "mark": "larmorx", "y": _round(md["replica-1"] - md["lps-replica"])}]},
    ]  # fmt: skip
    repro_same = _same_text(d, "replica-reproduce", "afni26", "_md.1D") and _same_text(
        d, "lps-replica-reproduce", "lps-afni26", "_md.1D"
    )
    lps_same = _same_text(d, "lps-replica", "lps-afni26", "_md.1D")

    def big(a: np.ndarray) -> float:
        return float(np.max(np.abs(a)))

    rows = [
        {"head": "AFNI 26.3.05: LAS − LPS", "class": "is-yardstick",
         "sub": "the same volumes; AFNI's maximum displacement depends on the storage order",
         "cells": [common.cell(big(md["afni26"] - md["lps-afni26"])), common.cell("–")]},
        {"head": "replica, default: LAS − LPS", "verdict": "d13",
         "sub": "the voxels' own displacements, in either storage order",
         "cells": [common.cell(big(md["replica-1"] - md["lps-replica"])),
                   common.cell("AFNI-99 corrected, named")]},
        {"head": "replica, default − AFNI 26.3.05, stored LAS", "verdict": "d13",
         "sub": "the second axis is stored posterior to anterior: AFNI mirrors the voxels along it",
         "cells": [common.cell(big(md["replica-1"] - md["afni26"])), common.cell("AFNI-99 corrected, named")]},
        {"head": "replica, default − AFNI 26.3.05, stored LPS",
         "verdict": "bit-identical" if lps_same else "open",
         "sub": "every axis positive in AFNI's order: nothing to correct",
         "cells": [common.cell(big(md["lps-replica"] - md["lps-afni26"])),
                   common.cell("identical as text" if lps_same else "differs")]},
        {"head": "replica, reproduce − AFNI 26.3.05, both storages",
         "verdict": "bit-identical" if repro_same else "open",
         "sub": "AFNI's numbers, byte for byte",
         "cells": [common.cell(max(big(md["replica-reproduce"] - md["afni26"]),
                                   big(md["lps-replica-reproduce"] - md["lps-afni26"]))),
                   common.cell("identical as text" if repro_same else "differs")]},
    ]  # fmt: skip
    stats = common.table(
        "afni99-md",
        "The maximum displacements compared (-maxdisp1D, over every volume)",
        ["Comparison", {"label": "largest |Δ| (mm)", "num": True}, "text"],
        rows,
        note="-maxdisp1D writes 3 decimals. The registration itself does not depend on the "
        "storage order beyond the last bits of the parameters (the viewer above).",
    )
    lines = {
        "md": {
            "title": f"Maximum displacement over the {n} volumes, in two storage orders",
            "x_label": "volume",
            "x": list(range(n)),
            "key": [
                {"label": "AFNI 26.3.05 (the replica's reproduce mode gives these numbers)", "mark": "original"},
                {"label": "larmorx replica, default mode", "mark": "larmorx"},
            ],
            "panels": panels,
            "note": "-maxdisp1D: for each volume, the largest displacement of a voxel inside "
            "the base's automask. Move the pointer over a chart to read every value at one volume.",
            "stats": stats,
        }
    }  # fmt: skip
    a99 = reg.get("AFNI-99", {})
    return {
        "id": "afni-99",
        "title": "AFNI-99: the maximum displacement of a mirrored voxel",
        "summary": (
            "3dvolreg's -maxdisp1D (and -savedisp) convert the registration's rotation and shift to "
            "the dataset's axes with each axis's sign, but take the voxel positions without it: for "
            "an axis stored left to right, posterior to anterior or superior to inferior, the "
            "displacement computed is that of the voxel mirrored about the axis's centre. "
            "fMRIPrep's volumes of the first example are stored that way along their second axis. "
            "The same volumes stored with that axis reversed (LPS: every axis positive in AFNI's "
            "order), registered by AFNI 26.3.05 and the replica in both modes."
        ),
        "facts": [
            {
                "label": "Input",
                "text": "the first example's sliced volumes; and the same stored "
                "integers, scale factor and header, the second axis reversed and the qform and sform "
                "changed so that every voxel stays where it was",
            },
            {"label": "Command", "text": f"`{info['command']}`"},
            {
                "label": "Defect",
                "text": f"[AFNI-99](repo:{link[0]['href']}): "
                + (
                    a99.get("title")
                    or "-maxdisp and -savedisp give the displacements of a mirrored position"
                )
                + "; corrected by default, named; reproduce gives AFNI's numbers",
            },
            {
                "label": "Size",
                "text": "on this run, whose head moves little (at most "
                f"{float(md['afni26'].max()):.3f} mm from the base), AFNI's maximum displacements "
                f"change by up to {big(md['afni26'] - md['lps-afni26']):.3f} mm with the storage "
                "order; the error grows with the rotation and with the voxels' distance from the "
                "axis's centre (on the API page's synthetic series, 1.718 against 1.672 mm)",
            },
            {
                "label": "fMRIPrep",
                "text": "RobustAverage writes -maxdisp1D but uses only the "
                "registered volumes: fMRIPrep's results do not change",
            },
        ],
        **out,
        "lines": lines,
        "chain": None,
        "metrics_note": NOTE.format(n=f"{int(np.prod(img.shape)):,}"),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [DATA]),
    }


def _link(reg: dict, rid: str) -> dict:
    """A defect's link: its register entry, or (until the orchestrator adds it there at the
    merge) the API page's table of 3dvolreg's defects."""
    r = reg.get(rid)
    return {"label": rid, "href": r["link"] if r else "docs/api/afni-volreg.md#upstream-defects"}


# ------------------------------------------------------------------------------------ cases

#: The register's entries 3dvolreg's replica corrects by default (the others: an error in both
#: modes, rule B, or AFNI's messages).
CORRECTED = frozenset({"AFNI-86", "AFNI-87", "AFNI-90", "AFNI-92", "AFNI-93", "AFNI-95",
                       "AFNI-97", "AFNI-98", "AFNI-99"})  # fmt: skip


def input_name(name: str) -> str:
    kind, _, rest = name.partition(":")
    if kind == "fmriprep":
        return f"<fMRIPrep {rest}: RobustAverage's sliced volumes>"
    if kind == "fmriprep-average":
        return f"<fMRIPrep {rest}: its BOLD reference>"
    if kind == "blank":
        return f"<generated: {rest}, in a directory with a blank>"
    if kind == "dt":
        return f"<random testing: {rest}>"
    return f"<generated: {name}>"


def command(s) -> str:
    import shlex

    names = [input_name(n) for n in s.inputs]
    parts = [PROGRAM]
    for a in s.args:
        m = re.fullmatch(r"\{(\d+)\}", a)
        parts.append(names[int(m.group(1))] if m else shlex.quote(a) if re.search(r"\s", a) else a)
    env = " ".join(f"{k}={v}" for k, v in s.env if v is not None)
    files = " ".join(f"(with {n} present)" for n, _ in s.files)
    return common.clip(" ".join(x for x in (env, " ".join(parts), files) if x), 600)


def d13_cases(md: str) -> dict[str, list[str]]:
    """The defects the record's D13 section names per case (``- AFNI-99: ... : `case`, ...``;
    a defect's list is cut after its first cases)."""
    out: dict[str, list[str]] = {}
    for line in md.splitlines():
        m = re.match(r"\s*- ((?:AFNI|GLIBC)-\d+)(?: \(not named\))?: \d+ case", line)
        if m:
            for c in re.findall(r"`([^`]+/[^`]+)`", line):
                out.setdefault(c, []).append(m.group(1))
    return out


#: Why AFNI-99 meets every fMRIPrep call: its inputs (RobustAverage's sliced volumes, every
#: oracle run's) are stored LAS, the second axis posterior to anterior.
FMRIPREP_AFNI99 = (
    "the maximum displacements of -maxdisp1D, as every fMRIPrep input of the record is stored "
    "with its second axis posterior to anterior; the registered volumes, and so fMRIPrep's "
    "reference, are the same in both modes"
)


def cases(rec, examples: dict[str, str], md: str = "") -> list[dict]:
    from larmorx_validation.parity.afni_volreg import _scenario_list

    defs = {cid: (desc, s) for cid, _cat, desc, s in _scenario_list()}
    listed = d13_cases(md)
    out = []
    for r in rec.results:
        desc, s = defs.get(r["case"], ("", None))
        v, why = am.case_verdict(r, desc, "replica", s.fix_refuses if s is not None else None)
        named = sorted(
            (set(re.findall(r"\bAFNI-\d+\b", desc)) & CORRECTED) | set(listed.get(r["case"], []))
        )
        if "a defect of the register" in why:
            if named == ["GLIBC-07"]:
                why = why.replace(
                    "corrects a defect of the register and names it",
                    "computes with correctly rounded maths where glibc 2.35's is not correctly "
                    "rounded (GLIBC-07, not named)",
                )
            elif named:
                why = why.replace("a defect of the register", ", ".join(named))
            elif r["category"] == "fmriprep":
                why = why.replace("a defect of the register", "AFNI-99").replace(
                    "values change (the record's D13 section)",
                    f"values change: {FMRIPREP_AFNI99} (the record's D13 section)",
                )
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": desc,
                "command": command(s) if s else "",
                "results": {
                    "replica": {
                        "outcome": r["status"],
                        "verdict": v,
                        "checks": f"{r['checks_passed']}/{r['checks_total']}",
                        "reason": why,
                    }
                },
                **({"example": examples[r["case"]]} if r["case"] in examples else {}),
            }
        )
    return out


# ------------------------------------------------------------------------------------ speed

TIMINGS = "validation/investigations/afni-volreg/speed-2026-11-06.json"
SPEED_SERIES = [
    ("AFNI", "AFNI 26.3.05, 1 thread", "original"),
    ("larmorx", "replica, larmorx afni 3dvolreg", "replica"),
    ("python -m larmorx", "replica, python -m larmorx", "replica-open"),
]
SPEED_LABELS = {
    "ds000005-run-01": "ds000005 run 1, single-band",
    "ds003592-echo-1": "ds003592 echo 1, multi-echo",
    "ds005454": "ds005454, multiband",
    "ds004958": "ds004958, multiband 8",
    "pepolar-ds005040": "ds005040, PEPOLAR average",
}


def timings_source(repo: Path) -> list[dict]:
    import hashlib

    path = repo / TIMINGS
    if not path.is_file():
        return []
    return [{"path": TIMINGS, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}]


def informal_speed(repo: Path) -> dict | None:
    import json

    path = repo / TIMINGS
    if not path.is_file():
        return None
    data = json.loads(path.read_text(encoding="utf-8"))
    series = [{"id": i, "label": label, "mark": mark} for i, label, mark in SPEED_SERIES]
    rows = []
    for r in data:
        s = {i: float(r["seconds"][i]) for i, _, _ in SPEED_SERIES if i in r["seconds"]}
        a, b = s.get("AFNI"), s.get("larmorx")
        shape = " × ".join(map(str, r["shape"]))
        rows.append(
            {
                "label": f"{SPEED_LABELS.get(r['label'], r['label'])} ({shape})",
                "input": r["label"],
                "seconds": s,
                "speedup": a / b if a and b else None,
            }
        )
    axis, ticks = common.ticks_log([v for r in rows for v in r["seconds"].values()], " s")
    return {
        "title": "3dvolreg: wall time of fMRIPrep's RobustAverage call (informal timings, 2026-11-06)",
        "axis": axis,
        "ticks": ticks,
        "series": series,
        "rows": rows,
        "note": (
            "fMRIPrep's call (-Fourier -twopass -zpad 4 with -1Dfile, -1Dmatrix_save, -maxdisp1D) "
            "on the parity suite's inputs, one thread each (3dvolreg has no parallel loop on this "
            "path), the median of three runs, reading and writing compressed NIfTI included, every "
            "output checked byte for byte; on the main machine while another agent worked: "
            "indications, not a benchmark report. The ratio is AFNI's time over the replica's "
            "program's. Tool: validation/investigations/afni-volreg/speed.py."
        ),
    }


# ----------------------------------------------------------------------------------- export


def export(ctx: core.Context) -> dict:
    reg = core.register(ctx.repo)
    rec = records.load(ctx.repo, f"{TOOL}-replica")
    examples = {
        f"fmriprep/{LABEL}": "fmriprep-volreg",
    }
    md = (ctx.repo / f"docs/validation/{TOOL}-replica.md").read_text(encoding="utf-8")
    cs = cases(rec, examples, md)
    e = entry(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "3dvolreg",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": timings_source(ctx.repo),
            "cache": [fcache.provenance(e)],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cs,
                "replica",
                "larmorx replica",
                "replica",
                f"docs/validation/{TOOL}-replica.md",
                am.status_of(rec),
            )
        ],
        "cases": cs,
        "cases_total": len(cs),
        "speed": informal_speed(ctx.repo),
        "examples": [
            _series_example(ctx, e, reg),
            _average_example(ctx, e),
            _afni99_example(ctx, e, reg),
        ],
    }


if __name__ == "__main__":  # compute the cache entry only
    from larmorx_validation.figures.core import REPO, default_cache

    print("done", entry(core.Context(repo=REPO, site=Path("."), cache=default_cache())).dir)
