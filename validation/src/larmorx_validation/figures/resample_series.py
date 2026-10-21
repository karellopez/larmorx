# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's one-shot BOLD resampler (``lx.transforms.resample_series``): the record's
agreement, PIPE-03's Jacobian on fMRIPrep's own field map (fMRIPrep's factor and larmorx's
corrected one against a reference), and speed."""

from __future__ import annotations

from functools import cache
from pathlib import Path

import numpy as np

from larmorx_validation.figures import brain, common, records, style
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures.core import Context, Figure

TOOL = "resample-series"
RUN = "ds005040-sub-002-noreconall"
TARGET = "bold_std_wf/resample"  # the MNI152NLin2009cAsym res-2 grid
SPEED_LABELS = {
    "ds000005 run 1 → boldref (native): 64×64×34, 240 volumes; motion + field map": (
        "ds000005 → native (240 vol.)"
    ),
    "ds000005 run 1 → T1w grid: motion + boldref→T1w affine + field map": "ds000005 → T1w grid",
    "ds000005 run 1 → MNI152NLin2009cAsym res-2 (97×115×97): motion + affine + .h5 warp": (
        "ds000005 → MNI (.h5 warp)"
    ),
    "ds006010 sub-206 → native: 96×96×69, 113 volumes (uint16); motion + field map": (
        "ds006010 → native (113 vol.)"
    ),
    "ds005454 sub-16 rest → native: 160×160×96, 280 volumes (multiband 4); motion + field map": (
        "ds005454 → native (280 vol.)"
    ),
}


def _compute(e: fcache.Entry) -> None:
    import nibabel as nib

    from larmorx_validation.parity.resample_jacobian_oracle import (
        CARRY_ORDER,
        NATIVE,
        _ones,
        _resample,
        _value,
        _without_hmc,
    )
    from larmorx_validation.stages.catalogue import normalise
    from larmorx_validation.stages.nodes import _brain_mask
    from larmorx_validation.stages.oracle import OracleRun
    from larmorx_validation.stages.runners import Settings

    root = common.fmriprep_oracle() / RUN
    if not (root / "work").is_dir():
        raise FileNotFoundError(f"the fMRIPrep oracle run {RUN} is missing")
    run = OracleRun(root)
    s = Settings(run=run, compare=root)
    native = target = None
    for node in run.all_nodes:
        if node.interface != "resampling":
            continue
        norm, _ = normalise(node.path)
        inputs = {k: _value(v) for k, v in node.inputs.items()}
        if not inputs.get("fieldmap"):
            continue
        if norm.endswith(NATIVE) and native is None:
            native = inputs
        if norm.endswith(TARGET) and target is None:
            target = inputs
    if native is None or target is None:
        raise FileNotFoundError(f"{RUN}: no resampling with a field map")
    xfms, inverse = _without_hmc(run, native)
    nat = _resample(
        run, native, _ones(run, native), xfms, inverse, run.host(native["fieldmap"]), "reproduce"
    )
    txfms, tinv = _without_hmc(run, target)
    ones = _ones(run, target)
    tfmap = run.host(target["fieldmap"])
    out = {d: _resample(run, target, ones, txfms, tinv, tfmap, d) for d in ("reproduce", "fix")}
    out["reference"] = _resample(
        run,
        target,
        (np.asarray(nat.data, np.float32), nat.affine),
        txfms,
        tinv,
        None,
        "fix",
        order=CARRY_ORDER,
    )
    img = out["fix"]
    mask, mask_file = _brain_mask(s, img.shape, img.affine)
    for k, v in out.items():
        nib.save(nib.Nifti1Image(np.asarray(v.data, np.float32), v.affine), e.dir / f"{k}.nii.gz")
    nib.save(nib.Nifti1Image(np.asarray(mask, np.uint8), img.affine), e.dir / "mask.nii.gz")
    e.finish(
        settings="the Jacobian factors on the MNI152NLin2009cAsym res-2 grid of fMRIPrep 25.2.6's "
        f"run {RUN} (task clip, run 1), computed in this process as "
        "larmorx_validation.parity.resample_jacobian_oracle computes them: a volume of ones "
        "resampled with fMRIPrep's transforms (no head motion), its field map on that grid, its "
        "phase encoding and readout; upstream_defects='reproduce' gives fMRIPrep's factor, "
        "'fix' larmorx's; the reference is the factor on the BOLD's own grid (where both "
        "agree) carried to the MNI grid by cubic B-splines",
        mask=str(mask_file),
        pe_dir=str(target.get("pe_dir")),
        ro_time=str(target.get("ro_time")),
    )


@cache
def _maps(cache_dir: str) -> dict:
    d = Path(cache_dir)
    out = {k: brain.load(d / f"{k}.nii.gz") for k in ("reproduce", "fix", "reference")}
    out["mask"] = brain.load(d / "mask.nii.gz").data > 0
    return out


def jacobian_figure(ctx: Context) -> Figure:
    state: dict = {}

    def prepare(fig: Figure) -> None:
        e = fcache.entry(
            ctx,
            TOOL,
            "ds005040-sub-002-jacobian-mni",
            "PIPE-03: the Jacobian factors of ds005040 sub-002's BOLD on the MNI grid",
            _compute,
        )
        state["m"] = _maps(str(e.dir))
        fig.cache = [fcache.provenance(e)]

    def draw(theme: style.Theme, width: int):
        m = state["m"]
        mask = m["mask"]
        plane, _ = brain.centre_planes(mask, axial_shift=-0.18)
        sp = plane.spacing(m["fix"].zooms)
        m2 = plane.take(mask)
        crop = brain.bbox(m2, 1)
        seq = style.diverging(theme)
        lo, hi = 0.5, 1.5
        e_f = m["reproduce"].data - m["reference"].data
        e_l = m["fix"].data - m["reference"].data
        lim = float(np.percentile(np.abs(np.concatenate([e_f[mask], e_l[mask]])), 99.5))

        def factor(title, key, sub):
            return brain.Panel(
                title,
                plane.take(m[key].data),
                m2,
                seq,
                lo,
                hi,
                sp,
                crop=crop,
                outline=m2,
                subtitle=sub,
            )

        def err(title, v):
            a = np.abs(v[mask])
            return brain.Panel(
                title,
                plane.take(v),
                m2,
                style.diverging(theme),
                -lim,
                lim,
                sp,
                crop=crop,
                outline=m2,
                subtitle=f"median |error| {np.median(a):.4f} · 99th pct {np.percentile(a, 99):.3f}",
            )

        panels = [
            factor("fMRIPrep's factor", "reproduce", "upstream_defects='reproduce'"),
            factor("larmorx's factor", "fix", "default: PIPE-03 corrected"),
            factor("Reference", "reference", "the BOLD grid's factor, carried"),
            err("fMRIPrep − reference", e_f),
            err("larmorx − reference", e_l),
        ]
        bars = {
            "f": brain.Bar(
                seq,
                lo,
                hi,
                "Jacobian factor (1: no intensity change)",
                ticks=[lo, 1.0, hi],
                fmt="{:.1f}",
            ),
            "e": brain.Bar(
                style.diverging(theme),
                -lim,
                lim,
                "error against the reference (99.5th percentile range)",
                fmt="{:.2f}",
            ),
        }
        return brain.draw_grid(theme, width, brain.Grid(panels, cols=3, cols_narrow=2, bars=bars))

    return Figure(
        "jacobian",
        "PIPE-03: the Jacobian on fMRIPrep's own field map, on the MNI grid",
        "Axial slices on the MNI152NLin2009cAsym 2 mm grid of OpenNeuro ds005040 sub-002's "
        "distortion-correction Jacobian: fMRIPrep's factor, larmorx's corrected factor and the "
        "reference (the factor on the BOLD's own grid carried to this grid), then each factor's "
        "error against the reference on a symmetric scale.",
        draw,
        data=[
            {
                "dataset": "OpenNeuro ds005040",
                "subject": "sub-002",
                "licence": "CC0-1.0",
                "files": [f"fMRIPrep 25.2.6's field map, transforms and masks ({RUN})"],
            },
            {
                "dataset": "MNI152NLin2009cAsym (TemplateFlow)",
                "licence": "MNI ICBM 152 notice",
                "notice": common.MNI_NOTICE,
            },
        ],
        prepare=prepare,
    )


def figures(ctx: Context) -> list[Figure]:
    rec = records.load(ctx.repo, TOOL)
    figs = [
        common.agreement_figure(
            rec,
            title="Agreement with fMRIPrep's resampler, nitransforms and SciPy, by category",
            alt=f"Stacked bars, one per category of the resampler record ({len(rec.results)} "
            "cases): every compared case bit-identical to the reference, one expected "
            "divergence and two rejected by both.",
            split=lambda cat, passing: {"identical": len(passing)},
            state_labels={
                "identical": "bit-identical to the reference (fMRIPrep, nitransforms, SciPy)"
            },
        ),
        jacobian_figure(ctx),
    ]
    bench = records.load_bench(ctx.repo, TOOL)
    if bench is not None:
        series = (
            {
                ("fMRIPrep", "1 threads"): "fMRIPrep, 1 thread",
                ("fMRIPrep", "12 threads"): "fMRIPrep, 12 threads",
                ("larmorx", "1 threads"): "larmorx, 1 thread",
                ("larmorx", "12 threads"): "larmorx, 12 threads",
            },
            [
                ("fMRIPrep, 1 thread", "original", "o"),
                ("fMRIPrep, 12 threads", "original", "o-open"),
                ("larmorx, 1 thread", "larmorx", "o"),
                ("larmorx, 12 threads", "larmorx", "o-open"),
            ],
            ("fMRIPrep, 1 thread", "larmorx, 1 thread"),
        )
        figs.append(
            common.speed_figure(
                bench,
                series=series,
                labels=SPEED_LABELS,
                title="Speed against fMRIPrep's resampler",
                alt="Median seconds per BOLD run for fMRIPrep's resampler and larmorx at 1 and 12 "
                "threads, on a log scale, with larmorx's speed-up at one thread.",
            )
        )
    return figs
