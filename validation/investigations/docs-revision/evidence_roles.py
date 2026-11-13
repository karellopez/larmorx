# SPDX-License-Identifier: Apache-2.0
"""What the site's evidence compares, as the writing guide's sections 4 and 5 count it.

Reads `static/evidence/*.json` of a checkout of the documentation site and prints:

- every comparison label (`role_label`) with how many differences carry it and two examples;
- every panel role (`role`: input, original, larmorx, fmriprep, reference) with its count;
- the chart series' marks (`mark`) per page, which colour a series takes today.

    python -P validation/investigations/docs-revision/evidence_roles.py <site checkout>
"""

from __future__ import annotations

import collections
import json
import pathlib
import sys

ROLES = {"input", "original", "larmorx", "fmriprep", "reference"}


def main(site: pathlib.Path) -> None:
    labels: collections.Counter[str] = collections.Counter()
    examples: dict[str, list[tuple[str, str]]] = collections.defaultdict(list)
    roles: collections.Counter[str] = collections.Counter()
    marks: collections.Counter[tuple[str, str]] = collections.Counter()

    def walk(node: object, page: str) -> None:
        if isinstance(node, dict):
            label = node.get("role_label")
            if isinstance(label, str):
                labels[label] += 1
                if len(examples[label]) < 2:
                    examples[label].append((page, str(node.get("label"))))
            role = node.get("role")
            if isinstance(role, str) and role in ROLES:
                roles[role] += 1
            mark = node.get("mark")
            if isinstance(mark, str):
                marks[(page, mark)] += 1
            for value in node.values():
                walk(value, page)
        elif isinstance(node, list):
            for value in node:
                walk(value, page)

    for path in sorted((site / "static" / "evidence").glob("*.json")):
        walk(json.loads(path.read_text(encoding="utf-8")), path.stem)

    print(f"comparison labels: {len(labels)} kinds, {sum(labels.values())} in all")
    for label, count in labels.most_common():
        print(f"{count:5d}  {label}  e.g. {examples[label]}")
    print("panel roles:", dict(roles))
    print("chart marks per page:")
    for (page, mark), count in sorted(marks.items()):
        print(f"{count:5d}  {page}  {mark}")


if __name__ == "__main__":
    main(pathlib.Path(sys.argv[1]))
