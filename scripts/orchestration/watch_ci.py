# SPDX-License-Identifier: Apache-2.0
# Waits for the CI run of a pushed commit and prints each job's result, and a failed job's
# annotations (GitHub serves them without a login). Usage: watch_ci.py <sha> [<branch or tag>]
# (default main; the release workflow's dry run: release/dry-run, a release: its tag v<version>).
import json
import pathlib
import subprocess
import sys
import time
import urllib.parse
import urllib.request

# the full SHA of a commit this repository holds: a mistyped prefix (2026-11-03: `3a15db3a` for
# 3a15db38) matched no run, and the watcher waited for hours without a word
try:
    sha = subprocess.run(
        ["git", "rev-parse", "--verify", f"{sys.argv[1]}^{{commit}}"],
        cwd=pathlib.Path(__file__).resolve().parent,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
except subprocess.CalledProcessError:
    print(f"not a commit of this repository: {sys.argv[1]}")
    sys.exit(2)
API = "https://api.github.com/repos/karellopez/larmorx/actions"
branch = urllib.parse.quote(sys.argv[2] if len(sys.argv) > 2 else "main", safe="")


def get(url):
    with urllib.request.urlopen(
        urllib.request.Request(url, headers={"Accept": "application/vnd.github+json"}), timeout=30
    ) as r:
        return json.load(r)


run = None
for _ in range(240):
    try:
        runs = get(f"{API}/runs?branch={branch}&event=push&per_page=20")["workflow_runs"]
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
    if j["conclusion"] == "failure":
        checks = API.replace("/actions", f"/check-runs/{j['id']}/annotations")
        for a in get(checks)[:20]:
            print(f"    {a['annotation_level']}: {a['title'] or ''} {a['message'][:300]}")
