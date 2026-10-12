# SPDX-License-Identifier: Apache-2.0
"""PIPE-03 on fMRIPrep's own field maps: the two Jacobian factors against a reference that
does not depend on the target grid's axes.

For every BOLD run of an fMRIPrep oracle run that was resampled with a field map, the factors
are computed on each output grid with a volume of ones (fMRIPrep's run's transforms without
head motion, its ``ReconstructFieldmap`` output on that grid, its phase encoding and readout):
fMRIPrep's (``upstream_defects="reproduce"``) and larmorx's (``"fix"``). The reference is the
factor on the BOLD's own grid, where both methods agree (fMRIPrep's derivative is then taken
along the phase-encoding axis, in BOLD voxels), carried to each output grid through the same
transforms by cubic B-spline interpolation (as fMRIPrep resamples the BOLD): the factor at the
source point each output voxel maps to, without any choice of axis. Inside fMRIPrep's BOLD
brain mask on each grid.

Used by :mod:`larmorx_validation.parity.resample_jacobian` (``--oracle``), which writes the
results into ``docs/validation/resample-series-jacobian.md``.
"""

from __future__ import annotations

import os
import warnings
from pathlib import Path
from typing import Any

import numpy as np

#: The stage comparison's resampling nodes (``larmorx_validation.stages.catalogue``).
NATIVE = "bold_native_wf/boldref_bold"
#: Interpolation of the native factor onto the output grids: cubic B-splines, as fMRIPrep
#: resamples the BOLD (trilinear doubles the reference's own error where the field is steep).
CARRY_ORDER = 3
TARGETS = {"bold_anat_wf/resample": "T1w", "bold_std_wf/resample": "MNI152NLin2009cAsym res-2"}


def _value(text: str | None) -> Any:
    from larmorx_validation.stages.nodes import _value as value

    return value(text)


def _resample(
    run, inputs: dict, source, transforms, inverse, fieldmap, defects: str, order: int = 1
) -> Any:
    import larmorx as lx

    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return lx.transforms.resample_series(
            source,
            run.host(inputs["ref_file"]),
            transforms,
            inverse=inverse,
            fieldmap=fieldmap,
            pe_dir=inputs.get("pe_dir") if fieldmap is not None else None,
            ro_time=inputs.get("ro_time") if fieldmap is not None else None,
            jacobian=True,
            order=order,
            mode="nearest",
            n_threads=0,
            upstream_defects=defects,
        )


def _without_hmc(run, inputs: dict) -> tuple[list[str], Any]:
    transforms = [run.host(t) for t in (inputs.get("transforms") or [])]
    keep = [k for k, t in enumerate(transforms) if "desc-hmc_xfm" not in os.path.basename(t)]
    inverse = inputs.get("inverse") or False
    if isinstance(inverse, list | tuple) and len(inverse) == len(transforms):
        inverse = [inverse[k] for k in keep] or False
    return [transforms[k] for k in keep], inverse


def _ones(run, inputs: dict):
    import nibabel as nib

    src = nib.load(run.host(inputs["in_file"]))
    return np.ones(src.shape[:3], np.float32), src.affine


def measure_oracle(root: Path) -> list[dict[str, Any]]:
    """One row per output grid of every BOLD run resampled with a field map."""
    from larmorx_validation.stages.catalogue import normalise
    from larmorx_validation.stages.oracle import OracleRun
    from larmorx_validation.stages.runners import Settings

    run = OracleRun(root)
    s = Settings(run=run, compare=root)
    from larmorx_validation.stages.nodes import _brain_mask

    natives: dict[str, Any] = {}
    targets: list[tuple[str, str, Any]] = []
    for node in run.all_nodes:
        if node.interface != "resampling":
            continue
        norm, keys = normalise(node.path)
        inputs = {k: _value(v) for k, v in node.inputs.items()}
        if not inputs.get("fieldmap"):
            continue
        if norm.endswith(NATIVE):
            natives[keys.get("run", "")] = inputs
        for suffix, name in TARGETS.items():
            if norm.endswith(suffix):
                targets.append((keys.get("run", ""), name, inputs))
    rows = []
    for key, native in sorted(natives.items()):
        # the factor on the BOLD's own grid (both methods agree there)
        xfms, inverse = _without_hmc(run, native)
        fmap = run.host(native["fieldmap"])
        j_native = {
            d: _resample(run, native, _ones(run, native), xfms, inverse, fmap, d)
            for d in ("reproduce", "fix")
        }
        same = np.array_equal(np.asarray(j_native["reproduce"].data), j_native["fix"].data)
        nat = j_native["reproduce"]
        mask, mask_file = _brain_mask(s, nat.shape, nat.affine)
        inside = mask if mask is not None else np.ones(nat.shape, bool)
        jn = np.asarray(nat.data)[inside]
        rows.append(
            {
                "run": key,
                "grid": "native (BOLD reference)",
                "shape": "×".join(map(str, nat.shape[:3])),
                "jacobian_in_run": bool(native.get("jacobian")),
                "pe_dir": native.get("pe_dir"),
                "ro_time": native.get("ro_time"),
                "mask": mask_file,
                "voxels": int(inside.sum()),
                "fmriprep_range": [float(jn.min()), float(jn.max())],
                "larmorx_range": [float(jn.min()), float(jn.max())],
                "native_identical": bool(same),
            }
        )
        for tkey, name, inputs in targets:
            if tkey != key:
                continue
            txfms, tinv = _without_hmc(run, inputs)
            tfmap = run.host(inputs["fieldmap"])
            ones = _ones(run, inputs)
            j = {
                d: _resample(run, inputs, ones, txfms, tinv, tfmap, d) for d in ("reproduce", "fix")
            }
            # the native factor at the source point of every output voxel
            carried = _resample(
                run,
                inputs,
                (np.asarray(nat.data, np.float32), nat.affine),
                txfms,
                tinv,
                None,
                "fix",
                order=CARRY_ORDER,
            )
            img = j["fix"]
            mask, mask_file = _brain_mask(s, img.shape, img.affine)
            inside = mask if mask is not None else np.ones(img.shape, bool)
            ref = np.asarray(carried.data, np.float64)[inside]
            jr = np.asarray(j["reproduce"].data, np.float64)[inside]
            jf = np.asarray(j["fix"].data, np.float64)[inside]
            er, ef = np.abs(jr - ref), np.abs(jf - ref)
            zooms = np.sqrt((img.affine[:3, :3] ** 2).sum(axis=0))
            rows.append(
                {
                    "run": key,
                    "grid": name,
                    "shape": "×".join(map(str, img.shape[:3])),
                    "zooms": "×".join(f"{z:.3g}" for z in zooms),
                    "jacobian_in_run": bool(inputs.get("jacobian")),
                    "mask": mask_file,
                    "voxels": int(inside.sum()),
                    "fmriprep_range": [float(jr.min()), float(jr.max())],
                    "larmorx_range": [float(jf.min()), float(jf.max())],
                    "reference_range": [float(ref.min()), float(ref.max())],
                    "gap_over": {
                        f"{t:g}": float(np.mean(np.abs(jf - jr) > t)) for t in (0.01, 0.05)
                    },
                    "err_fmriprep": _errors(er),
                    "err_larmorx": _errors(ef),
                    "fmriprep_step": _fmriprep_step(run, inputs, txfms, tinv, img),
                }
            )
    return rows


def _fmriprep_step(run, inputs: dict, transforms: list[str], inverse, img) -> dict[str, float]:
    """What fMRIPrep's derivative steps over, at the centre of the output grid: one voxel along
    the output axis that has the BOLD's phase-encoding index, mapped into the BOLD's voxels
    (after sdcflows' ``ensure_positive_cosines``): its length in BOLD voxels and its angle to
    the phase-encoding axis. The right step is one BOLD voxel along that axis (length 1,
    angle 0); fMRIPrep's factor departs from the Jacobian as the two differ."""
    import nibabel as nib

    import larmorx as lx
    from larmorx.transforms.resample import ensure_positive_cosines

    src = nib.load(run.host(inputs["in_file"]))
    _, src_affine, _ = ensure_positive_cosines(np.zeros(src.shape[:3], np.float32), src.affine)
    pe = "ijk".index(str(inputs["pe_dir"])[0])
    centre = (np.array(img.shape[:3], float) - 1) / 2
    vox = np.array([centre, centre + np.eye(3)[pe]])
    world = vox @ img.affine[:3, :3].T + img.affine[:3, 3]
    if transforms:
        world = lx.transforms.load_transforms(transforms, inverse).map(world, n_threads=1)
    src_vox = np.c_[world, np.ones(2)] @ np.linalg.inv(src_affine).T
    d = src_vox[1, :3] - src_vox[0, :3]
    length = float(np.linalg.norm(d))
    return {
        "length": length,
        "angle_deg": float(np.degrees(np.arccos(min(1.0, abs(d[pe]) / length)))),
    }


def _errors(e: np.ndarray) -> dict[str, float]:
    return {
        "median": float(np.median(e)),
        "p99": float(np.percentile(e, 99)),
        "over_0.01": float(np.mean(e > 0.01)),
        "over_0.05": float(np.mean(e > 0.05)),
    }


def _pct(x: float) -> str:
    v = 100 * x
    return "0 %" if v == 0 else (f"{v:.2g} %" if v < 0.1 else f"{v:.1f} %")


def report_oracle(runs: dict[str, list[dict[str, Any]]]) -> list[str]:
    """The Markdown section for :func:`measure_oracle`'s rows, by oracle run."""
    lines = [
        "## On fMRIPrep's own field maps",
        "",
        "The oracle runs with field maps that reach `ResampleSeries` ([stage records]"
        "(README.md)): the factors on each output grid of every BOLD run, computed from "
        "fMRIPrep's own field map on that grid (`ReconstructFieldmap`'s output), its "
        "transforms without head motion (both factors are computed before it), its phase "
        "encoding and readout. The reference is the factor on the BOLD's own grid, where the "
        "two methods agree bit for bit (checked), carried to each output grid through the same "
        "transforms by cubic B-spline interpolation (as fMRIPrep resamples the BOLD): the "
        "factor at the source point each output voxel maps to, with no choice of axis. It is "
        "not exact: the native grid's central differences (over the BOLD's voxels) and the "
        "interpolation smooth it where the field is steep, so part of either factor's residual "
        "against it there is the reference's own error. Brain voxels: fMRIPrep's "
        "BOLD brain mask on each grid. fMRIPrep's `space-T1w` grid is the T1w's orientation at "
        "the BOLD's voxel size (`gen_ref`, `keep_native`), not the 1 mm grid of the synthetic "
        "study above. fMRIPrep's step: what its derivative steps over, at the centre of the "
        "output grid (one voxel along the output axis that has the BOLD's phase-encoding "
        "index, mapped into the BOLD's voxels); the Jacobian needs one BOLD voxel along the "
        "phase-encoding axis (1 / 0°). A shorter step shrinks the correction in proportion; "
        "an angle takes the field's slope along another direction.",
        "",
        "| Oracle run | BOLD run | Output grid | Shape | Voxel size (mm) | fMRIPrep's step "
        "(BOLD voxels / angle to the PE axis) | Jacobian in fMRIPrep's run | Brain voxels | "
        "fMRIPrep's factor | larmorx's factor | Reference | Factors differ by > 0.01 | "
        "Error, fMRIPrep (median / 99th pct / > 0.01) | "
        "Error, larmorx (median / 99th pct / > 0.01) |",
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|",
    ]
    for oracle, rows in runs.items():
        for r in rows:
            if r["grid"].startswith("native"):
                lines.append(
                    f"| {oracle} | {r['run']} | {r['grid']} | {r['shape']} | – | 1 / 0° | "
                    f"{'on' if r['jacobian_in_run'] else 'off'} | {r['voxels']:,} | "
                    f"{r['fmriprep_range'][0]:.3f} to {r['fmriprep_range'][1]:.3f} | "
                    f"{'the same, bit for bit' if r['native_identical'] else '**differs**'} | "
                    "– | – | – | – |"
                )
                continue
            ef, el = r["err_fmriprep"], r["err_larmorx"]
            step = r["fmriprep_step"]
            lines.append(
                f"| {oracle} | {r['run']} | {r['grid']} | {r['shape']} | {r['zooms']} | "
                f"{step['length']:.2f} / {step['angle_deg']:.0f}° | "
                f"{'on' if r['jacobian_in_run'] else 'off'} | {r['voxels']:,} | "
                f"{r['fmriprep_range'][0]:.3f} to {r['fmriprep_range'][1]:.3f} | "
                f"{r['larmorx_range'][0]:.3f} to {r['larmorx_range'][1]:.3f} | "
                f"{r['reference_range'][0]:.3f} to {r['reference_range'][1]:.3f} | "
                f"{_pct(r['gap_over']['0.01'])} | "
                f"{ef['median']:.4f} / {ef['p99']:.4f} / {_pct(ef['over_0.01'])} | "
                f"{el['median']:.4f} / {el['p99']:.4f} / {_pct(el['over_0.01'])} |"
            )
    lines.append("")
    return lines


def stage_changes(records: Path, oracle: str) -> list[dict[str, Any]]:
    """PIPE-03's fix pass on the real series, from the stage record of ``oracle``
    (``fmriprep-stages*.json`` in ``records``, written by ``pipeline-stages``): per resampling
    node with a field map, the change of larmorx's default against fMRIPrep's output."""
    import json

    for path in sorted(records.glob("fmriprep-stages*.json")):
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except ValueError:
            continue
        if payload.get("oracle", {}).get("run") != oracle:
            continue
        rows = []
        for r in payload.get("results", []):
            jf = r.get("jacobian_fix")
            if not jf:
                continue
            p = jf.get("forced") or jf
            rows.append(
                {
                    "record": path.stem,
                    "stage": r["stage"],
                    "node": r["name"],
                    "keys": r.get("keys", {}),
                    "jacobian_in_run": jf["jacobian"],
                    "shape": p["fix_vs_reproduce"]["shape"],
                    "identical": p["fix_vs_reproduce"]["differing"] == 0,
                    "change": p.get("change"),
                }
            )
        return rows
    return []


def report_changes(changes: dict[str, list[dict[str, Any]]]) -> list[str]:
    """What larmorx's default does to fMRIPrep's preprocessed BOLD (the stage records)."""
    rows = []
    for oracle, items in changes.items():
        for c in items:
            keys = c["keys"]
            space = keys.get("space", "")
            label = f"{c['stage']} `{c['node']}`" + (f" {space}" if space else "")
            on = "on" if c["jacobian_in_run"] else "off (forced on)"
            shape = "×".join(map(str, c["shape"]))
            ch = c["change"]
            if c["identical"] or not ch:
                rows.append(f"| {oracle} | {label} | {on} | {shape} | unchanged | | | | | |")
                continue
            pc = ch["abs_percentiles"]
            at = ", ".join(f"{x:g}" for x in ch["max_at_mm"])
            rows.append(
                f"| {oracle} | {label} | {on} | {shape} | {ch['brain_voxels']:,} | "
                f"{_pct(ch['over']['0.001'])} | {_pct(ch['over']['0.01'])} | "
                f"{_pct(ch['over']['0.05'])} | {_pct(pc['50'])} / {_pct(pc['90'])} / "
                f"{_pct(pc['99'])} | ({at}) |"
            )
    if not rows:
        return []
    return [
        "What larmorx's default does to fMRIPrep's preprocessed BOLD of these runs, per brain "
        "voxel: the relative change of the voxel's temporal mean against fMRIPrep's output "
        "(which larmorx's `reproduce` mode gives bit for bit), from the stage records "
        "(`fmriprep-stages-<dataset>.md`, sections F05 to F07 and S02, where the "
        "distributions, signed ranges and clusters are). Where fMRIPrep ran without the "
        "Jacobian, both modes with it forced on:",
        "",
        "| Oracle run | Node | Jacobian in fMRIPrep's run | Shape | Brain voxels | "
        "Changed > 0.1 % | > 1 % | > 5 % | Median / 90th / 99th pct | Largest at (mm) |",
        "|---|---|---|---|---|---|---|---|---|---|",
        *rows,
        "",
    ]


__all__ = ["measure_oracle", "report_changes", "report_oracle", "stage_changes"]
