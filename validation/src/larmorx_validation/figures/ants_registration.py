# SPDX-License-Identifier: Apache-2.0
"""antsRegistration: the record's agreement, ANTs' own thread band, and sMRIPrep's template
registration (Rigid, Affine, SyN) of a real T1w on every third voxel, by ANTs and larmorx."""

from __future__ import annotations

import re
from functools import cache
from pathlib import Path

import numpy as np

from larmorx_validation.figures import brain, common, records, style
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures.core import Context, Figure

TOOL = "ants-registration"
RUN = "ds000005-sub-01-noreconall"
NODE = "anat_fit_wf/register_template_wf/_template_MNI152NLin2009cAsym/registration"
SPEC = f"{RUN}/sub_01_wf/{NODE}"
LABELS = {
    "real": "real (fMRIPrep's)",
    "real-linear": "real, linear stages",
    "real-syn-small": "real, SyN (small)",
    "real-bsplinesyn-small": "real, B-spline SyN (small)",
}


def headline_split(rec: records.Record) -> dict[str, int]:
    m = re.search(
        r"(\d+) cases compared: (\d+) identical to the unmodified ANTs.*?, (\d+) identical once "
        "ANTs uses CORE-MATH",
        rec.text,
    )
    if not m:
        raise LookupError(f"{rec.path_md.name}: the identical / CORE-MATH sentence is missing")
    return {"identical": int(m.group(2)), "identical-cm": int(m.group(3))}


def thread_band(rec: records.Record) -> list[tuple[str, float]]:
    """The thread band per case, from the record's sentence: ANTs at 2, 4, 8 threads against its
    one-thread result, the largest change in a transform parameter."""
    m = re.search(r"^Thread band, \d+ cases: .*$", rec.text, re.M)
    if not m:
        raise LookupError(f"{rec.path_md.name}: no thread band")
    return [(c, float(v)) for c, v in re.findall(r"`([^`]+)` ([0-9.e+-]+)", m.group(0))]


def agreement(rec: records.Record, program: str) -> Figure:
    split = headline_split(rec)
    return common.agreement_figure(
        rec,
        title=f"Agreement with {program}, by category",
        alt=f"Stacked bars, one per category of the {program} record ({len(rec.results)} "
        f"cases): {split['identical']} compared cases identical to ANTs as built and "
        f"{split['identical-cm']} identical once ANTs uses correctly rounded maths (the first "
        "bar), rejected by both, and expected divergences.",
        original="ANTs",
        headline_split=split,
        labels=LABELS,
        state_labels={"agree": "identical, as built or with CORE-MATH (split in the first bar)"},
    )


def band(rec: records.Record, program: str) -> Figure:
    pts = thread_band(rec)

    def draw(theme: style.Theme, width: int):
        return _band_chart(theme, width, pts, program)

    return Figure(
        "thread-band",
        f"{program}'s own variability: its result at 2, 4 and 8 threads",
        f"For each of the {len(pts)} cases measured, the largest change in a transform parameter "
        "between ANTs at one thread and ANTs at 2, 4 and 8 threads, on a log scale; cases where "
        "the threads change nothing are counted at the left. larmorx gives the one-thread "
        "result at any thread count.",
        draw,
        records=rec.source(),
    )


def _band_chart(theme: style.Theme, width: int, pts: list[tuple[str, float]], program: str):
    from larmorx_validation.figures.canvas import Canvas

    narrow = width < 500
    zero = [c for c, v in pts if v == 0]
    vals = sorted([(v, c) for c, v in pts if v > 0])
    row_h = 15
    top = 30
    h = top + row_h * len(vals) + 56
    c = Canvas(theme, width, h)
    label_w = 0 if narrow else 300
    ax = c.axes(label_w, top, width - label_w - 8, row_h * len(vals))
    ys = np.arange(len(vals))
    xs = [v for v, _ in vals]
    ax.hlines(ys, 1e-23, xs, color=theme.line, lw=0.9)
    ax.scatter(xs, ys, s=26, color=theme.original, edgecolors=theme.paper, linewidths=1, zorder=3)
    ax.set_xscale("log")
    ax.set_xlim(1e-23, 30)
    ax.set_ylim(len(vals) - 0.5, -0.5)
    for i, (_v, case) in enumerate(vals):
        if narrow:
            ax.text(
                1.5e-23,
                i - 0.05,
                case,
                fontfamily=style.MONO,
                fontsize=style.pt(9),
                color=theme.ink2,
                va="center",
            )
        else:
            c.text(
                label_w - 10,
                top + row_h * (i + 0.5),
                case,
                family=style.MONO,
                size=10,
                color=theme.ink2,
                ha="right",
                va="center_baseline",
            )
    style.style_axes(ax, theme, grid="x")
    ax.spines["left"].set_visible(False)
    ax.set_yticks([])
    ax.set_xticks([1e-20, 1e-15, 1e-10, 1e-5, 1])
    for t in ax.get_xticklabels():
        t.set_fontfamily(style.MONO)
    ax.set_xlabel(
        f"largest change in a transform parameter, {program} at 2, 4, 8 threads vs 1 (log)",
        fontsize=style.pt(11.5),
    )
    c.text(
        0,
        14,
        f"{len(zero)} cases unchanged by the thread count: " + ", ".join(zero)
        if not narrow
        else f"{len(zero)} more cases unchanged by the thread count",
        family=style.MONO,
        size=10,
        color=theme.ink3,
    )
    return c.fig


# ------------------------------------------------------------------- the image figure


def _compute(e: fcache.Entry) -> None:
    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload
    from larmorx_validation.parity.ants_registration import SEED, SMALL, _fmriprep_args

    b, lib = ants_bin(), crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload is missing")
    tmp = e.dir / "inputs"
    tmp.mkdir(exist_ok=True)
    for label, threads, pre in (
        ("ants-1", 1, False),
        ("ants-4", 4, False),
        ("ants-1-crmath", 1, True),
        ("larmorx", 4, False),
    ):
        out = e.dir / label
        out.mkdir(exist_ok=True)
        args = _fmriprep_args(SPEC, out, tmp=tmp, stand_ins=True, small=SMALL)
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads), "ANTS_RANDOM_SEED": SEED}
        if pre:
            env["LD_PRELOAD"] = str(lib)
        if label == "larmorx":
            env["LARMORX_UPSTREAM_DEFECTS"] = "reproduce"
            argv = fcache.larmorx_cli("ants", "antsRegistration", *args)
        else:
            argv = [str(b / "antsRegistration"), *args]
        e.run(label, argv, env=env, unset=("LD_PRELOAD",))
    e.finish(
        versions={"ants": ants_version()},
        threads={"ants-1": 1, "ants-4": 4, "ants-1-crmath": 1, "larmorx": 4},
        seed=f"ANTS_RANDOM_SEED={SEED}",
        settings=f"fMRIPrep 25.2.6's template registration of {RUN} (its command.txt), its "
        f"images on every {SMALL}rd voxel, as the record's real-syn-small case; larmorx with "
        "upstream_defects=reproduce",
    )


def _field(path: Path) -> tuple[np.ndarray, np.ndarray]:
    """The displacement field of a composite .h5 (the last component), as (z, y, x, 3) in mm,
    and its fixed parameters."""
    import larmorx as lx

    parts = lx.transforms.read(path, upstream_defects="reproduce")
    disp = [t for t in parts if t.is_displacement_field]
    if not disp:
        raise ValueError(f"{path}: no displacement field")
    t = disp[-1]
    size = [round(v) for v in t.fixed_parameters[:3]]
    return np.asarray(t.parameters, dtype=np.float64).reshape(size[2], size[1], size[0], 3), (
        np.asarray(t.fixed_parameters)
    )


@cache
def _images(cache_dir: str) -> dict:
    d = Path(cache_dir)
    out: dict = {}
    for k in ("ants-1", "ants-4", "ants-1-crmath", "larmorx"):
        out[k] = brain.load(d / k / "ants_t1_to_mni_Warped.nii.gz")
        out[k + "-field"], fp = _field(d / k / "transformComposite.h5")
    fixed = sorted((d / "inputs" / "small").glob("*fixed_masked.nii.gz"))
    out["fixed"] = brain.load(fixed[0])
    out["mask"] = out["fixed"].data > 0
    out["field-params"] = fp
    return out


def registration_figure(ctx: Context) -> Figure:
    state: dict = {}

    def prepare(fig: Figure) -> None:
        e = fcache.entry(
            ctx,
            TOOL,
            "ds000005-sub-01-template-small",
            "sMRIPrep's T1w-to-MNI registration of ds000005 sub-01 on every third voxel",
            _compute,
        )
        state["img"] = _images(str(e.dir))
        fig.cache = [fcache.provenance(e)]

    def draw(theme: style.Theme, width: int):
        im = state["img"]
        mask = im["mask"]
        plane, _ = brain.centre_planes(mask, axial_shift=0.05)
        sp = plane.spacing(im["fixed"].zooms)
        m2 = plane.take(mask)
        crop = brain.bbox(m2, 1)
        fixed = plane.take(im["fixed"].data)
        warped = plane.take(im["larmorx"].data)
        hi = float(np.percentile(fixed[m2], 99.5))
        hw = float(np.percentile(warped[m2], 99.5))
        # displacement differences: magnitudes in mm, on the field's own grid (z, y, x)
        f1 = im["ants-1-field"]

        def mag(a, b_):
            return np.sqrt(((a - b_) ** 2).sum(-1))

        # the field's grid is the fixed image's (ITK order z, y, x): back to x, y, z
        d_lx = mag(im["larmorx-field"], f1).transpose(2, 1, 0)
        d_thr = mag(im["ants-4-field"], f1).transpose(2, 1, 0)
        d_cm = mag(im["larmorx-field"], im["ants-1-crmath-field"]).transpose(2, 1, 0)
        # LPS (ITK) to RAS: the fixed image was read RAS; flip x and y if the field grid is LPS
        d_lx, d_thr, d_cm = (_to_ras(v, im["field-params"]) for v in (d_lx, d_thr, d_cm))
        lim = float(max(d_lx[mask].max(), d_thr[mask].max(), 1e-6))
        seq = style.sequential(theme)
        same = not np.any(d_cm[mask])
        n = 6
        cb = _checker(fixed / hi, warped / hw, n)
        panels = [
            brain.Panel(
                "Template (fixed)",
                fixed,
                m2,
                "gray",
                0,
                hi,
                sp,
                crop=crop,
                contour=warped,
                contour_level=0.55 * hw,
                subtitle="larmorx's warped T1w: contour",
            ),
            brain.Panel(
                "Checkerboard",
                cb,
                m2,
                "gray",
                0,
                1,
                sp,
                crop=crop,
                subtitle="template / warped T1w",
            ),
            brain.Panel("Warped T1w, larmorx", warped, m2, "gray", 0, hw, sp, crop=crop),
            brain.Panel(
                "larmorx − ANTs as built",
                plane.take(d_lx),
                m2,
                seq,
                0,
                lim,
                sp,
                crop=crop,
                outline=m2,
                subtitle=_mm(d_lx, mask),
            ),
            brain.Panel(
                "ANTs: 4 − 1 thread",
                plane.take(d_thr),
                m2,
                seq,
                0,
                lim,
                sp,
                crop=crop,
                outline=m2,
                subtitle=_mm(d_thr, mask),
            ),
            brain.Panel(
                "larmorx − ANTs, CORE-MATH",
                plane.take(d_cm),
                m2,
                seq,
                0,
                lim,
                sp,
                crop=crop,
                outline=m2,
                note="bit-identical" if same else "",
                subtitle=_mm(d_cm, mask),
            ),
        ]
        bars = {
            "d": brain.Bar(
                seq,
                0.0,
                lim,
                "difference in the displacement field (magnitude, mm; full range in the brain)",
                fmt="{:.2g}",
            )
        }
        return brain.draw_grid(theme, width, brain.Grid(panels, cols=3, cols_narrow=2, bars=bars))

    return Figure(
        "template-registration",
        "sMRIPrep's T1w-to-MNI registration, by ANTs and larmorx",
        "Axial slices in MNI152NLin2009cAsym space at a third of its resolution: the template "
        "with the contour of larmorx's warped T1w, a checkerboard of the two, the warped T1w, "
        "and the magnitude of the difference between displacement fields: larmorx against ANTs "
        "as built, ANTs at four threads against one, and larmorx against ANTs with correctly "
        "rounded maths.",
        draw,
        data=[
            {
                **common.DS000005,
                "files": [f"fMRIPrep 25.2.6's desc-preproc_T1w and brain mask ({RUN})"],
            },
            {
                "dataset": "MNI152NLin2009cAsym (TemplateFlow)",
                "licence": "MNI ICBM 152 notice",
                "notice": common.MNI_NOTICE,
            },
        ],
        prepare=prepare,
    )


def _mm(v: np.ndarray, mask: np.ndarray) -> str:
    m = float(v[mask].max())
    return "identical" if m == 0 else f"largest {m:.2g} mm"


def _to_ras(v: np.ndarray, fixed_params: np.ndarray) -> np.ndarray:
    """A volume on the field's grid (ITK, x y z) in the fixed image's canonical RAS order."""
    direction = np.asarray(fixed_params[9:18]).reshape(3, 3)
    # ITK's direction is in LPS; its columns' signs along the diagonal say which axes to flip
    lps_to_ras = np.diag([-1.0, -1.0, 1.0])
    ras = lps_to_ras @ direction
    out = v
    for ax in range(3):
        col = ras[:, ax]
        k = int(np.argmax(np.abs(col)))
        if k != ax:
            raise ValueError("an oblique or permuted field grid")
        if col[k] < 0:
            out = np.flip(out, axis=ax)
    return out


def _checker(a: np.ndarray, b: np.ndarray, n: int) -> np.ndarray:
    h, w = a.shape
    yy, xx = np.indices((h, w))
    sel = ((yy * n // h) + (xx * n // w)) % 2 == 0
    return np.clip(np.where(sel, a, b), 0, 1)


def figures(ctx: Context) -> list[Figure]:
    rec = records.load(ctx.repo, TOOL)
    return [agreement(rec, "antsRegistration"), band(rec, "ANTs"), registration_figure(ctx)]
