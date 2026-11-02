# SPDX-License-Identifier: Apache-2.0
"""codestat.py <trial>: registrations and applyxfm4D runs whose reference has one form unset:
verdicts by (kind, -nosearch, which form unset), and whether the detail names a form code."""

import collections
import json
import re
import sys
from pathlib import Path

from larmorx_validation.difftest import core
from larmorx_validation.difftest.tools import fsl_flirt as dt

rows = [json.loads(l) for l in open(Path(sys.argv[1]) / "results.jsonl")]
c = collections.Counter()
for r in rows:
    k = r["features"].get("kind")
    if k not in ("register", "applyxfm4D", "apply") or (
        "-out" not in r["options"] and k == "register"
    ):
        continue
    rec = core.make_recipe(dt.ORIGINAL, r["seed"], r["n"])
    ref = rec["inputs"]["ref"]
    g = ref.get("geometry", {})
    q, s = g.get("qform_code"), g.get("sform_code")
    if ((q or 0) > 0 and (s or 0) > 0) or not (q or s):
        continue
    det = r["impls"]["original"].get("detail", "")
    named = bool(re.search(r"(headers differ|same header)[^;]*(sform_code|qform_code)", det))
    c[
        (
            k,
            "-nosearch" in r["options"],
            "-applyisoxfm" in r["options"],
            "s unset" if not s else "q unset",
            named,
        )
    ] += 1
for key, v in sorted(c.items()):
    print(v, key)
