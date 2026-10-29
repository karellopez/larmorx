# SPDX-License-Identifier: Apache-2.0
"""Command line: ``python -m larmorx_validation parity <suite> [--tier smoke] [--out DIR]``.

``python -m larmorx_validation pipeline-stages --oracle <run dir> --out DIR`` compares larmorx
with a real fMRIPrep run, stage by stage (:mod:`larmorx_validation.stages`).

``python -m larmorx_validation figures <tool>|all --out DIR`` draws a tool's figures from the
committed records and the figures cache (:mod:`larmorx_validation.figures`, ``docs/figures.md``).

``--implementation replica`` runs a suite against the GPL replica (``larmorx-gpl``, which
larmorx runs as a separate process) instead of the clean-room original, where a suite has one.
The FSL replica (``larmorx-nc``) has its own suite, ``fsl-mcflirt-replica``.

``--oracle-cache use|refresh|off`` says how the original programs' runs use the oracle-output
cache (:mod:`larmorx_validation.oracle_cache`); ``python -m larmorx_validation oracle-cache
verify --sample N`` re-runs cached entries to check them.
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
    "afni-automask": "afni_automask",
    "afni-unifize": "afni_unifize",
    "resample-series": "resample_series",
    "numpy-linalg": "numpy_linalg",
    "mri-hmc": "mri_hmc",
    "fsl-mcflirt-replica": "fsl_mcflirt_replica",
    "ants-image-math": "ants_image_math",
    "ants-threshold-image": "ants_threshold_image",
    "ants-multiply-images": "ants_multiply_images",
    "ants-smooth-image": "ants_smooth_image",
    "ants-resample-image-by-spacing": "ants_resample_image_by_spacing",
    "ants-resample-image": "ants_resample_image",
    "ants-n4": "ants_n4",
    "ants-denoise": "ants_denoise",
    "ants-registration": "ants_registration",
    "ants-registration-corrections": "ants_registration_corrections",
    "ants-ai": "ants_ai",
    "ants-affine-initializer": "ants_affine_initializer",
    "ants-atropos": "ants_atropos",
    "fsl-bet": "fsl_bet",
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
    "ants-ai": "ants_ai",
}
BENCH_SUITES = tuple(BENCH_MODULES)


#: Suites that can also validate a replica binary (docs/licensing.md).
REPLICA_SUITES = ("afni-tshift", "afni-automask", "afni-unifize", "fsl-bet")


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
    """What the JSON record keeps of a case: its status, anything that did not pass, the
    values a suite reports without a threshold (``threshold="reported"`` or ``"reported (...)"``,
    e.g. the number of values that differ from the original's where a case passes within
    tolerances, or a comparison that glibc's maths decided), and the measured value of every
    other check that has one (``measured``, e.g. a run's 95th-percentile deviation). The
    documentation site's case explorer reads them to decide each case's verdict
    (``python -m larmorx_validation evidence``)."""
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

    def is_reported(c) -> bool:
        return isinstance(c.threshold, str) and c.threshold.startswith("reported")

    reported = {c.name: c.value for c in result.checks if is_reported(c)}
    if reported:
        out["reported"] = reported
    measured = {
        c.name: c.value for c in result.checks if c.value is not None and not is_reported(c)
    }
    if measured:
        out["measured"] = measured
    return out


def oracle_summary(results, mode: str) -> dict:
    """The original programs' runs of a parity run, summed over its cases (``CaseResult.oracle``),
    with the cache's mode and location (as a record names it)."""
    from larmorx_validation import oracle_cache
    from larmorx_validation.oracle_cache.store import workspace
    from larmorx_validation.report import redact

    tally = oracle_cache.Tally()
    for r in results:
        tally.add(oracle_cache.Tally.from_dict(r.oracle))
    where = oracle_cache.location()
    ws = workspace()
    if ws is not None and where.is_relative_to(ws):
        shown = "<workspace>/" + where.relative_to(ws).as_posix()
    else:
        shown = redact(str(where))
    return {"mode": mode, "location": shown, **tally.to_dict()}


def _report_differences(summary: dict) -> None:
    """The differences between fresh runs and their stored entries, loudly (findings)."""
    found = summary.get("differences", [])
    if not found:
        return
    print(
        f"\nORACLE CACHE: {len(found)} run(s) of an original program differ from their stored "
        "entries. Each is a finding (the original varies, or the key misses something): the "
        "entries are never served again, both results are kept.",
        file=sys.stderr,
    )
    for d in found:
        print(f"  {d['case']} ({d['program']}): {'; '.join(d['differences'])[:300]}",
              file=sys.stderr)  # fmt: skip
        print(f"    dispute: {d['dispute']}", file=sys.stderr)


def cmd_parity(args: argparse.Namespace) -> int:
    import os

    from larmorx_validation import oracle_cache
    from larmorx_validation.parity import FAIL, run, summarise
    from larmorx_validation.parity.harness import suite_by_name
    from larmorx_validation.parity.render import render

    if args.oracle_cache:
        os.environ[oracle_cache.ENV_MODE] = args.oracle_cache  # the workers inherit it
    cache_mode = oracle_cache.mode()
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
    import re

    pattern = re.compile(args.case) if args.case else None
    results = run(
        suite,
        args.tier,
        select=(lambda c: pattern.search(c.id) is not None) if pattern else None,
        progress=progress,
        jobs=args.jobs,
        factory=functools.partial(suite_by_name, args.suite, args.implementation),
    )
    print(file=sys.stderr)
    counts = summarise(results)
    oracle = oracle_summary(results, cache_mode)
    print(
        f"{suite.name}: {len(results)} cases: "
        + ", ".join(f"{v} {k}" for k, v in counts.items() if v),
        file=sys.stderr,
    )
    print(
        f"original programs: {oracle['served']} runs from the oracle cache "
        f"({oracle['saved_seconds']:.0f} s saved), {oracle['ran']} run; cache "
        f"{oracle_cache.location()} (mode {cache_mode})",
        file=sys.stderr,
    )
    for why, n in sorted(oracle["fresh"].items(), key=lambda kv: -kv[1]):
        print(f"    {n:5d} ran: {why}", file=sys.stderr)
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
        if args.case:
            command += f" --case '{args.case}'"
        if args.implementation != "original":
            command += f" --implementation {args.implementation}"
        out = Path(args.out)
        out.mkdir(parents=True, exist_ok=True)
        (out / f"{suite.name}.md").write_text(
            render(suite, results, env, args.tier, command, oracle), encoding="utf-8"
        )
        write_json(
            out / f"{suite.name}.json",
            {
                "suite": suite.name,
                "tier": args.tier,
                "reference": suite.reference,
                "environment": env,
                "oracle_cache": {k: v for k, v in oracle.items() if k != "differences"}
                | {"differences": len(oracle["differences"])},
                "summary": counts,
                "results": [_summary(r) for r in results],
            },
        )
        print(f"report: {out / (suite.name + '.md')}", file=sys.stderr)
    _report_differences(oracle)
    if counts[FAIL]:
        return 1
    # A difference ``refresh`` was asked to look for fails the command (a finding).
    return 1 if cache_mode == "refresh" and oracle["differences"] else 0


def cmd_oracle_cache(args: argparse.Namespace) -> int:
    from larmorx_validation.oracle_cache import cli

    return cli.main(args)


def cmd_difftest(args: argparse.Namespace) -> int:
    from larmorx_validation.difftest import cli

    return cli.main(args)


def cmd_tables(args: argparse.Namespace) -> int:
    from larmorx_validation import tables

    return tables.main(args)


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
    p.add_argument(
        "--tier",
        default="smoke",
        choices=("smoke", "standard", "full", "panel"),
        help="smoke, standard and full each include the tiers before them; panel runs the data "
        "panel's cases alone (fMRIPrep's calls on every oracle run, the panel's images)",
    )
    p.add_argument(
        "--case",
        help="only cases whose id this regular expression matches (a plain text: the ids that "
        "contain it)",
    )
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
    p.add_argument(
        "--oracle-cache",
        choices=("use", "refresh", "off"),
        default=None,
        help="the original programs' runs: 'use' the oracle cache (the default; a run missing "
        "from it runs and is stored), 'refresh' (run every cacheable run again and compare it "
        "with its entry: a difference is a finding and fails the command), or 'off' "
        "(LARMORX_ORACLE_CACHE_MODE; the cache is LARMORX_ORACLE_CACHE, else "
        "<workspace>/oracles/cache; docs/validation/oracle-cache.md)",
    )
    p.set_defaults(func=cmd_parity)
    oc = sub.add_parser(
        "oracle-cache",
        help="the oracle-output cache: verify entries by running them again, confirm runs "
        "above one thread, statistics, disputes",
    )
    from larmorx_validation.oracle_cache.cli import add_arguments as oracle_arguments

    oracle_arguments(oc)
    oc.set_defaults(func=cmd_oracle_cache)
    dtp = sub.add_parser(
        "difftest",
        help="differential random testing: random valid inputs and options through each original "
        "program and larmorx, differences shrunk (docs/validation/difftest.md)",
    )
    from larmorx_validation.difftest.cli import add_arguments as difftest_arguments

    difftest_arguments(dtp)
    dtp.set_defaults(func=cmd_difftest)
    tb = sub.add_parser(
        "tables",
        help="write the tables that repeat the records' facts (README, the validation index, the "
        "site's status) from validation/tools.toml and the records; --check fails on a difference",
    )
    from larmorx_validation.tables import add_arguments as tables_arguments

    tables_arguments(tb)
    tb.set_defaults(func=cmd_tables)
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
    ev.add_argument(
        "tool",
        help="a tool (larmorx_validation.evidence.TOOLS), 'summary', 'panel' (the Data panel "
        "page), 'all', or 'sizes' (the published site's size per page, against the budgets)",
    )
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
