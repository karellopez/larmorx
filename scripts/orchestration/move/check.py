#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Is the new machine ready? Run on the NEW machine after pull.sh (docs/machine-move.md, step 5).

Compares it with <workspace>/MOVE-MANIFEST.json (made on the old machine by manifest.py): the
system libraries the originals' results depend on, every oracle program's SHA-256, each copied
tree's file count and size, and the tools the work needs. Prints one line per check: OK, WARN
(works, but the proof run must show the results unchanged) or FAIL (fix before going on).

    python3 larmorx/scripts/orchestration/move/check.py [--workspace DIR]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import manifest as mf

results: list[tuple[str, str, str]] = []


def say(status: str, what: str, detail: str = "") -> None:
    results.append((status, what, detail))
    print(f"{status:4}  {what}" + (f": {detail}" if detail else ""), flush=True)


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--workspace", type=Path, help="default: the workspace this script is in")
    ap.add_argument("--quick", action="store_true", help="skip counting the trees (minutes)")
    a = ap.parse_args()
    ws = (a.workspace or Path(__file__).resolve().parents[4]).resolve()
    old = json.loads((ws / "MOVE-MANIFEST.json").read_text())

    say(
        "OK" if str(ws) == old["workspace"] else "FAIL",
        "workspace path",
        f"{ws} (old {old['workspace']})",
    )
    say("OK" if str(Path.home()) == old["home"] else "FAIL", "home", str(Path.home()))
    new = mf.run(["ldd", "--version"]).splitlines()
    glibc = new[0] if new else ""
    say(
        "OK" if glibc == old["machine"]["glibc"] else "WARN",
        "glibc",
        f"{glibc} (old {old['machine']['glibc']})",
    )
    for name, info in old["system_libs"].items():
        p = mf.system_lib(name)
        same = p is not None and mf.sha256(p) == info["sha256"]
        say(
            "OK" if same else "WARN",
            f"{name} identical to the old machine's",
            "" if same else "differs: the originals' results may change; the proof run decides",
        )
    osr = dict(
        line.split("=", 1)
        for line in Path("/etc/os-release").read_text().splitlines()
        if "=" in line
    )
    name = osr.get("PRETTY_NAME", "").strip('"')
    say(
        "OK" if name.split(" LTS")[0][:12] == old["machine"]["os"][:12] else "WARN",
        "OS",
        f"{name} (old {old['machine']['os']})",
    )
    say(
        "OK",
        "CPUs and memory",
        f"{os.cpu_count()} threads, "
        f"{os.sysconf('SC_PAGE_SIZE') * os.sysconf('SC_PHYS_PAGES') / 2**30:.0f} GiB "
        f"(old {old['machine']['cpus']}, {old['machine']['mem_gib']} GiB)",
    )

    for d, files in old["binaries"].items():
        root = Path.home() / d[2:] if d.startswith("~/") else ws / d
        now = mf.binaries(root)
        missing = sorted(set(files) - set(now))
        changed = sorted(k for k in files if k in now and now[k] != files[k])
        ok = not missing and not changed
        say(
            "OK" if ok else "FAIL",
            f"programs in {d} ({len(files)})",
            "" if ok else f"missing {missing[:5]}, changed {changed[:5]}",
        )

    tools = {
        "apptainer": (
            ["apptainer", "--version"],
            "sudo apt install apptainer (or the .deb of the old version)",
        ),
        "python3.12": (
            ["/usr/bin/python3.12", "--version"],
            "deadsnakes: sudo apt install python3.12 python3.12-venv python3.12-dev",
        ),
        "rustc": (
            [str(Path.home() / ".cargo/bin/rustc"), "--version"],
            "copied with ~/.rustup and ~/.cargo",
        ),
        "git": (["git", "--version"], "sudo apt install git"),
        "venv_python": (
            [str(ws / ".venv/bin/python"), "--version"],
            "python3.12 must be at the old path",
        ),
    }
    for key, (cmd, fix) in tools.items():
        got = mf.run(cmd)
        want = old["tools"].get(key, "")
        status = "OK" if got and got == want else ("WARN" if got else "FAIL")
        say(status, key, f"{got or 'missing'} (old {want})" + ("" if got else f"; {fix}"))
    for exe in ("rsync", "tmux", "systemd-run"):
        say("OK" if shutil.which(exe) else "WARN", exe, "" if shutil.which(exe) else "missing")
    # The repositories: copied (not cloned: they hold commits not yet on GitHub), so each must be
    # at the old laptop's latest commit, with the same working-tree state and the same commits
    # not yet pushed (git's record of GitHub's branch, origin/main, comes with the copy).
    for repo in ("larmorx", "larmorx_documentation", "larmorx-testdata"):
        git = ["git", "-C", str(ws / repo)]
        queries = (
            [*git, "log", "-1", "--format=%H"],
            [*git, "status", "--porcelain"],
            [*git, "rev-parse", "origin/main"],
            [*git, "rev-list", "--count", "origin/main..HEAD"],
        )
        remote = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=20", "old-laptop"]
        here = tuple(mf.run(q) for q in queries)
        there = tuple(mf.run(remote + q) for q in queries)
        if not there[0]:
            say("WARN", f"repository {repo}", "the old laptop is not reachable as old-laptop")
            continue
        say(
            "OK" if here == there else "FAIL",
            f"repository {repo} at the old laptop's latest commit",
            f"{here[0][:7]} (old {there[0][:7]}); {here[3] or '?'} commits not yet on GitHub "
            f"(old {there[3] or '?'})"
            + ("" if here[1] == there[1] else "; working trees differ: run pull.sh --verify"),
        )
    # No AI attribution (CLAUDE.md rule 9): the commit-msg guard in each repository, and Claude
    # Code's own attribution turned off.
    guard = ws / "larmorx/scripts/orchestration/git-hooks/commit-msg"
    for repo in ("larmorx", "larmorx_documentation", "larmorx-testdata"):
        hook = ws / repo / ".git/hooks/commit-msg"
        same = hook.is_file() and guard.is_file() and hook.read_bytes() == guard.read_bytes()
        say(
            "OK" if same and os.access(hook, os.X_OK) else "FAIL",
            f"no-AI-attribution guard in {repo}",
            "" if same else "run larmorx/scripts/orchestration/git-hooks/install.sh",
        )
    try:
        settings = json.loads((Path.home() / ".claude/settings.json").read_text())
    except (OSError, ValueError):
        settings = {}
    attribution = settings.get("attribution")
    off = attribution is False or (
        isinstance(attribution, dict)
        and attribution.get("commit") == ""
        and attribution.get("pr") == ""
    )
    say(
        "OK" if off else "FAIL",
        "Claude Code's commit attribution off",
        ""
        if off
        else 'add "attribution": {"commit": "", "pr": "", "sessionUrl": false}'
        " to ~/.claude/settings.json",
    )
    # The usage tool (tools/claude-usage/, docs/machine-move.md step 8) reads the plan's limits
    # with Claude Code's own program: on PATH, the native install, or the VS Code extension's.
    usage = ws / "tools/claude-usage/usage.py"
    say("OK" if usage.is_file() else "FAIL", "usage tool", str(usage))
    claude = shutil.which("claude") or next(
        (
            str(p)
            for p in sorted(
                (Path.home() / ".vscode/extensions").glob(
                    "anthropic.claude-code-*/resources/native-binary/claude"
                )
            )
            if p.is_file()
        ),
        None,
    )
    say(
        "OK" if claude else "WARN",
        "Claude Code program for the usage tool",
        claude or "missing: install VS Code's Claude Code extension (step 1)",
    )
    lic = Path("/usr/local/freesurfer/license.txt")
    say(
        "OK" if lic.is_file() else "FAIL",
        "FreeSurfer licence for the fMRIPrep oracle runs",
        str(lic) if lic.is_file() else "missing: copy it by hand (docs/machine-move.md, step 4)",
    )

    if not a.quick:
        for d, st in old["trees"].items():
            root = Path.home() / d[2:] if d.startswith("~/") else ws / d
            if not root.exists():
                say("FAIL", f"tree {d}", "missing")
                continue
            now = mf.tree_stats(root, mf.left_on_old(ws))
            ok = now["files"] >= st["files"] and now["bytes"] >= st["bytes"] * 0.999
            say(
                "OK" if ok else "FAIL",
                f"tree {d}",
                f"{now['files']} files, {now['bytes'] / 2**30:.1f} GiB (old {st['files']}, {st['bytes'] / 2**30:.1f} GiB)",
            )

    for d in mf.LEFT_ON_OLD:
        say("OK", f"left on the old laptop: {d}", "fetch with rsync -a old-laptop:<path>/ <path>/")

    fails = sum(1 for s, _, _ in results if s == "FAIL")
    warns = sum(1 for s, _, _ in results if s == "WARN")
    print(f"\n{len(results)} checks: {fails} FAIL, {warns} WARN")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
