# SPDX-License-Identifier: Apache-2.0
"""Speed of fs.maths' heavy functions, the clean room against the probe, black box (indicative).

The products (``MatrixMultiply``, ``MatrixMultiplyD``), the inverse (``MatrixInverse`` above 4
rows: VNL's QR in float), the SVD (``MatrixSVD``) and the histograms (``HISTOsmooth``,
``HISTOcount``, ``HISTOrobustGaussianFit``) run on large seeded arguments, the same batch files on
``fs_maths_probe`` and on the candidate, best of three wall times each. A baseline batch of as many
lines of a cheap command of the same shape (the same arguments parsed, about as much printed)
estimates each program's own costs (the probe forks a child for every line; the products'
baseline is a mismatched product of the same inputs, so their times include printing the result), so
``(batch - baseline) / lines`` approximates the time per call.

    scripts/orchestration/boxed.sh 4 python -P \\
        validation/investigations/freesurfer-cleanroom-maths/speed.py \\
        --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_maths_probe \\
        --candidate <the candidate> --scratch <scratch>/speed

Timing runs are never cached (they measure the programs); both run with one thread.
"""

from __future__ import annotations

import argparse
import os
import random
import struct
import subprocess
import time
from pathlib import Path


def f32(v: float) -> str:
    return struct.pack(">f", v).hex()


def mat(r: random.Random, rows: int, cols: int) -> str:
    return f"1 {rows} {cols} " + " ".join(f32(r.uniform(-4, 4)) for _ in range(rows * cols))


def hist(r: random.Random, n: int) -> str:
    bins = " ".join(f32(float(i)) for i in range(n))
    counts = " ".join(
        f32(float(int(1000 * (1 + r.random()) * 2.718 ** (-(((i - n / 2) / (n / 8)) ** 2)))))
        for i in range(n)
    )
    return f"{n} {n} {f32(1.0)} {f32(0.0)} {f32(float(n))} {bins} {counts}"


def batches(r: random.Random) -> dict[str, tuple[list[str], list[str]]]:
    """name: (lines, baseline lines)."""
    out: dict[str, tuple[list[str], list[str]]] = {}
    k = 40
    for n in (128, 256):
        # the baseline: the same inputs parsed and printed, a mismatched product (no arithmetic)
        a = [(mat(r, n, n), mat(r, n, n), mat(r, n + 1, n)) for _ in range(k)]
        out[f"mmul {n}x{n}"] = (
            [f"mmul {x} {y} new" for x, y, _ in a],
            [f"mmul {x} {z} new" for x, _, z in a],
        )
        out[f"mmuld {n}x{n}"] = (
            [f"mmuld {x} {y} new" for x, y, _ in a],
            [f"mmuld {x} {z} new" for x, _, z in a],
        )
    for n in (64, 128):
        a = [mat(r, n, n) for _ in range(k)]
        out[f"minverse {n}x{n}"] = ([f"minverse {x} new" for x in a], [f"mcopy {x} new" for x in a])
    for n in (64, 128):
        a = [mat(r, n, n) for _ in range(k)]
        w = "1 1 " + str(n) + " " + " ".join(f32(0.0) for _ in range(n))
        out[f"msvd {n}x{n}"] = (
            [f"msvd {x} {w} new" for x in a],
            [f"mcopy {x} new" for x in a],
        )
    h = [hist(r, 8192) for _ in range(k)]
    out["hsmooth 8192 bins"] = (
        [f"hsmooth {x} new {f32(8.0)}" for x in h],
        [f"hscale {x} new {f32(1.0)} {f32(0.0)}" for x in h],
    )
    samples = " ".join(struct.pack(">d", r.uniform(0, 8192)).hex() for _ in range(4000))
    out["hcount 4000 samples"] = (
        [f"hcount {x} 4000 {samples}" for x in h],
        [f"hsumnorm {x}" for x in h],
    )
    hr = [hist(r, 1024) for _ in range(k)]
    out["hrobust 1024 bins"] = (
        [f"hrobust {x} {struct.pack('>d', 0.5).hex()}" for x in hr],
        [f"hcopy {x} new" for x in hr],
    )
    return out


def wall(prog: str, path: Path, cwd: Path, env: dict[str, str]) -> float:
    best = float("inf")
    for _ in range(3):
        t = time.perf_counter()
        subprocess.run(
            [prog, "batch", str(path)], cwd=cwd, env=env, stdout=subprocess.DEVNULL, check=True
        )
        best = min(best, time.perf_counter() - t)
    return best


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("--probe", required=True)
    ap.add_argument("--candidate", required=True)
    ap.add_argument("--scratch", required=True, type=Path)
    ap.add_argument("--seed", type=int, default=7)
    a = ap.parse_args()
    a.scratch.mkdir(parents=True, exist_ok=True)
    env = {
        "PATH": "/usr/bin:/bin",
        "LC_ALL": "C",
        "OMP_NUM_THREADS": "1",
        "LARMORX_UPSTREAM_DEFECTS": "reproduce",
        "HOME": os.environ.get("HOME", "/"),
    }
    print(f"{'calls':24} {'probe ms/call':>14} {'clean room ms/call':>19} {'speed-up':>9}")
    for name, (lines, base) in batches(random.Random(a.seed)).items():
        tag = name.replace(" ", "_")
        bp, bb = a.scratch / f"{tag}.txt", a.scratch / f"{tag}.base.txt"
        bp.write_text("\n".join(f"@60 {x}" for x in lines) + "\n")
        bb.write_text("\n".join(f"@60 {x}" for x in base) + "\n")
        per = {}
        for who, prog in (("probe", a.probe), ("candidate", a.candidate)):
            per[who] = (
                max(wall(prog, bp, a.scratch, env) - wall(prog, bb, a.scratch, env), 0.0)
                / len(lines)
                * 1e3
            )
        ratio = per["probe"] / per["candidate"] if per["candidate"] > 0 else float("inf")
        print(f"{name:24} {per['probe']:14.3f} {per['candidate']:19.3f} {ratio:8.1f}x", flush=True)


if __name__ == "__main__":
    main()
