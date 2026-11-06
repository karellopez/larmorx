# SPDX-License-Identifier: Apache-2.0
"""The clean-room audit's public summary (``docs/validation/clean-room-audit.md``) and the merge
step's gate.

The summary is made in two steps, so the published page can be regenerated and checked without
the private dossier:

1. :func:`extract` reads one private dossier (``<workspace>/archive/clean-room-audit/<date>/``: the
   scan, the verdicts and a similarity record) and the human judgements
   (``validation/cleanroom-audit/judgements.toml``) and keeps the public facts only: counts, dates,
   repository-relative paths, the agents' launch descriptions and harness ids, the judgements'
   words. They are written to ``docs/validation/clean-room-audit.json``; :func:`privacy_problems`
   refuses any absolute path, account name, host name, session id or tool-call id.
2. :func:`render` turns those facts into the page. ``cleanroom-audit summary --check`` renders the
   committed JSON and compares it with the committed page (a test does the same).

:func:`gate` answers the merge step's question from a verdicts record: was a clean-room original
edited by an agent after it had seen restricted material of that tool's family, in a way the
judgements do not cover? Blocking items stop the merge; items of other kinds are listed for the
orchestrator to judge.

    cleanroom-audit summary --dossier <dir> [--scan D] [--verdicts D] [--similarity D]
                            [--judgements F] [--json F] [--out F] [--verbatim F]
    cleanroom-audit summary [--check]          (from the committed JSON)
    cleanroom-audit gate --verdicts <dir> [--judgements F] [--strict]
"""

from __future__ import annotations

import argparse
import collections
import contextlib
import getpass
import hashlib
import json
import re
import socket
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

from larmorx_validation.cleanroom_audit import restrictions as rs
from larmorx_validation.cleanroom_audit import similarity as sm
from larmorx_validation.cleanroom_audit import verdicts as vd

REPO = Path(__file__).resolve().parents[4]
PUBLIC_JSON = "docs/validation/clean-room-audit.json"
PUBLIC_MD = "docs/validation/clean-room-audit.md"
JUDGEMENTS = "validation/cleanroom-audit/judgements.toml"
SCHEMA = 1
IMPL = "clean-room implementer"

# --- the judgements -----------------------------------------------------------------------------


@dataclass
class Judgements:
    """The human judgements: accesses by (agent id, record line), edits by (agent, short commit)."""

    access: dict[tuple[str, int], dict] = field(default_factory=dict)
    edit: dict[tuple[str, str], dict] = field(default_factory=dict)
    verdict: dict[str, str] = field(default_factory=dict)
    limits: list[dict] = field(default_factory=list)
    recommendations: list[str] = field(default_factory=list)


def load_judgements(path: Path) -> Judgements:
    if not path.is_file():
        return Judgements()
    d = tomllib.loads(path.read_text(encoding="utf-8"))
    j = Judgements()
    for a in d.get("access", []):
        j.access[(a["agent"], int(a["line"]))] = a
    for e in d.get("edit", []):
        j.edit[(e["agent"], e.get("commit", "")[:9])] = e
    j.verdict = dict(d.get("verdict", {}))
    j.limits = list(d.get("limit", []))
    j.recommendations = [r["text"] for r in d.get("recommendation", [])]
    return j


def agent_id(key: str) -> str:
    """The harness's agent id of a scan key (``<session>/agent-<id>``); the orchestrator's sessions
    are all "orchestrator"."""
    a = key.split("/", 1)[-1]
    return "orchestrator" if a == "main" else a.removeprefix("agent-")


# --- times --------------------------------------------------------------------------------------


def _utc(ts: str) -> str:
    """An ISO time as 'YYYY-MM-DD HH:MM UTC'."""
    from datetime import UTC, datetime

    t = datetime.fromisoformat(ts.replace("Z", "+00:00")).astimezone(UTC)
    return t.strftime("%Y-%m-%d %H:%M UTC")


def _day(ts: str) -> str:
    return _utc(ts)[:10]


# --- privacy ------------------------------------------------------------------------------------

#: What a public file must never hold (besides this machine's account and host names).
PRIVATE = (
    (re.compile(r"(?<![\w.$<>~-])/(?:home|Users|tmp|root|mnt|media|private)/"), "an absolute path"),
    (re.compile(r"[A-Za-z]:\\\\?[A-Za-z]"), "a Windows path"),
    (re.compile(r"~/\.claude|\.claude/projects"), "the transcripts' folder"),
    (re.compile(r"\.jsonl\b"), "a transcript file name"),
    (re.compile(r"\btoolu_[0-9A-Za-z]+"), "a tool-call id"),
    (
        re.compile(r"\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b"),
        "a session id",
    ),
    (re.compile(r"[\w.+-]+@[\w-]+\.[A-Za-z]{2,}"), "an e-mail address"),
)


def _local_names() -> list[str]:
    names = set()
    with contextlib.suppress(Exception):  # no user database
        names.add(getpass.getuser())
    names.add(Path.home().name)
    host = socket.gethostname()
    names |= {host, host.split(".")[0]}
    return sorted(n for n in names if n and len(n) >= 3)


def privacy_problems(text: str, names: list[str] | None = None) -> list[str]:
    """Every private string in a text meant for publication: (what, the matching text)."""
    out = []
    for rx, what in PRIVATE:
        for m in rx.finditer(text):
            out.append(f"{what}: {m[0]!r}")
    for n in _local_names() if names is None else names:
        for m in re.finditer(rf"(?<![\w-]){re.escape(n)}(?![\w-])", text):
            out.append(f"a local account or host name: {m[0]!r}")
    return out


# --- categories ---------------------------------------------------------------------------------

#: The public groups of the scan's categories (an implementer's accesses, by what they reached).
GROUPS = (
    ("the behaviour specs (specs/)", ("spec",)),
    ("the clean-room code", ("cleanroom",)),
    ("larmorx's other files (records, documents, tests, permissive ports)", ("repo",)),
    ("the documentation site and the test-data catalog", ("repo-site", "repo-testdata")),
    ("the defect register (its restricted lines are counted below)", ("register",)),
    ("random-testing permanent cases", ("difftest-cases",)),
    ("findings files the rules do not list", ("findings-unlisted",)),
    ("the original programs, run or named as black boxes", ("run", "fsl-bin")),
    (
        "the originals' installations besides their source (data, packages)",
        ("fsl-install", "freesurfer-install", "data"),
    ),
    ("upstream documentation", ("docs",)),
    ("oracle outputs and inputs", ("oracle",)),
    (
        "permissive upstream source (ANTs, ITK, nibabel, CORE-MATH, numpy, SciPy, ...)",
        ("permissive-source", "probe-permissive"),
    ),
    (
        "the agent's own scratch notes, task outputs and memory",
        ("scratch", "task-output", "tool-output", "memory", "own-transcript"),
    ),
    (
        "the system, the home directory, the workspace and the archive",
        ("system", "home", "workspace", "archive"),
    ),
    ("the web", ("web",)),
)
RESTRICTED_GROUP = "restricted material (by rule and kind below)"
SEEN_OPS = frozenset(
    {"read", "search", "search-hit", "search-match", "list", "stat", "copy", "diff-shown",
     "script-path", "diagnostic", "web"}
)  # fmt: skip
RUN_OPS = frozenset({"exec", "build", "mention"})
WRITE_OPS = frozenset({"write", "edit", "delete", "move"})


def group_of(category: str) -> str:
    if category.startswith("restricted") or category.startswith("replica-"):
        return RESTRICTED_GROUP
    for label, cats in GROUPS:
        if category in cats:
            return label
    return "other"


def kind_of(op: str) -> str:
    if op in WRITE_OPS:
        return "written"
    if op in RUN_OPS:
        return "run"
    return "seen"


#: How the register's restricted lines weigh (R12).
REGISTER_LEVELS = {
    "headings": "titles of restricted defect entries only (the register's open table describes every entry)",
    "body lines": "lines of a restricted section's text",
}


def scope_of(r: dict, family: str) -> str:
    """Whose material a judged restricted access was, from the tool's point of view."""
    if r.get("level") == "clean-writer":
        return "written by clean-room implementers only"
    if r["rule"] in ("R13", "R18"):
        roles = r.get("writer_roles") or []
        if r.get("part_kind") == "clean-room" or (
            roles and all(x in ("clean-room implementer", "orchestrator", "audit") for x in roles)
        ):
            return "written by clean-room implementers only"
    if r.get("relevant"):
        return "this family"
    fams = set(re.split(r"[/+]", r.get("family") or ""))
    if family in fams or "all" in fams or r["rule"] in ("R13", "R17", "R18"):
        return "before the rule applied"
    return "another family"


SCOPE_ORDER = (
    "this family",
    "before the rule applied",
    "written by clean-room implementers only",
    "another family",
)
LEVEL_ORDER = (*vd.LEVELS, *REGISTER_LEVELS)


# --- extraction ---------------------------------------------------------------------------------


def find_dossier_parts(dossier: Path) -> tuple[Path, Path, Path]:
    """The scan, verdicts and similarity folders of a dossier (the newest similarity record of
    any dossier when this one has none)."""
    scans = sorted(p for p in dossier.glob("scan*") if (p / "agents.json").is_file())
    if not scans:
        raise SystemExit(f"cleanroom-audit summary: no scan in {dossier}")
    verdicts = dossier / "verdicts"
    sim = dossier / "similarity"
    if not (sim / "similarity.json").is_file():
        cands = sorted(dossier.parent.glob("*/similarity/similarity.json"))
        if not cands:
            raise SystemExit("cleanroom-audit summary: no similarity record")
        sim = cands[-1].parent
    return scans[-1], verdicts, sim


def _verdicts_head(verdicts_dir: Path) -> str:
    md = verdicts_dir / "verdicts.md"
    m = (
        re.search(r"repository at `([0-9a-f]+)`", md.read_text(encoding="utf-8"))
        if md.is_file()
        else None
    )
    return m[1] if m else ""


def prune_similarity(res: dict) -> dict:
    """The part of a similarity record the public section uses (public file names only)."""

    def run(lr: dict | None) -> dict:
        if not lr:
            return {}
        out = {"tokens": lr.get("tokens", 0)}
        for side in ("ours", "theirs"):
            if side in lr:
                out[side] = {
                    "file": sm.public_name(lr[side]["file"]),
                    "lines": list(lr[side]["lines"]),
                }
        return out

    def cell(c: dict | None, full: bool = False) -> dict | None:
        if not c:
            return None
        nm = c["names"]
        out = {
            "identifiers_shared": c.get("identifiers_shared"),
            "names": {"longest_run": run(nm.get("longest_run"))},
        }
        for k in ("coverage_8", "coverage_16") if full else ("coverage_8",):
            if k in nm:
                out["names"][k] = nm[k]
        return out

    tools = {}
    for k, e in res["tools"].items():
        tools[k] = {
            "name": e["name"],
            "family": e["family"],
            "tool": e["tool"],
            "cleanroom": {"tokens": e["cleanroom"]["tokens"], "files": e["cleanroom"].get("files")},
            "cleanroom_vs_upstream": cell(e.get("cleanroom_vs_upstream")),
            "cleanroom_vs_replica": cell(e.get("cleanroom_vs_replica")),
            "replica_vs_upstream": cell(e.get("replica_vs_upstream")),
        }
    matrices = {
        fam: {
            k: m[k]
            for k in (
                "tools",
                "cleanroom_vs_upstream_floors",
                "replica_vs_upstream_floors",
                "cleanroom_vs_replica_floors",
            )
        }
        for fam, m in res["matrices"].items()
    }
    calibration = [
        {
            "name": c["name"],
            "kind": c["kind"],
            "tokens": c["tokens"],
            "identifiers_shared": c["identifiers_shared"],
            "names": {
                "coverage_8": c["names"]["coverage_8"],
                "coverage_16": c["names"]["coverage_16"],
                "longest_run": {"tokens": c["names"]["longest_run"]["tokens"]},
            },
        }
        for c in res["calibration"]
    ]
    wide = {}
    for fam, w in (res.get("wide") or {}).items():
        wide[fam] = {
            "files": w["files"],
            "tokens": w["tokens"],
            "floor": {
                k: {"names": {"coverage_8": c["names"]["coverage_8"]}}
                for k, c in w.get("floor", {}).items()
            },
            "tools": {k: cell(c) for k, c in w["tools"].items()},
            "replicas": {
                k: {"names": {"coverage_8": c["names"]["coverage_8"]}}
                for k, c in w.get("replicas", {}).items()
            },
        }
    out = {
        "commit": res.get("commit", ""),
        "method": res.get("method", {}),
        "tools": tools,
        "matrices": matrices,
        "calibration": calibration,
    }
    if wide:
        out["wide"] = wide
    return out


def _floor(f: dict) -> dict:
    return {
        "median": f.get("median_coverage_8"),
        "max": f.get("max_coverage_8"),
        "max_run": f.get("max_longest_run"),
    }


def tool_similarity(sim: dict, key: str) -> dict | None:
    """One tool's similarity figures from the matrices (its diagonal cells and floors)."""
    fam = key.split("/", 1)[0]
    m = sim["matrices"].get(fam)
    e = sim["tools"].get(key)
    if not m or not e or e["tool"] not in m["tools"]:
        return None
    i = m["tools"].index(e["tool"])
    cu = m["cleanroom_vs_upstream_floors"][i]
    ru = m["replica_vs_upstream_floors"][i]
    cr = m["cleanroom_vs_replica_floors"][i]
    out = {
        "tokens": e["cleanroom"]["tokens"],
        "files": e["cleanroom"].get("files"),
        "upstream": cu.get("diagonal"),
        "upstream_floor": _floor(cu),
        "replica_upstream": ru.get("diagonal"),
        "replica": cr.get("diagonal"),
        "replica_floor": _floor(cr),
    }
    w = (sim.get("wide") or {}).get(fam)
    if w and key in w["tools"]:
        c = w["tools"][key]
        fl = [x["names"]["coverage_8"] for x in w.get("floor", {}).values()]
        rep = w.get("replicas", {}).get(key)
        out["tree"] = {
            "family": fam,
            "coverage_8": c["names"].get("coverage_8"),
            "longest_run": c["names"]["longest_run"].get("tokens"),
            "floor_max": max(fl) if fl else None,
            "replica": rep["names"]["coverage_8"] if rep else None,
        }
    return out


def _counter_rows(c: collections.Counter) -> list[list]:
    return [[k, n] for k, n in sorted(c.items())]


def extract(scan: Path, verdicts_dir: Path, similarity_dir: Path, judgements: Judgements) -> dict:
    """The public facts of one dossier."""
    summary = json.loads((scan / "summary.json").read_text(encoding="utf-8"))
    manifest = json.loads((scan / "manifest.json").read_text(encoding="utf-8"))
    agents = json.loads((scan / "agents.json").read_text(encoding="utf-8"))
    v = json.loads((verdicts_dir / "verdicts.json").read_text(encoding="utf-8"))
    sim_full = json.loads((similarity_dir / "similarity.json").read_text(encoding="utf-8"))
    sim = prune_similarity(sim_full)
    if sm.public_markdown(sim) != sm.public_markdown(sim_full):  # the pruning must lose nothing
        raise RuntimeError("prune_similarity dropped something the public section uses")
    roles = {k: r["role"] for k, r in v["roles"].items()}

    # The evidence: the transcripts and the archive's checksum lists.
    archives = {}
    for root in summary["roots"]:
        sums = Path(root).parent / "SHA256SUMS"
        if sums.is_file():
            data = sums.read_bytes()
            archives[Path(root).parent.name] = {
                "sha256": hashlib.sha256(data).hexdigest(),
                "files": sum(1 for x in data.splitlines() if x.strip()),
            }
    archived = sum(1 for m in manifest if m.get("archive_sha256"))
    ranges = [r for r in summary["session_ranges"].values() if r[0]]
    gaps = summary["gaps"]
    evidence = {
        "transcript_files": summary["transcript_files"],
        "distinct_transcripts": summary["distinct_transcripts"],
        "transcript_bytes": summary["transcript_bytes"],
        "archived_files": archived,
        "live_files": summary["transcript_files"] - archived,
        "archive_checksums_matched": summary["archive_checksums_matched"],
        "archive_checksums_differ": summary["archive_checksums_differ"],
        "archives": archives,
        "sessions": summary["sessions"],
        "first_record": _utc(min(r[0] for r in ranges)),
        "last_record": _utc(max(r[1] for r in ranges)),
        "agents": summary["agents"],
        "tool_calls": summary["tool_calls"],
        "tool_calls_by_tool": _counter_rows(collections.Counter(summary["tool_calls_by_tool"])),
        "accesses": summary["accesses"],
        "unique_paths": summary["unique_paths"],
        "commits_printed": summary["commits_printed"],
        "gaps": {
            k: gaps.get(k, 0)
            for k in (
                "bad_lines",
                "calls_without_result",
                "results_without_call",
                "request_only_calls",
                "compactions",
                "compaction_parent_missing",
                "refused_calls",
                "refused_calls_restricted_accesses",
                "unresolved_paths",
                "agents_without_worktree",
            )
        },
    }

    # The commits of clean-room code and their makers.
    commits = v["commits"]
    tool_commits: dict[str, set[str]] = collections.defaultdict(set)
    tool_commit_roles: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    by_role: collections.Counter = collections.Counter()
    for h, c in commits.items():
        role = roles.get(c.get("maker") or "", "unattributed")
        by_role[role] += 1
        for f in c["files"]:
            t = vd.tool_of(f["path"])
            if t and h not in tool_commits["/".join(t)]:
                tool_commits["/".join(t)].add(h)
                tool_commit_roles["/".join(t)][role] += 1
    overview = {
        "commits": len(commits),
        "attributed_by": _counter_rows(collections.Counter(c["how"] for c in commits.values())),
        "unattributed": len(v["unattributed"]),
        "commits_by_role": _counter_rows(by_role),
        "agents_by_role": _counter_rows(
            collections.Counter(r for k, r in roles.items() if not k.endswith("/main"))
        ),
        "implementers": sum(1 for k, r in roles.items() if r == IMPL),
        "register_restricted_lines": v["register_restricted_lines"],
        "single_file_scopes": {
            "checked": v["single_file_scopes"]["checked"],
            "shown": len(v["single_file_scopes"]["shown"]),
        },
    }

    # Each implementer's accesses (before or during its work on each tool), from the scan.
    logs: dict[str, list[tuple[float, str, str, bool, str]]] = {}

    def log_of(key: str):
        if key not in logs:
            rows = []
            info = agents.get(key)
            if info:
                for r in vd.access_log(scan, info):
                    if r.get("refused"):
                        continue
                    rows.append(
                        (
                            vd.epoch(r["ts"]),
                            r["category"],
                            r["op"],
                            bool(r.get("unresolved")),
                            r.get("via", ""),
                        )
                    )
            logs[key] = rows
        return logs[key]

    tools = []
    for key, entry in sorted(v["tools"].items(), key=lambda kv: _tool_order(kv[0])):
        family = entry["family"]
        impl = [a for a in entry["agents"] if a["role"] == IMPL]
        others = [a for a in entry["agents"] if a["role"] != IMPL]
        acc: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
        cannot = collections.Counter()
        for a in impl:
            last = vd.epoch(a["last_touch"])
            for t, cat, op, unresolved, via in log_of(a["key"]):
                if t > last:
                    continue
                acc[group_of(cat)][kind_of(op)] += 1
                if op in ("exec", "build"):
                    cannot["program_runs"] += 1
                if op == "list" and via in ("find", "ls"):
                    cannot["listings"] += 1
                if unresolved:
                    cannot["unresolved"] += 1
            info = agents.get(a["key"], {})
            cannot["compactions"] += int(info.get("compactions") or 0)
            if not (info.get("meta") or {}).get("worktreePath"):
                cannot["without_worktree"] += 1
        labels = [g for g, _ in GROUPS] + [RESTRICTED_GROUP, "other"]
        accesses = [
            {"group": g, "seen": acc[g]["seen"], "run": acc[g]["run"], "written": acc[g]["written"]}
            for g in labels
            if sum(acc[g].values())
        ]
        # The restricted accesses, judged (verdicts), by rule, kind and whose material.
        rows: dict[tuple, dict] = {}
        for a in impl:
            aid = agent_id(a["key"])
            for r in a["restricted"]:
                k = (r["rule"], r["level"], scope_of(r, family))
                row = rows.setdefault(k, {"before": 0, "after": 0, "agents": set()})
                row["before" if r["when"] == "before or during" else "after"] += 1
                row["agents"].add(aid)
            for g in a["register"]:
                kind = "body lines" if g["body_lines"] else "headings"
                k = ("R12", kind, "this family" if g["relevant"] else "another family")
                row = rows.setdefault(k, {"before": 0, "after": 0, "agents": set()})
                row["before" if g["when"] == "before or during" else "after"] += 1
                row["agents"].add(aid)
        restricted = [
            {
                "rule": k[0],
                "level": k[1],
                "scope": k[2],
                "before": r["before"],
                "after": r["after"],
                "agents": len(r["agents"]),
            }
            for k, r in sorted(
                rows.items(),
                key=lambda kv: (
                    SCOPE_ORDER.index(kv[0][2]),
                    kv[0][0],
                    LEVEL_ORDER.index(kv[0][1]) if kv[0][1] in LEVEL_ORDER else 99,
                ),
            )
        ]
        findings = _findings(impl, judgements)
        status = entry["verdict"]["status"]
        if findings:  # the machine's "review": every finding read and judged, or not yet
            status = (
                "review"
                if any(not f["judged"] for f in findings)
                else "reviewed (every finding judged)"
            )
        tools.append(
            {
                "id": key,
                "family": family,
                "name": entry["name"],
                "first": _day(min(a["first_touch"] for a in entry["agents"])),
                "last": _day(max(a["last_touch"] for a in entry["agents"])),
                "commits": len(tool_commits.get(key, ())),
                "commits_by_role": _counter_rows(tool_commit_roles.get(key, collections.Counter())),
                "implementers": [
                    {
                        "agent": agent_id(a["key"]),
                        "description": a["description"],
                        "first": _day(a["first_touch"]),
                        "last": _day(a["last_touch"]),
                        "commits": len(a["commits"]),
                    }
                    for a in sorted(impl, key=lambda a: vd.epoch(a["first_touch"]))
                ],
                "other_roles": [
                    {
                        "agent": agent_id(a["key"]),
                        "description": a["description"],
                        "role": a["role"],
                        "commits": list(a["commits"]),
                        "touches": a["touches"],
                        "relevant_content_before": next(
                            (
                                o["relevant_content_before"]
                                for o in entry["verdict"]["other_roles"]
                                if o["description"] == a["description"]
                            ),
                            0,
                        ),
                    }
                    for a in others
                ],
                "accesses": accesses,
                "restricted": restricted,
                "findings": findings,
                "slip_agents": sorted({agent_id(a["key"]) for a in impl}),
                "verdict": {
                    "status": status,
                    "words": judgements.verdict.get(key, ""),
                },
                "similarity": tool_similarity(sim, key),
                "cannot_see": dict(sorted(cannot.items())),
            }
        )

    slips = _slips(v, judgements)
    for t in tools:
        t["slips"] = [i for i, s in enumerate(slips) if s["agent"] in t["slip_agents"]]
        del t["slip_agents"]
    edits = _edits(v, judgements)
    facts = {
        "schema": SCHEMA,
        "dossier": verdicts_dir.parent.name,
        "made_from": {
            "scan": scan.name,
            "verdicts_repository": _verdicts_head(verdicts_dir),
            "similarity_commit": sim.get("commit", ""),
        },
        "evidence": evidence,
        "rules": [
            {
                "id": r.id,
                "title": r.title,
                "family": r.family,
                "since": r.since[:10],
                "named": r.named[:10],
                "basis": r.basis,
            }
            for r in rs.RULES.values()
        ],
        "levels": [[k, t] for k, t in vd.LEVELS.items()],
        "register_levels": [[k, t] for k, t in REGISTER_LEVELS.items()],
        "overview": overview,
        "tools": tools,
        "slips": slips,
        "edits": edits,
        "limits": [{"title": x["title"], "text": x["text"]} for x in judgements.limits],
        "recommendations": list(judgements.recommendations),
        "similarity": sim,
    }
    facts = json.loads(json.dumps(facts, sort_keys=True, default=sorted))
    text = json.dumps(facts, indent=1, sort_keys=True)
    problems = privacy_problems(text)
    if problems:
        raise RuntimeError("private strings in the public facts: " + "; ".join(problems[:10]))
    return facts


def _tool_order(key: str) -> tuple:
    order = [
        "AFNI/tshift", "AFNI/automask", "AFNI/unifize", "AFNI/afni-shared", "FSL/hmc", "FSL/bet",
        "FSL/flirt", "FSL/fast", "FSL/topup", "FSL/fslmaths", "FSL/standard_space_roi",
        "FSL/fslroi", "FSL/fsl-shared", "GNU/cbrtf_glibc", "GNU/sincos_glibc", "GNU/tanh_glibc",
    ]  # fmt: skip
    return (order.index(key) if key in order else len(order), key)


def _judged(j: dict | None) -> dict:
    if not j:
        return {"judged": False, "declared": "", "shown": "", "nature": "", "judgement": ""}
    return {
        "judged": True,
        "declared": j.get("declared", ""),
        "shown": j.get("shown", ""),
        "nature": j.get("nature", ""),
        "judgement": j.get("judgement", ""),
    }


def _findings(impl: list[dict], judgements: Judgements) -> list[dict]:
    """The implementers' calls that showed restricted content of the tool's own family before or
    during their work on it (the verdicts' findings), one per call, with their judgement."""
    items: dict[tuple[str, int], dict] = {}

    def item(a: dict, r: dict) -> dict:
        aid = agent_id(a["key"])
        return items.setdefault(
            (aid, int(r["line"])),
            {
                "agent": aid,
                "description": a["description"],
                "line": int(r["line"]),
                "time": _utc(r["ts"]),
                "rules": collections.Counter(),
                "files": set(),
            },
        )

    for a in impl:
        for r in a["restricted"]:
            if r["when"] == "before or during" and r["relevant"] and r["level"] in vd.SEVERE:
                it = item(a, r)
                it["rules"][f"{r['rule']} {r['level']}"] += 1
                it["files"].add(r["show"])
        for g in a["register"]:
            if g["when"] == "before or during" and g["relevant"] and g["body_lines"]:
                it = item(a, g)
                it["rules"]["R12 body lines"] = max(it["rules"]["R12 body lines"], g["body_lines"])
                it["files"].add(vd.REGISTER)
    out = []
    for k, it in sorted(items.items(), key=lambda kv: kv[1]["time"]):
        out.append(
            {
                **{x: it[x] for x in ("agent", "description", "line", "time")},
                "rules": _counter_rows(it["rules"]),
                "files": sorted(it["files"]),
                **_judged(judgements.access.get(k)),
            }
        )
    return out


def _slip_rules(rules: dict) -> list[list]:
    out = collections.Counter()
    for k, n in rules.items():
        rule, _, rest = k.partition(":")
        what, _, level = rest.rpartition(":")
        out[f"{rule} {level} ({what})"] += n
    return _counter_rows(out)


def _slips(v: dict, judgements: Judgements) -> list[dict]:
    """Every implementer's call that showed restricted material of any family, or a file its
    brief forbade, one per call, in time order, with its judgement."""
    items: dict[tuple[str, int], dict] = {}
    rules: dict[tuple[str, int], collections.Counter] = collections.defaultdict(collections.Counter)

    def item(x: dict, kind: str) -> tuple[str, int]:
        k = (agent_id(x["agent"]), int(x["line"]))
        items.setdefault(
            k,
            {
                "agent": k[0],
                "description": x["description"],
                "line": k[1],
                "time": _utc(x["ts"]),
                "kind": kind,
                "files": set(),
                "register": None,
            },
        )
        return k

    for s in v["slips"]:
        k = item(s, "restricted material")
        for r, n in _slip_rules(s.get("rules") or {}):
            rules[k][r] += n
        items[k]["files"] |= set(s.get("paths") or [])
        if s.get("register_lines"):
            items[k]["register"] = dict(s["register_lines"])
    for b in v["brief_dependent"]:
        k = item(b, "a file its brief forbade")
        rules[k][b["kind"].split(";")[0]] += 1
        items[k]["files"].add(b["rel"])
    for s in v["single_file_scopes"]["shown"]:
        k = item(s, "a search of one restricted file")
        rules[k][f"{s.get('rule')}: {s.get('lines')} lines of the file"] += 1
        items[k]["files"].add(s.get("rel", ""))
    return [
        {
            **it,
            "files": sorted(it["files"]),
            "rules": _counter_rows(rules[k]),
            **_judged(judgements.access.get(k)),
        }
        for k, it in sorted(items.items(), key=lambda kv: (kv[1]["time"], kv[0]))
    ]


def _edits(v: dict, judgements: Judgements) -> list[dict]:
    """Every edit of clean-room code by an agent that is not a clean-room implementer: commits,
    merge resolutions and uncommitted moves, with their judgement."""
    items: dict[tuple[str, str], dict] = {}
    for entry in v["tools"].values():
        for a in entry["agents"]:
            if a["role"] == IMPL:
                continue
            aid = agent_id(a["key"])
            rel = next(
                (
                    o["relevant_content_before"]
                    for o in entry["verdict"]["other_roles"]
                    if o["description"] == a["description"]
                ),
                0,
            )
            for h in a["commits"] or [""]:
                if not h and aid == "orchestrator":
                    continue
                k = (aid, h[:9])
                it = items.setdefault(
                    k,
                    {
                        "agent": aid,
                        "description": a["description"],
                        "role": a["role"],
                        "commit": h[:9],
                        "date": _day(v["commits"][_full(v, h)]["commit_date"])
                        if h
                        else _day(a["first_touch"]),
                        "tools": set(),
                        "added": None,
                        "removed": None,
                        "comments_or_blank": None,
                        "relevant_content_before": 0,
                    },
                )
                it["tools"].add(entry["name"])
                it["relevant_content_before"] = max(it["relevant_content_before"], rel)
                d = (a.get("diffs") or {}).get(h)
                if d:
                    it.update(
                        added=d["added"],
                        removed=d["removed"],
                        comments_or_blank=d["comments_or_blank"],
                    )
    for m in v["merge_edits"]:
        k = ("orchestrator", m["hash"][:9])
        it = items.setdefault(
            k,
            {
                "agent": "orchestrator",
                "description": "ORCHESTRATOR",
                "role": "orchestrator",
                "commit": m["hash"][:9],
                "date": _day(m["commit_date"]),
                "tools": set(),
                "added": m.get("lines_added"),
                "removed": m.get("lines_removed"),
                "comments_or_blank": None,
                "relevant_content_before": 1,
            },
        )
        for f in m["files"]:
            t = vd.tool_of(f)
            if t:
                it["tools"].add(vd.TOOL_NAMES.get(t, "/".join(t)))
    for lst in v["orchestrator_edits"].values():
        for e in lst:
            if e["op"] not in ("move", "delete"):
                continue  # edits and writes reach a commit, listed above
            k = ("orchestrator", "")
            it = items.setdefault(
                k,
                {
                    "agent": "orchestrator",
                    "description": "ORCHESTRATOR",
                    "role": "orchestrator",
                    "commit": "",
                    "date": _day(e["ts"]),
                    "tools": set(),
                    "added": None,
                    "removed": None,
                    "comments_or_blank": None,
                    "relevant_content_before": 1,
                },
            )
            t = vd.tool_of(e["rel"])
            it["tools"].add(vd.TOOL_NAMES.get(t, "/".join(t)) if t else e["rel"])
    out = []
    for k, it in sorted(items.items(), key=lambda kv: (kv[1]["date"], kv[0])):
        j = judgements.edit.get(k)
        out.append(
            {
                **it,
                "tools": sorted(it["tools"]),
                "judged": j is not None,
                "what": (j or {}).get("what", ""),
                "judgement": (j or {}).get("judgement", ""),
            }
        )
    return out


def _full(v: dict, short: str) -> str:
    return next((h for h in v["commits"] if h.startswith(short)), short)


# --- the gate -----------------------------------------------------------------------------------


def gate(v: dict, judgements: Judgements) -> tuple[list[str], list[str]]:
    """(blocking, to judge): blocking are the edits of clean-room code by an agent after it had
    seen restricted content of the tool's family (an implementer's finding, or another role's
    edit after such content) with no judgement, and commits nobody made; to judge, the other
    unjudged slips (another family, headings, files the briefs forbade)."""
    blocking, to_judge = [], []
    for key, entry in sorted(v["tools"].items()):
        impl = [a for a in entry["agents"] if a["role"] == IMPL]
        for f in _findings(impl, judgements):
            if not f["judged"]:
                rules = ", ".join(f"{r} x{n}" for r, n in f["rules"])
                blocking.append(
                    f"{key}: implementer {f['agent']} ({f['description']}) line {f['line']},"
                    f" {f['time']}: {rules} in {', '.join(f['files'])}"
                )
    for e in _edits(v, judgements):
        if e["judged"]:
            continue
        what = (
            f"{e['role']} {e['agent']} ({e['description']}) edited {', '.join(e['tools'])}"
            f" (commit {e['commit'] or 'none'}, {e['date']})"
        )
        if e["relevant_content_before"] or e["role"] == "orchestrator":
            blocking.append(what + f" after {e['relevant_content_before']} restricted views")
        else:
            to_judge.append(what)
    for h in v["unattributed"]:
        blocking.append(f"commit {h[:9]} changed clean-room code and no agent's record made it")
    for s in _slips(v, judgements):
        if not s["judged"]:
            rules = ", ".join(r for r, _ in s["rules"])
            to_judge.append(
                f"slip: {s['agent']} ({s['description']}) line {s['line']}, {s['time']}: {rules}"
                f" in {', '.join(s['files'])}"
            )
    return blocking, to_judge


# --- verbatim (private) -------------------------------------------------------------------------


def write_verbatim(scan: Path, v: dict, judgements: Judgements, path: Path) -> int:
    """Every judged or unjudged item's call and output, verbatim, with its record (private)."""
    agents = json.loads((scan / "agents.json").read_text(encoding="utf-8"))
    wanted: dict[str, set[str]] = collections.defaultdict(set)
    calls: dict[tuple[str, int], dict] = {}
    by_id = {agent_id(k): (k, info) for k, info in agents.items() if not k.endswith("/main")}
    keys = set()
    for entry in v["tools"].values():
        for f in _findings([a for a in entry["agents"] if a["role"] == IMPL], judgements):
            keys.add((f["agent"], f["line"]))
    for s in _slips(v, judgements):
        keys.add((s["agent"], s["line"]))
    for aid, line in sorted(keys):
        if aid not in by_id:
            continue
        key, info = by_id[aid]
        for r in vd.access_log(scan, info):
            if int(r.get("line") or 0) == line and r.get("call"):
                calls[(aid, line)] = {
                    "key": key,
                    "file": r["file"],
                    "call": r["call"],
                    "ts": r["ts"],
                    "command": r.get("command", ""),
                }
                wanted[r["file"]].add(r["call"])
                break
    results = vd.fetch_results(wanted)
    L = [
        "# The clean-room audit: every judged item verbatim (private: paths, sessions, outputs)",
        "",
    ]
    for (aid, line), c in sorted(calls.items(), key=lambda kv: kv[1]["ts"]):
        j = judgements.access.get((aid, line))
        L += [
            f"## {aid} line {line} ({c['ts']})",
            "",
            f"- record: `{c['key']}`, `{c['file']}` line {line}, call `{c['call']}`",
            f"- judgement: {j['judgement'] if j else 'NOT YET JUDGED'}",
            "",
            "Command:",
            "",
            "```",
            c["command"] or "(a tool call without a command: see the record)",
            "```",
            "",
            "Output:",
            "",
            "```",
            results.get(c["call"], "(no output found)"),
            "```",
            "",
        ]
    path.write_text("\n".join(L), encoding="utf-8")
    return len(calls)


# --- rendering ----------------------------------------------------------------------------------


def _pct(x) -> str:
    return "-" if x is None else f"{100 * x:.1f} %"


def _n(x) -> str:
    return f"{x:,}"


def _cell(c: dict | None) -> str:
    if not c:
        return "-"
    return f"{_pct(c.get('identifiers_shared'))} of names, {_pct(c.get('coverage_8'))} of tokens in runs of 8+, longest run {c.get('longest_run')}"


DECLARED = {
    "yes": "declared",
    "partly": "declared, under-counted",
    "covered": "covered by a declaration",
    "no": "**undeclared**",
    "": "not yet judged",
}


def _rule_title(facts: dict, rid: str, short: bool = False) -> str:
    t = next((r["title"] for r in facts["rules"] if r["id"] == rid), rid)
    return t.split(" (", 1)[0] if short else t


def _level_words(facts: dict, level: str) -> str:
    words = dict(map(tuple, facts["levels"])) | dict(map(tuple, facts["register_levels"]))
    return words.get(level, level)


def _plural(n: int, word: str) -> str:
    return f"{n:,} {word}" + ("" if n == 1 else "s")


def _item_text(x: dict) -> str:
    if not x["judged"]:
        rules = "; ".join(f"{r} ({n})" for r, n in x["rules"])
        return f"**not yet judged** ({rules}; {', '.join(f'`{f}`' for f in x['files'])})"
    return f"{x['shown']}. *Nature:* {x['nature']}. *Judgement:* {x['judgement']}"


def render(facts: dict) -> str:
    """The page, from the public facts alone."""
    ev, ov = facts["evidence"], facts["overview"]
    L: list[str] = []
    w = L.append
    tools = facts["tools"]
    n_impl_commits = dict(map(tuple, ov["commits_by_role"])).get(IMPL, 0)
    pending = [t for t in tools if t["verdict"]["status"] == "review"]
    undeclared = [s for s in facts["slips"] if s["declared"] == "no"]
    w("# The clean-room audit")
    w("")
    w(
        "<!-- Generated by `python -m larmorx_validation cleanroom-audit summary` from"
        " `docs/validation/clean-room-audit.json`; do not edit by hand. -->"
    )
    w("")
    w(
        f"The evidence, from records made at the time, that the clean-room originals in larmorx were"
        f" written without their upstream's source. Dossier of {facts['dossier']}; the records"
        f" from {ev['first_record']} to {ev['last_record']}."
    )
    w("")
    w("## What this page proves")
    w("")
    w(
        "Some of the tools larmorx re-implements are under licences that do not allow their code"
        " into an Apache-2.0 package (FSL's; AFNI's until its release 26.3.05; glibc's exact"
        " mathematics). For each, larmorx has two implementations ([licensing](../licensing.md)):"
        " a **replica**, a faithful port of the upstream source, kept in a separate package under"
        " the upstream's own licence; and a **clean-room original**, written only from a behaviour"
        " specification (`specs/`), the upstream's documentation, published papers and black-box"
        " runs of the upstream program, by agents that never saw the upstream source, the replica,"
        " or anything written from them. A specification is written by a different agent, which"
        " may read the source but describes only behaviour, mathematics and the command line,"
        " never code ([porting guide](../porting-guide.md) §1)."
    )
    w("")
    w(
        "**The records.** Every agent that worked on larmorx was an AI agent (Claude Code), launched"
        " by one orchestrating session. Claude Code keeps a transcript of every session: each tool"
        " call (each file read, search, command and edit) with its result. The audit reads every"
        " transcript and lists, for each agent, what it read, searched, listed, ran and wrote."
        " Each path is classified by a restriction table written from the project's rules, with"
        " the date each rule began to apply. Git's history says which agent made each commit of"
        " clean-room code: the commit hash the agent printed when it committed, or its commit"
        " command. For each clean-room original the question is: **did any agent that had seen"
        " restricted material of that tool's family edit its code afterwards?**"
    )
    w("")
    answer = (
        f"**The answer.** {_n(ov['commits'])} commits changed clean-room code;"
        f" {_n(ov['commits'] - ov['unattributed'])} are attributed to the agent that made them"
        f" ({_n(n_impl_commits)} to clean-room implementers; the others are listed under"
        " [edits by other roles](#edits-of-clean-room-code-by-other-roles))."
    )
    if pending:
        answer += (
            f" **{_plural(len(pending), 'original')} {'has' if len(pending) == 1 else 'have'} items"
            " not yet judged**: " + ", ".join(t["name"] for t in pending) + "."
        )
    else:
        answer += (
            " For every clean-room original below, no agent that had seen its upstream's source"
            " or its replica's code wrote its algorithms. The implementers' slips, lines of"
            " restricted material that a search printed, are each listed with their nature and"
            f" judgement ({len(facts['slips'])} calls, {len(undeclared)} of them not declared by"
            " their part): titles of defect entries, behaviour the specification already gave,"
            " another family's code, test-harness lines; none carries an upstream routine."
        )
    w(answer)
    w("")
    w(
        "Code similarity is reported as extra evidence only ([below](#code-similarity-extra-evidence-no-threshold)):"
        " a figure, never a pass or a failure."
    )
    w("")

    # The verdicts at a glance.
    w("## The verdicts at a glance")
    w("")
    w(
        "| clean-room original | family | first and last edit | clean-room implementers | other roles | commits | verdict |"
    )
    w("|---|---|---|---|---|---|---|")
    for t in tools:
        anchor = _anchor(t["name"])
        others = ", ".join(sorted({o["role"] for o in t["other_roles"]})) or "-"
        w(
            f"| [{t['name']}](#{anchor}) | {t['family']} | {t['first']} to {t['last']} |"
            f" {len(t['implementers'])} | {others} | {t['commits']} | {t['verdict']['words'] or t['verdict']['status']} |"
        )
    w("")

    # The evidence.
    w("## The evidence")
    w("")
    by_tool = ", ".join(
        f"{k} {_n(n)}" for k, n in sorted(ev["tool_calls_by_tool"], key=lambda x: -x[1])[:6]
    )
    w(
        f"- **The transcripts:** {_n(ev['transcript_files'])} files ({ev['transcript_bytes'] / 1e9:.2f} GB),"
        f" {_n(ev['distinct_transcripts'])} distinct transcripts (copies of one transcript on two"
        f" machines or in the archive are merged): {ev['sessions']} orchestrator sessions and"
        f" {_n(ev['agents'])} agents, {_n(ev['tool_calls'])} tool calls ({by_tool}, ...),"
        f" {_n(ev['accesses'])} accesses to {_n(ev['unique_paths'])} distinct paths."
    )
    for name, a in sorted(ev["archives"].items()):
        w(
            f"- **The archive** `{name}`: {_n(a['files'])} transcripts copied before Claude Code's"
            f" 30-day clean-up, with their checksum list; the list's SHA-256 is `{a['sha256']}`."
        )
    w(
        f"  Of the files read, {_n(ev['archived_files'])} are archived copies (each matches its"
        f" checksum: {_n(ev['archive_checksums_matched'])} match, {ev['archive_checksums_differ']}"
        f" differ) and {_n(ev['live_files'])} were read where Claude Code keeps them (newer"
        " sessions; the merge step archives them from now on). The private dossier lists the"
        " SHA-256 of every file read."
    )
    g = ev["gaps"]
    w(
        f"- **What the reader could not use:** {g['bad_lines']} unreadable lines;"
        f" {g['results_without_call']} results without their call; {g['calls_without_result']}"
        f" calls without a result (interrupted runs at a transcript's end); {g['compactions']}"
        " context compactions, every earlier record still in its file"
        f" ({g['compaction_parent_missing']} boundaries name a parent record found in no file, with"
        f" every earlier record present); {_n(g['refused_calls'])} calls the harness refused (their"
        f" {_n(g['refused_calls_restricted_accesses'])} restricted paths are not counted: nothing"
        f" ran); {_n(g['unresolved_paths'])} paths with a variable set elsewhere (classified by"
        " their visible part)."
    )
    att = dict(map(tuple, ov["attributed_by"]))
    w(
        f"- **The commits of clean-room code:** {_n(ov['commits'])} in every branch;"
        f" {_n(att.get('printed', 0))} attributed by the hash their maker printed when it"
        f" committed, {_n(att.get('subject', 0))} by the commit command carrying their subject;"
        f" {ov['unattributed']} unattributed."
    )
    roles = ", ".join(
        f"{'other tasks' if r == 'other' else r} {n}"
        for r, n in sorted(ov["agents_by_role"], key=lambda x: -x[1])
    )
    w(
        f"- **The agents' roles** (from their launch descriptions and briefs; every agent that"
        f" touched clean-room code checked by hand): {roles}."
    )
    w("")

    # The restriction table.
    w("## What counts as restricted")
    w("")
    w(
        "The restriction table, from the project's rules (CLAUDE.md rule 1 and its agent rules,"
        " the [porting guide](../porting-guide.md) §1, [licensing](../licensing.md)). *Restricted"
        " since*: when its content became restricted by nature (the decision of 2026-10-09 for the"
        " upstream sources and the replicas; a file's first commit for a finding written from the"
        " source). *Named since*: when the rules first named it. An access between the two still"
        " counts: reading source-derived content is a contamination whatever the rules said that"
        " day."
    )
    w("")
    w("| rule | what | family | restricted since | named since | basis |")
    w("|---|---|---|---|---|---|")
    for r in facts["rules"]:
        w(
            f"| {r['id']} | {r['title']} | {r['family']} | {r['since']} | {r['named']} | {r['basis']} |"
        )
    w("")
    w(
        "**Allowed, and logged by kind:** black-box runs of the original programs (FSL's, AFNI's,"
        " ANTs', FreeSurfer's, fMRIPrep's), their `-help` and documentation, permissive upstream"
        " source (ANTs, ITK, nibabel, SciPy, numpy, OpenBLAS, CORE-MATH, Arm's routines,"
        " nitransforms, fMRIPrep and its libraries), `specs/`, the clean-room code, oracle outputs"
        " and data."
    )
    w("")
    w("**How each restricted access weighs** (the verdicts' levels):")
    w("")
    w("| level | meaning |")
    w("|---|---|")
    for k, txt in facts["levels"]:
        w(f"| {k} | {txt} |")
    for k, txt in facts["register_levels"]:
        w(f"| register {k} | {txt} |")
    w("")
    w(
        "*Whose material:* **this family** (the tool's own upstream, its replica or findings"
        " about it, while the rule applied); **another family** (for example AFNI's replica seen"
        " by an agent that wrote FSL code); **before the rule applied**; **written by clean-room"
        " implementers only** (other parts' notes whose every writer was a clean-room"
        " implementer of the same kind, or the orchestrator's brief)."
    )
    w("")

    # Per clean-room original.
    w("## Per clean-room original")
    w("")
    for t in tools:
        _render_tool(facts, t, w)

    # Edits by other roles.
    w("## Edits of clean-room code by other roles")
    w("")
    w(
        "The orchestrator reads upstream source to merge and verify; the rules allow it no edit of"
        " clean-room code beyond the mechanics of a merge. Every edit of clean-room code by an"
        " agent that was not a clean-room implementer:"
    )
    w("")
    w("| date | who | commit | original | change | judgement |")
    w("|---|---|---|---|---|---|")
    for e in facts["edits"]:
        who = (
            "the orchestrator"
            if e["agent"] == "orchestrator"
            else f"{e['role']} `{e['agent']}` ({e['description']})"
        )
        change = e["what"] if e["judged"] else "**not yet judged**"
        if e["added"] is not None:
            change += f" (+{e['added']}/-{e['removed']} lines"
            change += (
                f", {e['comments_or_blank']} comments or blank)"
                if e["comments_or_blank"] is not None
                else ")"
            )
        w(
            f"| {e['date']} | {who} | {e['commit'] or 'none'} | {', '.join(e['tools'])} | {change} |"
            f" {e['judgement'] or '**not yet judged**'} |"
        )
    w("")

    # Every slip.
    w("## Every slip of the clean-room implementers")
    w("")
    w(
        "Every call of a clean-room implementer, of any tool, whose output showed restricted"
        " material of any family (lines of it, not only names), or that read a file its brief"
        " forbade, in time order. *Declared*: the part's own report or notes listed it."
        f" {len(facts['slips'])} calls; {len(undeclared)} undeclared."
    )
    w("")
    w("| time | implementer | kind | declared | what it showed, its nature, the judgement |")
    w("|---|---|---|---|---|")
    for s in facts["slips"]:
        w(
            f"| {s['time']} | `{s['agent']}` ({s['description']}), record line {s['line']} |"
            f" {s['kind']} | {DECLARED.get(s['declared'], s['declared'])} | {_item_text(s)} |"
        )
    so = ov["single_file_scopes"]
    w("")
    w(
        f"Searches of one restricted file (they print no file name) were checked against that"
        f" file's lines: {so['checked']} such searches by implementers, {so['shown']} with a line"
        f" of the file in their output. The register's restricted lines"
        f" ({_n(ov['register_restricted_lines'])} distinct lines over every version) were compared"
        " with every output of every implementer's read or search of the register."
    )
    w("")

    # Similarity.
    sim = sm.public_markdown(facts["similarity"])
    _, _, rest = sim.partition("\n")
    w("## Code similarity (extra evidence, no threshold)")
    w(rest.rstrip("\n"))
    w("")
    w(
        f"The figures come from the similarity record made at `{facts['similarity'].get('commit', '')}`."
    )
    w("")

    # Limits.
    w("## What the evidence cannot see")
    w("")
    for x in facts["limits"]:
        w(f"- **{x['title']}.** {x['text']}")
    w("")
    if facts["recommendations"]:
        w("## Recommendations")
        w("")
        for i, r in enumerate(facts["recommendations"], 1):
            w(f"{i}. {r}")
        w("")

    # How to re-run.
    w("## How to re-run")
    w("")
    w(
        "On the machine that holds the transcripts (the outputs are private: they hold absolute"
        " paths and the account name; they stay in `<workspace>/archive/clean-room-audit/<date>/`),"
        " each step inside a memory box:"
    )
    w("")
    w("```bash")
    w("D=<workspace>/archive/clean-room-audit/<date>")
    w(
        "scripts/orchestration/boxed.sh 24 scripts/orchestration/lxv.sh . cleanroom-audit scan --out $D/scan --jobs 12"
    )
    w("scripts/orchestration/lxv.sh . cleanroom-audit verdicts --scan $D/scan --out $D/verdicts")
    w(
        "scripts/orchestration/boxed.sh 24 scripts/orchestration/lxv.sh . cleanroom-audit similarity --out $D/similarity --wide --jobs 12"
    )
    w(
        "scripts/orchestration/lxv.sh . cleanroom-audit summary --dossier $D --verbatim $D/verbatim.md"
    )
    w("scripts/orchestration/lxv.sh . cleanroom-audit gate --verdicts $D/verdicts")
    w("```")
    w("")
    w(
        "The scan reads every transcript (the archives `<workspace>/archive/*session-transcripts*/`"
        " and Claude Code's own folder) in about 15 seconds; the verdicts take seconds; the"
        " similarity about 15 minutes with `--wide` (every C and C++ file of AFNI's and FSL's"
        " trees). The summary writes `docs/validation/clean-room-audit.json` (the public facts) and"
        " this page; `cleanroom-audit summary --check` renders the committed facts and compares"
        " them with this page. At every merge, `scripts/orchestration/cleanroom-audit-merge.sh`"
        " archives the part's transcripts with their SHA-256 and runs the scan, the verdicts and"
        " the gate, which stops the merge when a clean-room original was edited after restricted"
        " material of its family was seen, unless the item is judged in"
        f" `{JUDGEMENTS}`."
    )
    w("")
    w(
        f"Made from the scan `{facts['made_from']['scan']}` and the verdicts of the repository at"
        f" `{facts['made_from']['verdicts_repository']}`."
    )
    return "\n".join(L) + "\n"


def _anchor(name: str) -> str:
    a = name.lower()
    a = re.sub(r"[^\w\- ]", "", a)
    return a.replace(" ", "-")


def _render_tool(facts: dict, t: dict, w) -> None:
    w(f"### {t['name']}")
    w("")
    roles = dict(map(tuple, t["commits_by_role"]))
    by = ", ".join(f"{r} {n}" for r, n in sorted(roles.items(), key=lambda x: -x[1]))
    w(
        f"- **Family:** {t['family']}. **Edited** from {t['first']} to {t['last']}; **commits:** {t['commits']} (by role: {by})."
    )
    impl = "; ".join(
        f"`{a['agent']}` {a['description']} ({a['first']}{'' if a['first'] == a['last'] else ' to ' + a['last']}, {_plural(a['commits'], 'commit')})"
        for a in t["implementers"]
    )
    w(f"- **Clean-room implementers ({len(t['implementers'])}):** {impl}.")
    if t["other_roles"]:
        others = "; ".join(
            f"{'the orchestrator' if o['agent'] == 'orchestrator' else o['role'] + ' `' + o['agent'] + '` (' + o['description'] + ')'}:"
            f" {_plural(len(o['commits']), 'commit')} ({', '.join(o['commits']) or 'none'})"
            for o in t["other_roles"]
        )
        w(
            f"- **Other roles that edited it:** {others} (see [edits by other roles](#edits-of-clean-room-code-by-other-roles))."
        )
    words = (t["verdict"]["words"] or "not yet judged").rstrip(".")
    w(f"- **Verdict:** {words}. Machine status: {t['verdict']['status']}.")
    w("")
    w(
        "What its implementers reached before or during their work on it (accesses: seen = read, searched, listed; run = ran or named; written):"
    )
    w("")
    w("| reached | seen | run | written |")
    w("|---|---|---|---|")
    for a in t["accesses"]:
        w(f"| {a['group']} | {_n(a['seen'])} | {_n(a['run'])} | {_n(a['written'])} |")
    w("")
    if t["restricted"]:
        w(
            "Its implementers' restricted accesses, by rule and kind (before or during their work on it; after it):"
        )
        w("")
        w("| whose material | rule | kind | before or during | after | agents |")
        w("|---|---|---|---|---|---|")
        for r in t["restricted"]:
            kind = _level_words(facts, r["level"])
            w(
                f"| {r['scope']} | {r['rule']} {_rule_title(facts, r['rule'], short=True)} | {kind} | {_n(r['before'])} |"
                f" {_n(r['after'])} | {r['agents']} |"
            )
        w("")
    else:
        w("Its implementers made no restricted access.")
        w("")
    if t["findings"]:
        w(
            "**Content of this family's restricted material seen before or during the work** (each call):"
        )
        w("")
        for f in t["findings"]:
            w(
                f"- {f['time']}, `{f['agent']}` ({f['description']}), record line {f['line']},"
                f" {DECLARED.get(f['declared'], f['declared'])}: {_item_text(f)}."
            )
        w("")
    else:
        w("**Content of this family's restricted material seen before or during the work:** none.")
        w("")
    slips = [facts["slips"][i] for i in t["slips"]]
    if slips:
        d = sum(1 for s in slips if s["declared"] in ("yes", "partly", "covered"))
        w(
            f"**Slips of its implementers** (any family, any time; [every slip](#every-slip-of-the-clean-room-implementers)):"
            f" {len(slips)} calls, {d} declared or covered by a declaration, {len(slips) - d} not:"
            + " "
            + "; ".join(
                f"{s['time'][:10]} `{s['agent']}` line {s['line']} ({DECLARED.get(s['declared'], s['declared'])})"
                for s in slips
            )
            + "."
        )
    else:
        w("**Slips of its implementers:** none.")
    w("")
    s = t.get("similarity")
    if s:
        txt = (
            f"**Similarity** ({_n(s['tokens'])} tokens): against its upstream, {_cell(s['upstream'])};"
            f" the other tools' upstream gives {_pct(s['upstream_floor']['median'])} (median) to"
            f" {_pct(s['upstream_floor']['max'])} of tokens in runs of 8+; its replica against the"
            f" same upstream (a translation), {_cell(s['replica_upstream'])}. Against its replica,"
            f" {_cell(s['replica'])}; the family's other replicas give"
            f" {_pct(s['replica_floor']['median'])} to {_pct(s['replica_floor']['max'])}."
        )
        tr = s.get("tree")
        if tr:
            txt += (
                f" Against every C and C++ file of {tr['family']}'s tree, {_pct(tr['coverage_8'])}"
                f" (longest run {tr['longest_run']}), where other families' clean-room code reaches"
                f" up to {_pct(tr['floor_max'])} and its replica {_pct(tr['replica'])}."
            )
        w(
            txt
            + " The calibration is under [code similarity](#code-similarity-extra-evidence-no-threshold)."
        )
        w("")
    c = t["cannot_see"]
    w(
        f"*Not seen by the records:* what {_n(c.get('program_runs', 0))} program runs (builds, tests,"
        f" scripts) read beyond their arguments and output; the names {_n(c.get('listings', 0))}"
        f" listings printed; {_n(c.get('unresolved', 0))} paths with a variable set elsewhere"
        f" (classified by their visible part); {c.get('without_worktree', 0)} of its implementers"
        f" ran without a harness worktree (attributed by their printed commits);"
        f" {c.get('compactions', 0)} context compactions (no record lost)."
    )
    w("")


# --- the command line ---------------------------------------------------------------------------


def add_arguments(sub) -> None:
    s = sub.add_parser("summary", help="the public summary (docs/validation/clean-room-audit.md)")
    s.add_argument("--dossier", help="a private dossier: extract the public facts from it")
    s.add_argument("--scan", help="the scan (default: the dossier's scan*)")
    s.add_argument("--verdicts", help="the verdicts (default: the dossier's verdicts/)")
    s.add_argument("--similarity", help="a similarity record (default: the newest)")
    s.add_argument("--judgements", default=str(REPO / JUDGEMENTS))
    s.add_argument("--json", default=str(REPO / PUBLIC_JSON), help="the public facts")
    s.add_argument("--out", default=str(REPO / PUBLIC_MD), help="the page")
    s.add_argument("--verbatim", help="also write every item's call and output here (private)")
    s.add_argument(
        "--check",
        action="store_true",
        help="render the facts and compare with the page (1: differ)",
    )
    g = sub.add_parser("gate", help="the merge step: unjudged edits after restricted views")
    g.add_argument("--verdicts", required=True, help="a verdicts output directory")
    g.add_argument("--judgements", default=str(REPO / JUDGEMENTS))
    g.add_argument("--strict", action="store_true", help="also fail on items to judge")


def main_summary(args: argparse.Namespace) -> int:
    jpath, mpath = Path(args.json), Path(args.out)
    if args.dossier:
        dossier = Path(args.dossier)
        scan, verdicts_dir, sim = find_dossier_parts(dossier)
        scan = Path(args.scan) if args.scan else scan
        verdicts_dir = Path(args.verdicts) if args.verdicts else verdicts_dir
        sim = Path(args.similarity) if args.similarity else sim
        judgements = load_judgements(Path(args.judgements))
        facts = extract(scan, verdicts_dir, sim, judgements)
        jpath.write_text(json.dumps(facts, indent=1, sort_keys=True) + "\n", encoding="utf-8")
        if args.verbatim:
            v = json.loads((verdicts_dir / "verdicts.json").read_text(encoding="utf-8"))
            n = write_verbatim(scan, v, judgements, Path(args.verbatim))
            print(f"cleanroom-audit summary: {n} calls verbatim in {args.verbatim}")
    facts = json.loads(jpath.read_text(encoding="utf-8"))
    text = render(facts)
    problems = privacy_problems(text) + privacy_problems(jpath.read_text(encoding="utf-8"))
    if problems:
        print(
            "cleanroom-audit summary: private strings: " + "; ".join(problems[:10]), file=sys.stderr
        )
        return 1
    if args.check:
        same = mpath.is_file() and mpath.read_text(encoding="utf-8") == text
        print(
            f"cleanroom-audit summary: {mpath.name} {'agrees with' if same else 'DIFFERS from'} {jpath.name}"
        )
        return 0 if same else 1
    mpath.write_text(text, encoding="utf-8")
    pending = [t["id"] for t in facts["tools"] if t["verdict"]["status"] == "review"]
    unjudged = [s for s in facts["slips"] if not s["judged"]]
    print(
        f"cleanroom-audit summary: {len(facts['tools'])} originals, {len(pending)} with items not"
        f" yet judged, {len(unjudged)} slips not yet judged; {mpath}"
    )
    return 0


def main_gate(args: argparse.Namespace) -> int:
    v = json.loads((Path(args.verdicts) / "verdicts.json").read_text(encoding="utf-8"))
    blocking, to_judge = gate(v, load_judgements(Path(args.judgements)))
    for b in blocking:
        print(f"STOP  {b}")
    for t in to_judge:
        print(f"JUDGE {t}")
    print(
        f"cleanroom-audit gate: {len(v['tools'])} originals, {len(v['commits'])} commits;"
        f" {len(blocking)} blocking, {len(to_judge)} to judge"
    )
    if blocking:
        return 1
    return 1 if (to_judge and args.strict) else 0
