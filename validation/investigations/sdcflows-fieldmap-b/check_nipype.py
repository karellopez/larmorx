# SPDX-License-Identifier: Apache-2.0
"""nipype's command lines (``larmorx._nipype``) against every ``command.txt`` the fMRIPrep
25.2.6 oracle runs hold for the interfaces larmorx formats: ``antsRegistration`` (niworkflows'
``FixHeaderRegistration``), ``antsApplyTransforms`` (nipype's and niworkflows'
``FixHeaderApplyTransforms``), ``N4BiasFieldCorrection``, ``ImageMath``, ``DenoiseImage``,
``bet``, ``fslmaths`` (``BinaryMaths``), ``3dAutomask`` and ``topup`` (its encoding file
too). Each node's inputs are read from its report (``_report/report.rst``, "Execution Inputs"),
given to larmorx's formatter with the node's directory, and the line compared character for
character with the node's ``command.txt``. The container's ``/work`` and ``/out`` are the run's
``work`` and ``out`` folders: the inputs are given with those (TOPUP reads its input's volume
count, and checks its output folder exists, as nipype does), and the line is written back with
the container's names before the comparison.

Only nodes of nipype's interfaces are compared: an interface that builds its command itself
(niworkflows' ``RobustMNINormalization``: ``norm``; fMRIPrep's own ANTs calls) is recognised by its
report's module and counted apart (``other interface``). Runs whose queue line is not ``exit 0``
yet are skipped node by node (a node without its result file).

    python -P check_nipype.py [--runs ds002422,...] [--programs antsRegistration,bet,...]

Prints one line per program and interface module (nodes, identical), the first difference of
each, and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import ast
import os
import re
import sys
from collections import defaultdict
from pathlib import Path

from larmorx import _nipype

W = Path(os.environ.get("LARMORX_WORKSPACE", "."))
RUNS = W / "oracles" / "fmriprep-25.2.6"

#: program -> (formatter, the interface modules nipype names in the report that it formats).
FORMATTERS = {
    "antsRegistration": (_nipype.registration, ("fixes", "ants")),
    "antsApplyTransforms": (_nipype.apply_transforms, ("fixes", "ants")),
    "N4BiasFieldCorrection": (_nipype.n4_bias_field_correction, ("ants", "fixes")),
    "ImageMath": (_nipype.image_math, ("ants",)),
    "DenoiseImage": (_nipype.denoise_image, ("ants", "utils")),
    "bet": (_nipype.bet, ("fsl",)),
    "fslmaths": (_nipype.binary_maths, ("fsl",)),
    "3dAutomask": (_nipype.automask, ("afni",)),
    "topup": (_nipype.topup, ("fsl",)),
}

#: Report entries that are not inputs of the interface (nipype's trait helpers, the environment
#: nipype sets from other inputs).
NOT_INPUTS = re.compile(r"^(environ|.*_item_trait|.*_stage_trait)$")


def report_inputs(node: Path) -> tuple[str, dict[str, object]]:
    """The interface's module and its execution inputs (literal values; undefined ones left
    out)."""
    text = (node / "_report" / "report.rst").read_text(encoding="utf-8")
    first = text.splitlines()[0]
    found = re.findall(r"\((\w+)\)\s*$", first.strip()) or re.findall(r"^Node: (\w+)\s*$", first)
    module = found[-1] if found else "?"
    section = text.split("Execution Inputs")[-1].split("Execution Outputs")[0]
    out: dict[str, object] = {}
    for name, value in re.findall(r"^\* (\w+) : (.*)$", section, flags=re.M):
        if value == "<undefined>" or NOT_INPUTS.match(name):
            continue
        try:
            out[name] = ast.literal_eval(value)
        except (ValueError, SyntaxError):
            out[name] = value
    return module, out


def node_dirs(runs: list[str] | None) -> list[tuple[str, Path, Path]]:
    found = []
    for run in sorted(RUNS.iterdir()):
        if runs and not any(run.name.startswith(r) for r in runs):
            continue
        work = run / "work"
        if not work.is_dir():
            continue
        for cmd in sorted(work.rglob("command.txt")):
            node = cmd.parent
            if not list(node.glob("result_*.pklz")):
                continue
            found.append((run.name, work, node))
    return found


def host(value, roots: dict[str, str]):
    """``value`` with the container's ``/work/`` and ``/out/`` paths made the run's folders."""
    if isinstance(value, str):
        for container, real in roots.items():
            if value.startswith(container):
                return real + value[len(container) :]
        return value
    if isinstance(value, list):
        return [host(v, roots) for v in value]
    if isinstance(value, tuple):
        return tuple(host(v, roots) for v in value)
    if isinstance(value, dict):
        return {k: host(v, roots) for k, v in value.items()}
    return value


def nipype_encfile(inputs: dict) -> str:
    """The encoding file nipype's ``TOPUP`` writes for these inputs (in a temporary folder; the
    input file named is this script, which exists: only its name is used)."""
    import tempfile

    from nipype.interfaces.fsl import TOPUP

    with tempfile.TemporaryDirectory() as tmp:
        cwd = os.getcwd()
        os.chdir(tmp)
        try:
            t = TOPUP(
                in_file=__file__,
                encoding_direction=inputs["encoding_direction"],
                readout_times=inputs["readout_times"],
            )
            return Path(t._generate_encfile()).read_text(encoding="utf-8")
        finally:
            os.chdir(cwd)


# ------------------------------------------------------------------------------------------
# synthetic: nipype's own interfaces (this environment's nipype) with options the runs do not use


def synthetic_cases(tmp: Path) -> list[tuple[str, str, dict, str]]:
    """(label, program, inputs, nipype's interface path) with the input files made in ``tmp``."""
    f = {}
    for name in (
        "in.nii.gz",
        "ref.nii.gz",
        "mask.nii.gz",
        "mask2.nii",
        "t.mat",
        "w.nii.gz",
        "op.nii",
    ):
        (tmp / name).write_bytes(b"")
        f[name] = str(tmp / name)
    reg = dict(
        fixed_image=[f["ref.nii.gz"]],
        moving_image=[f["in.nii.gz"]],
        metric=["Mattes", ["CC", "MI"]],
        metric_weight=[1.0, [0.5, 0.5]],
        radius_or_number_of_bins=[32, [4, 32]],
        sampling_strategy=["Regular", [None, "Random"]],
        sampling_percentage=[0.25, [None, 0.3]],
        transforms=["Affine", "BSplineSyN"],
        transform_parameters=[(0.1,), (0.1, 26, 0, 3)],
        number_of_iterations=[[100, 50], [20]],
        smoothing_sigmas=[[2, 1], [0]],
        shrink_factors=[[2, 1], [1]],
        convergence_threshold=[1e-6],
        convergence_window_size=[10, 5],
    )
    return [
        (
            "registration: warped, inverse, no collapse, masks per stage",
            "antsRegistration",
            {
                **reg,
                "output_warped_image": True,
                "output_inverse_warped_image": "inv.nii.gz",
                "collapse_output_transforms": False,
                "fixed_image_masks": [f["mask.nii.gz"], "NULL"],
                "initial_moving_transform": [f["t.mat"]],
                "invert_initial_moving_transform": [True],
                "use_histogram_matching": [False, True],
                "interpolation": "BSpline",
                "interpolation_parameters": (5,),
                "random_seed": 7,
                "float": True,
                "verbose": True,
                "args": "--minc 0",
            },
            "nipype.interfaces.ants.Registration",
        ),
        (
            "registration: composite, warped file, sigma units unset, moving mask only",
            "antsRegistration",
            {
                **reg,
                "write_composite_transform": True,
                "output_warped_image": "w.nii.gz",
                "moving_image_masks": f["mask2.nii"],
                "initial_moving_transform_com": 0,
                "save_state": "state.h5",
                "winsorize_upper_quantile": 0.9,
            },
            "nipype.interfaces.ants.Registration",
        ),
        (
            "apply_transforms: flags, gaussian, composite warp, 4D type",
            "antsApplyTransforms",
            {
                "input_image": f["in.nii.gz"],
                "reference_image": f["ref.nii.gz"],
                "transforms": [f["t.mat"], f["w.nii.gz"]],
                "invert_transform_flags": [True, False],
                "interpolation": "Gaussian",
                "interpolation_parameters": (1.0, 1.0),
                "dimension": 3,
                "input_image_type": 3,
                "default_value": 2.5,
                "output_image": "out.nii.gz",
                "print_out_composite_warp_file": True,
                "float": True,
            },
            "nipype.interfaces.ants.ApplyTransforms",
        ),
        (
            "n4: bias, order, mask, sharpening",
            "N4BiasFieldCorrection",
            {
                "input_image": f["in.nii.gz"],
                "mask_image": f["mask.nii.gz"],
                "save_bias": True,
                "bspline_fitting_distance": 300,
                "bspline_order": 5,
                "n_iterations": [50, 40],
                "convergence_threshold": 1e-7,
                "shrink_factor": 2,
                "histogram_sharpening": (0.2, 0.01, 100),
                "rescale_intensities": True,
                "dimension": 3,
            },
            "nipype.interfaces.ants.N4BiasFieldCorrection",
        ),
        (
            "image_math: PadImage",
            "ImageMath",
            {
                "op1": f["in.nii.gz"],
                "operation": "PadImage",
                "op2": "5",
                "output_image": "pad.nii.gz",
            },
            "nipype.interfaces.ants.ImageMath",
        ),
        (
            "image_math: MD with a file",
            "ImageMath",
            {"op1": f["in.nii.gz"], "operation": "MD", "op2": "2", "dimension": 3, "args": "-v"},
            "nipype.interfaces.ants.ImageMath",
        ),
        (
            "denoise: noise image, Rician",
            "DenoiseImage",
            {
                "input_image": f["in.nii.gz"],
                "save_noise": True,
                "noise_model": "Rician",
                "shrink_factor": 2,
                "dimension": 3,
                "verbose": True,
            },
            "nipype.interfaces.ants.DenoiseImage",
        ),
        (
            "bet: every flag",
            "bet",
            {
                "in_file": f["in.nii.gz"],
                "frac": 0.3,
                "vertical_gradient": -0.1,
                "mask": True,
                "outline": True,
                "skull": True,
                "mesh": True,
                "radius": 70,
                "center": [10, 20, 30],
                "threshold": True,
                "robust": True,
            },
            "nipype.interfaces.fsl.BET",
        ),
        (
            "bet: surfaces, named output",
            "bet",
            {
                "in_file": f["in.nii.gz"],
                "surfaces": True,
                "out_file": "brain.nii.gz",
                "no_output": True,
            },
            "nipype.interfaces.fsl.BET",
        ),
        (
            "binary maths: value, data types",
            "fslmaths",
            {
                "in_file": f["in.nii.gz"],
                "operation": "div",
                "operand_value": 2.5,
                "internal_datatype": "float",
                "output_datatype": "short",
                "nan2zeros": True,
            },
            "nipype.interfaces.fsl.BinaryMaths",
        ),
        (
            "automask: AFNI type, erode, clfrac",
            "3dAutomask",
            {"in_file": f["in.nii.gz"], "erode": 1, "clfrac": 0.4},
            "nipype.interfaces.afni.Automask",
        ),
        (
            "volreg: RobustAverage's call",
            "3dvolreg",
            {
                "in_file": f["in.nii.gz"],
                "args": "-twopass",
                "zpad": 4,
                "interp": "Fourier",
                "outputtype": "NIFTI_GZ",
            },
            "nipype.interfaces.afni.Volreg",
        ),
        (
            "volreg: base, weight, tshift",
            "3dvolreg",
            {
                "in_file": f["in.nii.gz"],
                "basefile": f["ref.nii.gz"],
                "in_weight_volume": (f["w.nii.gz"], 2),
                "timeshift": True,
                "copyorigin": True,
                "verbose": True,
                "outputtype": "NIFTI",
            },
            "nipype.interfaces.afni.Volreg",
        ),
        (
            "topup: options",
            "topup",
            {
                "in_file": f["in.nii.gz"],
                "encoding_direction": ["x", "x-", "z"],
                "readout_times": [0.05],
                "config": "b02b0_2.cnf",
                "warp_res": 10.0,
                "subsamp": 2,
                "fwhm": 8.0,
                "max_iter": 5,
                "reg_lambda": 0.005,
                "ssqlambda": 1,
                "regmod": "bending_energy",
                "estmov": 1,
                "minmet": 0,
                "splineorder": 3,
                "numprec": "double",
                "interp": "spline",
                "scale": 1,
                "regrid": 1,
            },
            "nipype.interfaces.fsl.TOPUP",
        ),
    ]


SYNTH = {
    "antsRegistration": _nipype.registration,
    "antsApplyTransforms": _nipype.apply_transforms,
    "N4BiasFieldCorrection": _nipype.n4_bias_field_correction,
    "ImageMath": _nipype.image_math,
    "DenoiseImage": _nipype.denoise_image,
    "bet": _nipype.bet,
    "fslmaths": _nipype.binary_maths,
    "3dAutomask": _nipype.automask,
    "3dvolreg": _nipype.volreg,
    "topup": _nipype.topup,
}


def check_synthetic(tally) -> None:
    """Each case's line and outputs against nipype's own ``cmdline`` and ``_list_outputs()``."""
    import importlib
    import tempfile

    import nibabel as nb
    import numpy as np

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp = Path(tmpdir)
        cases = synthetic_cases(tmp)
        nb.save(nb.Nifti1Image(np.zeros((2, 2, 2, 3), np.float32), np.eye(4)), tmp / "in.nii.gz")
        cwd = os.getcwd()
        os.chdir(tmp)
        try:
            for label, program, inputs, path in cases:
                mod, cls = path.rsplit(".", 1)
                iface = getattr(importlib.import_module(mod), cls)(**inputs)
                theirs = iface.cmdline
                outs = {k: v for k, v in iface._list_outputs().items() if v not in (None, [])}
                cmd = SYNTH[program](cwd=tmp, **inputs)
                e = tally[(program, "synthetic, nipype's own")]
                e[0] += 1
                ours_out = {k: v for k, v in cmd.outputs.items() if v not in (None, [])}
                same_out = all(
                    ours_out.get(k) == v for k, v in outs.items() if not str(v).startswith("<")
                )
                if cmd.cmdline == theirs and same_out:
                    e[1] += 1
                elif not e[2]:
                    e[2] = (
                        f"{label}: {first_difference(cmd.cmdline, theirs)}"
                        if cmd.cmdline != theirs
                        else f"{label}: outputs {ours_out} vs {outs}"
                    )
        finally:
            os.chdir(cwd)


def first_difference(a: str, b: str) -> str:
    i = next((k for k, (x, y) in enumerate(zip(a, b, strict=False)) if x != y), min(len(a), len(b)))
    return f"at {i}: ours {a[max(0, i - 40) : i + 60]!r} vs nipype's {b[max(0, i - 40) : i + 60]!r}"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", default="")
    ap.add_argument("--programs", default="")
    a = ap.parse_args()
    runs = [r for r in a.runs.split(",") if r] or None
    programs = [p for p in a.programs.split(",") if p] or list(FORMATTERS)
    tally: dict[tuple[str, str], list] = defaultdict(lambda: [0, 0, ""])
    skipped: dict[tuple[str, str], int] = defaultdict(int)
    if not runs:
        check_synthetic(tally)
    for run, work, node in node_dirs(runs):
        line = (node / "command.txt").read_text(encoding="utf-8").rstrip("\n")
        program = line.split(" ", 1)[0]
        if program not in programs or program not in FORMATTERS:
            continue
        formatter, modules = FORMATTERS[program]
        module, inputs = report_inputs(node)
        if module not in modules:
            skipped[(program, module)] += 1
            continue
        roots = {"/work/": f"{work}/", "/out/": f"{work.parent / 'out'}/"}
        label = f"{run}/{node.relative_to(work).as_posix()}"
        e = tally[(program, module)]
        e[0] += 1
        try:
            cmd = formatter(cwd=node, **host(inputs, roots))
            ours = cmd.cmdline
            for container, real in roots.items():
                ours = ours.replace(real, container)
        except Exception as exc:
            if not e[2]:
                e[2] = f"{label}: {type(exc).__name__}: {exc}"
            continue
        ok = ours == line
        if ok and program == "topup":
            # the run kept no encoding file: nipype's own (this environment's nipype) instead
            for text in cmd.files.values():
                ok = ok and nipype_encfile(inputs) == text
                tally[("topup", "encoding file")][0] += 1
                tally[("topup", "encoding file")][1] += int(ok)
        if ok:
            e[1] += 1
        elif not e[2]:
            e[2] = f"{label}: {first_difference(ours, line)}"
    bad = 0
    for (program, module), (n, ok, first) in sorted(tally.items()):
        bad += n - ok
        print(
            f"{program} ({module}): {n} nodes, identical {ok}"
            + (f"  (first: {first})" if first else "")
        )
    for (program, module), n in sorted(skipped.items()):
        print(f"INFO {program} ({module}): {n} nodes of another interface, not compared")
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
