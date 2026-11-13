# SPDX-License-Identifier: Apache-2.0
"""``check_nipype_cli.py``'s cases: the generated inputs (seeded integers and plain arithmetic,
and the files nipype's own interfaces make from them for the steps after them), the cases, and
fMRIPrep's real calls read from the oracle runs' working folders."""

from __future__ import annotations

import ast
import contextlib
import json
import os
import re
import shutil
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve()
WORKTREE = Path(os.environ.get("LARMORX_WORKTREE", HERE.parents[3]))
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", HERE.parents[6]))
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
DATA = json.loads((WORKTREE / "python/larmorx/nipype/interfaces.json").read_text(encoding="utf-8"))
MODULES = list(DATA["modules"])


@dataclass
class Case:
    name: str
    args: list[str]
    #: "same": larmorx in both modes equals nipype's command line; "fix:<ID>": nipype's
    #: command line refuses it (reproduce equals it), larmorx's default corrects the defect and
    #: is checked against nipype's Python API (``api``); "auto": "same" where nipype's command
    #: line runs it, else "fix:" either defect; "reproduce": only ``reproduce`` compared (the
    #: default's message names a corrected type); "larmorx": a documented difference (larmorx's
    #: own message: a module it does not serve, an interface it has not ported)
    expect: str = "same"
    columns: int = 80
    copy: list[str] = field(default_factory=list)
    api: tuple[str, str, dict[str, Any]] | None = None
    #: tables compared by column name where their bytes differ (PIPE-43)
    by_name: bool = False


def make_inputs(root: Path) -> None:
    import importlib
    import warnings

    import nibabel as nb
    import numpy as np

    rng = np.random.default_rng(20261113)
    shape = (10, 9, 7)
    t = 24
    aff = np.diag([2.0, 2.0, 2.5, 1.0])
    aff[:3, 3] = [-9.0, -8.0, -7.5]
    base = rng.integers(300, 1200, shape).astype(np.float32)
    trend = np.arange(t, dtype=np.float32) * 3
    noise = rng.integers(-40, 40, (*shape, t)).astype(np.float32)
    bold = base[..., None] + trend + noise
    bold[..., :2] += 400  # two non-steady volumes
    img = nb.Nifti1Image(bold, aff)
    img.header.set_xyzt_units("mm", "sec")
    img.header["pixdim"][4] = 2.0
    img.to_filename(root / "bold.nii.gz")
    nb.Nifti1Image(bold[..., 5], aff).to_filename(root / "boldref.nii.gz")
    mask = np.zeros(shape, np.uint8)
    mask[2:8, 2:7, 1:6] = 1
    nb.Nifti1Image(mask, aff).to_filename(root / "mask.nii.gz")
    small = np.zeros(shape, np.uint8)
    small[3:7, 3:6, 2:5] = 1
    nb.Nifti1Image(small, aff).to_filename(root / "small.nii.gz")
    for name, seed in (("wm", 1), ("csf", 2), ("gm", 3)):
        p = (np.random.default_rng(seed).integers(0, 9, shape) / 8).astype(np.float32)
        p[mask == 0] = 0
        nb.Nifti1Image(p, aff).to_filename(root / f"{name}.nii.gz")
        nb.Nifti1Image((p > 0.5).astype(np.uint8), aff).to_filename(root / f"{name}_bin.nii.gz")
    rot = rng.integers(-200, 200, (t, 3)) / 100000
    trans = rng.integers(-300, 300, (t, 3)) / 1000
    np.savetxt(root / "mc.par", np.hstack([rot, trans]), fmt="%.6f")
    lines = ["#Insight Transform File V1.0"]
    for k in range(t):
        m = np.eye(3) + rng.integers(-100, 100, (3, 3)) / 1e5
        m = (m + m.T) / 2
        p = list(m.ravel()) + list(rng.integers(-300, 300, 3) / 1000)
        lines += [
            f"#Transform {k}",
            "Transform: AffineTransform_float_3_3",
            "Parameters: " + " ".join("%g" % np.float32(v) for v in p),  # noqa: UP031 (ITK's %g)
            "FixedParameters: 0 0 0",
            "",
        ]
    (root / "xfm.txt").write_text("\n".join(lines), encoding="utf-8")

    def nipype(module: str, iface: str, **inputs: Any) -> Any:
        d = root / f"_{iface}"
        d.mkdir(exist_ok=True)
        with contextlib.chdir(d), warnings.catch_warnings():
            warnings.simplefilter("ignore")
            node = getattr(importlib.import_module(module), iface)(**inputs)
            return node.run().outputs

    def r(n: str) -> str:
        return str(root / n)

    nac = "nipype.algorithms.confounds"
    out = nipype(
        nac, "ACompCor", realigned_file=r("bold.nii.gz"),
        mask_files=[r("csf_bin.nii.gz"), r("wm_bin.nii.gz"), r("mask.nii.gz")],
        mask_names=["CSF", "WM", "combined"], merge_method="none", variance_threshold=0.5,
        pre_filter="cosine", save_pre_filter=True, save_metadata=True, repetition_time=2.0,
        components_file="acompcor.tsv", header_prefix="a_comp_cor_",
    )  # fmt: skip
    shutil.copy(out.components_file, root / "acompcor.tsv")
    shutil.copy(out.metadata_file, root / "acompcor_metadata.tsv")
    shutil.copy(out.pre_filter_file, root / "cosine.tsv")
    out = nipype(
        nac, "TCompCor", realigned_file=r("bold.nii.gz"), mask_files=[r("mask.nii.gz")],
        num_components=3, pre_filter="cosine", save_metadata=True, repetition_time=2.0,
        components_file="tcompcor.tsv", header_prefix="t_comp_cor_",
    )  # fmt: skip
    shutil.copy(out.components_file, root / "tcompcor.tsv")
    out = nipype(nac, "ComputeDVARS", in_file=r("bold.nii.gz"), in_mask=r("mask.nii.gz"),
                 save_nstd=True)  # fmt: skip
    shutil.copy(out.out_std, root / "dvars_std.tsv")
    shutil.copy(out.out_nstd, root / "dvars_nstd.tsv")
    for src, col, dst in (("dvars_std.tsv", "std_dvars", "std_dvars.tsv"),
                          ("dvars_nstd.tsv", "dvars", "dvars.tsv")):  # fmt: skip
        out = nipype(
            "niworkflows.interfaces.utility", "AddTSVHeader", in_file=r(src), columns=[col]
        )
        shutil.copy(out.out_file, root / dst)
    fmc = "fmriprep.interfaces.confounds"
    out = nipype(fmc, "FSLMotionParams", xfm_file=r("xfm.txt"), boldref_file=r("boldref.nii.gz"))
    shutil.copy(out.out_file, root / "motion.tsv.gz")
    out = nipype(fmc, "FSLRMSDeviation", xfm_file=r("xfm.txt"), boldref_file=r("boldref.nii.gz"))
    shutil.copy(out.out_file, root / "rmsd.tsv.gz")
    out = nipype(fmc, "FramewiseDisplacement", in_file=r("motion.tsv.gz"))
    shutil.copy(out.out_file, root / "fd.tsv.gz")
    out = nipype(
        "niworkflows.interfaces.images", "SignalExtraction", in_file=r("bold.nii.gz"),
        label_files=[r("mask.nii.gz"), r("csf_bin.nii.gz"), r("wm_bin.nii.gz")],
        class_labels=["global_signal", "csf", "white_matter"],
    )  # fmt: skip
    shutil.copy(out.out_file, root / "signals.tsv")
    out = nipype(
        fmc, "GatherConfounds", signals=r("signals.tsv"), dvars=r("dvars.tsv"),
        std_dvars=r("std_dvars.tsv"), fd=r("fd.tsv.gz"), rmsd=r("rmsd.tsv.gz"),
        tcompcor=r("tcompcor.tsv"), acompcor=r("acompcor.tsv"), cos_basis=r("cosine.tsv"),
        motion=r("motion.tsv.gz"),
    )  # fmt: skip
    shutil.copy(out.confounds_file, root / "confounds.tsv")


# --- fMRIPrep's real calls -------------------------------------------------------------------

REAL_RUNS = (
    "ds000005-sub-01-noreconall",
    "ds005454-sub-16-noreconall",
    "ds003592-sub-280-noreconall",
)
#: confounds nodes → (module, interface)
REAL_NODES = {
    "dvars": ("nipype.algorithms.confounds", "ComputeDVARS"),
    "acompcor": ("niworkflows.interfaces.patches", "RobustACompCor"),
    "crowncompcor": ("niworkflows.interfaces.patches", "RobustACompCor"),
    "tcompcor": ("niworkflows.interfaces.patches", "RobustTCompCor"),
    "motion_params": ("fmriprep.interfaces.confounds", "FSLMotionParams"),
    "rmsd": ("fmriprep.interfaces.confounds", "FSLRMSDeviation"),
    "fdisp": ("fmriprep.interfaces.confounds", "FramewiseDisplacement"),
    "signals": ("niworkflows.interfaces.images", "SignalExtraction"),
    "concat": ("fmriprep.interfaces.confounds", "GatherConfounds"),
    "model_expansion": ("niworkflows.interfaces.confounds", "ExpandModel"),
    "spike_regressors": ("niworkflows.interfaces.confounds", "SpikeRegressors"),
    "dilated_mask": ("niworkflows.interfaces.morphology", "BinaryDilation"),
    "subtract_mask": ("niworkflows.interfaces.morphology", "BinarySubtraction"),
    "rename_acompcor": ("fmriprep.interfaces.confounds", "RenameACompCor"),
    "add_dvars_header": ("niworkflows.interfaces.utility", "AddTSVHeader"),
    "add_std_dvars_header": ("niworkflows.interfaces.utility", "AddTSVHeader"),
    "acc_masks": ("fmriprep.interfaces.confounds", "aCompCorMasks"),
}


def node_inputs(run: Path, node: Path) -> dict[str, Any] | None:
    """The node's execution inputs (``_report/report.rst``), with fMRIPrep's container paths
    mapped to the run's folders; ``None`` when an input file is gone."""
    rep = node / "_report" / "report.rst"
    if not rep.exists():
        return None
    text = rep.read_text(errors="replace")
    m = re.search(r"Execution Inputs\n-+\n(.*?)\n\nExecution Outputs", text, re.S)
    if not m:
        return None
    inputs: dict[str, Any] = {}
    for line in m.group(1).splitlines():
        mm = re.match(r"\* (\w+) : (.*)$", line)
        if not mm or mm.group(2) == "<undefined>":
            continue
        raw = mm.group(2).replace("'/work/", f"'{run}/work/").replace("'/out/", f"'{run}/out/")
        if raw.startswith(("/work/", "/out/")):
            raw = repr(str(run) + raw)
        try:
            inputs[mm.group(1)] = ast.literal_eval(raw)
        except (ValueError, SyntaxError):
            inputs[mm.group(1)] = raw
    for v in inputs.values():
        for p in v if isinstance(v, list) else [v]:
            if isinstance(p, str) and p.startswith(str(run)) and not Path(p).exists():
                return None
    return inputs


def cli_args(module: str, iface: str, inputs: dict[str, Any]) -> list[str] | None:
    """The command line that sets ``inputs`` (nipype's parser's form); ``None`` when one cannot
    be given (a false boolean whose default is true: PIPE-83)."""
    spec = DATA["modules"][module]["specs"][iface]
    calls = spec.get("calls") or spec["calls_corrected"]
    pos: list[str] = []
    opt: list[str] = []
    for c in calls:
        flag = c["args"][0]
        name = flag.lstrip("-")
        if name not in inputs or inputs[name] is None:
            continue
        v = inputs[name]
        default = spec["inputs"][name]["printed"]
        if c.get("action") == "store_true":
            if v is True and default != "True":
                opt.append(flag)
            elif str(v) != default:
                return None
            continue
        if isinstance(v, tuple):
            vals = [",".join(map(str, v))]
        elif isinstance(v, list):
            vals = [str(x) for x in v]
        else:
            vals = [str(v)]
        if flag.startswith("-"):
            if str(v) != default:
                opt += [flag, *vals]
        else:
            pos += vals
    return pos + opt


def real_cases() -> list[Case]:
    out = []
    for run_name in REAL_RUNS:
        run = RUNS / run_name
        wfs = sorted((run / "work").glob("fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_confounds_wf"))
        if not wfs:
            continue
        wf = wfs[0]
        for node_name, (module, iface) in REAL_NODES.items():
            if not (wf / node_name).is_dir():
                continue
            inputs = node_inputs(run, wf / node_name)
            if inputs is None:
                continue
            args = cli_args(module, iface, inputs)
            if args is None:
                continue
            spec = DATA["modules"][module]["specs"][iface]
            api = {k: v for k, v in inputs.items() if k in spec["inputs"]}
            out.append(
                Case(
                    f"real/{run_name.split('-')[0]}/{node_name}",
                    [module, iface, *args],
                    expect="auto",
                    api=(module, iface, api),
                    by_name=node_name in ("model_expansion", "spike_regressors"),
                )
            )
    return out


# --- the cases -------------------------------------------------------------------------------


def cases(tier: str, inputs: Path) -> list[Case]:
    def i(n: str) -> str:
        return str(inputs / n)

    nac = "nipype.algorithms.confounds"
    nwc = "niworkflows.interfaces.confounds"
    fmc = "fmriprep.interfaces.confounds"
    nwi = "niworkflows.interfaces.images"
    nwu = "niworkflows.interfaces.utility"
    nwm = "niworkflows.interfaces.morphology"
    nwn = "niworkflows.interfaces.nibabel"
    nwp = "niworkflows.interfaces.patches"
    b, k = i("bold.nii.gz"), i("mask.nii.gz")
    csf, wm = i("csf_bin.nii.gz"), i("wm_bin.nii.gz")
    cs: list[Case] = [
        Case("click/no-args", []),
        Case("click/list-only", ["--list"]),
        Case("click/help-only", ["--help"]),
        Case("click/h-only", ["-h"]),
        Case("click/list-value", ["--list=x", nac]),
        Case("click/help-value", [nac, "--help=1"]),
        Case("click/double-dash", ["--", nac]),
        Case("click/empty-interface", [nac, ""]),
        Case("click/unknown-interface", [nac, "Nope"]),
        Case("click/unknown-module", ["nipype.nope"], expect="larmorx"),
        Case("click/other-module", ["nipype.algorithms.misc", "--list"], expect="larmorx"),
        Case("click/list-after", [nac, "ComputeDVARS", "--list"]),
        Case("click/h-cluster", [nac, "-hq", "ComputeDVARS"]),
        Case("click/help-after-interface", [nac, "ComputeDVARS", "-h"]),
        Case("click/help-among-inputs", [nac, "ComputeDVARS", b, "-h", k]),
        Case("click/argparse-help", [nac, "ComputeDVARS", "--", "-h"]),
        Case("click/init-error", [nwu, "KeySelect"]),
        Case("click/base-interface-arg", [nac, "BaseInterface", "x"]),
    ]
    for m in MODULES:
        cs.append(Case(f"list/{m}", [m, "--list"]))
        cs.append(Case(f"list-bare/{m}", [m]))
        for n in DATA["modules"][m]["interfaces"]:
            spec = DATA["modules"][m]["specs"][n]
            exp = "fix:PIPE-81" if "parser_error" in spec else "same"
            cs.append(Case(f"help/{m}/{n}", [m, n], expect=exp))
    for m, n in ((nac, "CompCor"), (fmc, "GatherConfounds"), (nwc, "SpikeRegressors")):
        cs.append(Case(f"help132/{m}/{n}", [m, n], columns=132))
    cs += [
        # argparse's and traits' errors
        Case("error/missing-positional", [nac, "ComputeDVARS", b]),
        Case("error/unknown-option", [nac, "ComputeDVARS", b, k, "--bogus"]),
        Case("error/bad-float", [nac, "ComputeDVARS", b, k, "--variance_tol", "x"]),
        Case("error/missing-file", [nac, "ComputeDVARS", i("nope.nii.gz"), k]),
        Case("error/enum", [nac, "ACompCor", b, "--failure_mode", "x"]),
        Case("error/range", [nac, "ACompCor", b, "--ignore_initial_volumes", "-1"]),
        Case("error/list-element", [nac, "ACompCor", b, "--mask_files", k, i("nope.nii")]),
        Case("error/xor", [nac, "ACompCor", b, "--mask_files", k, "--mask_index", "0",
                           "--merge_method", "union"]),
        Case("error/requires", [nac, "ACompCor", b, "--mask_index", "0", "--repetition_time", "2"]),
        Case("error/deprecated", [nac, "ACompCor", b, "--mask_files", k, "--use_regress_poly"]),
        Case("error/num-components-text", [nac, "ACompCor", b, "--mask_files", k,
                                           "--num_components", "x"]),
        Case("error/3d-dvars", [nac, "ComputeDVARS", i("boldref.nii.gz"), k]),
        Case("error/tuple-figsize", [nac, "ComputeDVARS", b, k, "--figsize", "11,3"],
             expect="fix:PIPE-82",
             api=(nac, "ComputeDVARS", {"in_file": b, "in_mask": k, "figsize": (11.0, 3.0)})),
        Case("error/not-ported", [nwi, "Conform", b], expect="larmorx"),
        # nipype's confounds
        Case("run/dvars", [nac, "ComputeDVARS", b, k]),
        Case("run/dvars-all", [nac, "ComputeDVARS", b, k, "--save_all", "--save_nstd",
                               "--save_vxstd"]),
        Case("run/dvars-options", [nac, "ComputeDVARS", b, k, "--intensity_normalization", "0",
                                   "--variance_tol", "0.5", "--series_tr", "2"]),
        Case("run/dvars-relative", [nac, "ComputeDVARS", "./bold.nii.gz", "mask.nii.gz"],
             copy=["bold.nii.gz", "mask.nii.gz"]),
        Case("run/acompcor-default", [nac, "ACompCor", b, "--mask_files", k,
                                      "--repetition_time", "2"]),
        Case("run/acompcor-options", [
            nac, "ACompCor", b, "--mask_files", csf, wm, "--mask_names", "CSF", "WM",
            "--merge_method", "none", "--variance_threshold", "0.5", "--pre_filter", "cosine",
            "--components_file", "comps.tsv", "--header_prefix", "a_",
            "--ignore_initial_volumes", "2", "--failure_mode", "NaN"]),
        Case("run/acompcor-union-all", [
            nac, "ACompCor", b, "--mask_files", csf, wm, "--merge_method", "union",
            "--num_components", "all", "--regress_poly_degree", "2"]),
        Case("run/acompcor-index", [
            nac, "ACompCor", b, "--mask_files", csf, wm, "--mask_index", "1",
            "--variance_threshold", "0.9", "--high_pass_cutoff", "100"]),
        Case("run/compcor", [nac, "CompCor", b, "--mask_files", k, "--variance_threshold", "0.6"]),
        Case("run/tcompcor", [nac, "TCompCor", b, "--percentile_threshold", "0.1",
                              "--variance_threshold", "0.5", "--repetition_time", "2"]),
        Case("run/tcompcor-mask", [nac, "TCompCor", b, "--mask_files", k, "--num_components",
                                   "all", "--pre_filter", "cosine"]),
        Case("run/robust-acompcor", [nwp, "RobustACompCor", b, "--mask_files", k,
                                     "--variance_threshold", "0.5"]),
        Case("run/robust-tcompcor", [nwp, "RobustTCompCor", b, "--variance_threshold", "0.5"]),
        Case("run/tsnr", [nac, "TSNR", b]),
        Case("run/tsnr-poly", [nac, "TSNR", b, "--regress_poly", "2", "--tsnr_file", "t.nii.gz"]),
        Case("run/fd-fsl", [nac, "FramewiseDisplacement", i("mc.par"), "FSL"]),
        Case("run/fd-options", [nac, "FramewiseDisplacement", i("mc.par"), "SPM", "--radius",
                                "40", "--out_file", "fd.txt"]),
        Case("run/nonsteady", [nac, "NonSteadyStateDetector", b]),
        # niworkflows' confounds
        Case("run/expand-default", [nwc, "ExpandModel", i("confounds.tsv")], by_name=True),
        Case("run/expand-formula", [nwc, "ExpandModel", i("confounds.tsv"), "--model_formula",
                                    "(dd1(rps + wm))^^2", "--output_file", "x.tsv"], by_name=True),
        Case("run/spikes", [nwc, "SpikeRegressors", i("confounds.tsv")]),
        Case("run/spikes-options", [
            nwc, "SpikeRegressors", i("confounds.tsv"), "--fd_thresh", "0.2", "--dvars_thresh",
            "1.0", "--lags", "0", "1", "--output_format", "mask", "--header_prefix", "o"]),
        Case("run/spikes-contiguous", [nwc, "SpikeRegressors", i("confounds.tsv"), "--fd_thresh",
                                       "0.1", "--minimum_contiguous", "2"]),
        Case("run/normalize-fsl", [nwc, "NormalizeMotionParams", i("mc.par"), "--format", "FSL"]),
        Case("run/normalize-spm", [nwc, "NormalizeMotionParams", i("mc.par"), "--format", "SPM"]),
        Case("run/nw-motion", [nwc, "FSLMotionParams", "--xfm_file", i("xfm.txt"),
                               "--boldref_file", i("boldref.nii.gz")]),
        Case("run/nw-rmsd", [nwc, "FSLRMSDeviation", i("boldref.nii.gz"), i("xfm.txt")]),
        Case("run/nw-fd", [nwc, "FramewiseDisplacement", "--in_file", i("motion.tsv.gz"),
                           "--radius", "60"]),
        Case("run/signals", [nwi, "SignalExtraction", "csf", "wm", b, i("csf.nii.gz"),
                             i("wm.nii.gz")]),
        Case("run/signals-options", [nwi, "SignalExtraction", "g", b, k, "--prob_thres", "0.2",
                                     "--out_file", "s.tsv"]),
        Case("run/dilation", [nwm, "BinaryDilation", k, "--radius", "1"]),
        Case("run/subtraction", [nwm, "BinarySubtraction", k, i("small.nii.gz")]),
        Case("run/apply-mask", [nwn, "ApplyMask", i("wm.nii.gz"), k, "--threshold", "0.4"]),
        Case("run/binarize", [nwn, "Binarize", i("wm.nii.gz"), "0.5"]),
        Case("run/add-tsv-header", [nwu, "AddTSVHeader", "std_dvars", i("dvars_std.tsv")]),
        # the field maps' part's niworkflows interfaces
        Case("run/nonsteady-states", ["niworkflows.interfaces.bold", "NonsteadyStatesDetector", b]),
        Case("run/nonsteady-states-options", ["niworkflows.interfaces.bold",
                                              "NonsteadyStatesDetector", b, "--n_volumes", "10",
                                              "--zero_dummy_masked", "5"]),
        Case("run/intensity-clip", [nwn, "IntensityClip", i("boldref.nii.gz")]),
        Case("run/intensity-clip-options", [nwn, "IntensityClip", i("boldref.nii.gz"), "--p_min",
                                            "10", "--p_max", "90", "--dtype", "float32",
                                            "--invert"]),
        Case("run/merge-series", [nwn, "MergeSeries", "--in_files", i("boldref.nii.gz"),
                                  i("boldref.nii.gz")]),
        Case("run/reorient", [nwn, "ReorientImage", i("boldref.nii.gz"), "--target_orientation",
                              "LPS"]),
        Case("run/reorient-target", [nwn, "ReorientImage", i("boldref.nii.gz"), "--target_file",
                                     i("mask.nii.gz")]),
        Case("run/sampling-reference", [nwn, "GenerateSamplingReference", i("mask.nii.gz"),
                                        i("boldref.nii.gz")]),
        Case("fix/sampling-xform", [nwn, "GenerateSamplingReference", i("mask.nii.gz"),
                                    i("boldref.nii.gz"), "--xform_code", "4"],
             expect="fix:PIPE-82",
             api=(nwn, "GenerateSamplingReference", {"fixed_image": i("mask.nii.gz"),
                                                     "moving_image": i("boldref.nii.gz"),
                                                     "xform_code": 4})),
        Case("fix/regrid", [nwn, "RegridToZooms", i("boldref.nii.gz"), "3,3,3"],
             expect="fix:PIPE-82",
             api=(nwn, "RegridToZooms", {"in_file": i("boldref.nii.gz"), "zooms": (3.0, 3.0, 3.0)})),
        Case("fix/regrid-smooth", [nwi, "RegridToZooms", i("boldref.nii.gz"), "3,3,3", "--smooth",
                                   "True", "--order", "1"],
             expect="fix:PIPE-82",
             api=(nwi, "RegridToZooms", {"in_file": i("boldref.nii.gz"), "zooms": (3.0, 3.0, 3.0),
                                         "smooth": True, "order": 1})),
        # fMRIPrep's confounds
        Case("run/motion", [fmc, "FSLMotionParams", "--xfm_file", i("xfm.txt"), "--boldref_file",
                            i("boldref.nii.gz")]),
        Case("run/rmsd", [fmc, "FSLRMSDeviation", i("boldref.nii.gz"), i("xfm.txt")]),
        Case("run/fd", [fmc, "FramewiseDisplacement", "--in_file", i("motion.tsv.gz")]),
        Case("run/filter-dropped", [fmc, "FilterDropped", "--in_file", i("acompcor_metadata.tsv")]),
        Case("run/rename-acompcor", [fmc, "RenameACompCor", "--components_file", i("acompcor.tsv"),
                                     "--metadata_file", i("acompcor_metadata.tsv")]),
        Case("run/gather", [
            fmc, "GatherConfounds", "--signals", i("signals.tsv"), "--dvars", i("dvars.tsv"),
            "--std_dvars", i("std_dvars.tsv"), "--fd", i("fd.tsv.gz"), "--rmsd", i("rmsd.tsv.gz"),
            "--tcompcor", i("tcompcor.tsv"), "--acompcor", i("acompcor.tsv"), "--cos_basis",
            i("cosine.tsv"), "--motion", i("motion.tsv.gz")]),
        Case("run/gather-some", [fmc, "GatherConfounds", "--signals", i("signals.tsv"), "--fd",
                                 i("fd.tsv.gz")]),
        # PIPE-81 and PIPE-82: refused by nipype's command line, corrected by default
        Case("fix/num-components", [nac, "ACompCor", b, "--mask_files", k, "--num_components", "3"],
             expect="fix:PIPE-81",
             api=(nac, "ACompCor", {"realigned_file": b, "mask_files": [k], "num_components": 3})),
        Case("fix/save-files", [nac, "ACompCor", b, "--mask_files", k, "--variance_threshold",
                                "0.5", "--save_pre_filter", "pre.tsv", "--save_metadata", "True"],
             expect="fix:PIPE-81",
             api=(nac, "ACompCor", {"realigned_file": b, "mask_files": [k],
                                    "variance_threshold": 0.5, "save_pre_filter": "pre.tsv",
                                    "save_metadata": True})),
        Case("fix/float-radius", [nac, "FramewiseDisplacement", i("mc.par"), "FSL", "--radius",
                                  "40.5"],
             expect="fix:PIPE-82",
             api=(nac, "FramewiseDisplacement", {"in_file": i("mc.par"),
                                                 "parameter_source": "FSL", "radius": 40.5})),
        # argparse's error names the corrected type (float, nipype's int): reproduce compared
        Case("fix/invalid-float", [nac, "FramewiseDisplacement", i("mc.par"), "FSL", "--radius",
                                   "x"], expect="reproduce"),
        Case("fix/pre-filter-false", [nac, "TCompCor", b, "--pre_filter", "False",
                                      "--num_components", "all"],
             expect="fix:PIPE-82",
             api=(nac, "TCompCor", {"realigned_file": b, "pre_filter": False,
                                    "num_components": "all"})),
        Case("fix/acompcor-masks", [fmc, "aCompCorMasks", "2,2,2.5", "--in_vfs", i("gm.nii.gz"),
                                    i("wm.nii.gz"), i("csf.nii.gz")],
             expect="fix:PIPE-82",
             api=(fmc, "aCompCorMasks", {"bold_zooms": (2.0, 2.0, 2.5),
                                         "in_vfs": [i("gm.nii.gz"), i("wm.nii.gz"),
                                                    i("csf.nii.gz")]})),
        Case("fix/dict-merge", [nwu, "DictMerge", "--in_dicts", '{"a": 1, "c": {"x": 1}}',
                                '{"b": 2, "c": {"y": 2}}'],
             expect="fix:PIPE-81",
             api=(nwu, "DictMerge", {"in_dicts": [{"a": 1, "c": {"x": 1}},
                                                  {"b": 2, "c": {"y": 2}}]})),
        Case("fix/tsv2json", [nwu, "TSV2JSON", i("acompcor_metadata.tsv"), "component",
                              "--output", "m.json", "--drop_columns", '["mask"]'],
             expect="fix:PIPE-81",
             api=(nwu, "TSV2JSON", {"in_file": i("acompcor_metadata.tsv"),
                                    "index_column": "component", "output": "m.json",
                                    "drop_columns": ["mask"]})),
    ]  # fmt: skip
    if tier == "full":
        cs += real_cases()
    return cs
