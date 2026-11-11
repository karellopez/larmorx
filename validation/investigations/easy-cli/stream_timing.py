# SPDX-License-Identifier: Apache-2.0
"""When does a tool's output reach the user: as it runs, or only when it ends?

Runs one long, talkative command (``N4BiasFieldCorrection -v`` on a generated image) through a
``larmorx`` command line and records the time each line of its standard output and standard
error arrives (both read as pipes, as a terminal or a log would get them). Prints, for each
command line given, the run's duration, when the first and the last line arrived, and the share
of the lines that arrived before the last second of the run. A command line that shows the
tool's messages live has its first line early and most lines before the end; one that collects
them and writes them at the end has every line within its last moments.

Usage (``--larmorx`` repeatable: the standalone program, or the console script as
``python -m larmorx.cli``)::

    python -P stream_timing.py --work <folder> \\
        --larmorx "<path>/larmorx" \\
        --larmorx "python -c 'import sys; from larmorx.cli import main; sys.exit(main())'"

The images are made with plain arithmetic (no data files); the output folder is ``--work``.
"""

from __future__ import annotations

import argparse
import os
import selectors
import shlex
import subprocess
import time
from pathlib import Path

import nibabel as nib
import numpy as np


def make_image(path: Path, n: int) -> None:
    """A smooth bright blob with a bias across it and seeded noise: N4 has work to do."""
    rng = np.random.default_rng(20261111)
    x, y, z = np.meshgrid(*(np.arange(n, dtype=np.float32),) * 3, indexing="ij")
    c = (n - 1) / 2
    r2 = ((x - c) ** 2 + (y - c) ** 2 + (z - c) ** 2) / (0.4 * n) ** 2
    blob = np.where(r2 < 1, 1000.0 - 400.0 * r2, 50.0)
    bias = 1.0 + 0.5 * x / n
    data = (blob * bias + rng.normal(0, 10, size=blob.shape)).astype(np.float32)
    nib.Nifti1Image(data, np.eye(4)).to_filename(path)


def timed(
    command: list[str], cwd: Path, env: dict[str, str] | None = None
) -> dict[str, float | int]:
    """Runs ``command``; the arrival time of every line of its output streams."""
    start = time.monotonic()
    proc = subprocess.Popen(
        command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    assert proc.stdout is not None and proc.stderr is not None
    sel = selectors.DefaultSelector()
    sel.register(proc.stdout, selectors.EVENT_READ)
    sel.register(proc.stderr, selectors.EVENT_READ)
    arrivals: list[float] = []
    open_streams = 2
    while open_streams:
        for key, _ in sel.select():
            chunk = key.fileobj.read1(65536)  # type: ignore[union-attr]
            if not chunk:
                sel.unregister(key.fileobj)
                open_streams -= 1
                continue
            arrivals += [time.monotonic() - start] * chunk.count(b"\n")
    code = proc.wait()
    end = time.monotonic() - start
    early = sum(1 for t in arrivals if t < end - 1.0)
    return {
        "exit": code,
        "seconds": round(end, 2),
        "lines": len(arrivals),
        "first_line_at": round(arrivals[0], 2) if arrivals else -1,
        "last_line_at": round(arrivals[-1], 2) if arrivals else -1,
        "share_before_last_second": round(early / len(arrivals), 3) if arrivals else 0.0,
        "median_line_at": round(sorted(arrivals)[len(arrivals) // 2], 2) if arrivals else -1,
        "share_in_first_half": (
            round(sum(1 for t in arrivals if t < end / 2) / len(arrivals), 3) if arrivals else 0.0
        ),
    }


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--larmorx", action="append", required=True)
    ap.add_argument("--size", type=int, default=192)
    ap.add_argument("--iterations", type=int, default=300)
    a = ap.parse_args()
    a.work.mkdir(parents=True, exist_ok=True)
    make_image(a.work / "in.nii.gz", a.size)
    tool = [
        "--n-threads",
        "4",
        "ants",
        "N4BiasFieldCorrection",
        "-d",
        "3",
        "-i",
        "in.nii.gz",
        "-o",
        "out.nii.gz",
        "-c",
        f"[{a.iterations}x{a.iterations}x{a.iterations}x{a.iterations},0]",
        "-v",
        "1",
    ]
    for larmorx in a.larmorx:
        command = shlex.split(larmorx)
        result = timed(command + tool, a.work)
        if result["exit"] == 2:  # a larmorx before the options (0.1.0rc1): the variable
            env = dict(os.environ, ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=tool[1])
            result = timed(command + tool[2:], a.work, env)
        print(larmorx, result)


if __name__ == "__main__":
    main()
