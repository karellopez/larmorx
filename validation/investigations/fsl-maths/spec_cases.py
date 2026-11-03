# SPDX-License-Identifier: Apache-2.0
"""Checks that every case name a spec cites in brackets exists in the parity suite.

A name ending in ``*`` or ``...`` is a prefix; others must match a case id exactly.
"""

import pathlib
import re
import sys

from larmorx_validation.parity import fsl_maths_replica, fsl_stats_replica

root = pathlib.Path(sys.argv[1])
for spec, mod in (
    ("specs/fslmaths.md", fsl_maths_replica),
    ("specs/fslstats.md", fsl_stats_replica),
):
    ids = {c.id for c in mod.cases("standard")} | {c.id for c in mod.cases("smoke")}
    text = (root / spec).read_text()
    cited = set()
    for group in re.findall(r"\[([a-z0-9][^\]\n]*)\]", text):
        for name in group.split(","):
            name = name.strip()
            if "/" in name:
                cited.add(name)
    missing = []
    for name in sorted(cited):
        if name.endswith("...") or name.endswith("*"):
            prefix = name.rstrip(".*")
            if not any(i.startswith(prefix) for i in ids):
                missing.append(name)
        elif name not in ids:
            close = sorted(i for i in ids if i.startswith(name.split("/")[0] + "/"))
            missing.append(f"{name}   (group has: {', '.join(close[:40])})")
    print(spec, len(cited), "cited,", len(missing), "missing")
    for m in missing:
        print("  ", m)
