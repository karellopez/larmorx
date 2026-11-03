# SPDX-License-Identifier: Apache-2.0
"""Benchmarks of fslmaths' and fslstats' clean-room original (sub-task 5 of their clean-room
part; the report: ``docs/benchmarks/fsl-maths.md``).

Each case runs once per implementation (informal: one run each, on a shared machine): FSL
6.0.7.23 (``~/fsl-6.0.7.23/bin/<program>``, timed directly: benchmarks are the oracle-output
cache's exception), the replica (``larmorx fsl <program>`` with ``LARMORX_IMPLEMENTATION=replica``
and ``LARMORX_NC_BIN``: the ``larmorx-nc`` program as a black box) and the original
(``LARMORX_IMPLEMENTATION=original``) with ``OMP_NUM_THREADS`` 1 and 14. Measured: the wall time
and the peak resident memory (the child's ``ru_maxrss`` from ``wait4``, its own children
included). Checked: the original's outputs at 1 and 14 threads are the same bits (images
decompressed, text as printed), and the original's output against FSL's (the largest absolute
difference relative to FSL's largest value, the share of identical voxels).

Inputs (fMRIPrep 25.2.6's ds005040 run, ``oracles/fmriprep-25.2.6/ds005040-sub-002-noreconall``):
the BOLD series in T1w space (50 x 62 x 44 x 63, float32), the preprocessed T1w (191 x 288 x
288, float32), the combine_masks call's two masks, and the goodvoxels chain's inputs as the
replica suite makes them (``fsl_maths_replica._goodvoxels_chain``: the ribbon a stand-in, the
segmentation's grey matter). Cases: fMRIPrep's calls (combine_masks; the goodvoxels chain's 14
fslmaths and 4 fslstats calls in order, the numbers each implementation's own fslstats prints;
BET's ``fslstats -l 0.001 -C``), the spec's speed cases (``-s`` and ``-dilD`` on a head, ``-Tstd``
of 63 volumes, ``fslstats -t -M -S -r`` of the series) and the heavy operations on the series
(``-Tmean``, ``-Tmedian``, ``-s 5``, ``-fmedian``, ``-bptf``) and kernels past FSL's FFT switch
(§10.2: ``-kernel boxv 13 -fmeanu`` on the series, ``-kernel sphere 6 -fmean`` on the T1w).

Run inside a memory box, from the repository with the validation package on the path::

    scripts/orchestration/boxed.sh 24 python -P validation/investigations/cleanroom-maths/bench.py \\
        <outdir> <larmorx binary> --nc <larmorx-nc binary> [--impl fsl,replica,orig1,orig14] \\
        [--case <regex>]

It writes ``<outdir>/bench.jsonl`` (one line per run) and prints a Markdown table.
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
from larmorx_validation.parity import fsl_maths_replica as rep

RUN = "ds005040-sub-002-noreconall"
IMPLS = ("fsl", "replica", "orig1", "orig14")


def _sources() -> dict[str, Path]:
    ws = rep.workspace()
    assert ws is not None, "the workspace (oracle runs) is needed"
    out = ws / rep.FMRIPREP_RUNS / RUN / "out"
    bold, _dseg = rep._goodvoxels_sources(RUN)
    t1w = next(iter(sorted(out.glob("sub-*/anat/sub-*_desc-preproc_T1w.nii.gz"))))
    node = next(n for r, n in rep.fmriprep_calls() if r == RUN)
    a, _op, b, _out = rep.fmriprep_args(RUN, node)
    return {"bold": bold, "t1w": t1w, "mask_a": Path(a), "mask_b": Path(b)}


def _cases(src: dict[str, Path]) -> dict[str, list[tuple[str, list[str]]]]:
    """name -> steps ``(program, arguments)``; ``{x}`` an input of ``src``; the chain's steps
    are built by :func:`_goodvoxels_steps`."""
    b, t = str(src["bold"]), str(src["t1w"])
    return {
        "combine_masks": [
            ("fslmaths", [str(src["mask_a"]), "-mul", str(src["mask_b"]), "out.nii.gz"])
        ],
        "goodvoxels": [],
        "bold-Tmean": [("fslmaths", [b, "-Tmean", "out.nii.gz"])],
        "bold-Tstd": [("fslmaths", [b, "-Tstd", "out.nii.gz"])],
        "bold-Tmedian": [("fslmaths", [b, "-Tmedian", "out.nii.gz"])],
        "bold-s5": [("fslmaths", [b, "-s", "5", "out.nii.gz"])],
        "bold-fmedian": [("fslmaths", [b, "-fmedian", "out.nii.gz"])],
        "bold-bptf": [("fslmaths", [b, "-bptf", "25", "2", "out.nii.gz"])],
        "bold-fmeanu-boxv13": [("fslmaths", [b, "-kernel", "boxv", "13", "-fmeanu", "out.nii.gz"])],
        "t1w-s5": [("fslmaths", [t, "-s", "5", "out.nii.gz"])],
        "t1w-dilD": [("fslmaths", [t, "-dilD", "out.nii.gz"])],
        "t1w-fmean-sphere6": [("fslmaths", [t, "-kernel", "sphere", "6", "-fmean", "out.nii.gz"])],
        "stats-bet": [("fslstats", [t, "-l", "0.001", "-C"])],
        "stats-series": [("fslstats", ["-t", b, "-M", "-S", "-r"])],
    }


def _env(impl: str, lx: str, nc: str | None) -> tuple[list[str], dict[str, str]]:
    if impl == "fsl":
        return [str(rep.fsldir() / "bin")], rep._environment({})
    env = {k: v for k, v in os.environ.items() if not k.startswith(("FSL", "LARMORX_", "OMP_"))}
    env.update(FSLOUTPUTTYPE="NIFTI_GZ", LARMORX_UPSTREAM_DEFECTS="fix")
    if impl == "replica":
        assert nc is not None, "--nc is needed for the replica"
        env.update(LARMORX_IMPLEMENTATION="replica", LARMORX_NC_BIN=nc, OMP_NUM_THREADS="14")
    else:
        env.update(
            LARMORX_IMPLEMENTATION="original", OMP_NUM_THREADS="1" if impl == "orig1" else "14"
        )
    return [lx, "fsl"], env


def _measure(argv: list[str], env: dict[str, str], cwd: Path) -> dict:
    """Runs ``argv`` in a fresh child of a helper process, so that the helper's
    ``RUSAGE_CHILDREN`` maximum is this run's peak."""
    helper = (
        "import json,resource,subprocess,sys,time\n"
        "a=json.loads(sys.argv[1])\n"
        "t=time.perf_counter()\n"
        "p=subprocess.run(a,capture_output=True,text=True)\n"
        "w=time.perf_counter()-t\n"
        "r=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss\n"
        "print(json.dumps({'code':p.returncode,'wall':w,'rss_kib':r,"
        "'stdout':p.stdout,'stderr':p.stderr}))\n"
    )
    p = subprocess.run(
        [sys.executable, "-I", "-c", helper, json.dumps(argv)],
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(p.stdout)


def _goodvoxels(
    prefix: list[str], env: dict[str, str], d: Path, files: dict[str, Path]
) -> tuple[dict, dict[str, Path]]:
    """The chain's 18 calls in order, each implementation's own fslstats numbers feeding the
    next calls; the total wall time, the largest peak."""
    values: dict[str, float] = {}
    files = dict(files)
    total, peak, code = 0.0, 0, 0
    for node, program, template in rep.GOODVOXELS_STEPS:
        nd = d / node
        nd.mkdir(parents=True, exist_ok=True)
        args = rep._goodvoxels_args(template, files, values, nd)
        larmorx = prefix[-1] == "fsl"
        argv = [*prefix, program, *args] if larmorx else [f"{prefix[0]}/{program}", *args]
        if program == "fslmaths":
            argv.append(f"{node}.nii.gz")
        r = _measure(argv, env, nd)
        total += r["wall"]
        peak = max(peak, r["rss_kib"])
        code = code or r["code"]
        if r["code"] != 0:
            break
        if program == "fslstats":
            values[node] = float(r["stdout"].split()[0])
            if node == "mod_ribbon_std":
                values["upper_thresh"] = values["mod_ribbon_mean"] + values[node] * 0.5
        else:
            files[node] = nd / f"{node}.nii.gz"
    return {"code": code, "wall": total, "rss_kib": peak, "stdout": json.dumps(values)}, files


def _data(path: Path) -> bytes:
    raw = path.read_bytes()
    return gzip.decompress(raw) if path.suffix == ".gz" else raw


def _outputs(d: Path) -> dict[str, Path]:
    return {
        str(p.relative_to(d)): p
        for p in sorted(d.rglob("*.nii.gz"))
        if not p.name.startswith(("bold", "ribbon"))
    }


def _vs_fsl(ours: Path, fsl: Path) -> str:
    import nibabel as nib

    a = np.asarray(nib.load(str(ours)).dataobj, dtype=np.float64)
    b = np.asarray(nib.load(str(fsl)).dataobj, dtype=np.float64)
    if a.shape != b.shape:
        return f"shape {a.shape} vs {b.shape}"
    same = (a == b) | (np.isnan(a) & np.isnan(b))
    if same.all():
        return "identical"
    fin = np.isfinite(a) & np.isfinite(b)
    big = np.nanmax(np.abs(b[np.isfinite(b)])) if np.isfinite(b).any() else 1.0
    rel = np.max(np.abs(a - b)[fin]) / big if fin.any() else float("nan")
    return f"max|d| {rel:.2e} of max|FSL|, {100 * same.mean():.3f} % of the voxels the same"


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("outdir", type=Path)
    ap.add_argument("larmorx")
    ap.add_argument("--nc")
    ap.add_argument("--impl", default=",".join(IMPLS))
    ap.add_argument("--case", default=".")
    o = ap.parse_args(argv)
    src = _sources()
    for p in src.values():
        p.read_bytes()  # into the page cache before the first timed run
    chain = rep._goodvoxels_chain(RUN)
    assert chain is not None
    gv_files = {"bold": src["bold"], "ribbon": chain / "ribbon.nii.gz"}
    impls = o.impl.split(",")
    o.outdir.mkdir(parents=True, exist_ok=True)
    log = (o.outdir / "bench.jsonl").open("a", encoding="utf-8")
    rows = []
    for name, steps in _cases(src).items():
        if not re.search(o.case, name):
            continue
        res = {}
        for impl in impls:
            d = o.outdir / name / impl
            shutil.rmtree(d, ignore_errors=True)
            d.mkdir(parents=True)
            prefix, env = _env(impl, o.larmorx, o.nc)
            if name == "goodvoxels":
                r, _ = _goodvoxels(prefix, env, d, gv_files)
            else:
                program, args = steps[0]
                cmd = [f"{prefix[0]}/{program}"] if impl == "fsl" else [*prefix, program]
                r = _measure([*cmd, *args], env, d)
            res[impl] = r
            line = {"case": name, "impl": impl, **{k: r[k] for k in ("code", "wall", "rss_kib")}}
            log.write(json.dumps(line) + "\n")
            log.flush()
            print(
                f"{name:22s} {impl:8s} exit {r['code']} {r['wall']:8.3f} s "
                f"{r['rss_kib'] / 1024:8.1f} MiB",
                flush=True,
            )
        notes = []
        if "orig1" in res and "orig14" in res:
            a, b = o.outdir / name / "orig1", o.outdir / name / "orig14"
            fa, fb = _outputs(a), _outputs(b)
            same = fa.keys() == fb.keys() and all(_data(fa[k]) == _data(fb[k]) for k in fa)
            same = same and res["orig1"]["stdout"] == res["orig14"]["stdout"]
            notes.append("1 = 14 threads" if same else "THREADS DIFFER")
        if "fsl" in res and ("orig14" in res or "orig1" in res):
            ours_dir = o.outdir / name / ("orig14" if "orig14" in res else "orig1")
            ours_res = res["orig14" if "orig14" in res else "orig1"]
            fo, ff = _outputs(ours_dir), _outputs(o.outdir / name / "fsl")
            if ff:
                worst = [f"{k}: {_vs_fsl(fo[k], ff[k])}" for k in ff if k in fo]
                odd = [w for w in worst if not w.endswith(": identical")]
                notes.append(
                    f"vs FSL: {len(worst) - len(odd)}/{len(ff)} identical"
                    + (f" ({'; '.join(odd)})" if odd else "")
                )
            else:
                notes.append(
                    "vs FSL: same text"
                    if ours_res["stdout"] == res["fsl"]["stdout"]
                    else f"vs FSL: {ours_res['stdout'].strip()!r} / {res['fsl']['stdout'].strip()!r}"
                )
        rows.append((name, res, notes))
        print(f"{name:22s} {'; '.join(notes)}", flush=True)
    print()
    head = "| Case | " + " | ".join(f"{i} s (MiB)" for i in impls) + " | Notes |"
    print(head)
    print("|" + "---|" * (len(impls) + 2))
    for name, res, notes in rows:
        cells = [
            f"{res[i]['wall']:.2f} ({res[i]['rss_kib'] / 1024:.0f})" if i in res else "-"
            for i in impls
        ]
        print(f"| {name} | " + " | ".join(cells) + f" | {'; '.join(notes)} |")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
