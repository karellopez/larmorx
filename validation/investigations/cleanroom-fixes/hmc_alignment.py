# SPDX-License-Identifier: Apache-2.0
"""Item 13 of the 2026-10-29 fixes: how well mcflirt and larmorx's clean-room head-motion
correction align every random phantom where their motion estimates differ (all of a night's
`mri-hmc` findings, not a sample). For each finding: its shrunk inputs rebuilt, FSL 6.0.7's
mcflirt and `larmorx mri hmc` (in `reproduce`, as the night compared, and by default) run on its
arguments, and the corrected series' temporal standard deviation in the phantom (voxels of
mcflirt's corrected mean above 10 % of its maximum; lower is better aligned), beside the
uncorrected input's: the measure of `../difftest/hmc_residuals.py`, run in parallel and written
to a JSON-lines file with each finding's options and motion, for the subsets. Black-box: only
the programs' outputs are read.

    python -P hmc_alignment.py <night> <work> <out.jsonl> [--jobs N] [--max N]
    python -P hmc_alignment.py --summary <out.jsonl>

The summary: in how many findings larmorx's residual is lower, within 1 %, or higher than
mcflirt's (the ratio of the two residuals), its quantiles, and the same per option and by the
size of the drawn motion, so that a subset aligned clearly worse would show.
"""

from __future__ import annotations

import json
import multiprocessing as mp
import os
import shutil
import sys
from collections import defaultdict
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np


def tsd(path: Path, mask_from: Path | None = None) -> float:
    import nibabel as nib

    x = np.asarray(nib.load(path).dataobj, dtype=np.float64)
    if x.ndim != 4 or x.shape[3] < 2:
        return float("nan")
    m = np.asarray(nib.load(mask_from).dataobj, dtype=np.float64) if mask_from else x
    mean = m.mean(axis=3) if m.ndim == 4 else m
    mask = mean > 0.1 * mean.max() if mean.max() > 0 else np.ones(mean.shape, bool)
    return float(x.std(axis=3)[mask].mean())


def largest_rotation(motion: list[dict] | None) -> float:
    """The largest rotation angle (degrees) of the drawn per-volume motions."""
    if not motion:
        return 0.0
    out = 0.0
    for m in motion:
        q = np.asarray(m.get("quaternion", (1, 0, 0, 0)), dtype=np.float64)
        q = q / np.linalg.norm(q)
        out = max(out, float(np.degrees(2 * np.arccos(min(1.0, abs(q[0]))))))
    return out


def one(p: str, work: str) -> dict:
    os.environ.setdefault("LARMORX_ORACLE_CACHE_MODE", "off")
    from larmorx_validation.difftest import core, inputs
    from larmorx_validation.parity import defects, mri_hmc
    from larmorx_validation.parity import fsl_mcflirt_replica as fr

    p = Path(p)
    finding = json.loads(p.read_text(encoding="utf-8"))
    recipe = finding.get("shrunk") or finding["recipe"]
    case = Path(work) / p.stem
    if case.exists():
        shutil.rmtree(case)
    case.mkdir(parents=True)
    paths = {}
    for name, spec in recipe["inputs"].items():
        q = case / (name + inputs.suffix_of(spec))
        inputs.materialize(spec, q)
        paths[name] = str(q)
    line = " ".join(core.substitute(core.arguments(recipe), paths.__getitem__))
    out = {}
    for who in ("mcflirt", "reproduce", "default"):
        d = case / who
        d.mkdir()
        args = line.replace("{run}", str(d)).split()
        if who == "mcflirt":
            code = fr.run_oracle(args, cwd=d, settings={}).code
        else:
            mode = defects.REPRODUCE if who == "reproduce" else defects.FIX
            code = mri_hmc._run_larmorx(args, {"FSLOUTPUTTYPE": "NIFTI_GZ"}, mode)[0]
        f = d / "out_mcf.nii.gz"
        ok = code == 0 and f.is_file()
        out[who] = tsd(f, case / "mcflirt" / "out_mcf.nii.gz") if ok else None
    before = tsd(Path(paths["in"]))
    shutil.rmtree(case, ignore_errors=True)
    spec = recipe["inputs"]["in"]
    return {
        "finding": p.stem,
        "input": before,
        "mcflirt": out["mcflirt"],
        "reproduce": out["reproduce"],
        "default": out["default"],
        "args": " ".join(core.arguments(recipe)),
        "shape": spec.get("shape"),
        "rotation": largest_rotation(spec.get("motion")),
        "failed": [c["name"] for c in finding.get("checks", []) if c.get("passed") is False],
    }


def run(night: Path, work: Path, target: Path, jobs: int, most: int | None) -> None:
    files = sorted((night / "findings" / "mri-hmc").glob("*-original.json"))[:most]
    work.mkdir(parents=True, exist_ok=True)
    done = set()
    if target.exists():
        done = {json.loads(x)["finding"] for x in target.read_text().splitlines() if x.strip()}
    todo = [str(p) for p in files if p.stem not in done]
    ctx = mp.get_context("spawn")
    with ProcessPoolExecutor(jobs, mp_context=ctx) as ex, target.open("a") as f:
        for r in ex.map(one, todo, [str(work)] * len(todo)):
            f.write(json.dumps(r) + "\n")
            f.flush()
    print("HMC-ALIGNMENT-DONE", len(todo), "run")


def _stats(rows: list[dict], key: str) -> str:
    ratios = np.array([r[key] / r["mcflirt"] for r in rows if r.get(key) and r.get("mcflirt")])
    if ratios.size == 0:
        return "no cases"
    lower = int((ratios < 0.99).sum())
    same = int(((ratios >= 0.99) & (ratios <= 1.01)).sum())
    higher = int((ratios > 1.01).sum())
    q = np.quantile(ratios, [0.05, 0.25, 0.5, 0.75, 0.95])
    return (
        f"{ratios.size} cases: lower {lower}, within 1 % {same}, higher {higher}; "
        f"ratio quantiles 5/25/50/75/95 % {q[0]:.3f} {q[1]:.3f} {q[2]:.3f} {q[3]:.3f} "
        f"{q[4]:.3f}; mean {ratios.mean():.3f}; above 1.05: {int((ratios > 1.05).sum())}, "
        f"above 1.10: {int((ratios > 1.10).sum())}, below 0.95: {int((ratios < 0.95).sum())}, "
        f"below 0.90: {int((ratios < 0.90).sum())}"
    )


def summary(target: Path) -> None:
    rows = [json.loads(x) for x in target.read_text().splitlines() if x.strip()]
    print(f"{len(rows)} findings; residual larmorx / mcflirt (temporal SD in the phantom)")
    print("  reproduce:", _stats(rows, "reproduce"))
    print("  default:  ", _stats(rows, "default"))
    gain = [r["input"] / r["mcflirt"] for r in rows if r.get("mcflirt") and r.get("input")]
    print(f"  mcflirt's own residual against the input's: median ratio {np.median(gain):.2f}")
    failed = [r for r in rows if r.get("mcflirt") is None or r.get("reproduce") is None]
    print(f"  runs without an output: {len(failed)}")
    by = defaultdict(list)
    for r in rows:
        for w in r["args"].split():
            if w.startswith("-") and w not in ("-in", "-out"):
                by[w].append(r)
    print("by option (reproduce):")
    for k in sorted(by, key=lambda k: -len(by[k])):
        if len(by[k]) >= 10:
            print(f"  {k:12s} {_stats(by[k], 'reproduce')}")
    print("by the largest drawn rotation (reproduce):")
    for lo, hi in ((0, 5), (5, 10), (10, 20), (20, 360)):
        sub = [r for r in rows if lo <= r.get("rotation", 0) < hi]
        print(f"  {lo:3d}-{hi:3d} deg  {_stats(sub, 'reproduce')}")
    worst = sorted(
        (r for r in rows if r.get("reproduce") and r.get("mcflirt")),
        key=lambda r: -r["reproduce"] / r["mcflirt"],
    )[:12]
    print("the 12 highest ratios:")
    for r in worst:
        print(f"  {r['finding']} {r['reproduce'] / r['mcflirt']:.3f} input {r['input']:.2f} "
              f"mcflirt {r['mcflirt']:.3f} larmorx {r['reproduce']:.3f} rot "
              f"{r.get('rotation', 0):.1f} | {r['args'][:120]}")  # fmt: skip


def main(argv: list[str]) -> int:
    if argv and argv[0] == "--summary":
        summary(Path(argv[1]))
        return 0
    jobs, most = 8, None
    if "--jobs" in argv:
        i = argv.index("--jobs")
        jobs = int(argv[i + 1])
        del argv[i : i + 2]
    if "--max" in argv:
        i = argv.index("--max")
        most = int(argv[i + 1])
        del argv[i : i + 2]
    run(Path(argv[0]), Path(argv[1]).resolve(), Path(argv[2]), jobs, most)
    summary(Path(argv[2]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
