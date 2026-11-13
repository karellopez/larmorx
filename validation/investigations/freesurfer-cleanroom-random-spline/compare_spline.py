# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx's clean room of FreeSurfer 8.2.0's cubic spline (``larmorx_core::cubic_spline``,
``specs/burkardt-spline.md``) with FreeSurfer's own functions, run as black boxes through the
probe ``fs_spline_probe`` (prebuilt in ``<workspace>/oracles/freesurfer-8.2.0-unit/probes/``):
the spec's validation plan §9.1 to §9.3, bit for bit and byte for byte.

- ``cases``: seeded random cases (``n`` from 2 to 80; knots as row positions, random floats,
  neighbours one unit in the last place apart, widely spread magnitudes; random and constant
  values; every pair of end conditions; points below, at and above every knot, between knots,
  ±0, ±∞, NaN; one case in 50 with NaN values of two payloads and a NaN beginning value, one in
  77 with an infinite value);
- ``d3``: tridiagonal systems (zero diagonals, diagonals that become 0, dominant and not);
- ``set1``: the five fatal errors' messages and exit status;
- FS-103: NaN knots, the probe against the clean room's ``reproduce``, and the default's E2.

The probe reads its cases on standard input; the oracle-output cache passes none, so it runs the
probe through ``stdin-runner/run_stdin.sh`` (the probe and the input file are its arguments, keyed
by their content). Usage: ``python -P compare_spline.py --cleanroom <cleanroom-probe> --out
<result.json> [--cases 20000]``; exits 1 when anything differs.
"""

from __future__ import annotations

import argparse
import json
import os
import random
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import numpy as np

from larmorx_validation import oracle_cache

HERE = Path(__file__).resolve().parent
RUNNER = HERE / "stdin-runner/run_stdin.sh"
BATCH = 500


def hx(v: float | np.float32) -> str:
    return f"{struct.unpack('<I', struct.pack('<f', float(v)))[0]:08x}"


def hx_bits(b: int) -> str:
    return f"{b:08x}"


def f32(v: float) -> np.float32:
    return np.float32(v)


def increasing(vals: list[np.float32]) -> bool:
    return all(b > a for a, b in zip(vals, vals[1:]))


def knots(rng: random.Random, n: int) -> list[np.float32]:
    kind = rng.choice(["rows", "floats", "ulp", "spread"])
    while True:
        if kind == "rows":
            t = sorted(rng.sample(range(256), n))
            out = [f32(v) for v in t]
        elif kind == "floats":
            v = rng.uniform(-100, 100)
            out = []
            for _ in range(n):
                out.append(f32(v))
                v += rng.uniform(0.01, 10)
        elif kind == "ulp":
            v = f32(rng.choice([-1, 1]) * rng.uniform(1e-3, 1e3))
            out = []
            for _ in range(n):
                out.append(v)
                if rng.random() < 0.5:
                    v = np.nextafter(v, np.float32(np.inf), dtype=np.float32)
                else:
                    v = f32(float(v) + abs(float(v)) * rng.uniform(1e-3, 0.5) + 1e-3)
        else:
            mags = sorted(rng.choice([-1, 1]) * 10 ** rng.uniform(-6, 6) for _ in range(n))
            out = [f32(m) for m in mags]
        if increasing(out):
            return out


def special(rng: random.Random) -> int:
    return rng.choice([0x7FC00123, 0xFFC00456, 0x7FA00077, 0xFF800000, 0x7F800000])


def case_line(rng: random.Random, k: int, nan_knot: bool = False) -> str:
    n = rng.choice([2, 2, 3, rng.randint(2, 80), rng.randint(2, 12)])
    t = knots(rng, n)
    if rng.random() < 0.1:
        c = f32(rng.uniform(-50, 50))
        y = [c] * n
    else:
        y = [f32(rng.uniform(-1000, 1000)) for _ in range(n)]
    ib = rng.randint(0, 2)
    ie = rng.randint(0, 2)
    if rng.random() < 0.2:
        b = e = f32(0)
    else:
        b, e = f32(rng.uniform(-10, 10)), f32(rng.uniform(-10, 10))
    yb = [hx(v) for v in y]
    bb, eb = hx(b), hx(e)
    if k % 50 == 7 and n >= 2:
        # two NaN values of different payloads and a NaN beginning value
        i, j = rng.sample(range(n), 2)
        yb[i], yb[j] = "7fc00123", "ffc00456"
        bb = "7fc00789"
    if k % 77 == 11:
        yb[rng.randrange(n)] = rng.choice(["7f800000", "ff800000"])
    tb = [hx(v) for v in t]
    if nan_knot:
        tb[rng.randrange(n)] = rng.choice(["7fc00000", "ffc00000", "7fc00abc"])
    xs: list[str] = [hx(float(t[0]) - 1.0), "ff800000", "7f800000", "00000000", "80000000"]
    xs += ["7fc00000", "ffc00321"]
    xs += [hx(v) for v in t]
    xs.append(hx(float(t[-1]) + 1.0))
    for a, c in zip(t, t[1:]):
        xs.append(hx(float(a) + (float(c) - float(a)) * rng.random()))
    rng.shuffle(xs)
    xs = xs[: rng.randint(1, len(xs))]
    return " ".join([str(n), str(ib), bb, str(ie), eb, str(len(xs)), *tb, *yb, *xs])


def d3_line(rng: random.Random, k: int) -> str:
    n = rng.randint(1, 40)
    kind = k % 4
    a = []
    for _ in range(n):
        if kind == 0:  # diagonally dominant
            lo, up = rng.uniform(-1, 1), rng.uniform(-1, 1)
            a += [up, rng.choice([-1, 1]) * rng.uniform(2.5, 10), lo]
        elif kind == 1:  # random, not dominant
            a += [rng.uniform(-10, 10) for _ in range(3)]
        else:  # small integers: exact zero pivots during the elimination happen
            a += [float(rng.randint(-3, 3)), float(rng.choice([-3, -2, -1, 1, 2, 3])), float(rng.randint(-3, 3))]
    b = [rng.uniform(-100, 100) for _ in range(n)]
    ab = [hx(v) for v in a]
    if kind == 3:  # a zero diagonal (+0 or -0) before the elimination: refused
        ab[3 * rng.randrange(n) + 1] = rng.choice(["00000000", "80000000"])
    if k % 13 == 5:
        ab[3 * rng.randrange(n) + 1] = rng.choice(["7fc00001", "ffc00002"])
    return " ".join([str(n), *ab, *[hx(v) for v in b]])


SET1 = [
    ["0", "0", "0"],
    ["1", "0", "0", "0", "5"],
    ["3", "1", "1", "0", "1", "1", "5", "6", "7"],
    ["3", "1", "1", "0", "-1", "2", "5", "6", "7"],
    ["2", "1", "1", "-0", "0", "5", "6"],
    ["3", "3", "0", "0", "1", "2", "5", "6", "7"],
    ["3", "-1", "7", "0", "1", "2", "5", "6", "7"],
    ["3", "0", "5", "0", "1", "2", "5", "6", "7"],
    ["3", "2", "-2", "0", "1", "2", "5", "6", "7"],
    ["2", "1", "1", "0", "1.4e-45", "5", "6"],
    ["2", "1", "0", "0", "1.4e-45", "5", "6"],
    ["2", "0", "0", "0", "1.4e-45", "5", "6"],
    ["3", "1", "1", "0", "nan", "2", "5", "6", "7"],
    ["3", "1", "1", "inf", "nan", "2", "5", "6", "7"],
    ["3", "1", "1", "0", "1", "1e30", "5", "6", "7"],
    ["4", "2", "1", "-3.5", "0.25", "7", "1e6", "1", "-2", "3", "-4"],
]


class Context:
    def __init__(self, args: argparse.Namespace) -> None:
        workspace = Path(args.workspace)
        self.probe = workspace / "oracles/freesurfer-8.2.0-unit/probes/fs_spline_probe"
        self.cleanroom = Path(args.cleanroom)
        self.scratch = Path(args.scratch)
        self.env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
        self.policy = oracle_cache.Policy(threads=1, family="freesurfer")

    def original_stdin(self, command: str, text: str) -> oracle_cache.Completed:
        with tempfile.TemporaryDirectory(dir=self.scratch) as tmp:
            path = Path(tmp) / "input.txt"
            path.write_text(text)
            return oracle_cache.run(
                [str(RUNNER), str(self.probe), command, str(path)],
                env=self.env,
                policy=self.policy,
                runner="freesurfer-cleanroom-spline",
            )

    def original(self, args: list[str]) -> oracle_cache.Completed:
        return oracle_cache.run(
            [str(self.probe), *args], env=self.env, policy=self.policy, runner="freesurfer-cleanroom-spline"
        )

    def ours(self, args: list[str], text: str = "", reproduce: bool = False) -> subprocess.CompletedProcess:
        opts = ["--reproduce"] if reproduce else []
        return subprocess.run(
            [str(self.cleanroom), *opts, *args], input=text.encode(), capture_output=True, check=False
        )


def first_diff(a: bytes, b: bytes) -> str:
    for i, (x, y) in enumerate(zip(a.split(b"\n"), b.split(b"\n"))):
        if x != y:
            return f"line {i}: {x[:120]!r} vs {y[:120]!r}"
    return "lengths differ"


def batches(ctx: Context, command: str, lines: list[str], reproduce: bool) -> dict:
    same_out = same_status = 0
    bad = []
    out_lines = 0
    for i in range(0, len(lines), BATCH):
        text = "\n".join(lines[i : i + BATCH]) + "\n"
        o = ctx.original_stdin(command, text)
        c = ctx.ours([command], text, reproduce=reproduce)
        ok_out = o.stdout_bytes == c.stdout
        ok_st = o.returncode == c.returncode
        same_out += ok_out
        same_status += ok_st
        out_lines += o.stdout_bytes.count(b"\n")
        if not (ok_out and ok_st):
            bad.append({"batch": i // BATCH, "status": [o.returncode, c.returncode], "diff": first_diff(o.stdout_bytes, c.stdout)})
    n = (len(lines) + BATCH - 1) // BATCH
    return {
        "inputs": len(lines),
        "batches": n,
        "identical_batches": min(same_out, same_status),
        "output_lines": out_lines,
        "failures": bad[:5],
        "ok": not bad,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--workspace", default=os.environ.get("LARMORX_WORKSPACE", str(HERE.parents[3])))
    ap.add_argument("--cleanroom", required=True)
    ap.add_argument("--scratch", default=tempfile.gettempdir())
    ap.add_argument("--out", required=True)
    ap.add_argument("--cases", type=int, default=20000)
    ap.add_argument("--systems", type=int, default=1000)
    args = ap.parse_args()
    ctx = Context(args)
    rng = random.Random(20261113)
    result: dict = {"date": time.strftime("%Y-%m-%d"), "machine": os.uname().machine}
    status = 0

    t0 = time.perf_counter()
    lines = [case_line(rng, k) for k in range(args.cases)]
    r = batches(ctx, "cases", lines, reproduce=False)
    r["nan_value_cases"] = sum(1 for k in range(args.cases) if k % 50 == 7)
    r["infinite_value_cases"] = sum(1 for k in range(args.cases) if k % 77 == 11)
    r["evaluation_points"] = sum(int(line.split()[5]) for line in lines)
    r["seconds"] = round(time.perf_counter() - t0, 1)
    result["cases"] = r

    t0 = time.perf_counter()
    systems = [d3_line(rng, k) for k in range(args.systems)]
    r = batches(ctx, "d3", systems, reproduce=False)
    r["seconds"] = round(time.perf_counter() - t0, 1)
    result["d3"] = r

    rows = []
    for a in SET1:
        o = ctx.original(["set1", *a])
        c = ctx.ours(["set1", *a])
        rows.append(
            {
                "args": " ".join(a),
                "status": [o.returncode, c.returncode],
                "same": o.stdout_bytes == c.stdout and o.returncode == c.returncode,
                "stdout": o.stdout_bytes.decode(errors="replace"),
            }
        )
    # FS-103: the default refuses a NaN knot (E2) where FreeSurfer accepts it
    fix_rows = []
    for a in SET1:
        if "nan" in a:
            c = ctx.ours(["set1", *a])
            r_ = ctx.ours(["set1", *a], reproduce=True)
            o = ctx.original(["set1", *a])
            fix_rows.append(
                {
                    "args": " ".join(a),
                    "default_status": c.returncode,
                    "default_stdout": c.stdout.decode(),
                    "reproduce_same": r_.stdout == o.stdout_bytes and r_.returncode == o.returncode,
                }
            )
    result["set1"] = {
        "calls": len(rows),
        "identical": sum(r["same"] for r in rows),
        "rows": rows,
        "fs103_default": fix_rows,
        "ok": all(r["same"] for r in rows if "nan" not in r["args"].split())
        and all(f["default_status"] == 1 and "nan" in f["default_stdout"] and f["reproduce_same"] for f in fix_rows),
    }
    # the NaN-knot rows of SET1 compare the probe with the default above: they differ by design
    for r_ in rows:
        if "nan" in r_["args"].split():
            r_["note"] = "FS-103: compared under reproduce (fs103_default)"

    t0 = time.perf_counter()
    nan_lines = [case_line(rng, k, nan_knot=True) for k in range(500)]
    r = batches(ctx, "cases", nan_lines, reproduce=True)
    r["seconds"] = round(time.perf_counter() - t0, 1)
    result["fs103_reproduce"] = r
    # by default the first such case is refused with E2 (the run ends there)
    c = ctx.ours(["cases"], nan_lines[0] + "\n")
    result["fs103_default"] = {"status": c.returncode, "stdout_tail": c.stdout.decode()[-200:], "ok": c.returncode == 1}

    for name in ("cases", "d3", "set1", "fs103_reproduce", "fs103_default"):
        r = result[name]
        summary = {k: v for k, v in r.items() if isinstance(v, (int, float, str, bool))}
        print(f"{name}: {json.dumps(summary)}", flush=True)
        if not r["ok"]:
            status = 1
    Path(args.out).write_text(json.dumps(result, indent=1) + "\n")
    print(f"COMPARE-SPLINE exit {status}")
    return status


if __name__ == "__main__":
    sys.exit(main())
