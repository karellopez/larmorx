# SPDX-License-Identifier: Apache-2.0
"""Peak memory and time of an ANTs program and of larmorx on the same call on real files (the
data panel's largest inputs): each run its own process under ``/usr/bin/time -v`` (its maximum
resident set), ANTs at one thread (as built, or with CORE-MATH preloaded with ``--crmath``),
larmorx through the command users run (``python -m larmorx ants <program> ...``) at one thread
in both D13 modes; every output compared byte for byte (images: header and values as stored).

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P memory_real.py <work dir> [--crmath] [--threads N] <program> -- <arguments>

The arguments are the program's own, with ``{out}`` for the run's output directory (each run
writes into its own). Prints one line per run (exit status, seconds, peak MiB) and larmorx's
peak against ANTs'; writes ``<work dir>/memory.json``.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path


def peak_of(cmd: list[str], env: dict[str, str], cwd: Path) -> tuple[int, float, float]:
    p = subprocess.run(
        ["/usr/bin/time", "-v", *cmd], capture_output=True, text=True, env=env, cwd=cwd,
        check=False,
    )  # fmt: skip
    rss = re.search(r"Maximum resident set size \(kbytes\): (\d+)", p.stderr)
    wall = re.search(r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): ([\d:.]+)", p.stderr)
    seconds = 0.0
    if wall:
        for part in wall.group(1).split(":"):
            seconds = seconds * 60 + float(part)
    if p.returncode != 0:
        print(p.stderr[-1500:], file=sys.stderr)
    return p.returncode, (int(rss.group(1)) / 1024 if rss else float("nan")), seconds


def same_outputs(a: Path, b: Path) -> tuple[bool, str]:
    """Whether two output directories hold the same files with the same contents (images by
    their header and values as stored)."""
    import gzip

    names_a = sorted(p.name for p in a.iterdir())
    names_b = sorted(p.name for p in b.iterdir())
    if names_a != names_b:
        return False, f"files {names_a} vs {names_b}"
    for name in names_a:
        x, y = (a / name).read_bytes(), (b / name).read_bytes()
        if x[:2] == b"\x1f\x8b":
            x = gzip.decompress(x)
        if y[:2] == b"\x1f\x8b":
            y = gzip.decompress(y)
        if name.endswith((".nii", ".nii.gz")):
            if x[:348] != y[:348] or x[352:] != y[352:]:
                return False, f"{name} differs"
        elif x != y:
            return False, f"{name} differs"
    return True, f"{len(names_a)} file(s) identical"


def main() -> int:
    args = sys.argv[1:]
    work = Path(args.pop(0))
    crmath = "--crmath" in args
    if crmath:
        args.remove("--crmath")
    threads = "1"
    if "--threads" in args:
        i = args.index("--threads")
        threads = args[i + 1]
        del args[i : i + 2]
    program = args.pop(0)
    assert args.pop(0) == "--"
    from larmorx_validation.parity.ants_programs import ants_bin, crmath_preload

    work.mkdir(parents=True, exist_ok=True)
    env = {
        **os.environ,
        "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": threads,
        "OMP_NUM_THREADS": threads,
    }
    env.pop("NSLOTS", None)
    env.setdefault("ANTS_RANDOM_SEED", "50495")
    rows = {}
    runs = [("ANTs", [str(ants_bin() / program)], {})]
    if crmath:
        runs[0][2]["LD_PRELOAD"] = str(crmath_preload())
    for mode in ("reproduce", "fix"):
        runs.append(
            (
                f"larmorx-{mode}",
                [sys.executable, "-m", "larmorx", "ants", program],
                {"LARMORX_UPSTREAM_DEFECTS": mode},
            )
        )
    for name, head, extra in runs:
        out = work / name
        out.mkdir(exist_ok=True)
        for f in out.iterdir():
            f.unlink()
        cmd = [*head, *(a.replace("{out}", str(out)) for a in args)]
        code, mib, seconds = peak_of(cmd, {**env, **extra}, out)
        rows[name] = {"exit": code, "peak_mib": mib, "seconds": seconds}
        print(f"{name:18s} exit {code}  {seconds:9.1f} s  {mib:9.1f} MiB", flush=True)
    same, detail = same_outputs(work / "ANTs", work / "larmorx-reproduce")
    worst = max(rows["larmorx-reproduce"]["peak_mib"], rows["larmorx-fix"]["peak_mib"])
    print(f"outputs (reproduce vs ANTs): {detail}")
    print(f"larmorx's peak / ANTs': {worst / rows['ANTs']['peak_mib']:.3f} "
          f"({worst - rows['ANTs']['peak_mib']:+.1f} MiB)")  # fmt: skip
    (work / "memory.json").write_text(
        json.dumps(
            {
                "program": program,
                "args": args,
                "threads": threads,
                "crmath": crmath,
                "runs": rows,
                "identical": same,
                "detail": detail,
            },
            indent=1,
        ),
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
