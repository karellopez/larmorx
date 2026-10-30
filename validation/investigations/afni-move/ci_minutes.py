# SPDX-License-Identifier: Apache-2.0
"""The CI minutes of the `gpl` jobs and of their `crates-gpl` steps (rustfmt, clippy, cargo test),
from the three newest successful runs on main (GitHub's public API, no login).

Usage: python -P ci_minutes.py
"""

import datetime as dt
import json
import urllib.request

API = "https://api.github.com/repos/karellopez/larmorx/actions"


def get(url):
    with urllib.request.urlopen(url, timeout=30) as r:
        return json.load(r)


def t(s):
    return dt.datetime.fromisoformat(s.replace("Z", "+00:00"))


runs = get(f"{API}/runs?branch=main&status=success&per_page=5")["workflow_runs"]
for run in runs[:3]:
    jobs = get(f"{API}/runs/{run['id']}/jobs?per_page=50")["jobs"]
    total_gpl = total_steps = 0.0
    print(run["id"], run["head_sha"][:7], run["created_at"])
    for j in jobs:
        if not j["name"].startswith("gpl"):
            continue
        job_min = (t(j["completed_at"]) - t(j["started_at"])).total_seconds() / 60
        gpl_steps = 0.0
        for st in j["steps"]:
            if st["name"] in ("rustfmt", "clippy", "cargo test") and st.get("started_at"):
                gpl_steps += (t(st["completed_at"]) - t(st["started_at"])).total_seconds() / 60
        total_gpl += job_min
        total_steps += gpl_steps
        print(f"  {j['name']}: job {job_min:.1f} min, crates-gpl steps {gpl_steps:.1f} min")
    print(f"  total gpl jobs {total_gpl:.1f} min, of which crates-gpl steps {total_steps:.1f} min")
