# SPDX-License-Identifier: Apache-2.0
"""The session transcripts: finding them, reading them as a stream, and every tool call in them.

Layout (Claude Code's): ``<root>/<session>.jsonl`` is the orchestrator's session,
``<root>/<session>/subagents/agent-<id>.jsonl`` an agent's, with ``agent-<id>.meta.json``
(``description``, ``worktreePath``, ``worktreeBranch``, ``toolUseId`` of the call that launched
it, ``parentAgentId`` for an agent's own agents), and ``<root>/<session>/tool-results/`` the long
outputs a result only names (``<persisted-output>``). A record is a JSON line; ``assistant``
records carry the ``tool_use`` blocks (the calls), ``user`` records the ``tool_result`` blocks.
A summarised session (context compaction, a ``compact_boundary`` record) keeps its earlier
records in the same file; ``api-request-blob`` records repeat what was sent to the model and are
used only to check that no call is missing from the assistant records.

The same session can be in several roots (the archive's two machines, the live directory): every
copy is read, and a call is counted once by its tool-use id.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import asdict, dataclass, field
from pathlib import Path

from larmorx_validation.cleanroom_audit import accesses as acc
from larmorx_validation.cleanroom_audit.restrictions import Context

PERSISTED = re.compile(r"Full output saved to: (\S+/tool-results/[^\s/]+)")
AGENT_ID = re.compile(r"agentId[\"']?\s*[:=]\s*[\"']?([a-z0-9]{8,})")
CONTINUED = "This session is being continued from a previous conversation"
#: Results of calls that never ran (the harness's worktree isolation, a permission denied).
REFUSED = re.compile(
    r"^(This agent is isolated in the worktree|<tool_use_error>|Permission to use \w+ .*denied|"
    r"The user doesn't want to proceed|Claude requested permissions? to .* haven't granted|"
    r"Hook \S+ blocked|.*Refusing to run it)",
    re.M,
)
REMINDER = re.compile(r"<system-reminder>.*?</system-reminder>", re.S)
MAX_RESULT = 8_000_000
SCRIPT_SUFFIX = (".py", ".sh", ".bash", ".pl", ".R")


def _words(s: str) -> frozenset[str]:
    """A set of names written as one string."""
    return frozenset(s.split())


@dataclass
class TranscriptFile:
    path: str
    root: str
    session: str
    agent: str  # "main" (the orchestrator) or "agent-<id>"

    @property
    def key(self) -> str:
        return f"{self.session}/{self.agent}"


def discover(roots: list[Path]) -> list[TranscriptFile]:
    """Every transcript under the roots (a root holds ``<session>.jsonl`` files)."""
    out: list[TranscriptFile] = []
    for root in roots:
        for p in sorted(root.rglob("*.jsonl")):
            rel = p.relative_to(root)
            parts = rel.parts
            if len(parts) == 1:
                out.append(TranscriptFile(str(p), str(root), p.stem, "main"))
            elif len(parts) == 3 and parts[1] == "subagents":
                out.append(TranscriptFile(str(p), str(root), parts[0], p.stem))
    return out


@dataclass
class Call:
    id: str
    tool: str
    ts: str
    line: int
    cwd: str
    command: str = ""
    result_chars: int = 0
    is_error: bool = False
    has_result: bool = False
    persisted: str = ""  # "", "loaded" or "missing"
    refused: bool = False  # the harness or the permission system refused it: nothing ran
    accesses: list[dict] = field(default_factory=list)
    commits: list[dict] = field(default_factory=list)
    spawn: dict | None = None
    unparsed: list[str] = field(default_factory=list)


@dataclass
class FileResult:
    file: TranscriptFile
    sha256: str = ""
    size: int = 0
    records: int = 0
    bad_lines: int = 0
    first_ts: str = ""
    last_ts: str = ""
    compactions: int = 0
    compaction_parent_missing: int = 0
    continued: bool = False
    types: dict = field(default_factory=dict)
    tools: dict = field(default_factory=dict)
    versions: list = field(default_factory=list)
    meta: dict = field(default_factory=dict)
    brief: str = ""
    calls: list[Call] = field(default_factory=list)
    calls_without_result: int = 0
    results_without_call: int = 0
    request_only_calls: int = 0
    persisted_missing: int = 0
    persisted_loaded: int = 0
    unknown_tools: dict = field(default_factory=dict)


#: Tools whose calls open, list, search, run or write nothing the audit tracks.
NEUTRAL_TOOLS = _words(
    "ToolSearch TaskStop TaskOutput SendMessage AskUserQuestion SubagentHandback Skill "
    "ReadNotifications Artifact ArtifactComments ArtifactData WebSearch TodoWrite ExitPlanMode "
    "EnterPlanMode BashOutput KillShell KillBash EnterWorktree ExitWorktree "
    "ListMcpResourcesTool ReadMcpResourceTool"
)
MODELLED_TOOLS = _words(
    "Read Write Edit MultiEdit NotebookEdit Grep Glob LS WebFetch Bash Monitor Agent Task"
)


def _text(content) -> str:
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for b in content:
            if isinstance(b, dict) and b.get("type") == "text":
                parts.append(str(b.get("text", "")))
            elif isinstance(b, str):
                parts.append(b)
        return "\n".join(parts)
    return ""


def _persisted_path(stub: str, file: TranscriptFile, roots: list[str]) -> Path | None:
    m = PERSISTED.search(stub)
    if not m:
        return None
    name = Path(m[1]).name
    session = Path(m[1]).parent.parent.name
    for root in [file.root, *roots]:
        cand = Path(root) / session / "tool-results" / name
        if cand.is_file():
            return cand
    return None


def parse_file(file: TranscriptFile, ctx: Context, roots: list[str]) -> FileResult:
    """One transcript, read as a stream: its calls with their accesses, and its statistics."""
    fr = FileResult(file)
    p = Path(file.path)
    meta_path = p.with_name(p.stem + ".meta.json")
    if meta_path.is_file():
        try:
            fr.meta = json.loads(meta_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            fr.meta = {"error": "unreadable meta"}
    h = hashlib.sha256()
    pending: dict[str, Call] = {}
    calls: dict[str, Call] = {}
    requested: set[str] = set()
    uuids: set[str] = set()
    parents: list[str] = []
    scripts: dict[str, str] = {}
    versions: set[str] = set()
    with p.open("rb") as fh:
        for lineno, raw in enumerate(fh, 1):
            h.update(raw)
            fr.size += len(raw)
            fr.records += 1
            try:
                r = json.loads(raw)
            except (json.JSONDecodeError, UnicodeDecodeError):
                fr.bad_lines += 1
                continue
            if not isinstance(r, dict):
                fr.bad_lines += 1
                continue
            t = r.get("type")
            fr.types[t] = fr.types.get(t, 0) + 1
            if r.get("uuid"):
                uuids.add(r["uuid"])
            if r.get("version"):
                versions.add(str(r["version"]))
            ts = r.get("timestamp") or ""
            if ts:
                fr.first_ts = fr.first_ts or ts
                fr.last_ts = ts
            if t == "system" and r.get("subtype") == "compact_boundary":
                fr.compactions += 1
                if r.get("logicalParentUuid"):
                    parents.append(r["logicalParentUuid"])
                continue
            msg = r.get("message")
            if not isinstance(msg, dict):
                continue
            content = msg.get("content")
            if t == "api-request-blob":
                if isinstance(content, list):
                    for b in content:
                        if isinstance(b, dict) and b.get("type") == "tool_use" and b.get("id"):
                            requested.add(b["id"])
                continue
            if t == "user":
                if not fr.brief and not r.get("isMeta"):
                    text = REMINDER.sub("", _text(content)).strip()
                    if text and not text.startswith("<"):
                        fr.brief = text[:200_000]
                    if text.startswith(CONTINUED):
                        fr.continued = True
                if isinstance(content, list):
                    for b in content:
                        if isinstance(b, dict) and b.get("type") == "tool_result":
                            _result(b, r, pending, fr, file, roots, ctx, scripts)
                continue
            if t == "assistant" and isinstance(content, list):
                cwd = r.get("cwd") or ""
                for b in content:
                    if not (isinstance(b, dict) and b.get("type") == "tool_use"):
                        continue
                    cid = b.get("id") or f"noid-{lineno}"
                    if cid in calls:
                        continue
                    name = str(b.get("name", ""))
                    fr.tools[name] = fr.tools.get(name, 0) + 1
                    inp = b.get("input") if isinstance(b.get("input"), dict) else {}
                    cmd = inp.get("command", "") if name in ("Bash", "Monitor") else ""
                    c = Call(cid, name, ts, lineno, cwd, command=str(cmd)[:100_000])
                    c.input = inp  # type: ignore[attr-defined]
                    calls[cid] = c
                    pending[cid] = c
                    known = name in MODELLED_TOOLS or name in NEUTRAL_TOOLS
                    if not known and not name.startswith("mcp__"):
                        fr.unknown_tools[name] = fr.unknown_tools.get(name, 0) + 1
    # Calls whose result never came (an interrupted run, the end of the file).
    for c in list(pending.values()):
        fr.calls_without_result += 1
        _finish(c, "", ctx, scripts)
    fr.sha256 = h.hexdigest()
    fr.request_only_calls = len(requested - set(calls))
    fr.compaction_parent_missing = sum(1 for u in parents if u not in uuids)
    fr.versions = sorted(versions)
    for c in calls.values():
        if hasattr(c, "input"):
            del c.input  # type: ignore[attr-defined]
    fr.calls = sorted(calls.values(), key=lambda c: c.line)
    return fr


def _result(b: dict, r: dict, pending: dict, fr: FileResult, file, roots, ctx, scripts) -> None:
    cid = b.get("tool_use_id")
    c = pending.pop(cid, None)
    if c is None:
        fr.results_without_call += 1
        return
    text = _text(b.get("content"))
    if "<persisted-output>" in text[:200]:
        path = _persisted_path(text, file, roots)
        if path is None:
            c.persisted = "missing"
            fr.persisted_missing += 1
        else:
            try:
                text = path.read_text(encoding="utf-8", errors="replace")[:MAX_RESULT]
                c.persisted = "loaded"
                fr.persisted_loaded += 1
            except OSError:
                c.persisted = "missing"
                fr.persisted_missing += 1
    c.has_result = True
    c.is_error = bool(b.get("is_error"))
    c.refused = c.is_error and bool(REFUSED.search(text[:2000]))
    c.result_chars = len(text)
    if c.tool in ("Agent", "Task"):
        tur = r.get("toolUseResult")
        aid = tur.get("agentId") if isinstance(tur, dict) else None
        if not aid:
            m = AGENT_ID.search(text)
            aid = m[1] if m else None
        c.spawn = {"agent": f"agent-{aid}" if aid else None}
    _finish(c, text, ctx, scripts)


def _finish(c: Call, text: str, ctx: Context, scripts: dict) -> None:
    inp = getattr(c, "input", {})
    try:
        res = acc.call_accesses(c.tool, inp, text, c.cwd, ctx, scripts)
    except Exception as e:  # a parsing gap, never a crash of the audit
        c.unparsed.append(f"{type(e).__name__}: {e}"[:300])
        return
    c.accesses = [asdict(a) for a in res.accesses]
    c.commits = [asdict(k) for k in res.commits]
    c.unparsed.extend(res.unparsed)
    if res.spawns:
        c.spawn = {**(c.spawn or {}), **res.spawns[0]}
    if c.tool == "Write":
        p = inp.get("file_path", "")
        if p and not p.endswith(SCRIPT_SUFFIX) and not str(inp.get("content", "")).startswith("#!"):
            scripts.pop(acc._abs(p, c.cwd, ctx), None)


def parse_file_job(args: tuple) -> FileResult:
    """For a process pool: ``(TranscriptFile fields, Context fields, roots)``."""
    fdict, cdict, roots = args
    ctx = Context(cdict["workspace"], cdict["home"], tuple(cdict["fsldirs"]), cdict["freesurfer"])
    return parse_file(TranscriptFile(**fdict), ctx, roots)
