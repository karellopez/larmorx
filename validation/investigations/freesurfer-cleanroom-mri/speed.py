# SPDX-License-Identifier: Apache-2.0
"""Speed of fs.mri's heavy functions, the clean room against the probe, black box (indicative).

The bounding box (``MRIboundingBox``: a scan of the first frame, 7 integers printed) of large
volumes and the old tkregister's fix (``MRIfixTkReg``: 3,000 draws and products of 4 x 1,000
matrices) run on the same batch files on ``fs_mri_probe`` and on the candidate, best of three
wall times each. A baseline batch of as many lines of the same shape estimates each program's own
costs (the probe forks a child for every line; both allocate and fill the volume): the bounding
box of a volume of the same bytes in a type it refuses (RGB: no scan), and for the fix the
tkregister-to-native conversion of the same arguments; ``(batch - baseline) / lines``
approximates the time per call.

    scripts/orchestration/boxed.sh 4 python -P \\
        validation/investigations/freesurfer-cleanroom-mri/speed.py \\
        --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_mri_probe \\
        --candidate <the candidate> --scratch <scratch>/speed

Timing runs are never cached (they measure the programs); both run with one thread.
"""

from __future__ import annotations

import argparse
import random
import struct
import subprocess
import time
from pathlib import Path


def f32(v: float) -> str:
    return struct.pack(">f", v).hex()


def geom(r: random.Random) -> str:
    c = " ".join(f32(r.uniform(-50, 50)) for _ in range(3))
    return (
        f"1 64 64 48 {f32(1.0)} {f32(1.0)} {f32(1.25)} 1 "
        f"{f32(-1.0)} {f32(0.0)} {f32(0.0)} {f32(0.0)} {f32(0.0)} {f32(-1.0)} "
        f"{f32(0.0)} {f32(1.0)} {f32(0.0)} {c} {f32(0.0)} {f32(0.0)} {f32(0.0)} x"
    )


def mat4(r: random.Random) -> str:
    vals = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
    vals = [v + r.uniform(-0.05, 0.05) if i % 4 != 3 or i < 12 else v for i, v in enumerate(vals)]
    return "4 4 " + " ".join(f32(v) for v in vals)


def batches(r: random.Random) -> dict[str, tuple[list[str], list[str]]]:
    """name: (lines, baseline lines)."""
    out: dict[str, tuple[list[str], list[str]]] = {}
    k = 5
    for kind, name in ((3, "float"), (1, "int")):
        n = 160
        lines, base = [], []
        for _ in range(k):
            seed = r.randrange(256)
            sets = " ".join(
                f"{r.randrange(n)} {r.randrange(n)} {r.randrange(n)} 0 {f32(1000.0)}"
                for _ in range(3)
            )
            lines.append(f"@60 bbox {n} {n} {n} {kind} 1 0 {seed} 3 {sets} 70")
            base.append(f"@60 bbox {n} {n} {n} 9 1 0 {seed} 3 {sets} 70")
        out[f"bbox {name} {n}^3"] = (lines, base)
    lines, base = [], []
    for _ in range(4 * k):
        g = geom(r)
        m = mat4(r)
        lines.append(f"@60 fixtk 2 2 2 0 1 1 {g} {r.randrange(1000)} {m}")
        base.append(f"@60 tk2nat 2 2 2 0 1 1 {g} 2 2 2 0 1 1 {g} {m}")
    out["fixtk"] = (lines, base)
    return out


def best(argv: list[str], env: dict[str, str], cwd: Path) -> float:
    times = []
    for _ in range(3):
        t = time.perf_counter()
        subprocess.run(argv, env=env, cwd=cwd, capture_output=True, check=True)
        times.append(time.perf_counter() - t)
    return min(times)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--probe", type=Path, required=True)
    ap.add_argument("--candidate", type=Path, required=True)
    ap.add_argument("--scratch", type=Path, required=True)
    ap.add_argument("--seed", type=int, default=7)
    a = ap.parse_args()
    a.scratch.mkdir(parents=True, exist_ok=True)
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OMP_NUM_THREADS": "1"}
    env_c = dict(env, LARMORX_UPSTREAM_DEFECTS="reproduce")
    r = random.Random(a.seed)
    print(f"{'case':22s} {'probe ms/call':>14s} {'clean room ms/call':>19s} {'ratio':>7s}")
    for name, (lines, base) in batches(r).items():
        f = a.scratch / (name.replace(" ", "_").replace("^", "") + ".txt")
        fb = a.scratch / (name.replace(" ", "_").replace("^", "") + ".base.txt")
        f.write_text("\n".join(lines) + "\n")
        fb.write_text("\n".join(base) + "\n")
        res = []
        for prog, e in ((a.probe, env), (a.candidate, env_c)):
            t = best([str(prog), "batch", str(f)], e, a.scratch)
            tb = best([str(prog), "batch", str(fb)], e, a.scratch)
            res.append(max(t - tb, 0.0) / len(lines) * 1000)
        ratio = res[0] / res[1] if res[1] > 0 else float("inf")
        print(f"{name:22s} {res[0]:14.3f} {res[1]:19.3f} {ratio:7.1f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
