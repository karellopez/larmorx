# SPDX-License-Identifier: Apache-2.0
# Waits for the CI run of a pushed commit and prints each job's result. Usage: watch_ci.py <sha>
import json
import sys
import time
import urllib.request

sha = sys.argv[1]
API = "https://api.github.com/repos/karellopez/larmorx/actions"


def get(url):
    with urllib.request.urlopen(
        urllib.request.Request(url, headers={"Accept": "application/vnd.github+json"}), timeout=30
    ) as r:
        return json.load(r)


run = None
for _ in range(240):
    try:
        runs = get(f"{API}/runs?branch=main&event=push&per_page=20")["workflow_runs"]
        mine = [r for r in runs if r["head_sha"].startswith(sha)]
        if mine:
            run = mine[0]
            if run["status"] == "completed":
                break
    except Exception as e:
        print("retry:", e, flush=True)
    time.sleep(60)
if run is None:
    print("no run found")
    sys.exit(1)
print("run", run["id"], run["head_sha"][:7])
print("run:", run["status"], run["conclusion"])
for j in get(f"{API}/runs/{run['id']}/jobs?per_page=50")["jobs"]:
    print(j["name"], j["conclusion"], j["id"])
