# SPDX-License-Identifier: Apache-2.0
"""antsAI's blob alignment (``-b``) on real data, for the antsAI page: fMRIPrep's T1w of
ds000005 sub-01 against two copies whose transform is known by construction (its origin moved
by (6, −4, 3) mm; rotated by 8° and shifted), aligned by ANTs 2.6.5 (one thread as built and
with correctly rounded maths, twice; at 2, 4 and 6 threads it crashes, ANTS-60) and by larmorx
(``reproduce`` at 1 and 6 threads, the default at 6). What each program computed at each step
comes from ANTs' own code (``validation/probes/ants-ai-blobs``, an instrumented copy of
antsAI's blob code built against the oracle's ITK) and from ``lx.ants.blob_alignment``: the
blobs, the pairs, the landmarks and the transforms; each transform also places the moving
image on the fixed grid (antsApplyTransforms as built, linear).

Run ``python -m larmorx_validation.evidence.ants_ai_blobs dump <fixed> <moving> <out.json>
<mode> <threads>`` to write larmorx's steps as JSON (the cache entry does).
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache

TOOL = "ants-ai"
CASE = "blobs-t1w"
RUN = "ds000005-sub-01-noreconall"
BRAIN = "out/sub-01/anat/sub-01_desc-brain_mask.nii.gz"
N_BLOBS = 30
LX_THREADS = 6
BAND = (2, 4, 6)
PROBE_SRC = Path(__file__).resolve().parents[3] / "probes" / "ants-ai-blobs"
#: move -> (input builder, truth key, description)
MOVES = {
    "shift": (
        "ai-blobs-t1w-shift",
        "t1w:shift",
        "the same voxels with the origin moved by (6, −4, 3) mm",
    ),
    "rotated": (
        "ai-blobs-t1w-rotated",
        "t1w:rotated",
        "rotated by 8° (in millimetres) about the z axis through the centre and shifted by "
        "(5, −3, 2) voxels (linear interpolation)",
    ),
}
LPS = np.diag([-1.0, -1.0, 1.0])


def _ai_args(fixed: Path, moving: Path, out: Path) -> list[str]:
    """The record's ``real/ds000005-sub-01/blobs-t1w-*`` arguments."""
    return [
        "-d",
        "3",
        "-m",
        f"Mattes[{fixed},{moving},32,Regular,0.25]",
        "-t",
        "Rigid[0.1]",
        "-b",
        str(N_BLOBS),
        "-o",
        str(out),
        "-v",
        "1",
    ]


# ------------------------------------------------------------------------- larmorx's steps


def dump(fixed: str, moving: str, out: str, mode: str, threads: int) -> None:
    """``lx.ants.blob_alignment``'s steps as JSON (floats exact: Python's ``repr``)."""
    import warnings

    import larmorx as lx

    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            r = lx.ants.blob_alignment(
                fixed, moving, blobs=N_BLOBS, n_threads=threads, upstream_defects=mode
            )
            error = None
        except lx.ants.AIError as e:
            r, error = None, str(e)
    doc: dict = {
        "mode": mode,
        "threads": threads,
        "warnings": [str(w.message) for w in caught],
        "error": error,
    }
    if r is not None:
        doc.update(
            fixed_blobs=[[*b.index, b.sigma, b.radius, b.value] for b in r.fixed_blobs],
            moving_blobs=[[*b.index, b.sigma, b.radius, b.value] for b in r.moving_blobs],
            correspondence=np.asarray(r.correspondence, np.float64).tolist(),
            pairs=[list(p) for p in r.pairs],
            landmarks=list(r.landmarks),
            parameters=[float(v) for v in r.transform.parameters],
        )
    Path(out).write_text(json.dumps(doc) + "\n", encoding="utf-8")


def _probe_parse(log: Path) -> dict:
    """ANTs' steps from the probe's log: blobs (index, radius, value), correspondence entries,
    pairs, landmarks and the transform (matrix, offset)."""
    text = log.read_text(encoding="utf-8")
    blobs: dict[str, list] = {"fixed": [], "moving": []}
    which = None
    corr, pairs, landmarks, transform, exc = {}, [], [], None, None
    for line in text.splitlines():
        w = line.split()
        if not w:
            continue
        if w[0] == "BLOBS":
            which = w[1]
        elif w[0] == "B" and which:
            blobs[which].append([int(v) for v in w[2:5]] + [float.fromhex(v) for v in w[5:7]])
        elif w[0] == "C":
            corr[(int(w[1]), int(w[2]))] = float.fromhex(w[3])
        elif w[0] == "P":
            pairs.append([int(w[1]), int(w[2])])
        elif w[0] == "L":
            landmarks.append(int(w[1]))
        elif w[0] == "T":
            transform = [float.fromhex(v) for v in w[1:]]
        elif w[0] == "EXC":
            exc = line[4:]
    return {
        "fixed_blobs": blobs["fixed"],
        "moving_blobs": blobs["moving"],
        "corr": corr,
        "pairs": pairs,
        "landmarks": landmarks,
        "transform": transform,
        "exception": exc,
    }


# ------------------------------------------------------------------------------ the entry


def _truth(key: str, fixed: Path) -> tuple[np.ndarray, np.ndarray]:
    """The true transform, fixed to moving, in ITK's physical space (LPS mm): matrix, offset."""
    from larmorx_validation.parity import ants_ai_blobs as pb

    f, m = pb.truth_points(key, fixed)
    x = np.c_[f, np.ones(len(f))]
    sol, *_ = np.linalg.lstsq(x, m, rcond=None)
    a_ras, t_ras = sol[:3].T, sol[3]
    return LPS @ a_ras @ LPS, LPS @ t_ras


def _write_mat(path: Path, a: np.ndarray, t: np.ndarray) -> None:
    import larmorx as lx
    from larmorx.transforms import ItkTransform

    params = np.r_[np.asarray(a, np.float64).ravel(), np.asarray(t, np.float64)]
    lx.transforms.write(
        str(path), [ItkTransform("AffineTransform_double_3_3", params, np.zeros(3))]
    )


def _probe_exe(e: fcache.Entry) -> Path:
    from larmorx_validation.figures.core import workspace

    ws = workspace()
    build = e.ctx.cache / TOOL / "blob-probe-build"
    exe = build / "blob_probe"
    if not exe.is_file():
        gxx = ws / "oracles/toolchains/gcc-14.3/bin/x86_64-conda-linux-gnu-g++"
        e.run(
            "probe-configure",
            [
                "cmake",
                "-S",
                str(PROBE_SRC),
                "-B",
                str(build),
                "-G",
                "Ninja",
                f"-DCMAKE_CXX_COMPILER={gxx}",
                f"-DITK_DIR={ws / 'oracles/ants-2.6.5-src/itk-build'}",
                f"-DANTS_UTILITIES={ws / 'tags/ANTs-v2.6.5/Utilities'}",
            ],
            unset=common.CLEAN_ENV,
        )
        e.run("probe-build", ["cmake", "--build", str(build), "-j", "6"], unset=common.CLEAN_ENV)
    return exe


def _compute(e: fcache.Entry) -> None:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.parity import ants_ai_blobs as pb
    from larmorx_validation.parity import ants_inputs
    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    b, lib = ants_bin(), crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload")
    run_dir = common.fmriprep_run(RUN)
    fixed = e.input(
        "fixed: fMRIPrep's preprocessed T1w", run_dir / "out" / pb.DS000005_T1W.split("/out/")[1]
    )
    mask = e.input("fMRIPrep's T1w brain mask", run_dir / BRAIN)
    probe = _probe_exe(e)
    aat = str(b / "antsApplyTransforms")
    one = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}
    fimg, mimg = nib.load(str(fixed)), nib.load(str(mask))
    for move, (builder, key, _what) in MOVES.items():
        moving = e.dir / f"{move}-moving.nii.gz"
        ants_inputs.BUILDERS[builder][1](moving)
        e.input(f"moving ({move}), built by the parity suite's `{builder}`", moving)
        # the moving image's brain mask (the fixed one carried by the same construction) and
        # the moving image times it: the placed views leave the face out
        md = np.asanyarray(mimg.dataobj) > 0
        if move == "rotated":
            m, off = pb._t1w_rotation(md.shape, fimg.header.get_zooms())
            md = ndimage.affine_transform(md.astype(np.float32), m, offset=off, order=0) > 0.5
        mv = nib.load(str(moving))
        masked = e.dir / f"{move}-moving-masked.nii.gz"
        nib.Nifti1Image(
            (np.asanyarray(mv.dataobj).astype(np.float32) * md).astype(np.float32), mv.affine
        ).to_filename(masked)
        a, t = _truth(key, fixed)
        _write_mat(e.dir / f"{move}-truth.mat", a, t)
        runs = [(f"{move}-ants-1", 1, False), (f"{move}-ants-1b", 1, False)]
        runs += [(f"{move}-ants-1-cr", 1, True)]
        runs += [(f"{move}-ants-{n}", n, False) for n in BAND]
        for label, threads, cr in runs:
            env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
            if cr:
                env["LD_PRELOAD"] = str(lib)
            e.run(
                label,
                [str(b / "antsAI"), *_ai_args(fixed, moving, e.dir / f"{label}.mat")],
                env=env,
                unset=common.CLEAN_ENV,
                check=threads == 1,
            )
        e.run(
            f"{move}-probe",
            [str(probe), "3", str(fixed), str(moving), str(N_BLOBS), str(N_BLOBS), "1"],
            env=one,
            unset=common.CLEAN_ENV,
        )
        for label, threads, mode in (
            (f"{move}-larmorx", LX_THREADS, "reproduce"),
            (f"{move}-larmorx-1", 1, "reproduce"),
            (f"{move}-larmorx-fix", LX_THREADS, "fix"),
        ):
            e.run(
                label,
                fcache.larmorx_cli(
                    "ants", "antsAI", *_ai_args(fixed, moving, e.dir / f"{label}.mat")
                ),
                env={
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                    "LARMORX_UPSTREAM_DEFECTS": mode,
                },
                unset=common.CLEAN_ENV,
            )
        for mode in ("reproduce", "fix"):
            e.run(
                f"{move}-steps-{mode}",
                [
                    sys.executable,
                    "-m",
                    "larmorx_validation.evidence.ants_ai_blobs",
                    "dump",
                    str(fixed),
                    str(moving),
                    str(e.dir / f"{move}-steps-{mode}.json"),
                    mode,
                    str(LX_THREADS),
                ],
                unset=common.CLEAN_ENV,
            )
        for label in ("identity", "truth", "ants-1", "larmorx", "larmorx-fix"):
            mat = None if label == "identity" else e.dir / f"{move}-{label}.mat"
            if mat is not None and not np.all(np.isfinite(common.itk_linear(mat)[0])):
                continue  # ANTs' transform of NaNs places nothing
            e.run(
                f"{move}-place-{label}",
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
                    str(e.dir / f"{move}-place-{label}.nii.gz"),
                ],
                env=one,
                unset=common.CLEAN_ENV,
            )
    del fimg
    e.finish(
        versions={"ants": ants_version()},
        threads={"ants": [1, *BAND], "larmorx": [LX_THREADS, 1]},
        settings="fMRIPrep's preprocessed T1w of ds000005 sub-01 against two copies with a "
        "transform known by construction (the parity suite's ai-blobs-t1w-shift and "
        "ai-blobs-t1w-rotated); antsAI -b 30 with the record's arguments (Rigid[0.1], Mattes "
        "32 bins, Regular 0.25); ANTs' steps from the blob probe (validation/probes/"
        "ants-ai-blobs, built against the oracle's ITK with GCC 14.3); larmorx's from "
        "lx.ants.blob_alignment; each transform applied to the moving image times its brain "
        "mask by antsApplyTransforms (ANTs as built, linear, one thread); the true transform "
        "fitted exactly to the construction's points",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "antsAI -b on fMRIPrep's T1w of ds000005 sub-01 against a shifted and a rotated copy: "
        "ANTs 2.6.5 (1 thread, CORE-MATH, 2-6 threads), its steps from the blob probe, larmorx "
        "in both modes",
        _compute,
    )


# ------------------------------------------------------------------------------ the display


def _affine(path: Path) -> tuple[np.ndarray, np.ndarray]:
    """A linear transform file (antsAI writes ``Euler3DTransform`` for ``Rigid``) as a 4 × 4
    matrix in ITK's physical space (LPS mm), from four points mapped as
    antsApplyTransformsToPoints maps them; and its parameters as stored."""
    import larmorx as lx

    p = np.array([[0, 0, 0], [100, 0, 0], [0, 100, 0], [0, 0, 100]], np.float64)  # RAS
    q = lx.ants.apply_transforms_to_points(p, [str(path)], coordinates="ras")[:, :3]
    sol = np.linalg.solve(np.c_[p, np.ones(4)], q)
    m = np.eye(4)
    m[:3, :3] = LPS @ sol[:3].T @ LPS
    m[:3, 3] = LPS @ sol[3]
    return m, np.asarray(common.itk_linear(path)[0], np.float64)


def _truth_exact(move: str, fimg) -> np.ndarray:
    """The true transform (fixed to moving points, LPS mm) as a 4 × 4 matrix, from the
    construction: the shift's translation, or the rotation ``R`` in millimetres (the moving
    voxel ``o`` shows the fixed voxel ``m o + off`` with ``m = Z⁻¹ R Z``, so a fixed point ``p``
    is at ``Rᵀ (p − b) − Rᵀ Z off + b`` for the T1w's diagonal affine ``[Z | b]``)."""
    from larmorx_validation.parity import ants_ai_blobs as pb

    out = np.eye(4)
    if move == "shift":
        out[:3, 3] = LPS @ np.asarray(pb.T1W_ORIGIN_SHIFT, np.float64)
        return out
    aff = np.asarray(fimg.affine, np.float64)
    assert np.count_nonzero(aff[:3, :3] - np.diag(np.diag(aff[:3, :3]))) == 0
    z, b = aff[:3, :3], aff[:3, 3]
    _m, off = pb._t1w_rotation(fimg.shape[:3], fimg.header.get_zooms())
    a = np.deg2rad(pb.T1W_ROTATION)
    rt = np.array([[np.cos(a), np.sin(a), 0.0], [-np.sin(a), np.cos(a), 0.0], [0.0, 0.0, 1.0]])
    t = b - rt @ b - rt @ (z @ off)
    out[:3, :3] = LPS @ rt @ LPS
    out[:3, 3] = LPS @ t
    return out


def _angle(r: np.ndarray) -> float:
    """The rotation angle (degrees) of a rotation matrix."""
    return float(np.degrees(np.arccos(np.clip((np.trace(r) - 1.0) / 2.0, -1.0, 1.0))))


def _distance(m1: np.ndarray, m2: np.ndarray, pts: np.ndarray) -> dict:
    d = np.linalg.norm((pts @ m1[:3, :3].T + m1[:3, 3]) - (pts @ m2[:3, :3].T + m2[:3, 3]), axis=1)
    return {"max": float(d.max()), "mean": float(d.mean())}


def _spheres(shape, blobs: list, zooms) -> np.ndarray:
    """A mask of the voxels within each blob's radius (mm) of its centre voxel."""
    out = np.zeros(shape, dtype=bool)
    z = np.asarray(zooms[:3], np.float64)
    for bl in blobs:
        c = np.asarray(bl[:3], np.int64)
        r = float(bl[3])
        half = np.ceil(r / z).astype(int)
        lo = np.maximum(c - half, 0)
        hi = np.minimum(c + half + 1, np.array(shape))
        idx = np.indices(tuple(hi - lo)).reshape(3, -1).T + lo
        sel = idx[np.linalg.norm((idx - c) * z, axis=1) <= r]
        out[tuple(sel.T)] = True
    return out


def _drawn(data: np.ndarray, blobs: list, zooms, value: float) -> np.ndarray:
    """``data`` at half its intensity with each blob a filled sphere of its radius at
    ``value`` (above the T1w's range, so the blobs stand out in any window)."""
    out = (np.asarray(data, dtype=np.float32) * np.float32(0.5)).astype(np.float32)
    out[_spheres(out.shape, blobs, zooms)] = np.float32(value)
    return out


def _brain(fixed: Path, mask: Path):
    import nibabel as nib

    fimg = nib.load(str(fixed))
    brain = np.asanyarray(nib.load(str(mask)).dataobj) > 0
    return fimg, brain


def _points(fimg, brain) -> np.ndarray:
    v = np.argwhere(brain)
    pts = (fimg.affine @ np.c_[v, np.ones(len(v))].T).T[:, :3]
    return pts * np.array([-1.0, -1.0, 1.0])  # RAS mm -> ITK's LPS


def _exited(e: fcache.Entry, label: str) -> int | None:
    for r in e.info().get("runs", []):
        if r["label"] == label:
            return r["exit"]
    return None


def _pairs_table(
    tid: str, title: str, ants: dict, lx_fix: dict, fixed_affine, moving_affine, truth, note: str
) -> dict:
    """Every pair ANTs and larmorx's default matched: the blobs, their correlation, how far the
    moving blob is from where the true transform sends the fixed one, and whether each program
    kept the pair as a landmark."""

    def pt(idx, affine) -> np.ndarray:
        p = (affine @ np.r_[np.asarray(idx[:3], np.float64), 1.0])[:3]
        return p * np.array([-1.0, -1.0, 1.0])

    def key(pair, side) -> tuple:
        fb = side["fixed_blobs"][pair[0]]
        mb = side["moving_blobs"][pair[1]]
        return tuple(fb[:3]), tuple(mb[:3])

    rows, seen = [], {}
    sides = (("ANTs", ants), ("larmorx's default", lx_fix))
    for _name, side in sides:
        for k, pair in enumerate(side["pairs"]):
            kk = key(pair, side)
            if kk in seen:
                continue
            seen[kk] = (side, pair, k)
    a_keys = [key(p, ants) for p in ants["pairs"]]
    f_keys = [key(p, lx_fix) for p in lx_fix["pairs"]]
    a_land = {key(ants["pairs"][i], ants) for i in ants["landmarks"]}
    f_land = {key(lx_fix["pairs"][i], lx_fix) for i in lx_fix["landmarks"]}
    tm, tt = truth
    for n, (kk, (side, pair, _k)) in enumerate(seen.items(), 1):
        fb = side["fixed_blobs"][pair[0]]
        mb = side["moving_blobs"][pair[1]]
        corr = side["corr"].get((pair[0], pair[1]), 0.0)
        fp, mp = pt(fb, fixed_affine), pt(mb, moving_affine)
        err = float(np.linalg.norm(tm @ fp + tt - mp))
        radius = float(fb[3])
        rows.append(
            {
                "head": f"{n}",
                "cells": [
                    {"text": " ".join(str(int(v)) for v in fb[:3])},
                    {"text": " ".join(str(int(v)) for v in mb[:3])},
                    common.cell(radius),
                    common.cell(float(corr)),
                    common.cell(err),
                    {"text": ("landmark" if kk in a_land else "paired") if kk in a_keys else "–"},
                    {"text": ("landmark" if kk in f_land else "paired") if kk in f_keys else "–"},
                ],
            }
        )
    return common.table(
        tid,
        title,
        [
            "Pair",
            "fixed blob (voxel)",
            "moving blob (voxel)",
            {"label": "radius (mm)", "num": True},
            {"label": "patch correlation", "num": True},
            {"label": "off the true transform (mm)", "num": True},
            "ANTs",
            "larmorx's default",
        ],
        rows,
        note=note,
    )


def _side(steps: dict) -> dict:
    """larmorx's steps in the probe's shape (blob rows: index, radius, value)."""
    corr = np.asarray(steps.get("correspondence", []), np.float64)
    return {
        "fixed_blobs": [[*b[:3], b[4], b[5]] for b in steps.get("fixed_blobs", [])],
        "moving_blobs": [[*b[:3], b[4], b[5]] for b in steps.get("moving_blobs", [])],
        "corr": {
            (i, j): float(corr[i, j]) for i in range(corr.shape[0]) for j in range(corr.shape[1])
        }
        if corr.size
        else {},
        "pairs": steps.get("pairs", []),
        "landmarks": steps.get("landmarks", []),
    }


def _same_steps(ants: dict, lx: dict) -> bool:
    """ANTs' steps (the probe) and larmorx's ``reproduce`` steps are the same numbers."""
    if ants["pairs"] != lx["pairs"] or ants["landmarks"] != lx["landmarks"]:
        return False
    for k in ("fixed_blobs", "moving_blobs"):
        if len(ants[k]) != len(lx[k]):
            return False
        for a, b in zip(ants[k], lx[k], strict=True):
            if list(a[:3]) != list(b[:3]) or a[3] != b[3] or a[4] != b[4]:
                return False
    return all(lx["corr"].get(ij, 0.0) == v for ij, v in ants["corr"].items()) and all(
        ants["corr"].get(ij, 0.0) == v for ij, v in lx["corr"].items() if v != 0.0
    )


def _notices(steps: dict) -> list[str]:
    return sorted(
        {m.split(" ")[0] for m in steps.get("warnings", []) if re.match(r"[A-Z]+-\d+ ", m)}
    )


def _transform_tables(move: str, mats: dict, pts: np.ndarray, crashed: dict, same_file) -> tuple:
    def row(label, a, b, cls="", sub=""):
        if a not in mats or b not in mats:
            cells = [
                {"text": "identical file" if same_file(a, b) else "differ"},
                {"text": "no transform (NaN)"},
                {"text": "–"},
                {"text": "–"},
            ]
        else:
            d = _distance(mats[a][0], mats[b][0], pts)
            cells = [
                {"text": "identical file" if same_file(a, b) else "differ"},
                common.cell(d["max"]),
                common.cell(d["mean"]),
                common.cell(float(np.abs(mats[a][0][:3] - mats[b][0][:3]).max())),
            ]
        return {
            "head": label,
            **({"class": cls} if cls else {}),
            **({"sub": sub} if sub else {}),
            "cells": cells,
        }

    rows = [
        row("larmorx (reproduce) − ANTs, 1 thread", "larmorx", "ants-1"),
        row("larmorx (reproduce) − ANTs with CORE-MATH", "larmorx", "ants-1-cr"),
        row("larmorx 1 thread − 6 threads", "larmorx-1", "larmorx"),
        row("larmorx's default − ANTs", "larmorx-fix", "ants-1"),
        row("ANTs − the true transform", "ants-1", "truth", "is-sep"),
        row("larmorx's default − the true transform", "larmorx-fix", "truth"),
        row("ANTs, 1 thread: run 2 − run 1", "ants-1b", "ants-1", "is-sep is-yardstick"),
        row("ANTs with CORE-MATH − as built", "ants-1-cr", "ants-1", "is-yardstick"),
    ]
    for n, code in crashed.items():
        rows.append(
            {
                "head": f"ANTs, {n} threads",
                "class": "is-yardstick",
                "cells": [
                    {
                        "text": f"crashed (signal {-code})"
                        if code is not None and code < 0
                        else f"exit {code}"
                    },
                    {"text": "–"},
                    {"text": "–"},
                    {"text": "–"},
                ],
            }
        )
    dist = common.table(
        f"blobs-{move}-distance",
        "Where two transforms send the T1w's brain apart (mm), and how far their matrices differ",
        [
            "Comparison",
            "the .mat files",
            {"label": "largest distance (mm)", "num": True},
            {"label": "mean distance (mm)", "num": True},
            {"label": "largest |Δ| in the matrix or translation", "num": True},
        ],
        rows,
        note=f"Over the {len(pts):,} voxels of fMRIPrep's brain mask of the fixed T1w: each "
        "transform maps them into the moving image; the distance is between the two images of "
        "each point. The shaded rows are ANTs against itself: at more than one thread its blob "
        "detector writes past its per-thread heaps (ANTS-60).",
    )
    names = [f"matrix {i},{j}" for i in (1, 2, 3) for j in (1, 2, 3)]
    names += ["translation x", "translation y", "translation z"]
    cols = [k for k in ("truth", "ants-1", "larmorx", "larmorx-fix") if k in mats]
    label = {
        "truth": "true transform",
        "ants-1": "ANTs 2.6.5, 1 thread",
        "larmorx": "larmorx (reproduce)",
        "larmorx-fix": "larmorx's default",
    }
    params = common.table(
        f"blobs-{move}-params",
        "The transforms as matrices (ITK's physical space, LPS mm, centre 0)",
        ["Entry", *({"label": label[k], "num": True} for k in cols)],
        [
            {
                "head": n,
                "cells": [
                    common.cell(
                        float(mats[k][0][j // 3, j % 3] if j < 9 else mats[k][0][j - 9, 3]),
                        "sig6",
                    )
                    for k in cols
                ],
            }
            for j, n in enumerate(names)
        ],
        note="antsAI's output (an Euler3DTransform for Rigid): the blob transform after its "
        "search of rotations about the image axes (with no iterations, the best start point), "
        "written as the matrix and translation that map a fixed point to the moving image; the "
        "true transform as constructed. A transform of NaNs (no landmark) is left out.",
    )
    return dist, params


def _example(ctx: core.Context, move: str) -> dict:
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = entry(ctx)
    info = e.info()
    run_dir = common.fmriprep_run(RUN)
    fixed = run_dir / BRAIN.replace("desc-brain_mask", "desc-preproc_T1w")
    fimg, brain = _brain(fixed, run_dir / BRAIN)
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(fimg, show)
    ex = exm.Example(ctx, grid, show)
    fdata = common.load(str(fixed))

    ants = _probe_parse(e.dir / f"{move}-probe.log")
    steps = {
        m: json.loads((e.dir / f"{move}-steps-{m}.json").read_text()) for m in ("reproduce", "fix")
    }
    lx_rep, lx_fix = _side(steps["reproduce"]), _side(steps["fix"])
    same_steps = _same_steps(ants, lx_rep)

    def placed(label: str) -> np.ndarray | None:
        p = e.dir / f"{move}-place-{label}.nii.gz"
        return common.load(str(p)) if p.is_file() else None

    _bid, _key, what = MOVES[move]
    ex.add(
        exm.Panel(
            "fixed",
            "input",
            "ds000005 sub-01 T1w (fixed)",
            "fMRIPrep's preprocessed T1w",
            {"all": fdata},
        )
    )
    ex.add(
        exm.Panel(
            "moving",
            "input",
            "its copy (moving), unaligned",
            f"{what}; × its brain mask, on the fixed grid with no transform",
            {"all": placed("identity")},
        )
    )
    ex.add(
        exm.Panel(
            "truth",
            "input",
            "its copy placed by the true transform",
            "the transform known by construction (antsApplyTransforms as built, linear)",
            {"all": placed("truth")},
        )
    )
    outputs = [{"id": "placed", "label": "moving T1w placed on the fixed grid", "unit": ""}]
    runs = [
        (
            "ants",
            "ants-1",
            ants,
            "original",
            "antsAI: ANTs 2.6.5, 1 thread",
            "the copy placed by its transform",
        ),
        (
            "larmorx",
            "larmorx",
            lx_rep,
            "larmorx",
            "antsAI: larmorx (port)",
            "the copy placed by its transform; reproduce mode, 6 threads",
        ),
        (
            "larmorx-fix",
            "larmorx-fix",
            lx_fix,
            "larmorx",
            "antsAI: larmorx (port)",
            "the copy placed by its transform; default mode (ANTS-110 corrected), 6 threads",
        ),
    ]
    for pid, label, _steps, role, text, sub in runs:
        arr = placed(label)
        if arr is None:
            continue
        ex.add(exm.Panel(pid, role, text, sub, {"placed": arr}))
    shown = {p.id for p in ex.panels}
    dd = common.d
    diffs = []
    if {"larmorx", "ants"} <= shown:
        diffs.append(
            dd(
                "lx-ants",
                "larmorx",
                "ants",
                "main",
                "larmorx (reproduce) − ANTs, 1 thread",
                common.ROLE_LX,
            )
        )
    if {"larmorx-fix", "ants"} <= shown:
        diffs.append(
            dd(
                "fix-ants",
                "larmorx-fix",
                "ants",
                "main",
                "larmorx's default − ANTs, 1 thread",
                common.ROLE_LX,
            )
        )
    for pid, text in (("ants", "ANTs"), ("larmorx-fix", "larmorx's default")):
        if pid in shown:
            diffs.append(
                dd(
                    f"{pid}-truth",
                    pid,
                    "truth",
                    "reference",
                    f"{text} − the true transform",
                    "against the truth",
                )
            )
    common.compare_all(ex, diffs, outputs)
    same_img = common.identical(diffs)

    mats = {}
    for k in ("truth", "ants-1", "ants-1b", "ants-1-cr", "larmorx", "larmorx-1", "larmorx-fix"):
        m, p = _affine(e.dir / f"{move}-{k}.mat")
        if np.all(np.isfinite(p)):
            mats[k] = (m, p)
    # the tables' truth from the construction (the placed copy used the fit in truth.mat,
    # within 1e-12 mm of it)
    exact = _truth_exact(move, fimg)
    assert np.abs(exact - mats["truth"][0]).max() < 1e-9
    mats["truth"] = (exact, mats["truth"][1])
    pts = _points(fimg, brain)

    def same_file(a, b):
        return (e.dir / f"{move}-{a}.mat").read_bytes() == (e.dir / f"{move}-{b}.mat").read_bytes()

    crashed = {n: _exited(e, f"{move}-ants-{n}") for n in BAND}
    dist, params = _transform_tables(move, mats, pts, crashed, same_file)
    truth = (mats["truth"][0][:3, :3], mats["truth"][0][:3, 3])
    import nibabel as nib

    mimg = nib.load(str(e.dir / f"{move}-moving.nii.gz"))
    pairs = _pairs_table(
        f"blobs-{move}-pairs",
        "The pairs: each fixed blob matched to a moving blob, and which pairs became landmarks",
        ants,
        lx_fix,
        fimg.affine,
        mimg.affine,
        truth,
        "ANTs' blobs, correlations, pairs and landmarks from the probe (antsAI's own code); "
        "larmorx's default from lx.ants.blob_alignment. A pair is a landmark when its distances "
        "to at least 3 other pairs are preserved within 10 % (squared); ANTs tests the lengths "
        "of the pairs' displacements instead (ANTS-110), so a rotation leaves it no landmark. "
        "'Off the true transform': how far the moving blob is from where the true transform "
        "sends the fixed blob's centre.",
    )
    fix_named = _notices(steps["fix"])
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if x["kind"] == "reference":
            x["verdict"] = None
            k = {"ants": "ants-1", "larmorx-fix": "larmorx-fix"}[x["a"]]
            d = _distance(mats[k][0], mats["truth"][0], pts)
            angle = _angle(mats[k][0][:3, :3])
            x["reason"] = (
                "The same placed image as the true transform's."
                if same_img[i]
                else f"A rotation of {angle:.2f}° (the truth: {_angle(mats['truth'][0][:3, :3]):.2f}°); "
                f"it sends the brain at most {d['max']:.2g} mm (mean {d['mean']:.2g} mm) from "
                "where the true transform does."
            )
        elif i == "lx-ants":
            ok = same_file("larmorx", "ants-1") and same_steps
            x["verdict"] = "bit-identical" if ok else "open"
            x["reason"] = (
                "The same transform file, byte for byte, and the same blobs, correlations, pairs "
                "and landmarks as ANTs' own code computes them (the probe)."
                if ok
                else "Not accounted for by the other runs of this example."
            )
        elif i == "fix-ants":
            d_fix = _distance(mats["larmorx-fix"][0], mats["truth"][0], pts)["max"]
            d_ants = _distance(mats["ants-1"][0], mats["truth"][0], pts)["max"]
            x["verdict"] = "d13" if fix_named else "open"
            x["reason"] = (
                f"{', '.join(fix_named)} corrected by default: a pair is a landmark when its "
                "distances to three other pairs are preserved (as the code's comment and "
                "ImageMath's MatchBlobs say), where ANTs compares the lengths of the pairs' "
                f"displacements. Here larmorx keeps {len(lx_fix['landmarks'])} of "
                f"{len(lx_fix['pairs'])} pairs and ANTs {len(ants['landmarks'])}: for a pure "
                "translation ANTs' test is the stricter one: larmorx's transform is "
                f"{d_fix:.2g} mm off the truth at most, ANTs' is exact up to rounding "
                f"({d_ants:.1g} mm; the tables below). For a rotation ANTs keeps no landmark (the "
                "next example)."
                if fix_named
                else "Not accounted for by the other runs of this example."
            )
            x["links"] = [{"label": "ANTS-110", "href": "docs/findings/ants-ai.md#blobs"}]
    if move == "shift":
        at, _k = common.largest_in_slice(ex, "larmorx-fix", "ants", "placed")
        default = {
            "output": "placed",
            "plane": "axial",
            "voxel": at,
            "panels": ["fixed", "moving", "ants", "larmorx-fix"],
            "diffs": ["lx-ants", "fix-ants", "ants-truth"],
            "compare": {"a": "fixed", "b": "larmorx-fix"},
        }
        window_panel = "ants"
    else:
        at, _k = common.largest_in_slice(ex, "larmorx-fix", "truth", "placed")
        default = {
            "output": "placed",
            "plane": "axial",
            "voxel": at,
            "panels": ["fixed", "moving", "truth", "larmorx-fix"],
            "diffs": ["larmorx-fix-truth"],
            "compare": {"a": "fixed", "b": "larmorx-fix"},
        }
        window_panel = "larmorx-fix"
    out = common.finish(
        ctx,
        ex,
        diffs,
        outputs,
        default=default,
        window_panel=window_panel,
        scale="log",
        caption="the fixed T1w, its copy before and after each transform; Checkerboard (under "
        "Compare) overlays the fixed T1w and a placed copy.",
    )
    vers = info.get("versions", {})
    facts = [
        {
            "label": "Data",
            "text": f"{common.DS000005['dataset']}, sub-01's T1w ({common.DS000005['licence']}), "
            f"as fMRIPrep 25.2.6 preprocessed it; the moving image: {what}",
        },
        {
            "label": "Command",
            "text": "`antsAI -d 3 -m Mattes[fixed.nii.gz,moving.nii.gz,32,Regular,0.25] -t "
            "Rigid[0.1] -b 30 -o t.mat -v 1` (the record's arguments)",
        },
        {
            "label": "Programs",
            "text": f"{vers.get('ants', '?')}; ANTs' steps from the blob probe "
            "(`validation/probes/ants-ai-blobs`, antsAI's code with prints, built against the "
            f"same ITK); larmorx at commit `{info['larmorx_commit'][:12]}`",
        },
        {"label": "Grid", "text": common.describe_grid(fimg, grid.shape)},
    ]
    if move == "rotated":
        facts.append(
            {
                "label": "ANTs",
                "text": "no pair keeps the lengths of its displacement (ANTS-110), so no "
                "landmark: ANTs' rigid fit divides 0 by 0 and antsAI writes a transform of NaNs "
                "(ANTS-114), which places nothing. larmorx's reproduce mode writes the same file.",
            }
        )
    tables = [dist, pairs, params]
    title = {
        "shift": "Blob alignment of a T1w and its shifted copy, by ANTs and larmorx",
        "rotated": "Blob alignment of a T1w and its rotated copy: ANTs finds no landmark",
    }[move]
    summary = {
        "shift": "antsAI's blob alignment (`-b 30`) between fMRIPrep's T1w of ds000005 sub-01 "
        "and the same voxels with the origin moved by (6, −4, 3) mm, so the true transform is "
        "known. Each program's pairs and landmarks are listed, and its transform places the copy "
        "back on the fixed grid.",
        "rotated": "The same T1w against a copy rotated by 8° and shifted: ANTs' distance test "
        "keeps no landmark and its transform is NaN; larmorx's corrected test keeps the pairs "
        "whose distances are preserved and finds a rotation close to the truth.",
    }[move]
    return {
        "id": f"blobs-{move}",
        "title": title,
        "summary": summary,
        "facts": facts,
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against ANTs"],
            ["reference", "Each against the true transform (known by construction)"],
        ],
        "tables": tables,
        "metrics_note": "Δ is A − B between the placed images, over every voxel of the "
        f"{' × '.join(map(str, fimg.shape))} grid.",
        "provenance": common.provenance(info, f"{TOOL}/{CASE}", [common.DS000005]),
    }


def _blobs_example(ctx: core.Context) -> dict:
    """The blobs each program found in the fixed T1w, drawn as spheres of their radius."""
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = entry(ctx)
    info = e.info()
    run_dir = common.fmriprep_run(RUN)
    fixed = run_dir / BRAIN.replace("desc-brain_mask", "desc-preproc_T1w")
    fimg, brain = _brain(fixed, run_dir / BRAIN)
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(fimg, show)
    ex = exm.Example(ctx, grid, show)
    zooms = fimg.header.get_zooms()
    fdata = common.load(str(fixed))
    hi = float(np.percentile(fdata[brain], 99.5)) * 1.3
    ants = _probe_parse(e.dir / "shift-probe.log")
    rotated = _probe_parse(e.dir / "rotated-probe.log")
    steps = {
        m: _side(json.loads((e.dir / f"shift-steps-{m}.json").read_text()))
        for m in ("reproduce", "fix")
    }
    # the fixed image is the same in both moves: so are its blobs
    assert ants["fixed_blobs"] == rotated["fixed_blobs"]
    ex.add(
        exm.Panel(
            "fixed",
            "input",
            "ds000005 sub-01 T1w (fixed)",
            "fMRIPrep's preprocessed T1w",
            {"all": fdata},
        )
    )
    for pid, side, role, text, sub_ in (
        ("ants", ants, "original", "ANTs 2.6.5's blobs", "antsAI's code with prints (the probe)"),
        ("larmorx", steps["reproduce"], "larmorx", "larmorx's blobs", "reproduce mode, 6 threads"),
        (
            "larmorx-fix",
            steps["fix"],
            "larmorx",
            "larmorx's blobs",
            "default mode (ANTS-112 corrected)",
        ),
    ):
        ex.add(
            exm.Panel(
                pid, role, text, sub_, {"blobs": _drawn(fdata, side["fixed_blobs"], zooms, hi)}
            )
        )
    dd = common.d
    diffs = [
        dd("lx-ants", "larmorx", "ants", "main", "larmorx (reproduce) − ANTs", common.ROLE_LX),
        dd("fix-ants", "larmorx-fix", "ants", "main", "larmorx's default − ANTs", common.ROLE_LX),
    ]
    outputs = [
        {
            "id": "blobs",
            "label": "the blobs: spheres of their radius over the T1w at half intensity",
            "unit": "",
        }
    ]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    exact = _same_steps(ants, steps["reproduce"])
    for x in diffs:
        x["links"] = []
        if x["id"] == "lx-ants":
            ok = same["lx-ants"] and exact
            x["verdict"] = "bit-identical" if ok else "open"
            x["reason"] = (
                f"The same {len(ants['fixed_blobs'])} blobs, each at the same voxel with the same "
                "radius and the same Laplacian value, bit for bit, as ANTs' own code finds them."
                if ok
                else "Not accounted for."
            )
        else:
            x["verdict"] = "bit-identical" if same["fix-ants"] else "d13"
            x["reason"] = (
                "ANTS-112's correction keeps the strongest maxima; here the largest scales fill "
                "the list before ANTs' early threshold can drop one, so both lists are the same."
                if same["fix-ants"]
                else "ANTS-112 corrected by default: the strongest maxima."
            )
            x["links"] = [{"label": "ANTS-112", "href": "docs/findings/ants-ai.md#blobs"}]
    radii = sorted(b[3] for b in ants["fixed_blobs"])
    # the view: the axial slice (display grid) with the most blob voxels, the crosshair at the
    # centre of the largest blob that slice cuts
    to_disp = np.linalg.inv(grid.to_file())
    shown = ex.values("ants:blobs") >= float(np.float32(hi))
    k = int(np.argmax(shown.sum(axis=(0, 1))))
    cut = [
        b
        for b in ants["fixed_blobs"]
        if abs((to_disp @ np.r_[b[:3], 1.0])[2] - k) * grid.spacing[2] <= b[3]
    ]
    centre = max(cut or ants["fixed_blobs"], key=lambda b: b[3])
    at = [int(v) for v in np.rint((to_disp @ np.r_[centre[:3], 1.0])[:3])]
    at[2] = k
    default = {
        "output": "blobs",
        "plane": "axial",
        "voxel": at,
        "panels": ["fixed", "ants", "larmorx", "larmorx-fix"],
        "diffs": ["lx-ants", "fix-ants"],
        "compare": {"a": "fixed", "b": "ants"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        outputs,
        default=default,
        window_panel="ants",
        caption="each program's blobs, filled spheres over the fixed T1w at half its intensity "
        "(the crosshair at the largest blob this slice cuts).",
    )
    vers = info.get("versions", {})
    return {
        "id": "blobs-found",
        "title": "The blobs ANTs and larmorx find in a T1w",
        "summary": f"antsAI `-b 30` keeps the {len(ants['fixed_blobs'])} strongest maxima of the "
        "scale-normalised Laplacian of Gaussian over 53 scales; here they lie at the edges of "
        f"ventricles, vessels and sulci, with radii from {radii[0]:.2g} to {radii[-1]:.2g} mm. "
        "Each is drawn as a filled sphere of its radius around its voxel, over the T1w at half "
        "its intensity.",
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01's T1w ({common.DS000005['licence']}), "
                "as fMRIPrep 25.2.6 preprocessed it",
            },
            {
                "label": "Programs",
                "text": f"{vers.get('ants', '?')}; ANTs' blobs from the blob probe "
                "(`validation/probes/ants-ai-blobs`, antsAI's code with prints, built against the "
                f"same ITK); larmorx's from `lx.ants.blob_alignment` at commit "
                f"`{info['larmorx_commit'][:12]}`",
            },
            {"label": "Grid", "text": common.describe_grid(fimg, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [["main", "LARMORX against ANTs"]],
        "tables": [],
        "metrics_note": "Δ is A − B between the drawn blobs, over every voxel of the "
        f"{' × '.join(map(str, fimg.shape))} grid.",
        "provenance": common.provenance(info, f"{TOOL}/{CASE}", [common.DS000005]),
    }


def examples(ctx: core.Context) -> list[dict]:
    return [_blobs_example(ctx), _example(ctx, "shift"), _example(ctx, "rotated")]


def provenance(ctx: core.Context) -> dict:
    return fcache.provenance(entry(ctx))


if __name__ == "__main__":
    if len(sys.argv) == 7 and sys.argv[1] == "dump":
        dump(sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5], int(sys.argv[6]))
    else:
        sys.exit(
            "usage: python -m larmorx_validation.evidence.ants_ai_blobs dump FIXED MOVING OUT MODE THREADS"
        )
