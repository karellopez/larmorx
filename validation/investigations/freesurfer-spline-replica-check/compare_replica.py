# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx's replica of FreeSurfer 8.2.0's cubic spline (``fs.spline`` in
``crates-freesurfer/larmorx-freesurfer-core``: John Burkardt's 2006 routines as FreeSurfer changed
them) with FreeSurfer's own functions and with larmorx's clean room (``larmorx_core::cubic_spline``):

- ``freesurfer``: ``specs/burkardt-spline.md`` §9.1 to §9.3 with the replica in the clean room's
  place: the clean room's ``compare_spline.py`` (beside the clean room's validation,
  ``validation/investigations/freesurfer-cleanroom-random-spline/``), run on the replica's probe
  ``fs_spline_replica_probe`` (the crate's example, the same commands): 20,000 seeded cases, 1,000
  tridiagonal systems, the five messages, FS-103 in both modes;
- ``nan_order``: tridiagonal systems whose super-diagonal entries and right-hand sides are NaNs of
  different payloads, so that the products of the elimination meet two NaNs (which one passes on
  is decided by the official build's operand order): the replica against ``fs_spline_probe``, and the
  clean room's count of differences;
- ``cleanroom``: the replica against the clean room's ``cleanroom-probe`` on every input above, in
  both modes: exit status and standard output byte for byte.

FreeSurfer's probe runs through the oracle-output cache (one thread, the family ``freesurfer``; the
cache passes no standard input, so through ``stdin-runner/run_stdin.sh``). Usage (README):
``python -P compare_replica.py --replica <fs_spline_replica_probe> --cleanroom <cleanroom-probe>
--out <result.json>``. Exits 1 when anything unexpected differs.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import random
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
CLEANROOM_DIR = HERE.parent / "freesurfer-cleanroom-random-spline"


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, CLEANROOM_DIR / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


cs = load("compare_spline")

NAN_PAYLOADS = ["7fc00111", "ffc00222", "7fc00333", "7fa00444", "ffc00555", "7fc00666"]


def nan_order_lines(rng: random.Random, count: int) -> list[str]:
    """Systems with NaNs of different payloads in the super-diagonal, the sub-diagonal and the
    right-hand side (the diagonal kept away from 0, so none is refused)."""
    lines = []
    for _ in range(count):
        n = rng.randint(2, 12)
        a = []
        for _ in range(n):
            a += [rng.uniform(-3, 3), rng.choice([-1, 1]) * rng.uniform(1, 8), rng.uniform(-3, 3)]
        b = [rng.uniform(-50, 50) for _ in range(n)]
        ab = [cs.hx(v) for v in a]
        bb = [cs.hx(v) for v in b]
        for _ in range(rng.randint(1, 4)):
            where = rng.choice(["u", "l", "b", "d"])
            j = rng.randrange(n)
            p = rng.choice(NAN_PAYLOADS)
            if where == "u":
                ab[3 * j] = p
            elif where == "l":
                ab[3 * j + 2] = p
            elif where == "d":
                ab[3 * j + 1] = p
            else:
                bb[j] = p
        lines.append(" ".join([str(n), *ab, *bb]))
    return lines


def run(argv: list[str], text: str = "") -> tuple[int, bytes]:
    p = subprocess.run(argv, input=text.encode(), capture_output=True, check=False, timeout=600)
    return p.returncode, p.stdout


def stamp(**programs: Path) -> dict:
    commit = subprocess.run(
        ["git", "-C", str(HERE), "describe", "--always", "--dirty", "--abbrev=12"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    sums = {k: hashlib.sha256(p.read_bytes()).hexdigest() for k, p in programs.items()}
    return {"larmorx_commit": commit, "sha256": sums}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument(
        "--workspace", default=os.environ.get("LARMORX_WORKSPACE", str(HERE.parents[3]))
    )
    ap.add_argument("--replica", required=True)
    ap.add_argument("--cleanroom", required=True)
    ap.add_argument("--scratch", default=tempfile.gettempdir())
    ap.add_argument("--out", required=True)
    ap.add_argument("--cases", type=int, default=20000)
    ap.add_argument("--systems", type=int, default=1000)
    args = ap.parse_args()
    replica, cleanroom = Path(args.replica), Path(args.cleanroom)
    ctx = cs.Context(
        argparse.Namespace(workspace=args.workspace, cleanroom=args.replica, scratch=args.scratch)
    )
    result: dict = {"date": time.strftime("%Y-%m-%d"), "machine": os.uname().machine}
    result.update(stamp(probe=ctx.probe, replica=replica, cleanroom=cleanroom))
    status = 0

    # freesurfer: the clean room's comparison, with the replica in its place
    t0 = time.perf_counter()
    with tempfile.TemporaryDirectory(dir=args.scratch) as tmp:
        out = Path(tmp) / "spline.json"
        p = subprocess.run(
            [
                sys.executable,
                "-P",
                str(CLEANROOM_DIR / "compare_spline.py"),
                "--workspace",
                args.workspace,
                "--cleanroom",
                str(replica),
                "--scratch",
                args.scratch,
                "--out",
                str(out),
                "--cases",
                str(args.cases),
                "--systems",
                str(args.systems),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        fs = json.loads(out.read_text()) if out.exists() else {}
    fs["exit"] = p.returncode
    fs["log"] = p.stdout.strip().splitlines()
    fs["ok"] = p.returncode == 0
    fs["seconds"] = round(time.perf_counter() - t0, 1)
    result["freesurfer"] = fs

    # nan_order: the operand order where two NaNs meet in the elimination
    rng = random.Random(20261113)
    systems = [
        "2 00000000 40000000 40400000 7fc00111 40a00000 00000000 40e00000 7fc00222",
        "2 00000000 40000000 7fc00333 7fc00111 40a00000 00000000 40e00000 41000000",
        *nan_order_lines(rng, 300),
    ]
    text = "\n".join(systems) + "\n"
    o = ctx.original_stdin("d3", text)
    r_st, r_out = run([str(replica), "d3"], text)
    c_st, c_out = run([str(cleanroom), "d3"], text)
    ol, rl, cl = (x.split(b"\n") for x in (o.stdout_bytes, r_out, c_out))
    result["nan_order"] = {
        "systems": len(systems),
        "replica_identical": sum(a == b for a, b in zip(ol, rl, strict=False)) - 1,
        "cleanroom_identical": sum(a == b for a, b in zip(ol, cl, strict=False)) - 1,
        "status": [o.returncode, r_st, c_st],
        "ok": o.stdout_bytes == r_out and o.returncode == r_st == 0,
    }

    # cleanroom: the replica against the clean room on the same inputs, both modes
    rng = random.Random(20261113)
    case_lines = [cs.case_line(rng, k) for k in range(args.cases)]
    d3_lines = [cs.d3_line(rng, k) for k in range(args.systems)]
    nan_knot = [cs.case_line(rng, k, nan_knot=True) for k in range(500)]
    rows = []
    for mode in ([], ["--reproduce"]):
        for name, cmd, lines in (
            ("cases", "cases", case_lines),
            ("d3", "d3", d3_lines),
            ("nan_knot_cases", "cases", nan_knot),
        ):
            for i in range(0, len(lines), cs.BATCH):
                text = "\n".join(lines[i : i + cs.BATCH]) + "\n"
                a = run([str(replica), *mode, cmd], text)
                b = run([str(cleanroom), *mode, cmd], text)
                rows.append({"what": f"{' '.join(mode)} {name} {i // cs.BATCH}", "same": a == b})
        for s in cs.SET1:
            a = run([str(replica), *mode, "set1", *s])
            b = run([str(cleanroom), *mode, "set1", *s])
            rows.append({"what": f"{' '.join(mode)} set1 {' '.join(s)}", "same": a == b})
    diff = [r["what"] for r in rows if not r["same"]]
    result["cleanroom"] = {
        "runs": len(rows),
        "identical": len(rows) - len(diff),
        "different": diff,
        "ok": not diff,
    }

    for name in ("freesurfer", "nan_order", "cleanroom"):
        r = result[name]
        summary = {k: v for k, v in r.items() if isinstance(v, (int, float, str, bool))}
        print(f"{name}: {json.dumps(summary)}", flush=True)
        if not r["ok"]:
            status = 1
    for line in result["freesurfer"].get("log", []):
        print(f"  freesurfer | {line}")
    Path(args.out).write_text(json.dumps(result, indent=1) + "\n")
    print(f"COMPARE-SPLINE-REPLICA exit {status}")
    return status


if __name__ == "__main__":
    sys.exit(main())
