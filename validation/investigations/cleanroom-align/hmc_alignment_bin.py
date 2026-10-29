# SPDX-License-Identifier: Apache-2.0
"""How well FSL 6.0.7.23's mcflirt and two builds of larmorx's clean-room head-motion correction
align the random phantoms of a random-testing run's `mri-hmc` findings: the measure of
`../cleanroom-fixes/hmc_alignment.py` (the corrected series' temporal standard deviation in the
phantom, voxels of mcflirt's corrected mean above 10 % of its maximum; lower is better aligned),
with the programs run as command lines, so that a build before a change and one after it are
measured on the same mcflirt runs. Black-box: only the programs' outputs are read.

    python -P hmc_alignment_bin.py <run> <work> <out.jsonl> --before <larmorx> --after <larmorx>
                                   [--cost normmi,mutualinfo] [--jobs N]
    python -P hmc_alignment_bin.py --summary <out.jsonl>

`<run>` is a random-testing output folder (its `findings/mri-hmc/*-original.json`); each finding's
shrunk inputs are rebuilt and mcflirt (`~/fsl-6.0.7.23/bin/mcflirt`, `$FSL_623`), larmorx before
and after in `reproduce` (the comparison random testing makes) and after by default are run on its
arguments. The summary gives, per cost, the ratio larmorx / mcflirt: its median and 5 % to 95 %,
and how many findings larmorx aligns better (ratio below 0.99), within 1 %, or worse (above 1.01).
"""

from __future__ import annotations

import json
import multiprocessing as mp
import os
import shutil
import subprocess
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np

FSL = Path(os.environ.get("FSL_623", Path.home() / "fsl-6.0.7.23"))


def tsd(path: Path, mask_from: Path | None = None) -> float | None:
    import nibabel as nib

    if not path.is_file():
        return None
    x = np.asarray(nib.load(path).dataobj, dtype=np.float64)
    if x.ndim != 4 or x.shape[3] < 2:
        return None
    m = np.asarray(nib.load(mask_from).dataobj, dtype=np.float64) if mask_from else x
    mean = m.mean(axis=3) if m.ndim == 4 else m
    mask = mean > 0.1 * mean.max() if mean.max() > 0 else np.ones(mean.shape, bool)
    return float(x.std(axis=3)[mask].mean())


def largest_rotation(motion: list[dict] | None) -> float:
    out = 0.0
    for m in motion or []:
        q = np.asarray(m.get("quaternion", (1, 0, 0, 0)), dtype=np.float64)
        q = q / np.linalg.norm(q)
        out = max(out, float(np.degrees(2 * np.arccos(min(1.0, abs(q[0]))))))
    return out


def run_program(argv: list[str], cwd: Path, extra: dict[str, str]) -> int:
    env = {"HOME": str(Path.home()), "PATH": "/usr/bin:/bin", "FSLOUTPUTTYPE": "NIFTI_GZ", **extra}
    cwd.mkdir(parents=True, exist_ok=True)
    return subprocess.run(argv, cwd=cwd, env=env, capture_output=True, timeout=3600).returncode


def one(p: str, work: str, before: str, after: str) -> dict:
    os.environ.setdefault("LARMORX_ORACLE_CACHE_MODE", "off")
    from larmorx_validation.difftest import core, inputs

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
    runs = {
        "mcflirt": ([str(FSL / "bin" / "mcflirt")], {"FSLDIR": str(FSL)}),
        "before": ([before, "mri", "hmc"], {"LARMORX_UPSTREAM_DEFECTS": "reproduce"}),
        "after": ([after, "mri", "hmc"], {"LARMORX_UPSTREAM_DEFECTS": "reproduce"}),
        "after_default": ([after, "mri", "hmc"], {"LARMORX_UPSTREAM_DEFECTS": "fix"}),
    }
    out = {}
    for who, (argv, extra) in runs.items():
        d = case / who
        args = line.replace("{run}", str(d)).split()
        extra = {**extra, "OMP_NUM_THREADS": "1"}
        code = run_program([*argv, *args], d, extra)
        f = d / "out_mcf.nii.gz"
        out[who] = tsd(f, case / "mcflirt" / "out_mcf.nii.gz") if code == 0 else None
    before_input = tsd(Path(paths["in"]))
    shutil.rmtree(case, ignore_errors=True)
    words = core.arguments(recipe)
    cost = words[words.index("-cost") + 1] if "-cost" in words else "normcorr"
    return {
        "finding": p.stem,
        "cost": cost,
        "input": before_input,
        **out,
        "args": " ".join(words),
        "rotation": largest_rotation(recipe["inputs"]["in"].get("motion")),
    }


def run(night: Path, work: Path, target: Path, before: str, after: str, costs, jobs: int) -> None:
    from larmorx_validation.difftest import core

    files = []
    for f in sorted((night / "findings" / "mri-hmc").glob("*-original.json")):
        recipe = json.loads(f.read_text(encoding="utf-8"))
        recipe = recipe.get("shrunk") or recipe["recipe"]
        words = core.arguments(recipe)
        cost = words[words.index("-cost") + 1] if "-cost" in words else "normcorr"
        if cost in costs:
            files.append(str(f))
    work.mkdir(parents=True, exist_ok=True)
    done = set()
    if target.exists():
        done = {json.loads(x)["finding"] for x in target.read_text().splitlines() if x.strip()}
    todo = [f for f in files if Path(f).stem not in done]
    ctx = mp.get_context("spawn")
    n = len(todo)
    with ProcessPoolExecutor(jobs, mp_context=ctx) as ex, target.open("a") as out:
        for r in ex.map(one, todo, [str(work)] * n, [before] * n, [after] * n):
            out.write(json.dumps(r) + "\n")
            out.flush()
    print("HMC-ALIGNMENT-BIN-DONE", n, "run")


def stats(rows: list[dict], key: str) -> str:
    ratios = np.array([r[key] / r["mcflirt"] for r in rows if r.get(key) and r.get("mcflirt")])
    if ratios.size == 0:
        return "no cases"
    lower = int((ratios < 0.99).sum())
    same = int(((ratios >= 0.99) & (ratios <= 1.01)).sum())
    higher = int((ratios > 1.01).sum())
    q = np.quantile(ratios, [0.05, 0.5, 0.95])
    return (
        f"{ratios.size} findings: larmorx lower {lower}, within 1 % {same}, higher {higher}; "
        f"median {q[1]:.3f} (5 % - 95 % {q[0]:.3f} - {q[2]:.3f}); above 1.10: "
        f"{int((ratios > 1.10).sum())}, below 0.90: {int((ratios < 0.90).sum())}"
    )


def summary(target: Path) -> None:
    rows = [json.loads(x) for x in target.read_text().splitlines() if x.strip()]
    for cost in sorted({r["cost"] for r in rows}):
        sub = [r for r in rows if r["cost"] == cost]
        print(f"-cost {cost}: residual larmorx / mcflirt 6.0.7.23 (temporal SD in the phantom)")
        for key in ("before", "after", "after_default"):
            print(f"  {key:14s} {stats(sub, key)}")
        same = sum(
            1 for r in sub if r.get("after") is not None and r.get("after") == r.get("mcflirt")
        )
        print(f"  after, the same residual as mcflirt to the last digit: {same} of {len(sub)}")
        failed = [r["finding"] for r in sub if None in (r.get("mcflirt"), r.get("after"))]
        print(f"  runs without an output: {failed}")


def main(argv: list[str]) -> int:
    if argv and argv[0] == "--summary":
        summary(Path(argv[1]))
        return 0
    opts = {"--jobs": "8", "--cost": "normmi,mutualinfo", "--before": "", "--after": ""}
    for k in list(opts):
        if k in argv:
            i = argv.index(k)
            opts[k] = argv[i + 1]
            del argv[i : i + 2]
    run(
        Path(argv[0]),
        Path(argv[1]).resolve(),
        Path(argv[2]),
        opts["--before"],
        opts["--after"],
        set(opts["--cost"].split(",")),
        int(opts["--jobs"]),
    )
    summary(Path(argv[2]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
