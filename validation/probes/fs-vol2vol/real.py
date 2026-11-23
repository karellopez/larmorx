# SPDX-License-Identifier: Apache-2.0
"""mri_vol2vol's recorded calls and fMRIPrep's on the recorded heads: the official program against
the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

The v8 stream's scripts call ``mri_vol2vol`` nine times per head (``recon-all.log``):
``fs-synthmorph-reg``'s check (``--regheader`` of its output onto FreeSurfer's MNI152 target),
``mri_mcadura_seg``'s (per hemisphere: the dura prior through ``--reg <reg.targ_to_invol.lta>``
onto the transform's destination, ``nu.mgz`` onto the prior's crop, the crop's segmentation back
onto ``nu.mgz`` by nearest neighbour) and ``mri_vsinus_seg``'s (the sinus prior through ``--lta``
onto ``nu.mgz``; the segmentation back onto ``nu.mgz`` by nearest neighbour). On each recorded v8
run (``oracles/freesurfer-8.2.0/runs/*-v8``) every form runs on the head's own files: the calls
whose inputs the runs keep exactly (the priors through ``reg.targ_to_invol.lta``); the crops made
again by the official ``mri_mask`` command the scripts ran; the scripts' temporary inputs that
cannot be made again (SynthMorph's output, the network's segmentations) replaced by the head's
own volumes of the same kind (``nu.mgz``; ``aseg.mgz`` taken onto the crop's grid, itself a call;
``vsinus.mgz``). And fMRIPrep's call (sMRIPrep's ``ApplyVolTransform``: ``--mov aseg.mgz --targ
<T1w> --lta <fsnative-to-T1w> --interp nearest --o seg.nii.gz``) on the same heads, its transform
the inverse of ``rawavg2orig.lta`` (official ``lta_convert --invert``) and the T1w ``rawavg.mgz``.
And part 2's non-linear paths (relay 54) on the recorded classic runs (``*-classic``), whose
``talairach.m3z`` is ``mri_ca_register``'s morph: ``norm.mgz`` onto the atlas (``--morph``),
``aseg.mgz`` by nearest neighbour (``--morph --nearest``), ``norm.mgz`` through ``--gcam`` (no LTAs:
the atlas as the output grid) and the morphed ``norm.mgz`` (made by the official program) back onto
the head (``--inv-morph``). Each call: the official program through the oracle-output cache (one thread,
``FS_DISABLE_LANG=1``), the port (``--port <larmorx-freesurfer>``) with ``reproduce`` and
``fix``, each in a folder of its own; exit status, both streams (the noise seed and ``VmPeak``
masked) and the files written compared byte for byte.

    python -P validation/probes/fs-vol2vol/real.py --port <larmorx-freesurfer> [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

#: mri_vol2vol prints a seed it takes from the clock (``SynthSeed``, PDFtodSeed) unless --seed is
#: given, and the crop's ``VmPeak``: what changes from run to run without changing the result
VOLATILE = (r"SynthSeed  -?\d+", r"VmPeak \d+", r"MRIvol2volGCAM: t=[ 0-9.]+")
POLICY = oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer",
                             volatile=VOLATILE)  # fmt: skip
CACHED = oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer")


def mask(text: str, args: list[str]) -> str:
    seeded = any(a.lower() == "--seed" for a in args)
    text = re.sub(r"VmPeak \d+", "VmPeak <kB>", text)
    text = re.sub(r"MRIvol2volGCAM: t=[ 0-9.]+", "MRIvol2volGCAM: t=<t>", text)
    if not seeded:
        text = re.sub(r"SynthSeed  -?\d+", "SynthSeed  <seed>", text)
    return text


def files(d: Path) -> dict[str, bytes]:
    return {str(p.relative_to(d)): p.read_bytes() for p in sorted(d.rglob("*")) if p.is_file()}


def official(exe: Path, args: list[str], env: dict, d: Path, runner: str) -> oracle_cache.Completed:
    policy = POLICY if exe.name == "mri_vol2vol" else CACHED
    return oracle_cache.run([exe.name, *args], env=env, cwd=d, outputs=[d], policy=policy,
                            timeout=1200, runner=runner)  # fmt: skip


def compare(port: Path, exe: Path, env: dict, work: Path, run: str, name: str,
            args: list[str]) -> dict:  # fmt: skip
    """One call: the official program, the port with ``reproduce`` and ``fix``, each in a folder
    of its own; the row (identical or not, the notices of ``fix``)."""
    outs = {}
    for side in ("official", "reproduce", "fix"):
        d = work / run / name / side
        if d.exists():
            shutil.rmtree(d)
        d.mkdir(parents=True)
        if side == "official":
            done = official(exe, args, env, d, "fs-vol2vol-real")
            rc, so, se = done.returncode, done.stdout, done.stderr
            ids: list[str] = []
        else:
            p = subprocess.run([str(port), "--upstream-defects", side, "freesurfer",
                                "mri_vol2vol", *args], env=env, cwd=d,
                               capture_output=True, text=True, timeout=1200,
                               check=False)  # fmt: skip
            lines = p.stderr.splitlines()
            se = "".join(x + "\n" for x in lines if not x.startswith("larmorx:"))
            ids = sorted({m[1] for x in lines
                          for m in [re.match(r"larmorx: upstream defect (\S+)", x)] if m})  # fmt: skip
            rc, so = p.returncode, p.stdout
        outs[side] = (rc, mask(so, args), mask(se, args), files(d), ids)
    o = outs["official"][:4]
    row = {"run": run, "call": name, "status": outs["official"][0],
           "reproduce": outs["reproduce"][:4] == o, "fix": outs["fix"][:4] == o,
           "fix_notices": outs["fix"][4]}  # fmt: skip
    print(json.dumps(row), flush=True)
    shutil.rmtree(work / run / name)
    return row


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=lambda s: Path(s).resolve(), required=True)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--runs", nargs="*", help="the v8 runs (default: all)")
    ap.add_argument("--classic", nargs="*", help="the classic runs (default: all)")
    a = ap.parse_args(argv)
    ws = workspace()
    fsh = ws / FS_HOME
    exe = fsh / "bin" / "mri_vol2vol"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "FS_DISABLE_LANG": "1", "USER": "probe",
           **freesurfer_env()}  # fmt: skip
    runs_dir = ws / "oracles" / "freesurfer-8.2.0" / "runs"
    runs = (
        a.runs
        if a.runs is not None
        else sorted(p.name for p in runs_dir.iterdir() if p.name.endswith("-v8"))
    )
    classic = (
        a.classic
        if a.classic is not None
        else sorted(p.name for p in runs_dir.iterdir() if p.name.endswith("-classic"))
    )
    work = ws / "oracles" / "inputs" / "fs-vol2vol-real"
    avg = fsh / "average"
    rows = []
    for run in runs:
        subj = next(p for p in (runs_dir / run / "subjects").iterdir() if p.name != "fsaverage")
        mri = subj / "mri"
        lta = mri / "transforms" / "synthmorph.1.0mm.1.0mm" / "reg.targ_to_invol.lta"
        nu = str(mri / "nu.mgz")
        inter = work / run / "inter"
        if inter.exists():
            shutil.rmtree(inter)
        inter.mkdir(parents=True)
        env["SUBJECTS_DIR"] = str(subj.parent)

        def made(args: list[str], name: str, prog: Path = exe, inter: Path = inter,
                 run: str = run) -> Path:  # fmt: skip
            done = official(prog, args, env, inter, f"fs-vol2vol-real-{name}")
            if done.returncode != 0:
                raise SystemExit(f"{run}: the official {prog.name} {args}: {done.stderr}")
            return inter

        # the scripts' intermediates the runs do not keep: made again by the official programs
        for h in ("lh", "rh"):
            prior = f"mca-dura.prior.warp.mni152.1.0mm.{h}.nii.gz"
            made(
                ["--reg", str(lta), "--mov", str(avg / prior), "--o", f"prior.{h}.nii.gz"],
                f"prior-{h}",
            )
            made(["-T", ".001", "-crop-to-fov-mm", "80", "80", "80", nu,
                  str(inter / f"prior.{h}.nii.gz"), f"{h}.crop.nii.gz"], f"crop-{h}",
                 fsh / "bin" / "mri_mask")  # fmt: skip
            made(["--regheader", "--interp", "nearest", "--mov", str(mri / "aseg.mgz"),
                  "--targ", str(inter / f"{h}.crop.nii.gz"), "--o", f"seg.{h}.mgz"],
                 f"seg-{h}")  # fmt: skip
        lc = fsh / "bin" / "lta_convert"
        made(["--inlta", str(mri / "rawavg2orig.lta"), "--outlta", "orig2rawavg.lta", "--invert"],
             "fsnative2t1w", lc)  # fmt: skip
        calls: list[tuple[str, list[str]]] = [
            ("synthmorph-check", ["--regheader", "--mov", nu, "--targ",
                                  str(avg / "mni_icbm152_nlin_asym_09c" / "reg-targets" /
                                      "mni152.1.0mm.nii.gz"), "--o", "morph.out.nii.gz"]),
            ("vsinus-prior", ["--mov", str(avg / "vsinus.no-sp.prior.mni152.1.0mm.mgz"), "--lta",
                              str(lta), "--targ", nu, "--o", "native.avg12567.prior.mgz"]),
            ("vsinus-back", ["--mov", str(mri / "vsinus.mgz"), "--targ", nu, "--interp",
                             "nearest", "--o", "vsinus.mgz", "--regheader"]),
            ("fmriprep-aseg", ["--mov", str(mri / "aseg.mgz"), "--targ", str(mri / "rawavg.mgz"),
                               "--lta", str(inter / "orig2rawavg.lta"), "--interp", "nearest",
                               "--o", "seg.nii.gz"]),
        ]  # fmt: skip
        for h in ("lh", "rh"):
            calls += [
                (f"mcadura-prior-{h}", ["--reg", str(lta), "--mov",
                                        str(avg / f"mca-dura.prior.warp.mni152.1.0mm.{h}.nii.gz"),
                                        "--o", f"mcadura.prior.{h}.nii.gz"]),
                (f"mcadura-invol-{h}", ["--regheader", "--mov", nu, "--targ",
                                        str(inter / f"{h}.crop.nii.gz"), "--o",
                                        f"invol.{h}.crop.nii.gz"]),
                (f"mcadura-aseg-crop-{h}", ["--regheader", "--interp", "nearest", "--mov",
                                            str(mri / "aseg.mgz"), "--targ",
                                            str(inter / f"{h}.crop.nii.gz"), "--o",
                                            f"seg.{h}.mgz"]),
                (f"mcadura-uncrop-{h}", ["--regheader", "--interp", "nearest", "--mov",
                                         str(inter / f"seg.{h}.mgz"), "--targ", nu, "--o",
                                         f"seg.{h}.uncropped.mgz"]),
            ]  # fmt: skip
        for name, args in calls:
            rows.append(compare(a.port, exe, env, work, run, name, args))
        shutil.rmtree(inter)
    for run in classic:
        subj = next(p for p in (runs_dir / run / "subjects").iterdir() if p.name != "fsaverage")
        mri = subj / "mri"
        env["SUBJECTS_DIR"] = str(subj.parent)
        inter = work / run / "inter"
        if inter.exists():
            shutil.rmtree(inter)
        inter.mkdir(parents=True)
        done = official(exe, ["--mov", str(mri / "norm.mgz"), "--s", subj.name, "--morph", "--o",
                              "norm.morphed.mgz"], env, inter, "fs-vol2vol-real-morphed")  # fmt: skip
        if done.returncode != 0:
            raise SystemExit(f"{run}: the official morph: {done.stderr}")
        m3z = str(mri / "transforms" / "talairach.m3z")
        calls = [
            ("morph-norm", ["--mov", str(mri / "norm.mgz"), "--s", subj.name, "--morph", "--o",
                            "norm.morphed.mgz"]),
            ("morph-aseg", ["--mov", str(mri / "aseg.mgz"), "--s", subj.name, "--morph",
                            "--nearest", "--o", "aseg.morphed.mgz"]),
            ("gcam-norm", ["--gcam", str(mri / "norm.mgz"), "0", m3z, "0", "0", "1",
                           "norm.gcam.mgz"]),
            ("inv-morph-norm", ["--mov", str(mri / "norm.mgz"), "--targ",
                                str(inter / "norm.morphed.mgz"), "--s", subj.name,
                                "--inv-morph", "--o", "norm.back.mgz"]),
        ]  # fmt: skip
        for name, args in calls:
            rows.append(compare(a.port, exe, env, work, run, name, args))
        shutil.rmtree(inter)
    summary = {
        "calls": len(rows),
        "runs": runs,
        "classic": classic,
        "reproduce_identical": sum(r["reproduce"] for r in rows),
        "fix_identical": sum(r["fix"] for r in rows),
        "fix_differ_without_notice": sum(not r["fix"] and not r["fix_notices"] for r in rows),
        "rows": rows,
    }
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 0 if summary["reproduce_identical"] == len(rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
