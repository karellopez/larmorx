# SPDX-License-Identifier: Apache-2.0
"""The licence of every AFNI file larmorx's AFNI replicas derive from, in two releases.

Usage::

    python -P validation/investigations/version-afni/licence_inventory.py \
        [--workspace W] [--old AFNI-25.2.09] [--new AFNI-26.3.05] [--markdown]

For each ``src/...`` path named in ``crates-gpl/larmorx-gpl-afni/PROVENANCE.md`` it reads the
file's opening comment in both trees and prints the licence statement it carries, word for word
(the MCW notice, the NIfTI library's public-domain line), or "no notice" (a file written at the
NIH: public domain by AFNI's ``LICENSE.txt``). Licence text only: nothing of the routines.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PROVENANCE = REPO / "crates-gpl" / "larmorx-gpl-afni" / "PROVENANCE.md"
#: Sentences in an opening comment that state a licence.
STATEMENT = re.compile(
    r"(Major portions of this software are copyrighted by the Medical College\s+of\s+Wisconsin,"
    r"\s+1994-2000,\s+and are released under the [^.]*?(?:Version 2|\(CC BY 4\.0\))\.?"
    r"|released to the public domain\.?"
    r"|This code is released to the public domain\.?"
    r"|GNU (?:Lesser )?General Public License[^.]*\.?)",
    re.S,
)


def named_files() -> list[str]:
    text = PROVENANCE.read_text(encoding="utf-8")
    return sorted({m for m in re.findall(r"`(src/[^`]+)`", text) if not m.endswith("/")})


def opening(path: Path, lines: int = 40) -> str:
    with path.open(encoding="latin-1") as f:
        head = "".join(f.readline() for _ in range(lines))
    return head


def statement(path: Path) -> str:
    if not path.is_file():
        return "(file missing)"
    found = STATEMENT.findall(opening(path))
    if not found:
        return "no notice (NIH: public domain, AFNI's LICENSE.txt)"
    return " / ".join(" ".join(s.split()) for s in found)


def kind(text: str) -> str:
    if text.startswith("no notice"):
        return "no notice"
    if "CC BY 4.0" in text:
        return "MCW, CC BY 4.0"
    if "Medical College" in text and "Version 2" in text:
        return "MCW, GPL-2"
    if "public domain" in text.lower():
        return "public domain (stated)"
    if "General Public License" in text:
        return "GPL (other)"
    return text


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--workspace", default=os.environ.get("LARMORX_WORKSPACE"))
    ap.add_argument("--old", default="AFNI-25.2.09")
    ap.add_argument("--new", default="AFNI-26.3.05")
    ap.add_argument("--markdown", action="store_true")
    args = ap.parse_args(argv)
    ws = Path(args.workspace) if args.workspace else None
    if ws is None:
        ws = next(p for p in REPO.parents if (p / "tags").is_dir())
    old, new = ws / "tags" / args.old, ws / "tags" / args.new
    rows = []
    for rel in named_files():
        a, b = statement(old / rel), statement(new / rel)
        rows.append((rel, kind(a), kind(b), a, b))
    if args.markdown:
        print(f"| File | {args.old} | {args.new} |")
        print("|---|---|---|")
        for rel, ka, kb, _, _ in rows:
            print(f"| `{rel}` | {ka} | {kb} |")
        print()
        texts = sorted({x for r in rows for x in r[3:5]})
        print("Statements, word for word:")
        for t in texts:
            print(f"- {t}")
    else:
        for rel, ka, kb, _, _ in rows:
            print(f"{rel:40s} {ka:20s} -> {kb}")
    still = [r[0] for r in rows if "GPL" in r[2]]
    print(f"\n{len(rows)} files; still GPL in {args.new}: {still or 'none'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
