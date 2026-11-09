# SPDX-License-Identifier: Apache-2.0
"""The originals' own step, case by case: runs one library script in both kept Python
environments with ``LARMORX_STEP_DUMP`` (``steplines.py``) and prints, for every comparison
whose originals' outputs changed between the shared releases (numpy 2.3.5, SciPy 1.15.3) and
the newest (numpy 2.5.4, SciPy 1.18.1), how many of its cases changed, how many exist in one
environment only, and the first changed case's outputs in both: what each change's reason in
``larmorx_validation.parity.sdcflows_scripts.REASONS`` is read from.

Usage (from the repository, its ``python/`` and ``validation/src`` on ``PYTHONPATH``)::

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 LARMORX_WORKSPACE=<workspace> \\
        python -P step_diff.py --work <folder> [--show 3] -- <script> [its arguments]

The two runs go into ``<folder>/{shared,newest}.jsonl`` (and their output beside them).
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent


def _workspace() -> Path:
    for parent in HERE.parents:
        if (parent / "oracles").is_dir():
            return parent
    raise SystemExit("the workspace (its oracles/ folder) was not found")


ENVS = {"shared": ".venv", "newest": "oracles/python-newest"}


def run(env: str, work: Path, script: list[str]) -> dict[str, list[dict]]:
    dump = work / f"{env}.jsonl"
    dump.unlink(missing_ok=True)
    py = _workspace() / ENVS[env] / "bin" / "python"
    e = {**os.environ, "LARMORX_STEP_DUMP": str(dump)}
    e.pop("LARMORX_STEP_LINES", None)
    folder = work / f"{env}-run"
    folder.mkdir(parents=True, exist_ok=True)
    args = [a.replace("{work}", str(folder)) for a in script[1:]]
    path = script[0] if os.path.isabs(script[0]) else str(HERE / script[0])
    with open(work / f"{env}.log", "w", encoding="utf-8") as log:
        subprocess.run([str(py), "-P", path, *args], env=e, stdout=log, stderr=subprocess.STDOUT,
                       cwd=folder, check=False)  # fmt: skip
    out: dict[str, list[dict]] = defaultdict(list)
    if dump.is_file():
        for line in dump.read_text(encoding="utf-8").splitlines():
            r = json.loads(line)
            out[r["name"]].append(r)
    return out


def main() -> int:
    argv = sys.argv[1:]
    if "--" not in argv:
        raise SystemExit(__doc__)
    k = argv.index("--")
    ap = argparse.ArgumentParser()
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--show", type=int, default=1)
    args = ap.parse_args(argv[:k])
    script = argv[k + 1 :]
    args.work.mkdir(parents=True, exist_ok=True)
    a, b = run("shared", args.work, script), run("newest", args.work, script)
    changed = 0
    for name in sorted(set(a) | set(b)):
        x, y = a.get(name, []), b.get(name, [])
        diffs = [
            (i, p, q)
            for i, (p, q) in enumerate(zip(x, y, strict=False))
            if p["digest"] != q["digest"]
        ]
        if not diffs and len(x) == len(y):
            continue
        changed += 1
        print(f"CHANGED {name}: {len(diffs)} of {min(len(x), len(y))} cases; "
              f"{len(x)} cases in shared, {len(y)} in newest")  # fmt: skip
        for i, p, q in diffs[: args.show]:
            print(f"    case {i + 1}: shared {p['shown']}")
            print(f"    case {i + 1}: newest {q['shown']}")
    print(f"{changed} comparisons changed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
