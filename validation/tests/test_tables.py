# SPDX-License-Identifier: Apache-2.0
"""The generated tables (``python -m larmorx_validation tables``): a committed table that differs
from its generated form fails, and the generator's pieces behave."""

from __future__ import annotations

import json

import pytest
from larmorx_validation import tables


def test_committed_tables_are_generated():
    generated = tables.generate()
    differ = [str(p) for p, text in generated.items() if p.read_text(encoding="utf-8") != text]
    assert not differ, (
        "these tables differ from their generated form; edit validation/tools.toml and run "
        f"`python -m larmorx_validation tables`: {differ}"
    )


def test_every_description_names_existing_records():
    desc = tables.descriptions()
    for r in desc["record"]:
        assert (tables.RECORDS / f"{r['id']}.md").is_file(), r["id"]
    ids = [t["id"] for t in desc["tool"]]
    assert len(ids) == len(set(ids))


def test_links_are_written_for_each_place(tmp_path):
    (tmp_path / "x.json").write_text(
        json.dumps({"summary": {"pass": 3, "fail": 1}, "results": [{}] * 5})
    )
    m = tables._Records(tmp_path)
    text = "[{x.cases} cases](record:x), {x.compared} compared; [fast](repo:docs/benchmarks/x.md)"
    assert (
        tables.render_text(text, tables.README, m)
        == "[5 cases](docs/validation/x.md), 4 compared; [fast](docs/benchmarks/x.md)"
    )
    assert (
        tables.render_text(text, tables.INDEX, m)
        == "[5 cases](x.md), 4 compared; [fast](../benchmarks/x.md)"
    )
    assert (
        tables.render_text(text, tables.SITE, m)
        == "[5 cases](repo:docs/validation/x.md), 4 compared; [fast](repo:docs/benchmarks/x.md)"
    )


def test_identity_is_not_confused_with_one():
    # 1 == True in Python: a count of 1 differing value is not "identical".
    assert tables._zero(0) and tables._zero(True) and tables._zero("True")
    assert not tables._zero(1) and not tables._zero(False) and not tables._zero("False")


def test_markers_are_required():
    with pytest.raises(ValueError, match="markers"):
        tables.replace_block("no markers here", "tools table", "body")
