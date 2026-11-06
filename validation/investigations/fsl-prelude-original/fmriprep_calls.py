# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep 25.2.6's seven ``prelude`` calls (sdcflows' phase-difference workflow) run with
larmorx's clean-room original (and, when given, another program such as the replica), each
output compared bit for bit with fMRIPrep's own unwrapped phase, with the wall time of each run.

No FSL program runs here: the reference is the file fMRIPrep's container wrote (its FSL 6.0.7.x
prelude). Usage::

    python -P fmriprep_calls.py <workspace>/oracles/fmriprep-25.2.6 <scratch dir> \
        --program "<larmorx> fsl prelude" [--program "<larmorx-nc> fsl prelude"] \
        [--threads 1] [--mode fix|reproduce]

One line per call and program: whether the data equal fMRIPrep's bit for bit (and the header's
fields but ``descrip``), the time in seconds (the best of ``--repeat`` runs).
"""

from __future__ import annotations

import argparse
import os
import shlex
import shutil
import subprocess
import sys
import time
from pathlib import Path

import nibabel as nib
import numpy as np


def calls(root: Path) -> list[tuple[str, Path, list[str]]]:
    """Each call: its name, its node folder, its arguments with the container's /work mapped."""
    out = []
    for cmd in sorted(root.glob("*/work/**/phdiff_wf/prelude/command.txt")):
        run = cmd.relative_to(root).parts[0]
        work = root / run / "work"
        words = shlex.split(cmd.read_text())[1:]
        words = [w.replace("=/work/", f"={work}/") for w in words]
        out.append((f"{run}/{cmd.parent.parent.parent.name}", cmd.parent, words))
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("root", type=Path)
    ap.add_argument("scratch", type=Path)
    ap.add_argument("--program", action="append", required=True)
    ap.add_argument("--threads", type=int, default=1)
    ap.add_argument("--mode", default="fix")
    ap.add_argument("--repeat", type=int, default=1)
    a = ap.parse_args()
    found = calls(a.root)
    print(f"{len(found)} calls")
    failures = 0
    for name, node, words in found:
        reference = next(node.glob("*_unwrapped.nii.gz"))
        ref = nib.load(reference)
        for k, program in enumerate(a.program):
            tmp = a.scratch / f"{name.replace('/', '_')}_{k}"
            shutil.rmtree(tmp, ignore_errors=True)
            tmp.mkdir(parents=True)
            args = [w if not w.startswith("--unwrap=") else "--unwrap=out.nii.gz" for w in words]
            env = {
                "PATH": "/usr/bin:/bin",
                "FSLOUTPUTTYPE": "NIFTI_GZ",
                "OMP_NUM_THREADS": str(a.threads),
                "LARMORX_UPSTREAM_DEFECTS": a.mode,
                "HOME": os.environ.get("HOME", "/tmp"),
            }
            best = float("inf")
            for _ in range(a.repeat):
                t0 = time.perf_counter()
                r = subprocess.run(
                    [*shlex.split(program), *args], cwd=tmp, env=env, capture_output=True, text=True
                )
                best = min(best, time.perf_counter() - t0)
            if r.returncode != 0:
                print(f"{name} [{k}]: exit {r.returncode}: {r.stderr.strip()[-300:]}")
                failures += 1
                continue
            got = nib.load(tmp / "out.nii.gz")
            a_data = np.asanyarray(got.dataobj)
            b_data = np.asanyarray(ref.dataobj)
            same = a_data.dtype == b_data.dtype and a_data.tobytes() == b_data.tobytes()
            header_diff = [
                key
                for key in ref.header.keys()
                if key != "descrip"
                and np.asarray(ref.header[key]).tobytes() != np.asarray(got.header[key]).tobytes()
            ]
            n_diff = (
                int(np.sum(a_data.view(np.uint32) != b_data.view(np.uint32)))
                if a_data.shape == b_data.shape and a_data.dtype == b_data.dtype
                else -1
            )
            ok = same and not header_diff
            failures += not ok
            print(
                f"{name} [{k}]: {'identical' if same else f'{n_diff} values differ'}"
                f"{'' if not header_diff else f'; header {header_diff}'}; {best:.3f} s"
            )
    print(f"{failures} failures")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
