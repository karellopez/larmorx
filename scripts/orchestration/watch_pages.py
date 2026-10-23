# SPDX-License-Identifier: Apache-2.0
"""Wait for the documentation site's GitHub Pages deployment of a commit. Usage: watch_pages.py <sha>"""

import json
import sys
import time
import urllib.request

API = "https://api.github.com/repos/karellopez/larmorx_documentation/actions/runs?per_page=5"
for _ in range(60):
    with urllib.request.urlopen(
        urllib.request.Request(API, headers={"Accept": "application/vnd.github+json"}), timeout=60
    ) as r:
        runs = json.load(r)["workflow_runs"]
    mine = [r for r in runs if r["head_sha"].startswith(sys.argv[1])]
    if mine and mine[0]["status"] == "completed":
        print("deploy", mine[0]["head_sha"][:7], mine[0]["conclusion"])
        sys.exit(0 if mine[0]["conclusion"] == "success" else 1)
    time.sleep(20)
print("no completed deployment")
sys.exit(1)
