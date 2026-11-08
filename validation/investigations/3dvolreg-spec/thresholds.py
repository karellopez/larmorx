# SPDX-License-Identifier: Apache-2.0
"""The thresholds of ``specs/3dvolreg.md`` §14.3, checked on AFNI's binary and the spec's model.

Usage (inside a memory box; one case per call, so several can run side by side):

    scripts/orchestration/boxed.sh 4 python -P validation/investigations/3dvolreg-spec/thresholds.py \
        <work folder> <case>
    python -P validation/investigations/3dvolreg-spec/thresholds.py <work folder> --summary

For each case, AFNI 26.3.05's 3dvolreg runs through the oracle cache (``afni_run.py``) and the
model (``model.py``) runs three times, once per way of breaking §5.3's exact ties (``TIES``:
the smallest computed norm, the first and the last of the tied orders): three results a
clean-room implementation may get from the spec alone. Each is held to §14.3's requirements for
the parameters (per volume and the median), the matrices, the maximum displacements and their
deltas, the registered volumes and, for the cases marked so, ``-savedisp``'s displacements and
``-SAVEDISP``'s normalised displacements where they are well conditioned. The derived thresholds
follow from each volume's own parameter differences (§14.3, "Derived quantities"). Written from
the spec and black-box runs only.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import afni_run as A  # noqa: E402
import check as C  # noqa: E402
import model as M  # noqa: E402

INPUTS = A.WORKSPACE / "oracles/inputs/afni-volreg"
SLICED = INPUTS / "fmriprep"
FMRIPREP = C.FMRIPREP

#: name -> (input, AFNI's options, the model's settings, savedisp check)
CASES = {
    "s1-heptic": ("s1.nii.gz", (), {}, True),
    "s1-linear": ("s1.nii.gz", ("-linear",), {"method": "linear"}, True),
    "s1-cubic": ("s1.nii.gz", ("-cubic",), {"method": "cubic"}, False),
    "s1-quintic": ("s1.nii.gz", ("-quintic",), {"method": "quintic"}, False),
    "s1-fourier": ("s1.nii.gz", ("-Fourier",), {"method": "fourier"}, False),
    "s1-twopass": ("s1.nii.gz", ("-twopass",), {"twopass": True}, False),
    "s1-linear-twopass": (
        "s1.nii.gz",
        ("-linear", "-twopass"),
        {"method": "linear", "twopass": True},
        False,
    ),
    "s1-fmriprep": ("s1.nii.gz", FMRIPREP, {"method": "fourier", "twopass": True}, False),
    "syn-heptic": ("syn", (), {}, False),
    "syn-linear": ("syn", ("-linear",), {"method": "linear"}, False),
    "ds000005-heptic": ("ds000005-run-01_sliced.nii.gz", (), {}, True),
    "ds000005-linear": ("ds000005-run-01_sliced.nii.gz", ("-linear",), {"method": "linear"}, True),
    "ds000005-fmriprep": (
        "ds000005-run-01_sliced.nii.gz",
        FMRIPREP,
        {"method": "fourier", "twopass": True},
        False,
    ),
    "ds003592-linear": ("ds003592-echo-1_sliced.nii.gz", ("-linear",), {"method": "linear"}, False),
    "ds003592-fmriprep": (
        "ds003592-echo-1_sliced.nii.gz",
        FMRIPREP,
        {"method": "fourier", "twopass": True},
        False,
    ),
}


def input_path(kind: str, work: Path) -> Path:
    if kind == "syn":
        p = work / "syn.nii.gz"
        if not p.exists():
            C.moving_series(p)
        return p
    if kind.startswith("ds"):
        return SLICED / kind
    return INPUTS / kind


def lines(path: Path) -> list[str]:
    return [
        x
        for x in path.read_text(encoding="utf-8").splitlines()
        if x.strip() and not x.startswith("#")
    ]


def numbers(path: Path) -> np.ndarray:
    return np.array([[float(v) for v in x.split()] for x in lines(path)])


def rotation(roll, pitch, yaw):
    """§4.5's R (DICOM order) from the reported angles (degrees)."""
    f = math.pi / 180.0
    return M.R3(2, -f * float(roll), 0, -f * float(pitch), 1, -f * float(yaw))


def displacements(grid: M.Grid, params) -> np.ndarray:
    """§10.3's -savedisp values (AFNI's P, as in reproduce) per volume: u[v, k, j, i, axis]."""
    nx, ny, nz = grid.n
    P = grid.P()
    i, j, k = np.meshgrid(np.arange(nx), np.arange(ny), np.arange(nz), indexing="ij")
    X = np.stack(
        [
            (grid.org[a] + (idx.astype(np.float32) * grid.step[a]).astype(np.float32)).astype(
                np.float32
            )
            for a, idx in enumerate((i, j, k))
        ],
        axis=-1,
    ).astype(np.float64)
    c = grid.axis_centre().astype(np.float64)
    out = []
    for roll, pitch, yaw, dS, dL, dP in params:
        Rd = P.T @ rotation(roll, pitch, yaw) @ P
        td = P.T @ np.array([dL, dP, dS], np.float32).astype(np.float64)
        q = (X - c) @ Rd.T + td + c
        out.append((X - q).transpose(2, 1, 0, 3))
    return np.stack(out)


def detrend_normmax(series: np.ndarray) -> np.ndarray:
    """§10.3's normalisation along axis 0 (the volumes), in double: an approximation of the
    spec's float32 arithmetic, which the bound it is checked against allows for."""
    n = series.shape[0]
    t = np.arange(n, dtype=np.float64).reshape((n,) + (1,) * (series.ndim - 1))
    x = series.astype(np.float64)
    if n >= 3:
        tm = t.mean()
        b = ((t - tm) * (x - x.mean(axis=0))).sum(axis=0) / ((t - tm) ** 2).sum()
        x = x - (x.mean(axis=0) + b * (t - tm))
    m = np.abs(x).max(axis=0)
    return np.where(m > 0, x / np.where(m > 0, m, 1), x)


def axis_steps(grid: M.Grid) -> dict[str, float]:
    """The voxel size along S, L and P (the axes the reported shifts dS, dL, dP run along)."""
    out = {}
    for n, o in enumerate(grid.orient):
        letter = {"R2L": "L", "L2R": "L", "P2A": "P", "A2P": "P", "I2S": "S", "S2I": "S"}[o]
        out[letter] = float(grid.delta[n])
    return out


def run_case(name: str, work: Path) -> dict:
    kind, opts, kw, savedisp = CASES[name]
    inp = input_path(kind, work)
    d = work / name
    args = [
        *opts,
        "-prefix",
        "a.nii",
        "-1Dfile",
        "a.1D",
        "-1Dmatrix_save",
        "a",
        "-maxdisp1D",
        "a_md.1D",
    ]
    if savedisp:
        args += ["-savedisp", "a_sd.nii"]
    r = A.run([*args, str(inp)], d)
    if r.status != 0:
        raise RuntimeError(r.stderr[-400:])
    if savedisp:
        r2 = A.run([*opts, "-prefix", "NULL", "-SAVEDISP", "a_SD.nii", str(inp)], d / "SAVEDISP")
        if r2.status != 0:
            raise RuntimeError(r2.stderr[-400:])
    img = nib.load(inp)
    grid = M.read_grid(img)
    linear = kw.get("method") == "linear"
    k_prop = 0.03 if linear else 0.005
    med_max = 0.005 if linear else 0.001
    fourier = kw.get("method") == "fourier"
    st = axis_steps(grid)
    thr = np.array([0.02, 0.02, 0.02, 0.01 * st["S"], 0.01 * st["L"], 0.01 * st["P"]])
    pa = numbers(d / "a.1D")
    ma = numbers(d / "a.aff12.1D")
    mda = np.array([float(x) for x in lines(d / "a_md.1D")])
    dea = np.array([float(x) for x in lines(d / "a_md_delt.1D")])
    ad = np.asarray(nib.load(d / "a.nii").dataobj).astype(np.float64)
    if ad.ndim == 3:
        ad = ad[..., None]
    cb = grid.centre().astype(np.float64)
    r_half = 0.5 * math.sqrt(sum(((grid.n[a] - 1) * float(grid.delta[a])) ** 2 for a in range(3)))
    out = {"case": name, "variants": {}}
    for ties in ("smallest", "first", "last"):
        M.TIES = ties
        try:
            res = M.volreg(str(inp), M.Settings(**kw))
        finally:
            M.TIES = "smallest"
        M.write_1d(res, d / f"m_{ties}.1D")
        M.write_matrices(res, d / f"m_{ties}.aff12.1D")
        pm = numbers(d / f"m_{ties}.1D")
        mm = numbers(d / f"m_{ties}.aff12.1D")
        dp = np.abs(pm - pa)
        fails = {}
        # parameters
        allow = thr + k_prop * np.abs(pa)
        fails["parameters"] = int(np.sum(dp > allow + 1e-9))
        fails["median"] = int(np.median(dp) > med_max)
        # derived quantities, from each volume's own parameter differences
        alpha = (math.pi / 180.0) * np.sum(dp[:, :3] + 1e-4, axis=1)
        tau = np.sqrt(np.sum((dp[:, 3:] + 1e-4) ** 2, axis=1))
        rot = np.abs(mm - ma)[:, [0, 1, 2, 4, 5, 6, 8, 9, 10]].max(axis=1)
        fails["matrix rotation"] = int(np.sum(rot > alpha + 1e-6))
        tlen = np.sqrt(np.sum(pa[:, 3:] ** 2, axis=1))
        uabs = np.maximum(np.abs(ma), np.abs(mm))[:, [3, 7, 11]].max(axis=1)
        tra = np.abs(mm - ma)[:, [3, 7, 11]].max(axis=1)
        tra_allow = (
            tau
            + alpha * (tlen + np.linalg.norm(cb))
            + 1e-6 * (tlen + 2 * np.linalg.norm(cb))
            + 1e-5 * uabs
        )
        fails["matrix translation"] = int(np.sum(tra > tra_allow))
        md_allow = alpha * r_half + tau + 0.001
        fails["maxdisp"] = int(np.sum(np.abs(np.array(res.maxdisp) - mda) > md_allow))
        de_allow = md_allow + np.concatenate([[0.0], md_allow[:-1]])
        fails["maxdisp delta"] = int(np.sum(np.abs(np.array(res.maxdelta) - dea) > de_allow))
        # registered volumes: per volume, the measures §14.3 uses
        mdat = np.stack([v.transpose(2, 1, 0) for v in res.registered], axis=3).astype(np.float64)
        big = float(np.abs(ad).max())
        per_volume = []
        for v in range(ad.shape[3]):
            a, b = ad[..., v], mdat[..., v]
            ac, bc = a - a.mean(), b - b.mean()
            den = math.sqrt((ac * ac).sum() * (bc * bc).sum())
            ncc = float((ac * bc).sum() / den) if den > 0 else 1.0
            diff = np.abs(a - b) / big
            per_volume.append(
                {
                    "params within 1e-4": bool(np.all(dp[v] <= 1e-4 + 1e-9)),
                    "identical": bool(np.all(a == b)),
                    "max": float(diff.max()),
                    "p99.9": float(np.quantile(diff, 0.999)),
                    "mean": float(diff.mean()),
                    "ncc": ncc,
                }
            )
        fails["volumes"] = sum(not volume_ok(x, fourier) for x in per_volume)
        worst_ncc = min(x["ncc"] for x in per_volume)
        if tuple(opts) == tuple(FMRIPREP):
            # RobustAverage's reference: the median of the registered volumes (§2), from AFNI's
            # volumes (fMRIPrep's own output, byte for byte) and from the model's
            med_a = np.median(ad.astype(np.float32), axis=3)
            med_m = np.median(mdat.astype(np.float32), axis=3)
            rel = float(np.abs(med_a - med_m).max() / np.abs(med_a).max())
            # §14.3 holds fMRIPrep's real inputs (§14.2) to it; on s1 it is reported
            if kind.startswith("ds"):
                fails["fMRIPrep's median"] = int(rel > 1e-4)
            fmriprep_median = rel
        else:
            fmriprep_median = None
        info = {
            "max |d| angle": float(dp[:, :3].max()),
            "max |d| shift": float(dp[:, 3:].max()),
            "median": float(np.median(dp)),
            "max proportional need": float(np.max((dp - thr) / np.maximum(np.abs(pa), 1e-12))),
            "matrix rotation max": float(rot.max()),
            "matrix rotation old threshold (1e-5) fails": int(np.sum(rot > 1e-5)),
            "maxdisp max |d|": float(np.abs(np.array(res.maxdisp) - mda).max()),
            "maxdisp old threshold (0.01) fails": int(
                np.sum(np.abs(np.array(res.maxdisp) - mda) > 0.01)
            ),
            "worst NCC": worst_ncc,
            "volumes": per_volume,
            "fourier": fourier,
            "fMRIPrep's median max |diff| / max": fmriprep_median,
        }
        if savedisp:
            ua = np.stack(
                [
                    np.asarray(nib.load(d / f"a_sd_D{c}.nii").dataobj, dtype=np.float64)
                    for c in "XYZ"
                ],
                axis=-1,
            ).transpose(3, 2, 1, 0, 4)  # [v, k, j, i, axis]
            um = displacements(grid, res.params).astype(np.float32).astype(np.float64)
            nx, ny, nz = grid.n
            i, j, k = np.meshgrid(np.arange(nx), np.arange(ny), np.arange(nz), indexing="ij")
            X = np.stack(
                [
                    (
                        grid.org[a] + (idx.astype(np.float32) * grid.step[a]).astype(np.float32)
                    ).astype(np.float64)
                    for a, idx in enumerate((i, j, k))
                ],
                axis=-1,
            ).transpose(2, 1, 0, 3)
            dist = np.linalg.norm(X - grid.axis_centre().astype(np.float64), axis=-1)
            bound = alpha[:, None, None, None] * dist[None] + tau[:, None, None, None]
            bound = bound[..., None] + 1e-6 * (1 + np.abs(ua))
            fails["savedisp"] = int(np.sum(np.abs(um - ua) > bound))
            # -SAVEDISP: the three normalised displacements where the normalisation is well conditioned
            na = np.stack(
                [
                    np.asarray(
                        nib.load(d / "SAVEDISP" / f"a_SD_D{c}.nii").dataobj, dtype=np.float64
                    )
                    for c in "XYZ"
                ],
                axis=-1,
            ).transpose(3, 2, 1, 0, 4)
            nm = detrend_normmax(um)
            m_afni = np.abs(detrend_normmax_raw(ua)).max(axis=0)
            t_vox = bound.max(axis=0)
            good = m_afni > 30 * t_vox
            sd_allow = 6 * t_vox / np.maximum(m_afni - 3 * t_vox, 1e-30)
            fails["SAVEDISP (well conditioned)"] = int(
                np.sum((np.abs(nm - na) > sd_allow[None] + 1e-6) & good[None])
            )
            info["SAVEDISP voxels compared"] = f"{int(good.sum())}/{good.size}"
            info["SAVEDISP max |d| everywhere"] = float(np.abs(nm - na).max())
        out["variants"][ties] = {"fails": fails, "info": info}
    return out


def volume_ok(x: dict, fourier: bool) -> bool:
    """§14.3's registered float32 volumes: every volume NCC >= 0.9998 and mean |diff| <= 2e-3 of
    AFNI's largest value, whether or not its parameters agree (the output's own resampling meets
    §5.3's ties); the largest single difference is reported, not required."""
    return x["ncc"] >= 0.9998 and x["mean"] <= 2e-3


def detrend_normmax_raw(series: np.ndarray) -> np.ndarray:
    """The detrended series before the division by its largest value (for m)."""
    n = series.shape[0]
    t = np.arange(n, dtype=np.float64).reshape((n,) + (1,) * (series.ndim - 1))
    x = series.astype(np.float64)
    if n >= 3:
        tm = t.mean()
        b = ((t - tm) * (x - x.mean(axis=0))).sum(axis=0) / ((t - tm) ** 2).sum()
        x = x - (x.mean(axis=0) + b * (t - tm))
    return x


def summary(work: Path) -> int:
    bad = 0
    for f in sorted(work.glob("*.json")):
        o = json.loads(f.read_text(encoding="utf-8"))
        for ties, v in o["variants"].items():
            vols = v["info"]["volumes"]
            v["fails"]["volumes"] = sum(not volume_ok(x, v["info"]["fourier"]) for x in vols)
            agree = [x for x in vols if x["params within 1e-4"] and not x["identical"]]
            other = [x for x in vols if not x["params within 1e-4"]]
            v["info"]["volume measures"] = (
                f"agreeing {len(agree)}: NCC>={min((x['ncc'] for x in agree), default=1):.7f}"
                f" mean<={max((x['mean'] for x in agree), default=0):.1e} max<={max((x['max'] for x in agree), default=0):.1e};"
                f" others {len(other)}: NCC>={min((x['ncc'] for x in other), default=1):.6f}"
                f" mean<={max((x['mean'] for x in other), default=0):.1e} max<={max((x['max'] for x in other), default=0):.1e}"
            )
            nf = sum(v["fails"].values())
            bad += nf
            i = v["info"]
            med = i.get("fMRIPrep's median max |diff| / max")
            print(
                f"{o['case']:20s} {ties:8s} fails {nf:3d}  angle {i['max |d| angle']:.4f} shift {i['max |d| shift']:.4f}"
                f" median {i['median']:.4f}  matrix rot {i['matrix rotation max']:.1e} (old 1e-5 fails"
                f" {i['matrix rotation old threshold (1e-5) fails']})  maxdisp {i['maxdisp max |d|']:.4f} (old 0.01"
                f" fails {i['maxdisp old threshold (0.01) fails']})  NCC {i['worst NCC']:.6f}"
                + (
                    f"  SAVEDISP {i['SAVEDISP voxels compared']}"
                    if "SAVEDISP voxels compared" in i
                    else ""
                )
                + (f"  fMRIPrep's median {med:.1e}" if med is not None else "")
            )
            print("       volumes:", v["info"]["volume measures"])
            if nf:
                print("       FAILS:", {k: n for k, n in v["fails"].items() if n})
    print(f"total failures: {bad}")
    return 1 if bad else 0


def main() -> int:
    work = Path(sys.argv[1]).resolve()
    work.mkdir(parents=True, exist_ok=True)
    if sys.argv[2] == "--summary":
        return summary(work)
    for name in sys.argv[2:]:
        res = run_case(name, work)
        (work / f"{name}.json").write_text(json.dumps(res, indent=1), encoding="utf-8")
        for ties, v in res["variants"].items():
            print(name, ties, v["fails"], flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
