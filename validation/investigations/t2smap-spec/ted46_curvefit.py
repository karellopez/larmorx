# SPDX-License-Identifier: Apache-2.0
"""TED-46 under ``--fittype curvefit`` (rule D: measure first; spec writer 4, 2026-11-12).

On the voxels whose log-linear observations are all equal, finite and not all 0 (in a group of
lstsq's rank 2: ``specs/t2smap.md`` §7.2), four ways of running t2smap's curve fit, everything
else tedana's (``model.py``, the spec's model, in its ``exact`` variant):

- ``tedana``: tedana's batched least squares (its rounding noise as the slope, §7.2), the curve
  fit started there (§7.3);
- ``default``: TED-46's exact slope 0 (T2* = +inf), the curve fit started there, which SciPy
  refuses (§7.3 item 4): a failure that keeps the log-linear S0 = exp(log(x + 1)) = x + 1;
- ``finite``: the same, but the curve fit of those voxels started at T2* = 500 ms (the value §8
  gives +inf), so that it converges;
- ``truth``: the exact solution of the curve fit on equal observations x: S0 = x, T2* = +inf (no
  decay; §8 then writes 500 ms), no failure, the variances SciPy's rule gives a zero residual
  (0 with more than two samples, +inf with two).

Each variant's maps go through §8's floors, limited maps and cap (``model.floors``, ``model.cap``)
and §9's RMSE (``model.rmse_series``); the report compares them, per case, at those voxels (S0,
T2*, the failures) and at every other voxel (the limited T2* that the cap changes, the RMSE map).
The cases: replayed trial cases (``<work>/f<n>/{inputs,tedana/out}``, as ``no_slope_cases.py``
takes them; tedana's own files check that the ``tedana`` variant is tedana's) whose call has
``--fittype curvefit``, and synthetic series written here (decaying voxels with noise and a block
of samples clipped at 255 in every echo and volume: 0.2 %, 1 % and 5 % of the mask)::

    python -I ted46_curvefit.py <this folder> <work> <out.json> [f<n> ...]

Run with ``oracles/python-newest``'s Python in the reference setup (``OPENBLAS_NUM_THREADS=1
OPENBLAS_CORETYPE=Haswell``, numpy's AVX-512 kernels off), boxed.
"""

from __future__ import annotations

import json
import sys
import warnings
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, sys.argv[1])
import check
import model

F32, F64 = np.float32, np.float64
VARIANTS = ("tedana", "default", "finite", "truth")


def flat_columns(xg: np.ndarray, tes: np.ndarray) -> np.ndarray:
    """§7.2's TED-46 voxels of a group: observations equal, finite, not all 0, rank 2."""
    n_vox, n, tu = xg.shape
    if n_vox == 0 or tu == 0 or len(set(tes[:n].tolist())) < 2:
        return np.zeros(n_vox, dtype=bool)
    with np.errstate(divide="ignore", invalid="ignore"):
        y = np.log(np.abs(xg) + 1).reshape(n_vox, n * tu)
    first = y[:, :1]
    return np.isfinite(first[:, 0]) & (first[:, 0] != 0) & np.all(y == first, axis=1)


def fit(x_used, count, tes, variant: str, marks: np.ndarray):
    """``model.fit`` (curvefit, exact) with the variant's handling of the TED-46 voxels; marks
    them in ``marks``."""
    n_vox = x_used.shape[0]
    t2s = np.zeros(n_vox)
    s0 = np.zeros(n_vox)
    failed = np.zeros(n_vox, dtype=bool)
    var = np.zeros((3, n_vox))
    for n, members in model.groups_of(count):
        xg = x_used[members, :n, :]
        g_t2s, g_s0 = model.loglin_group(xg, tes, "exact")
        flat = flat_columns(xg, tes)
        marks[members[flat]] = True
        if variant != "tedana":
            with np.errstate(divide="ignore"):
                y = np.log(np.abs(xg[flat, 0, 0]) + 1).astype(F64)
                g_t2s[flat] = 1.0 / np.zeros(flat.sum())
                g_s0[flat] = np.exp(y)
        t2s[members] = g_t2s
        s0[members] = g_s0
        tu = xg.shape[2]
        te = np.repeat(tes[:n], tu)
        for j, v in enumerate(members):
            one = count[v] == 1
            if flat[j] and variant == "truth":
                xv = float(xg[j, 0, 0])
                s0[v], t2s[v] = xv, np.inf
                if not one:
                    var[:, v] = 0.0 if n * tu > 2 else np.inf
                continue
            p0 = (g_s0[j], g_t2s[j])
            if flat[j] and variant == "finite":
                p0 = (g_s0[j], 500.0)
            res = model.curvefit_voxel(xg[j].reshape(-1), te, p0, "exact")
            if res is None:
                failed[v] = not one
                continue
            s0[v], t2s[v], pcov = res
            if not one:
                var[:, v] = (pcov[0, 0], pcov[1, 1], pcov[0, 1])
    return t2s, s0, failed, var


def count_flat(c: dict, fitmode: str) -> int:
    """The voxels (voxel-volume pairs in ``ts``) §7.2's TED-46 concerns, without the fits."""
    x = c["x"] if fitmode == "ts" else c["x_used"]
    cols = [x[:, :, t : t + 1] for t in range(x.shape[2])] if fitmode == "ts" else [x]
    k = 0
    for xc in cols:
        for n, members in model.groups_of(c["count"]):
            k += int(flat_columns(xc[members, :n, :], c["tes"]).sum())
    return k


def run_variant(c: dict, fitmode: str, variant: str) -> dict:
    x, x_used, tes, count = c["x"], c["x_used"], c["tes"], c["count"]
    n_t = x.shape[2]
    marks = np.zeros(count.size, dtype=bool)
    if fitmode == "ts":
        cols = [fit(x[:, :, t : t + 1], count, tes, variant, marks) for t in range(n_t)]
        t2s = np.stack([k[0] for k in cols], axis=1)
        s0 = np.stack([k[1] for k in cols], axis=1)
        failed = np.stack([k[2] for k in cols], axis=1)
    else:
        t2s, s0, failed, _ = fit(x_used, count, tes, variant, marks)
    fv = count >= 1
    t2s_f, s0_f = t2s[fv], s0[fv]
    model.floors(t2s_f, s0_f, tes)
    one = count[fv] == 1
    t2s_l, s0_l = t2s_f.copy(), s0_f.copy()
    t2s_l[one] = 0
    s0_l[one] = 0
    c_val = model.percentile_lower(t2s_l.reshape(-1), 99.5)
    model.cap(t2s_l)

    def on_s(vals):
        full = np.zeros((count.size, *vals.shape[1:]))
        full[fv] = vals
        return full

    t2s_lim, s0_lim = on_s(t2s_l), on_s(s0_l)
    r = model.rmse_series(x, count, t2s_lim, s0_lim, tes, "exact")
    return dict(
        marks=marks,
        t2s_full=on_s(t2s_f),
        s0_full=on_s(s0_f),
        t2s_lim=t2s_lim,
        failed=failed,
        cap=float(c_val),
        rmse=model.rmse_map(r, "exact"),
        x0=x[:, 0, 0] if x.shape[2] else np.zeros(count.size),
    )


def rel(a, b):
    with np.errstate(divide="ignore", invalid="ignore"):
        return np.abs(a - b) / np.abs(b)


def summarise(name: str, res: dict, fitmode: str) -> dict:
    truth = res["truth"]
    marks = truth["marks"]
    k = int(marks.sum())
    fit_vox = int(np.sum(truth["t2s_full"].reshape(len(marks), -1)[:, 0] != 0))
    out: dict = dict(case=name, fitmode=fitmode, ted46_voxels=k, fit_voxels=fit_vox)
    other = ~marks
    for v in VARIANTS:
        r = res[v]
        t2 = r["t2s_full"][marks].reshape(-1)
        s0 = r["s0_full"][marks].reshape(-1)
        s0_truth = truth["s0_full"][marks].reshape(-1)
        fail = r["failed"][marks].reshape(-1)
        lim_o, lim_ot = r["t2s_lim"][other], truth["t2s_lim"][other]
        rm_o, rm_ot = r["rmse"][other], truth["rmse"][other]
        with np.errstate(divide="ignore", invalid="ignore"):
            rm_rel = r["rmse"][marks] / np.abs(r["x0"][marks])
        out[v] = dict(
            cap_ms=r["cap"],
            t2s_ms={
                "500 (no decay)": int(np.sum(t2 == 500)),
                "1 (a negative start floored)": int(np.sum(t2 == 1)),
                "above 1e6": int(np.sum(t2 > 1e6)),
                "other": int(np.sum((t2 != 500) & (t2 != 1) & ~(t2 > 1e6))),
                "median": float(np.median(t2)) if t2.size else None,
            },
            s0_minus_x={
                "max": float(np.max(s0 - s0_truth)) if s0.size else None,
                "min": float(np.min(s0 - s0_truth)) if s0.size else None,
            },
            failures=int(fail.sum()),
            other_limited_t2s_unlike_truth=int(np.sum(~same(lim_o, lim_ot))),
            other_rmse_unlike_truth=int(np.sum(~same(rm_o, rm_ot))),
            rmse_over_x_median=float(np.nanmedian(rm_rel)) if k else None,
        )
    return out


def same(a, b) -> np.ndarray:
    return (a == b) | (np.isnan(a) & np.isnan(b))


def synthetic(work: Path) -> list[tuple[str, list[str]]]:
    """Seeded decaying series (3 echoes, 12/28/44 ms, 40 volumes, 20 x 20 x 10 voxels) with a
    block clipped at 255 in every echo and volume: 8, 40 and 200 voxels of the 4,000."""
    rng = np.random.default_rng(20261112)
    shape = (20, 20, 10, 40)
    tes = [0.012, 0.028, 0.044]
    s0 = 150 + 80 * rng.random(shape[:3])
    t2 = 0.02 + 0.06 * rng.random(shape[:3])
    aff = np.diag([3.0, 3.0, 3.5, 1.0])
    cases = []
    for k in (8, 40, 200):
        d = work / f"syn-sat{k}"
        d.mkdir(parents=True, exist_ok=True)
        flat = np.zeros(shape[:3], dtype=bool)
        flat.reshape(-1)[rng.choice(flat.size, size=k, replace=False)] = True
        names = []
        for i, te in enumerate(tes):
            sig = (s0 * np.exp(-te / t2))[..., None] * (1 + 0.02 * rng.standard_normal(shape))
            sig[flat] = 255.0
            p = d / f"e{i + 1}.nii.gz"
            nib.Nifti1Image(sig.astype(F32), aff).to_filename(p)
            names.append(str(p))
        nib.Nifti1Image(np.ones(shape[:3], dtype=np.uint8), aff).to_filename(d / "mask.nii.gz")
        cases.append(
            (
                f"syn-sat{k}",
                [
                    "-d",
                    *names,
                    "-e",
                    *(str(t) for t in tes),
                    "--mask",
                    str(d / "mask.nii.gz"),
                    "--fittype",
                    "curvefit",
                    "--out-dir",
                    str(d / "out"),
                ],
            )
        )
    return cases


def main() -> int:
    work = Path(sys.argv[2])
    out_json = Path(sys.argv[3])
    names = sys.argv[4:]
    rows = []
    cases: list[tuple[str, list[str], Path | None]] = []
    for case in sorted(work.glob("f*")):
        if names and case.name not in names:
            continue
        tout = case / "tedana/out"
        if not (tout / "t2smap_call.sh").exists() or not any(tout.glob("*.json")):
            continue
        w = check.call_words(tout)
        if "curvefit" not in w:
            continue
        cases.append((case.name, w, tout))
    syn_dir = out_json.parent / "ted46-synthetic"
    for name, w in synthetic(syn_dir):
        cases.append((name, w, None))
    for name, w, tout in cases:
        a = model.parser().parse_args(w)
        if tout is not None:
            base = tout.parent
            a.data = [str((base / p).resolve()) if not Path(p).is_absolute() else p for p in a.data]
            if a.mask and not Path(a.mask).is_absolute():
                a.mask = str((base / a.mask).resolve())
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                c = model.prepare(a)
                if count_flat(c, a.fitmode) == 0:
                    rows.append(dict(case=name, ted46_voxels=0))
                    continue
                res = {v: run_variant(c, a.fitmode, v) for v in VARIANTS}
        except Exception as e:  # a case the model cannot run: reported
            rows.append(dict(case=name, error=f"{type(e).__name__}: {e}"))
            continue
        row = summarise(name, res, a.fitmode)
        if tout is not None:
            row["tedana_variant_vs_files"] = files_check(res["tedana"], tout, c, a)
        rows.append(row)
        print(json.dumps(row)[:600], flush=True)
    out_json.write_text(json.dumps(rows, indent=1), encoding="utf-8")
    return 0


def files_check(r: dict, tout: Path, c: dict, a) -> dict:
    """How many S0 and limited T2* values of the ``tedana`` variant differ from tedana's files
    (the model's voxels are the mask's in C order of the grid)."""
    prefix = a.prefix if (a.prefix == "" or a.prefix.endswith("_")) else a.prefix + "_"
    idx = np.flatnonzero(np.asarray(c["mask"]).ravel(order="C"))
    out = {}
    for key, desc in (("s0", "s0 img"), ("limited t2s", "limited t2star img")):
        img = np.asanyarray(nib.load(tout / model.name(desc, a.convention, prefix)).dataobj)
        theirs = img.reshape(-1, *img.shape[3:])[idx]
        mine = (r["s0_full"] if key == "s0" else r["t2s_lim"] / 1000).astype(F32)
        out[key] = int(np.sum(~same(mine, theirs)))
    return out


if __name__ == "__main__":
    sys.exit(main())
