# SPDX-License-Identifier: Apache-2.0
"""The newest release of each GitHub Action the release workflow would use, with its date and the
commit SHA its tag points to (GitHub's public API, no login; about 2 requests per action)."""

import json
import sys
import urllib.request

ACTIONS = [
    "actions/checkout",
    "actions/setup-python",
    "actions/upload-artifact",
    "actions/download-artifact",
    "actions/attest-build-provenance",
    "PyO3/maturin-action",
    "pypa/gh-action-pypi-publish",
    "docker/setup-buildx-action",
    "docker/login-action",
    "docker/build-push-action",
    "docker/metadata-action",
    "Swatinem/rust-cache",
]


def get(url):
    req = urllib.request.Request(
        url, headers={"Accept": "application/vnd.github+json", "User-Agent": "release-inventory"}
    )
    with urllib.request.urlopen(req, timeout=20) as r:
        return json.load(r)


for repo in ACTIONS:
    try:
        rel = get(f"https://api.github.com/repos/{repo}/releases/latest")
        tag = rel["tag_name"]
        ref = get(f"https://api.github.com/repos/{repo}/commits/{tag}")
        print(f"{repo:34s} {tag:10s} {rel['published_at'][:10]}  {ref['sha']}")
    except Exception as e:
        print(f"{repo:34s} ERROR {e}", file=sys.stdout)
