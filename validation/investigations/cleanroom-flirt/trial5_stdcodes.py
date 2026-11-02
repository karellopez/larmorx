# SPDX-License-Identifier: Apache-2.0
import collections
import json
import sys
from pathlib import Path

from larmorx_validation.difftest import core
from larmorx_validation.difftest.tools import fsl_flirt as dt

rows = [json.loads(x) for x in Path(sys.argv[1] + "/results.jsonl").read_text().splitlines()]
c = collections.Counter()
for r in rows:
    if r["features"].get("kind") != "coords":
        continue
    rec = core.make_recipe(dt.ORIGINAL, r["seed"], r["n"])
    prog = rec["extra"]["tool"]
    b = rec["inputs"].get("b")
    a = rec["inputs"]["a"]
    gb = (b or {}).get("geometry", {})
    ga = a.get("geometry", {})
    warn = "standard coordinates not set" in r["impls"]["original"].get("detail", "")
    key = (
        prog,
        "std" if b else "nostd",
        (gb.get("qform_code"), gb.get("sform_code")) if b else None,
        (ga.get("qform_code"), ga.get("sform_code")),
        r["verdict"],
        warn,
    )
    c[key] += 1
for k, v in sorted(c.items(), key=str):
    print(v, k)
