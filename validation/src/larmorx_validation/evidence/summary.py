# SPDX-License-Identifier: Apache-2.0
"""The Validation page's summary: every tool ported so far × its implementations × how many
of its record's cases have each verdict (``static/evidence/summary.json``).

Every tool counts its own evidence's verdicts, case by case (``static/evidence/<tool>.json``:
each case's verdict decided from its record's reported and measured values). A tool whose page
still shows part-1 figures (none since part 2b) would count the classification its agreement
figure draws from its record (``larmorx_validation.figures``), mapped onto the verdicts, what a
record calls *within its thresholds* kept apart as not yet classified.
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
        [("port", "ants-registration", "validated", None, None)],
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
        [("port", "ants-denoise", "validated", None, None)],
    ),
    (
        "/tools/ants-apply-transforms/",
        "antsApplyTransforms",
        "ants",
        [("port", "ants-apply-transforms", "validated", None, None)],
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
                None,
                None,
            ),
            (
                "ThresholdImage (port)",
                "ants-threshold-image",
                "validated",
                None,
                None,
            ),
            (
                "MultiplyImages (port)",
                "ants-multiply-images",
                "validated",
                None,
                None,
            ),
            (
                "SmoothImage (port)",
                "ants-smooth-image",
                "validated",
                None,
                None,
            ),
            (
                "ResampleImage (port)",
                "ants-resample-image",
                "validated",
                None,
                None,
            ),
            (
                "ResampleImageBySpacing (port)",
                "ants-resample-image-by-spacing",
                "validated",
                None,
                None,
            ),
        ],
    ),
    (
        "/tools/ants-ai/",
        "antsAI, antsAffineInitializer",
        "ants",
        [
            ("antsAI (port)", "ants-ai", "validated", None, None),
            (
                "antsAffineInitializer (port)",
                "ants-affine-initializer",
                "validated",
                None,
                None,
            ),
        ],
    ),
    (
        "/tools/ants-atropos/",
        "Atropos",
        "ants",
        [("port", "ants-atropos", "validated", None, None)],
    ),
    (
        "/tools/itk-hdf5/",
        "ITK transform files",
        "ants",
        [("port", "itk-hdf5", "validated", None, None)],
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
        "/tools/afni-automask/",
        "3dAutomask",
        "afni",
        [
            ("clean-room original", "afni-automask", "validated", None, None),
            ("larmorx-gpl replica", "afni-automask-replica", "validated", None, None),
        ],
    ),
    (
        "/tools/afni-unifize/",
        "3dUnifize",
        "afni",
        [
            ("clean-room original", "afni-unifize", "validated", None, None),
            ("larmorx-gpl replica", "afni-unifize-replica", "validated", None, None),
        ],
    ),
    (
        "/tools/fsl-bet/",
        "BET",
        "fsl",
        [("larmorx-nc replica", "fsl-bet-replica-full", "validated", None, None)],
    ),
    (
        "/tools/mri-hmc/",
        "Head-motion correction",
        "fsl",
        [("clean-room original", "mri-hmc", "experimental", None, None)],
    ),
    (
        "/tools/resample-series/",
        "One-shot BOLD resampler",
        "larmorx",
        [("port", "resample-series", "validated", None, None)],
    ),
    (
        "/tools/nifti-io/",
        "NIfTI I/O",
        "larmorx",
        [("own implementation", "nifti-io", "validated", None, None)],
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
        "note": "Every tool counts its evidence page's verdicts, case by case: each case's verdict "
        "is decided from what its record reports and measures (identical to the unmodified "
        "original, identical once it uses correctly rounded maths, within the original's own "
        "variability, an expected divergence, rejected by both, an open difference).",
        "pilot": sorted(TOOLS),
    }
