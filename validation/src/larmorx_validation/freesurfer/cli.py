# SPDX-License-Identifier: Apache-2.0
"""``python -m larmorx_validation freesurfer-replay``: list a recording's steps, rebuild the
subject directory before a step, replay steps and compare them, compare two FreeSurfer files.

    freesurfer-replay steps <run>
    freesurfer-replay rebuild <run> <step> <dest> [--threads N|recorded]
    freesurfer-replay run <run> <step>... [--threads N|recorded] [--command PREFIX] [--json FILE]
    freesurfer-replay compare <reference> <replay> [--pair OLD=NEW ...]

``<run>`` is a recording's directory, or its name under
``<workspace>/oracles/freesurfer-8.2.0/runs/``; ``<step>`` a number (``11``) or a program with its
occurrence (``mri_normalize``, ``mri_label2label:3``). ``run`` exits with 0 when every step was
reproduced (its files the same, provenance aside, and its exit status), 1 otherwise.
"""

from __future__ import annotations

import argparse
import json
import shlex
import sys
from pathlib import Path

from larmorx_validation.oracle_cache.store import workspace


def _recording(text: str):
    from larmorx_validation.freesurfer.recording import Recording

    p = Path(text)
    if not p.is_dir():
        ws = workspace()
        if ws is not None:
            p = ws / "oracles" / "freesurfer-8.2.0" / "runs" / text
    return Recording(p)


def _threads(text: str) -> int | None:
    if text == "recorded":
        return None
    n = int(text)
    if n < 1:
        raise argparse.ArgumentTypeError("--threads: a positive number or 'recorded'")
    return n


def add_arguments(p: argparse.ArgumentParser) -> None:
    sub = p.add_subparsers(dest="fs_command", required=True)
    s = sub.add_parser("steps", help="a recording's steps: number, program, time, outputs")
    s.add_argument("run")
    r = sub.add_parser("rebuild", help="the subject directory before a step, and its command")
    r.add_argument("run")
    r.add_argument("step")
    r.add_argument("dest", type=Path)
    r.add_argument(
        "--threads", type=_threads, default=1, help="a number, or 'recorded' (default 1)"
    )
    x = sub.add_parser("run", help="replay steps and compare their files with the recorded ones")
    x.add_argument("run")
    x.add_argument("steps", nargs="+")
    x.add_argument(
        "--threads", type=_threads, default=1, help="a number, or 'recorded' (default 1)"
    )
    x.add_argument(
        "--command",
        help="run this command line instead of FreeSurfer's program (uncached; '{program}' in it "
        "is the step's program, else the step's command line follows it)",
    )
    x.add_argument("--timeout", type=float, default=None, help="seconds per step")
    x.add_argument("--json", type=Path, help="write the reports here (one JSON list)")
    x.add_argument("--work", type=Path, help="the replays' root (default <base>/work/<run>/...)")
    x.add_argument(
        "--compare-only",
        action="store_true",
        help="compare replays made before again, without rebuilding or running",
    )
    c = sub.add_parser("compare", help="compare two FreeSurfer files, provenance aside")
    c.add_argument("reference", type=Path)
    c.add_argument("replay", type=Path)
    c.add_argument("--pair", action="append", default=[], help="OLD=NEW: texts naming one place")
    c.add_argument("--name", default="", help="the file's name, when the reference is a blob")


def _print_report(report) -> None:
    print(
        f"{report.run} {report.step} ({report.stream}, {report.threads} thread"
        f"{'s' if report.threads != 1 else ''}): {report.verdict.upper()}; exit "
        f"{report.exit_recorded} recorded, {report.exit_replayed} replayed; "
        f"{report.seconds_recorded:.1f} s recorded, {report.seconds_replayed:.1f} s now"
        f"{' (served by the oracle cache)' if report.served else ''}"
    )
    for f in report.files:
        name = (
            f.path
            if f.path == f.replay_path or not f.replay_path
            else f"{f.path} | {f.replay_path}"
        )
        detail = f": {f.comparison.detail}" if f.comparison.detail else ""
        print(f"  {f.comparison.verdict:10} {f.comparison.kind:8} {name or f.replay_path}{detail}")
    if report.note:
        print(f"  note: {report.note.strip()[-400:]}")


def main(args: argparse.Namespace) -> int:
    from larmorx_validation.freesurfer import files as fsfiles
    from larmorx_validation.freesurfer import replay as rp

    if args.fs_command == "compare":
        pairs = tuple(tuple(x.split("=", 1)) for x in args.pair)
        c = fsfiles.compare(
            args.reference, args.replay, fsfiles.Context(pairs=pairs), name=args.name
        )
        print(f"{c.verdict} ({c.kind}){': ' + c.detail if c.detail else ''}")
        return 0 if c.same else 1
    rec = _recording(args.run)
    if args.fs_command == "steps":
        for s in rec.steps:
            print(f"{s.index:4d} {s.program:30} {s.seconds:9.1f} s  exit {s.exit}  "
                  f"{len(s.outputs)} outputs  {len(s.between)} between")  # fmt: skip
        return 0
    if args.fs_command == "rebuild":
        rb = rp.rebuild(rec, rec.select(args.step), args.dest, threads=args.threads)
        print(
            f"rebuilt {rb.files} files ({rb.bytes / 2**20:.1f} MiB), {len(rb.links)} links: {rb.subject_dir}"
        )
        print(f"cd {shlex.quote(str(rb.cwd))}")
        print(" ".join(shlex.quote(a) for a in rb.argv))
        return 0
    reports = []
    for text in args.steps:
        step = rec.select(text)
        dest = None
        if args.work is not None:
            t = "recorded" if args.threads is None else f"t{args.threads}"
            dest = args.work / rec.name / f"{step.label}-{t}"
        command = shlex.split(args.command) if args.command else None
        report, _rb = rp.replay(rec, step, threads=args.threads, dest=dest, command=command,
                                timeout=args.timeout, compare_only=args.compare_only)  # fmt: skip
        _print_report(report)
        sys.stdout.flush()
        reports.append(report)
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(
            json.dumps([r.to_dict() for r in reports], indent=1) + "\n", encoding="utf-8"
        )
    bad = [r for r in reports if r.verdict == "differs"]
    print(f"REPLAY {len(reports) - len(bad)} of {len(reports)} reproduced")
    return 1 if bad else 0


__all__ = ["add_arguments", "main"]
