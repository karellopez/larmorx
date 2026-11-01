# SPDX-License-Identifier: Apache-2.0
"""Peak memory and time of FSL's flirt and of the replica on random testing's findings that ran
over the trial's memory budget: each finding's recipe (shrunk, or drawn with ``--drawn``) built
into two directories, each program run alone under ``/usr/bin/time -v`` (its maximum resident
set) with a timeout, the matrices compared.

Usage: python -P memory_findings.py <out dir> <timeout s> <finding.json>... [--drawn]
(with ``LARMORX_NC_BIN`` the replica's program; ``PYTHONPATH=<checkout>/python:<checkout>/
validation/src``). Prints one line per finding and program: exit status, seconds, peak MiB.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

from larmorx_validation.difftest import inputs
from larmorx_validation.difftest.core import arguments
from larmorx_validation.parity import fsl_flirt_replica as ff


def measure(cmd: list[str], cwd: Path, env: dict[str, str], timeout: float) -> str:
    try:
        p = subprocess.run(
            ["/usr/bin/time", "-v", *cmd],
            cwd=cwd,
            env=env,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        return f"timeout after {timeout:.0f} s"
    rss = re.search(r"Maximum resident set size \(kbytes\): (\d+)", p.stderr)
    wall = re.search(r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): ([\d:.]+)", p.stderr)
    seconds = 0.0
    if wall:
        for part in wall.group(1).split(":"):
            seconds = seconds * 60 + float(part)
    peak = int(rss.group(1)) / 1024 if rss else float("nan")
    code = re.search(r"Exit status: (\d+)", p.stderr)
    sig = re.search(r"Command terminated by signal (\d+)", p.stderr)
    status = f"signal {sig.group(1)}" if sig else f"exit {code.group(1) if code else '?'}"
    return f"{status}, {seconds:.1f} s, peak {peak:.0f} MiB"


def main() -> None:
    out = Path(sys.argv[1])
    timeout = float(sys.argv[2])
    drawn = "--drawn" in sys.argv
    for path in [a for a in sys.argv[3:] if a != "--drawn"]:
        finding = json.loads(Path(path).read_text(encoding="utf-8"))
        recipe = finding["recipe"] if drawn or not finding.get("shrunk") else finding["shrunk"]
        prefix = inputs.register(recipe["inputs"])
        d = out / Path(path).stem
        if d.exists():
            shutil.rmtree(d)
        words = []
        for sub in ("fsl", "rep"):
            (d / sub).mkdir(parents=True)
        for w in arguments(recipe):
            if w.startswith("{in:"):
                src = inputs.path("dt:" + inputs.key(prefix, w[4:-1]))
                for sub in ("fsl", "rep"):
                    shutil.copy(src, d / sub / src.name)
                words.append(src.name)
            elif w.startswith("@fslsch:"):
                words.append(str(ff.fsldir() / "etc/flirtsch" / w[8:]))
            else:
                words.append(w)
        env = {
            "HOME": os.environ.get("HOME", "/tmp"),
            "PATH": "/usr/bin:/bin",
            "FSLOUTPUTTYPE": recipe["extra"]["output_type"],
            "FSLDIR": str(ff.fsldir()),
            "OPENBLAS_CORETYPE": "Haswell",
            "LARMORX_UPSTREAM_DEFECTS": "reproduce",
            "OMP_NUM_THREADS": "1",
        }
        prog = recipe["extra"]["tool"]
        print(Path(path).stem, prog, " ".join(words), flush=True)
        fsl = measure([str(ff.fsldir() / "bin" / prog), *words], d / "fsl", env, timeout)
        print("  FSL:     ", fsl, flush=True)
        rep = measure([str(ff.replica_binary()), "fsl", prog, *words], d / "rep", env, timeout)
        print("  replica: ", rep, flush=True)
        a, b = d / "fsl" / "out.mat", d / "rep" / "out.mat"
        if a.is_file() and b.is_file():
            print("  matrices:", "identical" if a.read_bytes() == b.read_bytes() else "DIFFER")


if __name__ == "__main__":
    main()
