# SPDX-License-Identifier: Apache-2.0
"""Re-runs a random-testing run's findings (shrunk and drawn recipes) through the replica suite's
own runner with the binary in LARMORX_NC_BIN, and says which still differ.

It runs only inside a memory box (``scripts/orchestration/boxed.sh <GiB> python -P ...``), as
``difftest`` does: on 2026-11-02 an unboxed re-check with 24 workers, the memory findings
included (FSL alone needs 9 GiB on some), filled the machine's memory in two minutes and
systemd-oomd stopped the editor with every agent in it. So each worker, with every program it
starts, is held in a leaf of the box with the trial's own per-case memory limit (``--memory-gib``,
the run's ``run.json`` case limit, else 3 GiB) and no swap, and the original programs get the
trial's address-space limit (twice that, at least 64 GiB: ``address_space.for_memory``); the
findings whose verdict was a memory limit are skipped unless ``--memory`` is given, which runs
them one at a time; at most 8 workers.

Usage: scripts/orchestration/boxed.sh <GiB> python -P recheck_findings.py <findings dir>
<kind: apply|register|convert|all> [jobs (8)] [--memory] [--memory-gib G]
"""

import json
import multiprocessing
import os
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

from larmorx_validation import address_space
from larmorx_validation.difftest import limits
from larmorx_validation.difftest.tools import get
from larmorx_validation.parity.harness import CheckList, Outcome

#: The most workers a re-check runs at once.
MAX_JOBS = 8
#: The words of a finding whose verdict was a memory limit.
MEMORY_WORDS = ("exceeds the memory limit", "exceeds 3 GiB", "the kernel stopped it")


def kind_of(recipe):
    words = [a for g in recipe["groups"] for a in g["args"]]
    if recipe["extra"]["tool"] == "convert_xfm":
        return "convert"
    return "apply" if any(w.startswith("-apply") for w in words) else "register"


def is_memory(finding: dict) -> bool:
    text = " ".join([str(finding.get("detail", "")), *map(str, finding.get("signature", []))])
    return any(w in text for w in MEMORY_WORDS)


def enter_leaf(limit: int) -> None:
    """A worker's start: its own leaf of the box (every program it runs held there)."""
    box = limits.MemoryBox.find()
    if box is None:
        raise SystemExit("recheck worker: no memory box")
    box.leaf(f"recheck-{os.getpid()}", limit).enter()


def run(path_and_which):
    path, which = path_and_which
    d = json.loads(Path(path).read_text())
    recipe = d["shrunk"] if which == "shrunk" and d.get("shrunk") else d["recipe"]
    tool = get(d.get("tool", "fsl-flirt"))
    case = tool.case(recipe, "replica")
    checks = CheckList()
    leaf = limits.current()
    kills = leaf.oom_kills() if leaf else 0
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
        if leaf and leaf.oom_kills() > kills:
            return path, which, f"memory: a run exceeded {limits.gib(leaf.limit)}"
        return path, which, f"error {type(e).__name__}: {e}"[:200]
    if leaf and leaf.oom_kills() > kills:
        return path, which, f"memory: a run exceeded {limits.gib(leaf.limit)}"
    failed = [c for c in checks.checks if not c.passed]
    if not failed:
        return path, which, "pass"
    return path, which, "FAIL " + "; ".join(f"{c.name}: {c.detail}"[:160] for c in failed)


def case_limit(root: Path, given: float | None) -> int:
    """The per-case memory limit in bytes: ``--memory-gib``, else the run's own, else 3 GiB."""
    if given is not None:
        return int(given * limits.GIB)
    for d in (root, *root.parents):
        info = d / "run.json"
        if info.is_file():
            case = (json.loads(info.read_text()).get("memory") or {}).get("case")
            if isinstance(case, str) and case.endswith(" GiB"):
                return int(float(case.split()[0]) * limits.GIB)
            break
    return int(limits.CASE_GIB * limits.GIB)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    flags = sys.argv[1:]
    with_memory = "--memory" in flags
    given = None
    if "--memory-gib" in flags:
        given = float(flags[flags.index("--memory-gib") + 1])
        args = [a for a in args if a != flags[flags.index("--memory-gib") + 1]]
    root = Path(args[0])
    kind = args[1]
    jobs = min(int(args[2]) if len(args) > 2 else MAX_JOBS, MAX_JOBS)
    box = limits.MemoryBox.find()
    if box is None:
        raise SystemExit(
            "recheck_findings.py runs only inside a memory box: "
            "scripts/orchestration/boxed.sh <GiB> python -P recheck_findings.py ... "
            "(each worker then gets the trial's per-case memory limit)"
        )
    limit = case_limit(root, given)
    if jobs * limit > box.limit:
        jobs = max(1, box.limit // limit)
    if with_memory:
        jobs = 1
    os.environ.setdefault(address_space.ENV, str(address_space.for_memory(limit)))
    box.nest()
    todo = []
    skipped = []
    for f in sorted(root.glob("*.json")):
        d = json.loads(f.read_text())
        if kind != "all" and kind_of(d["recipe"]) != kind:
            continue
        if is_memory(d) and not with_memory:
            skipped.append(f.stem)
            continue
        todo += [(str(f), "shrunk"), (str(f), "drawn")]
    print(
        f"box {limits.gib(box.limit)}, {jobs} worker(s) of {limits.gib(limit)} each, address "
        f"space {address_space.describe()}; {len(skipped)} memory finding(s) skipped"
        + (" (--memory runs them)" if skipped else ""),
        flush=True,
    )
    spawn = multiprocessing.get_context("spawn")
    with ProcessPoolExecutor(
        jobs, mp_context=spawn, initializer=enter_leaf, initargs=(limit,)
    ) as ex:
        results = list(ex.map(run, todo))
    bad = 0
    for path, which, res in results:
        ok = res == "pass" or res.startswith("outcome")
        bad += not ok
        print(Path(path).stem, which, res[:300])
    for s in skipped:
        print(s, "skipped (a memory finding)")
    print(f"{len(results)} runs, {bad} still differ, {len(skipped)} memory findings skipped")


if __name__ == "__main__":
    main()
