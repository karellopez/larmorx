# SPDX-License-Identifier: Apache-2.0
"""PIPE-03 on real geometry: fMRIPrep's Jacobian against the corrected one.

fMRIPrep's ``ResampleSeries`` multiplies the resampled BOLD by ``1 + np.gradient(vsm,
axis=pe)``: the gradient of the voxel-shift map along the *target* axis that has the source's
phase-encoding index, in target voxels (register entry PIPE-03). larmorx's default takes the
derivative along the phase-encoding direction mapped into the target grid. This module
measures the difference on the grids and transforms of a real fMRIPrep run: its BOLD
reference (native), T1w and MNI152NLin2009cAsym (res-2) grids, its boldref→T1w affine and its
T1w→MNI warp. The field map is synthetic but shaped like a 3 T susceptibility field (a
positive orbitofrontal lobe of 120 Hz and two negative temporal ones of 80 Hz), defined in the
BOLD reference's world space and carried to each target grid through the run's transforms, as
fMRIPrep's ``ReconstructFieldmap`` does; the readout is 30 ms along the BOLD's second axis
(``j``), so the largest shift is 3.6 voxels. The source is a volume of ones, so the output is
the Jacobian factor itself, and a difference of 0.01 in the factor is a 1 % difference in the
corrected BOLD signal.

``python -m larmorx_validation.parity.resample_jacobian --run <fMRIPrep output subject dir>
--out docs/validation`` writes ``resample-series-jacobian.md`` and its JSON (without ``--out``
it prints the Markdown).
"""

from __future__ import annotations

import argparse
import json
import time
import warnings
from pathlib import Path

import numpy as np

#: (offset from the brain's centre as a fraction of its half-extent along x, y, z; Hz; sigma in mm)
BLOBS = (
    ((0.0, 0.55, -0.35), 120.0, 16.0),  # orbitofrontal
    ((-0.55, 0.05, -0.45), -80.0, 14.0),  # left temporal
    ((0.55, 0.05, -0.45), -80.0, 14.0),  # right temporal
)
READOUT = 0.03
PE_DIR = "j"


def _grid_points(shape: tuple[int, ...], affine: np.ndarray) -> np.ndarray:
    """World coordinates of every voxel, in Fortran (NIfTI) order."""
    idx = np.indices(shape[:3]).reshape(3, -1, order="F").T.astype(np.float64)
    return idx @ affine[:3, :3].T + affine[:3, 3]


def _field(points: np.ndarray, centre: np.ndarray, half: np.ndarray) -> np.ndarray:
    f = np.zeros(len(points))
    for offset, hz, sigma in BLOBS:
        c = centre + np.asarray(offset) * half
        f += hz * np.exp(-np.sum((points - c) ** 2, axis=1) / (2 * sigma * sigma))
    return f


def _gradient(points: np.ndarray, centre: np.ndarray, half: np.ndarray) -> np.ndarray:
    """The field's exact gradient (Hz/mm)."""
    g = np.zeros_like(points)
    for offset, hz, sigma in BLOBS:
        d = points - (centre + np.asarray(offset) * half)
        w = hz * np.exp(-np.sum(d**2, axis=1) / (2 * sigma * sigma)) / (sigma * sigma)
        g -= w[:, None] * d
    return g


def _first(directory: Path, pattern: str, exclude: str | None = None) -> Path:
    return next(p for p in sorted(directory.glob(pattern)) if not exclude or exclude not in p.name)


def measure(run: Path) -> list[dict]:
    import larmorx as lx

    func = run / "func"
    anat = run / "anat"
    stem = _first(func, "*_desc-hmc_boldref.nii.gz").name.split("_desc-hmc_boldref")[0]
    boldref = lx.load(func / f"{stem}_desc-hmc_boldref.nii.gz")
    mask = lx.load(func / f"{stem}_desc-brain_mask.nii.gz")
    coreg = func / f"{stem}_from-boldref_to-T1w_mode-image_desc-coreg_xfm.txt"
    to_mni = _first(anat, "*_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5")
    inside = np.asarray(mask.data).reshape(-1, order="F") > 0
    pts = _grid_points(mask.shape, mask.affine)[inside]
    centre = pts.mean(axis=0)
    half = (pts.max(axis=0) - pts.min(axis=0)) / 2
    ones = lx.Image(np.ones(boldref.shape[:3], np.float32), boldref.affine, boldref.header)
    # The phase-encoding axis in world space (mm per voxel), after ensure_positive_cosines.
    pe_col = boldref.affine[:3, "ijk".index(PE_DIR)]
    pe_col = pe_col * np.sign(pe_col[np.argmax(np.abs(pe_col))])
    targets = [
        ("native (BOLD reference)", func / f"{stem}_desc-hmc_boldref.nii.gz", [], mask),
        (
            "T1w",
            _first(anat, "*_desc-preproc_T1w.nii.gz", exclude="space-"),
            [coreg],
            lx.load(_first(anat, "*_desc-brain_mask.nii.gz", exclude="space-")),
        ),
        (
            "T1w at the BOLD's voxel size (fMRIPrep's `space-T1w` BOLD)",
            func / f"{stem}_space-T1w_boldref.nii.gz",
            [coreg],
            lx.load(func / f"{stem}_space-T1w_desc-brain_mask.nii.gz"),
        ),
        (
            "MNI152NLin2009cAsym res-2",
            func / f"{stem}_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz",
            [coreg, to_mni],
            lx.load(func / f"{stem}_space-MNI152NLin2009cAsym_res-2_desc-brain_mask.nii.gz"),
        ),
    ]
    rows = []
    for name, target, xfms, target_mask in targets:
        t = lx.load(target)
        world = _grid_points(t.shape, t.affine)
        # fMRIPrep's derivative steps over one voxel along target axis j: that step in the
        # BOLD reference's voxels, at the grid's centre (length 1 and angle 0 on its own grid)
        mid = (np.array(t.shape[:3], float) - 1) / 2
        step = np.array([mid, mid + np.eye(3)[1]]) @ t.affine[:3, :3].T + t.affine[:3, 3]
        if xfms:
            chain = lx.transforms.load_transforms([str(x) for x in xfms])
            world = chain.map(world, n_threads=0)
            step = chain.map(step, n_threads=1)
        step = (np.c_[step, np.ones(2)] @ np.linalg.inv(boldref.affine).T)[:, :3]
        d = step[1] - step[0]
        step_text = (
            f"{np.linalg.norm(d):.2f} / "
            f"{np.degrees(np.arccos(min(1.0, abs(d[1]) / np.linalg.norm(d)))):.0f}°"
        )
        fmap = _field(world, centre, half).reshape(t.shape[:3], order="F").astype(np.float32)
        out = {}
        for mode in ("fix", "reproduce"):
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                out[mode] = lx.transforms.resample_series(
                    ones,
                    target,
                    [str(x) for x in xfms],
                    fieldmap=fmap,
                    pe_dir=PE_DIR,
                    ro_time=READOUT,
                    n_threads=0,
                    upstream_defects=mode,
                ).data
        brain = np.asarray(target_mask.data) > 0
        a = np.asarray(out["fix"])[brain].astype(np.float64)
        b = np.asarray(out["reproduce"])[brain].astype(np.float64)
        diff = np.abs(a - b)
        # The exact factor at each voxel: 1 + ∂vsm/∂s_pe at the source point it maps to.
        slope = _gradient(world, centre, half) @ pe_col
        exact = (1 + READOUT * slope).reshape(t.shape[:3], order="F")[brain]
        err_fix, err_fmriprep = np.abs(a - exact), np.abs(b - exact)
        rows.append(
            {
                "target": name,
                "shape": "×".join(map(str, t.shape[:3])),
                "zooms": "×".join(f"{z:.3g}" for z in t.header.zooms[:3]),
                "step": step_text,
                "voxels": int(brain.sum()),
                "fix_range": (float(a.min()), float(a.max())),
                "fmriprep_range": (float(b.min()), float(b.max())),
                "identical": int(np.sum(a == b)),
                "over_001": float(np.mean(diff > 0.01)),
                "over_005": float(np.mean(diff > 0.05)),
                "over_02": float(np.mean(diff > 0.2)),
                "median_diff": float(np.median(diff)),
                "max_diff": float(diff.max()),
                "exact_range": (float(exact.min()), float(exact.max())),
                "err_fix": [float(np.median(err_fix)), float(np.percentile(err_fix, 99))],
                "err_fmriprep": [
                    float(np.median(err_fmriprep)),
                    float(np.percentile(err_fmriprep, 99)),
                ],
            }
        )
    return rows


def _shown(run: Path) -> str:
    """The run directory from ``oracles/`` on, so the record names no home directory."""
    parts = run.resolve().parts
    return "/".join(parts[parts.index("oracles") :]) if "oracles" in parts else run.name


def report(rows: list[dict], run: Path) -> str:
    lines = [
        "# PIPE-03: fMRIPrep's Jacobian against the corrected one, on real geometry",
        "",
        "fMRIPrep's `ResampleSeries` multiplies the distortion-corrected BOLD by "
        "`1 + np.gradient(vsm, axis=pe)`, the gradient of the voxel-shift map along the *target* "
        "axis that has the source's phase-encoding index, in target voxels "
        "([register](../findings/upstream-defects.md), PIPE-03). That is the Jacobian only when "
        "the target grid is the BOLD's own. larmorx's default differentiates along the "
        "phase-encoding direction mapped into the target grid, `1 + ∇vsm · ∂t/∂s_pe` "
        "([resample-series.md](../api/resample-series.md)); "
        '`upstream_defects="reproduce"` gives fMRIPrep\'s factor bit for bit.',
        "",
        f"Geometry from the fMRIPrep 25.2.6 run `{_shown(run)}`: its BOLD reference (native "
        "grid), its T1w and MNI152NLin2009cAsym res-2 grids, its boldref→T1w affine and its "
        "T1w→MNI warp. The field map is synthetic, shaped like a 3 T susceptibility field (an "
        "orbitofrontal lobe of +120 Hz, sigma 16 mm, and two temporal lobes of -80 Hz, sigma 14 mm), "
        "defined in the BOLD reference's world space and carried to each target grid through the "
        "run's transforms, as `ReconstructFieldmap` does; the readout is 30 ms along `j`, so "
        "shifts reach 3.6 voxels. The source is a volume of ones, so the output is the Jacobian "
        "factor itself: a difference of 0.01 is a 1 % difference in the corrected signal. "
        "Voxels inside each grid's brain mask.",
        "",
        "Generated with `python -m larmorx_validation.parity.resample_jacobian --run <run>/out/"
        f"<subject> --out docs/validation` ({time.strftime('%Y-%m-%d')}).",
        "",
        "## Corrected against fMRIPrep",
        "",
        "| Target grid | Shape | Voxel size (mm) | fMRIPrep's step (BOLD voxels / angle to `j`) "
        "| Brain voxels | Jacobian, corrected | "
        "Jacobian, fMRIPrep | Identical | Differ by > 0.01 | > 0.05 | > 0.2 | "
        "Median / max difference |",
        "|---|---|---|---|---|---|---|---|---|---|---|---|",
    ]
    for r in rows:
        lines.append(
            f"| {r['target']} | {r['shape']} | {r['zooms']} | {r.get('step', '–')} | "
            f"{r['voxels']} | "
            f"{r['fix_range'][0]:.3f} to {r['fix_range'][1]:.3f} | "
            f"{r['fmriprep_range'][0]:.3f} to {r['fmriprep_range'][1]:.3f} | "
            f"{100 * r['identical'] / r['voxels']:.1f} % | {100 * r['over_001']:.1f} % | "
            f"{100 * r['over_005']:.1f} % | {100 * r['over_02']:.1f} % | "
            f"{r['median_diff']:.4f} / {r['max_diff']:.3f} |"
        )
    lines += [
        "",
        "## Against the exact factor",
        "",
        "The exact factor is `1 + T·∂f/∂s_pe` (T the readout time, f the field in Hz, s_pe the "
        "BOLD's phase-encoding voxel coordinate), from the field's analytic derivative at the "
        "source point each target voxel maps to. Both methods "
        "differentiate numerically (central differences on the target grid), which explains "
        "the corrected factor's small error.",
        "",
        "| Target grid | Exact factor | Error, corrected (median / 99th percentile) | "
        "Error, fMRIPrep (median / 99th percentile) |",
        "|---|---|---|---|",
    ]
    for r in rows:
        lines.append(
            f"| {r['target']} | {r['exact_range'][0]:.3f} to {r['exact_range'][1]:.3f} | "
            f"{r['err_fix'][0]:.4f} / {r['err_fix'][1]:.4f} | "
            f"{r['err_fmriprep'][0]:.4f} / {r['err_fmriprep'][1]:.4f} |"
        )
    lines += [
        "",
        "On the BOLD's own grid the two are bit-identical. On the T1w and MNI grids fMRIPrep's "
        "factor is compressed towards 1 (its derivative is taken in finer target voxels, and "
        "along whichever anatomical direction the target's second axis has), so it undoes "
        "only part of the intensity pile-up and spread of the distortion; the corrected factor "
        "follows the exact one. fMRIPrep's own `space-T1w` BOLD is not on the 1 mm T1w grid but "
        "on the T1w's orientation at the BOLD's voxel size (`bold_anat_wf/gen_ref`, "
        "`keep_native`): there the voxel sizes match, and only the angle between the BOLD's "
        "axes and the T1w's (through the boldref→T1w transform) separates the two factors. "
        '"fMRIPrep\'s step" is what its derivative steps over, at the centre of the grid: one '
        "voxel along target axis `j`, in the BOLD reference's voxels; the Jacobian needs one "
        'BOLD voxel along `j` (1 / 0°). In this run the angle is large, so the row "T1w at '
        "the BOLD's voxel size\" differs almost as much as the 1 mm grid; with a small angle "
        "the difference is small (ds005040, below).",
        "",
    ]
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> None:
    from larmorx_validation.parity.resample_jacobian_oracle import (
        measure_oracle,
        report_changes,
        report_oracle,
        stage_changes,
    )

    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--run", type=Path, required=True, help="fMRIPrep output directory of a subject")
    p.add_argument(
        "--oracle",
        type=Path,
        action="append",
        default=[],
        help="an fMRIPrep oracle run directory with field maps (repeatable): the factors on "
        "fMRIPrep's own field maps",
    )
    p.add_argument("--out", type=Path, help="write resample-series-jacobian.md/.json here")
    args = p.parse_args(argv)
    rows = measure(args.run)
    text = report(rows, args.run)
    oracle = {_shown(r).split("/")[-1]: measure_oracle(r) for r in args.oracle}
    if oracle:
        command = (
            "python -m larmorx_validation.parity.resample_jacobian --run <run>/out/<subject> "
            + " ".join(f"--oracle <workspace>/{_shown(r)}" for r in args.oracle)
            + " --out docs/validation"
        )
        text = text.replace(
            "Generated with `python -m larmorx_validation.parity.resample_jacobian --run "
            "<run>/out/<subject> --out docs/validation`",
            f"Generated with `{command}`",
        )
        text += "\n" + "\n".join(report_oracle(oracle))
        records = args.out if args.out is not None else Path("docs/validation")
        changes = {name: stage_changes(records, name) for name in oracle}
        text += "\n" + "\n".join(report_changes({k: v for k, v in changes.items() if v}))
    if args.out is None:
        print(text)
        return
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "resample-series-jacobian.json").write_text(
        json.dumps({"run": _shown(args.run), "rows": rows, "oracle": oracle}, indent=1),
        encoding="utf-8",
    )
    (args.out / "resample-series-jacobian.md").write_text(text, encoding="utf-8")


if __name__ == "__main__":
    main()
