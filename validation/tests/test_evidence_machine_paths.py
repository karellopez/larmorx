# SPDX-License-Identifier: Apache-2.0
"""No exported page holds a path of the machine that made it: every string of an evidence JSON
passes through ``figures.cache.machine_paths`` when it is written (``evidence.core.write_tool``),
so a home directory, the workspace, an agent's worktree or scratch folder, the user's name and
the machine's name never reach the published site (the documentation revision, 2026-11-13)."""

from __future__ import annotations

import getpass
import json
import socket
from pathlib import Path

from larmorx_validation.evidence import core
from larmorx_validation.figures.cache import machine_paths


def _leaks(text: str) -> list[str]:
    home = str(Path.home())
    found = [w for w in (home, ".claude/worktrees", "/tmp/claude-") if w in text]
    user = getpass.getuser()
    if len(user) > 2 and f"/{user}/" in text.replace("/home/fmriprep/", "/"):
        found.append(user)
    host = socket.gethostname()
    if len(host) > 2 and host in text:
        found.append(host)
    return found


def test_every_form_of_a_machine_path_is_replaced(tmp_path):
    home = str(Path.home())
    host = socket.gethostname()
    texts = [
        f"{home}/w/oracles/fmriprep-25.2.6/ds000005/x.nii.gz",
        f"PYTHONPATH={home}/w/larmorx/.claude/worktrees/agent-a1b2/python:{home}/w/larmorx/validation/src",
        "PYTHONPATH=<workspace>/larmorx/.claude/worktrees/agent-a1b2/python:<larmorx>/validation/src",
        "~/w/larmorx/.claude/worktrees/agent-a1b2/crates-nc/target/release/larmorx-nc",
        "/tmp/claude-1000/-home-u-w/0f1e2d3c-4b5a-6978-8a9b-0c1d2e3f4a5b/scratchpad/p/x.nii",
        f"Error: cant open file grefvol_{home}/w/oracles/inputs/x",
        "/home/someone/data/x.nii.gz",
        f"made on {host}",
    ]
    for t in texts:
        out = machine_paths(t)
        assert not _leaks(out), (t, out)
    # A container's own home is not the machine's.
    assert machine_paths("-B x:/home/fmriprep") == "-B x:/home/fmriprep"
    assert machine_paths("<workspace>/larmorx/.claude/worktrees/a1/python") == "<larmorx>/python"


def test_a_written_page_holds_no_machine_path():
    home = str(Path.home())
    doc = {
        "provenance": {"env": {"PYTHONPATH": f"{home}/w/larmorx/.claude/worktrees/a/python"}},
        "cases": [{"reason": f"AFNI exit 1: ** Dataset {home}/w/oracles/inputs/s1.nii.gz"}],
        "n": 3,
    }
    text = json.dumps(core._clean(doc))
    assert not _leaks(text), text
    assert json.loads(text)["n"] == 3
