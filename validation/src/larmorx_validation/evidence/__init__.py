# SPDX-License-Identifier: Apache-2.0
"""The evidence of larmorx's validation, for the documentation site's interactive pages.

``python -m larmorx_validation evidence <tool>|all --site <larmorx_documentation>`` exports, for
each tool, one JSON file (``static/evidence/<tool>.json``) and content-addressed binary blobs
(``static/evidence/blobs/``): the worked examples on real data (the input, the original
program's outputs, larmorx's outputs, each labelled with the exact program, build, thread count
and mode), the metrics of every comparison computed in float64 from those outputs, a verdict
per comparison from the site's one taxonomy with its reason, every parity case of the tool's
records for the case explorer, and the data of the charts. The site's components draw them;
the site never computes a number of its own. The guide: ``docs/figures.md``.
"""

from __future__ import annotations

#: Evidence pages: tool -> module in this package (each has ``export(ctx) -> dict``).
TOOLS: dict[str, str] = {
    "ants-registration": "ants_registration",
    "ants-n4": "ants_n4",
    "ants-denoise": "ants_denoise",
    "ants-apply-transforms": "ants_apply_transforms",
    "ants-programs": "ants_programs",
    "ants-ai": "ants_ai",
    "ants-atropos": "ants_atropos",
    "itk-hdf5": "itk_hdf5",
    "afni-tshift": "afni_tshift",
    "afni-automask": "afni_automask",
    "afni-unifize": "afni_unifize",
    "fsl-bet": "fsl_bet",
    "mri-hmc": "mri_hmc",
    "resample-series": "resample_series",
    "nifti-io": "nifti_io",
}


#: Targets of ``evidence`` that are not a tool.
OTHER = ("summary", "panel", "sizes", "all")


def main(args) -> int:
    import importlib
    from pathlib import Path

    from larmorx_validation.evidence import credits, sizes
    from larmorx_validation.evidence.core import Context, prune, write_tool
    from larmorx_validation.figures.core import REPO, default_cache

    if args.tool == "sizes":
        rep = sizes.report(Path(args.site))
        print(sizes.table(rep))
        bad = sizes.problems(rep)
        for p in bad:
            print("OVER BUDGET:", p)
        return 1 if bad else 0
    tools = list(TOOLS) if args.tool == "all" else [] if args.tool in OTHER else [args.tool]
    for tool in tools:
        if tool not in TOOLS:
            raise SystemExit(
                f"unknown tool {tool!r}; choose from {', '.join(TOOLS)}, {', '.join(OTHER)}"
            )
    ctx = Context(
        repo=REPO,
        site=Path(args.site),
        cache=Path(args.cache) if args.cache else default_cache(),
        compute=not args.no_compute,
    )
    status = 0
    for tool in tools:
        module = importlib.import_module(f"larmorx_validation.evidence.{TOOLS[tool]}")
        try:
            doc = module.export(ctx)
        except FileNotFoundError as e:  # a cache entry or an oracle is missing
            print(f"skipped {tool}: {e}")
            status = 1
            continue
        path = write_tool(ctx, tool, credits.attach(tool, doc))
        print(f"{tool}: {path}")
    if args.tool in ("all", "summary"):
        from larmorx_validation.evidence import summary

        print(f"summary: {write_tool(ctx, 'summary', summary.export(ctx))}")
    if args.tool in ("all", "panel"):
        from larmorx_validation.evidence import panel

        print(f"panel: {write_tool(ctx, 'panel', panel.export(ctx))}")
    removed = prune(ctx)
    if removed:
        print(f"removed {removed} blob(s) no evidence file refers to any more")
    print(f"manifest: {sizes.write_manifest(ctx.site)}")
    rep = sizes.report(ctx.site)
    print(sizes.table(rep))
    bad = sizes.problems(rep)
    for p in bad:
        print("OVER BUDGET:", p)
    return 1 if bad else status
