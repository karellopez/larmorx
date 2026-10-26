# SPDX-License-Identifier: Apache-2.0
"""Which of larmorx's ANTs programs run parallel work on rayon's global pool (sized by the CPUs
the process may use, not by the call's thread count) instead of their own pool of the requested
size (``larmorx_core::parallel::with_threads``, threads named ``larmorx-<n>-<i>``).

Each program runs as ``python -m larmorx ants <program> ...`` with
``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=1`` on small seeded inputs; its threads are sampled from
``/proc/<pid>/task`` every 20 ms. A thread that is neither the main thread nor a ``larmorx-<n>``
pool thread, and that ran (its CPU time grew), is a global-pool worker. With
``RAYON_NUM_THREADS`` unset, the global pool has as many threads as the CPUs the process may use.

    PYTHONPATH=<checkout>/python:<checkout>/validation/src python -P global_pool_probe.py <dir>
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

import numpy as np


def threads(pid: int) -> dict[int, tuple[str, int]]:
    out = {}
    for t in Path(f"/proc/{pid}/task").glob("*"):
        try:
            name = (t / "comm").read_text().strip()
            fields = (t / "stat").read_text().rsplit(")", 1)[1].split()
            out[int(t.name)] = (name, int(fields[11]) + int(fields[12]))  # utime + stime
        except (OSError, IndexError, ValueError):
            continue
    return out


def probe(cmd: list[str], env: dict[str, str]) -> tuple[int, int, dict[str, int]]:
    """(exit status, ``larmorx-<n>`` pool threads, the other threads by name), every thread the
    process had while it ran, the main thread aside."""
    p = subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    seen: dict[int, str] = {}
    while p.poll() is None:
        for tid, (name, _ticks) in threads(p.pid).items():
            if tid != p.pid:
                seen[tid] = name
        time.sleep(0.02)
    pool = sum(1 for n in seen.values() if n.startswith("larmorx-"))
    other: dict[str, int] = {}
    for n in seen.values():
        if not n.startswith("larmorx-"):
            other[n] = other.get(n, 0) + 1
    return p.returncode, pool, other


def main() -> int:
    import nibabel as nib

    d = Path(sys.argv[1])
    d.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(7)
    shape = (64, 64, 48)
    i, j, k = np.indices(shape, dtype=np.float64)
    r = np.sqrt((i - 32) ** 2 + (j - 32) ** 2 + ((k - 24) * 1.3) ** 2)
    head = np.where(r < 24, 400 + 3 * i, 20) + np.where(r < 12, 300, 0)
    img = (head + rng.normal(0, 10, shape)).astype(np.float32)
    a = np.diag([2.0, 2.0, 2.0, 1.0])
    nib.save(nib.Nifti1Image(img, a), d / "fixed.nii.gz")
    nib.save(nib.Nifti1Image(np.roll(img, 2, axis=0), a), d / "moving.nii.gz")
    nib.save(nib.Nifti1Image((r < 24).astype(np.uint8), a), d / "mask.nii.gz")
    f, m, x = str(d / "fixed.nii.gz"), str(d / "moving.nii.gz"), str(d / "mask.nii.gz")
    o = str(d / "out")
    programs = {
        "antsAI": ["-d", "3", "-m", f"Mattes[{f},{m},32,Regular,0.25]", "-t", "Rigid[0.1]",
                   "-s", "[20,0.12]", "-o", o + "_ai.mat"],
        "antsAffineInitializer": ["3", f, m, o + "_init.mat", "15", "0.1", "0", "10"],
        "antsRegistration (Rigid, MI)": ["-d", "3", "-o", o + "_r", "-t", "Rigid[0.1]",
                   "-m", f"MI[{f},{m},1,32,Regular,0.25]", "-c", "[20x10,1e-6,5]",
                   "-f", "2x1", "-s", "1x0vox"],
        "antsRegistration (SyN, CC)": ["-d", "3", "-o", o + "_s", "-t", "SyN[0.1,3,0]",
                   "-m", f"CC[{f},{m},1,2]", "-c", "[5x3,1e-6,5]", "-f", "2x1",
                   "-s", "1x0vox"],
        "Atropos": ["-d", "3", "-a", f, "-x", x, "-i", "KMeans[3]", "-c", "[3,0]",
                    "-m", "[0.1,1x1x1]", "-o", f"[{o}_seg.nii.gz,{o}_post%02d.nii.gz]"],
        "N4BiasFieldCorrection": ["-d", "3", "-i", f, "-o", o + "_n4.nii.gz", "-s", "2",
                                  "-c", "[10x10,0]"],
        "DenoiseImage": ["-d", "3", "-i", f, "-o", o + "_dn.nii.gz"],
        "antsApplyTransforms": ["-d", "3", "-i", m, "-r", f, "-t", "identity",
                                "-n", "LanczosWindowedSinc", "-o", o + "_at.nii.gz"],
        "ImageMath": ["3", o + "_im.nii.gz", "G", f, "1.5"],
    }  # fmt: skip
    env = {k: v for k, v in os.environ.items() if k not in ("RAYON_NUM_THREADS", "NSLOTS")}
    env["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = "1"
    print(f"CPUs this process may use: {len(os.sched_getaffinity(0))}")
    print("| Program | exit | `larmorx-<n>` pool threads | other threads (by name) |")
    print("|---|---|---|---|")
    for name, args in programs.items():
        prog = name.split(" (")[0]
        code, pool, other = probe([sys.executable, "-m", "larmorx", "ants", prog, *args], env)
        names = ", ".join(f"{k} x{v}" for k, v in sorted(other.items())) or "none"
        print(f"| {name} | {code} | {pool} | {names} |", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
