# SPDX-License-Identifier: Apache-2.0
"""§14.3's rule for ill-conditioned fits ("The ties' spread"), checked on random testing's findings.

Usage (inside a memory box; the findings of a random-testing night of ``afni-volreg-original``):

    scripts/orchestration/boxed.sh 8 python -P validation/investigations/3dvolreg-spec/spread.py \
        <work folder> <night folder> [--sample N] [--seed S] [--jobs J] [n ...]

For each finding, its input is made again from the recipe (the descriptor's own generator) and
its command line is read into the model's settings (§9.1's option scan, for the options the model
implements; a finding with another option is left out and counted). AFNI 26.3.05's 3dvolreg runs
through the oracle cache (``afni_run.py``) and the model (``model.py``) K + 1 times: the spec's
rule (``TIES = "smallest"``, the centre) and K seeded draws at every resampling whose orders tie
(``TIES = "random:<k>"``). Then §14.3's rule:

- a **runaway** (some volume of AFNI's or of any of the model's K + 1 runs beyond 15 degrees or
  20 mm, or AFNI crashing): counted, its numbers not required (the suite compares its
  structure); an input AFNI refuses with an error is left out (the model has no such checks);
- otherwise every quantity of the centre against AFNI within the larger of its fixed requirement
  and C times its spread (C squared for 1 - NCC): the spread is the largest difference, over the
  K runs and over the case's volumes, between the centre and a seeded run, in the requirement's
  own units (each parameter |difference| / its threshold, the median of all |differences|, each
  volume's mean |diff| / max and 1 - NCC).

Prints the counts and every case that fails. Written from the spec and black-box runs only.
"""

from __future__ import annotations

import json
import math
import random
import sys
from concurrent.futures import ProcessPoolExecutor
from multiprocessing import get_context
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import afni_run as A  # noqa: E402
import model as M  # noqa: E402

K = 12
C = 3.0


def _starts(arg: str, name: str, n: int) -> bool:
    return len(arg) >= n and name.startswith(arg[:n]) and arg.startswith(name[:n])


def settings_of(argv: list[str]) -> tuple[dict | None, str]:
    """§9.1's option scan, in its order, for the options the model implements: the model's
    settings, or None and the first option it does not implement."""
    kw: dict = {}
    i = 0
    while i < len(argv) and argv[i].startswith("-"):
        a = argv[i]
        nxt = argv[i + 1] if i + 1 < len(argv) else ""
        if a in ("-maxdisp", "-nomaxdisp"):
            kw["maxdisp"] = a == "-maxdisp"
        elif a == "-maxdisp1D":
            kw["maxdisp"] = True
            i += 1
        elif a in ("-savedisp", "-SAVEDISP", "-1Dmatrix_save"):
            i += 1
        elif a == "-sinit":
            kw["sinit_on"] = False
        elif _starts(a, "-params", 4):
            return None, a
        elif _starts(a, "-maxite", 4):
            m = int(float(nxt))
            if m > 0:
                kw["maxite"] = m
            i += 1
        elif _starts(a, "-x_thresh", 4):
            v = float(nxt)
            if v > 0:
                kw["x_t"] = float(np.float32(v))
            i += 1
        elif _starts(a, "-rot_thresh", 6):
            v = float(nxt)
            if v > 0:
                kw["phi_t"] = float(np.float32(v))
            i += 1
        elif _starts(a, "-delta", 4):
            v = float(nxt)
            if v > 0:
                kw["dfac"] = float(np.float32(v))
            i += 1
        elif _starts(a, "-final", 4):
            kw["final"] = {"NN": "nn", "Fourier": "fourier"}.get(nxt, nxt)
            i += 1
        elif a == "-rotcom":
            pass
        elif a == "-twopass":
            kw["twopass"] = True
        elif a == "-twoblur":
            v = float(np.float32(float(nxt)))
            if v > 14:
                return None, "-twoblur above 14 (the FFT blur)"
            kw["twoblur"] = v
            i += 1
        elif _starts(a, "-verbose", 5):
            pass
        elif _starts(a, "-clipit", 4):
            kw["clip"] = True
        elif _starts(a, "-noclip", 4):
            kw["clip"] = False  # also -nocoarserot (AFNI-86, reproduce)
        elif _starts(a, "-zpad", 5):
            kw["zpad"] = int(float(nxt))
            i += 1
        elif a[:5].lower() in ("-four", "-line", "-cubi", "-quin", "-hept"):
            kw["method"] = {"-four": "fourier", "-line": "linear", "-cubi": "cubic", "-quin": "quintic", "-hept": "heptic"}[a[:5].lower()]  # fmt: skip
        elif _starts(a, "-prefix", 4) or _starts(a, "-dfile", 4) or _starts(a, "-1Dfile", 4):
            i += 1
        elif _starts(a, "-base", 4):
            try:
                kw["base_index"] = int(nxt)
            except ValueError:
                return None, "-base dataset"
            i += 1
        elif a == "-coarse":
            kw["coarse_del"], kw["coarse_num"] = int(float(nxt)), int(float(argv[i + 2]))
            i += 2
        elif a == "-coarserot":
            kw["coarse_rot"] = True
        elif a == "-wtinp":
            kw["wtinp"] = True
        elif a == "-edging":
            if nxt.endswith("%"):
                kw["edging"] = ("percent", float(nxt[:-1]))
            else:
                kw["edging"] = ("voxels", float(nxt))
            i += 1
        else:
            # -wtrim, -weight, -twodup, -tshift, -rotparent, -gridparent, -float, -input, ...
            return None, a
        i += 1
    return kw, ""


def _numbers(path: Path) -> np.ndarray:
    rows = path.read_text(encoding="utf-8").splitlines()
    return np.array([[float(v) for v in x.split()] for x in rows if x.strip() and x[0] != "#"])


def _thresholds(grid: M.Grid, kw: dict, pa: np.ndarray) -> np.ndarray:
    """§14.3's per-volume parameter thresholds: phi_t + k |angle|, x_t delta_n + k |shift|."""
    k = 0.03 if kw.get("method") == "linear" else 0.005
    step = {}
    for n, o in enumerate(grid.orient):
        step[{"R2L": "L", "L2R": "L", "P2A": "P", "A2P": "P", "I2S": "S", "S2I": "S"}[o]] = float(grid.delta[n])  # fmt: skip
    x_t, phi_t = kw.get("x_t", 0.01), kw.get("phi_t", 0.02)
    base = np.array([phi_t] * 3 + [x_t * step["S"], x_t * step["L"], x_t * step["P"]])
    return base + k * np.abs(pa)


def _measures(pc, vc, pref, vref, thr):
    """Per volume: |d parameter| / threshold (largest), mean |diff| / max, 1 - NCC; and the median
    of all |d parameter|."""
    d = np.abs(pc - pref)
    par = (d / thr).max(axis=1)
    med = float(np.median(d))
    if vc is None or vref is None or vc.shape != vref.shape:
        return par, med, None, None
    mean, inc = [], []
    for v in range(vref.shape[3]):
        a, b = vref[..., v], vc[..., v]
        big = float(np.abs(a).max()) or 1.0
        mean.append(float(np.abs(a - b).mean()) / big)
        ac, bc = a - a.mean(), b - b.mean()
        den = math.sqrt(float((ac * ac).sum() * (bc * bc).sum()))
        inc.append(1.0 - (float((ac * bc).sum()) / den if den > 0 else 1.0))
    return par, med, np.array(mean), np.array(inc)


def one(args) -> dict:
    n, night, work = args
    from larmorx_validation.difftest import inputs
    from larmorx_validation.difftest.tools import afni_volreg_original as tool

    f = json.loads(Path(night, "findings", "afni-volreg-original", f"volreg-original-trial-{int(n):05d}-original.json").read_text())  # fmt: skip
    recipe = f["recipe"]
    argv = tool._argv(recipe, inputs.register(recipe["inputs"]))
    kw, missing = settings_of(argv)
    row = {"n": n, "argv": " ".join(w if "/" not in w else Path(w).name for w in argv)}
    if kw is None:
        row["status"] = f"not modelled ({missing})"
        return row
    oned = next((argv[i + 1] for i, w in enumerate(argv) if w.startswith("-1Df")), None)
    prefix = next((argv[i + 1] for i, w in enumerate(argv) if w.startswith("-pre")), None)
    if oned is None:
        row["status"] = "no -1Dfile"
        return row
    src = Path(argv[-1])
    d = Path(work) / str(n)
    r = A.run(argv, d / "afni")
    if r.status != 0 and "Fatal Signal" not in r.stderr:
        row["status"] = "AFNI refuses the input (not modelled)"
        return row
    if r.status != 0 or not (d / "afni" / oned).is_file():
        p_afni, runaway_afni = None, True  # a crash (AFNI-97's, in a runaway)
    else:
        p_afni = _numbers(d / "afni" / oned)
        runaway_afni = bool(np.abs(p_afni[:, :3]).max() > 15 or np.abs(p_afni[:, 3:]).max() > 20)
    v_afni = None
    if prefix and prefix != "NULL" and (d / "afni" / prefix).is_file():
        v_afni = np.asarray(nib.load(d / "afni" / prefix).dataobj).astype(np.float64)
        if v_afni.ndim == 3:
            v_afni = v_afni[..., None]
    runs = {}
    for ties in ["smallest"] + [f"random:{k}" for k in range(1, K + 1)]:
        M.TIES = ties
        try:
            res = M.volreg(str(src), M.Settings(**kw))
        except Exception as e:  # a failure of the model's run counts as its outcome
            runs[ties] = (None, None, f"{type(e).__name__}")
            continue
        finally:
            M.TIES = "smallest"
        M.write_1d(res, d / f"m_{ties.replace(':', '_')}.1D")
        p = _numbers(d / f"m_{ties.replace(':', '_')}.1D")
        v = np.stack([x.transpose(2, 1, 0) for x in res.registered], axis=3).astype(np.float64)
        runs[ties] = (p, v, "")
    outcomes = {
        k: (p is None) or bool(np.abs(p[:, :3]).max() > 15 or np.abs(p[:, 3:]).max() > 20)
        for k, (p, v, e) in runs.items()
    }
    if runaway_afni or any(outcomes.values()):
        # §14.3 "Runaways": AFNI's or any of the implementation's runs beyond 15 degrees or 20 mm
        # (or AFNI crashing): the numbers are reported, not required (the suite compares the
        # structure)
        row["status"] = "runaway"
        row["passed"] = True
        row["afni"] = "crash" if p_afni is None else [float(np.abs(p_afni[:, :3]).max()), float(np.abs(p_afni[:, 3:]).max())]  # fmt: skip
        row["model"] = {k: None if p is None else [float(np.abs(p[:, :3]).max()), float(np.abs(p[:, 3:]).max())] for k, (p, v, e) in runs.items()}  # fmt: skip
        return row
    grid = M.read_grid(nib.load(src))
    thr = _thresholds(grid, kw, p_afni)
    pc, vc, _ = runs["smallest"]
    par, med, mean, inc = _measures(pc, vc, p_afni, v_afni, thr)
    sp = [_measures(pc, vc, p, v, thr) for k, (p, v, e) in runs.items() if k != "smallest" and p is not None]  # fmt: skip
    s_par = max(float(x[0].max()) for x in sp)
    s_med = max(x[1] for x in sp)
    m_fixed = 0.005 if kw.get("method") == "linear" else 0.001
    theta = np.abs(p_afni[:, :3]).sum(axis=1)
    lim = np.maximum(2e-3, 1e-3 * theta)
    fails = []
    if np.any(par > np.maximum(1.0, C * s_par)):
        fails.append(f"parameters {par.max():.2f} thresholds, spread {s_par:.2f}")
    if med > max(m_fixed, C * s_med):
        fails.append(f"median {med:.4f}, spread {s_med:.4f}")
    if mean is not None and all(x[2] is not None for x in sp):
        s_mean = max(float(x[2].max()) for x in sp)
        s_inc = max(float(x[3].max()) for x in sp)
        if np.any(mean > np.maximum(lim, C * s_mean)):
            fails.append(f"volumes' mean {mean.max():.2e}, spread {s_mean:.2e}")
        if np.any(inc > np.maximum(50 * lim * lim, C * C * s_inc)):
            fails.append(f"volumes' 1 - NCC {inc.max():.2e}, spread {s_inc:.2e}")
    fixed_ok = bool(par.max() <= 1.0 and med <= m_fixed)
    row.update(
        status="converging",
        passed=not fails,
        fails=fails,
        fixed=fixed_ok,
        r=float(par.max()),
        spread=s_par,
    )
    return row


def main(argv: list[str]) -> int:
    work, night = Path(argv[0]).resolve(), Path(argv[1])
    rest = argv[2:]
    sample, seed, jobs, cases = 0, 20261108, 4, []
    i = 0
    while i < len(rest):
        if rest[i] == "--sample":
            sample = int(rest[i + 1])
            i += 2
        elif rest[i] == "--seed":
            seed = int(rest[i + 1])
            i += 2
        elif rest[i] == "--jobs":
            jobs = int(rest[i + 1])
            i += 2
        else:
            cases.append(rest[i])
            i += 1
    if sample:
        found = sorted(
            int(p.stem.split("-")[3])
            for p in (night / "findings" / "afni-volreg-original").glob("*.json")
        )
        cases = [str(x) for x in sorted(random.Random(seed).sample(found, min(sample, len(found))))]
    work.mkdir(parents=True, exist_ok=True)
    with ProcessPoolExecutor(jobs, mp_context=get_context("spawn")) as pool:
        rows = list(pool.map(one, [(n, str(night), str(work)) for n in cases]))
    (work / "spread.json").write_text(json.dumps(rows, indent=1), encoding="utf-8")
    counts: dict[str, int] = {}
    for r in rows:
        key = (
            r["status"]
            if "passed" not in r
            else f"{r['status']}: {'passed' if r['passed'] else 'FAILED'}"
        )
        if r.get("status") == "converging" and r.get("passed") and not r.get("fixed"):
            key += " (by the spread)"
        counts[key] = counts.get(key, 0) + 1
    for k, v in sorted(counts.items()):
        print(f"{v:5d}  {k}")
    for r in rows:
        if r.get("passed") is False:
            print("FAILED", r["n"], r.get("fails"), r["argv"])
    return 1 if any(r.get("passed") is False for r in rows) else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
