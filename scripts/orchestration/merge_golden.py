# SPDX-License-Identifier: Apache-2.0
"""Resolve a merge conflict in tests/golden/golden.json case by case (three-way, against the merge base).

Usage (during a conflicted `git merge`, from the checkout): python -P merge_golden.py
Each case takes the side that changed it; a case both sides changed differently is reported and the
merge stops (exit 1). Afterwards re-record from the committed merge (scripts/record_golden.py): the
checksums must come out unchanged; only the "recorded" line changes.
"""

import json
import subprocess
import sys
from pathlib import Path


def show(ref):
    return json.loads(subprocess.check_output(["git", "show", f"{ref}:tests/golden/golden.json"]))


def main():
    base = subprocess.check_output(["git", "merge-base", "HEAD", "MERGE_HEAD"]).decode().strip()
    base_doc, ours, theirs = show(base), show("HEAD"), show("MERGE_HEAD")
    src = Path("scripts/record_golden.py").read_text()
    ns = {}
    exec("import json\n" + src[src.index("def _one") : src.index("def record(")], ns)
    merged, conflicts = {}, []
    for k in sorted(set(base_doc["cases"]) | set(ours["cases"]) | set(theirs["cases"])):
        b, o, t = base_doc["cases"].get(k), ours["cases"].get(k), theirs["cases"].get(k)
        v = o if o == t else (t if o == b else (o if t == b else None))
        if o != t and o != b and t != b:
            conflicts.append(k)
        if v is not None:
            merged[k] = v
    if conflicts:
        print("cases changed differently on both sides:", conflicts)
        return 1
    rec = dict(
        ours["recorded"],
        commit=f"{ours['recorded']['commit']} and {theirs['recorded']['commit']}, merged (re-record from the merge)",
    )
    Path("tests/golden/golden.json").write_text(
        ns["dumps"]({"about": ours["about"], "recorded": rec, "cases": merged})
    )
    print(f"{len(merged)} cases written; re-record from the merge")
    return 0


if __name__ == "__main__":
    sys.exit(main())
