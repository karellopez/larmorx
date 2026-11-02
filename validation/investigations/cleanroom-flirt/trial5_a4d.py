# SPDX-License-Identifier: Apache-2.0
import collections
import json
import re
import sys

from larmorx_validation.difftest import core
from larmorx_validation.difftest.tools import fsl_flirt as dt

rows = [json.loads(l) for l in open(sys.argv[1] + "/results.jsonl")]
c = collections.Counter()
for r in rows:
    if r["features"].get("kind") != "applyxfm4D":
        continue
    rec = core.make_recipe(dt.ORIGINAL, r["seed"], r["n"])
    interp = next((o.split()[1] for o in r["options"] if o.startswith("-interp")), "sinc")
    interp = {"nn": "nearestneighbour"}.get(interp, interp)
    det = r["impls"]["original"].get("detail", "")
    vals = bool(re.search(r"values differ|correlation", det))
    c[
        (
            interp,
            "-3D" in r["options"],
            rec["inputs"]["in"]["dtype"],
            r["verdict"] if r["verdict"] == "agree" else ("values" if vals else "other"),
        )
    ] += 1
for k, v in sorted(c.items()):
    print(v, k)
