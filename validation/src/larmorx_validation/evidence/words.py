# SPDX-License-Identifier: Apache-2.0
"""The writing guide's words and order on an evidence page (``docs/writing-guide.md``, sections
1, 4 and 5): each panel and comparison names its port, every comparison says what it compares,
the panels and comparisons go in one order, and each worked example says what it shows.

This is presentation only. It runs on a tool's finished export and changes texts, adds the
fields the site reads (``port``, ``order``, ``shows``) and reorders lists of ids; the arrays,
the metrics and the verdicts are the export's own, so every blob stays byte for byte. The
panels' and comparisons' lists keep their order in the JSON (the site's number check and the
merge's inventory address numbers by their position); ``order`` gives the order shown.

A page that has been through it carries ``"presentation": "writing-guide"``: the site's
renderers then show the ports' names, colours and marks, the order and the "What this shows"
line (larmorx_documentation's ``tools/evidence.py`` and ``static/js/evidence.js``).
"""

from __future__ import annotations

import re

PRESENTATION = "writing-guide"

#: The ports and the other roles a panel can have, in the guide's order (section 4).
PORT_ORDER = ("input", "original", "fmriprep", "replica", "clean-room")
#: A port's name in labels and captions.
PORT_NAME = {"replica": "replica", "clean-room": "clean room"}
#: A panel role's port where the role says it (a LARMORX panel names its port itself).
ROLE_PORT = {
    "input": "input",
    "original": "original",
    "reference": "original",
    "fmriprep": "fmriprep",
}

#: The old words of the evidence's texts and the new ones (the guide's section 1), applied in
#: order. Each pair says the same thing; a text whose facts would change is not reworded here.
_REWORD: list[tuple[str, str, str]] = [
    # (pattern, with code spans, without them: for texts the site shows as plain text)
    (r"\bLARMORX's clean-room original\b", "LARMORX's clean room", "LARMORX's clean room"),
    (r"\blarmorx's clean-room original\b", "the clean room", "the clean room"),
    (r"\b[Ll]armorx clean-room original\b", "the clean room", "the clean room"),
    (r"\bthe larmorx-nc replica\b", "the replica (larmorx-nc)", "the replica (larmorx-nc)"),
    (r"\bThe clean-room original\b", "The clean room", "The clean room"),
    (r"\bthe clean-room original\b", "the clean room", "the clean room"),
    (r"\bClean-room originals?\b", "Clean room", "Clean room"),
    (r"\bclean-room originals\b", "clean rooms", "clean rooms"),
    (r"\bclean-room original\b", "clean room", "clean room"),
    (r"\bboth implementations\b", "both ports", "both ports"),
    (
        r"\bin its default and reproduce modes\b",
        "in `fix` and in `reproduce`",
        "in fix and in reproduce",
    ),
    (r"\bdefault and reproduce modes\b", "`fix` and `reproduce`", "fix and reproduce"),
    (r"\bIn its default mode\b", "In `fix`, its default,", "In fix, its default,"),
    (r"\bin its default mode\b", "in `fix`, its default,", "in fix, its default,"),
    (r"\bThe default mode\b", "The default, `fix`,", "The default, fix,"),
    (r"\bthe default mode\b", "the default, `fix`,", "the default, fix,"),
    (r"\bits default mode\b", "its default, `fix`,", "its default, fix,"),
    (r"'s default mode\b", "'s default, `fix`,", "'s default, fix,"),
    (r"\bdefault mode\b", "`fix`, the default", "fix, the default"),
    (r"\bfMRIPrep oracle run's\b", "fMRIPrep run's", "fMRIPrep run's"),
    # a row named, not placed: the guide's order moves the rows
    (
        r"\(the row (?:above|of the reproduce mode)\)",
        "(the row of `reproduce` against FSL in this table)",
        "(the row of reproduce against FSL in this table)",
    ),
    (
        r"\bThe original's reproduce mode\b",
        "The clean room's `reproduce`",
        "The clean room's reproduce",
    ),
    (r"\bBoth reproduce modes give\b", "In `reproduce` both give", "In reproduce both give"),
    (r"\bThe reproduce mode\b", "`reproduce`", "reproduce"),
    (r"\bBoth reproduce modes?\b", "Both in `reproduce`", "Both in reproduce"),
    (r"\b(?:the|its) reproduce mode\b", "`reproduce`", "reproduce"),
    (r"\breproduce mode\b", "`reproduce`", "reproduce"),
    (
        r"\bthe oracle-output cache\b",
        "the cache of the original's runs",
        "the cache of the original's runs",
    ),
    (
        r"\bthe oracle cache\b",
        "the cache of the original's runs",
        "the cache of the original's runs",
    ),
    (r"\bstandard tier\b", "standard comparison", "standard comparison"),
    (r"\bfull tier\b", "full comparison", "full comparison"),
    (r"\bthe full-tier record\b", "the full comparison's record", "the full comparison's record"),
    (
        r"\bthe standard record\b",
        "the standard comparison's record",
        "the standard comparison's record",
    ),
    (r"\bthe parity suite's\b", "the comparison's", "the comparison's"),
    # D13, a decision's number: its section of a record, by what it lists (a page that knows the
    # record links it: FIX_LIST)
    (
        r"\(the record's D13 section\)",
        "(the record's list of what `fix` changed)",
        "(the record's list of what fix changed)",
    ),
    # "the user" reads as the reader on a public page: the project's decision, by its date
    (
        r"\bnamed nowhere by the user's decision of (\d{4}-\d\d-\d\d)",
        r"named nowhere, as decided on \1",
        r"named nowhere, as decided on \1",
    ),
    (
        r"\bnot named: the user's decision of (\d{4}-\d\d-\d\d) for the iterative programs\b",
        r"not named in the iterative programs, as decided on \1",
        r"not named in the iterative programs, as decided on \1",
    ),
    (r"\(the user's decision of (\d{4}-\d\d-\d\d)\)", r"(as decided on \1)", r"(as decided on \1)"),
    (
        r"\bD13's rule for failure cases\b",
        "the rule of the corrections by default for failure cases",
        "the rule of the corrections by default for failure cases",
    ),
    # "original" where it names the clean room: the forms the bet page's texts hold
    (r"\(the original against FSL", "(the clean room against FSL", "(the clean room against FSL"),
    (r"\bthe original names\b", "the clean room names", "the clean room names"),
    (r"\bthe original could still\b", "the clean room could still", "the clean room could still"),
    (r"\bthe original's default\b", "the clean room's default", "the clean room's default"),
    (r"\bthe original: specs/", "the clean room: specs/", "the clean room: specs/"),
    (r"^the original: ", "the clean room: ", "the clean room: "),
    (
        r"\bThe original's differing cases\b",
        "The clean room's differing cases",
        "The clean room's differing cases",
    ),
]
_REWORD_RX = [(re.compile(p), code, plain) for p, code, plain in _REWORD]

#: The list a record keeps of what ``fix`` changed, as :func:`reword` names it.
FIX_LIST = "(the record's list of what `fix` changed)"


#: The register of the originals' defects, where an id is linked (its summary table).
REGISTER = "docs/findings/upstream-defects.md#summary"
_REGISTER_ID = re.compile(
    r"(?<![\[\w/#-])((?:ANTS|ITK|AFNI|FSL|GLIBC|NP|SCI|TED|PIPE|PD)-\d+)(?![\w\]-])"
)


def link_ids(text: str) -> str:
    """``text`` with every register id (FSL-09, GLIBC-07) linked to the register's summary
    table, outside code spans and existing links."""
    parts = re.split(r"(`[^`]*`|\[[^\]]*\]\([^)]*\))", text)
    return "".join(
        part if i % 2 else _REGISTER_ID.sub(rf"[\1](repo:{REGISTER})", part)
        for i, part in enumerate(parts)
    )


def fix_list_link(text: str | None, record: str) -> str | None:
    """``text`` with the record's list of what ``fix`` changed linked to ``record`` (a path in
    the repository, ``docs/validation/fsl-bet-replica.md``)."""
    if not text:
        return text
    return text.replace(FIX_LIST, f"([the record's](repo:{record}) list of what `fix` changed)")


def reword(text: str | None, code: bool = True) -> str | None:
    """A text in the guide's words; ``code=False`` for a text the site shows without Markdown."""
    if not text:
        return text
    for rx, with_code, plain in _REWORD_RX:
        text = rx.sub(with_code if code else plain, text)
    return text


def program_label(software: str, program: str, version: str) -> str:
    """The guide's form of an original in labels: "FSL bet 6.0.7.23" (rule 9, no code spans:
    the site shows panel labels as plain text)."""
    return f"{software} {program} {version}".replace("  ", " ")


def example(
    ex: dict,
    *,
    ports: dict[str, str],
    panels: dict[str, tuple[str, str | None]],
    diffs: dict[str, tuple[str, str]],
    groups: list[tuple[str, list[str]]],
    shows: str,
    verdicts: list[str],
    rank: dict[str, float] | None = None,
) -> dict:
    """One worked example in the guide's words and order.

    ``ports``: the port of each LARMORX panel (``replica`` or ``clean-room``); the other panels
    take theirs from their role. ``panels``: each panel's new label and second line (None keeps
    it). ``diffs``: each comparison's new label (A − B, LARMORX's port first) and what it
    compares (the guide's section 4). ``groups``: the metrics table's groups, each a title and
    its comparisons, in the order shown; every comparison in exactly one. ``shows``: the "What
    this shows" sentence; ``verdicts``: the comparisons whose verdicts stand beside it, one per
    port. ``rank``: a panel's place within its port where the list's order is not the guide's
    (an original's other builds after its versions)."""
    pid = [p["id"] for p in ex["panels"]]
    did = [d["id"] for d in ex["diffs"]]
    if set(panels) != set(pid):
        raise ValueError(f"{ex['id']}: panels {sorted(set(pid) ^ set(panels))} not presented")
    if set(diffs) != set(did):
        raise ValueError(f"{ex['id']}: comparisons {sorted(set(did) ^ set(diffs))} not presented")
    grouped = [d for _, ds in groups for d in ds]
    if sorted(grouped) != sorted(did):
        raise ValueError(f"{ex['id']}: the groups do not hold every comparison exactly once")
    port_of: dict[str, str] = {}
    for p in ex["panels"]:
        port = ports.get(p["id"]) or ROLE_PORT.get(p["role"])
        if port not in PORT_ORDER:
            raise ValueError(f"{ex['id']}: panel {p['id']} ({p['role']}) has no port")
        if p["role"] == "larmorx" and port not in PORT_NAME:
            raise ValueError(f"{ex['id']}: LARMORX's panel {p['id']} is not a port")
        port_of[p["id"]] = port
        label, sub = panels[p["id"]]
        p["port"] = port
        p["label"] = label
        if sub is not None:
            p["sub"] = sub
    for d in ex["diffs"]:
        d["label"], d["role_label"] = diffs[d["id"]]
        d["port"] = port_of[d["a"]]
        d["reason"] = reword(d.get("reason"))
    rank = rank or {}
    place = {p: i for i, p in enumerate(pid)}

    def key(p: str) -> tuple:
        return (PORT_ORDER.index(port_of[p]), rank.get(p, 0.0), place[p])

    panel_order = sorted(pid, key=key)
    diff_order = grouped
    ex["order"] = {
        "panels": panel_order,
        "diffs": diff_order,
        "groups": [[title, ds] for title, ds in groups],
    }
    ex["default"]["panels"] = sorted(ex["default"]["panels"], key=panel_order.index)
    ex["default"]["diffs"] = sorted(ex["default"]["diffs"], key=diff_order.index)
    by_id = {d["id"]: d for d in ex["diffs"]}
    shown = []
    for v in verdicts:
        d = by_id[v]
        if not d.get("verdict"):
            raise ValueError(f"{ex['id']}: {v} carries no verdict")
        shown.append({"diff": v, "port": d["port"], "verdict": d["verdict"]})
    if len({s["port"] for s in shown}) != len(shown):
        raise ValueError(f"{ex['id']}: one verdict per port beside its line")
    ex["shows"] = {"text": shows, "verdicts": shown}
    ex["summary"] = reword(ex.get("summary"))
    for f in ex.get("facts", []):
        f["label"] = reword(f["label"], code=False)
        f["text"] = reword(f["text"])
    ex["metrics_note"] = reword(ex.get("metrics_note"))
    ex.pop("groups", None)  # replaced by order.groups
    return ex


def cases(cs: list[dict]) -> None:
    """Every case's texts in the guide's words (the description is shown as plain text, the
    reasons with Markdown)."""
    for c in cs:
        c["description"] = reword(c.get("description"), code=False)
        for r in c["results"].values():
            r["reason"] = reword(r.get("reason"))


def table(t: dict) -> None:
    t["title"] = reword(t.get("title"))
    t["note"] = reword(t.get("note"))
    for r in t["rows"]:
        r["head"] = reword(r.get("head"))
        if r.get("sub"):
            r["sub"] = reword(r["sub"])
        for c in r["cells"]:
            if "text" in c:
                c["text"] = reword(c["text"])
