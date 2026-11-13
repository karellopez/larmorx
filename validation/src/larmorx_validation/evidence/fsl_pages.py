# SPDX-License-Identifier: Apache-2.0
"""The writing guide's presentation of FSL's pages of two ports (``docs/writing-guide.md``,
sections 1, 4 and 5; :mod:`present` does the work): the replica, in ``larmorx-nc`` (the FSL
Licence: non-commercial use only), and the clean room, in ``larmorx`` (Apache-2.0), which these
pages' old texts call "the clean-room original" or "the original" (FSL is named "FSL"). FLIRT's
page is the first; FAST's, topup's, fslmaths', fslstats' and prelude's follow it.

Presentation only: texts, the ``port``/``order``/``shows`` fields and the order of ids change;
arrays, metrics and verdicts do not."""

from __future__ import annotations

import re
from collections.abc import Callable

from larmorx_validation.evidence import present as present_

#: The replica's package and licence on the panels, as on BET's page.
REPLICA = "larmorx-nc, FSL Licence"

#: The old texts' words for the two ports and their modes, applied before the guide's:
#: "the original" on these pages is the clean room (FSL is named "FSL"), not in a quoted
#: phrase ("*the original against itself*") nor in "the cache of the original's runs" (the
#: guide's words for the oracle-output cache, where a text is reworded twice).
TEXTS = [
    (r"\bboth LARMORX implementations\b", "both ports"),
    (r"\bThe two implementations\b", "The two ports"),
    (
        r"\b(?:in|each in) (?:its|their) reproduce and default modes\b",
        "in `reproduce` and in `fix`",
    ),
    (
        r"\b(?:in|each in) (?:its|their) default and reproduce modes?\b",
        "in `fix` and in `reproduce`",
    ),
    (r"\bin both modes\b", "in `fix` and in `reproduce`"),
    (r"\bReproduce mode\b", "`reproduce`"),
    (r"\bboth modes give\b", "`fix` and `reproduce` give"),
    # a clean room's reason: what it is, not "a clean-room implementation"
    (
        r"\bA clean-room implementation, so not bit-identical by design:",
        "Written without FSL's source, the clean room is not bit-identical by design:",
    ),
    (r"\bNot a comparison of implementations\b", "Not a comparison of ports"),
    (r"\bA clean-room registration:", "The clean room's registration:"),
    # a part's steps by their number, not the relays that made them
    (r"\(the clean-room catch-up, relay (\d)\)", r"(the clean-room catch-up, its step \1)"),
    (r"\(catch-up relay (\d)\)", r"(the clean-room catch-up, its step \1)"),
    # "the user" reads as the reader on a public page; a reason clipped inside the phrase
    (r"\bnot named: the user's decisi…", "not named, as decided…"),
    (r"(?<!\*)\bThe original's\b(?!')", "The clean room's"),
    (r"(?<!\*)(?<!cache of )\bthe original's\b(?!')", "the clean room's"),
    (r"(?<!\*)\bThe original\b(?! against itself\*)", "The clean room"),
    (r"(?<!\*)(?<!cache of )\bthe original\b(?! against itself\*|'s')", "the clean room"),
]

#: The categories the FSL records share, as the pages name them (a page adds its own).
CATEGORIES = {
    "edge": "edge cases and errors",
    "defects": "defects of FSL",
    "difftest": "random testing",
}

#: Each record's port: the old implementation ids ("original" is the clean room).
PORTS = {"replica": "replica", "original": "clean-room"}

#: A clean room's panel names its licence and crate in its second line ("reproduce mode, 4
#: threads (Apache-2.0, crate larmorx-fsl; the record's run)"): the package line says the
#: licence, the page's facts the crate.
_CRATE = re.compile(r"\(Apache-2\.0, crate larmorx-fsl; ")


def page(program: str, version: str = "6.0.7.23", **kw) -> present_.Page:
    """An FSL page of two ports; ``texts``: the page's own rewordings, before these."""
    texts = list(kw.pop("texts", [])) + list(TEXTS)
    return present_.Page(
        software="FSL",
        program=program,
        version=version,
        packages={"replica": REPLICA},
        texts=texts,
        **kw,
    )


Shows = dict[str, Callable[[dict], tuple[str, list[str]]]]


def present(
    doc: dict,
    pg: present_.Page,
    shows: Shows,
    speed_caption: str,
    speed_labels: dict[str, str],
    categories: dict[str, str] | None = None,
    charts: present_.Page | None = None,
    diffs: dict[str, tuple[str, str]] | None = None,
    tables: dict[str, present_.Page] | None = None,
    examples: dict[str, present_.Page] | None = None,
    speed_page: present_.Page | None = None,
) -> dict:
    """One FSL page: each worked example (its "What this shows" line from ``shows``), the
    agreement charts by their records' ports (``charts``: the page that names what the records
    cover, by default ``pg``), the speed chart, the cases and their categories (``categories``:
    the page's own, beside :data:`CATEGORIES`); ``diffs``: a comparison's label and role where
    its shape names the program twice ("FSL's result − its initial alignment"); ``tables``: a
    page table's own page (a clean room's trial, where a bare "larmorx" is the clean room);
    ``examples``: a worked example's own page, where its call is of another program of the page
    (applytopup's on topup's page); ``speed_page``: the page that names what the speed chart
    covers, by default ``pg``."""
    for ex in doc["examples"]:
        for p in ex["panels"]:
            if p["role"] == "larmorx" and p.get("sub"):
                p["sub"] = _CRATE.sub("(", p["sub"])
        line, verdicts = shows[ex["id"]](ex)
        mine = {k: v for k, v in (diffs or {}).items() if any(d["id"] == k for d in ex["diffs"])}
        own = (examples or {}).get(ex["id"], pg)
        present_.example(own, ex, shows=line, verdicts=verdicts, diffs=mine or None)
        for f in ex.get("facts", []):
            if f["label"] == "Original":  # the clean room's matrix against FSL's
                f["label"] = "Clean room"
    for im in doc["implementations"]:
        present_.implementation(charts or pg, im, "standard comparison", port=PORTS[im["id"]])
    present_.speed(
        speed_page or pg,
        doc.get("speed"),
        speed_caption,
        marks={"larmorx": "clean-room", "larmorx-open": "clean-room-open"},
        labels=speed_labels,
    )
    present_.random_testing(charts or pg, doc, dict(PORTS))
    return present_.finish(pg, doc, {**CATEGORIES, **(categories or {})}, pages=tables, ports=PORTS)
