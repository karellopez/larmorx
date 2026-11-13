# SPDX-License-Identifier: Apache-2.0
"""The table of FreeSurfer 8.2.0's generated surface accessors (queue items 3.1-45 and 3.1-46).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it reads FreeSurfer's source and prints its
functions.

FreeSurfer's ``include/mrisurf_SurfaceFrom{MRIS,MRIS_MP,MRISPV}_generated_suffix.h`` hold, for
each of the three representations of a surface (the ``MRIS`` structure, the metric-properties
``MRIS_MP``, the properties-in-vectors ``MRISPV``) and each of its 13 access levels (``Existence``
to ``AllM``), the bodies of the ``Face``, ``Vertex`` and ``Surface`` views' member functions. This
tool reads those bodies (never the declarations: a body is the behaviour), classifies each as a
read (``return <storage>``), a write (``<storage> = to``), a write with ``cheapAssert`` or other,
and checks that a member function reads and writes the same storage at every level where it
exists. The replica (``larmorx-freesurfer-surf``'s ``views``) is checked against its output:

    python -P validation/investigations/freesurfer-surfio/accessors.py <freesurfer-tag> summary
    python -P validation/investigations/freesurfer-surfio/accessors.py <freesurfer-tag> json <out>
    python -P validation/investigations/freesurfer-surfio/accessors.py <freesurfer-tag> rust <views.rs>

``summary`` prints the counts, every storage that differs between levels (none expected), every
body that is neither a plain read nor a plain write, and the ``which_coords`` cases of each level.
``rust`` checks the replica's ``MetricSurface`` (``larmorx-freesurfer-surf/src/views.rs``) against
the ``MRIS_MP`` views' ``AllM`` level: every getter and setter present with the same read-only
status, but the ones the replica leaves out on purpose (``EXCLUDED``).
"""

from __future__ import annotations

import json
import re
import sys
from collections import defaultdict
from pathlib import Path

REPRS = ("MRIS", "MRIS_MP", "MRISPV")
LEVELS = (
    "Existence",
    "Topology",
    "XYZPosition",
    "XYZPositionConsequences",
    "Distort",
    "Analysis",
    "ExistenceM",
    "TopologyM",
    "XYZPositionM",
    "XYZPositionConsequencesM",
    "DistortM",
    "AnalysisM",
    "AllM",
)
CLASSES = ("Face", "Vertex", "Surface", "MRIS", "MRIS_MP", "MRISPV")

DEF = re.compile(
    r"^\s*(?P<ret>[A-Za-z_][\w:<>,* ]*?)\s+(?P<cls>"
    + "|".join(CLASSES)
    + r")::(?P<name>~?\w+)\s*\((?P<args>[^)]*)\)\s*(?P<const>const)?\s*\{"
)
CTOR = re.compile(r"^\s*(?P<cls>" + "|".join(CLASSES) + r")::(?P=cls)\s*\(")


def strip_comment(line: str) -> str:
    return line.split("//", 1)[0]


def parse(path: Path) -> list[dict]:
    lines = path.read_text(encoding="utf-8").splitlines()
    level = None
    out = []
    i = 0
    while i < len(lines):
        line = lines[i]
        m = re.match(r"^\s*namespace (\w+) \{", line)
        if m and m.group(1) in LEVELS:
            level = m.group(1)
            i += 1
            continue
        if CTOR.match(line):
            i += 1
            continue
        d = DEF.match(strip_comment(line) + ("{" if line.rstrip().endswith("{") else ""))
        d = DEF.match(line) if d is None else d
        if d and level:
            depth = line.count("{") - line.count("}")
            body = [strip_comment(line[line.index("{") + 1 :])]
            j = i
            while depth > 0:
                j += 1
                body.append(strip_comment(lines[j]))
                depth += lines[j].count("{") - lines[j].count("}")
            text = " ".join(" ".join(body).split())
            text = text[: text.rfind("}")].strip()
            out.append(
                {
                    "level": level,
                    "class": d.group("cls"),
                    "name": d.group("name"),
                    "ret": " ".join(d.group("ret").split()),
                    "args": " ".join(d.group("args").split()),
                    "const": bool(d.group("const")),
                    "body": text,
                    "line": i + 1,
                }
            )
            i = j + 1
            continue
        i += 1
    return out


def classify(body: str) -> str:
    if re.fullmatch(r"return [^;]+;", body):
        return "read"
    if re.fullmatch(r"[^;=]+ = [^;=]+;", body):
        return "write"
    if re.fullmatch(r"cheapAssert\(repr == to\.repr\); [^;=]+ = [^;=]+;", body):
        return "write-assert"
    return "other"


def which_cases(body: str) -> list[str]:
    return re.findall(r"CASE\((\w+),(\w*)\)", body)


def table(tag: Path) -> dict:
    res = {}
    for repr_ in REPRS:
        f = tag / "include" / f"mrisurf_SurfaceFrom{repr_}_generated_suffix.h"
        rows = parse(f)
        for r in rows:
            r["kind"] = classify(r["body"])
        res[repr_] = rows
    return res


def summary(t: dict) -> int:
    bad = 0
    for repr_, rows in t.items():
        per_level = defaultdict(int)
        storage = defaultdict(set)
        others = {}
        cases = {}
        for r in rows:
            per_level[(r["level"], r["class"])] += 1
            key = (r["class"], r["name"], r["args"])
            if r["name"] == "which_coords":
                cases[r["level"]] = which_cases(r["body"]) or [("body", r["body"])]
                continue
            storage[key].add(r["body"])
            if r["kind"] == "other":
                others[key] = r["body"]
        print(f"== {repr_}: {len(rows)} member functions")
        for lv in LEVELS:
            print(
                "  "
                + lv
                + ": "
                + ", ".join(f"{c} {per_level[(lv, c)]}" for c in CLASSES if per_level.get((lv, c)))
            )
        diff = {k: v for k, v in storage.items() if len(v) > 1}
        print(f"  distinct functions {len(storage)}; bodies differing between levels: {len(diff)}")
        for k, v in sorted(diff.items()):
            bad += 1
            print(f"    DIFFERS {k}: {sorted(v)}")
        print(f"  bodies that are neither a read nor a write: {len(others)}")
        for k, v in sorted(others.items()):
            print(f"    {k[0]}::{k[1]}({k[2]}) {{ {v} }}")
        for lv in LEVELS:
            if lv in cases:
                print(f"  which_coords {lv}: {' '.join(a + '=' + b for a, b in cases[lv])}")
    return 1 if bad else 0


#: MRIS_MP accessors the replica's trait leaves out (views.rs explains why).
EXCLUDED = {
    ("Face", "norm"),
    ("Surface", "faceNormCacheEntries"),
    ("Surface", "faceNormDeferredEntries"),
    ("Surface", "vertices"),
    ("Surface", "faces"),
}


def rust_check(t: dict, views: Path) -> int:
    text = views.read_text(encoding="utf-8")
    trait = text[text.index("pub trait MetricSurface") :]
    trait = trait[: trait.index("\n            }\n")]
    have = set(re.findall(r"fn (\w+)\(", trait))
    rows = re.findall(r"^\s+(\w+)(?: / (\w+))?: ", text[text.index("    table! {") :], re.M)
    for get, set_ in rows:
        have.add(get)
        if set_:
            have.add(set_)
    prefix = {"Vertex": "v_", "Face": "f_", "Surface": ""}
    want = set()
    for r in t["MRIS_MP"]:
        if r["level"] != "AllM" or r["name"] == "which_coords":
            continue
        base = r["name"][4:] if r["name"].startswith("set_") else r["name"]
        if (r["class"], base) in EXCLUDED:
            continue
        p = prefix[r["class"]]
        want.add(("set_" + p + base) if r["name"].startswith("set_") else (p + base))
    missing, extra = sorted(want - have), sorted(have - want)
    print(f"MRIS_MP AllM accessors: {len(want)}; replica's: {len(have & want)}")
    print(f"  missing in the replica: {missing}")
    print(f"  in the replica only: {extra}")
    return 1 if missing or extra else 0


def main(argv: list[str]) -> int:
    if len(argv) >= 3 and argv[1] == "rust":
        return rust_check(table(Path(argv[0])), Path(argv[2]))
    if len(argv) < 2 or argv[1] not in ("summary", "json"):
        print(__doc__)
        return 2
    t = table(Path(argv[0]))
    if argv[1] == "summary":
        return summary(t)
    Path(argv[2]).write_text(json.dumps(t, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
