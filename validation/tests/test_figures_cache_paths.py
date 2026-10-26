# SPDX-License-Identifier: Apache-2.0
"""The figures cache names a program built in any larmorx checkout alike (published provenance
must not depend on the checkout or worktree that ran it)."""

from larmorx_validation.figures.cache import checkout_paths


def test_every_workspace_build_is_named_alike():
    for sub in (
        "crates-nc/target/release/larmorx-nc",
        "crates-gpl/target/release/larmorx-gpl",
        "target/release/larmorx",
        "crates/larmorx-cli/src/main.rs",
    ):
        want = f"<larmorx>/{sub}"
        assert checkout_paths(f"/home/u/w/larmorx/{sub}") == want
        assert checkout_paths(f"/home/u/w/larmorx/.claude/worktrees/agent-1/{sub}") == want


def test_other_paths_are_kept():
    assert checkout_paths("/home/u/fsl/bin/bet in out") == "/home/u/fsl/bin/bet in out"
