# SPDX-License-Identifier: Apache-2.0
"""Black-box runs of 3dUnifize ``-EPI`` where its radius asks for a ball of neighbours no machine
holds (AFNI-75; specs/3dUnifize.md, the spec writer, 2026-10-30): AFNI 26.3.05 and larmorx's
command (``python -m larmorx afni 3dUnifize``, the replica and the clean-room original, both
modes) on a small 4D series whose header gives its voxels in micrometres or in tiny millimetres.

    python -P validation/investigations/specs-memory/unifize_epi_ball.py <out-dir> [--afni-gib 2]

AFNI runs under ``prlimit --as`` (``--afni-gib``, default 2 GiB of address space) and a
time limit, so its allocation of the ball fails at once (or the run is stopped) instead of
filling the machine; larmorx runs with ``LARMORX_MEMORY_LIMIT`` set (``--limit``, default 8G).
Prints one block per case (exit status, the last lines of standard error) and writes
``<out-dir>/results.json``.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

WORKSPACE = Path(__file__).resolve().parents[4]
# A worktree of the repository: <workspace>/larmorx/.claude/worktrees/<name>.
if WORKSPACE.name == "worktrees":
    WORKSPACE = Path(__file__).resolve().parents[7]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", WORKSPACE))


def series(path: Path, zooms: tuple[float, float, float], units: str) -> Path:
    shape = (12, 14, 10, 3)
    i, j, k, t = np.indices(shape, dtype=np.float64)
    r = np.sqrt((i - 6) ** 2 + (j - 7) ** 2 + (k - 5) ** 2)
    data = np.where(r < 5, 900.0 + 5 * i + 3 * t, 40.0 + j).astype(np.float32)
    img = nib.Nifti1Image(data, np.diag([*zooms, 1.0]))
    img.header.set_zooms((*zooms, 2.0))
    img.header.set_xyzt_units(units, "sec")
    nib.save(img, path)
    return path


#: name -> (voxel sizes as stored, units)
CASES = {
    "micron-3": ((3.0, 3.0, 3.0), "micron"),
    "micron-1": ((1.0, 1.0, 1.0), "micron"),
    "mm-0.05": ((0.05, 0.05, 0.05), "mm"),
    "mm-0.3": ((0.3, 0.3, 0.3), "mm"),
    "mm-3": ((3.0, 3.0, 3.0), "mm"),
}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("out", type=Path)
    ap.add_argument("--afni", type=Path, default=WORKSPACE / "oracles/afni-26.3.05/bin/3dUnifize")
    ap.add_argument("--afni-gib", type=int, default=2)
    ap.add_argument("--limit", default="8G")
    ap.add_argument("--timeout", type=int, default=120)
    ap.add_argument("--only", nargs="*")
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    base = {k: v for k, v in os.environ.items() if not k.upper().startswith(("AFNI_", "NIML_"))}
    base.update(
        {
            "OMP_NUM_THREADS": "2",
            "HOME": str(a.out),
            "AFNI_DONT_LOGFILE": "YES",
            "AFNI_NIFTI_NOEXT": "YES",
        }
    )
    results = {}
    for name, (zooms, units) in CASES.items():
        if a.only and name not in a.only:
            continue
        d = a.out / name
        d.mkdir(exist_ok=True)
        inp = series(d / "in.nii.gz", zooms, units)
        runs = {
            "afni": (
                [
                    "prlimit",
                    f"--as={a.afni_gib << 30}",
                    str(a.afni),
                    "-EPI",
                    "-prefix",
                    "afni.nii.gz",
                    "-input",
                    str(inp),
                ],
                {},
            ),
        }
        for impl in ("replica", "original"):
            for mode in ("fix", "reproduce"):
                runs[f"{impl}-{mode}"] = (
                    [
                        sys.executable,
                        "-m",
                        "larmorx",
                        "afni",
                        "3dUnifize",
                        "-EPI",
                        "-prefix",
                        f"{impl}-{mode}.nii.gz",
                        "-input",
                        str(inp),
                    ],
                    {
                        "LARMORX_IMPLEMENTATION": impl,
                        "LARMORX_UPSTREAM_DEFECTS": mode,
                        "LARMORX_MEMORY_LIMIT": a.limit,
                        "PYTHONPATH": os.environ.get("PYTHONPATH", ""),
                    },
                )
        results[name] = {}
        print(f"== {name}: voxels {zooms} {units}")
        for run, (argv, extra) in runs.items():
            if run.endswith("reproduce") and name.startswith(("micron", "mm-0.05")):
                # reproduce attempts the ball as AFNI does: not run where no machine holds it.
                continue
            try:
                p = subprocess.run(
                    argv,
                    cwd=d,
                    env={**base, **extra},
                    capture_output=True,
                    text=True,
                    check=False,
                    timeout=a.timeout,
                )
                code, err = p.returncode, p.stderr
            except subprocess.TimeoutExpired as e:
                code = "timeout"
                err = (
                    (e.stderr or b"").decode("utf-8", "replace")
                    if isinstance(e.stderr, bytes)
                    else (e.stderr or "")
                )
            written = sorted(f.name for f in d.iterdir() if f.name.startswith(run))
            results[name][run] = {
                "argv": argv[-6:],
                "returncode": code,
                "stderr": err[-3000:],
                "written": written,
            }
            print(f"  {run:20s} exit {code}; written {written}")
            for line in [x for x in err.splitlines() if x.strip()][-4:]:
                print(f"     | {line[:300]}")
    (a.out / "results.json").write_text(json.dumps(results, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
