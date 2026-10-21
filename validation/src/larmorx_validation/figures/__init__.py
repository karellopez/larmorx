# SPDX-License-Identifier: Apache-2.0
"""Figures of larmorx's validation: how each tool compares with the original program.

``python -m larmorx_validation figures <tool>|all --out <dir>`` draws a tool's figures from the
committed records (``docs/validation``, ``docs/benchmarks``) and, for image figures, from real
outputs of the original programs and of larmorx kept in the figures cache (computed when
missing). Every figure is written in the documentation site's two themes and at two widths,
with its provenance beside it. The guide: ``docs/figures.md``.
"""

from __future__ import annotations

#: Figure pages: tool -> module in this package (each has ``figures(ctx) -> list[Figure]``).
TOOLS: dict[str, str] = {
    "ants-registration": "ants_registration",
    "ants-n4": "ants_n4",
    "ants-denoise": "ants_denoise",
    "ants-apply-transforms": "ants_apply_transforms",
    "afni-tshift": "afni_tshift",
    "resample-series": "resample_series",
    "mri-hmc": "mri_hmc",
    "ants-programs": "ants_programs",
    "ants-ai": "ants_ai",
    "nifti-io": "nifti_io",
    "itk-hdf5": "itk_hdf5",
}


def main(args) -> int:
    import importlib
    from pathlib import Path

    from larmorx_validation.figures.core import REPO, Context, command_line, default_cache, render

    tools = list(TOOLS) if args.tool == "all" else [args.tool]
    ctx = Context(
        repo=REPO,
        out=Path(args.out),
        cache=Path(args.cache) if args.cache else default_cache(),
        compute=not args.no_compute,
        only=tuple(args.only or ()),
        command=command_line(),
    )
    status = 0
    for tool in tools:
        if tool not in TOOLS:
            raise SystemExit(f"unknown tool {tool!r}; choose from {', '.join(TOOLS)} or all")
        module = importlib.import_module(f"larmorx_validation.figures.{TOOLS[tool]}")
        for fig in module.figures(ctx):
            if ctx.only and fig.name not in ctx.only:
                continue
            try:
                path = render(fig, tool, ctx)
            except FileNotFoundError as e:  # a cache entry or an oracle is missing
                print(f"skipped {tool}/{fig.name}: {e}")
                status = 1
                continue
            print(f"{tool}/{fig.name}: {path.parent}")
    return status
