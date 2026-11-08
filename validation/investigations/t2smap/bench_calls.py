# SPDX-License-Identifier: Apache-2.0
"""Wall time of fMRIPrep 25.2.6's multi-echo ``t2smap`` calls: tedana 26.0.3 (the newest kept
environment, through ``oracle_cache.run`` with a reason never to store the run: a timed run is the
one kind the cache cannot serve) against the replica (``larmorx-gpl tedana t2smap``) in
``reproduce`` mode and in its default mode (which runs the stages from the mask on again for each
family of maths functions whose other choice changes a value, to name NP-03 and GLIBC-07 exactly),
at one thread and at ``--threads``; every replica run's values checked against tedana's
(``reproduce``: every image's and the confounds table's values identical) and the default's runs
at several threads against one thread's (identical values).

The calls: the four ``t2smap_node`` working directories of the multi-echo oracle runs
(``curvefit``, fMRIPrep's default) and ds003592's again with ``--fittype loglin``. Usage (inside a
memory box; the environment of ``scripts/orchestration/env.sh``, ``LARMORX_GPL_BIN``):

    scripts/orchestration/boxed.sh 16 python -P validation/investigations/t2smap/bench_calls.py \\
        --work <empty dir> --threads 14 --repeat 3 --json docs/benchmarks/tedana-t2smap.json
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path

from larmorx_validation import oracle_cache
from larmorx_validation.parity import tedana_t2smap_replica as tt


def _tedana(args: list[str], cwd: Path) -> tuple[float, int]:
    prog = tt.launcher("newest")
    if prog is None:
        sys.exit("tedana is not installed in the newest environment")
    r = oracle_cache.run(
        [str(prog), *args, "--out-dir", "out"],
        env=tt._base_env(),
        cwd=cwd,
        outputs=[cwd],
        policy=oracle_cache.Policy(threads=1, family="python", never="a timed run"),
        runner="tedana-t2smap-bench",
        timeout=3600,
    )
    return r.seconds, r.returncode


def _replica(args: list[str], cwd: Path, mode: str, threads: int) -> tuple[float, int, str]:
    exe = tt.replica_binary()
    if exe is None:
        sys.exit("larmorx-gpl is not built (LARMORX_GPL_BIN)")
    e = tt._base_env()
    e["LARMORX_UPSTREAM_DEFECTS"] = mode
    e["LARMORX_TEDANA_LIBRARIES"] = "newest"
    t0 = time.perf_counter()
    p = subprocess.run(
        [str(exe), "tedana", "t2smap", *args, "--n-threads", str(threads), "--out-dir", "out"],
        cwd=cwd,
        env=e,
        capture_output=True,
        text=True,
        timeout=3600,
        check=False,
    )
    return time.perf_counter() - t0, p.returncode, p.stderr


def _named(stderr: str) -> list[str]:
    return sorted(
        {
            ln.split("upstream defect ", 1)[1].split()[0]
            for ln in stderr.splitlines()
            if "upstream defect " in ln
        }
    )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--threads", type=int, default=14)
    ap.add_argument("--repeat", type=int, default=3)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args()
    stages = tt._investigation("check_stages")
    rows = []
    a.work.mkdir(parents=True, exist_ok=True)
    for name, args in stages.real_cases():
        base = a.work / name
        if base.exists():
            shutil.rmtree(base)
        (base / "tedana").mkdir(parents=True)
        t_s, t_code = _tedana(args, base / "tedana")
        row = {"call": name, "tedana_s": round(t_s, 2), "tedana_exit": t_code}
        ref = base / "tedana" / "out"
        fix_one: Path | None = None  # the default's files at one thread, for the others
        for mode in ("reproduce", "fix"):
            for threads in (1, a.threads):
                best = None
                for k in range(a.repeat):
                    d = base / f"{mode}-{threads}-{k}"
                    d.mkdir()
                    s, code, err = _replica(args, d, mode, threads)
                    if code != 0:
                        sys.exit(f"{name}: the replica exits {code}: {err[-400:]}")
                    best = s if best is None else min(best, s)
                    out = d / "out"
                    keep = False
                    if mode == "reproduce":
                        # every image's and the table's values (the call and the description
                        # name --n-threads, which tedana's run does not have)
                        diffs = {k: v for k, v in tt.value_differences(ref, out).items() if v[0]}
                        if diffs:
                            sys.exit(f"{name}: reproduce differs from tedana: {diffs}")
                    elif fix_one is None:
                        fix_one, keep = out, True
                        row["fix_named"] = _named(err)
                    elif any(v[0] for v in tt.value_differences(fix_one, out).values()):
                        sys.exit(f"{name}: the default at {threads} threads differs from 1")
                    if not keep:
                        shutil.rmtree(d)
                row[f"{mode}_{threads}_s"] = round(best, 3)
        rows.append(row)
        shutil.rmtree(base)
        print(json.dumps(row), flush=True)
    load = os.getloadavg()
    meta = {
        "machine": platform.machine(),
        "cpu_count": os.cpu_count(),
        "load_average_end": [round(x, 2) for x in load],
        "threads": a.threads,
        "repeat": a.repeat,
        "tedana": tt.TEDANA,
        "replica": tt.replica_version(),
    }
    if a.json:
        a.json.write_text(json.dumps({"meta": meta, "rows": rows}, indent=2) + "\n")
    print("| call | tedana | reproduce (1) | reproduce (N) | default (1) | default (N) |")
    print("|---|---|---|---|---|---|")
    for r in rows:
        n = a.threads
        cells = [
            f"{r[k]:.2f} s ({r['tedana_s'] / r[k]:.1f}x)"
            for k in ("reproduce_1_s", f"reproduce_{n}_s", "fix_1_s", f"fix_{n}_s")
        ]
        print(f"| {r['call']} | {r['tedana_s']:.1f} s | " + " | ".join(cells) + " |")
    print(json.dumps(meta))
    return 0


if __name__ == "__main__":
    sys.exit(main())
