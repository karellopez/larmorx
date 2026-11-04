# SPDX-License-Identifier: Apache-2.0
"""`-roc`'s arithmetic (findings 19): a numpy model of §15 against FSL's outputs, kept by
``fsl_outputs.py <dir> roc/noise roc/truth roc/never-reached roc/3d roc/double roc/short
d13/roc-noise-larger "args:rocA:@probe:rocramp -roc -2 roc.txt @probe:roctruthx out"
"args:rocB:@probe:rocramp -roc 2 roc.txt @probe:rocramp @probe:roctruthx out"
"args:rocE:@rocim -roc 0.05 roc.txt @rocnoise @rocim out"
"args:rocF:@rocim3 -roc 0.05 roc.txt @rocnoise @roctruth out"
"args:rocG:@probe:rocramp -roc -0.5 roc.txt @probe:roctruthsmallneg out"``.

Run: ``python -P fit_roc.py <dir> [variant]``. Prints, per kept run, the curve file's lines that
differ from the model's (C++'s default format, 6 significant digits) and the printed area and
quartiles of FSL and the model. ``variant`` replaces one choice of the model by the alternative
the findings measured: ``own-axis`` (each volume's area on its own false-positive rate),
``masked-noise`` (the truth's mask applied to the noise counts), ``float-means`` (the rates'
means summed in single precision), ``double-compare`` (a double image's log values compared in
double), ``zero-candidates`` (the false-positive candidates the truth's voxels equal to 0).
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np

F = np.float32
#: kept run -> (L, noise, truth, input)
RUNS = {
    "roc_noise": (0.05, "rocnoise", "roctruth", "rocim"),
    "roc_truth": (-0.05, None, "roctruth", "rocim"),
    "roc_never-reached": (2, "rocnoise", "roctruth", "rocim"),
    "roc_3d": (-0.1, None, "roctruth", "rocim3"),
    "roc_double": (0.05, "rocnoise", "roctruth", "rocimd"),
    "roc_short": (-0.05, None, "roctruth", "rocims"),
    "d13_roc-noise-larger": (0.05, "rocnoise4", "roctruth", "rocim"),
    "rocA": (-2, None, "probe_roctruthx", "probe_rocramp"),
    "rocB": (2, "probe_rocramp", "probe_roctruthx", "probe_rocramp"),
    "rocE": (0.05, "rocnoise", "rocim", "rocim"),
    "rocF": (0.05, "rocnoise", "roctruth", "rocim3"),
    "rocG": (-0.5, None, "probe_roctruthsmallneg", "probe_rocramp"),
}


def _load(d: Path, name: str) -> np.ndarray:
    import nibabel as nib

    return np.asarray(nib.load(str(d / f"{name}.nii.gz")).dataobj)


def _mean(v, variant: str) -> float:
    if variant == "float-means":
        s = F(0)
        for x in v:
            s = F(s + F(x))
        return float(F(s / F(len(v))))
    return float(F(sum(float(x) for x in v) / len(v)))


def model(d: Path, L: float, noise_name, truth_name: str, input_name: str, variant: str):
    raw = _load(d, input_name)
    dbl = raw.dtype == np.float64
    im = raw.astype(np.float64 if dbl else F)
    if im.ndim == 3:
        im = im[..., None]
    truth = _load(d, truth_name).astype(F)
    if truth.ndim == 4:
        truth = truth[..., 0]
    noise = _load(d, noise_name).astype(F) if noise_name else None
    minval = im.min()
    if noise is not None:
        minval = min(minval, F(noise.min()))
    if dbl:
        lg = np.log(im - minval + 1.0)
        if variant != "double-compare":
            lg = lg.astype(F)
    else:
        lg = np.log((im - F(minval) + F(1)).astype(F)).astype(F)
    maxlog = lg.max()
    nl = None
    if noise is not None:
        # indexed with the input's sizes (FSL-179): the offsets past them keep their raw values
        flat = (noise if noise.ndim == 4 else noise[..., None]).reshape(-1, order="F").copy()
        k = im.size
        flat[:k] = np.log((flat[:k] - F(minval) + F(1)).astype(F)).astype(F)
        maxlog = max(maxlog, flat.max())
        nl = flat[:k].reshape(im.shape, order="F")
    maxlog = F(maxlog)
    delta = F(maxlog / F(1000)) if maxlog != 0 else F(1)
    mask = truth >= F(-0.001)
    true = (truth >= F(0.05 * float(truth.max()))) & mask
    cand = ((truth == 0) if variant == "zero-candidates" else ~(truth > 0)) & mask
    nvox, nt, al = truth.size, im.shape[3], abs(L)
    lines, area, vareas = [], 0.0, np.zeros(nt)
    fp = tp = 0.0
    tps = fps = np.zeros(nt)
    h = F(maxlog + delta)
    while h > 0 and fp < al:
        tp_t = np.array([F(F((lg[..., t] >= h)[true].sum()) / F(true.sum())) for t in range(nt)])
        thr = float(F(F(math.exp(float(h))) - F(1)) + F(minval))
        if nl is None:
            fp_t = np.array(
                [F(F((lg[..., t] >= h)[cand].sum()) / F(cand.sum())) for t in range(nt)]
            )
            lines += [f"{a:g} {b:g} {thr:g}" for a, b in zip(fp_t, tp_t, strict=True)]
            new_fp = _mean(fp_t, variant)
        else:
            sel = mask if variant == "masked-noise" else np.ones_like(mask)
            c = np.array([(nl[..., t] >= h)[sel].sum() for t in range(nt)])
            new_fp = float(F(F((c > 0).sum()) / F(nt)))
            rates = np.array([F(F(x) / F(nvox)) for x in c], dtype=np.float64)
            sdf = rates.std(ddof=1) if nt > 1 else 0.0
            sdt = tp_t.astype(np.float64).std(ddof=1) if nt > 1 else 0.0
            lines.append(f"{new_fp:g} {_mean(tp_t, variant):g} {sdf:g} {sdt:g} {thr:g}")
            fp_t = np.full(nt, new_fp)
        new_tp = _mean(tp_t, variant)
        axis = (fps, fp_t) if variant == "own-axis" else (np.full(nt, fp), np.full(nt, new_fp))
        if new_fp < al:
            area += (new_fp - fp) * (new_tp + tp) / 2
        else:
            at = tp + (new_tp - tp) * (al - fp) / (new_fp - fp)
            area += (al - fp) * (tp + at) / 2
        for t in range(nt):
            x0, x1 = axis[0][t], axis[1][t]
            if variant == "own-axis" and x0 >= al:
                continue
            if x1 < al:
                vareas[t] += (x1 - x0) * (tps[t] + tp_t[t]) / 2
            else:
                at = tps[t] + (tp_t[t] - tps[t]) * (al - x0) / (x1 - x0)
                vareas[t] += (al - x0) * (tps[t] + at) / 2
        fp, tp, tps, fps = new_fp, new_tp, tp_t, fp_t
        h = F(h - delta)
    if fp < al:
        # never reached: the rectangle up to |L| at the last true-positive rates
        area += (al - fp) * tp
        vareas += (al - fp) * tps
    v = np.sort(vareas)
    n = len(v)
    xs = [0.0] + [100 * (i + 0.5) / n for i in range(n)] + [100.0]
    ys = [v[0], *v, v[-1]]
    printed = (area / al, np.interp(25, xs, ys) / al, np.interp(75, xs, ys) / al)
    return lines, " ".join(f"{x:g}" for x in printed)


def main(argv: list[str]) -> int:
    base = Path(argv[0])
    variant = argv[1] if len(argv) > 1 else ""
    for label, (L, noise, truth, inp) in RUNS.items():
        d = base / label
        if not d.exists():
            continue
        lines, printed = model(d, L, noise, truth, inp, variant)
        fsl = (d / "roc.txt").read_text().splitlines()
        bad = sum(a != b for a, b in zip(fsl, lines, strict=False)) + abs(len(fsl) - len(lines))
        out = (d / "RUN.txt").read_text().split("--stdout\n")[1].split("\n")[0]
        print(
            f"{label}: curve {bad} of {len(fsl)} lines differ; printed FSL {out} / model {printed}"
        )
    return 0


if __name__ == "__main__":
    with np.errstate(over="ignore", invalid="ignore", divide="ignore"):
        sys.exit(main(sys.argv[1:]))
