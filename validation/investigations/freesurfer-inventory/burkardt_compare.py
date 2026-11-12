# SPDX-License-Identifier: Apache-2.0
# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: reads FreeSurfer 8.2.0's source.
"""Compare FreeSurfer's cubic spline routines with John Burkardt's spline library.

FreeSurfer's ``utils/numerics.cpp`` holds ``SplineCubicSet``, ``SplineCubicValue`` and
``d3_np_fs`` (float), credited to Burkardt (third-party.md, TP-04). Burkardt's page carried no
licence statement in 2006, the GNU LGPL from about 2010 to 2022 and the MIT licence since about
2024 (placement table, third-party.md section 9). This tool extracts each routine's body from
FreeSurfer's file and from one or more Burkardt ``spline.C``/``spline.cpp`` files, removes
comments and blanks, maps Burkardt's names and ``double`` to FreeSurfer's, and prints, per
routine, whether the token streams are identical and the differing lines otherwise.

Usage::

    python -P burkardt_compare.py <freesurfer-tag>/utils/numerics.cpp <burkardt-file> [...]
"""

from __future__ import annotations

import difflib
import re
import sys
from pathlib import Path

PAIRS = {
    "SplineCubicSet": "spline_cubic_set",
    "SplineCubicValue": "spline_cubic_val",
    "d3_np_fs": "d3_np_fs",
}


def strip_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    return re.sub(r"//[^\n]*", " ", text)


def body(text: str, name: str) -> str | None:
    """The definition of ``name`` (signature and braces), comments removed."""
    clean = strip_comments(text)
    for m in re.finditer(r"\b" + re.escape(name) + r"\s*\(", clean):
        # a definition: the closing parenthesis is followed by '{', not ';'
        depth, i = 0, m.end() - 1
        while i < len(clean):
            if clean[i] == "(":
                depth += 1
            elif clean[i] == ")":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        j = i + 1
        while j < len(clean) and clean[j].isspace():
            j += 1
        if j >= len(clean) or clean[j] != "{":
            continue
        start = clean.rfind("\n", 0, m.start()) + 1
        depth, k = 0, j
        while k < len(clean):
            if clean[k] == "{":
                depth += 1
            elif clean[k] == "}":
                depth -= 1
                if depth == 0:
                    return clean[start : k + 1]
            k += 1
    return None


def tokens(code: str, rename: dict[str, str]) -> list[str]:
    toks = re.findall(r"[A-Za-z_][A-Za-z_0-9]*|\d+\.\d*(?:[eE][-+]?\d+)?|\d+|\S", code)
    out = []
    for t in toks:
        t = rename.get(t, t)
        out.append(t)
    return out


def lines_of(toks: list[str]) -> list[str]:
    """One statement per line, for a readable diff."""
    lines, cur = [], []
    for t in toks:
        cur.append(t)
        if t in (";", "{", "}"):
            lines.append(" ".join(cur))
            cur = []
    if cur:
        lines.append(" ".join(cur))
    return lines


def main(argv: list[str]) -> int:
    fs_text = Path(argv[1]).read_text(encoding="utf-8", errors="replace")
    rename_b = {"double": "float"} | {b: f for f, b in PAIRS.items()}
    for other in argv[2:]:
        b_text = Path(other).read_text(encoding="utf-8", errors="replace")
        print(f"== {other}")
        for fs_name, b_name in PAIRS.items():
            fb, bb = body(fs_text, fs_name), body(b_text, b_name)
            if fb is None or bb is None:
                print(
                    f"  {fs_name}: missing (FreeSurfer {fb is not None}, Burkardt {bb is not None})"
                )
                continue
            ft, bt = tokens(fb, {}), tokens(bb, rename_b)
            if ft == bt:
                print(f"  {fs_name}: identical token streams ({len(ft)} tokens)")
                continue
            sm = difflib.SequenceMatcher(a=bt, b=ft, autojunk=False)
            print(
                f"  {fs_name}: differs (similarity {sm.ratio():.3f}; {len(bt)} vs {len(ft)} tokens)"
            )
            for line in difflib.unified_diff(
                lines_of(bt), lines_of(ft), "burkardt", "freesurfer", n=0, lineterm=""
            ):
                print("    " + line)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
