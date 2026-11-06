# SPDX-License-Identifier: Apache-2.0
"""Speed and memory of prelude's implementations on large generated phase maps: each program run
on the same inputs (a wrapped smooth phase with noise over an ellipsoid; the noise sets how many
regions the labels make), its wall time (the best of ``--repeat``) and its peak resident memory
(``/usr/bin/time -v``'s maximum resident set), and whether its unwrapped phase equals the first program's bit for
bit. No FSL program runs here. Usage::

    python -P bench_large.py <work dir> --program "<label>=<command words>" ... \
        [--threads 1 --threads 14] [--sizes 128x128x128,256x256x160] [--repeat 3]
"""

from __future__ import annotations

import argparse
import os
import re
import shlex
import subprocess
import sys
import time
from pathlib import Path

import nibabel as nib
import numpy as np


def make(d: Path, shape: tuple[int, int, int], noise: float) -> None:
    rng = np.random.default_rng(7)
    grids = np.meshgrid(*(np.linspace(-1, 1, n, dtype=np.float32) for n in shape), indexing="ij")
    x, y, z = grids
    inside = x**2 + y**2 + z**2 < 0.9
    truth = 30 * x**2 + 20 * y**2 + 12 * x * z + 8 * z
    phase = np.mod(truth + rng.normal(0, noise, shape) + np.pi, 2 * np.pi) - np.pi
    mag = np.where(inside, 1000.0, 20.0) + rng.normal(0, 10, shape)
    aff = np.diag([-2.0, 2.0, 2.0, 1.0])
    nib.save(nib.Nifti1Image(phase.astype(np.float32), aff), d / "ph.nii.gz")
    nib.save(nib.Nifti1Image(mag.astype(np.float32), aff), d / "mag.nii.gz")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("work", type=Path)
    ap.add_argument("--program", action="append", required=True)
    ap.add_argument("--threads", action="append", type=int)
    ap.add_argument("--sizes", default="128x128x128,256x256x160")
    ap.add_argument("--noise", type=float, default=0.6)
    ap.add_argument("--repeat", type=int, default=3)
    ap.add_argument("--extra", default="", help="more prelude options")
    a = ap.parse_args()
    threads = a.threads or [1]
    programs = [p.split("=", 1) for p in a.program]
    for size in a.sizes.split(","):
        shape = tuple(int(v) for v in size.split("x"))
        d = a.work / f"{size}-{a.noise}"
        d.mkdir(parents=True, exist_ok=True)
        if not (d / "ph.nii.gz").exists():
            make(d, shape, a.noise)
        first = None
        for label, words in programs:
            for t in threads if label.startswith("original") else threads[:1]:
                out = d / f"out-{label}-{t}.nii.gz"
                env = {"PATH": "/usr/bin:/bin", "FSLOUTPUTTYPE": "NIFTI_GZ"}
                env["OMP_NUM_THREADS"] = str(t)
                best, peak = float("inf"), 0
                for _ in range(a.repeat):
                    t0 = time.perf_counter()
                    r = subprocess.run(
                        ["/usr/bin/time", "-v", *shlex.split(words), "-p", "ph", "-a", "mag",
                         "-o", out.name, "-v", *shlex.split(a.extra)],
                        cwd=d, env=env, capture_output=True, text=True,
                    )
                    best = min(best, time.perf_counter() - t0)
                    m = re.search(r"Maximum resident set size \(kbytes\): (\d+)", r.stderr)
                    peak = max(peak, int(m.group(1)) if m else 0)
                    if r.returncode != 0:
                        print(f"{size} {label} t={t}: exit {r.returncode} {r.stderr[-300:]}")
                        break
                regions = next(
                    (ln for ln in r.stdout.splitlines() if ln.startswith("Calculating")), ""
                )
                data = np.asanyarray(nib.load(out).dataobj)
                if first is None:
                    first = data
                same = data.tobytes() == first.tobytes()
                print(
                    f"{size} noise {a.noise} {label} threads {t}: {best:.3f} s, peak "
                    f"{peak / 1024:.0f} MiB, {regions.split('(')[-1].rstrip(')')}, "
                    f"{'identical' if same else 'DIFFERENT'}"
                )
    return 0


if __name__ == "__main__":
    os.environ.setdefault("OMP_NUM_THREADS", "1")
    sys.exit(main())
