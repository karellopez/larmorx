# SPDX-License-Identifier: Apache-2.0
"""antsRegistration's evidence: sMRIPrep's T1w-to-MNI registration of a real T1w (the record's
``real-syn-small`` case: Rigid, Affine and SyN on every third voxel of the images), the same
command with antsRegistrationSyN.sh's ``BSplineSyN[0.1,26,0,3]`` for its SyN stage
(``real-bsplinesyn-small``) and with a time-varying velocity field
(``real-time-varying-small``), by ANTs 2.6.5 (as built, with correctly rounded maths, at 2, 4
and 8 threads) and by larmorx (1 and 12 threads, default and ``reproduce`` modes); the record's
cases; ANTs' own thread band; what the corrections decided on 2026-10-21 change (the
``ants-registration-corrections`` record).

The arguments are fMRIPrep 25.2.6's own (its ``command.txt`` in the oracle run's working
directory), built by the parity suite (``parity.ants_registration._fmriprep_args``): the masked
template and T1w that nipype did not keep are remade from the run's outputs, as in the record.
"""

from __future__ import annotations

import re
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "ants-registration"
RUN = "ds000005-sub-01-noreconall"
NODE = "anat_fit_wf/register_template_wf/_template_MNI152NLin2009cAsym/registration"
SPEC = f"{RUN}/sub_01_wf/{NODE}"
#: ANTs' thread band, as the record measures it (parity.ants_registration.BAND_THREADS)
BAND = (2, 4, 8)
LX_THREADS = (1, 12)
WARPED = "ants_t1_to_mni_Warped.nii.gz"
COMPOSITE = "transformComposite.h5"
EXAMPLES = {
    # id -> (cache case, record case, stage label, description)
    "syn": (
        "fmriprep-ds000005-template-syn-small",
        "real-syn-small/ds000005-sub-01/template",
        "SyN[0.1,3,0]",
        "sMRIPrep's T1w-to-MNI registration (Rigid, Affine, SyN) of ds000005 sub-01 on every "
        "third voxel: ANTs 2.6.5 (1, 2, 4, 8 threads; CORE-MATH) and larmorx",
    ),
    "bsplinesyn": (
        "fmriprep-ds000005-template-bsplinesyn-small",
        "real-bsplinesyn-small/ds000005-sub-01/template",
        "BSplineSyN[0.1,26,0,3]",
        "sMRIPrep's T1w-to-MNI registration with BSplineSyN[0.1,26,0,3] for its SyN stage, "
        "ds000005 sub-01 on every third voxel: ANTs 2.6.5 (1, 2, 4, 8 threads; CORE-MATH) and "
        "larmorx",
    ),
    "tvf": (
        "fmriprep-ds000005-template-tvf-small",
        "real-time-varying-small/ds000005-sub-01/template",
        "TimeVaryingVelocityField[0.5,4,3,0,0,0]",
        "sMRIPrep's T1w-to-MNI registration with TimeVaryingVelocityField[0.5,4,3,0,0,0] for its "
        "SyN stage, ds000005 sub-01 on every third voxel: ANTs 2.6.5 (1, 2, 4, 8 threads; "
        "CORE-MATH) and larmorx",
    ),
}
#: Each example's title, summary, stages line and the stage that replaces SyN in the command.
TEXTS = {
    "syn": (
        "sMRIPrep's T1w-to-MNI registration, by ANTs and larmorx",
        "fMRIPrep 25.2.6's template registration of OpenNeuro ds000005 sub-01 (its own "
        "arguments, from the oracle run's command.txt) on every third voxel of its images, "
        "as the record's standard tier runs it, given to ANTs 2.6.5 and to larmorx. Choose "
        "the output (the warped T1w or a component of the displacement field), the plane "
        "and the panels; Checkerboard and Swipe compare the template with the warped T1w.",
        "Rigid, Affine and SyN, exactly fMRIPrep's",
        None,
    ),
    "bsplinesyn": (
        "The same registration with a B-spline SyN stage",
        "The command above with its SyN stage replaced by BSplineSyN[0.1,26,0,3], as "
        "antsRegistrationSyN.sh's `-t b` runs it (the record's real-bsplinesyn-small case): "
        "the update field and the total field are B-spline smoothed.",
        "Rigid and Affine as fMRIPrep's, then antsRegistrationSyN.sh's "
        "BSplineSyN[0.1,26,0,3] (its `-t b`) in place of SyN[0.1,3,0]",
        "BSplineSyN[0.1,26,0,3]",
    ),
    "tvf": (
        "The same registration with a time-varying velocity field",
        "The command above with its SyN stage replaced by TimeVaryingVelocityField[0.5,4,3,0,0,0] "
        "(the record's real-time-varying-small case): a velocity field over space and four time "
        "points, its displacement the flow from time 0 to 1 integrated by fourth-order "
        "Runge–Kutta, the update smoothed in space with SyN's variance of 3 voxels².",
        "Rigid and Affine as fMRIPrep's, then TimeVaryingVelocityField[0.5,4,3,0,0,0] in place "
        "of SyN[0.1,3,0], with SyN's levels, iterations and CC metric",
        "TimeVaryingVelocityField[0.5,4,3,0,0,0]",
    ),
}


def _args(example: str, out: Path, tmp: Path) -> list[str]:
    from larmorx_validation.parity.ants_registration import (
        SMALL,
        _fmriprep_args,
        _replace_syn,
    )

    args = _fmriprep_args(SPEC, out, tmp=tmp, stand_ins=True, small=SMALL)
    stage = TEXTS[example][3]
    return _replace_syn(args, stage) if stage else args


def _compute(example: str):
    def compute(e: fcache.Entry) -> None:
        from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload
        from larmorx_validation.parity.ants_registration import SEED

        b, lib = ants_bin(), crmath_preload()
        if b is None or lib is None:
            raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload is missing")
        tmp = e.dir / "inputs"
        tmp.mkdir(exist_ok=True)
        runs = [("ants-1", 1, "cr0"), ("ants-1-cr", 1, "cr1")]
        runs += [(f"ants-{t}", t, "cr0") for t in BAND]
        for label, threads, cr in runs:
            out = e.dir / label
            out.mkdir(exist_ok=True)
            env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads), "ANTS_RANDOM_SEED": SEED}
            if cr == "cr1":
                env["LD_PRELOAD"] = str(lib)
            e.run(
                label,
                [str(b / "antsRegistration"), *_args(example, out, tmp)],
                env=env,
                unset=common.CLEAN_ENV,
            )
        for threads in LX_THREADS:
            for mode in ("fix", "reproduce"):
                label = f"larmorx-{threads}" + ("-reproduce" if mode == "reproduce" else "")
                out = e.dir / label
                out.mkdir(exist_ok=True)
                e.run(
                    label,
                    fcache.larmorx_cli("ants", "antsRegistration", *_args(example, out, tmp)),
                    env={
                        "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                        "ANTS_RANDOM_SEED": SEED,
                        "LARMORX_UPSTREAM_DEFECTS": mode,
                    },
                    unset=common.CLEAN_ENV,
                )
        # the moving T1w on the template's grid before registration: no transform (its own
        # world coordinates), linear interpolation, by ANTs as built
        fixed, moving = _inputs(e.dir)
        e.input("fixed (MNI152NLin2009cAsym × brain mask, every third voxel)", fixed)
        e.input("moving (fMRIPrep's T1w × brain mask, every third voxel)", moving)
        e.run(
            "moving-unregistered",
            [
                str(b / "antsApplyTransforms"),
                "-d",
                "3",
                "--float",
                "1",
                "-i",
                str(moving),
                "-r",
                str(fixed),
                "-n",
                "Linear",
                "-o",
                str(e.dir / "moving-unregistered.nii.gz"),
            ],
            env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"},
            unset=common.CLEAN_ENV,
        )
        command = (
            (common.fmriprep_run(RUN) / "work" / "fmriprep_25_2_wf" / "sub_01_wf" / NODE)
            / "command.txt"
        ).read_text(encoding="utf-8")
        e.finish(
            versions={"ants": ants_version()},
            threads={
                "ants": [1, *BAND],
                "ants_crmath": 1,
                "larmorx": list(LX_THREADS),
                "fmriprep_run": 8,
            },
            seed=f"ANTS_RANDOM_SEED={SEED}",
            command=" ".join(command.split()),
            stage=EXAMPLES[example][2],
            settings=f"fMRIPrep 25.2.6's template registration of {RUN} (its command.txt), its "
            "images on every third voxel, as the record's "
            f"{EXAMPLES[example][1]} case; larmorx in both modes",
        )

    return compute


def _inputs(d: Path) -> tuple[Path, Path]:
    small = d / "inputs" / "small"
    fixed = sorted(small.glob("*_fixed_masked.nii.gz"))
    moving = sorted(small.glob("*_moving_masked.nii.gz"))
    if not fixed or not moving:
        raise FileNotFoundError(f"{small}: the small inputs")
    return fixed[0], moving[0]


def entry(ctx: core.Context, example: str) -> fcache.Entry:
    case, _, _, description = EXAMPLES[example]
    return fcache.entry(ctx.figures_context(), TOOL, case, description, _compute(example))


# ----------------------------------------------------------------------------------- export

#: panel id, cache run, role, label, sub-label; the closest references first (larmorx's
#: outputs are stored as they are, the others as their bits' difference from the closest)
RUNS = [
    ("larmorx", "larmorx-1-reproduce", "larmorx", "larmorx (port), 1 thread", "reproduce mode"),
    ("larmorx-12", "larmorx-12-reproduce", "larmorx", "larmorx (port), 12 threads", "reproduce"),
    ("larmorx-fix", "larmorx-1", "larmorx", "larmorx (port), 1 thread", "default mode"),
    ("larmorx-12-fix", "larmorx-12", "larmorx", "larmorx (port), 12 threads", "default mode"),
    (
        "ants-1-cr",
        "ants-1-cr",
        "reference",
        "ANTs 2.6.5, 1 thread, correctly rounded maths",
        "the same binary, CORE-MATH preloaded",
    ),
    (
        "ants-1",
        "ants-1",
        "original",
        "ANTs 2.6.5, 1 thread, as built",
        "the repository's oracle build (GCC 14.3), glibc's maths",
    ),
    ("ants-8", "ants-8", "original", "ANTs 2.6.5, 8 threads", "as built (fMRIPrep ran 8)"),
]
OUTPUTS = [
    {"id": "warped", "label": "Warped T1w", "unit": ""},
    {"id": "dx", "label": "Field x", "unit": "mm"},
    {"id": "dy", "label": "Field y", "unit": "mm"},
    {"id": "dz", "label": "Field z", "unit": "mm"},
]
PARAM_NAMES = [
    "matrix 1,1",
    "matrix 1,2",
    "matrix 1,3",
    "matrix 2,1",
    "matrix 2,2",
    "matrix 2,3",
    "matrix 3,1",
    "matrix 3,2",
    "matrix 3,3",
    "translation x",
    "translation y",
    "translation z",
]


def _run_arrays(d: Path) -> dict[str, np.ndarray]:
    f = common.itk_field(d / COMPOSITE)
    return {
        "warped": common.load(str(d / WARPED)),
        "dx": np.ascontiguousarray(f[..., 0]),
        "dy": np.ascontiguousarray(f[..., 1]),
        "dz": np.ascontiguousarray(f[..., 2]),
    }


def _diffs() -> list[dict]:
    d = common.d
    out = [
        d(
            "lx-ants1",
            "larmorx",
            "ants-1",
            "main",
            "larmorx − ANTs 2.6.5, 1 thread, as built",
            common.ROLE_LX,
        ),
        d(
            "lx-cr",
            "larmorx",
            "ants-1-cr",
            "main",
            "larmorx − ANTs 2.6.5, 1 thread, correctly rounded maths",
            common.ROLE_LX,
        ),
        d(
            "lx12-lx1",
            "larmorx-12",
            "larmorx",
            "check",
            "larmorx, 12 threads − 1 thread",
            common.ROLE_SELF,
        ),
        d(
            "lx-fix",
            "larmorx-fix",
            "larmorx",
            "check",
            "larmorx, default − reproduce mode",
            common.ROLE_SELF,
        ),
        d(
            "lx12-fix",
            "larmorx-12-fix",
            "larmorx-fix",
            "check",
            "larmorx default mode, 12 threads − 1 thread",
            common.ROLE_SELF,
        ),
    ]
    out += [
        d(
            f"ants{t}-ants1",
            f"ants-{t}",
            "ants-1",
            "variability",
            f"ANTs 2.6.5, {t} threads − 1 thread",
            common.ROLE_ORIG,
        )
        for t in BAND
    ]
    out.append(
        d(
            "cr-ants1",
            "ants-1-cr",
            "ants-1",
            "variability",
            "ANTs 2.6.5 with correctly rounded maths − as built, 1 thread",
            common.ROLE_ORIG,
        )
    )
    return out


def _field_norms(arrays: dict[str, dict], a: str, b: str, brain: np.ndarray) -> dict:
    """The length of the difference between two displacement fields, voxel by voxel (mm),
    over the template's brain."""
    da = np.stack([arrays[a][k] for k in ("dx", "dy", "dz")], -1).astype(np.float64)
    db = np.stack([arrays[b][k] for k in ("dx", "dy", "dz")], -1).astype(np.float64)
    n = np.sqrt(((da - db) ** 2).sum(-1))[brain]
    return {
        "max": float(n.max()),
        "p99": float(np.percentile(n, 99)),
        "rms": float(np.sqrt(np.mean(n * n))),
        "differ": int(np.count_nonzero(n)),
        "voxels": int(n.size),
    }


def _verdict(x: dict, same: dict[str, bool], reg: dict) -> None:
    i = x["id"]
    links = []
    if x["kind"] == "variability":
        x["verdict"] = None
        if i == "cr-ants1":
            x["reason"] = (
                "The same ANTs binary, its glibc functions replaced by correctly rounded ones."
                if not same[i]
                else "glibc's maths plays no part in this run."
            )
            links.append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
        else:
            x["reason"] = (
                "ANTs' result moves with its thread count: its metrics sum per-thread partial "
                "results in an order that depends on the threads."
                if not same[i]
                else "The thread count changes nothing here."
            )
    elif same[i]:
        x["verdict"] = "bit-identical"
        x["reason"] = {
            "lx-cr": "With correctly rounded maths, ANTs computes exactly what larmorx computes: "
            "the warped image and every value of the displacement field.",
            "lx12-lx1": "larmorx gives ANTs' single-thread result on any number of threads.",
            "lx-fix": "No correction of the register applies to this command: both modes give "
            "the same bytes.",
            "lx12-fix": "The same bytes on any number of threads.",
        }.get(i, "The same bytes.")
    elif i == "lx-ants1" and same["lx-cr"]:
        x["verdict"] = "identical-cr"
        x["reason"] = (
            "Only where glibc's exp, log, sin or cos is not correctly rounded: with CORE-MATH "
            "preloaded, the same ANTs binary gives larmorx's output bit for bit (next row). The "
            "change then grows through the optimiser's iterations, as ANTs' own threads do."
        )
        links.append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
    elif i == "lx-fix":
        x["verdict"] = "d13"
        x["reason"] = "A correction of the register changes this command's result (default mode)."
    else:
        x["verdict"] = "open"
        x["reason"] = "Not accounted for by the other runs of this example."
    x["links"] = links


def _params_table(e_dir: Path, tid: str) -> dict:
    """The linear stages' result (the composite's affine, Rigid then Affine collapsed) by each
    run, as stored (float32 with --float 1)."""
    p = {k: common.itk_linear(e_dir / k / COMPOSITE) for k in ("ants-1", "ants-1-cr", "ants-8")}
    lx = common.itk_linear(e_dir / "larmorx-1-reproduce" / COMPOSITE)
    rows = []
    for j, name in enumerate(PARAM_NAMES):
        a, c, b8, mine = p["ants-1"][0][j], p["ants-1-cr"][0][j], p["ants-8"][0][j], lx[0][j]
        rows.append(
            {
                "head": name,
                "cells": [
                    common.cell(float(a), "f32"),
                    common.cell(float(c), "f32"),
                    common.cell(float(mine), "f32"),
                    common.cell(float(mine) - float(a), "sig"),
                    common.cell(float(mine) - float(c), "sig"),
                    common.cell(float(b8) - float(a), "sig"),
                ],
            }
        )
    centre = p["ants-1"][1]
    same_centre = all(np.array_equal(centre, v[1]) for v in (*p.values(), lx))
    return common.table(
        tid,
        "The linear stages' result: the affine of the composite transform (Rigid then Affine, "
        f"collapsed; {p['ants-1'][2]})",
        [
            "Parameter",
            {"label": "ANTs, 1 thread, as built", "num": True},
            {"label": "ANTs, 1 thread, CORE-MATH", "num": True},
            {"label": "larmorx", "num": True},
            {"label": "larmorx − ANTs as built", "num": True},
            {"label": "larmorx − ANTs with CORE-MATH", "num": True},
            {"label": "ANTs 8 threads − 1 thread", "num": True},
        ],
        rows,
        note="The parameters as the files store them (float32: antsRegistration ran with "
        "`--float 1`), printed with the digits that identify each value; the matrix row by "
        "row in ITK's physical space (LPS), the translation in mm. The centre of rotation "
        + (
            f"is the same in every run ({', '.join(f'{v:.6g}' for v in centre)} mm)."
            if same_centre
            else "differs between the runs."
        ),
    )


def _norms_table(arrays: dict, brain: np.ndarray, tid: str) -> dict:
    pairs = [
        ("larmorx − ANTs as built, 1 thread", "larmorx", "ants-1"),
        ("larmorx − ANTs with CORE-MATH, 1 thread", "larmorx", "ants-1-cr"),
        *[(f"ANTs {t} threads − 1 thread", f"ants-{t}", "ants-1") for t in BAND],
        ("ANTs with CORE-MATH − as built, 1 thread", "ants-1-cr", "ants-1"),
    ]
    rows = []
    for label, a, b in pairs:
        n = _field_norms(arrays, a, b, brain)
        rows.append(
            {
                "head": label,
                "cells": [
                    common.cell(n["max"]),
                    common.cell(n["p99"]),
                    common.cell(n["rms"]),
                    common.cell(n["differ"], "count"),
                ],
            }
        )
    return common.table(
        tid,
        "The displacement fields: how far a voxel's displacement moves (the length of the "
        "difference vector, mm), over the template's brain",
        [
            "Comparison",
            {"label": "largest (mm)", "num": True},
            {"label": "99th percentile (mm)", "num": True},
            {"label": "RMS (mm)", "num": True},
            {"label": f"voxels that differ, of {int(brain.sum()):,}", "num": True},
        ],
        rows,
        note="ANTs' own runs at 2, 4 and 8 threads are the yardstick: larmorx's difference from "
        "ANTs as built is the size of a change of ANTs' thread count, and nothing with "
        "correctly rounded maths.",
    )


def _stdout_lines(e_dir: Path, a: str, b: str) -> tuple[int, int]:
    """How many lines of two runs' standard output differ (every iteration's DIAGNOSTIC line
    among them), normalised as the record normalises them (times, addresses and the output
    directory masked; larmorx's notices left out)."""
    from larmorx_validation.parity.ants_registration import _normalise

    def lines(label: str) -> list[str]:
        text = (e_dir / f"{label}.log").read_text(encoding="utf-8")
        out = text.split("--- stdout\n", 1)[1].split("\n--- stderr\n", 1)[0]
        return _normalise(out, e_dir / label)

    la, lb = lines(a), lines(b)
    n = max(len(la), len(lb))
    differ = sum(
        1 for i in range(n) if (la[i] if i < len(la) else None) != (lb[i] if i < len(lb) else None)
    )
    return differ, n


def _example(ctx: core.Context, ex_id: str, reg: dict) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = entry(ctx, ex_id)
    info = e.info()
    fixed, _moving = _inputs(e.dir)
    fimg = nib.load(str(fixed))
    template = common.load(str(fixed))
    brain = template > 0
    show = ndimage.binary_dilation(brain, iterations=1)
    grid = core.Grid.around(fimg, show)
    ex = exm.Example(ctx, grid, show)
    stage = info["stage"]
    ex.add(
        exm.Panel(
            "template",
            "input",
            "MNI152NLin2009cAsym (fixed)",
            "the 1 mm template × its brain mask, every third voxel",
            {"all": template},
        )
    )
    ex.add(
        exm.Panel(
            "moving",
            "input",
            "ds000005 sub-01 T1w (moving), unregistered",
            "fMRIPrep's T1w × brain mask, on this grid without any transform (ANTs, linear)",
            {"all": common.load(str(e.dir / "moving-unregistered.nii.gz"))},
        )
    )
    arrays = {}
    for pid, run, role, label, sub in RUNS:
        arrays[pid] = _run_arrays(e.dir / run)
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    for t in BAND:
        if f"ants-{t}" not in arrays:
            arrays[f"ants-{t}"] = _run_arrays(e.dir / f"ants-{t}")
    diffs = _diffs()
    common.compare_all(ex, diffs, OUTPUTS, full=arrays)
    same = common.identical(diffs)
    for x in diffs:
        _verdict(x, same, reg)
    by = {x["id"]: x for x in diffs}
    for i in ("lx-cr", "lx12-lx1", "lx12-fix"):
        if not same[i]:
            raise AssertionError(f"{TOOL}/{ex_id}: the chain assumes {i} is bit-identical")
    labels = {p.id: (p.label, p.sub, p.role) for p in ex.panels}

    def node(pid, label=None, sub=None):
        lab, s, role = labels[pid]
        return {"label": label or lab, "sub": sub if sub is not None else s, "role": role}

    chain = {
        "title": "What separates ANTs' result at fMRIPrep's 8 threads from larmorx's",
        "steps": [
            {"nodes": [node("ants-8")]},
            {"nodes": [node("ants-1")]},
            {
                "nodes": [
                    node("ants-1-cr"),
                    node(
                        "larmorx",
                        "larmorx (port)",
                        "1 and 12 threads, "
                        + ("default and reproduce" if same["lx-fix"] else "reproduce mode"),
                    ),
                ]
            },
        ],
        "links": [
            {"cause": "1 thread instead of 8", "max_abs": common.max_abs(by["ants8-ants1"])},
            {
                "cause": "correctly rounded maths instead of glibc's",
                "max_abs": common.max_abs(by["lx-ants1"]),
            },
        ],
        "note": "Each box holds runs that give the same bytes, every output (the warped T1w "
        "and the three components of the displacement field); the largest |Δ| of a link is "
        "over all four. ANTs' thread count and glibc's maths are the only two causes, and "
        "larmorx computes ANTs' one-thread result with correctly rounded maths, on any number "
        "of threads.",
    }
    at, _k = common.largest_in_slice(ex, "larmorx", "ants-1", "warped", "axial")
    default = {
        "output": "warped",
        "plane": "axial",
        "voxel": at,
        "panels": ["template", "moving", "ants-1", "larmorx"],
        "diffs": ["lx-ants1", "ants8-ants1", "lx-cr"],
        "compare": {"a": "template", "b": "larmorx"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        OUTPUTS,
        default=default,
        window_panel="larmorx",
        scale="log",
        caption="the crosshair where larmorx and ANTs as built differ most in it; the warped "
        "T1w in the brain's 0.5–99.5 % window; the differences on one symmetric log scale "
        "over six decades below the largest, any non-zero difference tinted.",
    )
    for o in out["outputs"]:
        if o["id"] != "warped":
            o["label"] = {
                "dx": "Field x (LPS)",
                "dy": "Field y (LPS)",
                "dz": "Field z (LPS)",
            }[o["id"]]
    so_a, so_n = _stdout_lines(e.dir, "larmorx-1-reproduce", "ants-1-cr")
    so_b, _ = _stdout_lines(e.dir, "larmorx-1-reproduce", "ants-1")
    vers = info.get("versions", {})
    command = info["command"]
    title, summary, stage_text, _stage = TEXTS[ex_id]
    n_vox = int(np.prod(fimg.shape))
    return {
        "id": f"template-{ex_id}",
        "title": title,
        "summary": summary,
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01's T1w as fMRIPrep preprocessed it "
                f"({common.DS000005['licence']}), × its brain mask; {common.MNI_NOTICE}",
            },
            {"label": "Stages", "text": stage_text},
            {
                "label": "Inputs",
                "text": "as the record's case runs it: the masked template and T1w nipype did "
                "not keep (`fixed_masked`, `moving_masked`) remade from the run's outputs, the "
                "initial transform it did not keep (`transform.mat`) replaced by "
                "`[fixed,moving,1]` (centres of mass), every image on every third voxel",
            },
            {"label": "Command", "text": "`" + _short(command, ex_id) + "`"},
            {
                "label": "Programs",
                "text": f"{vers.get('ants', '?')}, `ANTS_RANDOM_SEED=42`; larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`",
            },
            {
                "label": "Output",
                "text": "standard output (every iteration's DIAGNOSTIC line, normalised as the "
                f"record does): {so_n:,} lines; "
                + (
                    "identical to ANTs with CORE-MATH's"
                    if so_a == 0
                    else f"{so_a:,} differ from ANTs with CORE-MATH"
                )
                + f", {so_b:,} differ from ANTs as built",
            },
            {"label": "Grid", "text": common.describe_grid(fimg, grid.shape)},
        ],
        **out,
        "chain": chain,
        "tables": [
            _params_table(e.dir, f"{ex_id}-linear"),
            _norms_table(arrays, brain, f"{ex_id}-field"),
        ],
        "metrics_note": (
            f"Δ is A − B. Every number is computed in float64 over all {n_vox:,} voxels of the "
            f"{' × '.join(map(str, fimg.shape))} outputs (the field: each component), not only "
            "the brain shown; the range is B's maximum minus its minimum. The rows *the "
            "original against itself* are ANTs' own variability on this input. The field's "
            "components are in mm, in ITK's physical space (LPS)."
        ),
        "provenance": common.provenance(
            info,
            f"{TOOL}/{EXAMPLES[ex_id][0]}",
            [common.DS000005, common.MNI],
            [f"Stage under test: {stage}"],
        ),
    }


def _short(command: str, ex_id: str) -> str:
    """fMRIPrep's command as the parity suite reads it (``_fmriprep_args``: brackets without
    their spaces), its working directory's paths cut to file names."""
    c = re.sub(r"\s+", " ", command).strip()
    c = c.replace("[ ", "[").replace(" ]", "]").replace(", ", ",")
    c = re.sub(r"/work/\S*?/registration/", "", c)
    stage = TEXTS[ex_id][3]
    if stage:
        c = c.replace("--transform SyN[0.1,3.0,0.0]", f"--transform {stage}")
    return c


# ------------------------------------------------------------------------------------ cases


def _cases(ctx: core.Context, rec=None) -> list[dict]:
    from larmorx_validation.parity import ants_registration as suite

    rec = rec or records.load(ctx.repo, TOOL)
    defs = {c.id: c for c in suite.all_cases()}
    examples = {v[1]: f"template-{k}" for k, v in EXAMPLES.items()}
    out = []
    for r in rec.results:
        c = defs.get(r["case"])
        v = common.outcome_verdict(r)
        if v is None:
            rep = r.get("reported", {})
            glibc = sorted(k for k in rep if k.startswith("unmodified ANTs"))
            band = rep.get("thread band")
            if glibc:
                v = (
                    "identical-cr",
                    common.clip(
                        "differs from ANTs as built in "
                        + ", ".join(g.removeprefix("unmodified ANTs: ") for g in glibc[:4])
                        + (" …" if len(glibc) > 4 else "")
                        + "; identical to ANTs with CORE-MATH preloaded (every output)"
                    ),
                )
            else:
                v = ("bit-identical", "exit status, standard output, standard error, every file")
            if band is not None:
                v = (v[0], v[1] + f"; larmorx − ANTs at 1 thread: {band} in a parameter")
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": c.description if c else "",
                "command": _case_command(c) if c else "",
                "results": {
                    "port": {
                        "outcome": r["status"],
                        "verdict": v[0],
                        "checks": f"{r['checks_passed']}/{r['checks_total']}",
                        "reason": v[1],
                    }
                },
                **({"example": examples[r["case"]]} if r["case"] in examples else {}),
            }
        )
    return out


def _case_command(c) -> str:
    def sub(a: str) -> str:
        a = re.sub(r"\{gen:([^}]+)\}", lambda m: f"<generated: {m.group(1)}>", a)
        a = re.sub(r"\{in:([^}]+)\}", lambda m: m.group(1).split("/")[-1], a)
        a = re.sub(
            r"\{(?:missing|gen-points|gen-syn|gen-bsp|gen-transform):([^}]+)\}",
            lambda m: m.group(1),
            a,
        )
        a = re.sub(
            r"\{panel-node(-small)?:([^}]+)\}",
            lambda m: (
                f"<fMRIPrep 25.2.6's command.txt of {m.group(2).split('/')[0]}: "
                f"{'/'.join(m.group(2).split('/')[-2:])}"
                + (", every third voxel" if m.group(1) else "")
                + ">"
            ),
            a,
        )
        a = re.sub(
            r"\{fmriprep[\w-]*:([^}]+)\}",
            lambda m: (
                f"<fMRIPrep 25.2.6's command.txt of {m.group(1).split('/')[0]}: "
                f"{m.group(1).split('/')[-1]}>"
            ),
            a,
        )
        return a.replace("{out}/", "").replace("{out}", ".").replace("{tmp}/", "")

    return common.command_text("antsRegistration", c.args, sub)


def _band(rec) -> dict | None:
    """ANTs' own thread band per case (the record's sentence) next to larmorx's distance from
    ANTs at one thread (each case's reported value)."""
    m = re.search(r"^Thread band, \d+ cases: .*$", rec.text, re.M)
    if not m:
        return None
    band = dict(re.findall(r"`([^`]+)` ([0-9.e+-]+)", m.group(0)))
    dist = {
        r["case"]: r.get("reported", {}).get("thread band")
        for r in rec.results
        if r["case"] in band
    }
    rows = []
    for case, b in band.items():
        d = dist.get(case)
        rows.append(
            {
                "label": case,
                "input": case,
                "code": True,
                "values": {
                    "band": float(b),
                    **({"larmorx": float(d)} if d not in (None, "") else {}),
                },
            }
        )
    vals = [v for r in rows for v in r["values"].values() if v > 0]
    axis, ticks = common.ticks_log(vals)
    return {
        "title": "The largest change in a transform parameter: ANTs at 2, 4 and 8 threads "
        "against its one-thread result, and larmorx against ANTs at one thread (log scale; 0 "
        "at the left edge)",
        "axis": axis,
        "ticks": ticks,
        "unit": "",
        "series": [
            {"id": "band", "label": "ANTs' own band: 2, 4, 8 threads vs 1", "mark": "original"},
            {"id": "larmorx", "label": "larmorx vs ANTs, 1 thread, as built", "mark": "larmorx"},
        ],
        "rows": rows,
        "note": "From the record: its thread-band sentence and each case's reported distance. "
        "Where larmorx differs from ANTs as built, ANTs with correctly rounded maths gives "
        "larmorx's result exactly.",
    }


#: The corrections record's tables: id, title, the cases (record id, label), the columns
#: (check name in the record, column label, format).
CORRECTION_TABLES = [
    (
        "corrections-igdm",
        "IGDM (ANTS-67, ANTS-68, ANTS-70): a rigid registration of the point-set phantoms, whose "
        "moving blocks are shifted by 3.77 mm; the translation error each mode leaves",
        [
            ("igdm/one-neighbour", "given sigmas, one neighbour, central differences"),
            ("igdm/one-neighbour-gradient-sigma", "one neighbour, gradientSigma 1 mm"),
            ("igdm/five-neighbours", "kNeighborhood 5"),
            ("igdm/five-neighbours-gradient-sigma", "kNeighborhood 5, gradientSigma 1.5 mm"),
            ("igdm/estimated-sigmas", "estimated sigmas (the floor applies)"),
            ("igdm/same-sets-half-voxel", "an image with itself, half a voxel apart"),
        ],
        [
            ("reproduce: translation error (mm)", "ANTs (reproduce): error (mm)", "num"),
            ("fix: translation error (mm)", "corrected: error (mm)", "num"),
            ("reproduce: iterations", "ANTs: iterations", "count"),
            ("fix: iterations", "corrected: iterations", "count"),
        ],
    ),
    (
        "corrections-jhct",
        "JHCT (ITK-24, ITK-29): the same registration with the labelled phantoms",
        [
            ("jhct/isotropic-sigma-2", "isotropic, sigma 2: the total alone (ITK-24)"),
            ("jhct/isotropic-sigma-1", "isotropic, sigma 1 (ITK-24, ITK-23)"),
            ("jhct/anisotropic-sigma-1", "anisotropic, sigma 1 (ITK-29, ITK-24, ITK-26)"),
            ("jhct/anisotropic-sigma-2", "anisotropic, sigma 2, alpha 1.5"),
        ],
        [
            ("reproduce: translation error (mm)", "ANTs (reproduce): error (mm)", "num"),
            ("fix: translation error (mm)", "corrected: error (mm)", "num"),
            ("parameters changed by the corrections", "largest parameter change", "num"),
        ],
    ),
    (
        "corrections-mesh",
        "B-spline field meshes at the base level (ANTS-78; ITK-42): ANTs' stage is the corrected "
        "one with twice the mesh, byte for byte; how far the corrected field moves from ANTs' "
        "(mm, in the mask) and the fit each leaves",
        [
            ("mesh/bspline-field-3", "BSplineDisplacementField 3x3x3, two levels"),
            ("mesh/bspline-field-5-total-4", "5x5x5, total mesh 4x4x4"),
            ("mesh/bspline-field-knots", "a knot spacing of 12 mm"),
            ("mesh/bspline-exponential-4", "BSplineExponential 4x4x4"),
        ],
        [
            ("field moved by the correction: largest (mm)", "field moved: largest (mm)", "num"),
            ("field moved by the correction: mean (mm)", "mean (mm)", "num"),
            ("reproduce: mean squared difference in the mask", "ANTs' fit (MSE)", "num"),
            ("default: mean squared difference in the mask", "corrected fit (MSE)", "num"),
        ],
    ),
    (
        "corrections-chain",
        "Stages after a B-spline transform (ITK-35), which ANTs cannot run: the corrected stage "
        "against ANTs' closest runs, how far their results map the fixed points apart (largest "
        "and mean, mm, in the mask); with the field, the median of five ANTs runs, and how far "
        "ANTs' own runs are apart (ANTS-72)",
        [
            ("chain/bspline-then-affine", "BSpline, then Affine"),
            ("chain/bspline-then-rigid", "BSpline, then Rigid"),
            ("chain/initial-bspline-then-rigid", "-r a B-spline transform, then Rigid"),
            ("chain/initial-bspline-then-affine", "-r a B-spline transform, then Affine"),
            ("chain/bspline-then-bspline", "two BSpline stages"),
        ],
        [
            (
                "ANTs on the moving image resampled through the B-spline transform: largest distance (mm)",
                "vs ANTs on the resampled image: largest",
                "num",
            ),
            (
                "ANTs on the moving image resampled through the B-spline transform: mean distance (mm)",
                "mean",
                "num",
            ),
            (
                "ANTs with the B-spline transform as a displacement field (half its spacing): largest distance (mm)",
                "vs ANTs with the field (half spacing): largest",
                "num",
            ),
            (
                "ANTs with the B-spline transform as a displacement field (half its spacing): mean distance (mm)",
                "mean",
                "num",
            ),
            (
                "ANTs with the B-spline transform as a displacement field (half its spacing): ANTs' own runs",
                "ANTs' own runs apart: largest",
                "num",
            ),
        ],
    ),
]


def _corrections_tables(ctx: core.Context) -> tuple[list[dict], list[dict]]:
    """The corrections record's measurements as tables, and its source for the provenance
    (none when the record is missing)."""
    try:
        rec = records.load(ctx.repo, "ants-registration-corrections")
    except FileNotFoundError:
        return [], []
    by = {r["case"]: r for r in rec.results}
    tables = []
    for tid, title, rows, cols in CORRECTION_TABLES:
        out_rows = []
        for case, label in rows:
            r = by.get(case)
            if r is None:
                continue
            vals = {**r.get("measured", {}), **r.get("reported", {})}
            cells = []
            for name, _label, fmt in cols:
                v = vals.get(name)
                if v in (None, ""):
                    cells.append(common.cell("–", "text"))
                else:
                    cells.append(common.cell(float(v), "sig" if fmt == "num" else fmt))
            out_rows.append({"head": label, "cells": cells})
        tables.append(
            common.table(
                tid,
                title,
                ["Case", *({"label": c[1], "num": True} for c in cols)],
                out_rows,
                note="From the ants-registration-corrections record (larmorx in its default "
                "mode against `reproduce`, which the ants-registration record compares with "
                "ANTs bit for bit; for the B-spline chains, against ANTs 2.6.5 itself).",
            )
        )
    return tables, rec.source()


def export(ctx: core.Context) -> dict:
    reg = core.register(ctx.repo)
    rec = records.load(ctx.repo, TOOL)
    cases = _cases(ctx)
    from larmorx_validation.evidence import ants_panel

    prec = ants_panel.panel_record(ctx.repo, TOOL)
    if prec is not None:
        cases += ants_panel.panel_cases_tidy(_cases(ctx, prec))
    inventory = ants_panel.inventory_table(prec, "panel-nodes", "antsRegistration")
    examples = [_example(ctx, k, reg) for k in EXAMPLES] + ants_panel.examples(ctx, TOOL)
    corrections, corrections_source = _corrections_tables(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "antsRegistration",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source()
            + corrections_source
            + (prec.source() if prec is not None else []),
            "benchmarks": [],
            "cache": [fcache.provenance(entry(ctx, k)) for k in EXAMPLES]
            + [fcache.provenance(x) for x in ants_panel.entries(ctx, TOOL)],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cases, "port", "larmorx (port)", "port", f"docs/validation/{TOOL}.md", "validated"
            )
        ],
        "cases": cases,
        "cases_total": len(cases),
        "dots": {"band": _band(rec)},
        "tables": corrections + ([inventory] if inventory else []) + ants_panel.tables(ctx, TOOL),
        "examples": examples,
    }


if __name__ == "__main__":  # compute the cache entries only
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    for k in sys.argv[1:] or list(EXAMPLES):
        print("done", entry(c, k).dir)
