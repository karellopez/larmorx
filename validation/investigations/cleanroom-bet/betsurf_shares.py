# SPDX-License-Identifier: Apache-2.0
"""betsurf_shares.py <trial dir> <out.jsonl> [--jobs 16] [--mode reproduce|fix] [--only 47,78]:
every betsurf finding of a random-testing trial's clean-room original run again with ``-v``
added, by FSL 6.0.7.23 (through the oracle-output cache, use mode) and by the original (in
process), black box: the profiles' counts and the share set aside that each prints, and for every
mesh the vertex distance (95th percentile, maximum) and every mask's differing voxels, so that a
difference can be placed at the profiles (the shares differ) or after them (the shares agree).
Each worker runs in its own leaf of the memory box with the trial's case limit: run it inside
``scripts/orchestration/boxed.sh``. The clean-room catch-up, relay 7 (2026-11-05)."""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import re
import sys
import tempfile
from pathlib import Path

import numpy as np

_SHARE = re.compile(r" nb of profiles : (\S+)\n removed profiles : (\S+)%")


def _enter_leaf(limit: int) -> None:
    from larmorx_validation.difftest import limits

    box = limits.MemoryBox.find()
    if box is not None:
        box.leaf(f"case-{os.getpid()}", limit).enter()


def _points(p: Path) -> np.ndarray | None:
    if not p.exists():
        return None
    lines = p.read_text(encoding="utf-8").split("\n")
    i = next(k for k, ln in enumerate(lines) if ln.startswith("POINTS"))
    n = int(lines[i].split()[1])
    return np.array([[float(t) for t in lines[i + 1 + k].split()] for k in range(n)])


def one(job: tuple[int, dict, str]) -> dict:
    from larmorx_validation.difftest.tools import fsl_bet as tool
    from larmorx_validation.parity import fsl_bet as fb

    n, recipe, mode = job
    s = tool.case(recipe, "original").payload
    paths = [fb.input_path(x) for x in s.inputs]
    args = ["-v", *fb._arguments(s, paths)]
    env = fb.environment(s.output_type, s.env)
    fb.IMPLEMENTATION["name"] = "original"
    row: dict = {"n": n, "args": " ".join(args[:-4] if len(args) > 4 else args)}
    with tempfile.TemporaryDirectory(prefix="lx-shares-") as tmp:
        dirs = {}
        for who in ("fsl", "lx"):
            d = Path(tmp) / who
            d.mkdir()
            r = (
                fb.oracle_run("betsurf", args, env, d)
                if who == "fsl"
                else fb.run_larmorx("betsurf", args, mode, env, d)
            )
            m = _SHARE.search(r.out)
            row[who] = {
                "exit": r.code,
                "profiles": m.group(1) if m else None,
                "removed": m.group(2) if m else None,
            }
            dirs[who] = d
        for name in sorted(os.listdir(dirs["fsl"])):
            a, b = dirs["fsl"] / name, dirs["lx"] / name
            if name.endswith(".vtk"):
                pa, pb = _points(a), _points(b)
                if pa is not None and pb is not None and pa.shape == pb.shape:
                    dist = np.linalg.norm(pa - pb, axis=1)
                    row[name] = [float(np.percentile(dist, 95)), float(dist.max())]
                else:
                    row[name] = "other vertices"
            elif fb.is_image(name) and b.exists():
                try:
                    x, y = fb.image_data(a), fb.image_data(b)
                except Exception as e:
                    row[name] = f"unreadable: {type(e).__name__}"
                    continue
                if x.shape != y.shape:
                    row[name] = "other shape"
                    continue
                row[name] = [int((x != 0).sum()), int((y != 0).sum()), int((x != y).sum())]
    return row


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("trial")
    ap.add_argument("out")
    ap.add_argument("--jobs", type=int, default=16)
    ap.add_argument("--mode", default="reproduce")
    ap.add_argument("--only", default="")
    a = ap.parse_args(argv)
    os.environ.setdefault("LARMORX_ORACLE_CACHE_MODE", "use")
    trial = Path(a.trial)
    run = json.loads((trial / "run.json").read_text(encoding="utf-8"))
    limit = int(float(str(run.get("memory", {}).get("case", "3 GiB")).split()[0]) * (1 << 30))
    only = {int(x) for x in a.only.split(",") if x}
    jobs = []
    for f in sorted((trial / "findings").glob("*/*-original.json")):
        doc = json.loads(f.read_text(encoding="utf-8"))
        if doc["recipe"]["extra"].get("tool") != "betsurf" or (only and doc["n"] not in only):
            continue
        jobs.append((doc["n"], doc["recipe"], a.mode))
    from larmorx_validation.difftest import limits

    box = limits.MemoryBox.find()
    if box is not None:
        box.nest()
    with (
        open(a.out, "w", encoding="utf-8") as out,
        concurrent.futures.ProcessPoolExecutor(
            a.jobs, initializer=_enter_leaf, initargs=(limit,)
        ) as ex,
    ):
        for r in ex.map(one, jobs):
            out.write(json.dumps(r) + "\n")
            out.flush()
    print(f"SHARES DONE {len(jobs)}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
