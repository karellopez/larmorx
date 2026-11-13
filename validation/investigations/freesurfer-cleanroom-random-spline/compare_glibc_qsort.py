# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx's clean room of the C library's ``qsort`` and ``qsort_r``
(``larmorx_core::glibc_qsort``, ``specs/glibc-qsort.md``) with glibc 2.35's own functions, run as
a black box through the probe ``glibc_qsort_probe`` (``validation/investigations/
glibc-qsort-probe/``: calls of the C library's functions only, built with ``cc -O2``): the
spec's validation plan, §9 groups 1 to 5 and its reference lines, and the speed of both.

Every case is a line ``s n mode seed keyrange api fail``; both programs print, per case, the
number of comparator calls, a hash of the call sequence, a hash of the sorted bytes, the requests
of the temporary array and whether ``errno`` was kept, or ``<calls> <hash> outside`` where glibc's
fallback would pass the comparator memory outside the array. The clean room runs through
``cleanroom-probe qsort`` twice: its typed functions (``[u8; s]`` elements) and its byte
functions (``--bytes``). The probe runs through the oracle-output cache (``oracle_cache.run``,
one thread; the cache passes no standard input, so through ``stdin-runner/run_stdin.sh``, the
input file an argument keyed by its content).

Usage (the README): ``python -P compare_glibc_qsort.py --probe <glibc_qsort_probe> --cleanroom
<cleanroom-probe> --bench <qsort_bench> --out <result.json> [--scratch <dir>]``. Prints one line per group and exits 1
when anything differs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from larmorx_validation import oracle_cache

HERE = Path(__file__).resolve().parent
RUNNER = HERE / "stdin-runner/run_stdin.sh"
BATCH = 4000

INT_KINDS = ["3way", "tie+", "tie-"]
SMALL_SIZES = [1, 2, 4, 8, 12, 16, 24, 32]
LARGE_SIZES = [33, 40, 72, 104, 160]
OTHER_SIZES = [5, 6, 7, 20, 28]

# the spec's reference lines (§9, glibc 2.35-0ubuntu3.15 through the probe)
REFERENCE = [
    ("4 1000 3way 1 10 qsort 0", "8504 22b3708447d84162 985da63e885ae5b7 1 1"),
    ("4 1000 3way 1 10 qsort 1", "9540 1b768952e1c84670 985da63e885ae5b7 1 1"),
    ("12 1000 3way 2 10 qsort 0", "8465 cd3c5a72a7733a84 1331cd34ebdbf954 1 1"),
    ("12 1000 tie+ 2 10 qsort 0", "8494 4bcd1b66e61a1fa1 3c92d2e1fd018aac 1 1"),
    ("16 1000 tie- 3 10 qsort 0", "8544 c4bcc9a6525d3b2e 31c543dbc89ca695 1 1"),
    ("40 1000 hash 4 3 qsort 0", "6916 f672409666b84a2b 25ec899552189d77 1 1"),
    ("40 1000 hash 4 3 qsort_r 0", "6916 f672409666b84a2b 25ec899552189d77 1 1"),
    ("40 1000 hash 4 3 qsort 1", "9621 2ea5d1118facf2b7 2b1e1211fa2aad7f 1 1"),
    ("4 1000 f3 5 12 qsort 0", "5997 a8107a0175eee3f5 563fcc1dcef7a859 1 1"),
    ("4 1000 f2 5 12 qsort 0", "6127 5669dc3121f908a8 62a8788fd4a06369 1 1"),
    ("8 1000 d3 6 12 qsort 0", "5879 8cc18a31990e2485 edf4496b7e528b61 1 1"),
    ("8 1000 d2 6 12 qsort 1", "9262 c90bb93c4a7bba71 d7f7923ba5e9ce51 1 1"),
    ("104 100 3way 7 5 qsort 0", "531 0461360aad912fdb 84af1b838617c27b 1 1"),
    ("1 255 3way 8 4 qsort 0", "1612 5923cd99459aacc0 d1fe3ddfe7376da5 0 1"),
    ("4 100000 3way 9 1000 qsort 0", "1536229 8bdb84d427f725aa cd915035a4583feb 1 1"),
]


def line(s: int, n: int, mode: str, seed: int, keyrange: int, api: str, fail: int) -> str:
    return f"{s} {n} {mode} {seed} {keyrange} {api} {fail}"


def kinds_for(s: int) -> list[str]:
    """The comparator kinds the probe takes for elements of ``s`` bytes."""
    k = list(INT_KINDS)
    if s >= 8:
        k.append("hash")
    if s == 4:
        k += ["f3", "f2"]
    if s == 8:
        k += ["d3", "d2"]
    return k


def group_small() -> list[str]:
    """§9 group 1: n = 0 … 64, every kind, sizes 1 to 32, three seeds, key ranges 1, 2, 3 and
    1000 (12 for the float kinds); without and with a refused allocation (the latter matters
    where the temporary array is at least 1024 bytes); and sizes 5, 6, 7, 20 and 28."""
    out = []
    for fail in (0, 1):
        for s in SMALL_SIZES:
            for mode in kinds_for(s):
                ranges = [1, 2, 3, 12] if mode[0] in "fd" else [1, 2, 3, 1000]
                for n in range(65):
                    for seed in (1, 2, 3):
                        for kr in ranges:
                            out.append(line(s, n, mode, seed, kr, "qsort", fail))
    # sizes the clean room's byte functions sort element by element (not as arrays)
    for fail in (0, 1):
        for s in OTHER_SIZES:
            for mode in kinds_for(s):
                for n in range(65):
                    for kr in (3, 1000):
                        out.append(line(s, n, mode, 4, kr, "qsort", fail))
    return out


def group_stack() -> list[str]:
    """§9 group 2: temporary arrays of 1000 to 1100 bytes (n · s, or 16 n + s above 32 bytes)."""
    out = []
    for s in SMALL_SIZES + LARGE_SIZES:
        for n in range(1, 1101):
            t = n * s if s <= 32 else 16 * n + s
            if 1000 <= t <= 1100:
                for mode in kinds_for(s):
                    kr = 12 if mode[0] in "fd" else 3
                    for fail in (0, 1):
                        out.append(line(s, n, mode, 11, kr, "qsort", fail))
    return out


LARGE_SET = [
    (4, "3way"),
    (12, "tie+"),
    (16, "tie-"),
    (8, "hash"),
    (4, "f3"),
    (8, "d2"),
    (40, "3way"),
]


def group_large() -> list[str]:
    """§9 group 3: n = 1000, 4096, 10,007, 100,000 and 1,000,000, key ranges 3 and 1000."""
    out = []
    for n in (1000, 4096, 10007, 100000, 1000000):
        for s, mode in LARGE_SET:
            for kr in (3, 1000):
                k = 12 if mode[0] in "fd" else kr
                out.append(line(s, n, mode, 21, k, "qsort", 0))
    return out


def group_fallback() -> list[str]:
    """§9 group 4: a refused allocation (fail 1) with a temporary array of 1024 bytes or more:
    n = 0 … 64 with 1024- and 1040-byte elements, floats and doubles with n = 128 … 300, and
    arrays up to 10,007 elements."""
    out = []
    for s in (1024, 1040):
        for mode in ["3way", "tie+", "tie-", "hash"]:
            for n in range(65):
                for seed in (1, 2, 3):
                    for kr in (3, 1000):
                        out.append(line(s, n, mode, seed, kr, "qsort", 1))
    for s, modes in ((4, ["f3", "f2"]), (8, ["d3", "d2"])):
        for mode in modes:
            for n in range(128, 301):
                for seed in (1, 2):
                    out.append(line(s, n, mode, seed, 12, "qsort", 1))
    for n in (1000, 4096, 10007):
        for s, mode in LARGE_SET:
            for kr in (3, 1000):
                k = 12 if mode[0] in "fd" else kr
                out.append(line(s, n, mode, 31, k, "qsort", 1))
    return out


def group_big_elements() -> list[str]:
    """§9 group 5: elements of 33 to 160 bytes (the pointer path), ``qsort`` and ``qsort_r``,
    with and without a refused allocation."""
    out = []
    for s in LARGE_SIZES:
        for mode in kinds_for(s):
            for n in [*range(65), 100, 1000]:
                for seed in (1, 2):
                    for api in ("qsort", "qsort_r"):
                        for fail in (0, 1):
                            out.append(line(s, n, mode, seed, 3, api, fail))
    return out


def stamp(*programs: Path) -> dict:
    commit = subprocess.run(
        ["git", "-C", str(HERE), "describe", "--always", "--dirty", "--abbrev=12"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    libc = subprocess.run(["ldd", "--version"], capture_output=True, text=True, check=False)
    return {
        "larmorx_commit": commit,
        "sha256": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in programs},
        "libc": libc.stdout.splitlines()[0] if libc.stdout else "",
    }


class Context:
    def __init__(self, args: argparse.Namespace) -> None:
        self.probe = Path(args.probe).resolve()
        self.cleanroom = Path(args.cleanroom).resolve()
        self.scratch = Path(args.scratch)
        self.env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C"}
        self.policy = oracle_cache.Policy(threads=1, family="freesurfer")
        self.runs = 0
        self.served = 0
        self.stops = [0, 0]

    def original(self, text: str) -> bytes:
        with tempfile.TemporaryDirectory(dir=self.scratch) as tmp:
            path = Path(tmp) / "cases.txt"
            path.write_text(text)
            o = oracle_cache.run(
                [str(RUNNER), str(self.probe), "qsort", str(path)],
                env=self.env,
                policy=self.policy,
                runner="freesurfer-cleanroom-glibc-qsort",
            )
        self.runs += 1
        self.served += o.cached
        if o.returncode != 0:
            raise RuntimeError(f"probe: exit {o.returncode}: {o.stderr_bytes!r}")
        return o.stdout_bytes

    def ours(self, text: str, *opts: str) -> bytes:
        p = subprocess.run(
            [str(self.cleanroom), "qsort", *opts],
            input=text.encode(),
            capture_output=True,
            check=True,
            timeout=3600,
        )
        if not opts:
            # the clean room's count of the two ways the fallback stops
            w = p.stderr.decode().split()
            self.stops[0] += int(w[1])
            self.stops[1] += int(w[5])
        return p.stdout


def compare(ctx: Context, cases: list[str]) -> dict:
    """Both programs on every case, in batches; per-line agreement of the typed and the byte
    functions with the probe."""
    stops0 = list(ctx.stops)
    same_typed = same_bytes = outside = requests = 0
    failures: list[dict] = []
    for i in range(0, len(cases), BATCH):
        chunk = cases[i : i + BATCH]
        text = "\n".join(chunk) + "\n"
        o = ctx.original(text).decode().splitlines()
        t = ctx.ours(text).decode().splitlines()
        b = ctx.ours(text, "--bytes").decode().splitlines()
        if not (len(o) == len(t) == len(b) == len(chunk)):
            failures.append({"batch": i // BATCH, "lines": [len(chunk), len(o), len(t), len(b)]})
            continue
        for case, lo, lt, lb in zip(chunk, o, t, b, strict=True):
            same_typed += lo == lt
            same_bytes += lo == lb
            outside += lo.endswith("outside")
            requests += (not lo.endswith("outside")) and lo.split()[3] == "1"
            if (lo != lt or lo != lb) and len(failures) < 10:
                failures.append({"case": case, "probe": lo, "typed": lt, "bytes": lb})
    return {
        "cases": len(cases),
        "identical_typed": same_typed,
        "identical_bytes": same_bytes,
        "outside": outside,
        "outside_at_a_position": ctx.stops[0] - stops0[0],
        "outside_at_a_reversed_range": ctx.stops[1] - stops0[1],
        "with_a_request": requests,
        "failures": failures,
        "ok": same_typed == same_bytes == len(cases) and not failures,
    }


def section_reference(ctx: Context) -> dict:
    cases = [c for c, _ in REFERENCE]
    text = "\n".join(cases) + "\n"
    o = ctx.original(text).decode().splitlines()
    t = ctx.ours(text).decode().splitlines()
    b = ctx.ours(text, "--bytes").decode().splitlines()
    rows = [
        {"case": c, "spec": w, "probe": po, "typed": pt, "bytes": pb}
        for (c, w), po, pt, pb in zip(REFERENCE, o, t, b, strict=True)
    ]
    return {
        "cases": len(rows),
        "identical": sum(r["spec"] == r["probe"] == r["typed"] == r["bytes"] for r in rows),
        "rows": rows,
        "ok": all(r["spec"] == r["probe"] == r["typed"] == r["bytes"] for r in rows),
    }


SPEED = [
    "4 1000000 3way 9 1000 qsort 0",
    "4 1000000 f3 9 12 qsort 0",
    "8 1000000 d2 9 12 qsort 0",
    "12 1000000 tie+ 9 1000 qsort 0",
    "40 1000000 hash 9 3 qsort 0",
    "104 100000 3way 9 1000 qsort 0",
    "4 10000000 3way 9 1000 qsort 0",
    "4 100000 3way 9 1000 qsort 1",
]


def section_speed(ctx: Context, bench: Path | None) -> dict:
    """Both programs on the same cases (the elements made, every call counted and hashed, the
    result hashed: the same work around the sort), outside the cache (it measures time), best
    of 5; then the sorts alone with a plain comparator, best of 5 sorts: glibc's ``qsort``
    (``qsort_bench.c``) against the clean room's typed and byte functions (``qsort-bench``),
    their arrangements' hashes checked equal."""
    rows = []
    with tempfile.TemporaryDirectory(dir=ctx.scratch) as tmp:
        for case in SPEED:
            path = Path(tmp) / "case.txt"
            path.write_text(case + "\n")

            def timed(argv: list[str], path: Path = path) -> float:
                best = float("inf")
                for _ in range(5):
                    with path.open("rb") as stdin:
                        t0 = time.perf_counter()
                        subprocess.run(argv, stdin=stdin, stdout=subprocess.DEVNULL, check=True)
                        best = min(best, time.perf_counter() - t0)
                return round(best, 4)

            row: dict = {
                "case": case,
                "probe_s": timed([str(ctx.probe)]),
                "cleanroom_s": timed([str(ctx.cleanroom), "qsort"]),
            }
            w = case.split()
            if w[6] == "0" and bench is not None:
                g = subprocess.run(
                    [str(bench), *w[:5], "5"], capture_output=True, text=True, check=True
                ).stdout.split()
                c = subprocess.run(
                    [str(ctx.cleanroom), "qsort-bench", *w[:5], "5"],
                    capture_output=True,
                    text=True,
                    check=True,
                ).stdout.split()
                row["glibc_sort_s"] = float(g[0])
                if "typed," in c:
                    row["cleanroom_typed_sort_s"] = float(c[0])
                row["cleanroom_bytes_sort_s"] = float(c[-4])
                row["same_arrangement"] = g[-1] == c[-1]
                row["glibc_over_typed"] = round(
                    row["glibc_sort_s"] / row.get("cleanroom_typed_sort_s", float(c[-4])), 2
                )
            rows.append(row)
    return {"rows": rows, "ok": all(r.get("same_arrangement", True) for r in rows)}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--probe", required=True, help="glibc_qsort_probe, built with cc -O2")
    ap.add_argument("--cleanroom", required=True, help="the built cleanroom-probe program")
    ap.add_argument("--bench", help="qsort_bench (beside this script), built with cc -O2")
    ap.add_argument("--scratch", default=tempfile.gettempdir())
    ap.add_argument("--out", required=True)
    ap.add_argument("--no-speed", action="store_true")
    args = ap.parse_args()
    ctx = Context(args)
    result: dict = {"date": time.strftime("%Y-%m-%d"), "machine": os.uname().machine}
    programs = [ctx.probe, ctx.cleanroom] + ([Path(args.bench)] if args.bench else [])
    result.update(stamp(*programs))
    status = 0
    groups = {
        "reference": None,
        "small": group_small,
        "stack_limit": group_stack,
        "large": group_large,
        "fallback": group_fallback,
        "big_elements": group_big_elements,
    }
    for name, make in groups.items():
        t0 = time.perf_counter()
        r = section_reference(ctx) if make is None else compare(ctx, make())
        r["seconds"] = round(time.perf_counter() - t0, 1)
        result[name] = r
    if not args.no_speed:
        result["speed"] = section_speed(ctx, Path(args.bench) if args.bench else None)
    result["original_runs"] = {"runs": ctx.runs, "from_the_cache": ctx.served}
    for name in [*groups, "speed"]:
        if name not in result:
            continue
        r = result[name]
        summary = {k: v for k, v in r.items() if isinstance(v, (int, float, str, bool))}
        if name == "speed":
            summary = {
                row["case"]: [
                    row.get(k)
                    for k in ("probe_s", "cleanroom_s", "glibc_sort_s", "cleanroom_typed_sort_s")
                ]
                for row in r["rows"]
            }
        print(f"{name}: {json.dumps(summary)}", flush=True)
        if not r["ok"]:
            status = 1
    Path(args.out).write_text(json.dumps(result, indent=1) + "\n")
    print(f"COMPARE-GLIBC-QSORT exit {status}")
    return status


if __name__ == "__main__":
    sys.exit(main())
