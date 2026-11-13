# SPDX-License-Identifier: Apache-2.0
"""The writing guide's presentation of AFNI's pages of two ports, 3dTshift, 3dAutomask and
3dUnifize (``docs/writing-guide.md``, sections 1, 4 and 5; :mod:`present` does the work): the
replica (in LARMORX itself since 2026-10-29, Apache-2.0 with CC BY 4.0's attribution) and the
clean room, which these pages' old texts call "the original".

Presentation only: texts, the ``port``/``order``/``shows`` fields and the order of ids change;
arrays, metrics and verdicts do not."""

from __future__ import annotations

from collections.abc import Callable

from larmorx_validation.evidence import present as present_

#: The replica's package and licence on the panels.
REPLICA = "larmorx, Apache-2.0 with CC BY 4.0's attribution"

#: The old texts' words for the two ports, applied before the guide's: "the original" on these
#: pages is the clean room (AFNI is named "AFNI"), "the larmorx replica" the replica; two
#: sentences written before both ports' records followed AFNI 26.3.05 (their records' reference
#: since 2026-10-30: docs/validation/afni-*.md, "Reference") say what the records say now.
TEXTS = [
    (r"\bthe larmorx replica\b", "the replica"),
    (r"\bboth LARMORX implementations\b", "both ports"),
    (r"\bThe two implementations\b", "The two ports"),
    (r"\bReproduce mode\b", "`reproduce`"),
    (
        r"\b(?:in|each in) (?:its|their) default and reproduce modes?\b",
        "in `fix` and in `reproduce`",
    ),
    (r"\bin both modes\b", "in `fix` and in `reproduce`"),
    (r"\bboth modes give\b", "`fix` and `reproduce` give"),
    # a row named, not placed: the guide's order puts the rows on the usual encoding first
    (r"\(next rows\)", "(the rows on the usual encoding)"),
    (
        r"\bthe GPL replicas?'s? reference since 2026-10-29\b",
        "the replica's reference since 2026-10-29",
    ),
    (
        r"\(compared with AFNI 25\.2\.09, its reference until it follows 26\.3\.05\)",
        "(compared with AFNI 25.2.09 and 26.3.05, its record's reference)",
    ),
    (
        r"The clean-room original is compared with AFNI 25\.2\.09, its reference until it "
        r"follows 26\.3\.05's two fixes\.",
        "The clean room is compared with both releases; its record's reference is AFNI 26.3.05.",
    ),
    # the oracle's words (the guide's section 1): the repository's builds of AFNI
    (r"\bthe repository's oracle builds\b", "the repository's builds"),
    (r"\bgive the oracle's bytes\b", "give the bytes of the repository's build"),
    (r"\bAFNI's oracle output\b", "the output of the repository's AFNI build"),
    (r"\bThe original's\b", "The clean room's"),
    (r"\bthe original's\b", "the clean room's"),
    (r"\bThe original\b", "The clean room"),
    (r"\bthe original\b", "the clean room"),
]

#: The categories of the records' cases as the pages name them.
CATEGORIES = {
    "defects": "defects of AFNI",
    "synthetic": "synthetic series",
    "methods": "interpolation methods",
    "tpattern": "slice patterns (-tpattern)",
    "header-timing": "slice times from the header",
    "geometry": "geometry",
    "copy": "copies of the input",
    "global": "global options",
    "environment": "environment variables",
    "divergence": "expected divergences",
    "divergences": "expected divergences",
    "datatypes": "data types",
    "dimensions": "dimensions",
    "AFNI-75's guard": "AFNI-75's guard",
    "GLIBC-05's cube root": "GLIBC-05's cube root",
}

#: The runs of a worked example's time-course chart, by their new names.
SERIES = {"larmorx replica": "replica", "clean-room original": "clean room"}

#: Each record's port: the old implementation ids ("original" is the clean room).
PORTS = {"replica": "replica", "original": "clean-room"}


def page(program: str) -> present_.Page:
    return present_.Page(
        software="AFNI",
        program=program,
        version="26.3.05",
        packages={"replica": REPLICA},
        versions_in_labels=True,
        texts=list(TEXTS),
    )


def largest(ex: dict, diff: str) -> str:
    """A comparison's largest difference, as the "What this shows" lines give it."""
    d = next(x for x in ex["diffs"] if x["id"] == diff)
    return present_._sig(max(m["max_abs"] for m in d["metrics"].values()))


Shows = dict[str, Callable[[dict], tuple[str, list[str]]]]


def present(
    doc: dict,
    pg: present_.Page,
    shows: Shows,
    speed_caption: str,
    speed_labels: dict[str, str],
    speed_version: str,
) -> dict:
    """One AFNI page: each worked example (its "What this shows" line from ``shows``, else the
    data panel's from its verdicts), the agreement charts by their records' ports, the speed
    chart (``speed_version``: the AFNI its timings ran), the cases and their categories."""
    for ex in doc["examples"]:
        for ser in ex.get("series") or []:  # a voxel's time course: each run by its name
            ser["label"] = SERIES.get(ser["label"], ser["label"])
        if ex["id"] in shows:
            line, verdicts = shows[ex["id"]](ex)
        else:
            line, verdicts = present_.panel_shows(pg, ex)
        present_.example(pg, ex, shows=line, verdicts=verdicts)
    for im in doc["implementations"]:
        present_.implementation(pg, im, "standard comparison", port=PORTS[im["id"]])
    sp = doc.get("speed")
    present_.speed(
        pg,
        sp,
        speed_caption,
        marks={"larmorx": "clean-room", "larmorx-open": "clean-room-open"},
        labels=speed_labels,
    )
    if sp:
        sp["title"] = (
            f"Speed, both ports against the original: AFNI `{pg.program}` {speed_version}"
            if speed_version
            else f"Speed, both ports against the original: AFNI `{pg.program}`"
        )
    present_.random_testing(pg, doc, dict(PORTS))
    return present_.finish(pg, doc, CATEGORIES, ports=PORTS)
