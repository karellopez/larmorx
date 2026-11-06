# SPDX-License-Identifier: Apache-2.0
"""The clean-room audit's public summary and the merge step's gate (``cleanroom-audit summary``,
``cleanroom-audit gate``): the committed page agrees with its committed facts, neither holds a
private string, the facts of a small synthetic dossier (judged and unjudged findings, the
accesses' groups, whose material, the edits by other roles), and the gate's blocking and
to-judge items. When the private dossier the committed facts came from is on this machine, the
facts are extracted from it again and compared."""

from __future__ import annotations

import gzip
import json
from pathlib import Path

import pytest
from larmorx_validation.cleanroom_audit import cli
from larmorx_validation.cleanroom_audit import summary as su

REPO = Path(__file__).resolve().parents[2]
JSON = REPO / su.PUBLIC_JSON
MD = REPO / su.PUBLIC_MD
S1 = "11111111-2222-3333-4444-555555555555"
HOME = "/home/someone"


# --- the committed page ---------------------------------------------------------------------------


def test_the_page_agrees_with_its_facts():
    facts = json.loads(JSON.read_text(encoding="utf-8"))
    assert facts["schema"] == su.SCHEMA
    assert MD.read_text(encoding="utf-8") == su.render(facts), (
        "docs/validation/clean-room-audit.md differs from its facts: run"
        " `python -m larmorx_validation cleanroom-audit summary` (the page is generated)"
    )


def test_the_public_files_hold_nothing_private():
    for path in (JSON, MD):
        assert su.privacy_problems(path.read_text(encoding="utf-8")) == [], path.name


def test_every_committed_item_is_judged():
    facts = json.loads(JSON.read_text(encoding="utf-8"))
    assert all(t["verdict"]["status"] != "review" for t in facts["tools"])
    assert all(t["verdict"]["words"] for t in facts["tools"])
    assert all(x["judged"] for x in facts["slips"] + facts["edits"])
    assert all(f["judged"] for t in facts["tools"] for f in t["findings"])


def test_judgements_file_parses():
    j = su.load_judgements(REPO / su.JUDGEMENTS)
    assert j.access and j.edit and j.verdict and j.limits and j.recommendations
    for a in j.access.values():
        assert a["declared"] in ("yes", "no", "partly", "covered")
        assert a["shown"] and a["nature"] and a["judgement"]


def test_extracted_again_from_the_private_dossier():
    facts = json.loads(JSON.read_text(encoding="utf-8"))
    dossier = cli.default_workspace() / "archive" / "clean-room-audit" / facts["dossier"]
    if not (dossier / "verdicts" / "verdicts.json").is_file():
        pytest.skip("the private dossier is not on this machine")
    scan, verdicts, sim = su.find_dossier_parts(dossier)
    if scan.name != facts["made_from"]["scan"]:
        scan = dossier / facts["made_from"]["scan"]
    again = su.extract(scan, verdicts, sim, su.load_judgements(REPO / su.JUDGEMENTS))
    assert again == facts


# --- privacy --------------------------------------------------------------------------------------


@pytest.mark.parametrize(
    "text",
    [
        "/home/someone/x",
        "see /tmp/claude-1000/a/b",
        "C:\\Users\\RUNNER~1\\x",
        "toolu_01AbCdEf",
        S1,
        "~/.claude/projects/x",
        "agent-a1.jsonl",
        "someone@example.org",
        "on alice-laptop today",
    ],
)
def test_privacy_problems_found(text):
    assert su.privacy_problems(text, names=["alice-laptop"])


@pytest.mark.parametrize(
    "text",
    [
        "<workspace>/archive/clean-room-audit/2026-11-06",
        "$FSLDIR/src/fsl-bet2/bet2.cpp and <FSLDIR>/bin/bet",
        "~/fsl-6.0.7.23/src",
        "crates/larmorx-fsl/src/bet/mesh.rs",
        "`a0313b1bd242cb7eb` line 42, commit 0feafe98f",
        "the alice-laptops are fine",
    ],
)
def test_privacy_problems_allowed(text):
    assert su.privacy_problems(text, names=["alice-laptop"]) == []


# --- small pieces ---------------------------------------------------------------------------------


@pytest.mark.parametrize(
    ("category", "group"),
    [
        ("spec", "the behaviour specs (specs/)"),
        ("cleanroom", "the clean-room code"),
        ("restricted", su.RESTRICTED_GROUP),
        ("restricted-names", su.RESTRICTED_GROUP),
        ("replica-build-output", su.RESTRICTED_GROUP),
        ("run", "the original programs, run or named as black boxes"),
        ("fsl-bin", "the original programs, run or named as black boxes"),
        ("scratch", "the agent's own scratch notes, task outputs and memory"),
        ("something-new", "other"),
    ],
)
def test_group_of(category, group):
    assert su.group_of(category) == group


def test_kind_of():
    assert [su.kind_of(o) for o in ("read", "search-hit", "exec", "build", "edit", "move")] == [
        "seen",
        "seen",
        "run",
        "run",
        "written",
        "written",
    ]


@pytest.mark.parametrize(
    ("r", "scope"),
    [
        ({"rule": "R10", "level": "content", "family": "AFNI", "relevant": True}, "this family"),
        (
            {"rule": "R10", "level": "content", "family": "AFNI", "relevant": False},
            "another family",
        ),
        (
            {"rule": "R01", "level": "content", "family": "FSL", "relevant": False},
            "before the rule applied",
        ),
        (
            {"rule": "R18", "level": "clean-writer", "family": "all", "relevant": False},
            "written by clean-room implementers only",
        ),
        (
            {
                "rule": "R13",
                "level": "names",
                "family": "all",
                "relevant": False,
                "part_kind": "clean-room",
            },
            "written by clean-room implementers only",
        ),
        (
            {
                "rule": "R18",
                "level": "content",
                "family": "all",
                "relevant": True,
                "writer_roles": ["replica author"],
            },
            "this family",
        ),
    ],
)
def test_scope_of(r, scope):
    assert su.scope_of(r, "FSL") == scope


def test_agent_id():
    assert su.agent_id(f"{S1}/agent-a0313b1bd242cb7eb") == "a0313b1bd242cb7eb"
    assert su.agent_id(f"{S1}/main") == "orchestrator"


# --- a synthetic dossier --------------------------------------------------------------------------

A1, A2, A3 = "a1aaaaaaaaaaaaaaa", "a2bbbbbbbbbbbbbbb", "a3ccccccccccccccc"
TS0, TS1, TS2 = "2026-11-01T10:00:00.000Z", "2026-11-01T11:00:00.000Z", "2026-11-01T12:00:00.000Z"


def _restricted(line, level, relevant, when="before or during", rule="R10", family="FSL", ts=TS0):
    return {
        "rule": rule,
        "level": level,
        "family": family,
        "relevant": relevant,
        "when": when,
        "line": line,
        "ts": ts,
        "show": "crates-nc/larmorx-nc-bet/src/bet.rs",
        "in_force": True,
    }


def _agent(aid, role, restricted=(), register=(), commits=(), description=None):
    return {
        "key": f"{S1}/agent-{aid}" if aid != "main" else f"{S1}/main",
        "description": description or f"BET clean-room relay {aid[:2]}",
        "role": role,
        "first_touch": TS1,
        "last_touch": TS2,
        "touches": 2,
        "commits": list(commits),
        "restricted": list(restricted),
        "register": list(register),
        "diffs": {h: {"added": 3, "removed": 1, "comments_or_blank": 2} for h in commits},
    }


def _sim_cell(tokens=10):
    return {
        "identifiers_shared": 0.25,
        "names": {
            "coverage_8": 0.01,
            "coverage_16": 0.0,
            "longest_run": {
                "tokens": tokens,
                "ours": {"file": "crates/larmorx-fsl/src/bet/mesh.rs", "lines": [1, 2]},
                "theirs": {"file": "crates-nc/fsl-source/fsl-bet2/bet2.cpp", "lines": [3, 4]},
            },
        },
    }


def _floor():
    return {
        "diagonal": {"identifiers_shared": 0.25, "coverage_8": 0.01, "longest_run": 10},
        "median_coverage_8": 0.0,
        "max_coverage_8": 0.02,
        "max_longest_run": 12,
    }


def _dossier(
    tmp_path: Path, *, judged: bool, private_description: bool = False
) -> tuple[Path, su.Judgements]:
    d = tmp_path / "2026-11-01"
    scan = d / "scan"
    (scan / "access").mkdir(parents=True)
    archive = tmp_path / "2026-11-01-session-transcripts"
    (archive / "main-machine").mkdir(parents=True)
    (archive / "SHA256SUMS").write_text("ab  ./main-machine/x\n", encoding="utf-8")
    keys = {
        A1: f"{S1}/agent-{A1}",
        A2: f"{S1}/agent-{A2}",
    }
    agents = {}
    for aid, key in keys.items():
        log = f"access/{S1}__agent-{aid}.jsonl.gz"
        agents[key] = {
            "access_log": log,
            "meta": {"worktreePath": f"{HOME}/wt"} if aid == A1 else {},
            "compactions": 1,
        }
        rows = [
            {"ts": TS1, "category": "spec", "op": "read", "via": "Read"},
            {"ts": TS1, "category": "cleanroom", "op": "edit", "via": "Edit"},
            {"ts": TS1, "category": "run", "op": "exec", "via": "bet", "unresolved": False},
            {"ts": TS1, "category": "repo", "op": "list", "via": "find"},
            {"ts": TS1, "category": "restricted", "op": "read", "via": "cat", "unresolved": True},
            {"ts": TS1, "category": "restricted", "op": "read", "via": "cat", "refused": True},
            {"ts": "2026-11-02T00:00:00Z", "category": "spec", "op": "read", "via": "Read"},
        ]
        with gzip.open(scan / log, "wt", encoding="utf-8") as fh:
            for r in rows:
                fh.write(json.dumps(r) + "\n")
    (scan / "agents.json").write_text(json.dumps(agents), encoding="utf-8")
    (scan / "manifest.json").write_text(
        json.dumps(
            [{"sha256": "ab", "archive_sha256": "ab"}, {"sha256": "cd", "archive_sha256": None}]
        ),
        encoding="utf-8",
    )
    (scan / "summary.json").write_text(
        json.dumps(
            {
                "roots": [str(archive / "main-machine"), f"{HOME}/.claude/projects/x"],
                "transcript_files": 2,
                "distinct_transcripts": 2,
                "transcript_bytes": 1000,
                "archive_checksums_matched": 1,
                "archive_checksums_differ": 0,
                "sessions": 1,
                "session_ranges": {S1: [TS0, TS2]},
                "agents": 3,
                "tool_calls": 10,
                "tool_calls_by_tool": {"Bash": 8, "Read": 2},
                "accesses": 14,
                "unique_paths": 9,
                "commits_printed": 3,
                "gaps": {"compactions": 2},
            }
        ),
        encoding="utf-8",
    )
    impl1 = _agent(
        A1,
        su.IMPL,
        restricted=[
            _restricted(7, "content", True),
            _restricted(8, "replica-run", True),
            _restricted(9, "names", False, family="AFNI"),
            _restricted(10, "content", True, when="after", ts=TS2),
        ],
        commits=["c1"],
        description=f"BET part 1 {HOME}" if private_description else None,
    )
    impl2 = _agent(
        A2,
        su.IMPL,
        register=[
            {"when": "before or during", "relevant": True, "body_lines": 2, "line": 5, "ts": TS0},
            {"when": "before or during", "relevant": True, "body_lines": 0, "line": 6, "ts": TS0},
        ],
        commits=["c2"],
    )
    orch = _agent("main", "orchestrator", commits=["c3"], description="ORCHESTRATOR")
    verdicts = {
        "tools": {
            "FSL/bet": {
                "family": "FSL",
                "tool": "bet",
                "name": "BET",
                "agents": [impl1, impl2, orch],
                "verdict": {
                    "status": "review",
                    "findings": [],
                    "notes": {},
                    "other_roles": [
                        {
                            "description": "ORCHESTRATOR",
                            "role": "orchestrator",
                            "commits": ["c3"],
                            "relevant_content_before": 4,
                        }
                    ],
                },
            }
        },
        "commits": {
            h: {
                "maker": m,
                "how": "printed",
                "commit_date": "2026-11-01T12:00:00+01:00",
                "files": [{"path": "crates/larmorx-fsl/src/bet/mesh.rs"}],
            }
            for h, m in (
                ("c1" + "0" * 38, keys[A1]),
                ("c2" + "0" * 38, keys[A2]),
                ("c3" + "0" * 38, f"{S1}/main"),
            )
        },
        "roles": {
            keys[A1]: {"role": su.IMPL},
            keys[A2]: {"role": su.IMPL},
            f"{S1}/main": {"role": "orchestrator"},
        },
        "unattributed": [],
        "register_restricted_lines": 3,
        "single_file_scopes": {"checked": 1, "shown": []},
        "slips": [
            {
                "agent": keys[A1],
                "description": impl1["description"],
                "line": 7,
                "ts": TS0,
                "rules": {"R10:FSL:content": 2},
                "paths": ["crates-nc/larmorx-nc-bet/src/bet.rs"],
            }
        ],
        "brief_dependent": [
            {
                "agent": keys[A2],
                "description": impl2["description"],
                "line": 11,
                "ts": TS1,
                "kind": "R16: a replica's suite or evidence module (its brief: never read); a search printed 1 of its lines",
                "rel": "validation/src/larmorx_validation/parity/fsl_bet_replica.py",
            }
        ],
        "merge_edits": [],
        "orchestrator_edits": {},
    }
    (d / "verdicts").mkdir()
    (d / "verdicts" / "verdicts.json").write_text(json.dumps(verdicts), encoding="utf-8")
    (d / "verdicts" / "verdicts.md").write_text(
        "Scan: `scan`; repository at `abc1234`.\n", encoding="utf-8"
    )
    sim = {
        "commit": "def5678",
        "method": {},
        "tools": {
            "FSL/bet": {
                "name": "BET",
                "family": "FSL",
                "tool": "bet",
                "cleanroom": {"tokens": 100, "files": 2},
                "cleanroom_vs_upstream": _sim_cell(),
                "cleanroom_vs_replica": _sim_cell(41),
                "replica_vs_upstream": _sim_cell(),
            }
        },
        "matrices": {
            "FSL": {
                "tools": ["bet"],
                "cleanroom_vs_upstream_floors": [_floor()],
                "replica_vs_upstream_floors": [_floor()],
                "cleanroom_vs_replica_floors": [_floor()],
            }
        },
        "calibration": [
            {
                "name": "a port",
                "kind": "port",
                "tokens": 10,
                "identifiers_shared": 0.5,
                "names": {"coverage_8": 0.3, "coverage_16": 0.2, "longest_run": {"tokens": 9}},
                "shape": {},
            }
        ],
    }
    (d / "similarity").mkdir()
    (d / "similarity" / "similarity.json").write_text(json.dumps(sim), encoding="utf-8")
    j = su.Judgements()
    j.verdict = {"FSL/bet": "clean (synthetic)"}
    if judged:
        j.access[(A1, 7)] = {
            "agent": A1,
            "line": 7,
            "declared": "yes",
            "shown": "two lines of the replica",
            "nature": "behaviour",
            "judgement": "no contamination",
        }
        j.access[(A2, 5)] = {
            "agent": A2,
            "line": 5,
            "declared": "no",
            "shown": "two register lines",
            "nature": "behaviour",
            "judgement": "none",
        }
        j.access[(A2, 11)] = {
            "agent": A2,
            "line": 11,
            "declared": "yes",
            "shown": "a suite line",
            "nature": "harness",
            "judgement": "none",
        }
        j.edit[("orchestrator", "c3")] = {
            "agent": "orchestrator",
            "commit": "c3",
            "what": "a header",
            "judgement": "none",
        }
    return d, j


def _extract(d: Path, j: su.Judgements) -> dict:
    scan, verdicts, sim = su.find_dossier_parts(d)
    return su.extract(scan, verdicts, sim, j)


def test_extract_a_judged_dossier(tmp_path):
    d, j = _dossier(tmp_path, judged=True)
    facts = _extract(d, j)
    assert facts["dossier"] == "2026-11-01"
    assert facts["made_from"] == {
        "scan": "scan",
        "verdicts_repository": "abc1234",
        "similarity_commit": "def5678",
    }
    ev = facts["evidence"]
    assert (ev["archived_files"], ev["live_files"]) == (1, 1)
    assert list(ev["archives"]) == ["2026-11-01-session-transcripts"]
    (t,) = facts["tools"]
    assert t["verdict"] == {
        "status": "reviewed (every finding judged)",
        "words": "clean (synthetic)",
    }
    assert t["commits"] == 3
    # Findings: A1's content of this family before its last edit (line 7), A2's register body (line 5);
    # neither the black-box run, another family's names, nor the content seen after the work.
    assert [(f["agent"], f["line"], f["judged"]) for f in t["findings"]] == [
        (A1, 7, True),
        (A2, 5, True),
    ]
    assert t["findings"][0]["declared"] == "yes"
    # Accesses: before the last edit only, refused calls left out.
    acc = {a["group"]: (a["seen"], a["run"], a["written"]) for a in t["accesses"]}
    assert acc["the behaviour specs (specs/)"] == (2, 0, 0)
    assert acc["the clean-room code"] == (0, 0, 2)
    assert acc[su.RESTRICTED_GROUP] == (2, 0, 0)
    assert t["cannot_see"] == {
        "compactions": 2,
        "listings": 2,
        "program_runs": 2,
        "unresolved": 2,
        "without_worktree": 1,
    }
    scopes = {
        (r["rule"], r["level"], r["scope"]): (r["before"], r["after"]) for r in t["restricted"]
    }
    assert scopes[("R10", "content", "this family")] == (1, 1)
    assert scopes[("R10", "names", "another family")] == (1, 0)
    assert scopes[("R12", "headings", "this family")] == (1, 0)
    # Slips: one per call, in time order, with the brief-forbidden read.
    assert [(s["agent"], s["line"], s["kind"]) for s in facts["slips"]] == [
        (A1, 7, "restricted material"),
        (A2, 11, "a file its brief forbade"),
    ]
    assert t["slips"] == [0, 1]
    (e,) = facts["edits"]
    assert (e["agent"], e["commit"], e["judged"], e["what"]) == (
        "orchestrator",
        "c3",
        True,
        "a header",
    )
    page = su.render(facts)
    assert "### BET" in page and "two lines of the replica" in page
    assert su.privacy_problems(page, names=["someone"]) == []
    assert su.privacy_problems(json.dumps(facts), names=["someone"]) == []
    # The page renders the same facts the same way after a JSON round trip.
    assert su.render(json.loads(json.dumps(facts, indent=1, sort_keys=True))) == page


def test_extract_unjudged_items_are_marked(tmp_path):
    d, j = _dossier(tmp_path, judged=False)
    facts = _extract(d, j)
    (t,) = facts["tools"]
    assert t["verdict"]["status"] == "review"
    assert not any(f["judged"] for f in t["findings"])
    page = su.render(facts)
    assert "**not yet judged**" in page
    assert "1 original has items not yet judged" in page


def test_extract_refuses_private_strings(tmp_path):
    d, j = _dossier(tmp_path, judged=True, private_description=True)
    with pytest.raises(RuntimeError, match="private strings"):
        _extract(d, j)


def test_gate(tmp_path):
    d, j = _dossier(tmp_path, judged=False)
    v = json.loads((d / "verdicts" / "verdicts.json").read_text(encoding="utf-8"))
    blocking, to_judge = su.gate(v, j)
    assert len(blocking) == 3  # two findings, the orchestrator's commit
    assert any(f"{A1}" in b and "line 7" in b for b in blocking)
    assert any("orchestrator" in b and "c3" in b for b in blocking)
    assert len(to_judge) == 2  # the two slips
    _, j2 = _dossier(tmp_path / "b", judged=True)
    assert su.gate(v, j2) == ([], [])
    v["unattributed"] = ["e" * 40]
    blocking, _ = su.gate(v, j2)
    assert blocking == [f"commit {'e' * 9} changed clean-room code and no agent's record made it"]


def test_main_check_and_gate_exit_codes(tmp_path, capsys):
    import argparse

    d, j = _dossier(tmp_path, judged=False)
    ns = argparse.Namespace(
        verdicts=str(d / "verdicts"), judgements=str(tmp_path / "none.toml"), strict=False
    )
    assert su.main_gate(ns) == 1
    facts = _extract(d, j)
    jp, mp = tmp_path / "f.json", tmp_path / "p.md"
    jp.write_text(json.dumps(facts, indent=1, sort_keys=True) + "\n", encoding="utf-8")
    args = argparse.Namespace(dossier=None, json=str(jp), out=str(mp), check=True, verbatim=None)
    assert su.main_summary(args) == 1  # no page yet
    args.check = False
    assert su.main_summary(args) == 0
    args.check = True
    assert su.main_summary(args) == 0
    mp.write_text(mp.read_text(encoding="utf-8") + "edited by hand\n", encoding="utf-8")
    assert su.main_summary(args) == 1
