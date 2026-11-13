# SPDX-License-Identifier: Apache-2.0
"""Speed of fs.core's hot services (spec §20.3), the clean room against the probe, black box.

The same batch files (the fast arctangent, the selections and the median, the byte swaps of
whole buffers, the reproducible sums) run on ``fs_core_probe`` and on the candidate, best of
three wall times each; a baseline batch of as many lines of a cheap command of a similar shape
estimates each program's own parsing and printing, so ``(batch - baseline) / lines`` approximates
the time per call. The candidate's ``bench`` command then gives the library's own time per call,
in process.

    scripts/orchestration/boxed.sh 2 python -P \\
        validation/investigations/freesurfer-cleanroom-core/speed.py \\
        --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_core_probe \\
        --candidate <the candidate> --scratch <scratch>/speed

Timing runs are never cached (they measure the programs); both run with one thread.
"""

from __future__ import annotations

import argparse
import random
import struct
import subprocess
import sys
import time
from pathlib import Path


def f32(v: float) -> str:
    return struct.pack(">f", v).hex()


def f64(v: float) -> str:
    return struct.pack(">d", v).hex()


def batches(r: random.Random) -> dict[str, tuple[list[str], list[str]]]:
    """name: (lines, baseline lines)."""
    n = 200_000
    pairs = [(f32(r.uniform(-100, 100)), f32(r.uniform(-100, 100))) for _ in range(n)]
    out = {
        "atan2f": (
            [f"atan2f {y} {x}" for y, x in pairs],
            [f"deltaangle {y} {x}" for y, x in pairs],
        )
    }
    m = 20_000
    lists = [" ".join(f32(r.uniform(-1, 1)) for _ in range(25)) for _ in range(m)]
    ints = " ".join("1" for _ in range(25))
    for cmd in ("kth 13", "qselect 13", "median"):
        out[cmd.split()[0]] = (
            [f"{cmd} {lst}" for lst in lists],
            [f"nunique {ints}" for _ in lists],
        )
    buf = bytes(r.randrange(256) for _ in range(4096)).hex()
    for k in (2, 4, 8):
        out[f"byteswapbuf-k{k}"] = (
            [f"byteswapbuf {k} {4096 // k} x{buf}" for _ in range(2000)],
            [f"strupper x{buf}" for _ in range(2000)],
        )
    terms = " ".join(f64(r.uniform(-1, 1)) for _ in range(1000))
    out["rompsum"] = (
        [f"rompsum {f64(0.0)} {terms}" for _ in range(500)],
        [f"dliststats 1000 {terms}" for _ in range(500)],
    )
    return out


def best(argv: list[str], env: dict[str, str], reps: int = 3) -> float:
    t = []
    for _ in range(reps):
        t0 = time.perf_counter()
        subprocess.run(
            argv, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False
        )
        t.append(time.perf_counter() - t0)
    return min(t)


BENCH = (
    ("atan2", 2_000_000),
    ("kth", 200_000),
    ("qselect", 200_000),
    ("median", 200_000),
    ("swap2", 200),
    ("swap4", 200),
    ("swap8", 200),
    ("rompsum1", 2_000_000),
    ("rompsum4", 2_000_000),
)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--probe", required=True, type=Path)
    ap.add_argument("--candidate", required=True, type=Path)
    ap.add_argument("--scratch", required=True, type=Path)
    a = ap.parse_args()
    a.scratch.mkdir(parents=True, exist_ok=True)
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OMP_NUM_THREADS": "1"}
    cenv = dict(env, LARMORX_UPSTREAM_DEFECTS="reproduce")
    head = ("batch", "lines", "probe s", "cand. s", "probe ns/call", "cand. ns/call")
    print("%-18s %7s %9s %9s %14s %14s" % head)
    for name, (lines, base) in batches(random.Random(7)).items():
        f = a.scratch / f"{name}.txt"
        g = a.scratch / f"{name}-base.txt"
        f.write_text("\n".join(lines) + "\n")
        g.write_text("\n".join(base) + "\n")
        tp = best([str(a.probe), "batch", str(f)], env)
        tpb = best([str(a.probe), "batch", str(g)], env)
        tc = best([str(a.candidate), "batch", str(f)], cenv)
        tcb = best([str(a.candidate), "batch", str(g)], cenv)
        n = len(lines)
        per_p = max(tp - tpb, 0) / n * 1e9
        per_c = max(tc - tcb, 0) / n * 1e9
        print("%-18s %7d %9.3f %9.3f %14.1f %14.1f" % (name, n, tp, tc, per_p, per_c))
    print("the candidate's library in process:")
    for op, n in BENCH:
        r = subprocess.run(
            [str(a.candidate), "bench", op, str(n)], env=cenv, capture_output=True, text=True
        )
        print("  " + r.stdout.strip())
    return 0


if __name__ == "__main__":
    sys.exit(main())
