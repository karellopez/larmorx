# SPDX-License-Identifier: Apache-2.0
"""Recount the defect register from its summary table (docs/findings/upstream-defects.md).

Usage: python -P register_counts.py   (from the checkout)
Prints the entries per class (an entry with two classes counts under its first; "c (c? until …)"
counts as (c)), the questionable ones, the gap column, and every entry whose gap is "decide" or
"yes" — each "decide" must be listed under "Waiting for the user" in docs/next-steps.md.
Rows marked † (restricted) are counted too.
"""

import re
from collections import Counter
from pathlib import Path

s = Path("docs/findings/upstream-defects.md").read_text()
i = s.index("## Summary")
j = s.index("\n## ", i + 5)
rows = [ln for ln in s[i:j].splitlines() if re.match(r"\| [A-Z]+-\d+( †)? \|", ln)]
cls, q, gap, listed = Counter(), Counter(), Counter(), {"decide": [], "yes": []}
for row in rows:
    c = [x.strip() for x in row.strip().strip("|").split("|")]
    m = re.match(r"([a-f])(\?)?", c[3].split(",")[0].strip())
    if m:
        cls[m.group(1)] += 1
        if m.group(2) and "until" not in c[3]:
            q[m.group(1)] += 1
    g = c[-1]
    key = (
        "decide"
        if g.startswith("decide")
        else "yes"
        if g.startswith("yes")
        else "rule 8"
        if "rule 8" in g
        else "other"
    )
    gap[key] += 1
    if key in listed:
        listed[key].append(c[0])
print(
    f"{len(rows)} entries; classes {dict(sorted(cls.items()))}; questionable {dict(q)}; gap {dict(gap)}"
)
for k, v in listed.items():
    print(f"{k}: {', '.join(v) if v else '-'}")
