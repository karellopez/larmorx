# SPDX-License-Identifier: Apache-2.0
"""``python -m larmorx_validation difftest``: the night's run, one case, a replay, the report,
a permanent case (``docs/validation/difftest.md``).

    difftest --tools all --budget 6 --jobs 4 --seed 20261025          a night (hours of budget)
    difftest --case ants-image-math:20261025:17 [--show] [--no-shrink] one case, from its seed
    difftest --replay oracles/difftest/<date>/findings/<tool>/<f>.json  a finding's reproducer
    difftest --replay validation/difftest/cases/<tool>/<id>.json       a permanent case
    difftest --report oracles/difftest/<date>                          report.md, summary.json
    difftest --promote <finding.json> --id <name> [--tier smoke] ...   a permanent case
    difftest --list                                                    the tools, and why not
    difftest --heavy oracles/difftest/<date> [--heavy-hours 2]          the night's heavy cases

A night, ``--case`` and ``--replay`` run in a memory box (``--box-gib``, 8 GiB: the command
re-runs itself in a systemd user scope unless it already runs in one, e.g. through
``scripts/orchestration/boxed.sh``), each worker (each case) with a hard limit of
``--memory-gib`` (3 GiB) and no swap (:mod:`~larmorx_validation.difftest.limits`).
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import sys
from pathlib import Path

from larmorx_validation.difftest import core, tools


def add_arguments(p: argparse.ArgumentParser) -> None:
    from larmorx_validation.parity import afni_versions

    p.add_argument("--tools", default="all", help="'all' or a comma-separated list of tools")
    afni_versions.add_argument(p)
    p.add_argument(
        "--budget", type=float, default=6.0, help="hours of random cases (a night: about 6)"
    )
    p.add_argument(
        "--jobs", type=int, default=2, help="cases at a time (each at --threads threads)"
    )
    p.add_argument(
        "--threads", type=int, default=1, help="threads of each original and larmorx run"
    )
    p.add_argument("--seed", default=None, help="the night's seed (default: the date, YYYYMMDD)")
    p.add_argument(
        "--out",
        default=None,
        help="the night's directory (default: <workspace>/oracles/difftest/<date>)",
    )
    p.add_argument(
        "--shrink-budget", type=float, default=600.0, help="seconds to shrink one difference"
    )
    p.add_argument("--max-cases", type=int, default=None, help="at most this many cases per tool")
    p.add_argument(
        "--memory-gib",
        type=float,
        default=None,
        help="the hard memory limit of each worker and of every process it starts, no swap "
        "(default 3; 0: no limits, only where cgroup v2 is not writable)",
    )
    p.add_argument(
        "--box-gib",
        type=float,
        default=None,
        help="the memory box this command makes for itself when it runs in none (default 8)",
    )
    p.add_argument("--case", help="run one case: TOOL:SEED:N")
    p.add_argument("--show", action="store_true", help="with --case: print its recipe only")
    p.add_argument(
        "--no-shrink", action="store_true", help="with --case: do not shrink a difference"
    )
    p.add_argument("--replay", help="a finding's or a permanent case's JSON: run its recipe again")
    p.add_argument("--report", help="write report.md and summary.json of a night's directory")
    p.add_argument("--promote", help="a finding's JSON: write it as a permanent case")
    p.add_argument("--id", help="with --promote: the permanent case's name")
    p.add_argument("--tier", default="smoke", choices=("smoke", "standard", "full"))
    p.add_argument(
        "--expect",
        default="pass",
        choices=("pass", "divergence", "error", "abort", "crash", "guard"),
    )
    p.add_argument("--reason", default="")
    p.add_argument("--status", default="fixed", choices=("fixed", "register", "open"))
    p.add_argument("--defect", default=None)
    p.add_argument("--rule", default=None)
    p.add_argument("--description", default="")
    p.add_argument(
        "--list", action="store_true", help="list the tools and whether they can run here"
    )
    p.add_argument(
        "--heavy",
        help="a night's directory: run its heavy cases (heavy.jsonl) to their end, one at a "
        "time, in a box of 24 GiB and 48 GiB of swap (only when the machine is quiet)",
    )
    p.add_argument(
        "--heavy-hours", type=float, default=2.0, help="with --heavy: hours for each run"
    )
    p.add_argument(
        "--heavy-only",
        default=None,
        help="with --heavy: the cases to run (TOOL:SEED:N, comma-separated, or a file of them, "
        "one per line); the others wait",
    )
    p.add_argument("--case-timeout", type=float, default=None, help=argparse.SUPPRESS)
    p.add_argument("--judge-recipe", help=argparse.SUPPRESS)
    p.add_argument("--impl", help=argparse.SUPPRESS)


def _workspace() -> Path:
    from larmorx_validation.oracle_cache.store import workspace

    ws = workspace()
    if ws is None:
        raise SystemExit("the workspace (with oracles/) was not found; pass --out")
    return ws


def _cases_named(spec: str | None) -> set[str] | None:
    """``--heavy-only``: a comma-separated list of ``tool:seed:n``, or a file of them."""
    if not spec:
        return None
    path = Path(spec)
    text = path.read_text(encoding="utf-8") if path.is_file() else spec.replace(",", "\n")
    return {line.strip() for line in text.splitlines() if line.strip()}


def main(args: argparse.Namespace) -> int:
    from larmorx_validation.parity import afni_versions

    afni_versions.apply(args)  # --afni-version: the workers inherit it
    if args.list:
        for name in tools.names():
            t = tools.get(name)
            why = t.unavailable()
            print(f"{name:32s} {', '.join(t.implementations):20s} {why or 'ready'}")
        return 0
    if args.report:
        from larmorx_validation.difftest import report

        print(report.write(Path(args.report)))
        return 0
    if args.promote:
        from larmorx_validation.difftest import permanent

        if not args.id:
            raise SystemExit("--promote needs --id")
        path = permanent.promote(
            Path(args.promote),
            args.id,
            tier=args.tier,
            expect=args.expect,
            reason=args.reason,
            status=args.status,
            defect=args.defect,
            rule=args.rule,
            description=args.description,
        )
        print(path)
        return 0
    from larmorx_validation.difftest import limits, runner

    if args.heavy:
        limits.ensure_box(
            sys.argv[1:], args.box_gib or limits.HEAVY_GIB, swap_gib=limits.HEAVY_SWAP_GIB
        )
        os.environ.setdefault("LARMORX_ORACLE_CACHE_MODE", "off")
        runner.limit_threads(args.threads)
        done = runner.run_heavy(
            Path(args.heavy),
            memory_gib=args.memory_gib or limits.HEAVY_GIB,
            hours=args.heavy_hours,
            only=_cases_named(args.heavy_only),
            log=lambda s: print(s, file=sys.stderr, flush=True),
        )
        print(f"{len(done)} heavy case(s) run; {Path(args.heavy) / 'report.md'}")
        return 0
    memory_gib = limits.CASE_GIB if args.memory_gib is None else args.memory_gib
    if memory_gib and not args.judge_recipe and not args.show:
        # Every run of random inputs is held in a memory box (CLAUDE.md, 2026-10-25).
        limits.ensure_box(sys.argv[1:], args.box_gib or limits.BOX_GIB)
        box = limits.MemoryBox.find() if (args.case or args.replay) else None
        if box is not None:
            # One case in this process: it and every program it starts held in a leaf.
            box.nest()
            box.leaf("case", int(memory_gib * limits.GIB)).enter()
            limits.watch_exits()
    runner.limit_threads(args.threads)
    if args.judge_recipe:
        # A child process of a crash case: one verdict, as JSON on the last line.
        doc = json.loads(Path(args.judge_recipe).read_text(encoding="utf-8"))
        j = runner.judge_once(
            tools.get(doc["tool"]), doc["recipe"], args.impl, timeout=args.case_timeout
        )
        print(
            json.dumps(
                {
                    "verdict": j.verdict,
                    "signature": list(j.signature),
                    "detail": j.detail,
                    "status": j.status,
                    "checks": j.checks,
                },
                default=str,
            )
        )
        return 0
    if args.case:
        name, seed, n = args.case.rsplit(":", 2)
        t = tools.get(name)
        recipe = core.make_recipe(t, seed, int(n))
        if args.show:
            print(json.dumps(recipe, indent=1))
            return 0
        os.environ.setdefault("LARMORX_ORACLE_CACHE_MODE", "off")
        line = runner.run_case(
            name,
            seed,
            int(n),
            shrink=not args.no_shrink,
            shrink_budget=args.shrink_budget,
            store_findings=False,
        )
        print(json.dumps(line, indent=1, default=str)[:20000])
        return 1 if line.get("verdict") == core.DIFFER else 0
    if args.replay:
        doc = json.loads(Path(args.replay).read_text(encoding="utf-8"))
        recipe = doc.get("shrunk") or doc["recipe"]
        t = tools.get(doc["tool"])
        # A permanent case replays with its expectation (a guard case runs only larmorx's
        # default: its original would fill the machine).
        from larmorx_validation.difftest import permanent

        case_kw = permanent._kw(doc["tool"], doc) if "id" in doc and "found" in doc else {}
        status = 0
        for impl in doc.get("implementations") or [
            doc.get("implementation") or t.implementations[0]
        ]:
            j = runner.judge_once(
                t,
                recipe,
                impl,
                explore_mode=os.environ.get("LARMORX_ORACLE_CACHE_MODE", "use"),
                case_kw=case_kw,
            )
            print(f"{doc['tool']} ({impl}): {j.verdict} {', '.join(j.signature)}")
            if j.detail:
                print("  " + j.detail[:2000])
            status |= j.verdict == core.DIFFER
        return status
    seed = args.seed or dt.date.today().strftime("%Y%m%d")
    out = (
        Path(args.out)
        if args.out
        else _workspace() / "oracles" / "difftest" / dt.date.today().isoformat()
    )
    names = (
        tools.names()
        if args.tools == "all"
        else [s.strip() for s in args.tools.split(",") if s.strip()]
    )
    info = runner.night(
        out,
        tool_names=names,
        seed=seed,
        budget_s=args.budget * 3600.0,
        jobs=args.jobs,
        threads=args.threads,
        shrink_budget=args.shrink_budget,
        max_cases=args.max_cases,
        memory_gib=memory_gib,
        log=lambda s: print(s, file=sys.stderr, flush=True),
    )
    print(f"{out}/report.md ({info.get('elapsed_s')} s)")
    print("DIFFTEST FINISHED", flush=True)
    return 0


__all__ = ["add_arguments", "main"]
