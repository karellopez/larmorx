#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""What the new machine must match: run on the OLD machine before the move.

Writes <workspace>/MOVE-MANIFEST.json (outside the repository: it names this machine's paths):
the machine's facts that decide the originals' results (Ubuntu release, glibc and the exact
libm and libstdc++ files), the tools' versions, the SHA-256 of every oracle program and preload
the records were made with, and each copied tree's file count and size. `check.py` compares
the new machine with it after `pull.sh`.

    python3 larmorx/scripts/orchestration/move/manifest.py [--workspace DIR]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import time
from pathlib import Path

HOME = Path.home()
#: Program directories whose files are hashed (non-recursive; symlinks resolved).
BINARY_DIRS = [
    "oracles/ants-2.6.5/bin",
    "oracles/afni-25.2.09/bin",
    "oracles/core-math-preload",
    "oracles/fsl-6.0.7/mcflirt/bin",
    "oracles/images",
]
HOME_BINARY_DIRS = ["fsl/bin"]
SYSTEM_LIBS = ["libm.so.6", "libc.so.6", "libstdc++.so.6", "libgomp.so.1"]
#: The trees pull.sh copies (relative to the workspace or to $HOME), counted without build caches.
TREES = [
    "oracles",
    "archive",
    "larmorx",
    "larmorx_documentation",
    "larmorx-testdata",
    "tags",
    ".venv",
]
HOME_TREES = ["fsl", ".cache/larmorx-testdata"]
SKIP_DIRS = {"target", "__pycache__"}


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for block in iter(lambda: fh.read(1 << 22), b""):
            h.update(block)
    return h.hexdigest()


def run(cmd: list[str]) -> str:
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=60).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


def system_lib(name: str) -> Path | None:
    for d in ("/lib/x86_64-linux-gnu", "/usr/lib/x86_64-linux-gnu", "/lib/aarch64-linux-gnu"):
        p = Path(d) / name
        if p.exists():
            return p.resolve()
    return None


def tree_stats(root: Path) -> dict:
    files = size = 0
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for f in filenames:
            p = os.path.join(dirpath, f)
            try:
                st = os.lstat(p)
            except OSError:
                continue
            files += 1
            size += st.st_size
    return {"files": files, "bytes": size}


def binaries(root: Path) -> dict[str, str]:
    out = {}
    if not root.is_dir():
        return out
    for p in sorted(root.iterdir()):
        target = p.resolve()
        if target.is_file() and target.stat().st_size < 8 << 30:
            out[p.name] = sha256(target)
    return out


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--workspace", type=Path, default=Path(__file__).resolve().parents[4])
    ap.add_argument("--out", type=Path)
    a = ap.parse_args()
    ws = a.workspace.resolve()
    t0 = time.time()
    os_release = dict(
        line.split("=", 1)
        for line in Path("/etc/os-release").read_text().splitlines()
        if "=" in line
    )
    m = {
        "made": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "workspace": str(ws),
        "home": str(HOME),
        "user": os.environ.get("USER", ""),
        "machine": {
            "os": os_release.get("PRETTY_NAME", "").strip('"'),
            "kernel": platform.release(),
            "arch": platform.machine(),
            "glibc": run(["ldd", "--version"]).splitlines()[0] if run(["ldd", "--version"]) else "",
            "cpus": os.cpu_count(),
            "mem_gib": round(os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES") / 2**30, 1),
        },
        "system_libs": {},
        "tools": {
            "apptainer": run(["apptainer", "--version"]),
            "python3.12": run(["/usr/bin/python3.12", "--version"]),
            "rustc": run([str(HOME / ".cargo/bin/rustc"), "--version"]),
            "git": run(["git", "--version"]),
            "venv_python": run([str(ws / ".venv/bin/python"), "--version"]),
        },
        "binaries": {},
        "trees": {},
    }
    for name in SYSTEM_LIBS:
        p = system_lib(name)
        if p:
            m["system_libs"][name] = {"file": str(p), "sha256": sha256(p)}
    for d in BINARY_DIRS:
        m["binaries"][d] = binaries(ws / d)
    for d in HOME_BINARY_DIRS:
        m["binaries"]["~/" + d] = binaries(HOME / d)
    for d in TREES:
        if (ws / d).exists():
            m["trees"][d] = tree_stats(ws / d)
            print(f"  {d}: {m['trees'][d]}", flush=True)
    for d in HOME_TREES:
        if (HOME / d).exists():
            m["trees"]["~/" + d] = tree_stats(HOME / d)
            print(f"  ~/{d}: {m['trees']['~/' + d]}", flush=True)
    m["seconds"] = round(time.time() - t0)
    out = a.out or ws / "MOVE-MANIFEST.json"
    out.write_text(json.dumps(m, indent=1))
    if shutil.which("df"):
        print(run(["df", "-h", str(ws)]).splitlines()[-1])
    total = sum(v["bytes"] for v in m["trees"].values())
    print(
        f"wrote {out}: {len(m['trees'])} trees, {total / 2**30:.0f} GiB without build caches, "
        f"{sum(len(v) for v in m['binaries'].values())} programs hashed, {m['seconds']} s"
    )


if __name__ == "__main__":
    main()
