# SPDX-License-Identifier: Apache-2.0
"""The Validation page's summary: every tool ported so far × its implementations × how many
of its record's cases have each verdict (``static/evidence/summary.json``).

The pilot tools (``TOOLS`` of this package) count their own evidence's verdicts, case by case.
The others count the classification their part-1 agreement figures draw from their records
(``larmorx_validation.figures``: identical to the unmodified original, identical with correctly
rounded maths, within the record's thresholds, an expected divergence, rejected by both, a
failure), mapped onto the verdicts; what a record calls *within its thresholds* is kept apart
as not yet classified until that tool's evidence page is redone.
"""

from __future__ import annotations

import importlib
import json

from larmorx_validation.evidence import TOOLS, core

#: Every tool page of the site, in the status table's order: page, title, ecosystem, and its
#: implementations (label, record, status, the part-1 figures module and agreement figure).
PAGES = [
    (
        "/tools/ants-registration/",
        "antsRegistration",
        "ants",
        [("port", "ants-registration", "validated", "ants_registration", "agreement")],
    ),
    (
        "/tools/ants-n4/",
        "N4BiasFieldCorrection",
        "ants",
        [("port", "ants-n4", "validated", None, None)],
    ),
    (
        "/tools/ants-denoise/",
        "DenoiseImage",
        "ants",
        [("port", "ants-denoise", "validated", "ants_denoise", "agreement")],
    ),
    (
        "/tools/ants-apply-transforms/",
        "antsApplyTransforms",
        "ants",
        [("port", "ants-apply-transforms", "validated", "ants_apply_transforms", "agreement")],
    ),
    (
        "/tools/ants-programs/",
        "ImageMath and the filters",
        "ants",
        [
            (
                "ImageMath (port)",
                "ants-image-math",
                "validated",
                "ants_programs",
                "overview#ImageMath",
            ),
            (
                "ThresholdImage (port)",
                "ants-threshold-image",
                "validated",
                "ants_programs",
                "overview#ThresholdImage",
            ),
            (
                "MultiplyImages (port)",
                "ants-multiply-images",
                "validated",
                "ants_programs",
                "overview#MultiplyImages",
            ),
            (
                "SmoothImage (port)",
                "ants-smooth-image",
                "validated",
                "ants_programs",
                "overview#SmoothImage",
            ),
            (
                "ResampleImage (port)",
                "ants-resample-image",
                "validated",
                "ants_programs",
                "overview#ResampleImage",
            ),
            (
                "ResampleImageBySpacing (port)",
                "ants-resample-image-by-spacing",
                "validated",
                "ants_programs",
                "overview#ResampleImageBySpacing",
            ),
        ],
    ),
    (
        "/tools/ants-ai/",
        "antsAI, antsAffineInitializer",
        "ants",
        [
            ("antsAI (port)", "ants-ai", "validated", "ants_ai", "agreement-ai"),
            (
                "antsAffineInitializer (port)",
                "ants-affine-initializer",
                "validated",
                "ants_ai",
                "agreement-initializer",
            ),
        ],
    ),
    (
        "/tools/itk-hdf5/",
        "ITK transform files",
        "ants",
        [("port", "itk-hdf5", "validated", "itk_hdf5", "agreement")],
    ),
    (
        "/tools/afni-tshift/",
        "3dTshift",
        "afni",
        [
            ("clean-room original", "afni-tshift", "validated", None, None),
            ("larmorx-gpl replica", "afni-tshift-replica", "validated", None, None),
        ],
    ),
    (
        "/tools/mri-hmc/",
        "Head-motion correction",
        "fsl",
        [("clean-room original", "mri-hmc", "experimental", "mri_hmc", "agreement")],
    ),
    (
        "/tools/resample-series/",
        "One-shot BOLD resampler",
        "larmorx",
        [("port", "resample-series", "validated", "resample_series", "agreement")],
    ),
    (
        "/tools/nifti-io/",
        "NIfTI I/O",
        "larmorx",
        [("own implementation", "nifti-io", "validated", "nifti_io", "agreement")],
    ),
]
STATE = {
    "identical": "bit-identical",
    "identical-cm": "identical-cr",
    "agree": "bit-identical",
    "expected": "divergence",
    "fail": "open",
    "both": "both-reject",
}


def _part1_counts(ctx: core.Context, module: str, figure: str) -> dict[str, int]:
    """The headline row a part-1 agreement figure draws, captured from its drawing call."""
    from larmorx_validation.figures import charts, style
    from larmorx_validation.figures.core import Context as FigContext

    mod = importlib.import_module(f"larmorx_validation.figures.{module}")
    fctx = FigContext(repo=ctx.repo, out=ctx.out, cache=ctx.cache, compute=False, log=ctx.log)
    figure, _, row = figure.partition("#")
    figs = {f.name: f for f in mod.figures(fctx)}
    if figure not in figs:
        raise LookupError(f"{module}: no figure {figure!r} (has {sorted(figs)})")
    captured: list = []
    original = charts.agreement

    def capture(theme, width, rows, **kw):
        captured.append(rows)
        raise _Captured

    charts.agreement = capture
    try:
        figs[figure].draw(style.LIGHT, style.WIDE)
    except _Captured:
        pass
    finally:
        charts.agreement = original
    rows = captured[0]
    if row:
        return dict(next(r for r in rows if r.label == row).counts)
    head = next((r for r in rows if r.label.lower().startswith("all")), None)
    if head is None:  # no headline row: the sum of the rows
        total: dict[str, int] = {}
        for r in rows:
            for k, v in r.counts.items():
                total[k] = total.get(k, 0) + v
        return total
    return dict(head.counts)


class _Captured(Exception):
    pass


def export(ctx: core.Context) -> dict:
    tools = []
    for page, title, eco, impls in PAGES:
        entries = []
        for label, record, status, module, figure in impls:
            if module is None:
                tool = page.strip("/").split("/")[-1]
                doc = json.loads((ctx.out / f"{tool}.json").read_text(encoding="utf-8"))
                im = next(
                    i
                    for i in doc["implementations"]
                    if record in (i.get("name"), None) or i["record"].endswith(f"/{record}.md")
                )
                counts = dict(im["summary"]["verdicts"])
                source = "evidence"
            else:
                states = _part1_counts(ctx, module, figure)
                counts, source = {}, "part 1"
                for k, v in states.items():
                    key = STATE.get(k, "unclassified")
                    counts[key] = counts.get(key, 0) + v
            entries.append(
                {
                    "label": label,
                    "record": f"docs/validation/{record}.md" if record else "docs/validation/",
                    "status": status,
                    "cases": sum(counts.values()),
                    "verdicts": {
                        k: counts[k] for k in [*core.VERDICTS, "unclassified"] if counts.get(k)
                    },
                    "source": source,
                }
            )
        tools.append({"page": page, "title": title, "eco": eco, "implementations": entries})
    return {
        "schema": 1,
        "tools": tools,
        "note": "N4BiasFieldCorrection and 3dTshift count their evidence pages' verdicts, case by "
        "case. The other tools count what their records establish today: identical to the "
        "unmodified original, identical with correctly rounded maths, expected divergences, "
        "rejected by both, failures; a case its record passes *within its thresholds* is "
        "counted as not yet classified until that tool's page is redone with this taxonomy.",
        "pilot": sorted(TOOLS),
    }
