# SPDX-License-Identifier: Apache-2.0
# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: part of FreeSurfer's ITK 4.13.2 comparison.
"""Compare VNL's template definitions between two ITK trees, function by function.

``itk_diff.py`` compares whole files; for the large VNL containers (``vnl_matrix.hxx``,
``vnl_vector.hxx``, ...) this tool keys every out-of-line definition by its name and normalised
parameter list and says which bodies are identical after ``itk_diff.norm_mech``, which differ and
which have no counterpart (third-party.md section 10).

Usage::

    python -P itk_functions.py <itk-4.13.2>/Modules/ThirdParty/VNL/src/vxl/core/vnl \\
        <itk-5.4.5>/Modules/ThirdParty/VNL/src/vxl/core/vnl vnl_matrix.hxx vnl_vector.hxx
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import itk_diff as t

DEF = re.compile(
    r"((?:template\s*<[^>]*>\s*)?[\w:<>,&*\s]+?(vnl_\w+<T(?:,[^>]*)?>::[~\w]+|operator\s*\S+?)"
    r"\s*\(([^;{)]*)\)\s*(const)?\s*)\{"
)


def definitions(path: Path) -> dict[str, str]:
    g = t.strip_comments(path.read_text(encoding="latin-1"))
    g = re.sub(r"^\s*#[^\n]*$", " ", g, flags=re.M)
    out: dict[str, str] = {}
    for m in DEF.finditer(g):
        depth, k = 0, m.end() - 1
        while k < len(g):
            if g[k] == "{":
                depth += 1
            elif g[k] == "}":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        key = m.group(2) + "(" + " ".join(t.norm_mech(m.group(3))) + ")" + (m.group(4) or "")
        out.setdefault(re.sub(r"\s+", " ", key), " ".join(t.norm_mech(g[m.end() : k])))
    return out


def main(argv: list[str]) -> int:
    old, new = Path(argv[1]), Path(argv[2])
    for name in argv[3:]:
        a, b = definitions(old / name), definitions(new / name)
        same = [k for k in a if b.get(k) == a[k]]
        diff = [k for k in a if k in b and b[k] != a[k]]
        gone = [k for k in a if k not in b]
        print(
            f"{name}: {len(a)} definitions; same {len(same)}, differ {len(diff)}, not found {len(gone)}"
        )
        for k in diff:
            print("   differs  ", k[:120])
        for k in gone:
            print("   not found", k[:120])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
