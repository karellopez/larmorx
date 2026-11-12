# SPDX-License-Identifier: Apache-2.0
"""A random-testing finding of ``tedana-t2smap-original`` replayed by hand: its inputs rebuilt
from the recipe (the shrunk one by default), then tedana 26.0.3 (through the oracle-output cache),
the original in ``reproduce`` and by default, and the replica's default (``LARMORX_GPL_BIN``), each
one's exit status and standard error printed in full (``--keep DIR`` keeps the folders)::

    python -P replay.py <finding.json> [--drawn] [--keep DIR] [--only tedana,reproduce,default,replica]
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import sys
import tempfile
from pathlib import Path


def _contract(err: str) -> str:
    """Standard error as the contract keeps it (§12.3, §12.4): a traceback's stack and Python's
    warning location and source lines left out (the original's own and the programs' alike), so
    no line of a program's source is ever printed."""
    out, in_stack = [], False
    for line in err.splitlines():
        if line == "Traceback (most recent call last):":
            out.append(line)
            out.append("  <stack>")
            in_stack = True
            continue
        if in_stack:
            if line.startswith(" ") or line.startswith("\t"):
                continue
            in_stack = False
        if re.match(r"^\S+:\d*\S*: \w+Warning: ", line):
            out.append(re.sub(r"^\S+: ", "<location>: ", line))
            continue
        if out and out[-1].startswith("<location>: ") and line.startswith("  "):
            continue  # the warning's source line
        out.append(line)
    return "\n".join(out) + "\n"


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("finding", type=Path)
    p.add_argument("--drawn", action="store_true", help="the drawn recipe, not the shrunk one")
    p.add_argument("--keep", type=Path, default=None)
    p.add_argument("--only", default="tedana,reproduce,default,replica")
    a = p.parse_args(argv)
    from larmorx_validation.difftest.core import arguments
    from larmorx_validation.difftest.tools import tedana_t2smap_original as t
    from larmorx_validation.parity import tedana_t2smap_original as so

    d = json.loads(a.finding.read_text(encoding="utf-8"))
    recipe = d["recipe"] if a.drawn or not d.get("shrunk") else d["shrunk"]
    work = Path(a.keep) if a.keep else Path(tempfile.mkdtemp(prefix="lx-t2smap-replay-"))
    if work.exists() and a.keep:
        shutil.rmtree(work)
    inputs = work / "inputs"
    t.write_inputs(recipe["extra"]["series"], inputs)
    args = [w.replace("<I>", str(inputs)) for w in arguments(recipe)] + ["--out-dir", "out"]
    blackbox, _, _, _ = so.tools()
    env = blackbox.environment()
    print("args:", " ".join(w.replace(str(inputs), "<I>") for w in args))
    runs = {
        "tedana": lambda: so.run_tedana(args, work / "tedana", env, "a replayed random case"),
        "reproduce": lambda: so.run_original(
            args, work / "reproduce", {**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"}
        ),
        "default": lambda: so.run_original(
            args, work / "default", {**env, "LARMORX_UPSTREAM_DEFECTS": "fix"}
        ),
        "replica": lambda: so.run_replica(args, work / "replica", env),
    }
    for name in a.only.split(","):
        if name == "replica" and so.replica_program() is None:
            continue
        code, _, err = runs[name]()
        print(f"===== {name}: exit {code}")
        print(_contract(err).replace(str(inputs), "<I>").replace(str(work), "<W>"), end="")
    if not a.keep:
        shutil.rmtree(work, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
