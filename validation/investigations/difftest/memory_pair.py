# SPDX-License-Identifier: Apache-2.0
"""Peak memory of an ANTs program and of larmorx on the same call: each run as its own process
under ``/usr/bin/time -v`` (its maximum resident set), larmorx through the command users run
(``python -m larmorx ants <program> ...``) in both D13 modes, the outputs compared bit for bit.
The input is a deterministic float32 volume (plain arithmetic) of the size given.

    python -P validation/investigations/difftest/memory_pair.py <dir> <program> <size> <spacing> \\
        -- <program's arguments, with {in} and {out}>

e.g. ``... ResampleImageBySpacing 160x160x80 2,2,2 -- 3 {in} {out} 0.5 0.5 0.5`` (a mid-size
case: the output 640x640x320). Prints one line per run (seconds, peak MiB) and the ratio of
larmorx's peak to ANTs'.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np


def peak_of(cmd: list[str], env: dict[str, str]) -> tuple[int, float, float, str]:
    p = subprocess.run(
        ["/usr/bin/time", "-v", *cmd], capture_output=True, text=True, env=env, check=False
    )
    rss = re.search(r"Maximum resident set size \(kbytes\): (\d+)", p.stderr)
    wall = re.search(r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): ([\d:.]+)", p.stderr)
    seconds = 0.0
    if wall:
        for part in wall.group(1).split(":"):
            seconds = seconds * 60 + float(part)
    tail = " ".join(line for line in p.stderr.splitlines()[:3] if "Command" not in line)
    return p.returncode, (int(rss.group(1)) / 1024 if rss else float("nan")), seconds, tail


def volume(size: list[int], spacing: list[float], path: Path) -> None:
    n = int(np.prod(size))
    i = np.arange(n, dtype=np.int64)
    data = ((i * 7919) % 1000).astype(np.float32).reshape(size, order="F") / 10.0
    affine = np.diag([*spacing, 1.0])
    nib.save(nib.Nifti1Image(data, affine), str(path))


def run(work: Path, program: str, size: list[int], spacing: list[float], args: list[str]):
    from larmorx_validation.parity.ants_programs import ants_bin

    work.mkdir(parents=True, exist_ok=True)
    suffix = os.environ.get("MEMORY_PAIR_SUFFIX", ".nii.gz")
    inp = work / f"in{suffix}"
    volume(size, spacing, inp)
    env = {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1", "OMP_NUM_THREADS": "1"}
    rows = {}
    out_a = work / f"ants{suffix}"
    cmd = [str(ants_bin() / program), *(a.format(**{"in": inp, "out": out_a}) for a in args)]
    rows["ANTs"] = (*peak_of(cmd, env), out_a)
    for mode in ("reproduce", "fix"):
        out_l = work / f"larmorx-{mode}{suffix}"
        cmd = [
            sys.executable,
            "-m",
            "larmorx",
            "ants",
            program,
            *(a.format(**{"in": inp, "out": out_l}) for a in args),
        ]
        rows[mode] = (*peak_of(cmd, {**env, "LARMORX_UPSTREAM_DEFECTS": mode}), out_l)
    ref = np.asarray(nib.load(out_a).dataobj) if out_a.is_file() else None
    for who, (code, mib, seconds, tail, out) in rows.items():
        same = ""
        if who != "ANTs" and ref is not None and out.is_file():
            x = np.asarray(nib.load(out).dataobj)
            same = "bit-identical" if x.tobytes() == ref.tobytes() else "differs"
        print(f"{who}: exit {code}, {seconds:.1f} s, peak {mib:.0f} MiB {same} {tail[:120]}")
    a = rows["ANTs"][1]
    for mode in ("reproduce", "fix"):
        print(f"{mode}: {rows[mode][1] / a:.2f} x ANTs' peak")
    return rows


def main() -> int:
    sep = sys.argv.index("--")
    work, program, size, spacing = sys.argv[1:sep]
    run(
        Path(work),
        program,
        [int(x) for x in size.split("x")],
        [float(x) for x in spacing.split(",")],
        sys.argv[sep + 1 :],
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
