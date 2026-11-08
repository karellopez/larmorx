# SPDX-License-Identifier: Apache-2.0
"""``-SAVEDISP``'s arithmetic (``specs/3dvolreg.md`` §10.3), checked on AFNI's binary.

Usage (inside a memory box):

    scripts/orchestration/boxed.sh 4 python -P validation/investigations/3dvolreg-spec/savedisp.py \
        <work folder> [--original] [dataset ...]

For each dataset (default: the parity suite's ``s1`` series and fMRIPrep's input re-made from
ds000005), AFNI 26.3.05's 3dvolreg runs twice through the oracle cache (``afni_run.py``) with the
same options: ``-savedisp`` (the three raw displacement datasets) and ``-SAVEDISP`` (the nineteen
normalised ones). §10.3's arithmetic, written here from the spec, is applied to AFNI's raw
displacements and compared with AFNI's nineteen datasets value by value (spec writer, 2026-11-08,
step 3: 0 values differ on both, 4.3 million values).

With ``--original``, larmorx's clean-room original (``python -m larmorx``,
``LARMORX_IMPLEMENTATION=original``, ``reproduce``) runs ``-SAVEDISP`` too, and its sixteen products
are compared with §10.3's products of its own three normalised datasets: where they differ, the
original's products (or their normalisation) do not follow §10.3, whatever its parameters are.
Written from the spec and black-box runs only.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import afni_run as A  # noqa: E402

F = np.float32
PRODUCTS = ["XX", "YY", "ZZ", "XY", "XZ", "YZ", "XXX", "YYY", "ZZZ", "XXY", "XXZ", "XYY", "XZZ", "YYZ", "YZZ", "XYZ"]  # fmt: skip
DEFAULTS = (
    A.WORKSPACE / "oracles/inputs/afni-volreg/s1.nii.gz",
    A.WORKSPACE / "oracles/inputs/afni-volreg/fmriprep/ds000005-run-01_sliced.nii.gz",
)


def detrend(t: np.ndarray, order: str = "spec") -> np.ndarray:
    """§10.3's detrend (3dTshift §4 step 2's fit, with a and b evaluated as §10.3 states) along
    axis 0, for float32 series ``t`` of shape (volumes, voxels): S0 = v0 + v1 + ... and
    S1 = (v1 1) + (v2 2) + ... (each product float32, the sums in double, in order), N = n and
    M = n n (an integer product) as doubles, a = ((2/(N+1)) (1/N)) (((2 (N S0)) - 3 S1) - S0),
    b = ((-6/(M-1)) (1/N)) (((-S0) - 2 S1) + N S0), each rounded to float32; then
    v[i] - (a + b i) in float32 (b i, then a +). Fewer than 3 values: left alone.
    ``order="textbook"``: a and b by the usual closed forms instead (the same in exact
    arithmetic), to show what the order changes."""
    n = t.shape[0]
    if n < 3:
        return t.copy()
    s0 = t[0].astype(np.float64)
    s1 = np.zeros(t.shape[1])
    for i in range(1, n):
        s0 = s0 + t[i].astype(np.float64)
        s1 = s1 + (t[i] * F(i)).astype(np.float64)
    nd, md = float(n), float(n * n)
    if order == "spec":
        a = ((2.0 / (nd + 1.0)) * (1.0 / nd) * ((2.0 * (nd * s0) - 3.0 * s1) - s0)).astype(F)
        b = ((-6.0 / (md - 1.0)) * (1.0 / nd) * ((-s0 - 2.0 * s1) + nd * s0)).astype(F)
    else:
        a = ((2.0 * (2 * n - 1) * s0 - 6.0 * s1) / (n * (n + 1))).astype(F)
        b = ((12.0 * s1 - 6.0 * (n - 1) * s0) / (n * (n * n - 1))).astype(F)
    out = np.empty_like(t)
    for i in range(n):
        out[i] = t[i] - (a + b * F(i))
    return out


def normalise(t: np.ndarray) -> np.ndarray:
    """§10.3: detrended, then every value times 1/max|value| (float32), left alone at 0."""
    d = detrend(t)
    m = np.abs(d).max(axis=0)
    inv = F(1.0) / np.where(m != 0, m, F(1.0))
    return np.where(m != 0, d * inv, d).astype(F)


def legendre(k: int, x: np.ndarray) -> np.ndarray:
    """§10.3's P_k in float32, left to right: P_2(x) = ((1.5 x) x) - 0.5,
    P_3(x) = (((2.5 x) x) - 1.5) x."""
    if k == 0:
        return np.ones_like(x)
    if k == 1:
        return x
    if k == 2:
        return (F(1.5) * x) * x - F(0.5)
    return ((F(2.5) * x) * x - F(1.5)) * x


def products(nx: dict[str, np.ndarray]) -> dict[str, np.ndarray]:
    """The sixteen products (P_a(X) P_b(Y)) P_c(Z) of the normalised three, each normalised."""
    out = {}
    for p in PRODUCTS:
        a, b, c = p.count("X"), p.count("Y"), p.count("Z")
        out[p] = normalise((legendre(a, nx["X"]) * legendre(b, nx["Y"])) * legendre(c, nx["Z"]))
    return out


def series(path: Path) -> np.ndarray:
    a = np.asanyarray(nib.load(path).dataobj).astype(F)
    return a.reshape(-1, a.shape[3]).T.copy()


def run_original(argv: list[str], d: Path) -> int:
    d.mkdir(parents=True, exist_ok=True)
    (d / "home").mkdir(exist_ok=True)
    env = {
        "HOME": str(d / "home"),
        "PATH": "/usr/bin:/bin",
        "LARMORX_IMPLEMENTATION": "original",
        "LARMORX_UPSTREAM_DEFECTS": "reproduce",
        **{k: os.environ[k] for k in ("PYTHONPATH", "VIRTUAL_ENV") if k in os.environ},
    }
    p = subprocess.run([sys.executable, "-P", "-m", "larmorx", "afni", "3dvolreg", *argv], cwd=d, env=env, capture_output=True, text=True, check=False)  # fmt: skip
    return p.returncode


def main(argv: list[str]) -> int:
    work = Path(argv[0]).resolve()
    original = "--original" in argv
    data = [Path(x) for x in argv[1:] if x != "--original"] or list(DEFAULTS)
    bad = 0
    for src in data:
        tag = src.name.split(".")[0]
        runs = {}
        for opt in ("-savedisp", "-SAVEDISP"):
            d = work / f"{tag}{opt}"
            r = A.run(["-prefix", "NULL", "-1Dfile", "p.1D", opt, "sd.nii", str(src)], d)
            if r.status != 0:
                print(f"{tag}: AFNI {opt} failed: {r.stderr[-300:]}")
                return 2
            runs[opt] = d
        raw = {c: series(runs["-savedisp"] / f"sd_D{c}.nii") for c in "XYZ"}
        afni = {c: series(runs["-SAVEDISP"] / f"sd_D{c}.nii") for c in ["X", "Y", "Z", *PRODUCTS]}
        nx = {c: normalise(raw[c]) for c in "XYZ"}
        mine = {**nx, **products(nx)}
        diff = {k: int(np.count_nonzero(mine[k] != afni[k])) for k in afni}
        total = sum(v.size for v in afni.values())
        bad += sum(diff.values())
        print(f"{tag}: §10.3 on AFNI's raw displacements against AFNI's 19 datasets: "
              f"{sum(diff.values())} of {total} values differ" + (f" {diff}" if any(diff.values()) else ""))  # fmt: skip
        # what §10.3's order of a's and b's double operations decides: the textbook closed forms
        tb = {c: detrend(raw[c], "textbook") for c in "XYZ"}
        tbn = {}
        for c in "XYZ":
            m = np.abs(tb[c]).max(axis=0)
            tbn[c] = np.where(m != 0, tb[c] * (F(1.0) / np.where(m != 0, m, F(1.0))), tb[c]).astype(
                F
            )
        n_tb = sum(int(np.count_nonzero(tbn[c] != afni[c])) for c in "XYZ")
        print(f"{tag}: with the textbook closed forms of a and b instead, the normalised X, Y, Z differ "
              f"from AFNI's at {n_tb} of {sum(afni[c].size for c in 'XYZ')} values")  # fmt: skip
        if original:
            d = work / f"{tag}-original"
            if (
                run_original(
                    ["-prefix", "NULL", "-1Dfile", "p.1D", "-SAVEDISP", "sd.nii", str(src)], d
                )
                != 0
            ):
                print(f"{tag}: the original's -SAVEDISP run failed")
                continue
            own = {c: series(d / f"sd_D{c}.nii") for c in ["X", "Y", "Z", *PRODUCTS]}
            ref = products({c: own[c] for c in "XYZ"})
            off = {p: int(np.count_nonzero(ref[p] != own[p])) for p in PRODUCTS}
            print(f"{tag}: the original's 16 products against §10.3's of its own normalised X, Y, Z: "
                  f"{sum(off.values())} values differ" + (f" {off}" if any(off.values()) else ""))  # fmt: skip
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
