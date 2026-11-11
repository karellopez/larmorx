# SPDX-License-Identifier: Apache-2.0
"""Summarize the pip dry-run reports: per platform, each package's version and wheel tag."""

import json
import pathlib
import sys

here = pathlib.Path(sys.argv[1])
rows = {}
for rep in sorted(here.glob("r-*.json")):
    d = json.loads(rep.read_text())
    key = rep.stem[2:]
    pk = {}
    for it in d["install"]:
        name = it["metadata"]["name"].lower()
        url = it["download_info"]["url"]
        fname = url.rsplit("/", 1)[-1]
        tag = "-".join(fname[:-4].split("-")[-3:]) if fname.endswith(".whl") else fname
        pk[name] = (it["metadata"]["version"], tag)
    rows[key] = pk
names = sorted({n for pk in rows.values() for n in pk})
print("packages:", len(names))
for key, pk in rows.items():
    print(f"== {key}: {len(pk)} packages")
    for n in names:
        if n in pk and not pk[n][1].endswith("none-any"):
            print(f"   {n} {pk[n][0]} {pk[n][1]}")
allpure = [
    n for n in names if all(n not in pk or pk[n][1].endswith("none-any") for pk in rows.values())
]
print("pure:", " ".join(f"{n}" for n in allpure))
