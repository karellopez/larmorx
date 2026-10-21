# SPDX-License-Identifier: Apache-2.0
"""Command line: ``python -m larmorx_validation parity <suite> [--tier smoke] [--out DIR]``.

``python -m larmorx_validation pipeline-stages --oracle <run dir> --out DIR`` compares larmorx
with a real fMRIPrep run, stage by stage (:mod:`larmorx_validation.stages`).

``python -m larmorx_validation figures <tool>|all --out DIR`` draws a tool's figures from the
committed records and the figures cache (:mod:`larmorx_validation.figures`, ``docs/figures.md``).

``--implementation replica`` runs a suite against the GPL replica (``larmorx-gpl``, which
larmorx runs as a separate process) instead of the clean-room original, where a suite has one.
"""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

from larmorx_validation import environment
from larmorx_validation.report import write_json

#: Parity suites: name -> module (each has ``suite()``). Add new suites here.
PARITY_MODULES = {
    "nifti-io": "nifti_io",
    "itk-geometry": "itk_geometry",
    "ants-apply-transforms": "ants_apply_transforms",
    "itk-hdf5": "itk_hdf5",
    "afni-tshift": "afni_tshift",
    "resample-series": "resample_series",
    "numpy-linalg": "numpy_linalg",
    "mri-hmc": "mri_hmc",
    "ants-image-math": "ants_image_math",
    "ants-threshold-image": "ants_threshold_image",
    "ants-multiply-images": "ants_multiply_images",
    "ants-smooth-image": "ants_smooth_image",
    "ants-resample-image-by-spacing": "ants_resample_image_by_spacing",
    "ants-resample-image": "ants_resample_image",
    "ants-n4": "ants_n4",
    "ants-denoise": "ants_denoise",
    "ants-registration": "ants_registration",
    "ants-ai": "ants_ai",
    "ants-affine-initializer": "ants_affine_initializer",
}
PARITY_SUITES = tuple(PARITY_MODULES)
#: Benchmark suites: name -> module in ``larmorx_validation.bench`` (each has ``main(args)``).
BENCH_MODULES = {
    "nifti-io": "nifti_io",
    "ants-apply-transforms": "ants_apply_transforms",
    "afni-tshift": "afni_tshift",
    "resample-series": "resample_series",
    "mri-hmc": "mri_hmc",
    "ants-programs": "ants_programs",
    "ants-gaussian": "ants_gaussian",
    "ants-morphology": "ants_morphology",
    "ants-n4": "ants_n4",
    "ants-denoise": "ants_denoise",
}
BENCH_SUITES = tuple(BENCH_MODULES)


#: Suites that can also validate a replica binary (docs/licensing.md).
REPLICA_SUITES = ("afni-tshift",)


def _parity_suite(name: str, implementation: str = "original"):
    import importlib

    if name not in PARITY_MODULES:
        raise SystemExit(f"unknown parity suite {name!r}; choose from {PARITY_SUITES}")
    if implementation != "original" and name not in REPLICA_SUITES:
        raise SystemExit(f"suite {name!r} has no replica; replicas exist for {REPLICA_SUITES}")
    module = importlib.import_module(f"larmorx_validation.parity.{PARITY_MODULES[name]}")
    return module.suite(implementation) if name in REPLICA_SUITES else module.suite()


def _redacted(result):
    """The result with machine-specific paths removed from its messages."""
    from dataclasses import replace

    from larmorx_validation.report import redact

    checks = [replace(c, detail=redact(c.detail)) for c in result.checks]
    return replace(result, reason=redact(result.reason), checks=checks)


def _summary(result) -> dict:
    """What the JSON record keeps of a case: its status, anything that did not pass, and the
    values a suite reports without a threshold (``threshold="reported"``, e.g. the number of
    values that differ from the original's where a case passes within tolerances), which the
    documentation site's case explorer reads (``python -m larmorx_validation evidence``)."""
    out = {
        "case": result.case,
        "category": result.category,
        "status": result.status,
        "reason": result.reason,
        "checks_passed": sum(c.passed for c in result.checks),
        "checks_total": len(result.checks),
        "failed_checks": [c.__dict__ for c in result.failed_checks],
        "seconds": round(result.seconds, 3),
    }
    reported = {c.name: c.value for c in result.checks if c.threshold == "reported"}
    if reported:
        out["reported"] = reported
    return out


def cmd_parity(args: argparse.Namespace) -> int:
    from larmorx_validation.parity import FAIL, run, summarise
    from larmorx_validation.parity.harness import suite_by_name
    from larmorx_validation.parity.render import render

    suite = _parity_suite(args.suite, args.implementation)

    def progress(r):
        mark = {
            "pass": ".",
            "both-error": "b",
            "expected-divergence": "e",
            "fail": "F",
            "skipped": "s",
        }[r.status]
        print(mark, end="", flush=True, file=sys.stderr)

    import functools

    results = run(
        suite,
        args.tier,
        select=(lambda c: args.case in c.id) if args.case else None,
        progress=progress,
        jobs=args.jobs,
        factory=functools.partial(suite_by_name, args.suite, args.implementation),
    )
    print(file=sys.stderr)
    counts = summarise(results)
    print(
        f"{suite.name}: {len(results)} cases: "
        + ", ".join(f"{v} {k}" for k, v in counts.items() if v),
        file=sys.stderr,
    )
    for r in results:
        if r.status == FAIL:
            print(f"FAIL {r.case}", file=sys.stderr)
            for c in r.failed_checks:
                print(f"    {c.name}: {c.detail}", file=sys.stderr)
            if r.reason:
                print("    " + r.reason.strip().replace("\n", "\n    "), file=sys.stderr)
    if args.out:
        results = [_redacted(r) for r in results]
        env = environment.describe(suite.packages)
        command = f"python -m larmorx_validation parity {args.suite} --tier {args.tier}"
        if args.implementation != "original":
            command += f" --implementation {args.implementation}"
        out = Path(args.out)
        out.mkdir(parents=True, exist_ok=True)
        (out / f"{suite.name}.md").write_text(
            render(suite, results, env, args.tier, command), encoding="utf-8"
        )
        write_json(
            out / f"{suite.name}.json",
            {
                "suite": suite.name,
                "tier": args.tier,
                "reference": suite.reference,
                "environment": env,
                "summary": counts,
                "results": [_summary(r) for r in results],
            },
        )
        print(f"report: {out / (suite.name + '.md')}", file=sys.stderr)
    return 1 if counts[FAIL] else 0


def cmd_pipeline_stages(args: argparse.Namespace) -> int:
    from larmorx_validation import stages

    return stages.cmd(args)


def cmd_bench(args: argparse.Namespace) -> int:
    import importlib

    if args.suite not in BENCH_MODULES:
        raise SystemExit(f"unknown benchmark suite {args.suite!r}; choose from {BENCH_SUITES}")
    bench = importlib.import_module(f"larmorx_validation.bench.{BENCH_MODULES[args.suite]}")
    return bench.main(args)


def cmd_figures(args: argparse.Namespace) -> int:
    from larmorx_validation import figures

    return figures.main(args)


def cmd_evidence(args: argparse.Namespace) -> int:
    from larmorx_validation import evidence

    return evidence.main(args)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m larmorx_validation")
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("parity", help="compare larmorx with a reference implementation")
    p.add_argument("suite", choices=PARITY_SUITES)
    p.add_argument("--tier", default="smoke", choices=("smoke", "standard", "full"))
    p.add_argument("--case", help="only cases whose id contains this text")
    p.add_argument("--out", help="write <suite>.md and <suite>.json to this directory")
    p.add_argument(
        "--jobs",
        type=int,
        default=1,
        help="run the cases in this many worker processes (each case's own thread counts are "
        "unchanged; use on a machine with the cores to spare, never for benchmarks)",
    )
    p.add_argument(
        "--implementation",
        default="original",
        choices=("original", "replica"),
        help="the clean-room original (in-process) or the GPL replica (the larmorx-gpl binary, "
        "found through LARMORX_GPL_BIN or the crates-gpl build, which larmorx runs as a "
        "separate process)",
    )
    p.set_defaults(func=cmd_parity)
    b = sub.add_parser("bench", help="benchmark larmorx against reference implementations")
    b.add_argument("suite", choices=BENCH_SUITES)
    b.add_argument("--tier", default="full", choices=("smoke", "standard", "full"))
    b.add_argument("--repeats", type=int, default=5)
    b.add_argument(
        "--threads",
        type=int,
        nargs="+",
        default=[1, 4, 0],
        help="thread counts for larmorx (0 = all)",
    )
    b.add_argument("--out", help="write <suite>.md and <suite>.json to this directory")
    b.add_argument(
        "--quick", action="store_true", help="fewer files and repeats (for CI smoke runs)"
    )
    b.set_defaults(func=cmd_bench)
    st = sub.add_parser(
        "pipeline-stages",
        help="compare larmorx with an fMRIPrep oracle run, stage by stage, on fMRIPrep's own "
        "inputs and parameters",
    )
    from larmorx_validation.stages import add_arguments

    add_arguments(st)
    st.set_defaults(func=cmd_pipeline_stages)
    fg = sub.add_parser(
        "figures",
        help="draw a tool's figures (records, benchmarks, real outputs) for the documentation site",
    )
    fg.add_argument("tool", help="a tool (larmorx_validation.figures.TOOLS) or 'all'")
    fg.add_argument("--out", required=True, help="write <tool>/<figure>-<theme>.svg and .json here")
    fg.add_argument(
        "--cache",
        help="the figures cache (default: LARMORX_FIGURES_CACHE, else the workspace's "
        "oracles/figures-cache)",
    )
    fg.add_argument("--only", nargs="+", help="only these figures (their names)")
    fg.add_argument(
        "--no-compute",
        action="store_true",
        help="never compute a missing cache entry: skip the figures that need one",
    )
    fg.set_defaults(func=cmd_figures)
    ev = sub.add_parser(
        "evidence",
        help="export a tool's validation evidence (worked examples, metrics, verdicts, cases) "
        "for the documentation site's interactive pages",
    )
    ev.add_argument("tool", help="a tool (larmorx_validation.evidence.TOOLS), 'summary' or 'all'")
    ev.add_argument("--site", required=True, help="the documentation site's checkout")
    ev.add_argument(
        "--cache",
        help="the figures cache (default: LARMORX_FIGURES_CACHE, else the workspace's "
        "oracles/figures-cache)",
    )
    ev.add_argument(
        "--no-compute",
        action="store_true",
        help="never compute a missing cache entry: skip the tools that need one",
    )
    ev.set_defaults(func=cmd_evidence)
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
