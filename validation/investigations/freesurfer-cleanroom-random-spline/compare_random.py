# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx's clean room of FreeSurfer 8.2.0's random numbers (``larmorx_core::ranlux``,
``specs/ranlux.md``) with FreeSurfer's own functions and programs, run as black boxes: the spec's
validation plan, §10.1 to §10.5, and its speed (§9).

- the probe ``fs_rng_probe`` (FreeSurfer's compiled functions, prebuilt by the orchestrator in
  ``<workspace>/oracles/freesurfer-8.2.0-unit/probes/``) and the official ``mri_volsynth`` run
  through the oracle-output cache (``oracle_cache.run``, one thread, the family ``freesurfer``);
- the clean room runs through ``cleanroom-probe`` (beside this script: the same commands), with
  larmorx's correctly rounded ``tan`` (its default) and, to show that the tangent is the only cause
  of a difference, with the C library's ``tan`` as a black box (``--system-tan``). Every draw of a
  large-shape gamma that met a tangent argument where the two tangents differ is counted apart
  (``--tan-hits``): those are the expected differences of the user's decision (GLIBC-07).

Usage (the README): ``python -P compare_random.py --cleanroom <cleanroom-probe> --out <result.json>
[--jobs 4] [--quick]``. Prints one line per section and exits 1 when anything unexpected differs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import struct
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from larmorx_validation import oracle_cache

HERE = Path(__file__).resolve().parent
TWO_32 = 1 << 32
TWO_63 = 1 << 63
TWO_64 = 1 << 64
ZERO_SEED = 2147483563
TIMEOUT = 5.0  # the probe's draws that never return (FS-100, FS-101) are stopped after it

# §2.5's table: the seeds, as fs_rng_probe takes them
TABLE_SEEDS = [
    "53",
    "0",
    "1",
    "2",
    "1234",
    "2147483562",
    "4294967295",
    "4294967296",
    str(TWO_63 - 1),
    str(TWO_63),
    "-53",
]
EXTRA_SEEDS = [
    "2147483564",
    str(TWO_64 - 1),
    "314159265",
    str(-5 * 2147483563),
    str(9 * 2147483563),
]
BOUNDARY_SEEDS = [
    str(2**31 - 1),
    str(-(2**31)),
    str(2**53 - 1),
    str(2**53 + 1),
    str(TWO_63 - 1),
    str(TWO_63 + 1),
]
# §2.6: seeds whose state is all zero (the probe's stream is 0 for ever): m * 2147483563 for
# m = 1 to 8 and -1 to -4 (observed for m = -12 to 12 with both, sh/multiples.sh; -5 and 9 are not)
ZERO_SEEDS = [str(m * ZERO_SEED) for m in (1, 2, 3, 4, 5, 6, 7, 8, -1, -2, -3, -4)]

NAN2 = "nan(0x123)"  # a NaN with another payload
DIST_CASES: list[tuple[str, list[str]]] = [
    ("uni", []),
    ("uni_pos", []),
    *[
        ("flat", list(p))
        for p in [
            ("0", "1"),
            ("-3.5", "7.25"),
            ("5", "5"),
            ("2", "-1"),
            ("-0", "-0"),
            ("nan", "1"),
            ("1", "nan"),
            ("nan", NAN2),
            ("inf", "1"),
            ("0", "inf"),
        ]
    ],
    *[("gaussian", [s]) for s in ["1", "2.5", "-1", "0", "1e-300", "1e300", "nan", "inf"]],
    ("ugauss", []),
    *[("exponential", [m]) for m in ["1", "0.75", "0", "-2", "nan"]],
    *[
        ("gamma", [a, b])
        for a in [
            "0",
            "1e-300",
            "0.001",
            "0.4",
            "0.5",
            "0.999",
            "1",
            "2",
            "2.5",
            "7.3",
            "11",
            "11.5",
            "12",
            "12.5",
            "25",
            "31.6",
            "1000.25",
            "1e6",
            "4294967295.5",
        ]
        for b in ["1", "2", "0.5", "-1"]
    ],
    *[("gamma_int", [n]) for n in ["0", "1", "5", "11", "12", "40"]],
    *[("gamma_large", [a]) for a in ["12", "12.5", "31.6", "1000.25"]],
    *[("gamma_frac", [f]) for f in ["1e-300", "0.001", "0.4", "0.999"]],
    *[("chisq", [n]) for n in ["0", "1", "3", "5", "24", "25", "30", "1000"]],
    *[("fdist", list(p)) for p in [("4", "9"), ("3", "25"), ("1", "1"), ("30", "50")]],
    *[("tdist", [n]) for n in ["0", "0.5", "1", "1.5", "2", "2.25", "3", "7", "30", "1000", "nan"]],
]
DIST_SEEDS = ["53", "1", "4294967296"]

# §8.1: shapes FreeSurfer converts wrongly; under reproduce the probe's values, bit for bit
FS100_VALUES = ["4294967296.5", "4294967308.25", "8589934600", "1e300", "nan", "inf"]
FS100_HANGS = ["-1.5", "-inf"]
# §8.2 / §2.6, under reproduce with the all-zero seeds: the draws that return, and those that do not
ZERO_RETURN = [
    ("uni", []),
    ("flat", ["3", "5"]),
    ("flat", ["-2.5", "inf"]),
    ("gamma", ["0", "1"]),
    ("gamma", ["12", "1"]),
    ("gamma", ["25", "0.5"]),
    ("gamma_large", ["30"]),
    ("chisq", ["0"]),
    ("chisq", ["24"]),
    ("fdist", ["24", "24"]),
]
ZERO_HANG = [
    ("uni_pos", []),
    ("gaussian", ["1"]),
    ("ugauss", []),
    ("exponential", ["1"]),
    ("gamma", ["0.4", "1"]),
    ("gamma", ["3", "1"]),
    ("gamma", ["12.5", "1"]),
    ("chisq", ["5"]),
    ("fdist", ["4", "9"]),
    ("tdist", ["1.5"]),
    ("tdist", ["7"]),
]
# §10.4: the official mri_volsynth: options, the clean room's draw, seed
VOLSYNTH = [
    (["--pdf", "z"], ("gaussian", ["1"]), 53),
    (["--pdf", "z"], ("gaussian", ["1"]), 1234),
    (["--pdf", "chi2", "--dof", "30"], ("chisq", ["30"]), 7),
    (["--pdf", "chi2", "--dof", "3"], ("chisq", ["3"]), 7),
    (["--pdf", "chi2", "--dof", "24"], ("chisq", ["24"]), 5),
    (["--pdf", "t", "--dof", "9"], ("tdist", ["9"]), 7),
    (["--pdf", "t", "--dof", "2"], ("tdist", ["2"]), 13),
    (["--pdf", "t", "--dof", "1"], ("tdist", ["1"]), 3),
    (["--pdf", "F", "--dof-num", "3", "--dof-den", "25"], ("fdist", ["3", "25"]), 11),
    (["--pdf", "F", "--dof-num", "30", "--dof-den", "50"], ("fdist", ["30", "50"]), 2),
]
VOLSYNTH_DIM = ["8", "7", "6", "3"]


def stamp(*programs: Path) -> dict:
    """The larmorx commit (``-dirty`` when the tree is not clean) and the programs' SHA-256."""
    commit = subprocess.run(
        ["git", "-C", str(HERE), "describe", "--always", "--dirty", "--abbrev=12"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    sums = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in programs}
    return {"larmorx_commit": commit, "sha256": sums}


class Context:
    def __init__(self, args: argparse.Namespace) -> None:
        self.workspace = Path(args.workspace)
        self.probe = self.workspace / "oracles/freesurfer-8.2.0-unit/probes/fs_rng_probe"
        self.fs_home = self.workspace / "oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0"
        self.license = Path(args.fs_license)
        self.cleanroom = Path(args.cleanroom)
        self.scratch = Path(args.scratch)
        self.env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OMP_NUM_THREADS": "1"}
        self.policy = oracle_cache.Policy(threads=1, family="freesurfer")

    def original(self, args: list[str], timeout: float | None = None) -> oracle_cache.Completed:
        """The probe, through the oracle-output cache."""
        return oracle_cache.run(
            [str(self.probe), *args],
            env=self.env,
            policy=self.policy,
            timeout=timeout,
            runner="freesurfer-cleanroom-random",
        )

    def ours(
        self, args: list[str], *, opts: tuple[str, ...] = (), hits: bool = False
    ) -> tuple[int, bytes, bytes, list[int]]:
        """The clean room: (exit status, standard output, standard error, the draws that met a
        differing tangent)."""
        with tempfile.TemporaryDirectory(dir=self.scratch) as tmp:
            extra: list[str] = []
            path = Path(tmp) / "hits.txt"
            if hits:
                extra = ["--tan-hits", str(path)]
            p = subprocess.run(
                [str(self.cleanroom), *opts, *extra, *args],
                capture_output=True,
                timeout=600,
                check=False,
            )
            got = [int(x) for x in path.read_text().split()] if hits and path.exists() else []
        return p.returncode, p.stdout, p.stderr, got


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def first_difference(a: bytes, b: bytes) -> int | None:
    la, lb = a.split(b"\n"), b.split(b"\n")
    for i, (x, y) in enumerate(zip(la, lb, strict=False)):
        if x != y:
            return i
    return None if len(la) == len(lb) else min(len(la), len(lb))


def random_seeds(n: int) -> tuple[list[str], list[str]]:
    rng = random.Random(20261113)
    wide = [str(rng.randrange(1, TWO_64)) for _ in range(n)]
    narrow = []
    while len(narrow) < n:
        v = rng.randrange(-(2**31), 2**31)
        if v != 0:
            narrow.append(str(v))
    return wide, narrow


def section_raw(ctx: Context, jobs: int, n_random: int) -> dict:
    """§10.1: the first 100,000 values of each seed, the probe against the clean room."""
    wide, narrow = random_seeds(n_random)
    seeds = TABLE_SEEDS + EXTRA_SEEDS + BOUNDARY_SEEDS + wide + narrow
    count = "100000"

    def one(seed: str) -> dict:
        o = ctx.original(["raw", seed, count])
        st, out, _, _ = ctx.ours(["raw", seed, count])
        same = o.returncode == 0 and st == 0 and o.stdout_bytes == out
        return {
            "seed": seed,
            "same": same,
            "sha256": sha(o.stdout_bytes),
            "first_difference": None if same else first_difference(o.stdout_bytes, out),
        }

    with ThreadPoolExecutor(jobs) as pool:
        rows = list(pool.map(one, seeds))

    # the all-zero seeds: under reproduce the probe's stream (0 for ever); by default §8.2's
    zero_rows = []
    corrected_head = [16777215] * 10 + [16777214, 16777215]
    for seed in ZERO_SEEDS:
        o = ctx.original(["raw", seed, count])
        st, out, err, _ = ctx.ours(["raw", seed, count], opts=("--reproduce",))
        all_zero = set(o.stdout_bytes.split()) == {b"0"}
        _, outf, errf, _ = ctx.ours(["raw", seed, count])
        head = [int(x) for x in outf.split()[:12]]
        zero_rows.append(
            {
                "seed": seed,
                "probe_all_zero": all_zero,
                "reproduce_same": o.stdout_bytes == out and st == 0,
                "reproduce_notice": b"FS-101" in err,
                "fix_head_is_spec": head == corrected_head,
                "fix_notice": b"FS-101" in errf and b"corrected" in errf,
                "fix_nonzero": set(outf.split()) != {b"0"},
            }
        )
    ok_zero = all(all(v for k, v in r.items() if k != "seed") for r in zero_rows)
    same = sum(r["same"] for r in rows)
    return {
        "seeds": len(rows),
        "identical": same,
        "different": [r for r in rows if not r["same"]],
        "table_hashes": {r["seed"]: r["sha256"] for r in rows if r["seed"] in TABLE_SEEDS},
        "zero_seeds": zero_rows,
        "ok": same == len(rows) and ok_zero,
    }


def compare_draws(ctx: Context, args: list[str], opts: tuple[str, ...] = ()) -> dict:
    """One distribution case: the probe's draws against the clean room's, with both tangents."""
    o = ctx.original(args)
    st, out, _, hits = ctx.ours(args, opts=opts, hits=True)
    st_s, out_s, _, _ = ctx.ours(args, opts=("--system-tan", *opts))
    a, b = o.stdout_bytes.split(), out.split()
    diff = [i for i, (x, y) in enumerate(zip(a, b, strict=False)) if x != y]
    if len(a) != len(b):
        diff.append(min(len(a), len(b)))
    hitset = set(hits)
    expected = [i for i in diff if i in hitset]
    return {
        "case": " ".join(args),
        "draws": len(a),
        "identical": len(a) - len(diff),
        "tan_draws": len(hits),
        "expected_tan_differences": len(expected),
        "unexpected": [i for i in diff if i not in hitset][:10],
        "system_tan_identical": o.stdout_bytes == out_s and st_s == st == o.returncode,
        "status": [o.returncode, st],
    }


def section_dist(ctx: Context, jobs: int, count: str) -> dict:
    """§10.2: 20,000 draws per case and seed, bit for bit; and §10.2's mixed stream."""
    jobs_list = [
        ["dist", name, seed, count, *params] for seed in DIST_SEEDS for name, params in DIST_CASES
    ]
    jobs_list += [["mixed", seed, "14000"] for seed in DIST_SEEDS]
    # Student's t just above 2 (§4.8): slow, 3 draws
    jobs_list += [["dist", "tdist", "53", "3", "0x1.0000000000001p+1"]]
    with ThreadPoolExecutor(jobs) as pool:
        rows = list(pool.map(lambda a: compare_draws(ctx, a), jobs_list))
    draws = sum(r["draws"] for r in rows)
    ident = sum(r["identical"] for r in rows)
    expected = sum(r["expected_tan_differences"] for r in rows)
    bad = [
        r
        for r in rows
        if r["unexpected"] or not r["system_tan_identical"] or r["status"][0] != r["status"][1]
    ]
    return {
        "cases": len(rows),
        "draws": draws,
        "identical": ident,
        "expected_tan_differences": expected,
        "draws_meeting_a_differing_tangent": sum(r["tan_draws"] for r in rows),
        "failures": bad,
        "with_tan": [r for r in rows if r["tan_draws"]],
        "per_case": [[r["case"], r["draws"], r["identical"], r["tan_draws"]] for r in rows],
        "ok": not bad and ident + expected == draws,
    }


def section_defects(ctx: Context) -> dict:
    """§10.5: FS-100 and FS-101 under reproduce against the probe; their errors; the defaults."""
    rows = []
    for a in FS100_VALUES:
        for seed in DIST_SEEDS:
            r = compare_draws(ctx, ["dist", "gamma", seed, "2000", a, "1"], opts=("--reproduce",))
            st, _, err, _ = ctx.ours(["dist", "gamma", seed, "3", a, "1"])
            r["default"] = {"status": st, "message": err.decode().strip()}
            rows.append(r)
    hangs = []
    for a in FS100_HANGS:
        o = ctx.original(["dist", "gamma", "53", "1", a, "1"], timeout=TIMEOUT)
        st_r, _, err_r, _ = ctx.ours(["dist", "gamma", "53", "1", a, "1"], opts=("--reproduce",))
        st_f, _, err_f, _ = ctx.ours(["dist", "gamma", "53", "1", a, "1"])
        hangs.append(
            {
                "shape": a,
                "probe_timed_out": o.timed_out,
                "reproduce": [st_r, err_r.decode().strip()],
                "fix": [st_f, err_f.decode().strip()],
            }
        )
    zero = []
    for seed in (str(ZERO_SEED), str(-ZERO_SEED)):
        for name, params in ZERO_RETURN:
            r = compare_draws(ctx, ["dist", name, seed, "100", *params], opts=("--reproduce",))
            zero.append(r)
        for name, params in ZERO_HANG:
            o = ctx.original(["dist", name, seed, "1", *params], timeout=TIMEOUT)
            st, _, err, _ = ctx.ours(["dist", name, seed, "1", *params], opts=("--reproduce",))
            zero.append(
                {
                    "case": f"dist {name} {seed} 1 {' '.join(params)}",
                    "probe_timed_out": o.timed_out,
                    "reproduce": [st, err.decode().strip().splitlines()[-1]],
                }
            )
    ok = (
        all(
            not r["unexpected"] and r["system_tan_identical"] and r["default"]["status"] in (0, 1)
            for r in rows
        )
        and all(h["probe_timed_out"] and h["reproduce"][0] == 1 and h["fix"][0] == 1 for h in hangs)
        and all(
            ("unexpected" in z and not z["unexpected"] and z["system_tan_identical"])
            or ("probe_timed_out" in z and z["probe_timed_out"] and z["reproduce"][0] == 1)
            for z in zero
        )
    )
    return {"fs100": rows, "fs100_hangs": hangs, "fs101": zero, "ok": ok}


def section_volsynth(ctx: Context) -> dict:
    """§10.4: the official mri_volsynth's float32 voxels against the clean room's draws, taken in
    FreeSurfer's field order (column outermost, then row, slice, frame)."""
    import nibabel as nib
    import numpy as np

    env = dict(
        ctx.env,
        FREESURFER_HOME=str(ctx.fs_home),
        FS_LICENSE=str(ctx.license),
    )
    rows = []
    w, h, d, f = (int(x) for x in VOLSYNTH_DIM)
    n = w * h * d * f
    for opts, (name, params), seed in VOLSYNTH:
        with tempfile.TemporaryDirectory(dir=ctx.scratch) as tmp:
            vol = Path(tmp) / "out.mgz"
            o = oracle_cache.run(
                [
                    str(ctx.fs_home / "bin/mri_volsynth"),
                    *opts,
                    "--dim",
                    *VOLSYNTH_DIM,
                    "--seed",
                    str(seed),
                    "--vol",
                    str(vol),
                ],
                env=env,
                cwd=Path(tmp),
                outputs=[Path(tmp)],
                policy=oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer"),
                runner="freesurfer-cleanroom-random",
            )
            img = np.asarray(nib.load(vol).dataobj) if o.returncode == 0 else None
        res = {}
        for label, extra in (("cr_tan", ()), ("system_tan", ("--system-tan",))):
            _, out, _, hits = ctx.ours(
                ["dist", name, str(seed), str(n), *params], opts=extra, hits=True
            )
            draws = [struct.unpack(">d", bytes.fromhex(x.decode()))[0] for x in out.split()]
            ours = np.empty((w, h, d, f), dtype=np.float32)
            it = iter(draws)
            for c in range(w):
                for r in range(h):
                    for s in range(d):
                        for fr in range(f):
                            v = next(it)
                            ours[c, r, s, fr] = np.float32(v + 0.0 if name == "gaussian" else v)
            same = (
                0
                if img is None
                else int(np.sum(ours.view(np.uint32) == img.astype(np.float32).view(np.uint32)))
            )
            res[label] = {"identical": same, "tan_draws": len(hits)}
        rows.append(
            {
                "command": " ".join([*opts, "--seed", str(seed)]),
                "status": o.returncode,
                "voxels": n,
                **res,
            }
        )
    ok = all(r["status"] == 0 and r["system_tan"]["identical"] == r["voxels"] for r in rows)
    return {"cases": rows, "ok": ok}


def section_speed(ctx: Context) -> dict:
    """§9: 20 million integers, printed (as the probe prints them) and not printed."""

    def timed(argv: list[str]) -> float:
        best = None
        for _ in range(3):
            t0 = time.perf_counter()
            subprocess.run(argv, stdout=subprocess.DEVNULL, check=True)
            dt = time.perf_counter() - t0
            best = dt if best is None else min(best, dt)
        return best

    probe = timed([str(ctx.probe), "raw", "53", "20000000"])
    ours = timed([str(ctx.cleanroom), "raw", "53", "20000000"])
    bench = subprocess.run(
        [str(ctx.cleanroom), "bench", "53", "20000000"], capture_output=True, text=True, check=True
    ).stdout.strip()
    return {
        "probe_raw_20M_printed_s": round(probe, 3),
        "cleanroom_raw_20M_printed_s": round(ours, 3),
        "cleanroom_generation_only": bench,
        "ok": True,
    }


def section_tan(ctx: Context) -> dict:
    """§10.3: the arguments where the C library's tan differs from the correctly rounded one."""
    p = subprocess.run([str(ctx.cleanroom), "tan-list"], capture_output=True, text=True, check=True)
    ks = [int(x) for x in p.stdout.split()]
    low = sum(k < 2**23 for k in ks)
    return {
        "differing_arguments": len(ks),
        "below_2_23": low,
        "sha256_of_list": sha(p.stdout.encode()),
        "ok": len(ks) == 41708 and low == 20360,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument(
        "--workspace", default=os.environ.get("LARMORX_WORKSPACE", str(HERE.parents[3]))
    )
    ap.add_argument("--fs-license", default="/usr/local/freesurfer/license.txt")
    ap.add_argument("--cleanroom", required=True, help="the built cleanroom-probe program")
    ap.add_argument("--scratch", default=tempfile.gettempdir())
    ap.add_argument("--out", required=True, help="the result's JSON file")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--quick", action="store_true", help="20 random seeds and 2,000 draws per case")
    ap.add_argument("--sections", default="raw,dist,defects,volsynth,tan,speed")
    args = ap.parse_args()
    ctx = Context(args)
    sections = args.sections.split(",")
    result: dict = {"date": time.strftime("%Y-%m-%d"), "machine": os.uname().machine}
    result.update(stamp(ctx.probe, ctx.cleanroom, ctx.fs_home / "bin/mri_volsynth"))
    status = 0
    for name in sections:
        t0 = time.perf_counter()
        if name == "raw":
            r = section_raw(ctx, args.jobs, 20 if args.quick else 1000)
        elif name == "dist":
            r = section_dist(ctx, args.jobs, "2000" if args.quick else "20000")
        elif name == "defects":
            r = section_defects(ctx)
        elif name == "volsynth":
            r = section_volsynth(ctx)
        elif name == "tan":
            r = section_tan(ctx)
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
    Path(args.out).write_text(json.dumps(result, indent=1) + "\n")
    print(f"COMPARE-RANDOM exit {status}")
    return status


if __name__ == "__main__":
    sys.exit(main())
