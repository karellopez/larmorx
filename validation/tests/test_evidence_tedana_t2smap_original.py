# SPDX-License-Identifier: Apache-2.0
"""The texts of the clean-room t2smap original's evidence (2026-11-12): the site renders them
through markdown-it's typographer, so ``(b)/(c)`` is written with ``\\(c)`` (else the copyright
sign shows), and the random-testing table's row heads give the section that the harness's
signatures write with ``N`` for each digit."""

import pytest
from larmorx_validation.evidence import tedana_t2smap_original as m


def test_signature_gives_the_section_again():
    assert m._signature("(A) values (§N.N)") == "(A) values (§17.3)"
    assert m._signature("(B) messages") == "(B) messages"
    with pytest.raises(ValueError, match="section"):
        m._signature("(A) something (§N)")


def test_b_c_survives_the_typographer():
    markdown_it = pytest.importorskip("markdown_it")
    md = markdown_it.MarkdownIt("commonmark", {"html": True, "typographer": True})
    md.enable(["table", "replacements", "smartquotes"])
    assert md.renderInline("(b)/(c)") == "(b)/©"  # the site's rendering without the escape
    assert md.renderInline(f"the {m.BC} share") == "the (b)/(c) share"
