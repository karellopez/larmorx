# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx's clean room of the C library's ``rand``, ``srand``, ``random`` and
``srandom`` (``larmorx_core::glibc_random``, ``specs/glibc-random.md``) with glibc 2.35's own
functions and FreeSurfer 8.2.0's ``mri_robust_template``, run as black boxes: the spec's validation
plan, §10 items 1 to 7, and the speed of both.

- the probe ``glibc_random_probe`` (``validation/investigations/glibc-random-probe/``: calls of the
  C library's functions only, built with ``cc -O2``) and the official ``mri_robust_template`` and
  ``mri_volsynth`` run through the oracle-output cache (``oracle_cache.run``, one thread, the
  family ``freesurfer``; the probe's key holds the C library it loads);
- the clean room runs through ``cleanroom-probe glibc`` (beside this script: the probe's commands).

Usage (the README): ``python -P compare_glibc_random.py --probe <glibc_random_probe> --cleanroom
<cleanroom-probe> --out <result.json> [--jobs 4] [--sections ...]``. Prints one line per section
and exits 1 when anything differs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import re
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from larmorx_validation import oracle_cache

HERE = Path(__file__).resolve().parent
TWO_31 = 1 << 31
TWO_32 = 1 << 32
M = TWO_31 - 1
RUNNER = "freesurfer-cleanroom-glibc-random"

# §10 item 2: the edge seeds (32-bit), and C's conversion of a long
EDGE_SEEDS = [
    0,
    1,
    2,
    3,
    53,
    1234,
    2836,
    16807,
    127772,
    127773,
    127774,
    M - 1,
    M,
    TWO_31,
    TWO_31 + 1,
    M + 127773,
    TWO_32 - 2,
    TWO_32 - 1,
    2 * M,
    TWO_32 - 1 - 127773,
    (16807 * 127773) % TWO_32,
]
LONG_SEEDS = [
    -1,
    -2,
    -53,
    -M,
    -TWO_31,
    -TWO_31 - 1,
    TWO_32,
    TWO_32 + 5,
    (1 << 40) + 7,
    -(1 << 40) - 7,
    (1 << 63) - 1,
    -(1 << 63),
]
# §6: the reference numbers
LONG_HASHES = {
    1: "8b12006d0a30178c",
    53: "8f2bb11c5dd49223",
    1234: "54ec7c5ad4bf1bfe",
    TWO_31: "98464c3c54ef732f",
}
BLOCK_HASHES = {(0, 1 << 20, 8): "5d031cec3a78334a", (2147483136, 1024, 64): "d3acc423130f2161"}
# §10 item 6: every seed, in blocks of 2^24 seeds, the first 4 values of each
EVERY_BLOCK = 1 << 24
EVERY_VALUES = 4
# §10 item 7: mri_robust_template's random first time point
TP_COUNTS = [2, 3, 5, 7]
TP_SEEDS = [1, 2, 53, 1234, 99991, M, -1]
TP_LINE = re.compile(r"Will use TP (\d+) as random initial target \(seed (-?\d+) \)")


def stamp(*programs: Path) -> dict:
    """The larmorx commit (``-dirty`` when the tree is not clean), the programs' SHA-256 and the C
    library's version."""
    commit = subprocess.run(
        ["git", "-C", str(HERE), "describe", "--always", "--dirty", "--abbrev=12"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    libc = subprocess.run(["ldd", "--version"], capture_output=True, text=True, check=False)
    sums = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in programs}
    return {
        "larmorx_commit": commit,
        "sha256": sums,
        "libc": libc.stdout.splitlines()[0] if libc.stdout else "",
    }


class Context:
    def __init__(self, args: argparse.Namespace) -> None:
        self.workspace = Path(args.workspace)
        self.probe = Path(args.probe).resolve()
        self.cleanroom = Path(args.cleanroom).resolve()
        self.fs_home = self.workspace / "oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0"
        self.license = Path(args.fs_license)
        self.scratch = Path(args.scratch)
        self.env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OMP_NUM_THREADS": "1"}
        self.policy = oracle_cache.Policy(threads=1, family="freesurfer")
        self.runs = 0
        self.served = 0

    def original(self, args: list[str]) -> bytes:
        """The probe's standard output, through the oracle-output cache."""
        o = oracle_cache.run(
            [str(self.probe), *args], env=self.env, policy=self.policy, runner=RUNNER
        )
        self.count(o)
        if o.returncode != 0:
            raise RuntimeError(f"probe {args}: exit {o.returncode}: {o.stderr_bytes!r}")
        return o.stdout_bytes

    def count(self, o: oracle_cache.Completed) -> None:
        self.runs += 1
        self.served += o.cached

    def ours(self, args: list[str]) -> bytes:
        p = subprocess.run(
            [str(self.cleanroom), "glibc", *args], capture_output=True, timeout=3600, check=True
        )
        return p.stdout


def first_difference(a: bytes, b: bytes) -> int | None:
    la, lb = a.split(b"\n"), b.split(b"\n")
    for i, (x, y) in enumerate(zip(la, lb, strict=False)):
        if x != y:
            return i
    return None if len(la) == len(lb) else min(len(la), len(lb))


def compare(ctx: Context, args: list[str]) -> dict:
    a, b = ctx.original(args), ctx.ours(args)
    same = a == b
    row = {"command": " ".join(args), "lines": a.count(b"\n"), "identical": same}
    if not same:
        row["first_difference_line"] = first_difference(a, b)
    return row


def compare_all(ctx: Context, commands: list[list[str]], jobs: int) -> dict:
    with ThreadPoolExecutor(jobs) as pool:
        rows = list(pool.map(lambda c: compare(ctx, c), commands))
    bad = [r for r in rows if not r["identical"]]
    values = sum(r["lines"] for r in rows)
    return {
        "commands": len(rows),
        "values": values,
        "identical": len(rows) - len(bad),
        "different": bad,
        "ok": not bad,
    }


def random_seeds() -> tuple[list[int], list[int]]:
    """§10 item 3: 150 32-bit seeds and 30 64-bit seeds (signed), from a fixed seed."""
    rng = random.Random(20261113)
    narrow = [rng.randrange(0, TWO_32) for _ in range(150)]
    wide = [rng.randrange(-(1 << 63), 1 << 63) for _ in range(30)]
    return narrow, wide


def section_default(ctx: Context, jobs: int) -> dict:
    """§10 item 1: the stream before any seeding."""
    return compare_all(ctx, [["default", "100000"]], jobs)


def section_edge(ctx: Context, jobs: int) -> dict:
    """§10 item 2: the edge seeds and C's conversion of a long, 10,000 values each, through
    srand and rand, and through srandom and random."""
    seeds = [str(s) for s in EDGE_SEEDS + LONG_SEEDS]
    cmds = [[c, s, "10000"] for s in seeds for c in ("rand", "random")]
    r = compare_all(ctx, cmds, jobs)
    r["seeds"] = len(seeds)
    return r


def section_random(ctx: Context, jobs: int) -> dict:
    """§10 item 3: 150 32-bit and 30 64-bit seeds, 10,000 values each; srandom and random for
    every third seed."""
    narrow, wide = random_seeds()
    seeds = [str(s) for s in narrow + wide]
    cmds = [["rand", s, "10000"] for s in seeds]
    cmds += [["random", s, "10000"] for s in seeds[::3]]
    r = compare_all(ctx, cmds, jobs)
    r["seeds"] = len(seeds)
    return r


def section_shared(ctx: Context, jobs: int) -> dict:
    """§10 item 4: one state: srand then alternating rand and random; srand and srandom; srandom
    alone. The three blocks of 1,000 values must be the same (and the clean room's the probe's)."""
    narrow, _ = random_seeds()
    seeds = [str(s) for s in [0, 1, 53, M, TWO_31, TWO_32 - 1, -1, *narrow[:13]]]
    rows = []
    for s in seeds:
        out = ctx.original(["shared", s, "1000"])
        lines = out.split(b"\n")
        rows.append(lines[0:1000] == lines[1000:2000] == lines[2000:3000])
    r = compare_all(ctx, [["shared", s, "1000"] for s in seeds], jobs)
    r["three_blocks_alike"] = sum(rows)
    r["ok"] = r["ok"] and all(rows)
    return r


def section_long(ctx: Context, jobs: int) -> dict:
    """§10 item 5 and §6: the 10^6-value hashes and the two block hashes, against the spec's
    numbers too."""
    cmds = [["long", str(s), "1000000"] for s in LONG_HASHES]
    cmds += [["hash", str(a), str(b), str(c)] for a, b, c in BLOCK_HASHES]
    r = compare_all(ctx, cmds, jobs)
    want = list(LONG_HASHES.values()) + list(BLOCK_HASHES.values())
    got = [ctx.ours(c).decode().strip() for c in cmds]
    r["spec_numbers_identical"] = sum(g == w for g, w in zip(got, want, strict=True))
    r["ok"] = r["ok"] and r["spec_numbers_identical"] == len(want)
    return r


def section_every(ctx: Context, jobs: int) -> dict:
    """§10 item 6: every one of the 2^32 seeds, the first 4 values of each, compared block by
    block (2^24 seeds per block: the probe's ``hash`` of each block against the clean room's)."""
    nblocks = TWO_32 // EVERY_BLOCK
    t0 = time.perf_counter()
    ours_out = ctx.ours(
        ["every", "0", str(TWO_32), str(EVERY_VALUES), str(EVERY_BLOCK), str(jobs)]
    ).decode()
    ours_s = time.perf_counter() - t0
    ours = dict(line.split() for line in ours_out.splitlines())

    def one(b: int) -> tuple[str, str]:
        s0 = str(b * EVERY_BLOCK)
        out = ctx.original(["hash", s0, str(EVERY_BLOCK), str(EVERY_VALUES)])
        return s0, out.decode().strip()

    t1 = time.perf_counter()
    with ThreadPoolExecutor(jobs) as pool:
        probe = dict(pool.map(one, range(nblocks)))
    probe_s = time.perf_counter() - t1
    bad = [s for s in probe if probe[s] != ours.get(s)]
    return {
        "seeds": TWO_32,
        "values_per_seed": EVERY_VALUES,
        "blocks": nblocks,
        "identical_blocks": nblocks - len(bad),
        "different_blocks": bad,
        "cleanroom_seconds": round(ours_s, 1),
        "probe_seconds": round(probe_s, 1),
        "ok": not bad and len(ours) == nblocks,
    }


def section_robust(ctx: Context, jobs: int) -> dict:
    """§10 item 7: the official mri_robust_template's random first time point, "Will use TP k as
    random initial target", against (o0 mod N) + 1 from the clean room (o0 the first value after
    srand of the seed converted to unsigned int); the inputs are 8 x 8 x 8 volumes made by the
    official mri_volsynth."""
    env = dict(ctx.env, FREESURFER_HOME=str(ctx.fs_home), FS_LICENSE=str(ctx.license))
    bindir = ctx.fs_home / "bin"
    inputs = ctx.workspace / "oracles/inputs/freesurfer-cleanroom-glibc-random"
    inputs.mkdir(parents=True, exist_ok=True)
    fs_policy = oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer")
    for i in range(1, max(TP_COUNTS) + 1):
        with tempfile.TemporaryDirectory(dir=ctx.scratch, prefix="lx-volsynth_") as tmp:
            vol = Path(tmp) / "v.mgz"
            o = oracle_cache.run(
                [
                    str(bindir / "mri_volsynth"),
                    *("--pdf", "z", "--dim", "8", "8", "8", "1", "--seed", str(i)),
                    *("--vol", str(vol)),
                ],
                env=env,
                cwd=Path(tmp),
                outputs=[Path(tmp)],
                policy=fs_policy,
                runner=RUNNER,
            )
            ctx.count(o)
            if o.returncode != 0:
                raise RuntimeError(f"mri_volsynth {i}: exit {o.returncode}")
            shutil.copyfile(vol, inputs / f"v{i}.mgz")

    def one(case: tuple[int, int]) -> dict:
        n, seed = case
        with tempfile.TemporaryDirectory(dir=ctx.scratch, prefix="lx-robust_") as tmp:
            t0 = time.perf_counter()
            o = oracle_cache.run(
                [
                    str(bindir / "mri_robust_template"),
                    "--mov",
                    *(str(inputs / f"v{i}.mgz") for i in range(1, n + 1)),
                    *("--template", str(Path(tmp) / "template.mgz"), "--satit"),
                    *("--seed", str(seed)),
                ],
                env=env,
                cwd=Path(tmp),
                outputs=[Path(tmp)],
                policy=fs_policy,
                timeout=600,
                runner=RUNNER,
            )
            seconds = time.perf_counter() - t0
        ctx.count(o)
        text = o.text() + o.text("stderr")
        m = TP_LINE.search(text)
        o0 = int(ctx.ours(["rand", str(seed), "1"]).decode())
        want = o0 % n + 1
        got = int(m.group(1)) if m else None
        return {
            "inputs": n,
            "seed": seed,
            "program_tp": got,
            "program_seed": int(m.group(2)) if m else None,
            "cleanroom_tp": want,
            "status": o.returncode,
            "seconds": round(seconds, 1),
            "identical": got == want,
        }

    cases = [(n, s) for n in TP_COUNTS for s in TP_SEEDS]
    with ThreadPoolExecutor(jobs) as pool:
        rows = list(pool.map(one, cases))
    same = sum(r["identical"] for r in rows)
    return {"cases": rows, "identical": same, "total": len(rows), "ok": same == len(rows)}


def section_speed(ctx: Context) -> dict:
    """The speed of drawing (``long 53 10^8``: 10^8 values and their hash, in one process) and of
    seeding (``hash 0 2^20 8``: 2^20 seedings with 8 values each), the probe against the clean
    room, the best of five runs each (not through the cache: these runs measure time)."""

    def timed(argv: list[str]) -> float:
        best = None
        for _ in range(5):
            t0 = time.perf_counter()
            subprocess.run(argv, stdout=subprocess.DEVNULL, check=True)
            dt = time.perf_counter() - t0
            best = dt if best is None else min(best, dt)
        return round(best, 3)

    rows = {}
    for name, args in (
        ("draw_1e8", ["long", "53", "100000000"]),
        ("seed_2e20", ["hash", "0", "1048576", "8"]),
    ):
        rows[name] = {
            "probe_s": timed([str(ctx.probe), *args]),
            "cleanroom_s": timed([str(ctx.cleanroom), "glibc", *args]),
        }
        rows[name]["ratio"] = round(rows[name]["probe_s"] / rows[name]["cleanroom_s"], 2)
    return {**rows, "ok": True}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument(
        "--workspace", default=os.environ.get("LARMORX_WORKSPACE", str(HERE.parents[3]))
    )
    ap.add_argument("--fs-license", default="/usr/local/freesurfer/license.txt")
    ap.add_argument("--probe", required=True, help="glibc_random_probe, built with cc -O2")
    ap.add_argument("--cleanroom", required=True, help="the built cleanroom-probe program")
    ap.add_argument("--scratch", default=tempfile.gettempdir())
    ap.add_argument("--out", required=True, help="the result's JSON file")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--sections", default="default,edge,random,shared,long,every,robust,speed")
    args = ap.parse_args()
    ctx = Context(args)
    result: dict = {"date": time.strftime("%Y-%m-%d"), "machine": os.uname().machine}
    result.update(stamp(ctx.probe, ctx.cleanroom, ctx.fs_home / "bin/mri_robust_template"))
    status = 0
    for name in args.sections.split(","):
        t0 = time.perf_counter()
        if name == "default":
            r = section_default(ctx, args.jobs)
        elif name == "edge":
            r = section_edge(ctx, args.jobs)
        elif name == "random":
            r = section_random(ctx, args.jobs)
        elif name == "shared":
            r = section_shared(ctx, args.jobs)
        elif name == "long":
            r = section_long(ctx, args.jobs)
        elif name == "every":
            r = section_every(ctx, args.jobs)
        elif name == "robust":
            r = section_robust(ctx, args.jobs)
        elif name == "speed":
            r = section_speed(ctx)
        else:
            raise SystemExit(f"unknown section {name}")
        r["seconds"] = round(time.perf_counter() - t0, 1)
        result[name] = r
        summary = {k: v for k, v in r.items() if isinstance(v, (int, float, str, bool))}
        print(f"{name}: {json.dumps(summary)}", flush=True)
        if not r["ok"]:
            status = 1
    result["original_runs"] = {"total": ctx.runs, "from_cache": ctx.served}
    Path(args.out).write_text(json.dumps(result, indent=1) + "\n")
    print(f"COMPARE-GLIBC-RANDOM exit {status}")
    return status


if __name__ == "__main__":
    sys.exit(main())
