# SPDX-License-Identifier: Apache-2.0
"""trial_ssroi_matrices.py <trial dir> <work dir> <n,...> [--jobs 8]: the random-testing
findings of standard_space_roi (and of ``bet -B``'s inner standard_space_roi) whose FLIRT matrix
parts from FSL's, measured. Each finding's drawn call is run with ``-d`` by FSL 6.0.7.23 (the
oracle-output cache, use mode) and by the clean-room original (``reproduce``); then FSL's own
``flirt`` measures the cost of both final matrices on the same image pair (``-init`` the matrix,
a schedule of this script's own: ``setscale 1``, smoothing 1, ``measurecost 12`` of the identity,
``print U``, at each of the default schedule's four scales with its smoothing, 8, 4, 2 and 1 mm,
``specs/flirt.md`` §7.4: the last is the stage that picks FLIRT's result), so the two matrices
are judged by FSL's cost alone (and the identity, for the pair's scale of costs). Also the input's size, the matrices' RMS deviation
(over 80 mm) and, for standard_space_roi, the stage run given FSL's matrix (``-applyxfm
-init``): the output header fields that still differ (``descrip`` aside). One JSON line per
finding on standard output. The clean-room catch-up, relay 10 (2026-11-05).

    PYTHONPATH=python:validation/src LARMORX_ORACLE_CACHE_MODE=use \\
        python -P validation/investigations/cleanroom-bet/trial_ssroi_matrices.py \\
        oracles/difftest/2026-11-05-catchup-bet <work> 9,174,121
"""

from __future__ import annotations

import argparse
import json
import multiprocessing
import shutil
import sys
from pathlib import Path

SCHEDULE = (
    "setrow UA 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1\nclear U\n"
    + "".join(
        f"setscale {s}\nsetoption smoothing {s}\nmeasurecost 12 UA 0 0 0 0 0 0 rel\n"
        for s in (8, 4, 2, 1)
    )
    + "print U\n"
)
#: standard_space_roi's own options (``specs/standard_space_roi.md`` §2.1) and their word counts;
#: every later word is FLIRT's.
SSROI_OPTIONS = {
    "-maskFOV": 0, "-maskMASK": 1, "-maskNONE": 0, "-roiFOV": 0, "-roiMASK": 1, "-roiNONE": 0,
    "-ssref": 1, "-altinput": 1, "-d": 0, "-b": 0,
}  # fmt: skip
#: FLIRT options kept for the measurement (the cost function and its bins); the search's are
#: irrelevant to one measured cost, and the dof is 12 so the identity row recomposes exactly.
KEEP = {"-cost": 1, "-bins": 1}


def _flirt_call(tool: str, args: list[str], fsldir: Path) -> tuple[str, str, list[str]]:
    """The input, the reference and FLIRT's kept options of the call's first FLIRT run."""
    std = str(fsldir / "data" / "standard" / "MNI152_T1_2mm")
    if tool == "bet":
        return args[0], std, []
    ref, extra, i = std, [], 2
    while i < len(args):
        w = args[i]
        if w in SSROI_OPTIONS:
            if w == "-ssref":
                ref = args[i + 1]
            i += 1 + SSROI_OPTIONS[w]
            continue
        if w in KEEP and i + 1 < len(args):
            extra += [w, args[i + 1]]
            i += 2
            continue
        i += 1
    return args[0], ref, extra


def _cost(fb, d: Path, inp: str, ref: str, extra: list[str], mat: Path, env) -> list[float] | None:
    d.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(mat, d / "m.mat")
    (d / "cost.sch").write_text(SCHEDULE, encoding="utf-8")
    words = ["-in", inp, "-ref", ref, "-init", "m.mat", "-schedule", "cost.sch", "-dof", "12"]
    r = fb.oracle_run("flirt", [*words, *extra, "-omat", "o.mat"], env, d)
    try:
        return [float(ln.split()[0]) for ln in r.out.splitlines() if ln.strip()]
    except (IndexError, ValueError):
        return None


def _header_fields(a: Path, b: Path) -> list[str]:
    import nibabel as nib
    import numpy as np

    ha, hb = nib.load(str(a)).header, nib.load(str(b)).header
    out = []
    for k in ha:
        if k == "descrip" or k not in hb:
            continue
        va, vb = ha[k], hb[k]
        try:
            same = bool(np.array_equal(va, vb, equal_nan=va.dtype.kind == "f"))
        except Exception:
            same = str(va) == str(vb)
        if not same:
            out.append(f"{k}: FSL {va.tolist() if hasattr(va, 'tolist') else va}, "
                       f"larmorx {vb.tolist() if hasattr(vb, 'tolist') else vb}")  # fmt: skip
    return out


def one(job: tuple[str, str, int]) -> dict:
    import nibabel as nib
    import numpy as np
    from larmorx_validation.difftest.tools import fsl_bet as tool
    from larmorx_validation.parity import fsl_bet as fb
    from larmorx_validation.stages.compare import rms_deviation

    trial, work, n = Path(job[0]), Path(job[1]), job[2]
    doc = json.loads(
        (trial / "findings/fsl-bet" / f"bet-options-6-{n:05d}-original.json").read_text()
    )
    s = tool.case(doc["recipe"], "original").payload
    out = work / f"{n:05d}"
    (out / "in").mkdir(parents=True, exist_ok=True)
    paths = []
    for k, name in enumerate(s.inputs):
        p = fb.input_path(name)
        q = out / "in" / f"in{k}{''.join(p.suffixes)}"
        shutil.copyfile(p, q)
        paths.append(q)
    args = fb._arguments(s, paths)
    # standard_space_roi's -d before FLIRT's options (as the suite's stage, §2.1); bet's at the end
    if "-d" in args[2:]:
        with_d = args
    elif s.tool == "standard_space_roi":
        with_d = [args[0], args[1], "-d", *args[2:]]
    else:
        with_d = [*args, "-d"]
    env = fb.environment(s.output_type, s.env)
    fb.IMPLEMENTATION["name"] = "original"
    res: dict = {"n": n, "tool": s.tool, "args": " ".join(a.replace(str(out), ".") for a in args)}
    img = nib.load(str(paths[0]))
    res["input"] = (f"{'x'.join(str(x) for x in img.shape[:3])} voxels of "
                    f"{'x'.join(f'{z:g}' for z in img.header.get_zooms()[:3])} mm")  # fmt: skip
    runs = {}
    for who in ("fsl", "lx"):
        d = out / who
        if d.exists():
            shutil.rmtree(d)
        fb._prepare(d, s)
        r = (fb.oracle_run(s.tool, with_d, env, d, never=fb.case_never(s)) if who == "fsl"
             else fb.run_larmorx(s.tool, with_d, "reproduce", env, d))  # fmt: skip
        runs[who] = d
        res[f"{who}_exit"] = r.code
    stem = "out_tmp_premask_tmp_to_std.mat" if s.tool == "bet" else "out_tmp_to_std.mat"
    mf, ml = runs["fsl"] / stem, runs["lx"] / stem
    fsldir = fb.fsl_dir()
    if mf.is_file() and ml.is_file():
        a, b = fb._mat(mf), fb._mat(ml)
        inp, ref, extra = _flirt_call(s.tool, args, fsldir)
        centre = fb._ssroi_centre(args)
        res["rms_mm"] = (
            round(float(rms_deviation(a, b, centre)), 4)
            if a is not None and b is not None
            else None
        )
        res["cost_fsl"] = _cost(fb, out / "cost-fsl", inp, ref, extra, mf, env)
        res["cost_lx"] = _cost(fb, out / "cost-lx", inp, ref, extra, ml, env)
        eye = out / "identity.mat"
        eye.write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", encoding="utf-8")
        res["cost_identity"] = _cost(fb, out / "cost-identity", inp, ref, extra, eye, env)
        res["flirt"] = " ".join(["-in", Path(inp).name, "-ref", Path(ref).name, *extra])
    # the outputs the user receives (everything but the -d temporaries)
    finals = [p.name for p in runs["fsl"].iterdir() if p.is_file() and "_tmp" not in p.name]
    same = []
    for name in sorted(finals):
        q = runs["lx"] / name
        if not q.is_file():
            same.append(f"{name}: larmorx has none")
            continue
        if fb.is_image(name) and not name.endswith((".img", ".img.gz", ".img.bz2", ".img.zst")):
            try:
                da = np.asanyarray(nib.load(str(runs["fsl"] / name)).dataobj)
                db = np.asanyarray(nib.load(str(q)).dataobj)
                same.append(
                    f"{name}: {'the same values' if da.shape == db.shape and np.array_equal(da, db) else f'{da.shape} vs {db.shape}' if da.shape != db.shape else str(int(np.count_nonzero(da != db))) + ' values differ'}"
                )
            except Exception as e:
                same.append(f"{name}: {type(e).__name__}")
    res["outputs"] = same
    if s.tool == "standard_space_roi" and mf.is_file():
        given = "fsl-matrix.mat"
        stage = [*with_d, "-applyxfm", "-init", given]
        st = {}
        for who in ("fsl-stage", "lx-stage"):
            d = out / who
            if d.exists():
                shutil.rmtree(d)
            fb._prepare(d, s)
            shutil.copyfile(mf, d / given)
            r = (fb.oracle_run(s.tool, stage, env, d, never=fb.case_never(s)) if who == "fsl-stage"
                 else fb.run_larmorx(s.tool, stage, "reproduce", env, d))  # fmt: skip
            st[who] = d
        heads = {}
        for p in sorted(st["fsl-stage"].iterdir()):
            if p.name.startswith("out.") and p.name.endswith(
                (".nii", ".nii.gz", ".hdr", ".hdr.gz", ".hdr.bz2", ".hdr.zst")
            ):
                q = st["lx-stage"] / p.name
                heads[p.name] = _header_fields(p, q) if q.is_file() else ["larmorx has none"]
                try:
                    ia, ib = nib.load(str(p)), nib.load(str(q))
                    heads[p.name].append(f"shape FSL {ia.shape}, larmorx {ib.shape}")
                except Exception:
                    pass
        res["stage_headers"] = heads
    return res


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("trial")
    ap.add_argument("work")
    ap.add_argument("only")
    ap.add_argument("--jobs", type=int, default=8)
    a = ap.parse_args(argv)
    ns = [int(x) for x in a.only.split(",")]
    jobs = [(str(Path(a.trial).resolve()), str(Path(a.work).resolve()), n) for n in ns]
    with multiprocessing.get_context("spawn").Pool(a.jobs) as pool:
        for r in pool.imap(one, jobs):
            print(json.dumps(r), flush=True)
    print("TRIAL-SSROI-END", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
