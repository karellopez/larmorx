# SPDX-License-Identifier: Apache-2.0
"""The clean-room audit's merge step: ``scripts/orchestration/archive_transcripts.py`` (a part's
transcripts copied into the archive with their SHA-256; the part's agents found by id, worktree
and brief) and the wrapper's syntax (``cleanroom-audit-merge.sh``), on a synthetic folder."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

ORCH = Path(__file__).resolve().parents[2] / "scripts" / "orchestration"
S = "11111111-2222-3333-4444-555555555555"
A1, A2, A3, A4 = (f"a{c * 16}" for c in "1234")


@pytest.fixture(scope="module")
def at():
    spec = importlib.util.spec_from_file_location(
        "archive_transcripts", ORCH / "archive_transcripts.py"
    )
    module = importlib.util.module_from_spec(spec)
    sys.modules["archive_transcripts"] = module
    spec.loader.exec_module(module)
    return module


def _transcript(path: Path, prompt: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    recs = [
        {"type": "user", "isMeta": True, "message": {"content": "meta"}},
        {"type": "user", "message": {"content": [{"type": "text", "text": prompt}]}},
        {"type": "assistant", "message": {"content": [{"type": "text", "text": "ok"}]}},
    ]
    path.write_text("".join(json.dumps(r) + "\n" for r in recs), encoding="utf-8")


def _setup(tmp_path: Path) -> tuple[Path, Path]:
    ws, live = tmp_path / "ws", tmp_path / "live"
    sub = live / S / "subagents"
    _transcript(live / f"{S}.jsonl", "the orchestrator")
    _transcript(sub / f"agent-{A1}.jsonl", "a worktree agent")
    (sub / f"agent-{A1}.meta.json").write_text(
        json.dumps(
            {"worktreePath": f"{ws}/larmorx/.claude/worktrees/agent-{A1}", "description": "relay 1"}
        ),
        encoding="utf-8",
    )
    _transcript(
        sub / f"agent-{A2}.jsonl", "Read /tmp/claude-1/x/y/scratchpad/part-x/BRIEF.md first."
    )
    _transcript(sub / f"agent-{A3}.jsonl", "an older agent")
    (live / S / "tool-results").mkdir()
    (live / S / "tool-results" / "out.txt").write_text("long output", encoding="utf-8")
    (live / "memory").mkdir()
    (live / "memory" / "MEMORY.md").write_text("not a transcript", encoding="utf-8")
    old = ws / "archive" / "2026-11-05-session-transcripts"
    target = old / "main-machine" / S / "subagents" / f"agent-{A3}.jsonl"
    target.parent.mkdir(parents=True)
    shutil.copy2(sub / f"agent-{A3}.jsonl", target)
    h = hashlib.sha256(target.read_bytes()).hexdigest()
    (old / "SHA256SUMS").write_text(
        f"{h}  ./main-machine/{S}/subagents/agent-{A3}.jsonl\n", encoding="utf-8"
    )
    return ws, live


def _run(at, ws, live, *extra) -> int:
    return at.main(
        [
            "--part",
            "p1",
            "--workspace",
            str(ws),
            "--live",
            str(live),
            "--date",
            "2026-11-07",
            *extra,
        ]
    )


def test_archive_a_part(at, tmp_path, capsys):
    ws, live = _setup(tmp_path)
    sel = ["--worktree", f"agent-{A1}", "--scratch", "part-x", "--agent", A3]
    assert _run(at, ws, live, *sel, "--dry-run") == 0
    top = ws / "archive" / "2026-11-07-session-transcripts-p1"
    assert not top.exists()  # a dry run copies nothing
    assert _run(at, ws, live, *sel) == 0
    sums = (top / "SHA256SUMS").read_text(encoding="utf-8").splitlines()
    names = sorted(line.split("  ", 1)[1] for line in sums)
    assert names == [
        f"./main-machine/{S}.jsonl",
        f"./main-machine/{S}/subagents/agent-{A1}.jsonl",
        f"./main-machine/{S}/subagents/agent-{A1}.meta.json",
        f"./main-machine/{S}/subagents/agent-{A2}.jsonl",
        f"./main-machine/{S}/tool-results/out.txt",
    ]  # not the memory, not the agent archived before
    for line in sums:
        h, rel = line.split("  ", 1)
        assert hashlib.sha256((top / rel).read_bytes()).hexdigest() == h
    rows = {r["agent"]: r for r in json.loads((top / "part-p1.json").read_text(encoding="utf-8"))}
    assert {i: r["why"] for i, r in rows.items()} == {
        A1: "its worktree",
        A2: "its brief's scratch folder",
        A3: "named",
    }
    assert all(r["status"] == "archived" and r["sha256"] for r in rows.values())
    assert "2026-11-07-session-transcripts-p1" in capsys.readouterr().out
    # Nothing new: nothing copied again; a grown transcript is copied again.
    assert _run(at, ws, live, *sel) == 0
    assert len((top / "SHA256SUMS").read_text(encoding="utf-8").splitlines()) == 5
    with (live / f"{S}.jsonl").open("a", encoding="utf-8") as fh:
        fh.write(json.dumps({"type": "assistant"}) + "\n")
    assert _run(at, ws, live, *sel) == 0
    readme = (top / "README.txt").read_text(encoding="utf-8")
    assert readme.count("files copied") == 2 and "1 files copied" in readme
    assert not list(top.rglob("*.before-*"))  # the grown copy extends the earlier one
    # A transcript rewritten (not only grown): the earlier copy is kept beside the new one.
    _transcript(live / f"{S}.jsonl", "rewritten")
    assert _run(at, ws, live, *sel) == 0
    assert len(list(top.rglob(f"{S}.jsonl.before-*"))) == 1


def test_a_missing_agent_fails(at, tmp_path):
    ws, live = _setup(tmp_path)
    notes = tmp_path / "NOTES.md"
    notes.write_text(f"relay 2 was agent-{A4}; relay 1 {A1}\n", encoding="utf-8")
    assert _run(at, ws, live, "--notes", str(notes)) == 1
    rows = {
        r["agent"]: r["status"]
        for r in json.loads(
            (ws / "archive" / "2026-11-07-session-transcripts-p1" / "part-p1.json").read_text(
                encoding="utf-8"
            )
        )
    }
    assert rows == {A1: "archived", A4: "MISSING"}


def test_bad_part_name(at, tmp_path):
    ws, live = _setup(tmp_path)
    assert at.main(["--part", "../x", "--workspace", str(ws), "--live", str(live)]) == 2


def test_the_wrapper_parses():
    bash = shutil.which("bash")
    if not bash:
        pytest.skip("no bash")
    r = subprocess.run(
        [bash, "-n", str(ORCH / "cleanroom-audit-merge.sh")], capture_output=True, text=True
    )
    assert r.returncode == 0, r.stderr
