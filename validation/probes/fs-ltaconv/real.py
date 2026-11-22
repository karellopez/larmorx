# SPDX-License-Identifier: Apache-2.0
"""lta_convert's fMRIPrep-like calls on the recorded heads: the official program against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

On three recorded runs (``oracles/freesurfer-8.2.0/runs/``), the calls fMRIPrep and sMRIPrep
make (niworkflows' ``--inlta <lta> --outfsl <mat>``, sMRIPrep's ``--inlta identity.nofile --src
--trg --outlta``, the coregistration's ``--inlta <lta> --outlta``, bbregister's ``--inlta <lta>
--outreg <dat>``, and the ITK and FSL ways back: ``--initk``, ``--infsl`` with the volumes) on
the head's own transforms (``mri/transforms/talairach.xfm.lta`` when the run has it, else
``rawavg2orig.lta``) and volumes, read in place: the official ``lta_convert`` through the
oracle-output cache (one thread, ``FSLOUTPUTTYPE=NIFTI_GZ``, ``FS_DISABLE_LANG=1``), the port in
both modes, each in a folder of its own; exit status, both streams and the files written compared
(the folder as ``<DIR>``, the ``created by`` line of the files ``LTAwrite`` makes aside: it holds
the date).

    python -P validation/probes/fs-ltaconv/real.py --port <larmorx-freesurfer> [--json out.json]
"""

from __future__ import annotations

import argparse
import json
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

RUNS = ("ds000005-sub-01-classic", "ds002422-sub-02-v8", "ds005454-sub-16-classic")


def calls(mri: Path) -> list[tuple[str, list[str]]]:
    lta = mri / "transforms" / "talairach.xfm.lta"
    if not lta.is_file():
        lta = mri / "rawavg2orig.lta"
    src, trg = str(mri / "rawavg.mgz"), str(mri / "orig.mgz")
    return [
        ("outfsl", ["--inlta", str(lta), "--outfsl", "o.mat"]),
        ("identity", ["--inlta", "identity.nofile", "--src", src, "--trg", trg,
                      "--outlta", "o.lta"]),
        ("ras2ras", ["--inlta", str(lta), "--outlta", "o.lta"]),
        ("outreg", ["--inlta", str(lta), "--outreg", "o.dat"]),
        ("outitk", ["--inlta", str(lta), "--outitk", "o.txt", "--outvox", "o.vox",
                    "--outniftyreg", "o.nr", "--outmni", "o.xfm"]),
        ("initk", ["--initk", "@itk", "--src", src, "--trg", trg, "--outlta", "o.lta"]),
        ("infsl", ["--infsl", "@fsl", "--src", src, "--trg", trg, "--outlta", "o.lta",
                   "--invert"]),
    ]  # fmt: skip


def files(d: Path) -> dict[str, bytes]:
    out = {}
    for p in sorted(d.rglob("*")):
        if p.is_file():
            b = p.read_bytes().replace(str(d).encode(), b"<DIR>")
            out[str(p.relative_to(d))] = b"\n".join(
                x for x in b.split(b"\n") if not x.startswith(b"# created by")
            )
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=Path, required=True)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    exe = ws / FS_HOME / "bin" / "lta_convert"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "FS_DISABLE_LANG": "1", "USER": "probe",
           "FSLOUTPUTTYPE": "NIFTI_GZ", **freesurfer_env()}  # fmt: skip
    work = ws / "oracles" / "inputs" / "fs-ltaconv-real"
    rows = []
    for run in RUNS:
        subj = next(p for p in (ws / "oracles" / "freesurfer-8.2.0" / "runs" / run / "subjects")
                    .iterdir() if p.name != "fsaverage")  # fmt: skip
        mri = subj / "mri"
        made: dict[str, Path] = {}
        for name, args in calls(mri):
            outs = {}
            for side in ("official", "reproduce", "fix"):
                d = work / name / side
                if d.exists():
                    shutil.rmtree(d)
                d.mkdir(parents=True)
                arg = [str(made[x[1:]]) if x.startswith("@") else x for x in args]
                if side == "official":
                    done = oracle_cache.run(["lta_convert", *arg], env=env, cwd=d, outputs=[d],
                                            policy=oracle_cache.Policy(threads=1, family="freesurfer",
                                                                       formats="freesurfer"),
                                            timeout=300, runner="fs-ltaconv-real")  # fmt: skip
                    rc, so, se = done.returncode, done.stdout, done.stderr
                else:
                    p = subprocess.run([str(a.port), "--upstream-defects", side, "freesurfer",
                                        "lta_convert", *arg], env=env, cwd=d, capture_output=True,
                                       text=True, timeout=300)  # fmt: skip
                    se = "".join(
                        x + "\n" for x in p.stderr.splitlines() if not x.startswith("larmorx:")
                    )
                    rc, so = p.returncode, p.stdout
                outs[side] = (
                    rc,
                    so.replace(str(d), "<DIR>"),
                    se.replace(str(d), "<DIR>"),
                    files(d),
                )
            # the ITK and FSL files the next calls read: the official's
            if name == "outitk":
                made["itk"] = work / "kept-itk.txt"
                shutil.copyfile(work / name / "official" / "o.txt", made["itk"])
            if name == "outfsl":
                made["fsl"] = work / "kept-fsl.mat"
                shutil.copyfile(work / name / "official" / "o.mat", made["fsl"])
            row = {"run": run, "call": name, "status": outs["official"][0],
                   "reproduce": outs["reproduce"] == outs["official"],
                   "fix": outs["fix"] == outs["official"]}  # fmt: skip
            rows.append(row)
            print(json.dumps(row), flush=True)
    shutil.rmtree(work)
    summary = {
        "calls": len(rows),
        "reproduce_identical": sum(r["reproduce"] for r in rows),
        "fix_identical": sum(r["fix"] for r in rows),
        "rows": rows,
    }
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 0 if summary["reproduce_identical"] == len(rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
