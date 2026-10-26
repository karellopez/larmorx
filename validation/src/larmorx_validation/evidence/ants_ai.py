# SPDX-License-Identifier: Apache-2.0
"""antsAI's and antsAffineInitializer's evidence: the initial transform sMRIPrep's brain
extraction found for a real T1w (fMRIPrep 25.2.6's ``init_aff``, antsAI), by fMRIPrep, by ANTs
2.6.5 (1 and 8 threads, correctly rounded maths) and by larmorx (both modes); niworkflows'
antsAffineInitializer call on the same images; each transform as its parameters, as the
moving image it places on the template (resampled by ANTs as built), and as the distance in
mm between where two transforms send the template's brain; antsAI's blob alignment on a T1w
and two copies with known transforms (``ants_ai_blobs``); the records' cases.
"""

from __future__ import annotations

import re
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import ants_ai_blobs, common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "ants-ai"
CASE = "fmriprep-ds000005-init-aff"
RUN = "ds000005-sub-01-noreconall"
NODE = "sub_01_wf/anat_fit_wf/brain_extraction_wf/init_aff"
SPEC = f"{RUN}/{NODE}"
BRAIN = "out/sub-01/anat/sub-01_desc-brain_mask.nii.gz"
OASIS_MASK = (
    "templateflow/tpl-OASIS30ANTs/tpl-OASIS30ANTs_res-01_desc-BrainCerebellumExtraction_mask.nii.gz"
)
#: niworkflows' antsAffineInitializer arguments (search factor, radian fraction, principal
#: axes, local search), the record's ``search/niworkflows``
NW_ARGS = ("15", "0.1", "0", "10")


def _compute(e: fcache.Entry) -> None:
    from larmorx_validation.parity.ants_ai import _fmriprep_node
    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    b, lib = ants_bin(), crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload")
    work, args, seed = _fmriprep_node(SPEC)
    i = args.index("-o")
    m = re.search(r"Mattes\[([^,]+),([^,]+),", " ".join(args))
    fixed, moving = Path(m.group(1)), Path(m.group(2))
    e.input("fixed (OASIS30ANTs template, resampled by fMRIPrep)", fixed)
    e.input("moving (the N4-corrected T1w, resampled by fMRIPrep)", moving)
    e.input("fMRIPrep's transform", work / "initialization.mat")
    base = {"ANTS_RANDOM_SEED": seed or ""}

    def ai_args(out: Path) -> list[str]:
        a = list(args)
        a[i + 1] = str(out)
        return a

    for label, threads, cr in (
        ("ai-ants-1", 1, False),
        ("ai-ants-8", 8, False),
        ("ai-ants-1-cr", 1, True),
    ):
        env = {**base, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
        if cr:
            env["LD_PRELOAD"] = str(lib)
        e.run(
            label,
            [str(b / "antsAI"), *ai_args(e.dir / f"{label}.mat")],
            env=env,
            unset=common.CLEAN_ENV,
        )
    for label, threads, mode in (
        ("ai-larmorx", 8, "reproduce"),
        ("ai-larmorx-1", 1, "reproduce"),
        ("ai-larmorx-fix", 8, "fix"),
    ):
        e.run(
            label,
            fcache.larmorx_cli("ants", "antsAI", *ai_args(e.dir / f"{label}.mat")),
            env={
                **base,
                "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                "LARMORX_UPSTREAM_DEFECTS": mode,
            },
            unset=common.CLEAN_ENV,
        )
    init = ["3", str(fixed), str(moving)]
    for label, threads, cr in (
        ("init-ants-1", 1, False),
        ("init-ants-8", 8, False),
        ("init-ants-1-cr", 1, True),
    ):
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
        if cr:
            env["LD_PRELOAD"] = str(lib)
        e.run(
            label,
            [str(b / "antsAffineInitializer"), *init, str(e.dir / f"{label}.mat"), *NW_ARGS],
            env=env,
            unset=common.CLEAN_ENV,
        )
    for label, mode in (("init-larmorx", "reproduce"), ("init-larmorx-fix", "fix")):
        e.run(
            label,
            fcache.larmorx_cli(
                "ants", "antsAffineInitializer", *init, str(e.dir / f"{label}.mat"), *NW_ARGS
            ),
            env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "8", "LARMORX_UPSTREAM_DEFECTS": mode},
            unset=common.CLEAN_ENV,
        )
    e.finish(
        versions={"ants": ants_version()},
        seed=f"ANTS_RANDOM_SEED={seed} (fMRIPrep's, from the node's report)",
        threads={"ants": [1, 8], "larmorx": [8, 1], "fmriprep_run": 8},
        command=" ".join((work / "command.txt").read_text(encoding="utf-8").split()),
        settings="fMRIPrep 25.2.6's init_aff (antsAI) of the brain extraction of "
        f"{RUN}: its arguments, inputs and seed; niworkflows' antsAffineInitializer arguments "
        "(15 0.1 0 10) on the same images; each transform applied to the moving image by "
        "antsApplyTransforms (ANTs as built, linear, one thread)",
    )


def _compute_placed(ctx: core.Context):
    def compute(e: fcache.Entry) -> None:
        """The moving image, times fMRIPrep's brain mask (brought onto its grid, nearest
        neighbour), placed on the fixed grid by each transform of the first entry (ANTs as
        built, linear): the views of the worked example, the face left out."""
        import nibabel as nib

        from larmorx_validation.parity.ants_ai import _fmriprep_node
        from larmorx_validation.parity.ants_programs import ants_bin, ants_version

        first = entry(ctx)
        b = ants_bin()
        work, args, _seed = _fmriprep_node(SPEC)
        m = re.search(r"Mattes\[([^,]+),([^,]+),", " ".join(args))
        fixed, moving = Path(m.group(1)), Path(m.group(2))
        mask = e.input("fMRIPrep's T1w brain mask", common.fmriprep_run(RUN) / BRAIN)
        aat = str(b / "antsApplyTransforms")
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}
        e.run(
            "mask-on-moving",
            [
                aat,
                "-d",
                "3",
                "-i",
                str(mask),
                "-r",
                str(moving),
                "-n",
                "NearestNeighbor",
                "-o",
                str(e.dir / "mask-on-moving.nii.gz"),
            ],
            env=env,
            unset=common.CLEAN_ENV,
        )
        img = nib.load(str(moving))
        mk = np.asanyarray(nib.load(str(e.dir / "mask-on-moving.nii.gz")).dataobj) > 0
        data = np.asanyarray(img.dataobj).astype(np.float32) * mk
        masked = e.dir / "moving-masked.nii.gz"
        nib.Nifti1Image(data, img.affine, img.header).to_filename(masked)
        for label, mat in [
            ("place-identity", None),
            ("place-fmriprep", work / "initialization.mat"),
            *[
                (f"place-{k}", first.dir / f"{k}.mat")
                for k in (
                    "ai-ants-1",
                    "ai-ants-8",
                    "ai-ants-1-cr",
                    "ai-larmorx",
                    "init-ants-1",
                    "init-larmorx",
                )
            ],
        ]:
            e.run(
                label,
                [
                    aat,
                    "-d",
                    "3",
                    "--float",
                    "1",
                    "-i",
                    str(masked),
                    "-r",
                    str(fixed),
                    "-n",
                    "Linear",
                    *(["-t", str(mat)] if mat else []),
                    "-o",
                    str(e.dir / f"{label}.nii.gz"),
                ],
                env=env,
                unset=common.CLEAN_ENV,
            )
        e.finish(
            versions={"ants": ants_version()},
            settings="the moving image of fMRIPrep's init_aff times fMRIPrep's T1w brain mask "
            "(antsApplyTransforms, nearest neighbour, onto the moving grid), placed on the fixed "
            f"grid by each transform of {TOOL}/{CASE} (antsApplyTransforms, linear, one thread)",
        )

    return compute


def placed_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE + "-placed",
        "the T1w × its brain mask placed on the template by each transform of the init_aff entry",
        _compute_placed(ctx),
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's init_aff (antsAI) of ds000005 sub-01's brain extraction and niworkflows' "
        "antsAffineInitializer on the same images: fMRIPrep, ANTs 2.6.5, larmorx",
        _compute,
    )


# ----------------------------------------------------------------------------------- export

PARAMS = [
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


def _affine(path: Path) -> tuple[np.ndarray, np.ndarray]:
    """The 4×4 matrix (ITK's physical space) and the raw parameters of a .mat transform."""
    p, fixed, _name = common.itk_linear(path)
    p = np.asarray(p, np.float64)
    c = np.asarray(fixed, np.float64)
    a = p[:9].reshape(3, 3)
    t = p[9:12]
    m = np.eye(4)
    m[:3, :3] = a
    m[:3, 3] = t + c - a @ c
    return m, p


def _mm(m1: np.ndarray, m2: np.ndarray, pts: np.ndarray) -> dict:
    d = np.linalg.norm((pts @ m1[:3, :3].T + m1[:3, 3]) - (pts @ m2[:3, :3].T + m2[:3, 3]), axis=1)
    return {"max": float(d.max()), "mean": float(d.mean())}


def _example(ctx: core.Context, reg: dict) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = entry(ctx)
    info = e.info()
    pe = placed_entry(ctx)
    from larmorx_validation.parity.ants_ai import _fmriprep_node

    work, args, _seed = _fmriprep_node(SPEC)
    m = re.search(r"Mattes\[([^,]+),([^,]+),", " ".join(args))
    fixed = Path(m.group(1))
    fimg = nib.load(str(fixed))
    import larmorx_testdata as td

    mask_path = Path(td.get(OASIS_MASK))
    # the template's brain mask on the fixed grid (nearest neighbour; both are OASIS grids)
    mimg = nib.load(str(mask_path))
    ijk = np.rint(np.linalg.inv(mimg.affine) @ fimg.affine).astype(int)
    idx = np.indices(fimg.shape[:3]).reshape(3, -1)
    src = ijk[:3, :3] @ idx + ijk[:3, 3:4]
    ok = np.all((src >= 0) & (src < np.array(mimg.shape[:3])[:, None]), axis=0)
    brain = np.zeros(idx.shape[1], bool)
    md = np.asanyarray(mimg.dataobj) > 0
    brain[ok] = md[src[0, ok], src[1, ok], src[2, ok]]
    brain = brain.reshape(fimg.shape[:3])
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(fimg, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(
        exm.Panel(
            "fixed",
            "input",
            "OASIS30ANTs template (fixed)",
            "as fMRIPrep resampled it for this call",
            {"all": common.load(str(fixed))},
        )
    )
    ex.add(
        exm.Panel(
            "moving",
            "input",
            "ds000005 sub-01 T1w (moving), unaligned",
            "N4-corrected, resampled by fMRIPrep; × its brain mask for display; no transform",
            {"all": common.load(str(pe.dir / "place-identity.nii.gz"))},
        )
    )
    runs = [
        (
            "larmorx",
            "place-ai-larmorx",
            "larmorx",
            "antsAI: larmorx (port)",
            "the T1w placed by its transform; reproduce mode, 8 threads",
        ),
        (
            "ants-1",
            "place-ai-ants-1",
            "original",
            "antsAI: ANTs 2.6.5, 1 thread",
            "the T1w placed by its transform",
        ),
        (
            "ants-8",
            "place-ai-ants-8",
            "original",
            "antsAI: ANTs 2.6.5, 8 threads",
            "the T1w placed by its transform",
        ),
        (
            "fmriprep",
            "place-fmriprep",
            "fmriprep",
            "antsAI: fMRIPrep 25.2.6, as it ran",
            "the T1w placed by fMRIPrep's transform (ANTs 2.6.2, 8 threads)",
        ),
        (
            "init-larmorx",
            "place-init-larmorx",
            "larmorx",
            "antsAffineInitializer: larmorx",
            "niworkflows' arguments; the T1w placed by its transform",
        ),
        (
            "init-ants-1",
            "place-init-ants-1",
            "original",
            "antsAffineInitializer: ANTs 2.6.5",
            "niworkflows' arguments, 1 thread; the T1w placed",
        ),
    ]
    for pid, label, role, text, sub in runs:
        ex.add(
            exm.Panel(
                pid, role, text, sub, {"placed": common.load(str(pe.dir / f"{label}.nii.gz"))}
            )
        )
    dd = common.d
    diffs = [
        dd(
            "lx-ants1",
            "larmorx",
            "ants-1",
            "main",
            "antsAI: larmorx − ANTs, 1 thread (placed T1w)",
            common.ROLE_LX,
        ),
        dd(
            "lx-fmriprep",
            "larmorx",
            "fmriprep",
            "fmriprep",
            "antsAI: larmorx − fMRIPrep (placed T1w)",
            common.ROLE_FMRIPREP,
        ),
        dd(
            "init-lx-ants",
            "init-larmorx",
            "init-ants-1",
            "main",
            "antsAffineInitializer: larmorx − ANTs (placed T1w)",
            common.ROLE_LX,
        ),
        dd(
            "ants8-ants1",
            "ants-8",
            "ants-1",
            "variability",
            "antsAI: ANTs 8 threads − 1 thread (placed T1w)",
            common.ROLE_ORIG,
        ),
        dd(
            "fmriprep-ants8",
            "fmriprep",
            "ants-8",
            "variability",
            "antsAI: fMRIPrep − ANTs 2.6.5, 8 threads (placed T1w)",
            common.ROLE_ORIG,
        ),
    ]
    outputs = [{"id": "placed", "label": "T1w placed on the template", "unit": ""}]
    common.compare_all(ex, diffs, outputs)
    same_img = common.identical(diffs)
    # the transforms themselves
    mats = {
        k: _affine(e.dir / f"{k}.mat")
        for k in (
            "ai-ants-1",
            "ai-ants-8",
            "ai-ants-1-cr",
            "ai-larmorx",
            "ai-larmorx-1",
            "ai-larmorx-fix",
            "init-ants-1",
            "init-ants-8",
            "init-ants-1-cr",
            "init-larmorx",
            "init-larmorx-fix",
        )
    }
    mats["fmriprep"] = _affine(work / "initialization.mat")
    pts_vox = np.argwhere(brain)
    pts = (fimg.affine @ np.c_[pts_vox, np.ones(len(pts_vox))].T).T[:, :3]
    pts = pts * np.array([-1.0, -1.0, 1.0])  # RAS mm -> ITK's LPS
    same_file = {k: (e.dir / f"{k}.mat").read_bytes() for k in mats if k != "fmriprep"}
    same_file["fmriprep"] = (work / "initialization.mat").read_bytes()

    import re as _re

    named = {
        k: sorted(
            set(
                _re.findall(
                    r"upstream defect ([A-Z]+-\d+)",
                    (e.dir / f"{k}.log").read_text(encoding="utf-8"),
                )
            )
        )
        for k in ("ai-larmorx-fix", "init-larmorx-fix")
    }

    def mmrow(label, a, b, cls="", sub=""):
        d = _mm(mats[a][0], mats[b][0], pts)
        ident = same_file[a] == same_file[b]
        return {
            "head": label,
            **({"class": cls} if cls else {}),
            **({"sub": sub} if sub else {}),
            "cells": [
                {"text": "identical file" if ident else "differ"},
                common.cell(d["max"]),
                common.cell(d["mean"]),
                common.cell(float(np.abs(mats[a][1] - mats[b][1]).max())),
            ],
        }

    dist = common.table(
        "ai-distance",
        "Where two transforms send the template's brain apart (mm), and their largest parameter "
        "difference",
        [
            "Comparison",
            "the .mat files",
            {"label": "largest distance (mm)", "num": True},
            {"label": "mean distance (mm)", "num": True},
            {"label": "largest |Δ parameter|", "num": True},
        ],
        [
            mmrow("antsAI: larmorx − ANTs, 1 thread", "ai-larmorx", "ai-ants-1"),
            mmrow("antsAI: larmorx − ANTs with CORE-MATH", "ai-larmorx", "ai-ants-1-cr"),
            mmrow("antsAI: larmorx − fMRIPrep's own", "ai-larmorx", "fmriprep"),
            mmrow("antsAI: larmorx 1 thread − 8 threads", "ai-larmorx-1", "ai-larmorx"),
            mmrow(
                "antsAI: larmorx default − reproduce",
                "ai-larmorx-fix",
                "ai-larmorx",
                sub=", ".join(named["ai-larmorx-fix"]) + " corrected"
                if named["ai-larmorx-fix"]
                else "no correction applies",
            ),
            mmrow(
                "antsAI: ANTs 8 threads − 1 thread", "ai-ants-8", "ai-ants-1", "is-sep is-yardstick"
            ),
            mmrow(
                "antsAI: fMRIPrep − ANTs 2.6.5, 8 threads", "fmriprep", "ai-ants-8", "is-yardstick"
            ),
            mmrow("antsAffineInitializer: larmorx − ANTs", "init-larmorx", "init-ants-1", "is-sep"),
            mmrow(
                "antsAffineInitializer: larmorx − ANTs with CORE-MATH",
                "init-larmorx",
                "init-ants-1-cr",
            ),
            mmrow(
                "antsAffineInitializer: larmorx default − reproduce",
                "init-larmorx-fix",
                "init-larmorx",
                sub=", ".join(named["init-larmorx-fix"]) + " corrected (D13)"
                if named["init-larmorx-fix"]
                else "no correction applies",
            ),
            mmrow(
                "antsAffineInitializer: ANTs 8 threads − 1 thread",
                "init-ants-8",
                "init-ants-1",
                "is-yardstick",
            ),
        ],
        note=f"Over the {len(pts):,} voxels of the template's brain (OASIS30ANTs' "
        "BrainCerebellumExtraction mask, the call's -x) on this grid: each transform maps "
        "them into the T1w's space; the distance is between the two images of each point. "
        "The shaded rows are ANTs against itself.",
    )
    params = common.table(
        "ai-params",
        "The transforms' parameters (antsAI's Affine, as written: ITK's AffineTransform, LPS)",
        [
            "Parameter",
            {"label": "fMRIPrep (ANTs 2.6.2, 8 threads)", "num": True},
            {"label": "ANTs 2.6.5, 1 thread", "num": True},
            {"label": "larmorx", "num": True},
            {"label": "larmorx − fMRIPrep", "num": True},
            {"label": "larmorx − ANTs, 1 thread", "num": True},
        ],
        [
            {
                "head": n,
                "cells": [
                    common.cell(float(mats["fmriprep"][1][j]), "f64"),
                    common.cell(float(mats["ai-ants-1"][1][j]), "f64"),
                    common.cell(float(mats["ai-larmorx"][1][j]), "f64"),
                    common.cell(float(mats["ai-larmorx"][1][j] - mats["fmriprep"][1][j])),
                    common.cell(float(mats["ai-larmorx"][1][j] - mats["ai-ants-1"][1][j])),
                ],
            }
            for j, n in enumerate(PARAMS)
        ],
        note="As the .mat files store them (double precision), printed with the digits that "
        "identify each value.",
    )
    tdiff = {
        "lx-ants1": same_file["ai-larmorx"] == same_file["ai-ants-1"],
        "init-lx-ants": same_file["init-larmorx"] == same_file["init-ants-1"],
    }
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if x["kind"] == "variability":
            x["verdict"] = None
            x["reason"] = (
                "The same placed image."
                if same_img[i]
                else "ANTs' search adds per-thread metric values in an order that depends on "
                "its threads: its transform moves in the last digits (the table below)."
            )
        elif tdiff.get(i):
            x["verdict"] = "bit-identical"
            x["reason"] = (
                "The same transform, byte for byte (the .mat file), so the same placed image."
            )
        elif same_img[i] and i == "lx-fmriprep":
            d = _mm(mats["ai-larmorx"][0], mats["fmriprep"][0], pts)
            d8 = _mm(mats["ai-ants-8"][0], mats["ai-ants-1"][0], pts)
            x["verdict"] = "bit-identical"
            x["reason"] = (
                "The placed images are the same bytes. The transforms themselves differ: "
                f"fMRIPrep's is ANTs at 8 threads, larmorx's ANTs' one-thread result; they send "
                f"the brain at most {d['max']:.2g} mm apart, ANTs' own 8 and 1 threads "
                f"{d8['max']:.2g} mm (the table below), far below what the interpolation sees."
            )
        elif i == "lx-fmriprep":
            d = _mm(mats["ai-larmorx"][0], mats["fmriprep"][0], pts)
            d8 = _mm(mats["ai-ants-8"][0], mats["ai-ants-1"][0], pts)
            x["verdict"] = (
                "within-variability" if d["max"] <= max(d8["max"], 1e-12) * 10 else "open"
            )
            x["reason"] = (
                f"fMRIPrep's transform is ANTs at 8 threads; larmorx's is ANTs' one-thread "
                f"result: they send the brain {d['max']:.2g} mm apart at most, ANTs' own 8 and "
                f"1 threads {d8['max']:.2g} mm (the table below)."
            )
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example."
    # the default view: the axial slice where most of the unaligned T1w lies inside the shown
    # template brain (before its transform the T1w sits far from the template), the crosshair
    # at that part's centre
    mv = (ex.values("moving:all") > 0) & ex.m
    k = int(np.argmax(mv.sum(axis=(0, 1))))
    ci, cj = (int(v) for v in np.round(np.argwhere(mv[:, :, k]).mean(0)))
    at = [ci, cj, k]
    default = {
        "output": "placed",
        "plane": "axial",
        "voxel": at,
        "panels": ["fixed", "moving", "fmriprep", "larmorx"],
        "diffs": ["lx-fmriprep", "lx-ants1", "ants8-ants1"],
        "compare": {"a": "fixed", "b": "larmorx"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        outputs,
        default=default,
        window_panel="larmorx",
        input_windows={"moving": "brain"},
        scale="log",
        caption="the template and the T1w before and after each transform; "
        "Checkerboard (under Compare) overlays the template and a placed T1w.",
    )
    vers = info.get("versions", {})
    command = info["command"].replace(
        "/work/fmriprep_25_2_wf/sub_01_wf/anat_fit_wf/brain_extraction_wf/", "…/"
    )
    return {
        "id": "init-aff",
        "title": "The initial transform of sMRIPrep's brain extraction, by fMRIPrep, ANTs and larmorx",
        "summary": (
            "fMRIPrep 25.2.6's `init_aff` node (antsAI's multi-start search: rotations around "
            "the principal axes, an Affine local search with Mattes MI) on OpenNeuro ds000005 "
            "sub-01, with its arguments, inputs and seed; and niworkflows' antsAffineInitializer "
            "call on the same images. Each transform is shown as the T1w it places on the "
            "template (resampled by ANTs as built, linear), as its parameters and as distances "
            "in mm."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01's T1w "
                f"({common.DS000005['licence']}); the OASIS30ANTs template (Avants et al., CC BY 4.0, "
                "TemplateFlow); shown inside the template's brain mask, dilated by 2 voxels",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `anat_fit_wf/brain_extraction_wf/init_aff`, `NSLOTS=8`",
            },
            {"label": "Command", "text": f"`{command}`"},
            {
                "label": "Also",
                "text": "`antsAffineInitializer 3 <fixed> <moving> out.mat 15 0.1 0 "
                "10` (niworkflows' arguments; fMRIPrep 25.2.6 does not call it here)",
            },
            {
                "label": "Programs",
                "text": f"{vers.get('ants', '?')}, {info.get('seed', '')}; "
                f"larmorx at commit `{info['larmorx_commit'][:12]}`",
            },
            {"label": "Grid", "text": common.describe_grid(fimg, grid.shape)},
        ],
        **out,
        "chain": None,
        "tables": [dist, params],
        "metrics_note": "Δ is A − B between the placed images, over every voxel of the "
        f"{' × '.join(map(str, fimg.shape))} grid.",
        "provenance": common.provenance(
            info,
            f"{TOOL}/{CASE} and {TOOL}/{CASE}-placed",
            [
                common.DS000005,
                {
                    "dataset": "OASIS30ANTs (Avants et al.), from TemplateFlow",
                    "licence": "CC BY 4.0",
                },
            ],
        ),
    }


# ------------------------------------------------------------------------------------ cases

#: The record's cases that a blob example shows.
BLOB_EXAMPLES = {
    "real/ds000005-sub-01/blobs-t1w-shift": "blobs-shift",
    "real/ds000005-sub-01/blobs-t1w-rotated": "blobs-rotated",
}

IMPLS = {
    "ai": ("antsAI", "ants-ai", "ants_ai"),
    "init": ("antsAffineInitializer", "ants-affine-initializer", "ants_affine_initializer"),
}


def _cases(ctx: core.Context) -> list[dict]:

    from larmorx_validation.evidence import ants_panel

    out = []
    for impl, (program, name, module) in IMPLS.items():
        for rec in (records.load(ctx.repo, name), ants_panel.panel_record(ctx.repo, name)):
            if rec is not None:
                out += _record_cases(rec, impl, program, module)
    return out


def _record_cases(rec, impl: str, program: str, module: str) -> list[dict]:
    """The cases of one record of a program, with their verdicts."""
    import importlib

    out = []
    mod = importlib.import_module(f"larmorx_validation.parity.{module}")
    defs = {
        c.id: c
        for c in (
            mod.all_cases()
            if hasattr(mod, "all_cases")
            else [x.payload for x in mod.cases_for("full")]
        )
    }
    for r in rec.results:
        c = defs.get(r["case"])
        v = common.outcome_verdict(r)
        rep = r.get("reported", {})
        if v is None:
            glibc = sorted(k for k in rep if k.startswith("unmodified ANTs"))
            band = rep.get("thread band")
            if glibc:
                v = (
                    "identical-cr",
                    "differs from ANTs as built in "
                    + ", ".join(g.removeprefix("unmodified ANTs: ") for g in glibc[:3])
                    + "; identical to ANTs with CORE-MATH preloaded",
                )
            else:
                v = (
                    "bit-identical",
                    "exit status, standard output, standard error, every file",
                )
            if band is not None:
                v = (v[0], v[1] + f"; larmorx − ANTs at 1 thread: {band} in a parameter")
            if "alignment error" in rep:
                v = (
                    v[0],
                    v[1] + f"; from the known transform: ANTs {rep.get('alignment error: ANTs')}, "
                    f"larmorx's default {rep['alignment error']}",
                )
        out.append(
            {
                "id": f"{program}/{r['case']}",
                "category": r["category"],
                "description": c.description if c else "",
                "command": "",
                "results": {
                    impl: {
                        "outcome": r["status"],
                        "verdict": v[0],
                        "checks": f"{r['checks_passed']}/{r['checks_total']}",
                        "reason": common.clip(v[1], 400),
                    }
                },
                **(
                    {"example": BLOB_EXAMPLES.get(r["case"], "init-aff")}
                    if r["case"] == "real/ds000005-sub-01/brain-extraction"
                    or r["case"] in BLOB_EXAMPLES
                    else {}
                ),
            }
        )
    return out


def export(ctx: core.Context) -> dict:
    reg = core.register(ctx.repo)
    cases = _cases(ctx)
    a116, a116_sources = ants_ai_blobs.a116_table(ctx)
    from larmorx_validation.evidence import ants_panel

    impls, sources = [], []
    prec = ants_panel.panel_record(ctx.repo, "ants-ai")
    inventory = ants_panel.inventory_table(prec, "panel-nodes", "antsAI")
    for impl, (program, name, _m) in IMPLS.items():
        rec = records.load(ctx.repo, name)
        sources += rec.source()
        if (p := ants_panel.panel_record(ctx.repo, name)) is not None:
            sources += p.source()
        impls.append(
            common.implementation(
                cases,
                impl,
                f"{program} (port)",
                program,
                f"docs/validation/{name}.md",
                "validated",
                name=name,
            )
        )
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "antsAI, antsAffineInitializer",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": sources,
            "benchmarks": [],
            "cache": [
                fcache.provenance(entry(ctx)),
                fcache.provenance(placed_entry(ctx)),
                ants_ai_blobs.provenance(ctx),
                *(fcache.provenance(x) for x in ants_panel.entries(ctx, TOOL)),
            ],
            "measurements": a116_sources,
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": impls,
        "cases_per_impl": True,
        "cases": cases,
        "cases_total": len(cases),
        "tables": [a116, *([inventory] if inventory else []), *ants_panel.tables(ctx, TOOL)],
        "examples": [
            _example(ctx, reg),
            *ants_ai_blobs.examples(ctx),
            *ants_panel.examples(ctx, TOOL),
        ],
    }


if __name__ == "__main__":
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    print("done", entry(c).dir, placed_entry(c).dir)
