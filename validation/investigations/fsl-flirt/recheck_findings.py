# SPDX-License-Identifier: Apache-2.0
"""Re-runs the trial's findings (shrunk and drawn recipes) through the replica suite's own runner
with the binary in LARMORX_NC_BIN, and says which still differ.

Usage: python -P recheck_findings.py <findings dir> <kind: apply|register|convert|all> [jobs]
"""

import json
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

from larmorx_validation.difftest.tools import get
from larmorx_validation.parity.harness import CheckList, Outcome


def kind_of(recipe):
    words = [a for g in recipe["groups"] for a in g["args"]]
    if recipe["extra"]["tool"] == "convert_xfm":
        return "convert"
    return "apply" if any(w.startswith("-apply") for w in words) else "register"


def run(path_and_which):
    path, which = path_and_which
    d = json.loads(Path(path).read_text())
    recipe = d["shrunk"] if which == "shrunk" and d.get("shrunk") else d["recipe"]
    tool = get(d.get("tool", "fsl-flirt"))
    case = tool.case(recipe, "replica")
    checks = CheckList()
    try:
        tool.run("replica")(case, checks)
    except Outcome as o:
        if all(c.passed for c in checks.checks):
            return path, which, f"outcome {o.status}"
        return (
            path,
            which,
            f"FAIL ({o.status}) "
            + "; ".join(f"{c.name}: {c.detail}"[:160] for c in checks.checks if not c.passed),
        )
    except Exception as e:
        return path, which, f"error {type(e).__name__}: {e}"[:200]
    failed = [c for c in checks.checks if not c.passed]
    if not failed:
        return path, which, "pass"
    return path, which, "FAIL " + "; ".join(f"{c.name}: {c.detail}"[:160] for c in failed)


def main():
    root = Path(sys.argv[1])
    kind = sys.argv[2]
    jobs = int(sys.argv[3]) if len(sys.argv) > 3 else 8
    todo = []
    for f in sorted(root.glob("*.json")):
        d = json.loads(f.read_text())
        if kind != "all" and kind_of(d["recipe"]) != kind:
            continue
        todo += [(str(f), "shrunk"), (str(f), "drawn")]
    with ProcessPoolExecutor(jobs) as ex:
        results = list(ex.map(run, todo))
    bad = 0
    for path, which, res in results:
        ok = res == "pass" or res.startswith("outcome")
        bad += not ok
        print(Path(path).stem, which, res[:300])
    print(f"{len(results)} runs, {bad} still differ")


if __name__ == "__main__":
    main()
