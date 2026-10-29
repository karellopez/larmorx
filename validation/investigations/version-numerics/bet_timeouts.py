# SPDX-License-Identifier: Apache-2.0
"""Random testing's BET cases that ran out of time, run to their end: the case's recipe (seed and
number) rebuilt as random testing builds it, FSL's program and the replica (``larmorx-nc``) run
on the same arguments in directories of their own, each in its own session with a long limit
(the whole session stopped at the limit: a script's children included), timed; then every file
compared byte for byte (images decompressed) with the exit status and both streams.

    python -P bet_timeouts.py <out dir> <seed>:<n> [<seed>:<n> ...] [--limit 3600]

Prints one line per program and a verdict per case. Needs FSL 6.0.7.23 (``~/fsl-6.0.7.23``) and
``crates-nc/target/release/larmorx-nc``; run with ``PYTHONPATH=python:validation/src``.
"""

from __future__ import annotations

import argparse
import gzip
import os
import signal
import subprocess
import sys
import time
from pathlib import Path


def run(argv: list[str], env: dict[str, str], cwd: Path, limit: float) -> tuple[int | str, float]:
    cwd.mkdir(parents=True, exist_ok=True)
    start = time.monotonic()
    with (cwd / "stdout.txt").open("wb") as out, (cwd / "stderr.txt").open("wb") as err:
        p = subprocess.Popen(argv, env=env, cwd=cwd, stdout=out, stderr=err, start_new_session=True)
        try:
            code: int | str = p.wait(timeout=limit)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.wait()
            code = "timeout"
    return code, time.monotonic() - start


def content(p: Path) -> bytes:
    raw = p.read_bytes()
    return gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("out", type=Path)
    ap.add_argument("cases", nargs="+")
    ap.add_argument("--limit", type=float, default=3600.0)
    a = ap.parse_args()
    from larmorx_validation.difftest import core, tools
    from larmorx_validation.parity import fsl_bet as fb

    tool = tools.get("fsl-bet")
    for spec in a.cases:
        seed, n = spec.split(":")
        recipe = core.make_recipe(tool, seed, int(n))
        s = tool.case(recipe, "replica").payload
        paths = [fb.input_path(x) for x in s.inputs]
        args = fb._arguments(s, paths)
        env = fb.environment(s.output_type, s.env)
        d = a.out / f"{seed}-{n}"
        print(f"== {seed}:{n}: {s.tool} {' '.join(args)} (FSLOUTPUTTYPE={s.output_type})")
        fsl_code, fsl_s = run([str(fb.fsl_dir() / "bin" / s.tool), *args], env, d / "fsl", a.limit)
        print(f"   FSL 6.0.7.23: exit {fsl_code}, {fsl_s:.1f} s", flush=True)
        lx_env = dict(env, LARMORX_UPSTREAM_DEFECTS="reproduce")
        lx_code, lx_s = run(
            [str(fb.replica_binary()), "fsl", s.tool, *args], lx_env, d / "replica", a.limit
        )
        print(f"   replica:      exit {lx_code}, {lx_s:.1f} s", flush=True)
        names = sorted(
            {
                p.relative_to(d / side)
                for side in ("fsl", "replica")
                for p in (d / side).rglob("*")
                if p.is_file() and p.name not in ("stdout.txt", "stderr.txt")
            }
        )
        same = [
            x
            for x in names
            if (d / "fsl" / x).is_file()
            and (d / "replica" / x).is_file()
            and content(d / "fsl" / x) == content(d / "replica" / x)
        ]
        # both streams as the suite compares them: the bet script's own path read as ``bet``,
        # larmorx's own lines (its D13 notices) set aside
        streams = all(
            fb.fsl_text((d / "fsl" / f).read_text(errors="replace"))
            == fb.larmorx_text((d / "replica" / f).read_text(errors="replace"))
            for f in ("stdout.txt", "stderr.txt")
        )
        ok = fsl_code == lx_code and len(same) == len(names) and streams
        print(
            f"   {'agree' if ok else 'differ'}: {len(same)} of {len(names)} files byte for byte; "
            f"the streams {'the same' if streams else 'differ (see the directories)'}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
