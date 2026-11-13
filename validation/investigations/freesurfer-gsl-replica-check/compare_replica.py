# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx's GSL replica (``crates-gpl/larmorx-gpl-gsl``: GSL 1.8's RANLUX389 and
distributions as FreeSurfer 8.2.0's ``numerics.cpp`` transcribes them) with FreeSurfer's own
functions and programs, and with larmorx's clean room (``larmorx_core::ranlux``):

- ``freesurfer``: ``specs/ranlux.md`` §10.1 to §10.5 with the replica in the clean room's place:
  the sections of ``compare_random.py`` (beside the clean room's validation,
  ``validation/investigations/freesurfer-cleanroom-random-spline/``), run on the replica's probe
  ``gsl_probe`` (the crate's example, the same commands): 2,022 seeds' first 100,000 values, 421
  distribution cases of 20,000 draws, the ``mixed`` streams, FS-100 and FS-101 in both modes, the
  official ``mri_volsynth``; the large-shape gamma draws that meet one of glibc's 41,708 tangent
  arguments counted apart (larmorx's ``tan`` is correctly rounded: the user's decision of
  2026-11-04), and the same draws with the C library's ``tan`` (``--system-tan``) bit for bit;
- ``crafted``: §10.2's ``crafted`` draws (a state whose next two integers are chosen): the
  ``R = 1`` cases of §4.2 and 300 seeded pairs through 13 draws, the replica against
  ``fs_rng_probe`` built from the probe's current source (``--crafted-probe``);
- ``cleanroom``: the replica against the clean room's ``cleanroom-probe`` on every command above,
  in both modes and with both tangents: exit status, standard output and standard error byte for
  byte (the two implementations print the same notices and errors);
- ``speed``: 20 million integers, printed, by the probe, the replica and the clean room.

FreeSurfer's programs run through the oracle-output cache (one thread, the family ``freesurfer``);
the replica and the clean room run directly. Usage (README): ``python -P compare_replica.py
--replica <gsl_probe> --cleanroom <cleanroom-probe> --crafted-probe <fs_rng_probe> --out
<result.json> [--quick]``. Exits 1 when anything unexpected differs.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import random
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
CLEANROOM_DIR = HERE.parent / "freesurfer-cleanroom-random-spline"


def load_compare_random():
    spec = importlib.util.spec_from_file_location(
        "compare_random", CLEANROOM_DIR / "compare_random.py"
    )
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


cr = load_compare_random()

CRAFTED_DRAWS: list[tuple[str, list[str]]] = [
    ("uni", []),
    ("uni_pos", []),
    ("flat", ["-3.5", "7.25"]),
    ("gaussian", ["2.5"]),
    ("ugauss", []),
    ("exponential", ["0.75"]),
    ("gamma", ["3", "2"]),
    ("gamma", ["0.4", "1.5"]),
    ("gamma", ["25", "1"]),
    ("chisq", ["5"]),
    ("fdist", ["4", "9"]),
    ("tdist", ["1.5"]),
    ("tdist", ["7"]),
]
R_ONE = [
    ("gaussian", a, b, [s])
    for a, b in (("0", "8388608"), ("8388608", "0"))
    for s in ("1", "2.5", "-1")
] + [("ugauss", a, b, []) for a, b in (("0", "8388608"), ("8388608", "0"))]


def run(argv: list[str]) -> tuple[int, bytes, bytes]:
    p = subprocess.run(argv, capture_output=True, timeout=600, check=False)
    return p.returncode, p.stdout, p.stderr


def section_freesurfer(ctx, jobs: int, quick: bool) -> dict:
    out = {}
    out["raw"] = cr.section_raw(ctx, jobs, 20 if quick else 1000)
    out["dist"] = cr.section_dist(ctx, jobs, "2000" if quick else "20000")
    out["defects"] = cr.section_defects(ctx)
    out["volsynth"] = cr.section_volsynth(ctx)
    out["tan"] = cr.section_tan(ctx)
    out["ok"] = all(v["ok"] for v in out.values() if isinstance(v, dict))
    return out


def section_crafted(ctx, jobs: int, quick: bool) -> dict:
    rng = random.Random(20261113)
    cases = [(name, a, b, p) for name, a, b, p in R_ONE]
    n = 30 if quick else 300
    pairs = [(0, 1), (1, 0), (16777215, 16777215), (8388608, 8388608), (0, 16777215)]
    while len(pairs) < n:
        a, b = rng.randrange(1 << 24), rng.randrange(1 << 24)
        if a or b:
            pairs.append((a, b))
    cases += [(name, str(a), str(b), p) for a, b in pairs for name, p in CRAFTED_DRAWS]
    rows = list(
        ThreadPoolExecutor(jobs).map(
            lambda c: cr.compare_draws(ctx, ["crafted", c[0], c[1], c[2], *c[3]]), cases
        )
    )
    bad = [r for r in rows if r["unexpected"] or not r["system_tan_identical"]]
    return {
        "cases": len(rows),
        "identical": sum(r["identical"] == r["draws"] for r in rows),
        "expected_tan_differences": sum(r["expected_tan_differences"] for r in rows),
        "r_equals_1": [r for r in rows[: len(R_ONE)]],
        "failures": bad,
        "ok": not bad,
    }


def cleanroom_commands(quick: bool) -> list[list[str]]:
    wide, narrow = cr.random_seeds(20 if quick else 1000)
    seeds = cr.TABLE_SEEDS + cr.EXTRA_SEEDS + cr.BOUNDARY_SEEDS + wide + narrow
    count = "2000" if quick else "20000"
    cmds = [["raw", s, "100000"] for s in seeds + cr.ZERO_SEEDS]
    cmds += [
        ["dist", name, seed, count, *params]
        for seed in cr.DIST_SEEDS
        for name, params in cr.DIST_CASES
    ]
    cmds += [["mixed", seed, "14000"] for seed in cr.DIST_SEEDS]
    cmds += [["dist", "tdist", "53", "3", "0x1.0000000000001p+1"]]
    cmds += [
        ["dist", "gamma", seed, "2000", a, "1"]
        for a in cr.FS100_VALUES + cr.FS100_HANGS
        for seed in cr.DIST_SEEDS
    ]
    for seed in (str(cr.ZERO_SEED), str(-cr.ZERO_SEED)):
        cmds += [["dist", name, seed, "100", *p] for name, p in cr.ZERO_RETURN + cr.ZERO_HANG]
    return cmds


def section_cleanroom(replica: Path, cleanroom: Path, jobs: int, quick: bool) -> dict:
    cmds = cleanroom_commands(quick)
    variants = [(), ("--reproduce",), ("--system-tan",), ("--reproduce", "--system-tan")]

    def one(item):
        opts, cmd = item
        a = run([str(replica), *opts, *cmd])
        b = run([str(cleanroom), *opts, *cmd])
        return {"command": " ".join([*opts, *cmd]), "same": a == b, "status": [a[0], b[0]]}

    items = [(v, c) for v in variants for c in cmds]
    rows = list(ThreadPoolExecutor(jobs).map(one, items))
    diff = [r for r in rows if not r["same"]]
    return {
        "runs": len(rows),
        "identical": len(rows) - len(diff),
        "errors_both": sum(r["status"] == [1, 1] for r in rows),
        "different": diff[:50],
        "ok": not diff,
    }


def section_speed(probe: Path, replica: Path, cleanroom: Path) -> dict:
    def timed(argv: list[str]) -> float:
        best = None
        for _ in range(3):
            t0 = time.perf_counter()
            subprocess.run(argv, stdout=subprocess.DEVNULL, check=True)
            dt = time.perf_counter() - t0
            best = dt if best is None else min(best, dt)
        return round(best, 3)

    cmd = ["raw", "53", "20000000"]
    return {
        "probe_raw_20M_printed_s": timed([str(probe), *cmd]),
        "replica_raw_20M_printed_s": timed([str(replica), *cmd]),
        "cleanroom_raw_20M_printed_s": timed([str(cleanroom), *cmd]),
        "replica_generation_only": subprocess.run(
            [str(replica), "bench", "53", "20000000"], capture_output=True, text=True, check=True
        ).stdout.strip(),
        "ok": True,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument(
        "--workspace", default=os.environ.get("LARMORX_WORKSPACE", str(HERE.parents[3]))
    )
    ap.add_argument("--fs-license", default="/usr/local/freesurfer/license.txt")
    ap.add_argument("--replica", required=True, help="the built gsl_probe (larmorx-gpl-gsl)")
    ap.add_argument("--cleanroom", required=True, help="the built cleanroom-probe")
    ap.add_argument(
        "--crafted-probe", required=True, help="fs_rng_probe built from its current source"
    )
    ap.add_argument("--scratch", default=tempfile.gettempdir())
    ap.add_argument("--out", required=True)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--quick", action="store_true")
    ap.add_argument("--sections", default="freesurfer,crafted,cleanroom,speed")
    args = ap.parse_args()
    ns = argparse.Namespace(
        workspace=args.workspace,
        fs_license=args.fs_license,
        cleanroom=args.replica,
        scratch=args.scratch,
    )
    ctx = cr.Context(ns)
    crafted_ctx = cr.Context(ns)
    crafted_ctx.probe = Path(args.crafted_probe)
    replica, cleanroom = Path(args.replica), Path(args.cleanroom)
    result: dict = {"date": time.strftime("%Y-%m-%d"), "machine": os.uname().machine}
    result.update(cr.stamp(ctx.probe, crafted_ctx.probe, replica, cleanroom))
    status = 0
    for name in args.sections.split(","):
        t0 = time.perf_counter()
        if name == "freesurfer":
            r = section_freesurfer(ctx, args.jobs, args.quick)
        elif name == "crafted":
            r = section_crafted(crafted_ctx, args.jobs, args.quick)
        elif name == "cleanroom":
            r = section_cleanroom(replica, cleanroom, args.jobs, args.quick)
        elif name == "speed":
            r = section_speed(ctx.probe, replica, cleanroom)
        else:
            raise SystemExit(f"unknown section {name}")
        r["seconds"] = round(time.perf_counter() - t0, 1)
        result[name] = r
        if name == "freesurfer":
            for k, v in r.items():
                if isinstance(v, dict):
                    flat = {a: b for a, b in v.items() if isinstance(b, (int, float, str, bool))}
                    print(f"freesurfer.{k}: {json.dumps(flat)}", flush=True)
        else:
            flat = {a: b for a, b in r.items() if isinstance(b, (int, float, str, bool))}
            print(f"{name}: {json.dumps(flat)}", flush=True)
        if not r["ok"]:
            status = 1
    Path(args.out).write_text(json.dumps(result, indent=1) + "\n")
    print(f"COMPARE-REPLICA exit {status}")
    return status


if __name__ == "__main__":
    sys.exit(main())
