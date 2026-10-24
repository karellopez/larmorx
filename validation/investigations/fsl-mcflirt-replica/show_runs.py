# SPDX-License-Identifier: Apache-2.0
"""Print the run.json steps and stdout/stderr of recorded mcflirt oracle runs.

Usage: python show_runs.py <case>...
"""

import json
import sys
from pathlib import Path


def _workspace() -> Path:
    """The workspace that holds oracles/ (LARMORX_WORKSPACE, else a parent of this file)."""
    import os

    if env := os.environ.get("LARMORX_WORKSPACE"):
        return Path(env)
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    raise SystemExit("the workspace with oracles/ was not found (LARMORX_WORKSPACE)")


def _repo() -> Path:
    return Path(__file__).resolve().parents[3]


ROOT = _workspace() / "oracles/fsl-6.0.7/mcflirt/runs"
limit = 1500
for name in sys.argv[1:]:
    d = ROOT / name
    print("===", name)
    if not d.is_dir():
        print("  (missing)")
        continue
    print("  files:", sorted(p.name for p in d.iterdir()))
    run = json.loads((d / "run.json").read_text())
    for s in run.get("steps", []):
        print("  argv:", " ".join(s["argv"][1:]))
        print("  exit:", s.get("exit"))
    for f in sorted(d.glob("std*")):
        t = f.read_text(errors="replace")
        print(f"--- {f.name} ({len(t)} chars)")
        print(t[:limit])
