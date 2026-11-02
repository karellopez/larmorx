# SPDX-License-Identifier: Apache-2.0
"""stderrcat.py <trial> <kind>: stderr-differing cases grouped by the FSL / original lines."""

import collections
import json
import re
import sys
from pathlib import Path

rows = [json.loads(x) for x in Path(sys.argv[1] + "/results.jsonl").read_text().splitlines()]
c = collections.Counter()
ex = {}
for r in rows:
    if r["features"].get("kind") != sys.argv[2] or r["verdict"] == "agree":
        continue
    det = r["impls"]["original"].get("detail", "")
    m = re.search(r"stderr[^:]*: [^;]*?line \d+: FSL (.*?), original (.*?)(;|$)", det)
    if not m:
        continue
    key = (re.sub(r"[-\d.e]+", "#", m.group(1))[:90], re.sub(r"[-\d.e]+", "#", m.group(2))[:90])
    c[key] += 1
    ex.setdefault(key, []).append(r["n"])
for k, v in c.most_common(20):
    print(v, k, ex[k][:4])
