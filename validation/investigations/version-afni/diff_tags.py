# SPDX-License-Identifier: Apache-2.0
"""Diff the AFNI files larmorx's AFNI ports name between two release trees.

Usage (from the repository, with ``LARMORX_WORKSPACE`` or ``--workspace``)::

    python -P validation/investigations/version-afni/diff_tags.py \
        --old AFNI-25.2.09 --new AFNI-26.3.05 --out <scratch>/diffs [--all]

It reads every ``src/...`` path named in ``crates-gpl/larmorx-gpl-afni/PROVENANCE.md`` and
``crates/larmorx-afni/PROVENANCE.md`` (plus ``--extra`` files), and for each prints the number
of changed lines, and how many of them are licence-header lines (lines that mention the
Medical College of Wisconsin, the GPL, Creative Commons or a copyright notice). With ``--out``
it writes one unified diff per changed file. ``--all`` lists every file of ``src/`` that
changed (for the macros and routines reached outside the listed files).

It reads only the two trees and writes only into ``--out``; it never runs git in them.
"""

from __future__ import annotations

import argparse
import difflib
import json
import os
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PROVENANCE = (
    REPO / "crates-gpl" / "larmorx-gpl-afni" / "PROVENANCE.md",
    REPO / "crates" / "larmorx-afni" / "PROVENANCE.md",
)
LICENCE_WORDS = re.compile(
    r"Medical College|General Public License|Gnu General|GPL|Creative Commons|CC BY|"
    r"copyright|Copyright|COPYRIGHT|public domain|License|license|LICENSE",
)


def named_files(extra: list[str]) -> list[str]:
    files: set[str] = set(extra)
    for p in PROVENANCE:
        for m in re.finditer(r"`(src/[^`]+)`", p.read_text(encoding="utf-8")):
            path = m.group(1)
            if path.endswith("/"):
                continue
            files.add(path)
    return sorted(files)


def changed_lines(a: list[str], b: list[str]) -> tuple[list[str], list[str]]:
    removed, added = [], []
    for line in difflib.unified_diff(a, b, lineterm="", n=0):
        if line.startswith(("---", "+++", "@@")):
            continue
        if line.startswith("-"):
            removed.append(line[1:])
        elif line.startswith("+"):
            added.append(line[1:])
    return removed, added


def read_lines(p: Path) -> list[str]:
    return p.read_text(encoding="latin-1").splitlines()


def compare(old: Path, new: Path, rel: str) -> dict:
    po, pn = old / rel, new / rel
    if not po.exists() or not pn.exists():
        return {"file": rel, "old": po.exists(), "new": pn.exists()}
    a, b = read_lines(po), read_lines(pn)
    removed, added = changed_lines(a, b)
    lic = sum(1 for x in removed + added if LICENCE_WORDS.search(x))
    blank = sum(1 for x in removed + added if not x.strip())
    return {
        "file": rel,
        "removed": len(removed),
        "added": len(added),
        "licence_lines": lic,
        "blank_lines": blank,
        "other_lines": len(removed) + len(added) - lic - blank,
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--workspace", default=os.environ.get("LARMORX_WORKSPACE", str(REPO.parent)))
    ap.add_argument("--old", default="AFNI-25.2.09")
    ap.add_argument("--new", default="AFNI-26.3.05")
    ap.add_argument("--out", help="directory for one unified diff per changed file")
    ap.add_argument("--extra", nargs="*", default=[], help="more src/ paths to compare")
    ap.add_argument("--all", action="store_true", help="also list every changed file of src/")
    ap.add_argument("--json", help="write the table as JSON here")
    args = ap.parse_args(argv)
    tags = Path(args.workspace) / "tags"
    old, new = tags / args.old, tags / args.new
    rows = [compare(old, new, f) for f in named_files(args.extra)]
    out = Path(args.out) if args.out else None
    if out:
        out.mkdir(parents=True, exist_ok=True)
    for r in rows:
        if "removed" not in r:
            print(f"{'?':>5} {r['file']}  (old: {r['old']}, new: {r['new']})")
            continue
        n = r["removed"] + r["added"]
        print(f"{n:5d} {r['file']}  (licence {r['licence_lines']}, other {r['other_lines']})")
        if out and n:
            a, b = read_lines(old / r["file"]), read_lines(new / r["file"])
            d = difflib.unified_diff(
                a, b, f"{args.old}/{r['file']}", f"{args.new}/{r['file']}", lineterm=""
            )
            (out / (r["file"].replace("/", "_") + ".diff")).write_text(
                "\n".join(d) + "\n", encoding="utf-8"
            )
    if args.all:
        print("\nEvery changed file of src/ (code files):")
        for p in sorted((new / "src").rglob("*")):
            if not p.is_file() or p.suffix not in {".c", ".h"}:
                continue
            rel = str(p.relative_to(new))
            po = old / rel
            if not po.exists():
                print(f"  new   {rel}")
                continue
            if po.read_bytes() != p.read_bytes():
                r = compare(old, new, rel)
                print(f"  {r['removed'] + r['added']:5d} {rel} (other {r['other_lines']})")
    if args.json:
        Path(args.json).write_text(json.dumps(rows, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
