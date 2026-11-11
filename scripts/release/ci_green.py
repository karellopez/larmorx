# SPDX-License-Identifier: Apache-2.0
"""Whether a commit may be released: it is on main, and CI passed for it.

The release workflow's first job asks this of a tag's commit: the commit must be on
`origin/main`, and CI (`.github/workflows/ci.yml`) must have passed for it on a push to main; or,
since CI does not run for a push that changes only documentation (its `paths-ignore`: `**.md`,
`docs/**`), for the nearest ancestor CI passed, with nothing but documentation changed since. With
`--report` (dry runs, whose commit is usually not on main yet) it prints the answer as a notice
and exits 0.

CI's successful runs come from GitHub's API (`GITHUB_TOKEN` and `GITHUB_REPOSITORY` from the
environment; the job needs `actions: read`), the history from the checkout (fetched whole).

Usage:
    python -P scripts/release/ci_green.py --sha <commit> [--report]
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import urllib.request

#: ci.yml's `paths-ignore` (a test keeps the two equal): a push that changes only such paths runs
#: no CI.
IGNORED = ("**.md", "docs/**")


def ignored(path: str) -> bool:
    """Whether a changed path is one CI ignores (GitHub's globs `**.md` and `docs/**`)."""
    return path.endswith(".md") or path.startswith("docs/")


def git(*args: str) -> str:
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout


def green_runs(repo: str, token: str) -> dict[str, int]:
    """The commits CI passed for on a push to main (head SHA -> run id), the newest 100."""
    url = (
        f"https://api.github.com/repos/{repo}/actions/workflows/ci.yml/runs"
        "?branch=main&event=push&status=success&per_page=100"
    )
    headers = {"Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=30) as r:
        runs = json.load(r)["workflow_runs"]
    return {run["head_sha"]: run["id"] for run in runs}


def verdict(sha: str, green: dict[str, int]) -> tuple[bool, str]:
    """(may be released, why), from the checkout's history and CI's green runs."""
    try:
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", sha, "origin/main"],
            capture_output=True,
            check=True,
        )
    except subprocess.CalledProcessError:
        return False, f"{sha[:12]} is not on origin/main: merge it first, then tag main's commit"
    for ancestor in git("rev-list", sha).split():
        if ancestor in green:
            changed = git("diff", "--name-only", ancestor, sha).split()
            others = [p for p in changed if not ignored(p)]
            if others:
                shown = ", ".join(others[:8]) + (" ..." if len(others) > 8 else "")
                return False, (
                    f"CI has not passed for {sha[:12]}: its last green ancestor "
                    f"{ancestor[:12]} (run {green[ancestor]}) differs in code ({shown})"
                )
            since = f"; since then documentation only ({len(changed)} files)" * bool(changed)
            return True, f"CI passed for {ancestor[:12]} (run {green[ancestor]}){since}"
    return False, f"no ancestor of {sha[:12]} among CI's last {len(green)} green runs on main"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--sha", required=True, help="the commit to release")
    parser.add_argument("--report", action="store_true", help="print the answer; exit 0")
    args = parser.parse_args(argv)
    repo = os.environ.get("GITHUB_REPOSITORY", "karellopez/larmorx")
    sha = git("rev-parse", "--verify", f"{args.sha}^{{commit}}").strip()
    ok, why = verdict(sha, green_runs(repo, os.environ.get("GITHUB_TOKEN", "")))
    kind = "notice" if ok or args.report else "error"
    print(f"CI-GREEN {'ok' if ok else 'NO'}: {why}")
    if os.environ.get("GITHUB_ACTIONS") == "true":
        print(f"::{kind} title=tagged commit on main, CI green::{why}")
    return 0 if ok or args.report else 1


if __name__ == "__main__":
    sys.exit(main())
