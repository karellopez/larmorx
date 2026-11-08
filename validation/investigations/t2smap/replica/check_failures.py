# SPDX-License-Identifier: Apache-2.0
"""tedana 26.0.3's t2smap failure cases (the TED defects of relay 5a), black box.

Writes three tiny seeded echoes and masks, runs tedana's ``t2smap`` program and the replica
(``larmorx-gpl tedana t2smap``) on each case, and checks that with
``LARMORX_UPSTREAM_DEFECTS=reproduce`` the replica ends as tedana does (exit status, the last line
of standard error) and that by default it names the defect (its ``upstream defect TED-..`` line).
Usage:

    <env>/bin/python -P validation/investigations/t2smap/replica/check_failures.py \\
        --t2smap <env>/bin/t2smap --replica <target>/release/larmorx-gpl --work <empty dir>
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

import nibabel as nb
import numpy as np


def make(d: Path) -> None:
    rng = np.random.default_rng(1)
    shape = (6, 6, 4, 5)
    s0 = 1000 + 200 * rng.random(shape[:3])
    t2 = 0.02 + 0.03 * rng.random(shape[:3])
    aff = np.diag([3.0, 3.0, 3.5, 1.0])
    for i, te in enumerate([0.014, 0.030, 0.046]):
        sig = (s0 * np.exp(-te / t2))[..., None] * (1 + 0.02 * rng.standard_normal(shape))
        nb.Nifti1Image(sig.astype(np.float32), aff).to_filename(d / f"e{i + 1}.nii.gz")
    m = np.zeros(shape[:3], np.uint8)
    m[1:5, 1:5, :] = 1
    nb.Nifti1Image(m, aff).to_filename(d / "mask.nii.gz")
    nb.Nifti1Image(m, np.diag([3.0, 3.0, 3.0, 1.0])).to_filename(d / "mask_aff.nii.gz")


D3 = ["-d", "e1.nii.gz", "e2.nii.gz", "e3.nii.gz"]
TE3 = ["-e", "0.014", "0.030", "0.046"]
M = ["--mask", "mask.nii.gz"]
CASES = [
    ("TED-02", [*D3, *TE3, *M, "--exclude", "5"]),
    (None, [*D3, *TE3, *M, "--exclude", "6"]),
    ("TED-10", [*D3, "-e", "inf", "inf", "inf", *M]),
    (None, ["-d", "e1.nii.gz", "e2.nii.gz", "-e", "0.014", *M]),
    ("TED-12", ["-d", "e1.nii.gz", "-e", "0.014", *M]),
    ("TED-12", ["-d", "e1.nii.gz", "-e", "0.014", *M, "--masktype", "none"]),
    (None, [*D3, "-e", "0.014", "0.030", *M]),
    ("TED-13", [*D3, *TE3, *M, "--dummy-scans", "5"]),
    ("TED-11", [*D3, *TE3, *M, "--n-independent-echos", "0"]),
    ("TED-04", [*D3, *TE3, "--mask", "mask_aff.nii.gz"]),
]


def run(cmd: list[str], env: dict[str, str]) -> tuple[int, str]:
    p = subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=600)
    return p.returncode, p.stderr


def last(err: str) -> str:
    lines = [x for x in err.strip().splitlines() if not x.startswith("larmorx-gpl:")]
    return lines[-1] if lines else ""


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--t2smap", required=True)
    ap.add_argument("--replica", required=True)
    ap.add_argument("--work", required=True, type=Path)
    a = ap.parse_args()
    a.work.mkdir(parents=True, exist_ok=True)
    os.chdir(a.work)
    make(a.work)
    env = dict(os.environ, OPENBLAS_CORETYPE="Haswell")
    bad = 0
    for k, (defect, args) in enumerate(CASES):
        t = run([a.t2smap, *args, "--out-dir", f"t{k}"], env)
        r = run(
            [a.replica, "tedana", "t2smap", *args, "--out-dir", f"r{k}"],
            dict(env, LARMORX_UPSTREAM_DEFECTS="reproduce"),
        )
        f = run(
            [a.replica, "tedana", "t2smap", *args, "--out-dir", f"f{k}"],
            dict(env, LARMORX_UPSTREAM_DEFECTS="fix"),
        )
        # A run tedana completes: the replica's run to the fit (relay 5a) ends with status 3.
        same = r[0] in (0, 3) if t[0] == 0 else (t[0] == r[0] and last(t[1]) == last(r[1]))
        named = defect is None or f"upstream defect {defect} " in f[1]
        ok = same and named
        bad += not ok
        print(
            f"{'ok  ' if ok else 'DIFF'} {defect or '-':7} exit {t[0]}/{r[0]}/{f[0]} "
            f"{last(t[1])[:100]!r} | default: {last(f[1])[:90]!r}"
        )
    print(f"{len(CASES)} cases, {bad} differ")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
