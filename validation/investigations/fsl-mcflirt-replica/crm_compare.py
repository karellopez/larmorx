# SPDX-License-Identifier: Apache-2.0
"""Run a recorded mcflirt oracle case again with the CORE-MATH preload and compare its outputs
with the as-built oracle's and with the replica's (a replay directory kept by ``replay.py
--keep``). This showed that the replica with correctly rounded maths gave mcflirt's bytes
whenever mcflirt itself ran with correctly rounded maths.

Usage: python crm_compare.py <case> [replay-dir]
"""

import gzip
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np


def _workspace() -> Path:
    """The workspace that holds oracles/ (LARMORX_WORKSPACE, else a parent of this file)."""
    import os

    if env := os.environ.get("LARMORX_WORKSPACE"):
        return Path(env)
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    raise SystemExit("the workspace with oracles/ was not found (LARMORX_WORKSPACE)")


def _repo() -> Path:
    return Path(__file__).resolve().parents[3]


ORACLE = _workspace() / "oracles/fsl-6.0.7/mcflirt"
PRELOAD = str(_workspace() / "oracles/core-math-preload/libcrmath.so")
SCRATCH = Path(tempfile.gettempdir()) / "larmorx-mcflirt-replay"


def data(p: Path):
    b = p.read_bytes()
    if b[:2] == b"\x1f\x8b":
        b = gzip.decompress(b)
    code = int.from_bytes(b[70:72], "little")
    t = {
        2: np.uint8,
        4: np.int16,
        8: np.int32,
        16: np.float32,
        64: np.float64,
        256: np.int8,
        512: np.uint16,
    }[code]
    off = int(np.frombuffer(b[108:112], np.float32)[0])
    return np.frombuffer(b[off:], t)


def main() -> int:
    case = sys.argv[1]
    replay = Path(sys.argv[2]) if len(sys.argv) > 2 else SCRATCH / case.replace("/", "__")
    odir = ORACLE / "runs" / case
    run = json.loads((odir / "run.json").read_text())
    work = SCRATCH / "crm" / case.replace("/", "__")
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    env = {k: v for k, v in run["env"].items()}
    env["HOME"] = os.environ["HOME"]
    env["PATH"] = "/usr/bin:/bin"
    env["LD_PRELOAD"] = PRELOAD
    for f in odir.iterdir():
        if (
            f.is_file()
            and not f.name.startswith(("std", "run.json"))
            and f.name not in run.get("outputs", {})
        ):
            shutil.copy2(f, work / f.name)
    for step in run["steps"]:
        argv = [x.replace(str(odir), str(work)) for x in step["argv"]]
        p = subprocess.run(argv, cwd=work, env=env, capture_output=True, text=True)
        print("exit", p.returncode, "as built", step["exit"])
    for f in sorted(run.get("outputs", {})):
        if not f.endswith((".nii.gz", ".nii")) or "MAT_" in f:
            continue
        a, b, c = work / f, odir / f, replay / f
        if not a.is_file():
            continue
        xa, xb = data(a), data(b)
        line = f"{f}: preload vs as-built {(xa != xb).sum()}"
        if c.is_file():
            xc = data(c)
            line += f", preload vs replica {(xa != xc).sum()}"
        print(line)
    for f in sorted(run.get("outputs", {})):
        if f.endswith((".nii.gz", ".nii")) or f.startswith("std"):
            continue
        a, b, c = work / f, odir / f, replay / f
        if a.is_file() and c.is_file():
            print(
                f,
                "preload==as-built",
                a.read_bytes() == b.read_bytes(),
                "preload==replica",
                a.read_bytes() == c.read_bytes(),
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
