# SPDX-License-Identifier: Apache-2.0
"""Stage-by-stage comparison of larmorx with a real fMRIPrep run.

For every step of fMRIPrep's pipeline that larmorx already implements, larmorx is given
fMRIPrep's own inputs and parameters for that step (read from nipype's working directory)
and its output is compared with fMRIPrep's output for that step. Differences are taken apart
into thread effects, the tool's version, glibc's maths and larmorx itself, by re-running the
step with fMRIPrep's own container and with the repository's oracle builds.

Usage::

    python -m larmorx_validation pipeline-stages \\
        --oracle <workspace>/oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall --out docs/validation

The oracle run directory is what ``scripts/run_fmriprep_oracle.sh`` writes (``RUN_INFO.txt``,
``out/``, ``work/``); larmorx's outputs and the reference runs go to
``<workspace>/oracles/compare/fmriprep-<version>/<run>/`` (never into git). It needs real data,
fMRIPrep's image (Apptainer) and, for the cause of differences, the ANTs 2.6.5 oracle build and
the CORE-MATH preload; it never runs in CI.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time
from collections.abc import Sequence
from pathlib import Path

from larmorx_validation.stages.catalogue import STAGES, missing_of, stage_of
from larmorx_validation.stages.nodes import compare_node
from larmorx_validation.stages.oracle import OracleRun
from larmorx_validation.stages.runners import Settings

__all__ = ["main", "run_stages"]


def _default_compare(run: OracleRun) -> Path:
    for parent in run.root.parents:
        if parent.name == "oracles":
            return parent / "compare" / f"fmriprep-{run.version}" / run.name
    return run.root.parent / "compare" / run.name


def _busy() -> list[str]:
    """Other heavy processes on the machine (parity runs, benchmarks, builds), so the record
    can say whether its times come from a quiet machine."""
    try:
        p = subprocess.run(
            ["pgrep", "-af", "[l]armorx_validation (parity|bench)|[c]argo (build|test)|[m]aturin"],
            capture_output=True,
            text=True,
        )
    except OSError:
        return []
    own = str(os.getpid())
    return [ln[:200] for ln in p.stdout.splitlines() if ln.split()[0] != own][:5]


def run_stages(s: Settings) -> dict:
    """Compare every node of every stage; returns the record's payload."""
    from larmorx_validation import environment

    run = s.run
    selected = []
    missing: dict[str, list] = {}
    for node in run.all_nodes:
        stage = stage_of(node)
        if stage is not None:
            if s.select and not any(k in node.path or k == stage.id for k in s.select):
                continue
            selected.append((stage, node))
            continue
        m = missing_of(node)
        if m is not None:
            missing.setdefault(m.tool, []).append(node)
    order = {st.id: i for i, st in enumerate(STAGES)}
    selected.sort(key=lambda sn: (order[sn[0].id], sn[1].path))
    # the environment as the run starts (the commit that produced the results)
    env = environment.describe(("nibabel", "scipy", "pandas"))
    load_before = os.getloadavg()
    busy_before = _busy()
    started = time.strftime("%Y-%m-%dT%H:%M:%S%z")
    results = []
    for i, (stage, node) in enumerate(selected, 1):
        s.say(f"[{i}/{len(selected)}] {stage.id} {node.path}")
        r = compare_node(s, node, stage)
        s.say(f"    {r.get('status')}")
        results.append(r)
    busy_after = _busy()
    return {
        "select": list(s.select),
        "started": started,
        "finished": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "load_before": load_before,
        "load_after": os.getloadavg(),
        "busy": sorted(set(busy_before + busy_after)),
        "environment": env,
        "results": results,
        "missing": {
            tool: [
                {"node": n.path, "program": n.program, "seconds": n.duration, "threads": n.threads}
                for n in nodes
            ]
            for tool, nodes in missing.items()
        },
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m larmorx_validation pipeline-stages")
    add_arguments(parser)
    return cmd(parser.parse_args(argv))


def add_arguments(p: argparse.ArgumentParser) -> None:
    p.add_argument("--oracle", required=True, help="the fMRIPrep oracle run directory")
    p.add_argument("--out", help="write fmriprep-stages.md and .json to this directory")
    p.add_argument("--name", default="fmriprep-stages", help="the record's file name")
    p.add_argument("--compare", help="where larmorx's outputs and reference runs go")
    p.add_argument("--stage", nargs="*", default=(), help="only these stage ids or node paths")
    p.add_argument("--threads", type=int, help="larmorx's threads (default: each node's own)")
    p.add_argument("--reuse", action="store_true", help="reuse reference runs from an earlier run")
    p.add_argument("--no-container", action="store_true", help="do not re-run fMRIPrep's tools")
    p.add_argument("--no-ants-oracle", action="store_true", help="do not run ANTs 2.6.5")
    p.add_argument(
        "--render",
        action="store_true",
        help="only write the Markdown record again from <out>/<name>.json (no runs)",
    )
    p.add_argument(
        "--bind",
        action="append",
        default=[],
        metavar="CONTAINER=HOST",
        help="map a container path of the run to this machine (e.g. /templateflow=...)",
    )


def cmd(args: argparse.Namespace) -> int:
    from larmorx_validation.stages.record import write_record

    binds = dict(b.split("=", 1) for b in args.bind)
    run = OracleRun(Path(args.oracle), binds={k: Path(v) for k, v in binds.items()})
    compare = Path(args.compare) if args.compare else _default_compare(run)
    compare.mkdir(parents=True, exist_ok=True)
    s = Settings(
        run=run,
        compare=compare,
        container=not args.no_container and run.image is not None,
        ants_oracle=not args.no_ants_oracle,
        reuse=args.reuse,
        threads=args.threads,
        select=tuple(args.stage),
        log=lambda text: print(text, file=sys.stderr, flush=True),
    )
    if args.render:
        import json

        if not args.out:
            raise SystemExit("--render needs --out (where <name>.json is)")
        payload = json.loads((Path(args.out) / f"{args.name}.json").read_text(encoding="utf-8"))
        payload = {k: v for k, v in payload.items() if k not in _RECORD_KEYS}
    else:
        payload = run_stages(s)
    if args.out:
        md, js = write_record(s, payload, Path(args.out), args.name)
        print(f"report: {md}\n        {js}", file=sys.stderr)
    statuses = [r.get("status") for r in payload["results"]]
    bad = [st for st in statuses if st in ("unexplained", "larmorx-error", "harness-error")]
    return 1 if bad else 0


#: Keys :func:`~larmorx_validation.stages.record.write_record` adds to the payload.
_RECORD_KEYS = ("record", "generated", "command", "oracle", "stages")


if __name__ == "__main__":
    sys.exit(main())
