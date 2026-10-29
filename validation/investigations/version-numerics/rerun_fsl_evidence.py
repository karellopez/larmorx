# SPDX-License-Identifier: Apache-2.0
"""The FSL 6.0.7.23 runs of the site's evidence (the figures cache's ``*-fsl-6.0.7.23`` entries of
``fsl-bet`` and ``mri-hmc``), run again into a scratch directory: FSL on OpenBLAS's Haswell core
(the entries were computed on the CPU's own, Cooperlake) and the replica at this checkout
(Armadillo 14.2.1's LAPACK path); every output compared byte for byte with the entry's
(compressed images decompressed). Nothing in the cache is changed: identical outputs mean the
pages' panels stand as they are.

Usage: ``python -P rerun_fsl_evidence.py <figures cache> <larmorx-nc binary> <scratch>``
"""

from __future__ import annotations

import gzip
import json
import os
import subprocess
import sys
from pathlib import Path

ENTRIES = (
    "fsl-bet/betsurf-synthetic-head-fsl-6.0.7.23",
    "fsl-bet/fmriprep-ds000005-run-01-fsl-6.0.7.23",
    "fsl-bet/panel-ds000228-pixar008-fsl-6.0.7.23",
    "mri-hmc/fmriprep-ds000005-run-01-fsl-6.0.7.23",
    "mri-hmc/fmriprep-ds000005-run-01-replica",
)


def content(p: Path) -> bytes:
    raw = p.read_bytes()
    return gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw


def owned(f: Path, out: Path, label: str) -> bool:
    """Whether ``f`` is an output of the run ``label``: in its directory, or named after it
    (``fsl23.nii.gz``, ``fsl23_mask.nii.gz``; not ``fsl23-reproduce...``, another run's)."""
    first = f.relative_to(out).parts[0]
    return first == label or (first.startswith(label) and first[len(label)] in "._")


def main() -> int:
    cache, binary, scratch = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
    differ = 0
    for name in ENTRIES:
        e = cache / name
        info = json.loads((e / "RUN_INFO.json").read_text(encoding="utf-8"))
        out = scratch / name
        for run in info["runs"]:
            argv = [a.replace(str(e), str(out)) for a in run["argv"]]
            if argv[0].endswith("larmorx-nc"):
                argv[0] = binary
            env = {"PATH": "/usr/bin:/bin", "HOME": os.environ.get("HOME", ""), **run["env"]}
            if argv[0].startswith(str(Path.home() / "fsl")):
                env["OPENBLAS_CORETYPE"] = "Haswell"
            label = run["label"]
            for a in argv:
                if a.startswith(str(out)):
                    Path(a).parent.mkdir(parents=True, exist_ok=True)
            (out / label).mkdir(parents=True, exist_ok=True)
            p = subprocess.run(argv, env=env, cwd=run.get("cwd") or out, capture_output=True)
            ours = sorted(
                f.relative_to(out) for f in out.rglob("*") if f.is_file() and owned(f, out, label)
            )
            same = [f for f in ours if (e / f).is_file() and content(e / f) == content(out / f)]
            missing = [f for f in ours if not (e / f).is_file()]
            diff = [f for f in ours if f not in same and f not in missing]
            differ += len(diff)
            print(
                f"{name} {label}: exit {p.returncode}, {len(same)} of {len(ours)} files identical"
                + (f"; differ: {[str(f) for f in diff]}" if diff else "")
                + (f"; not in the entry: {[str(f) for f in missing]}" if missing else "")
            )
    print(f"{differ} files differ")
    return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
