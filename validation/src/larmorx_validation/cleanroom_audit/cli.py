# SPDX-License-Identifier: Apache-2.0
"""``python -m larmorx_validation cleanroom-audit scan``: every transcript read, every agent's and
the orchestrator's access log written (private: it holds absolute paths and the account name).

    cleanroom-audit scan --out <private dir> [--root DIR ...] [--jobs 12]

Without ``--root`` it reads the archived transcripts (``<workspace>/archive/*session-transcripts*/
main-machine`` and ``old-laptop``) and the live directory (``~/.claude/projects/<workspace>``).
It writes ``summary.json`` (the counts and the parsing gaps), ``agents.json`` (each agent: its
launch, worktree, brief, transcripts, commits), ``manifest.json`` (every transcript with its
SHA-256, checked against the archive's ``SHA256SUMS``), ``commits.jsonl`` and
``access/<session>__<agent>.jsonl.gz`` (one line per access: when, which record, what, how
classified). Run it in a memory box (``scripts/orchestration/boxed.sh``): it reads gigabytes.
"""

from __future__ import annotations

import argparse
import collections
import gzip
import json
import multiprocessing
import os
import re
import sys
import time
from dataclasses import asdict
from pathlib import Path

from larmorx_validation.cleanroom_audit import restrictions as rs
from larmorx_validation.cleanroom_audit import transcripts as tr

HERE = Path(__file__).resolve()
REPO = HERE.parents[4]
SCRATCH_TASK = re.compile(
    r"/tmp/claude-\d+/[^/]+/[^/]+/scratchpad/([^/\s'\"]+)/(?:BRIEF|NOTES)\.md"
)


def default_workspace() -> Path:
    s = str(REPO)
    if "/larmorx/.claude/worktrees/" in s:
        return Path(s.split("/larmorx/.claude/worktrees/")[0])
    return REPO.parent


def default_context(workspace: Path, home: Path) -> rs.Context:
    fsldirs = []
    env = os.environ.get("FSLDIR")
    if env:
        fsldirs.append(env.rstrip("/"))
    for d in sorted(home.glob("fsl*")):
        if d.is_dir() and re.fullmatch(r"fsl(-[0-9.]+)?", d.name) and str(d) not in fsldirs:
            fsldirs.append(str(d))
    return rs.Context(str(workspace), str(home), tuple(fsldirs))


def default_roots(workspace: Path, home: Path) -> list[Path]:
    roots: list[Path] = []
    for arch in sorted((workspace / "archive").glob("*session-transcripts*")):
        for sub in sorted(arch.iterdir()):
            if sub.is_dir():
                roots.append(sub)
    live = home / ".claude" / "projects" / re.sub(r"[^A-Za-z0-9]", "-", str(workspace))
    if live.is_dir():
        roots.append(live)
    return roots


def add_arguments(p: argparse.ArgumentParser) -> None:
    sub = p.add_subparsers(dest="audit_command", required=True)
    s = sub.add_parser("scan", help="read every transcript and write the access logs")
    s.add_argument("--out", required=True, help="a private directory (never committed)")
    s.add_argument("--root", action="append", help="a transcript root (repeatable)")
    s.add_argument("--workspace", help="the workspace (default: this checkout's)")
    s.add_argument("--home", help="the home directory (default: the user's)")
    s.add_argument("--jobs", type=int, default=12, help="parallel readers (spawned processes)")


def main(args: argparse.Namespace) -> int:
    if args.audit_command == "scan":
        return scan(args)
    return 2


def scan(args: argparse.Namespace) -> int:
    t0 = time.time()
    workspace = Path(args.workspace) if args.workspace else default_workspace()
    home = Path(args.home) if args.home else Path.home()
    ctx = default_context(workspace, home)
    roots = [Path(r) for r in args.root] if args.root else default_roots(workspace, home)
    files = tr.discover(roots)
    files.sort(key=lambda f: -Path(f.path).stat().st_size)
    print(f"cleanroom-audit: {len(files)} transcripts under {len(roots)} roots", flush=True)
    jobs = [(asdict(f), asdict(ctx), [str(r) for r in roots]) for f in files]
    results: list[tr.FileResult] = []
    n = max(1, min(args.jobs, len(jobs)))
    if n == 1:
        results = [tr.parse_file_job(j) for j in jobs]
    else:
        with multiprocessing.get_context("spawn").Pool(n) as pool:
            for k, fr in enumerate(pool.imap_unordered(tr.parse_file_job, jobs), 1):
                results.append(fr)
                if k % 50 == 0:
                    print(f"  {k}/{len(jobs)} read ({time.time() - t0:.0f} s)", flush=True)
    out = Path(args.out)
    summary = write_outputs(results, roots, ctx, out)
    summary["seconds"] = round(time.time() - t0, 1)
    (out / "summary.json").write_text(json.dumps(summary, indent=1, sort_keys=True) + "\n")
    print(
        json.dumps(
            {
                k: summary[k]
                for k in ("sessions", "agents", "tool_calls", "accesses", "unique_paths", "seconds")
            }
        )
    )
    return 0


def checksums(roots: list[Path]) -> dict[str, str]:
    """The archive's own SHA-256 list: absolute path -> hash."""
    out: dict[str, str] = {}
    for root in roots:
        sums = root.parent / "SHA256SUMS"
        if sums.is_file():
            for line in sums.read_text(encoding="utf-8").splitlines():
                h, _, rel = line.partition("  ")
                if rel:
                    out[str((sums.parent / rel).resolve())] = h.strip()
    return out


def write_outputs(results: list[tr.FileResult], roots, ctx: rs.Context, out: Path) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    (out / "access").mkdir(exist_ok=True)
    sums = checksums(roots)
    by_key: dict[str, list[tr.FileResult]] = collections.defaultdict(list)
    for fr in results:
        by_key[fr.file.key].append(fr)
    manifest = []
    for fr in sorted(results, key=lambda f: f.file.path):
        expect = sums.get(str(Path(fr.file.path).resolve()))
        manifest.append(
            {
                "path": fr.file.path,
                "root": fr.file.root,
                "key": fr.file.key,
                "sha256": fr.sha256,
                "bytes": fr.size,
                "records": fr.records,
                "archive_sha256": expect,
                "matches_archive": None if expect is None else expect == fr.sha256,
            }
        )
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1) + "\n")

    gaps = collections.Counter()
    gap_examples: dict[str, list] = collections.defaultdict(list)
    tools = collections.Counter()
    ops = collections.Counter()
    cats = collections.Counter()
    rules = collections.Counter()
    rules_named = collections.Counter()
    paths: set[str] = set()
    agents: dict[str, dict] = {}
    all_commits = []
    spawned: dict[str, str] = {}
    launched_by: dict[str, str] = {}
    n_access = 0
    n_calls = 0
    for key, copies in sorted(by_key.items()):
        copies.sort(key=lambda f: (-len(f.calls), f.file.path))
        calls: dict[str, tr.Call] = {}
        for fr in copies:
            for c in fr.calls:
                prev = calls.get(c.id)
                if prev is None or (not prev.has_result and c.has_result):
                    calls[c.id] = c
        best = copies[0]
        meta = next((f.meta for f in copies if f.meta), {})
        brief = next((f.brief for f in copies if f.brief), "")
        session, agent = key.split("/", 1)
        is_main = agent == "main"
        written_tasks = set()
        for c in calls.values():
            for a in c.accesses:
                if a["op"] in rs.WRITE_OPS:
                    m = rs.SCRATCH.match(a["path"])
                    if m and m["task"]:
                        written_tasks.add(m["task"])
        own_tasks = written_tasks | set(SCRATCH_TASK.findall(brief))
        lines_out = []
        commits = []
        for c in sorted(calls.values(), key=lambda c: (c.ts, c.line)):
            n_calls += 1
            tools[c.tool] += 1
            if c.refused:
                gaps["refused_calls"] += 1
            if c.unparsed:
                gaps["unparsed_commands"] += 1
                if len(gap_examples["unparsed_commands"]) < 20:
                    gap_examples["unparsed_commands"].append(
                        {"key": key, "line": c.line, "error": c.unparsed[0]}
                    )
            if c.persisted == "missing":
                gaps["persisted_output_missing"] += 1
            if c.spawn:
                aid = c.spawn.get("agent")
                if aid:
                    spawned[f"{session}/{aid}"] = c.id
                launched_by[c.id] = key
            for k in c.commits:
                commits.append({**k, "key": key, "ts": c.ts, "line": c.line, "call": c.id})
            for a in c.accesses:
                rec = classify_access(a, ctx, c.ts, own_tasks, is_main)
                rec["refused"] = c.refused
                n_access += 1
                ops[a["op"]] += 1
                cats[rec["category"]] += 1
                paths.add(a["path"])
                if rec.get("rule") and rec["category"].startswith("restricted"):
                    if c.refused:
                        gaps["refused_calls_restricted_accesses"] += 1
                    else:
                        rules[rec["rule"]] += 1
                        if rec["named"]:
                            rules_named[rec["rule"]] += 1
                if rec["unresolved"]:
                    gaps["unresolved_paths"] += 1
                rec.update(
                    {
                        "ts": c.ts,
                        "file": best.file.path,
                        "line": c.line,
                        "call": c.id,
                        "tool": c.tool,
                    }
                )
                if rec["category"].startswith("restricted") or rec["category"] in (
                    "register",
                    "findings-unlisted",
                    "difftest-cases",
                ):
                    rec["command"] = c.command[:20_000] if c.command else ""
                lines_out.append(rec)
        name = f"{session}__{agent}.jsonl.gz"
        with gzip.open(out / "access" / name, "wt", encoding="utf-8") as fh:
            for rec in lines_out:
                fh.write(json.dumps(rec, sort_keys=True) + "\n")
        all_commits.extend(commits)
        agents[key] = {
            "session": session,
            "agent": agent,
            "role_hint": "orchestrator" if is_main else "",
            "meta": meta,
            "brief": brief,
            "transcripts": [f.file.path for f in copies],
            "first_ts": min((f.first_ts for f in copies if f.first_ts), default=""),
            "last_ts": max((f.last_ts for f in copies if f.last_ts), default=""),
            "records": max(f.records for f in copies),
            "calls": len(calls),
            "accesses": len(lines_out),
            "commits": len(commits),
            "own_scratch": sorted(own_tasks),
            "compactions": max(f.compactions for f in copies),
            "continued": any(f.continued for f in copies),
            "versions": sorted({v for f in copies for v in f.versions}),
            "access_log": f"access/{name}",
        }
        for f in copies:
            gaps["calls_without_result"] += f.calls_without_result
            gaps["results_without_call"] += f.results_without_call
            gaps["request_only_calls"] += f.request_only_calls
            gaps["bad_lines"] += f.bad_lines
            gaps["compaction_parent_missing"] += f.compaction_parent_missing
            for t, k in f.unknown_tools.items():
                gaps[f"unknown_tool:{t}"] += k
        gaps["compactions"] += agents[key]["compactions"]
        if agents[key]["continued"]:
            gaps["continued_sessions"] += 1
            gap_examples["continued_sessions"].append(key)
        if not is_main and not meta:
            gaps["agents_without_meta"] += 1
            gap_examples["agents_without_meta"].append(key)
        if not is_main and not meta.get("worktreePath"):
            gaps["agents_without_worktree"] += 1
    # Every launch has its agent's transcript, every agent its launch.
    for key, info in agents.items():
        if info["agent"] == "main":
            continue
        tid = info["meta"].get("toolUseId")
        info["launched_by_call"] = tid
        info["launched_by"] = launched_by.get(tid, "")
        if tid and tid not in launched_by:
            gaps["agents_whose_launch_is_missing"] += 1
            gap_examples["agents_whose_launch_is_missing"].append(key)
    for k in spawned:
        if k not in agents:
            gaps["launches_whose_transcript_is_missing"] += 1
            gap_examples["launches_whose_transcript_is_missing"].append(k)
    (out / "agents.json").write_text(json.dumps(agents, indent=1, sort_keys=True) + "\n")
    with (out / "commits.jsonl").open("w", encoding="utf-8") as fh:
        for k in all_commits:
            fh.write(json.dumps(k, sort_keys=True) + "\n")
    sessions = sorted({k.split("/")[0] for k in agents})
    return {
        "roots": [str(r) for r in roots],
        "transcript_files": len(results),
        "distinct_transcripts": len(by_key),
        "transcript_bytes": sum(f.size for f in results),
        "archive_checksums_matched": sum(1 for m in manifest if m["matches_archive"]),
        "archive_checksums_differ": sum(1 for m in manifest if m["matches_archive"] is False),
        "sessions": len(sessions),
        "session_ranges": {
            s: [agents[f"{s}/main"]["first_ts"], agents[f"{s}/main"]["last_ts"]]
            for s in sessions
            if f"{s}/main" in agents
        },
        "agents": sum(1 for k in agents if not k.endswith("/main")),
        "tool_calls": n_calls,
        "tool_calls_by_tool": dict(tools.most_common()),
        "accesses": n_access,
        "accesses_by_op": dict(ops.most_common()),
        "accesses_by_category": dict(cats.most_common()),
        "restricted_by_rule": dict(sorted(rules.items())),
        "restricted_by_rule_after_named": dict(sorted(rules_named.items())),
        "unique_paths": len(paths),
        "commits_printed": len(all_commits),
        "gaps": dict(sorted(gaps.items())),
        "gap_examples": {k: v[:20] for k, v in gap_examples.items()},
    }


def classify_access(a: dict, ctx: rs.Context, ts: str, own_tasks: set[str], is_main: bool) -> dict:
    """An access with its location, category, rule, family and whether the rule was in force."""
    loc = rs.locate(a["path"], ctx, unresolved=a.get("unresolved", False))
    if a["op"] == "exec" and a.get("hint"):
        cls = rs.Class("run", None, a["hint"])
    else:
        cls = rs.classify(loc, a["op"])
    other = loc.checkout and loc.checkout not in own_tasks
    if loc.area == "scratch" and cls.category == "scratch" and not is_main and other:
        cls = rs.restricted("R18", "all", a["op"], loc.rel)
    content, named = rs.in_force(cls, loc.rel, ts) if ts else (False, False)
    rec = {
        "op": a["op"],
        "via": a["via"],
        "detail": a.get("detail", ""),
        "path": a["path"],
        "area": loc.area,
        "rel": loc.rel,
        "checkout": loc.checkout,
        "show": loc.show(),
        "category": cls.category,
        "rule": cls.rule,
        "family": cls.family,
        "in_force": content,
        "named": named,
        "unresolved": bool(a.get("unresolved")),
    }
    if a.get("lines"):
        rec["lines"] = a["lines"]
    if a.get("hits"):
        rec["hits"] = a["hits"][:500]
    if cls.rule == "R13":
        m = re.search(r"docs/parts/([^/]+)", a["path"])
        rec["part"] = m[1] if m else ""
    return rec


if __name__ == "__main__":  # pragma: no cover
    ns = argparse.ArgumentParser()
    add_arguments(ns)
    sys.exit(main(ns.parse_args()))
