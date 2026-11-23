# SPDX-License-Identifier: Apache-2.0
"""FS-269 at the v8 stream's ``mri_vsinus_seg``: ``mri_mask -T .001 -crop 0`` (a pad of 0) with
FreeSurfer's box and with the corrected one, and what the rest of the step makes of each.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica; the official programs are run
as black boxes).

The step's prior is a temporary file the recorded runs do not keep, so it is made again by the
official script's own steps, from the recorded head's files (checked against the hashes the
recording holds for them at the step: ``nu.mgz``, ``synthseg.rca.mgz``, SynthMorph's
``reg.targ_to_invol.lta``; the recorded ``vsinus.mgz`` and the ``brain.finalsurfs.mgz`` the next
step reads), one folder per head under ``<workspace>/oracles/inputs/fs269-vsinus/<run>/``:

1. ``mri_vol2vol --mov <FREESURFER_HOME>/average/vsinus.no-sp.prior.mni152.1.0mm.mgz --lta
   <lta> --targ nu.mgz --o prior.mgz`` (official);
2. ``mri_mask -T .001 -crop 0 prior.mgz prior.mgz mincrop.mgz``: official, and the port with
   ``--upstream-defects reproduce`` and ``fix``; ``reproduce`` must be the official file (and,
   since the decision of 2026-11-23, rule C, ``fix`` too); the corrected box, the voxels the
   threshold keeps from the first to the last on each axis, by the port's ``mri_binarize --i
   prior.mgz --min .001 --bb 0`` with ``fix`` (``REGIONgetBoundingBox``'s rule-A box: its grid,
   the only thing the script takes from the crop, is the one ``mri_mask``'s ``fix`` gave before
   the decision, checked on the six heads);
3. for FreeSurfer's box (the official ``mincrop.mgz``) and the corrected one, the rest with the
   official programs: ``mri_binarize --crop-around-ras invol.crop.mgz nu.mgz nolta
   cras mincrop.mgz 0 144 144 144``; ``mri_sclimbic_seg`` (the script's options, 4 threads as
   recon-all ran it) when the crop differs from FreeSurfer's (else its result is the same file);
   ``mri_binarize --i synthseg.rca.mgz --match 3 42 --inv`` (the cortex mask); ``mri_vol2vol
   --mov seg.crop.mgz --targ nu.mgz --interp nearest --regheader``; ``mri_mask vsinus.mgz
   ctxsegmask.mgz vsinus.mgz``; then recon-all's ``mri_mask -oval 1 -invert`` of the recorded
   ``brain.finalsurfs.mgz`` by that ``vsinus.mgz``.

Reported per head: the official crop's centre and limits (``MRIcropAroundCRS``'s lines), the
corrected one's, whether FreeSurfer's chain gives the recorded ``vsinus.mgz`` again (so the
regenerated prior is the run's), and with the corrected box the voxels of ``vsinus.mgz`` and of
``brain.finalsurfs.mgz`` that change (per label, and the Dice of each label).

    scripts/orchestration/boxed.sh 24 python -P validation/investigations/freesurfer-fs269-bbox/vsinus.py \\
        [--runs a,b] [--json out.json]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer import files as fsfiles  # noqa: E402
from larmorx_validation.freesurfer.recording import Recording  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

RUNS = (
    "ds000005-sub-01-v8",
    "ds000228-sub-pixar008-v8",
    "ds002422-sub-02-v8",
    "ds004173-sub-603309-v8",
    "ds005003-sub-RS001-v8",
    "ds005454-sub-16-v8",
)
LTA = "mri/transforms/synthmorph.1.0mm.1.0mm/reg.targ_to_invol.lta"
FOV = "144"
POLICY = oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer")
#: The network at recon-all's 4 threads: TensorFlow's reductions over threads are not measured
#: here, so its runs are never cached.
NET_POLICY = oracle_cache.Policy(
    threads=4,
    family="freesurfer",
    formats="freesurfer",
    never="mri_sclimbic_seg: a TensorFlow network at 4 threads (its thread-dependence not measured)",
)
MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
CTAB = """     0  Unknown                           0   0   0  255
  6111  Left-Transverse-Sinus           229  11 152    0
  6112  Right-Transverse-Sinus          40 251  16    0
  6115  Straight-Sinus                 185 210  35    0
  6116  Superior-Sinus-P                26 129 125    0
  6117  Superior-Sinus-D               144  51 173    0
"""
CROP_LINES = re.compile(r"^(crsCenter .*|limits .*)$", re.M)


def sha256(path: Path) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest() if Path(path).is_file() else ""


def step_after(rec: Recording):
    """The recorded step that reads ``vsinus.mgz`` (recon-all's ``mri_mask -oval 1 -invert``)."""
    for s in rec.steps:
        if any(a.endswith("/vsinus.mgz") for a in s.argv):
            return s
    raise SystemExit(f"{rec.name}: no step reads vsinus.mgz")


def official(argv, env, cwd, policy=POLICY, timeout=1800):
    p = oracle_cache.run(argv, env=env, cwd=cwd, outputs=[cwd], policy=policy, timeout=timeout,
                         runner="fs269-vsinus")  # fmt: skip
    if p.returncode != 0:
        raise SystemExit(f"{argv[0]} failed in {cwd}: {p.stdout[-800:]} {p.stderr[-800:]}")
    return p


def geometry(path: Path) -> list:
    """The crop's grid as the script uses it: its size and its centre ``c_ras`` (the MGH header's
    single-precision numbers, exactly)."""
    h = nib.load(str(path)).header
    return [[int(n) for n in h.get_data_shape()[:3]], [float(x) for x in h["Pxyz_c"]]]


def labels_diff(a: np.ndarray, b: np.ndarray) -> dict:
    out = {"voxels_differ": int(np.count_nonzero(a != b))}
    for lab in sorted(set(np.unique(a)) | set(np.unique(b))):
        if lab == 0:
            continue
        x, y = a == lab, b == lab
        n = int(x.sum() + y.sum())
        out[str(int(lab))] = {
            "freesurfer": int(x.sum()),
            "corrected": int(y.sum()),
            "dice": round(2 * int((x & y).sum()) / n, 6) if n else 1.0,
        }
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", default=",".join(RUNS))
    ap.add_argument("--jobs", type=int, default=10)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    env_build = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-cli", "-j", str(a.jobs),
         "--manifest-path", str(MANIFEST)],
        check=True,
        env=env_build,
    )  # fmt: skip
    port = REPO / "crates-freesurfer" / "target" / "release" / "larmorx-freesurfer"
    home = ws / FS_HOME
    env = {"PATH": f"{home / 'bin'}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "USER": "probe", "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    net_env = dict(env, OMP_NUM_THREADS="4")
    ctx = fsfiles.Context(any_directory=True)
    rows = []
    for run in a.runs.split(","):
        rec = Recording(ws / "oracles" / "freesurfer-8.2.0" / "runs" / run)
        mri = Path(rec.root) / "subjects" / rec.subject / "mri"
        after = step_after(rec)
        before = after.before
        base = ws / "oracles" / "inputs" / "fs269-vsinus" / run
        # recon-all's SUBJECTS_DIR (mri_vol2vol refuses to run without one)
        env["SUBJECTS_DIR"] = net_env["SUBJECTS_DIR"] = str(Path(rec.root) / "subjects")
        if base.exists():
            shutil.rmtree(base)
        base.mkdir(parents=True)
        # the recorded inputs as they were at the step (the head's final files, or the blobs)
        src = {}
        for rel in ("mri/nu.mgz", "mri/synthseg.rca.mgz", LTA, "mri/vsinus.mgz",
                    "mri/brain.finalsurfs.mgz"):  # fmt: skip
            sha = before[rel]
            final = mri.parent / rel
            src[rel] = final if sha256(final) == sha else rec.blob(sha)
            if sha256(src[rel]) != sha:
                raise SystemExit(f"{run}: {rel} not recorded as the step read it")
        nu = base / "nu.mgz"
        shutil.copyfile(src["mri/nu.mgz"], nu)
        lta = base / "reg.targ_to_invol.lta"
        shutil.copyfile(src[LTA], lta)
        d = base / "prior"
        d.mkdir()
        official(["mri_vol2vol", "--mov", home / "average" / "vsinus.no-sp.prior.mni152.1.0mm.mgz",
                  "--lta", lta, "--targ", nu, "--o", d / "prior.mgz"], env, d)  # fmt: skip
        prior = d / "prior.mgz"
        crops = {}
        for side in ("official", "reproduce", "fix"):
            c = base / f"mincrop-{side}"
            c.mkdir()
            args = ["mri_mask", "-T", ".001", "-crop", "0", prior, prior, c / "mincrop.mgz"]
            if side == "official":
                p = official(args, env, c)
            else:
                p = subprocess.run(
                    [port, "--upstream-defects", side, "freesurfer", *map(str, args)],
                    env=env, cwd=c, capture_output=True, text=True, timeout=900,
                )  # fmt: skip
                if p.returncode != 0:
                    raise SystemExit(f"{run} {side}: {p.stderr[-800:]}")
            crops[side] = (c / "mincrop.mgz", p.stdout, p.stderr)
        c = base / "mincrop-full"
        c.mkdir()
        p = subprocess.run(
            [port, "--upstream-defects", "fix", "freesurfer", "mri_binarize", "--i", str(prior),
             "--min", ".001", "--bb", "0", "--o", str(c / "mincrop.mgz")],
            env=env, cwd=c, capture_output=True, text=True, timeout=900,
        )  # fmt: skip
        if p.returncode != 0:
            raise SystemExit(f"{run} full box: {p.stderr[-800:]}")
        crops["full"] = (c / "mincrop.mgz", p.stdout, p.stderr)
        rep = fsfiles.compare(crops["official"][0], crops["reproduce"][0], ctx, name="mincrop.mgz")
        fixed_same = fsfiles.compare(crops["official"][0], crops["fix"][0], ctx, name="mincrop.mgz")
        row = {
            "run": run,
            "mincrop_reproduce_identical": rep.same,
            "mincrop_fix_identical": fixed_same.same,
            "fix_notices": sorted(set(re.findall(r"FS-\d+", crops["fix"][2]))),
            "full_box_notices": sorted(set(re.findall(r"FS-\d+", crops["full"][2]))),
            "mincrop_geometry": {s: geometry(crops[s][0]) for s in ("official", "fix", "full")},
            "mincrop_shape": {
                s: [int(n) for n in nib.load(str(crops[s][0])).shape]
                for s in ("official", "fix", "full")
            },
        }
        mask = base / "ctxsegmask"
        mask.mkdir()
        shutil.copyfile(src["mri/synthseg.rca.mgz"], mask / "synthseg.rca.mgz")
        official(["mri_binarize", "--i", mask / "synthseg.rca.mgz", "--match", "3", "42", "--inv",
                  "--o", mask / "ctxsegmask.mgz"], env, mask)  # fmt: skip
        out = {}
        for box, side in (("freesurfer", "official"), ("corrected", "full")):
            b = base / box
            b.mkdir()
            p = official(["mri_binarize", "--crop-around-ras", b / "invol.crop.mgz", nu, "nolta",
                          "cras", crops[side][0], "0", FOV, FOV, FOV], env, b)  # fmt: skip
            row[f"{box}_crop"] = CROP_LINES.findall(p.stdout)
            same_crop = box == "corrected" and fsfiles.compare(
                base / "freesurfer" / "invol.crop.mgz", b / "invol.crop.mgz", ctx,
                name="invol.crop.mgz").same  # fmt: skip
            row[f"{box}_crop_identical_to_freesurfer"] = same_crop if box == "corrected" else True
            if same_crop:
                out[box] = out["freesurfer"]
                continue
            (b / "vsinus.no-sp.ctab").write_text(CTAB, encoding="utf-8")
            official(["mri_sclimbic_seg", "--model",
                      home / "models" / "vsinus.no-sp.m.all.nstd10-070.h5", "--ctab",
                      b / "vsinus.no-sp.ctab", "--keep_ac", "--conform", "--percentile", "99.9",
                      "--vmp", "--output-base", "vsinus", "--logfile", "mri_vsinus_seg.log",
                      "--fov", FOV, "--threads", "4", "--no-cite-sclimbic", "--i",
                      b / "invol.crop.mgz", "--o", b / "seg.crop.mgz"],
                     net_env, b, policy=NET_POLICY)  # fmt: skip
            official(["mri_vol2vol", "--mov", b / "seg.crop.mgz", "--targ", nu, "--interp",
                      "nearest", "--o", b / "vsinus.mgz", "--regheader"], env, b)  # fmt: skip
            official(["mri_mask", b / "vsinus.mgz", mask / "ctxsegmask.mgz", b / "vsinus.mgz"],
                     env, b)  # fmt: skip
            shutil.copyfile(src["mri/brain.finalsurfs.mgz"], b / "brain.finalsurfs.mgz")
            official(["mri_mask", "-oval", "1", "-invert", "brain.finalsurfs.mgz", b / "vsinus.mgz",
                      "brain.finalsurfs.mgz"], env, b)  # fmt: skip
            out[box] = b
        row["freesurfer_vsinus_is_recorded"] = fsfiles.compare(
            src["mri/vsinus.mgz"], out["freesurfer"] / "vsinus.mgz", ctx, name="vsinus.mgz"
        ).same
        rec_bfs = rec.blob(after.outputs["mri/brain.finalsurfs.mgz"])
        row["freesurfer_finalsurfs_is_recorded"] = fsfiles.compare(
            rec_bfs, out["freesurfer"] / "brain.finalsurfs.mgz", ctx,
            name="brain.finalsurfs.mgz").same  # fmt: skip
        va = np.asarray(nib.load(str(out["freesurfer"] / "vsinus.mgz")).dataobj)
        vb = np.asarray(nib.load(str(out["corrected"] / "vsinus.mgz")).dataobj)
        fa = np.asarray(nib.load(str(out["freesurfer"] / "brain.finalsurfs.mgz")).dataobj)
        fb = np.asarray(nib.load(str(out["corrected"] / "brain.finalsurfs.mgz")).dataobj)
        row["vsinus"] = labels_diff(va, vb)
        row["finalsurfs_voxels_differ"] = int(np.count_nonzero(fa != fb))
        rows.append(row)
        print(json.dumps(row), flush=True)
    summary = {
        "runs": a.runs.split(","),
        "reproduce_identical": sum(r["mincrop_reproduce_identical"] for r in rows),
        "recorded_vsinus_regenerated": sum(r["freesurfer_vsinus_is_recorded"] for r in rows),
        "crop_moves": sum(not r["corrected_crop_identical_to_freesurfer"] for r in rows),
        "vsinus_changes": sum(r["vsinus"]["voxels_differ"] > 0 for r in rows),
        "rows": rows,
    }
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 0 if summary["reproduce_identical"] == len(rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
