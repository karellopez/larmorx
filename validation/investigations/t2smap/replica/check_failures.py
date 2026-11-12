# SPDX-License-Identifier: Apache-2.0
"""tedana 26.0.3's t2smap failure cases (the TED defects of relay 5a), black box; since relay RS1
of the clean-room part (2026-11-12) also the replica's gaps that part found: TED-31 and TED-46
(both modes), the chained tracebacks of a 3D echo and of masks of another grid, the temporal
means' warning of a series with both infinities, the images' cast overflow before and after the
confounds table, numpy's scientific printing of the dropout thresholds, and ``--fitmode ts`` with
no volume (no echo-time check).

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
    nan1 = nb.load(d / "e1.nii.gz").get_fdata(dtype=np.float32)
    nan1[2, 2, 1, 3] = np.nan  # TED-01: a NaN first-echo sample inside the mask
    nb.Nifti1Image(nan1, aff).to_filename(d / "nan-e1.nii.gz")
    m = np.zeros(shape[:3], np.uint8)
    m[1:5, 1:5, :] = 1
    nb.Nifti1Image(m, aff).to_filename(d / "mask.nii.gz")
    nb.Nifti1Image(m, np.diag([3.0, 3.0, 3.0, 1.0])).to_filename(d / "mask_aff.nii.gz")
    # relay RS1 (2026-11-12): the replica's gaps of the clean-room part
    ech = [nb.load(d / f"e{i}.nii.gz").get_fdata(dtype=np.float32) for i in (1, 2, 3)]

    def save(x, name):
        nb.Nifti1Image(x, aff).to_filename(d / name)

    save(ech[0][..., 0], "e1-3d.nii.gz")  # a 3D echo: two chained tracebacks
    save(m[:-1], "mask_grid.nii.gz")  # another grid: chained after the fast path's mismatch
    save(np.zeros((6, 6, 5), np.uint8), "mask_grid_empty.nii.gz")  # empty, another grid
    save(-ech[0], "dead-e1.nii.gz")  # no voxel left for the fit (TED-31)
    both = ech[1].copy()
    both[2, 2, 1, 1], both[2, 2, 1, 2] = np.inf, -np.inf  # the means' invalid sum
    save(both, "both-e2.nii.gz")
    for i, x in enumerate(ech, 1):
        u = np.clip(np.round(x / 4.0), 0, 255).astype(np.uint8)
        u[1:3, 1:4] = 255  # equal observations in the mask (TED-46)
        save(u, f"sat-e{i}.nii.gz")
        # means of about 2.5e5, 30 and 3: numpy prints the thresholds in scientific form
        save((x * (400.0, 0.06, 0.009)[i - 1]).astype(np.float32), f"sci-e{i}.nii.gz")
    # T2* of about 0.1 ms from echo times 0.05 ms apart: S0 = exp(126), beyond float32
    for i, f in ((2, 0.6), (3, 0.37)):
        x = ech[i - 1].copy()
        x[1:4, 1:4] = ech[0][1:4, 1:4] * f
        save(x, f"big-e{i}.nii.gz")


D3 = ["-d", "e1.nii.gz", "e2.nii.gz", "e3.nii.gz"]
TE3 = ["-e", "0.014", "0.030", "0.046"]
M = ["--mask", "mask.nii.gz"]
CASES = [
    ("TED-01", ["-d", "nan-e1.nii.gz", "e2.nii.gz", "e3.nii.gz", *TE3, *M]),
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
    # relay RS1 (2026-11-12)
    ("TED-31", ["-d", "dead-e1.nii.gz", "e2.nii.gz", "e3.nii.gz", *TE3, *M, "--masktype", "none"]),
    (
        "TED-31",
        [
            "-d",
            "dead-e1.nii.gz",
            "e2.nii.gz",
            "e3.nii.gz",
            *TE3,
            *M,
            "--masktype",
            "none",
            "--fittype",
            "curvefit",
        ],
    ),
    (
        "TED-31",
        [
            "-d",
            "dead-e1.nii.gz",
            "e2.nii.gz",
            "e3.nii.gz",
            *TE3,
            *M,
            "--masktype",
            "none",
            "--fitmode",
            "ts",
        ],
    ),
    ("TED-46", ["-d", "sat-e1.nii.gz", "sat-e2.nii.gz", "sat-e3.nii.gz", *TE3, *M]),
    (
        "TED-46",
        [
            "-d",
            "sat-e1.nii.gz",
            "sat-e2.nii.gz",
            "sat-e3.nii.gz",
            *TE3,
            *M,
            "--fittype",
            "curvefit",
        ],
    ),
    (
        "TED-46",
        [
            "-d",
            "sat-e1.nii.gz",
            "sat-e2.nii.gz",
            "sat-e3.nii.gz",
            *TE3,
            *M,
            "--fitmode",
            "ts",
            "--masktype",
            "none",
        ],
    ),
    (None, ["-d", "e1-3d.nii.gz", "e1-3d.nii.gz", "e1-3d.nii.gz", *TE3, *M]),
    (None, [*D3, *TE3, "--mask", "mask_grid.nii.gz"]),
    (None, [*D3, *TE3, "--mask", "mask_grid_empty.nii.gz"]),
    ("TED-09", ["-d", "e1.nii.gz", "both-e2.nii.gz", "e3.nii.gz", *TE3, *M, "--combmode", "paid"]),
    (
        None,
        [
            "-d",
            "e1.nii.gz",
            "big-e2.nii.gz",
            "big-e3.nii.gz",
            "-e",
            "0.012",
            "0.01205",
            "0.0121",
            *M,
            "--masktype",
            "none",
            "--debug",
        ],
    ),
    (None, ["-d", "sci-e1.nii.gz", "sci-e2.nii.gz", "sci-e3.nii.gz", *TE3, *M]),
    (
        "TED-13",
        [
            *D3,
            "-e",
            "0.014",
            "0.030",
            *M,
            "--masktype",
            "none",
            "--fitmode",
            "ts",
            "--dummy-scans",
            "5",
        ],
    ),
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
