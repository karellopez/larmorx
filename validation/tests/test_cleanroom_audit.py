# SPDX-License-Identifier: Apache-2.0
"""The clean-room audit's reader (``python -m larmorx_validation cleanroom-audit scan``), on small
synthetic transcripts: each kind of tool call, Bash commands parsed for paths, ``cd`` handling, a
restricted access found, a summarised (compacted) session, copies of one session in two roots, a
long output kept in ``tool-results``, refused calls, commits, other agents' scratch, the
restriction table's dates."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path

import pytest
from larmorx_validation.cleanroom_audit import accesses as acc
from larmorx_validation.cleanroom_audit import cli, shell
from larmorx_validation.cleanroom_audit import restrictions as rs

WS = "/home/u/ws"
HOME = "/home/u"
CTX = rs.Context(WS, HOME, (f"{HOME}/fsl",))
WT = f"{WS}/larmorx/.claude/worktrees/agent-a1"
SCRATCH = "/tmp/claude-1000/-home-u-ws/s1/scratchpad"


def bash(command: str, output: str = "", cwd: str = WT) -> list[tuple[str, str, str, str | None]]:
    """(op, shown location, category, rule) of each access of one Bash call."""
    res = acc.call_accesses("Bash", {"command": command}, output, cwd, CTX)
    out = []
    for a in res.accesses:
        loc = rs.locate(a.path, CTX)
        cls = rs.Class("run", None, a.hint) if a.op == "exec" and a.hint else rs.classify(loc, a.op)
        out.append((a.op, loc.show(), cls.category, cls.rule))
    return out


# -- Bash commands parsed for paths ------------------------------------------------------------


@pytest.mark.parametrize(
    ("command", "output", "expected"),
    [
        (
            "cat docs/findings/fsl-bet.md | head -20",
            "",
            [("read", "docs/findings/fsl-bet.md", "restricted", "R11")],
        ),
        (
            "sed -n '10,40p' crates-nc/larmorx-nc-bet/src/lib.rs",
            "",
            [("read", "crates-nc/larmorx-nc-bet/src/lib.rs", "restricted", "R10")],
        ),
        (
            "grep -rn tanh docs/ | head",
            "docs/findings/fsl-bet.md:12:x tanh\ndocs/api/bet.md:3:tanh",
            [
                ("search", "docs", "repo", None),
                ("search-hit", "docs/findings/fsl-bet.md", "restricted", "R11"),
                ("search-hit", "docs/api/bet.md", "repo", None),
            ],
        ),
        (
            "git show HEAD~2:crates-nc/fsl-source/fsl-bet2/bet2.cc | head",
            "",
            [("read", "crates-nc/fsl-source/fsl-bet2/bet2.cc", "restricted", "R01")],
        ),
        (
            "git show abc1234 --stat",
            "diff --git a/crates/larmorx-fsl/src/a.rs b/crates/larmorx-fsl/src/a.rs\n",
            [("diff-shown", "crates/larmorx-fsl/src/a.rs", "cleanroom", None)],
        ),
        (
            "find ~/fsl/src/fsl-bet2 -name '*.cc' -exec grep -l mask {} \\;",
            "",
            [
                ("list", "<FSLDIR>/src/fsl-bet2", "restricted-names", "R01"),
                ("search", "<FSLDIR>/src/fsl-bet2", "restricted", "R01"),
            ],
        ),
        (
            "cp -r crates-nc/larmorx-nc-fast /tmp/x && echo done > notes.txt 2>&1",
            "",
            [
                ("copy", "crates-nc/larmorx-nc-fast", "restricted", "R10"),
                ("write", "/tmp/x", "system", None),
                ("write", "notes.txt", "repo", None),
            ],
        ),
        (
            "python3 - <<'EOF'\nprint(open('/home/u/ws/afni/src/thd_shift2.c').read())\nEOF",
            "",
            [("script-path", "<workspace>/afni/src/thd_shift2.c", "restricted", "R02")],
        ),
        (
            "scripts/orchestration/boxed.sh 24 bash -c 'cat /home/u/ws/reference_src/workbench/a.cxx'",
            "",
            [("read", "<workspace>/reference_src/workbench/a.cxx", "restricted", "R04")],
        ),
        (
            "for f in fsl-flirt.md fsl-flirt-original.md; do head -5 docs/findings/$f; done",
            "",
            [
                ("read", "docs/findings/fsl-flirt.md", "restricted", "R11"),
                ("read", "docs/findings/fsl-flirt-original.md", "repo", None),
            ],
        ),
        (
            'S=/tmp/claude-1000/-home-u-ws/s1/scratchpad/fsl-bet; ls $S && cat "$S/NOTES.md"',
            "",
            [
                ("list", "<scratch>/fsl-bet", "scratch", None),
                ("read", "<scratch>/fsl-bet/NOTES.md", "scratch", None),
            ],
        ),
        (
            "curl -sL https://git.fmrib.ox.ac.uk/fsl/bet2/-/raw/master/bet2.cc -o /tmp/b.cc",
            "",
            [
                (
                    "web",
                    "https://git.fmrib.ox.ac.uk/fsl/bet2/-/raw/master/bet2.cc",
                    "restricted",
                    "R01",
                ),
                ("write", "/tmp/b.cc", "system", None),
            ],
        ),
        (
            "$FSLDIR/bin/bet in.nii.gz out -f 0.3 && cat ~/fsl/bin/bet",
            "",
            [
                ("exec", "<FSLDIR>/bin/bet", "run", None),
                ("mention", "in.nii.gz", "repo", None),
                ("read", "<FSLDIR>/bin/bet", "restricted", "R01"),
            ],
        ),
        ("3dTshift -help", "", [("exec", "3dTshift", "run", None)]),
        (
            "cargo test --manifest-path crates-nc/Cargo.toml 2>&1 | tail -3",
            "error[E0308]: mismatched types\n  --> larmorx-nc-bet/src/x.rs:3:5\n",
            [
                ("build", "crates-nc/Cargo.toml", "restricted-build", "R10"),
                ("build", ".", "repo", None),
                ("mention", "crates-nc/Cargo.toml", "replica-metadata", "R10"),
                ("diagnostic", "crates-nc/larmorx-nc-bet/src/x.rs", "restricted", "R10"),
            ],
        ),
        (
            "rg -n prelude ~/fsl/src",
            "",
            [("search", "<FSLDIR>/src", "restricted", "R01")],
        ),
        (
            "sed -i 's/a/b/' crates/larmorx-fsl/src/bet.rs",
            "",
            [("edit", "crates/larmorx-fsl/src/bet.rs", "cleanroom", None)],
        ),
        (
            "tar xzf /home/u/ws/tags/glibc-2.35.tar.gz -C /tmp/g",
            "",
            [
                ("copy", "<workspace>/tags/glibc-2.35.tar.gz", "restricted", "R06"),
                ("write", "/tmp/g", "system", None),
            ],
        ),
        (
            "wc -l docs/parts/2026-11-02-fsl-fast/report.md",
            "",
            [("stat", "docs/parts/2026-11-02-fsl-fast/report.md", "restricted-names", "R13")],
        ),
    ],
)
def test_bash_paths(command, output, expected):
    assert bash(command, output) == expected


def test_register_lines_from_sed_head_and_tail():
    res = acc.call_accesses(
        "Bash",
        {
            "command": "sed -n '41,120p' docs/findings/upstream-defects.md; tail -n +3800 "
            "docs/findings/upstream-defects.md; head -50 docs/findings/upstream-defects.md"
        },
        "",
        WT,
        CTX,
    )
    assert [a.lines for a in res.accesses] == [((41, 120),), ((3800, None),), ((1, 50),)]
    loc = rs.locate(res.accesses[0].path, CTX)
    assert rs.classify(loc, "read").category == "register"


def test_grep_of_one_file_attributes_its_hits():
    res = acc.call_accesses(
        "Bash",
        {"command": "grep -n Restricted docs/findings/upstream-defects.md"},
        "12:x\n3789:y",
        WT,
        CTX,
    )
    hits = [a for a in res.accesses if a.op == "search-hit"]
    assert len(hits) == 1 and hits[0].hits == (12, 3789)


def test_hits_outside_the_searched_files_are_not_reads():
    # A program's message quoting a script's path and line, found in a log by a search.
    out = "/home/u/fsl/bin/standard_space_roi: 12: bad option"
    assert bash("grep -n 'bad option' /tmp/run/stdout", out) == [
        ("search", "/tmp/run/stdout", "system", None)
    ]


def test_git_diff_stat_shows_names_only():
    assert bash("git diff --stat main -- crates-nc") == [
        ("stat", "crates-nc", "restricted-names", "R10")
    ]


def test_replica_metadata_and_build_outputs():
    got = bash(
        "grep -n larmorx crates-gpl/Cargo.lock; cp NOTICE crates-gpl/LICENSES/NOTICE-larmorx.txt"
    )
    assert [g[2] for g in got] == ["replica-metadata", "repo", "replica-metadata"]
    got = bash("LARMORX_NC_BIN=/tmp/x/target-nc/release/larmorx-nc python3 run.py")
    assert ("exec", "run.py", "repo", None) in got


def test_a_names_only_search_names_files():
    out = "docs/findings/fsl-bet.md\ndocs/findings/mri-hmc-original.md\n"
    assert bash("grep -rln mri-hmc docs/findings/*.md", out) == [
        ("search", "docs/findings/*.md", "restricted", "R11"),
        ("search-match", "docs/findings/fsl-bet.md", "restricted-names", "R11"),
        ("search-match", "docs/findings/mri-hmc-original.md", "repo", None),
    ]


def test_a_search_of_a_pipe_opens_nothing():
    assert bash("cat specs/bet.md | grep -n mask", "12:mask") == [
        ("read", "specs/bet.md", "spec", None)
    ]


# -- cd handling -------------------------------------------------------------------------------


def test_cd_changes_relative_paths_and_subshells_do_not_leak():
    got = bash("(cd crates-nc && cat larmorx-nc-core/src/lib.rs); cat specs/fast.md")
    assert got == [
        ("read", "crates-nc/larmorx-nc-core/src/lib.rs", "restricted", "R10"),
        ("read", "specs/fast.md", "spec", None),
    ]
    got = bash("cd /home/u/ws/tags/AFNI-26.3.05/src && grep -n slice 3dTshift.c", "88:slice")
    assert got[0] == ("search", "<workspace>/tags/AFNI-26.3.05/src/3dTshift.c", "restricted", "R02")
    assert got[1][0] == "search-hit" and got[1][2] == "restricted"
    got = bash("pushd docs/findings && cat fsl-fast.md && popd && cat README.md")
    assert got == [
        ("read", "docs/findings/fsl-fast.md", "restricted", "R11"),
        ("read", "README.md", "repo", None),
    ]


def test_git_C_and_repository_root():
    got = bash(f"git -C {WS}/larmorx show main:docs/findings/fsl-topup.md", cwd="/tmp")
    assert got == [("read", "docs/findings/fsl-topup.md", "restricted", "R11")]
    loc = rs.locate(
        acc.call_accesses("Read", {"file_path": f"{WS}/larmorx/x.md"}, "", "/", CTX)
        .accesses[0]
        .path,
        CTX,
    )
    assert (loc.area, loc.checkout) == ("larmorx", "main")


def test_shell_reader_pieces():
    st = shell.State("/w", "/h")
    cmds = list(shell.simple_commands('X=/a; ls "$X/b" ~/c $(cat /d) `head /e` 2>/dev/null', st))
    words = [c.words for c in cmds]
    assert ["cat", "/d"] in words and ["head", "/e"] in words
    assert ["ls", "/a/b", "/h/c", "$(cat /d)", "$(head /e)"] in words
    cmds = list(shell.simple_commands("cat <<'EOF' > /out\nline $X\nEOF\necho next", st))
    assert cmds[0].redirs == [("<<", "EOF", "line $X"), (">", "/out", None)]
    assert cmds[1].words == ["echo", "next"]


# -- transcripts ---------------------------------------------------------------------------------


class Session:
    """A synthetic transcript: calls and their results as Claude Code writes them."""

    def __init__(self, session: str = "s1", agent: str | None = None, cwd: str = WT):
        self.records: list[dict] = []
        self.session, self.agent, self.cwd = session, agent, cwd
        self.n = 0

    def _base(self, kind: str, ts: str) -> dict:
        self.n += 1
        r = {"type": kind, "uuid": f"u{self.n}", "timestamp": ts, "sessionId": self.session}
        if self.agent:
            r["agentId"] = self.agent
        return r

    def prompt(self, text: str, ts: str = "2026-11-06T10:00:00Z") -> Session:
        r = self._base("user", ts)
        r["message"] = {"role": "user", "content": text}
        self.records.append(r)
        return self

    def call(
        self,
        name: str,
        inp: dict,
        result: str = "",
        ts: str = "2026-11-06T10:01:00Z",
        cwd: str | None = None,
        error: bool = False,
    ) -> Session:
        cid = f"toolu_{self.session}_{self.agent}_{self.n}"
        r = self._base("assistant", ts)
        r["cwd"] = cwd or self.cwd
        r["message"] = {
            "role": "assistant",
            "content": [{"type": "tool_use", "id": cid, "name": name, "input": inp}],
        }
        u = self._base("user", ts)
        u["message"] = {
            "role": "user",
            "content": [
                {"type": "tool_result", "tool_use_id": cid, "content": result, "is_error": error}
            ],
        }
        self.records += [r, u]
        return self

    def compact(self, ts: str) -> Session:
        r = self._base("system", ts)
        r.update({"subtype": "compact_boundary", "logicalParentUuid": f"u{self.n - 1}"})
        self.records.append(r)
        s = self._base("user", ts)
        s["message"] = {
            "role": "user",
            "content": "This session is being continued from a "
            "previous conversation that ran out of context.",
        }
        s["isCompactSummary"] = True
        self.records.append(s)
        return self

    def write(self, root: Path, meta: dict | None = None) -> Path:
        if self.agent:
            d = root / self.session / "subagents"
            d.mkdir(parents=True, exist_ok=True)
            p = d / f"agent-{self.agent}.jsonl"
            if meta is not None:
                (d / f"agent-{self.agent}.meta.json").write_text(json.dumps(meta))
        else:
            root.mkdir(parents=True, exist_ok=True)
            p = root / f"{self.session}.jsonl"
        p.write_text("".join(json.dumps(r) + "\n" for r in self.records), encoding="utf-8")
        return p


def scan(tmp_path: Path, roots: list[Path], monkeypatch) -> tuple[dict, dict, dict]:
    monkeypatch.delenv("FSLDIR", raising=False)
    out = tmp_path / "out"
    ns = argparse.Namespace(
        audit_command="scan",
        out=str(out),
        root=[str(r) for r in roots],
        workspace=WS,
        home=HOME,
        jobs=1,
    )
    assert cli.main(ns) == 0
    summary = json.loads((out / "summary.json").read_text())
    agents = json.loads((out / "agents.json").read_text())
    logs = {}
    for k, info in agents.items():
        with gzip.open(out / info["access_log"], "rt", encoding="utf-8") as fh:
            logs[k] = [json.loads(line) for line in fh]
    return summary, agents, logs


def test_each_tool_kind(tmp_path, monkeypatch):
    s = Session(agent="a1").prompt("Implement BET's clean-room original from specs/bet.md.")
    s.call("Read", {"file_path": f"{WT}/specs/bet.md"}, "     1\t# BET\n     2\tx")
    s.call("Write", {"file_path": f"{WT}/crates/larmorx-fsl/src/bet.rs", "content": "fn x() {}"})
    s.call(
        "Edit",
        {"file_path": f"{WT}/crates/larmorx-fsl/src/bet.rs", "old_string": "x", "new_string": "y"},
    )
    s.call("MultiEdit", {"file_path": f"{WT}/crates/larmorx-fsl/src/lib.rs", "edits": []})
    s.call("NotebookEdit", {"notebook_path": f"{WT}/n.ipynb", "new_source": ""})
    s.call(
        "Grep",
        {"pattern": "tanh", "path": f"{WT}/docs", "output_mode": "content"},
        f"{WT}/docs/findings/fsl-bet.md:3:tanh\n{WT}/docs/api/bet.md:9:tanh",
    )
    s.call("Grep", {"pattern": "mask", "path": f"{WT}/specs"}, f"Found 1 file\n{WT}/specs/bet.md")
    s.call(
        "Glob", {"pattern": "**/*.md", "path": f"{WT}/docs/parts"}, f"{WT}/docs/parts/p/brief.md"
    )
    s.call("LS", {"path": f"{WT}/crates"})
    s.call(
        "WebFetch",
        {"url": "https://fsl.fmrib.ox.ac.uk/fsl/docs/structural/bet.html", "prompt": "options"},
    )
    s.call("Bash", {"command": "ls crates-nc"})
    s.write(
        tmp_path / "root",
        meta={"description": "BET clean-room", "toolUseId": "t0", "worktreePath": WT},
    )
    summary, agents, logs = scan(tmp_path, [tmp_path / "root"], monkeypatch)
    log = logs["s1/agent-a1"]
    kinds = [(r["tool"], r["op"], r["category"]) for r in log]
    assert ("Read", "read", "spec") in kinds
    assert ("Write", "write", "cleanroom") in kinds
    assert ("Edit", "edit", "cleanroom") in kinds
    assert ("MultiEdit", "edit", "cleanroom") in kinds
    assert ("NotebookEdit", "edit", "repo") in kinds
    assert ("Grep", "search-hit", "restricted") in kinds  # the slip: lines of fsl-bet.md
    assert ("Grep", "search-match", "spec") in kinds
    assert ("Glob", "list", "restricted-names") in kinds
    assert ("LS", "list", "repo") in kinds
    assert ("WebFetch", "web", "docs") in kinds
    assert ("Bash", "list", "restricted-names") in kinds
    assert summary["agents"] == 1 and summary["sessions"] == 1
    assert agents["s1/agent-a1"]["brief"].startswith("Implement BET")
    assert summary["restricted_by_rule"]["R11"] == 1


def test_a_slip_is_found_with_its_dates(tmp_path, monkeypatch):
    s = Session(agent="a2").prompt("Clean-room FLIRT.")
    # Before fsl-flirt.md existed, before the rule named it, after both.
    for ts in ("2026-11-01T10:00:00Z", "2026-11-01T12:00:00Z", "2026-11-01T14:30:00Z"):
        s.call("Read", {"file_path": f"{WT}/docs/findings/fsl-flirt.md"}, "1\tx", ts=ts)
    s.write(tmp_path / "root", meta={"toolUseId": "t1"})
    summary, _, logs = scan(tmp_path, [tmp_path / "root"], monkeypatch)
    log = logs["s1/agent-a2"]
    assert [(r["rule"], r["in_force"], r["named"]) for r in log] == [
        ("R11", False, False),
        ("R11", True, False),
        ("R11", True, True),
    ]
    assert summary["restricted_by_rule"] == {"R11": 3}
    assert summary["restricted_by_rule_after_named"] == {"R11": 1}
    assert log[0]["command"] == ""  # a Read: no command; Bash accesses keep theirs verbatim


def test_compaction_keeps_both_halves(tmp_path, monkeypatch):
    s = Session().prompt("orchestrate", ts="2026-10-20T08:00:00Z")
    s.call(
        "Bash",
        {"command": "cat docs/next-steps.md"},
        ts="2026-10-20T08:01:00Z",
        cwd=f"{WS}/larmorx",
    )
    s.compact("2026-10-20T09:00:00Z")
    s.call("Bash", {"command": "cat CLAUDE.md"}, ts="2026-10-20T09:01:00Z", cwd=f"{WS}/larmorx")
    s.write(tmp_path / "root")
    summary, agents, logs = scan(tmp_path, [tmp_path / "root"], monkeypatch)
    assert [r["rel"] for r in logs["s1/main"]] == ["docs/next-steps.md", "CLAUDE.md"]
    assert summary["gaps"]["compactions"] == 1
    assert summary["gaps"]["compaction_parent_missing"] == 0
    assert agents["s1/main"]["role_hint"] == "orchestrator"


def test_an_agent_reading_its_own_transcript(tmp_path, monkeypatch):
    own = f"{HOME}/.claude/projects/-home-u-ws/s1/subagents/agent-a10.jsonl"
    other = f"{HOME}/.claude/projects/-home-u-ws/s1/subagents/agent-a11.jsonl"
    s = Session(agent="a10").prompt("x")
    s.call("Bash", {"command": f"grep -c bench {own}; grep -c bench {other}"}, "3")
    s.write(tmp_path / "root", meta={"toolUseId": "t10"})
    _, _, logs = scan(tmp_path, [tmp_path / "root"], monkeypatch)
    cats = [r["category"] for r in logs["s1/agent-a10"]]
    assert cats == ["own-transcript", "restricted"]  # -c prints counts: no lines, no hit


def test_the_orchestrator_cwd_comes_from_each_record(tmp_path, monkeypatch):
    s = Session().prompt("orchestrate")
    s.call("Bash", {"command": "cat a.md"}, cwd=f"{WS}/larmorx")
    s.call("Bash", {"command": "cat a.md"}, cwd=f"{WS}/larmorx_documentation")
    s.write(tmp_path / "root")
    _, _, logs = scan(tmp_path, [tmp_path / "root"], monkeypatch)
    assert [r["show"] for r in logs["s1/main"]] == ["a.md", "site:a.md"]


def test_copies_in_two_roots_are_counted_once(tmp_path, monkeypatch):
    short = Session(agent="a3").prompt("x").call("Bash", {"command": "cat specs/bet.md"})
    p1 = short.write(tmp_path / "arch" / "main-machine", meta={"toolUseId": "t3"})
    long = Session(agent="a3").prompt("x").call("Bash", {"command": "cat specs/bet.md"})
    long.call("Bash", {"command": "cat specs/fast.md"})
    long.write(tmp_path / "live", meta={"toolUseId": "t3"})
    rel = p1.relative_to(tmp_path / "arch")
    digest = hashlib.sha256(p1.read_bytes()).hexdigest()
    (tmp_path / "arch" / "SHA256SUMS").write_text(f"{digest}  ./{rel.as_posix()}\n")
    summary, _, logs = scan(
        tmp_path, [tmp_path / "arch" / "main-machine", tmp_path / "live"], monkeypatch
    )
    assert [r["rel"] for r in logs["s1/agent-a3"]] == ["specs/bet.md", "specs/fast.md"]
    assert summary["tool_calls"] == 2 and summary["transcript_files"] == 2
    assert summary["archive_checksums_matched"] == 1


def test_a_persisted_output_is_read_from_tool_results(tmp_path, monkeypatch):
    root = tmp_path / "root"
    tr_dir = root / "s1" / "tool-results"
    tr_dir.mkdir(parents=True)
    (tr_dir / "b1.txt").write_text("docs/findings/fsl-topup.md:5:x\n" * 3)
    stub = (
        "<persisted-output>\nOutput too large (40KB). Full output saved to: "
        f"{HOME}/.claude/projects/-home-u-ws/s1/tool-results/b1.txt\n\nPreview (first 2KB):\n"
    )
    s = Session(agent="a4").prompt("x").call("Bash", {"command": "grep -rn x docs"}, stub)
    s.write(root, meta={"toolUseId": "t4"})
    summary, _, logs = scan(tmp_path, [root], monkeypatch)
    hits = [r for r in logs["s1/agent-a4"] if r["op"] == "search-hit"]
    assert hits and hits[0]["rule"] == "R11" and hits[0]["hits"] == [5, 5, 5]
    assert summary["gaps"].get("persisted_output_missing", 0) == 0


def test_refused_calls_are_flagged_not_counted(tmp_path, monkeypatch):
    s = Session(agent="a5").prompt("x")
    s.call(
        "Bash",
        {"command": "cat crates-nc/README.md"},
        "This agent is isolated in the worktree ... Refusing to run it.",
        error=True,
    )
    s.write(tmp_path / "root", meta={"toolUseId": "t5"})
    summary, _, logs = scan(tmp_path, [tmp_path / "root"], monkeypatch)
    assert logs["s1/agent-a5"][0]["refused"] is True
    assert summary["restricted_by_rule"] == {}
    assert summary["gaps"]["refused_calls"] == 1


def test_commits_printed_and_quiet_commits(tmp_path, monkeypatch):
    s = Session(agent="a6").prompt("x")
    s.call("Bash", {"command": "git commit -m 'one'"}, "[worktree-agent-a6 1a2b3c4] one\n 1 file")
    s.call("Bash", {"command": "git commit -q -m 'two' && git log --oneline -1"}, "5d6e7f8 two\n")
    s.write(tmp_path / "root", meta={"toolUseId": "t6"})
    out = tmp_path / "out"
    scan(tmp_path, [tmp_path / "root"], monkeypatch)
    commits = [json.loads(line) for line in (out / "commits.jsonl").read_text().splitlines()]
    assert [(c["branch"], c["hash"], c["inferred"]) for c in commits] == [
        ("worktree-agent-a6", "1a2b3c4", False),
        ("", "5d6e7f8", True),
    ]
    assert commits[0]["repo"] == WT


def test_other_agents_scratch_is_restricted(tmp_path, monkeypatch):
    root = tmp_path / "root"
    rep = Session(agent="r1").prompt("FAST replica")
    rep.call(
        "Write",
        {"file_path": f"{SCRATCH}/fast-replica/NOTES.md", "content": "x"},
        ts="2026-11-02T09:00:00Z",
    )
    rep.write(root, meta={"toolUseId": "t8"})
    s = Session(agent="a7").prompt(f"Read your brief: {SCRATCH}/fast-cr/BRIEF.md")
    s.call("Read", {"file_path": f"{SCRATCH}/fast-cr/BRIEF.md"}, "1\tbrief")
    s.call("Read", {"file_path": f"{SCRATCH}/fast-replica/NOTES.md"}, "1\tnotes")
    s.call("Write", {"file_path": f"{SCRATCH}/fast-cr2/NOTES.md", "content": "x"})
    s.call("Read", {"file_path": f"{SCRATCH}/fast-cr2/NOTES.md"}, "1\tx")
    s.call("Read", {"file_path": f"{SCRATCH}/unclaimed/out.txt"}, "1\tx")
    s.write(root, meta={"toolUseId": "t7"})
    _, agents, logs = scan(tmp_path, [root], monkeypatch)
    reads = [r for r in logs["s1/agent-a7"] if r["op"] == "read"]
    assert [(r["rel"], r["category"], r["rule"]) for r in reads] == [
        ("fast-cr/BRIEF.md", "scratch", None),
        ("fast-replica/NOTES.md", "restricted", "R18"),
        ("fast-cr2/NOTES.md", "scratch", None),
        ("unclaimed/out.txt", "scratch", None),
    ]
    assert reads[1]["writers"] == ["s1/agent-r1"]
    assert agents["s1/agent-a7"]["own_scratch"] == ["fast-cr", "fast-cr2"]


def test_launches_and_agents_are_linked(tmp_path, monkeypatch):
    root = tmp_path / "root"
    main = Session().prompt("go")
    a8, a9, b1 = "a0123456789abcdef", "a1111111111111111", "b2222222222222222"
    main.call(
        "Agent", {"description": "BET clean-room", "prompt": "brief"}, f"agentId: {a8} (launched)"
    )
    main.call("Agent", {"description": "lost", "prompt": "brief"}, f"agentId: {a9} (launched)")
    main.write(root)
    launch_id = main.records[1]["message"]["content"][0]["id"]
    Session(agent=a8).prompt("brief").write(root, meta={"toolUseId": launch_id})
    Session(agent=b1).prompt("brief").write(root, meta={"toolUseId": "toolu_elsewhere"})
    summary, agents, _ = scan(tmp_path, [root], monkeypatch)
    assert agents[f"s1/agent-{a8}"]["launched_by"] == "s1/main"
    assert summary["gaps"]["launches_whose_transcript_is_missing"] == 1
    assert summary["gaps"]["agents_whose_launch_is_missing"] == 1


# -- the restriction table's dates ---------------------------------------------------------------


@pytest.mark.parametrize(
    ("rel", "when", "expected"),
    [
        ("docs/findings/fsl-bet.md", "2026-10-24T16:00:00Z", (False, False)),
        ("docs/findings/fsl-bet.md", "2026-10-25T00:00:00Z", (True, False)),
        ("docs/findings/fsl-bet.md", "2026-10-26T05:00:00Z", (True, True)),
        ("crates/larmorx-afni-replica/src/lib.rs", "2026-10-29T12:00:00Z", (False, False)),
        ("crates/larmorx-afni-replica/src/lib.rs", "2026-10-30T01:00:00Z", (True, True)),
        ("docs/parts/x/brief.md", "2026-10-23T12:00:00Z", (False, False)),
        ("docs/parts/x/brief.md", "2026-10-23T18:00:00Z", (True, True)),
        ("crates-nc/larmorx-nc-bet/src/lib.rs", "2026-10-09T12:00:00Z", (True, False)),
        ("crates-nc/larmorx-nc-bet/src/lib.rs", "2026-10-10T00:00:00Z", (True, True)),
    ],
)
def test_restriction_dates(rel, when, expected):
    loc = rs.locate(f"{WT}/{rel}", CTX)
    cls = rs.classify(loc, "read")
    assert cls.rule is not None
    assert rs.in_force(cls, loc.rel, when) == expected


def test_glibc_source_dates_and_the_rules_table():
    loc = rs.locate(f"{WS}/tags/glibc-2.35/sysdeps/ieee754/flt-32/s_cbrtf.c", CTX)
    cls = rs.classify(loc, "read")
    assert (cls.rule, cls.family) == ("R06", "GNU")
    assert rs.in_force(cls, loc.rel, "2026-10-29T12:00:00Z") == (False, False)
    assert rs.in_force(cls, loc.rel, "2026-11-05T12:00:00Z") == (True, True)
    for rule in rs.RULES.values():
        assert rs._ge(rule.named, rule.since), rule.id
    assert set(rs.SOURCE_FINDINGS) >= {"fsl-bet.md", "fsl-maths.md", "afni-tshift.md"}


@pytest.mark.parametrize(
    ("path", "op", "category", "rule"),
    [
        (f"{HOME}/fsl/doc/bet.html", "read", "docs", None),
        (f"{HOME}/fsl/bin/flirt", "exec", "run", None),
        (f"{HOME}/fsl-6.0.7.23/include/newimage/newimage.h", "read", "restricted", "R01"),
        (f"{HOME}/fsl/lib/python3.12/site-packages/fsl/scripts/x.py", "read", "restricted", "R01"),
        (f"{WS}/afni/doc/README/README.copyright", "read", "docs", None),
        (f"{WS}/ANTs/Examples/antsRegistration.cxx", "read", "permissive-source", None),
        (
            f"{WS}/archive/2026-11-05-session-transcripts/main-machine/x.jsonl",
            "read",
            "restricted",
            "R17",
        ),
        (f"{HOME}/.claude/projects/-home-u-ws/s1.jsonl", "read", "restricted", "R17"),
        (
            f"{WS}/archive/2026-11-02-fsl-fast-replica-scratch-scripts/a.py",
            "read",
            "restricted",
            "R18",
        ),
        (f"{WS}/oracles/afni-26.3.05/bin/3dTshift", "exec", "run", None),
        (f"{WS}/oracles/afni-26.3.05/build/src/3dTshift.c", "read", "restricted", "R02"),
        (f"{WS}/tedana/tedana/workflows/t2smap.py", "read", "restricted", "R07"),
        (f"{WS}/tags/tedana-26.0.3/tedana/decay.py", "read", "restricted", "R07"),
        (f"{WS}/.venv/lib/python3.12/site-packages/tedana/combine.py", "read", "restricted", "R07"),
        (
            "/opt/conda/envs/fmriprep/lib/python3.12/site-packages/tedana/io.py",
            "read",
            "restricted",
            "R07",
        ),
        (f"{WT}/crates-gpl/larmorx-gpl-tedana/src/decay.rs", "read", "restricted", "R10"),
        (f"{WT}/docs/findings/tedana-t2smap.md", "read", "restricted", "R11"),
        ("/usr/include/c++/11/bits/stl_algo.h", "read", "restricted", "R06"),
        (f"{WT}/validation/probes/fsl-miscmaths/a.cc", "read", "restricted", "R14"),
        (f"{WT}/validation/probes/ants-ai-blobs/a.py", "read", "probe-permissive", None),
        (f"{WT}/docs/findings/fsl-newtool.md", "read", "findings-unlisted", None),
        (f"{WT}/docs/findings/fsl-prelude.md", "read", "restricted", "R11"),
        (f"{WT}/docs/findings/fsl-bet-original.md", "read", "repo", None),
        (f"{WT}/docs/findings/fsl-bet-clean-room.md", "read", "repo", None),
        (f"{WT}/docs/findings/afni-volreg-clean-room.md", "read", "repo", None),
        (f"{SCRATCH}/x/larmorx_documentation/content/a.md", "read", "repo-site", None),
        (f"{SCRATCH}/x/clone/crates-nc/fsl-source/a.cc", "read", "restricted", "R01"),
        (f"{HOME}/.claude/projects/-home-u-ws/s1/tool-results/b1.txt", "read", "tool-output", None),
        (f"{HOME}/.claude/projects/-home-u-ws/memory/MEMORY.md", "read", "memory", None),
        ("/tmp/claude-1000/-home-u-ws/s1/tasks/b2.output", "read", "task-output", None),
        (f"{WS}/oracles/afni-25.2.09/bin/3dTshift", "script-path", "run", None),
        (f"{HOME}/fsl/bin/mcflirt", "script-path", "run", None),
        (f"{WS}/oracles/ants-2.6.5-src/Examples/a.cxx", "mention", "permissive-source", None),
        (f"{WT}/crates-nc/target/release/larmorx-nc", "copy", "replica-build-output", "R10"),
        (f"{SCRATCH}/t/nc-target/release/larmorx-nc", "exec", "replica-build-output", "R10"),
        # FreeSurfer (2026-11-12)
        (
            f"{WS}/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/bin/recon-all",
            "read",
            "restricted",
            "R03",
        ),
        (
            f"{WS}/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/bin/mri_convert",
            "exec",
            "run",
            None,
        ),
        (
            f"{WS}/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/python/packages/fsbindings/a.py",
            "read",
            "restricted",
            "R03",
        ),
        (
            f"{WS}/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/SetUpFreeSurfer.sh",
            "read",
            "restricted",
            "R03",
        ),
        (
            f"{WS}/oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0/average/RB_all_2019_10_25.talxfm.mni305.gca",
            "read",
            "freesurfer-install",
            None,
        ),
        (
            f"{WS}/oracles/freesurfer-8.2.0/runs/a-v8/subjects/s/scripts/recon-all.local-copy",
            "read",
            "restricted",
            "R03",
        ),
        (
            f"{WS}/oracles/freesurfer-8.2.0/runs/a-v8/subjects/s/mri/orig.mgz",
            "read",
            "oracle",
            None,
        ),
        (f"{WT}/crates-freesurfer/larmorx-freesurfer-core/src/mgh.rs", "read", "restricted", "R10"),
        (f"{WT}/docs/freesurfer/cards/mri_coreg.md", "read", "restricted", "R11"),
        (f"{WT}/docs/findings/freesurfer-mri_coreg.md", "read", "restricted", "R11"),
        (
            f"{WT}/validation/investigations/freesurfer-inventory/waves.py",
            "read",
            "restricted",
            "R11",
        ),
        (
            f"{WT}/validation/src/larmorx_validation/freesurfer/replay.py",
            "read",
            "restricted",
            "R11",
        ),
        (f"{WT}/validation/src/larmorx_validation/freesurfer_unit.py", "read", "restricted", "R11"),
        (
            f"{WT}/validation/investigations/freesurfer-unit-oracle/build.sh",
            "read",
            "restricted",
            "R11",
        ),
        (f"{WS}/oracles/freesurfer-8.2.0-unit/src/utils/mri.cpp", "read", "restricted", "R03"),
        (f"{WS}/oracles/freesurfer-8.2.0-unit/bin/fsunit", "exec", "run", None),
    ],
)
def test_classification(path, op, category, rule):
    cls = rs.classify(rs.locate(path, CTX), op)
    assert (cls.category, cls.rule) == (category, rule)


def test_shown_locations_carry_no_account_name():
    for p in (
        f"{WT}/crates/x.rs",
        f"{WS}/afni/a.c",
        f"{HOME}/fsl/src/a.cc",
        f"{SCRATCH}/t/N.md",
        f"{HOME}/.claude/projects/p/s.jsonl",
        f"{HOME}/notes.txt",
    ):
        shown = rs.locate(p, CTX).show()
        assert not shown.startswith("/") and "/home" not in shown, shown
