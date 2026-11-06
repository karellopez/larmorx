#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Copy a part's session transcripts into the archive, with their SHA-256 (the clean-room audit's
merge step, docs/orchestration.md; docs/validation/clean-room-audit.md).

Claude Code keeps every session's transcript in ``~/.claude/projects/<workspace>/`` (the
orchestrator's ``<session>.jsonl``; each agent's ``<session>/subagents/agent-<id>.jsonl`` and
``.meta.json``; long outputs in ``<session>/tool-results/``) and deletes the oldest after 30 days.
This copies every file there that the archive does not hold yet with the same content (the
part's agents and the orchestrator's growing session among them) into
``<workspace>/archive/<date>-session-transcripts-<part>/<machine>/`` (the layout of
``archive/2026-11-05-session-transcripts/``, which the audit's scan reads by itself), writes
``SHA256SUMS`` and ``README.txt`` there, and checks that every agent of the part has its
transcript archived: the agents named by ``--agent``, by an ``agent-<id>`` or ``a<16 hex>`` in a
``--notes`` file, by a ``--worktree`` in their ``.meta.json``, or by a ``--scratch`` folder named
in their first prompt (relay agents have no worktree of their own).

    python -P scripts/orchestration/archive_transcripts.py --part <name> [--agent ID]...
        [--notes FILE]... [--worktree NAME]... [--scratch NAME]... [--dry-run]

Prints one line per agent of the part and a summary; ``part-<name>.json`` beside SHA256SUMS lists
them. Exit status 0; 1 when an agent of the part has no transcript (the evidence would be
incomplete); 2 on a usage error. Private: the archive holds absolute paths and the account name.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve()
AGENT_FILE = re.compile(r"^agent-(a[0-9a-f]{16})\.jsonl$")
AGENT_IN_TEXT = re.compile(r"(?:agent-)?\b(a[0-9a-f]{16})\b")


def workspace_of(repo: Path) -> Path:
    s = str(repo)
    if "/larmorx/.claude/worktrees/" in s:
        return Path(s.split("/larmorx/.claude/worktrees/")[0])
    return repo.parent


def live_dir(workspace: Path, home: Path) -> Path:
    return home / ".claude" / "projects" / re.sub(r"[^A-Za-z0-9]", "-", str(workspace))


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for block in iter(lambda: fh.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def is_prefix(old: Path, new: Path) -> bool:
    """Whether a file's content starts with an older copy's (a transcript only grows)."""
    n = old.stat().st_size
    if new.stat().st_size < n:
        return False
    h = hashlib.sha256()
    with new.open("rb") as fh:
        left = n
        while left:
            block = fh.read(min(1 << 20, left))
            if not block:
                return False
            h.update(block)
            left -= len(block)
    return h.hexdigest() == sha256(old)


def archived_hashes(archive: Path) -> dict[str, set[str]]:
    """Every archived transcript file: its path below the machine folder -> the SHA-256 of its
    copies (from each archive's SHA256SUMS; files a list does not name are hashed)."""
    out: dict[str, set[str]] = {}
    for top in sorted(archive.glob("*session-transcripts*")):
        listed: dict[str, str] = {}
        sums = top / "SHA256SUMS"
        if sums.is_file():
            for line in sums.read_text(encoding="utf-8").splitlines():
                h, _, rel = line.partition("  ")
                if rel:
                    listed[str((top / rel).resolve())] = h.strip()
        for machine in sorted(p for p in top.iterdir() if p.is_dir()):
            for f in machine.rglob("*"):
                if f.is_file():
                    h = listed.get(str(f.resolve())) or sha256(f)
                    out.setdefault(str(f.relative_to(machine)), set()).add(h)
    return out


def first_prompt(path: Path, limit: int = 200) -> str:
    """The first user message of a transcript (an agent's brief), as text."""
    try:
        with path.open(encoding="utf-8", errors="replace") as fh:
            for k, raw in enumerate(fh):
                if k >= limit:
                    break
                try:
                    r = json.loads(raw)
                except json.JSONDecodeError:
                    continue
                if r.get("type") != "user" or r.get("isMeta"):
                    continue
                c = (r.get("message") or {}).get("content")
                if isinstance(c, list):
                    c = "\n".join(
                        str(b.get("text", ""))
                        for b in c
                        if isinstance(b, dict) and b.get("type") == "text"
                    )
                if isinstance(c, str) and c.strip():
                    return c
    except OSError:
        pass
    return ""


def live_agents(live: Path) -> dict[str, dict]:
    """Every agent in the live folder: id -> its transcript, meta and first prompt."""
    out: dict[str, dict] = {}
    for f in sorted(live.glob("*/subagents/agent-*.jsonl")):
        m = AGENT_FILE.match(f.name)
        if not m:
            continue
        meta_path = f.with_name(f.name[: -len(".jsonl")] + ".meta.json")
        try:
            meta = json.loads(meta_path.read_text(encoding="utf-8")) if meta_path.is_file() else {}
        except json.JSONDecodeError:
            meta = {}
        out[m[1]] = {"transcript": f, "meta": meta}
    return out


def part_agents(agents: dict[str, dict], ids, notes, worktrees, scratches) -> dict[str, str]:
    """The part's agents: id -> why it belongs to the part."""
    want: dict[str, str] = {i.removeprefix("agent-"): "named" for i in ids}
    for n in notes:
        for i in AGENT_IN_TEXT.findall(Path(n).read_text(encoding="utf-8", errors="replace")):
            want.setdefault(i, f"named in {Path(n).name}")
    for i, a in agents.items():
        meta = a["meta"]
        where = f"{meta.get('worktreePath', '')} {meta.get('worktreeBranch', '')}"
        if any(w and w in where for w in worktrees):
            want.setdefault(i, "its worktree")
    if scratches:
        rx = re.compile(r"/scratchpad/(?:" + "|".join(map(re.escape, scratches)) + r")(?:/|\b)")
        for i, a in agents.items():
            if i not in want and rx.search(first_prompt(a["transcript"])):
                want[i] = "its brief's scratch folder"
    return want


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--part", required=True, help="the part's name (letters, digits, - and _)")
    ap.add_argument("--agent", action="append", default=[], help="an agent id of the part")
    ap.add_argument("--notes", action="append", default=[], help="a file naming agent ids")
    ap.add_argument("--worktree", action="append", default=[], help="a worktree path or branch")
    ap.add_argument("--scratch", action="append", default=[], help="a scratch folder's name")
    ap.add_argument("--workspace", help="the workspace (default: this checkout's)")
    ap.add_argument("--live", help="Claude Code's folder of this workspace's transcripts")
    ap.add_argument("--machine", default="main-machine", help="the machine folder's name")
    ap.add_argument("--date", default=dt.date.today().isoformat())
    ap.add_argument("--dry-run", action="store_true", help="list, copy nothing")
    a = ap.parse_args(argv)
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", a.part) or not re.fullmatch(
        r"[A-Za-z0-9_.-]+", a.machine
    ):
        print(
            "archive_transcripts: --part and --machine: letters, digits, '.', '-', '_'",
            file=sys.stderr,
        )
        return 2
    workspace = Path(a.workspace) if a.workspace else workspace_of(HERE.parents[2])
    live = Path(a.live) if a.live else live_dir(workspace, Path.home())
    if not live.is_dir():
        print(f"archive_transcripts: no transcripts in {live}", file=sys.stderr)
        return 2
    archive = workspace / "archive"
    known = archived_hashes(archive)
    # a part named as its docs/parts folder (``2026-11-07-prelude-replica``) gives its own date
    m = re.match(r"(\d{4}-\d{2}-\d{2})-(.+)", a.part)
    date, name = (m.group(1), m.group(2)) if m else (a.date, a.part)
    dest_top = archive / f"{date}-session-transcripts-{name}"
    dest = dest_top / a.machine
    copied: list[tuple[str, str, int]] = []
    kept = 0
    for f in sorted(p for p in live.rglob("*") if p.is_file()):
        rel = str(f.relative_to(live))
        if rel.startswith("memory/") or not (rel.endswith((".jsonl", ".json", ".txt"))):
            continue  # the project memory and other files are not transcripts
        h = sha256(f)
        if h in known.get(rel, ()):
            kept += 1
            continue
        copied.append((rel, h, f.stat().st_size))
        if not a.dry_run:
            target = dest / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            if target.is_file() and not is_prefix(target, f):
                # An earlier copy of today's run that the new one does not extend: kept beside it.
                target.rename(target.with_name(f"{target.name}.before-{sha256(target)[:12]}"))
            shutil.copy2(f, target)
            if sha256(target) != h:  # the file grew while it was copied: copy again
                shutil.copy2(f, target)
                h = sha256(target)
                copied[-1] = (rel, h, target.stat().st_size)
            known.setdefault(rel, set()).add(h)
    agents = live_agents(live)
    want = part_agents(agents, a.agent, a.notes, a.worktree, a.scratch)
    rows = []
    missing = 0
    for i, why in sorted(want.items()):
        info = agents.get(i)
        rel = str(info["transcript"].relative_to(live)) if info else None
        archived = None
        if rel is None:
            # Not live any more: is it archived (an older session)?
            hit = next((r for r in known if r.endswith(f"/subagents/agent-{i}.jsonl")), None)
            if hit:
                rel, archived = hit, sorted(known[hit])
        else:
            archived = sorted(known.get(rel, ()))
        status = (
            "archived" if archived else ("would be archived" if a.dry_run and rel else "MISSING")
        )
        missing += status == "MISSING"
        rows.append(
            {
                "agent": i,
                "why": why,
                "description": (info or {}).get("meta", {}).get("description", ""),
                "transcript": rel,
                "sha256": archived or [],
                "status": status,
            }
        )
        print(f"{status:18s} {i}  {rows[-1]['description'][:60]}  ({why})")
    size = sum(n for _, _, n in copied)
    print(
        f"archive_transcripts: {len(copied)} files ({size / 1e6:.1f} MB)"
        f" {'to copy' if a.dry_run else 'copied'} into {dest_top.name}/{a.machine}, {kept} already"
        f" archived; the part: {len(rows)} agents, {missing} missing"
    )
    if a.dry_run:
        return 1 if missing else 0
    if copied:
        dest_top.mkdir(parents=True, exist_ok=True)
        sums = dest_top / "SHA256SUMS"
        lines = dict()
        if sums.is_file():
            for line in sums.read_text(encoding="utf-8").splitlines():
                h, _, rel = line.partition("  ")
                if rel:
                    lines[rel] = h
        for rel, h, _ in copied:
            lines[f"./{a.machine}/{rel}"] = h
        sums.write_text("".join(f"{h}  {r}\n" for r, h in sorted(lines.items())), encoding="utf-8")
        digest = sha256(sums)
        readme = dest_top / "README.txt"
        with readme.open("a", encoding="utf-8") as fh:
            fh.write(
                f"{dt.datetime.now(dt.UTC).isoformat(timespec='seconds')}: {len(copied)} files"
                f" copied from Claude Code's transcripts folder at the merge of the part {a.part}"
                f" (the clean-room audit's merge step, scripts/orchestration/archive_transcripts.py);"
                f" SHA256SUMS now lists {len(lines)} files, its SHA-256 {digest}. Private: local"
                " paths and the account name; never publish.\n"
            )
    if rows:
        dest_top.mkdir(parents=True, exist_ok=True)
        (dest_top / f"part-{a.part}.json").write_text(
            json.dumps(rows, indent=1) + "\n", encoding="utf-8"
        )
    return 1 if missing else 0


if __name__ == "__main__":
    os.umask(0o077)
    sys.exit(main())
