# SPDX-License-Identifier: Apache-2.0
"""The confounds' evidence: the column families, the cell comparison's sign test and the
register entries read from the part's findings (no cache entry, no workspace needed)."""

from __future__ import annotations

from pathlib import Path

from larmorx_validation.evidence import confounds as c
from larmorx_validation.evidence import confounds_run as cr
from larmorx_validation.evidence import core

REPO = Path(__file__).resolve().parents[2]


def test_columns_fall_into_their_families_and_groups():
    assert c.family("a_comp_cor_07") == "a_comp_cor_XX"
    assert c.family("edge_comp_23") == "edge_comp_XX"
    assert c.family("cosine03") == "cosineXX"
    assert c.family("motion_outlier12") == "motion_outlierXX"
    assert c.family("rot_x_derivative1_power2") == "rot_x_derivative1_power2"
    assert c.group("rot_x_derivative1_power2") == "motion"
    assert c.group("framewise_displacement") == "motion-derived"
    assert c.group("motion_outlierXX") == "motion-derived"
    assert c.group("std_dvars") == "dvars"
    assert c.group("t_comp_cor_XX") == "compcor"
    assert c.group("edge_comp_XX") == "compcor"
    assert c.group("cosineXX") == "prefilter"
    assert c.group("csf_wm") == "signals"
    assert c.group("tcompcor") == "signals"


def test_a_negated_cell_is_the_same_text_with_the_other_sign():
    assert cr._negated("-0.1234567890", "0.1234567890")
    assert cr._negated("0.5", "-0.5")
    assert not cr._negated("0.5", "0.5")
    assert not cr._negated("0.5", "-0.4")


def test_the_definitions_first_sentence():
    assert c._sentence("a mask ends the loop. The later masks are skipped") == (
        "a mask ends the loop."
    )
    assert c._sentence("no full stop here") == "no full stop here"


def test_every_entry_of_the_part_is_read_from_its_findings():
    ctx = core.Context(repo=REPO, site=REPO, cache=REPO)
    found = c._defects(ctx)
    want = {f"PIPE-{i}" for i in range(41, 84)}
    want |= {"PD-01", "PD-02", "PD-03", "NP-09", "NP-10", "SCI-41", "GLIBC-07"}
    assert want <= set(found), sorted(want - set(found))
    for i, f in found.items():
        assert (REPO / f["file"]).is_file(), i
        assert f["what"], i
