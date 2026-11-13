# SPDX-License-Identifier: Apache-2.0
"""The clean-room audit's verdicts (``cleanroom-audit verdicts``), on small synthetic inputs: the
clean-room code by tool, roles from descriptions and briefs, commits attributed to their makers,
the register's restricted-only lines and what an output shows of them, each restricted access
judged against a tool's family, a tool's verdict, git's exclusion pathspecs."""

from __future__ import annotations

from pathlib import Path

import pytest
from larmorx_validation.cleanroom_audit import accesses as acc
from larmorx_validation.cleanroom_audit import restrictions as rs
from larmorx_validation.cleanroom_audit import verdicts as vd

WS = "/home/u/ws"
CTX = rs.Context(WS, "/home/u", ("/home/u/fsl",))
WT = f"{WS}/larmorx/.claude/worktrees/agent-a1"


@pytest.mark.parametrize(
    ("rel", "expected"),
    [
        ("crates/larmorx-afni/src/tshift/series.rs", ("AFNI", "tshift")),
        ("crates/larmorx-afni/src/cli/unifize.rs", ("AFNI", "unifize")),
        ("crates/larmorx-afni/src/dataset.rs", ("AFNI", "afni-shared")),
        ("crates/larmorx-mri/src/hmc/cost.rs", ("FSL", "hmc")),
        ("crates/larmorx-fsl/src/bet/mesh.rs", ("FSL", "bet")),
        ("crates/larmorx-fsl/src/cli/betsurf.rs", ("FSL", "bet")),
        ("crates/larmorx-fsl/src/cli/convert_xfm.rs", ("FSL", "flirt")),
        ("crates/larmorx-fsl/src/fast/pve.rs", ("FSL", "fast")),
        ("crates/larmorx-fsl/src/cli/applytopup.rs", ("FSL", "topup")),
        ("crates/larmorx-fsl/src/cli/fslstats_usage.txt", ("FSL", "fslmaths")),
        ("crates/larmorx-fsl/src/cli/ssroi.rs", ("FSL", "standard_space_roi")),
        ("crates/larmorx-fsl/src/cli/fslroi.rs", ("FSL", "fslroi")),
        ("crates/larmorx-fsl/src/data.rs", ("FSL", "fsl-shared")),
        ("crates/larmorx-tedana/src/curvefit.rs", ("tedana", "t2smap")),
        ("crates-gpl/larmorx-gpl-tedana/src/decay.rs", None),
        ("crates/larmorx-core/src/math/sincos_glibc_tables.rs", ("GNU", "sincos_glibc")),
        ("crates/larmorx-core/src/math/sincosf_glibc.rs", None),  # Arm's MIT code, a replica
        ("crates/larmorx-core/src/ranlux/mod.rs", ("FreeSurfer", "ranlux")),
        ("crates/larmorx-core/src/cubic_spline/solve.rs", ("FreeSurfer", "cubic_spline")),
        ("crates/larmorx-core/src/glibc_random/mod.rs", ("GNU", "glibc_random")),
        ("crates/larmorx-core/src/glibc_qsort/merge.rs", ("GNU", "glibc_qsort")),
        ("crates/larmorx-core/src/ranlux.rs", None),  # the module must be a folder of its own
        ("crates/larmorx-afni-replica/src/tshift.rs", None),
        ("crates-nc/larmorx-nc-bet/src/lib.rs", None),
    ],
)
def test_tool_of(rel, expected):
    assert vd.tool_of(rel) == expected


def test_every_clean_room_crate_is_audited():
    """A crate whose PROVENANCE.md calls it a clean-room implementation is in the audit's lists
    (crates/larmorx-tedana was not until its merge, 2026-11-12), and every listed crate is one."""
    repo = Path(__file__).resolve().parents[2]
    declared = {
        f"crates/{d.name}"
        for d in (repo / "crates").iterdir()
        if (d / "PROVENANCE.md").is_file()
        and "Clean-room implementation"
        in "".join((d / "PROVENANCE.md").read_text(encoding="utf-8").splitlines(True)[:12])
    }
    assert declared == {c.rstrip("/") for c in rs.CLEANROOM_CRATES}
    assert declared <= set(vd.CLEANROOM_PATHS)
    for c in declared:
        assert vd.tool_of(f"{c}/src/lib.rs") in vd.TOOL_NAMES, c


@pytest.mark.parametrize(
    ("description", "brief", "role"),
    [
        ("ORCHESTRATOR", "", "orchestrator"),
        ("FLIRT clean-room part 2 relay 3", "", "clean-room implementer"),
        (
            "FAST replica relay 3",
            "the clean-room implementer works from it alone",
            "replica author",
        ),
        ("3dTshift globals: -overwrite, -D (replica+spec)", "", "replica author"),
        ("glibc maths part 1, relay 2 (spec)", "", "spec writer"),
        ("Docs revision writer relay 11 (restart)", "clean-room implementers ...", "documentation"),
        ("Docs revision critical reviewer round 1", "", "documentation"),
        ("Cross-part coherence review (reviewer)", "", "documentation"),
        ('implementation="auto": run the GPL replica', "", "integration"),
        (
            "glibc maths part 2, relay 1",
            "You are relay 1 of the glibc maths part, second half (larmorx): the CLEAN-ROOM "
            "implementer. You never open glibc's source.",
            "clean-room implementer",
        ),
        (
            "Decided corrections",
            "Your brief is in the file x.md. Another agent (a clean-room implementer) runs in "
            "parallel.\n**Your role: replica author** (you may read AFNI source).",
            "replica author",
        ),
        ("Find 7T datasets", "Search OpenNeuro.", "other"),
    ],
)
def test_role_of(description, brief, role):
    assert vd.role_of(description, brief)[0] == role


def _commit(h, when, subject, files=()):
    return {
        "hash": h,
        "parents": ["p"],
        "author_date": when,
        "commit_date": when,
        "subject": subject,
        "files": [{"status": "M", "path": f, "from": f} for f in files],
    }


def test_attribute_by_printed_hash_subject_and_time():
    commits = {
        "a" * 40: _commit("a" * 40, "2026-11-02T10:00:00+00:00", "printed one"),
        "b" * 40: _commit("b" * 40, "2026-11-02T11:00:00+00:00", "FLIRT: the search's grid"),
        "c" * 40: _commit("c" * 40, "2026-11-02T12:00:00+00:00", "x", ["crates/larmorx-fsl/a.rs"]),
        "d" * 40: _commit("d" * 40, "2026-11-02T13:00:00+00:00", "nobody"),
    }
    # The maker printed it right after; a later relay listed it in its own git log.
    printed = [
        {"hash": "aaaaaaa", "key": "s/agent-maker", "ts": "2026-11-02T10:00:03Z"},
        {"hash": "aaaaaaa", "key": "s/agent-later", "ts": "2026-11-02T10:20:00Z"},
    ]
    commands = [
        {
            "key": "s/agent-maker",
            "line": 5,
            "ts": "2026-11-02T10:00:01Z",
            "command": "git commit -q",
        },
        {
            "key": "s/agent-later",
            "line": 9,
            "ts": "2026-11-02T10:19:59Z",
            "command": "git commit -q",
        },
        {
            "key": "s/agent-subj",
            "line": 7,
            "ts": "2026-11-02T11:00:02Z",
            "command": 'git commit -qm "FLIRT: the search\'s grid"',
        },
        {
            "key": "s/agent-time",
            "line": 3,
            "ts": "2026-11-02T12:00:01Z",
            "command": "git commit -F m",
        },
        {
            "key": "s/agent-idle",
            "line": 4,
            "ts": "2026-11-02T12:00:00Z",
            "command": "git commit -F m",
        },
    ]
    edits = {"s/agent-time": [(vd.epoch("2026-11-02T11:30:00+00:00"), "crates/larmorx-fsl/a.rs")]}
    got = vd.attribute(commits, printed, commands, edits)
    assert (got["a" * 40]["maker"], got["a" * 40]["how"]) == ("s/agent-maker", "printed")
    assert (got["b" * 40]["maker"], got["b" * 40]["how"]) == ("s/agent-subj", "subject")
    assert (got["c" * 40]["maker"], got["c" * 40]["how"]) == ("s/agent-time", "time")
    assert got["d" * 40]["maker"] is None


def test_attribute_from_a_hash_in_the_commit_command_output():
    commits = {"e" * 40: _commit("e" * 40, "2026-11-02T10:00:30+00:00", "WIP")}
    commands = [
        {
            "key": "s/agent-r5",
            "line": 2,
            "ts": "2026-11-02T10:00:00Z",  # the command started before its commit (a build first)
            "command": "cargo build && git commit -qm WIP && git log --oneline -1",
            "result": "WHEEL DONE\neeeeeee WIP",
        }
    ]
    got = vd.attribute(commits, [], commands, {})
    assert got["e" * 40]["maker"] == "s/agent-r5"


def test_attribute_by_subject_from_a_command_that_started_before_its_commit():
    # 2026-11-08: a relay ran its wheel build and the formatters first, then `git commit -qm`, in
    # one background command 30 s before the commit's date; no hash was printed in the transcript
    commits = {
        "f" * 40: _commit("f" * 40, "2026-11-08T04:11:56+00:00", "3dvolreg original (WIP): x")
    }
    commands = [
        {
            "key": "s/agent-r3",
            "line": 1308,
            "ts": "2026-11-08T04:11:26Z",
            "command": 'wheel.sh; cargo fmt --all && git commit -q -m "3dvolreg original (WIP): x"',
        },
        {
            "key": "s/agent-other",
            "line": 4,
            "ts": "2026-11-08T03:30:00Z",  # 42 minutes before: too early even with the subject
            "command": 'git commit -q -m "3dvolreg original (WIP): x"',
        },
    ]
    got = vd.attribute(commits, [], commands, {})
    assert (got["f" * 40]["maker"], got["f" * 40]["how"]) == ("s/agent-r3", "subject")


REGISTER_V1 = """# Upstream defects
## Summary
| AFNI-26 † | 3dTshift | a failed write exits 0 |
## FSL: black-box findings (open to everyone)
### FSL-111 applyxfm4D reads uint8 images as signed bytes
## Restricted: AFNI 3dTshift (source-derived)
### AFNI-26 † A failed write exits 0 — (e)
- **What:** the writer's status is dropped by the routine that opens the output file.
"""
REGISTER_V2 = REGISTER_V1 + "- **larmorx:** both implementations exit 1 by default (the replica).\n"


def test_register_lines_and_what_an_output_shows():
    reg = vd.register_lines([REGISTER_V1, REGISTER_V2])
    assert (
        "- **What:** the writer's status is dropped by the routine that opens the output file."
        in reg
    )
    assert not any("FSL-111" in n for n in reg)  # the open section
    assert not any("| AFNI-26 †" in n for n in reg)  # the summary row is open
    out = (
        "7:### AFNI-26 † A failed write exits 0 — (e)\n"
        "docs/findings/upstream-defects.md:9:- **larmorx:** both implementations exit 1 by default "
        "(the replica).\n"
        "     3\t| AFNI-26 † | 3dTshift | a failed write exits 0 |\n"
    )
    hits = vd.printed_register_lines(out, reg)
    assert [vd.is_heading(x) for _, x in hits] == [True, False]
    assert {s for s, _ in hits} == {"Restricted: AFNI 3dTshift (source-derived)"}


@pytest.mark.parametrize(
    ("section", "family"),
    [
        ("Restricted: AFNI 3dvolreg (source-derived)", "AFNI"),
        ("Restricted: FSL mcflirt (source-derived)", "FSL"),
        ("Restricted: tedana t2smap (source-derived)", "tedana"),
    ],
)
def test_section_family(section, family):
    assert vd.section_family(section) == family


def _access(**kw):
    base = {
        "rule": "R10",
        "family": "FSL",
        "category": "restricted",
        "op": "read",
        "in_force": True,
        "show": "crates-nc/x.rs",
    }
    return {**base, **kw}


@pytest.mark.parametrize(
    ("access", "family", "level", "relevant"),
    [
        (_access(), "FSL", "content", True),
        (_access(), "AFNI", "content", False),  # another family's replica
        (_access(op="search"), "FSL", "scope", True),
        (_access(op="script-path"), "FSL", "possible", True),
        (_access(category="restricted-names", op="list"), "FSL", "names", True),
        (_access(category="restricted-build", op="build"), "FSL", "build", True),
        (_access(category="replica-build-output", op="exec"), "FSL", "replica-run", True),
        (_access(category="replica-build-output", op="copy"), "FSL", "build-cache", True),
        (_access(in_force=False), "FSL", "content", False),
        (_access(rule="R11", family="AFNI/FSL"), "FSL", "content", True),
        (_access(rule="R17", family="all"), "GNU", "content", True),
        (_access(family="AFNI/tedana"), "tedana", "content", True),  # crates-gpl
        (_access(rule="R07", family="tedana"), "FSL", "content", False),
    ],
)
def test_judge_levels_and_families(access, family, level, relevant):
    j = vd.judge(access, family, {}, vd.part_kind)
    assert (j.level, j.relevant) == (level, relevant)


def test_judge_material_written_by_clean_room_implementers():
    roles = {"s/agent-cr": "clean-room implementer", "s/agent-rep": "replica author"}
    clean = _access(rule="R18", family="all", writers=["s/agent-cr"])
    assert vd.judge(clean, "FSL", roles, vd.part_kind).level == "clean-writer"
    dirty = _access(rule="R18", family="all", writers=["s/agent-cr", "s/agent-rep"])
    j = vd.judge(dirty, "FSL", roles, vd.part_kind)
    assert (j.level, j.relevant) == ("content", True)
    part = _access(rule="R13", family="all", part="2026-10-30-cleanroom-bet-part1")
    assert vd.judge(part, "FSL", roles, vd.part_kind).level == "clean-writer"
    other = _access(rule="R13", family="all", part="2026-11-01-fsl-flirt-part1")
    assert vd.judge(other, "FSL", roles, vd.part_kind).relevant


def _agent(role, restricted=(), register=()):
    return {
        "description": "x",
        "role": role,
        "touches": 1,
        "commits": [],
        "restricted": list(restricted),
        "register": list(register),
    }


def test_verdict_separates_implementers_from_other_roles():
    seen = {
        **_access(),
        "level": "content",
        "relevant": True,
        "when": "before or during",
        "ts": "t",
    }
    late = {**seen, "when": "after"}
    names = {**seen, "level": "names"}
    assert vd.verdict({"agents": [_agent("clean-room implementer")]})["status"] == "clean"
    assert vd.verdict({"agents": [_agent("clean-room implementer", [late])]})["status"] == "clean"
    v = vd.verdict({"agents": [_agent("clean-room implementer", [names])]})
    assert v["status"] == "clean, with notes" and v["notes"] == {"R10/names/relevant": 1}
    assert vd.verdict({"agents": [_agent("clean-room implementer", [seen])]})["status"] == "review"
    v = vd.verdict({"agents": [_agent("clean-room implementer"), _agent("orchestrator", [seen])]})
    assert v["status"].startswith("clean (implementers)")
    assert v["other_roles"][0]["relevant_content_before"] == 1
    heading = {"when": "before or during", "relevant": True, "body_lines": 0, "ts": "t"}
    v = vd.verdict({"agents": [_agent("clean-room implementer", register=[heading])]})
    assert v["status"] == "clean, with notes"


def test_git_exclusion_pathspecs_are_not_accesses():
    res = acc.call_accesses(
        "Bash",
        {
            "command": "git diff a5ccbb0 HEAD -- crates ':!crates/larmorx-afni-replica' ':(exclude)x'"
        },
        "",
        WT,
        CTX,
    )
    shown = [rs.locate(a.path, CTX).show() for a in res.accesses]
    assert shown == ["crates"]
