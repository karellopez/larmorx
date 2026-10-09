"""Command line: ``python -m larmorx_validation parity <suite> [--tier smoke] [--out DIR]``."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

from larmorx_validation import environment
from larmorx_validation.report import write_json

PARITY_SUITES = ("nifti-io", "itk-geometry", "ants-apply-transforms", "afni-tshift")
BENCH_SUITES = ("nifti-io", "ants-apply-transforms")


def _parity_suite(name: str):
    if name == "nifti-io":
        from larmorx_validation.parity import nifti_io

        return nifti_io.suite()
    if name == "itk-geometry":
        from larmorx_validation.parity import itk_geometry

        return itk_geometry.suite()
    if name == "ants-apply-transforms":
        from larmorx_validation.parity import ants_apply_transforms

        return ants_apply_transforms.suite()
    if name == "afni-tshift":
        from larmorx_validation.parity import afni_tshift

        return afni_tshift.suite()
    raise SystemExit(f"unknown parity suite {name!r}; choose from {PARITY_SUITES}")


def _redacted(result):
    """The result with machine-specific paths removed from its messages."""
    from dataclasses import replace

    from larmorx_validation.report import redact

    checks = [replace(c, detail=redact(c.detail)) for c in result.checks]
    return replace(result, reason=redact(result.reason), checks=checks)


def _summary(result) -> dict:
    """What the JSON record keeps of a case: its status and anything that did not pass."""
    return {
        "case": result.case,
        "category": result.category,
        "status": result.status,
        "reason": result.reason,
        "checks_passed": sum(c.passed for c in result.checks),
        "checks_total": len(result.checks),
        "failed_checks": [c.__dict__ for c in result.failed_checks],
        "seconds": round(result.seconds, 3),
    }


def cmd_parity(args: argparse.Namespace) -> int:
    from larmorx_validation.parity import FAIL, run, summarise
    from larmorx_validation.parity.render import render

    suite = _parity_suite(args.suite)

    def progress(r):
        mark = {
            "pass": ".",
            "both-error": "b",
            "expected-divergence": "e",
            "fail": "F",
            "skipped": "s",
        }[r.status]
        print(mark, end="", flush=True, file=sys.stderr)

    results = run(
        suite,
        args.tier,
        select=(lambda c: args.case in c.id) if args.case else None,
        progress=progress,
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


def cmd_bench(args: argparse.Namespace) -> int:
    if args.suite == "nifti-io":
        from larmorx_validation.bench import nifti_io as bench
    elif args.suite == "ants-apply-transforms":
        from larmorx_validation.bench import ants_apply_transforms as bench
    else:
        raise SystemExit(f"unknown benchmark suite {args.suite!r}; choose from {BENCH_SUITES}")
    return bench.main(args)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m larmorx_validation")
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("parity", help="compare larmorx with a reference implementation")
    p.add_argument("suite", choices=PARITY_SUITES)
    p.add_argument("--tier", default="smoke", choices=("smoke", "standard", "full"))
    p.add_argument("--case", help="only cases whose id contains this text")
    p.add_argument("--out", help="write <suite>.md and <suite>.json to this directory")
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
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
