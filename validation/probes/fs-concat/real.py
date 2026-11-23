# SPDX-License-Identifier: Apache-2.0
"""mri_concat's recorded calls on the recorded heads: the official program against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

Both streams call ``mri_concat`` from ``pctsurfcon`` (``recon-all.log``: per hemisphere,
``<wm> <gm> --paired-diff-norm --mul 100 --o surf/<h>.w-g.pct.mgh``, the white and grey matter
samples ``mri_vol2surf`` made of ``rawavg.mgz``), and the v8 stream once more from
``mri_mcadura_seg`` (``seg.lh.uncropped.mgz seg.rh.uncropped.mgz --sum --ctab
$FREESURFER_HOME/models/mca-dura.ctab --o mri/mca-dura.mgz``). On each recorded run
(``oracles/freesurfer-8.2.0/runs/*-v8`` and ``*-classic``) the samples are made again by the
official ``mri_vol2surf`` with the commands of its ``recon-all.log`` (the head's own files), and the
official ``mri_concat``'s percent contrast must be the run's recorded ``surf/<h>.w-g.pct.mgh``
byte for byte (the inputs proved the same); the dura's segmentations, which a network made, are
replaced by the head's ``aseg.mgz`` taken onto each hemisphere's crop and back, made by the official
programs the script runs (``mri_vol2vol --reg``, ``mri_mask -crop-to-fov-mm``, ``mri_vol2vol
--regheader --interp nearest``). Each call: the official program through the oracle-output cache
(one thread, ``FS_DISABLE_LANG=1``), the port (``--port <larmorx-freesurfer>``) with
``reproduce`` and ``fix``, each in a folder of its own; exit status, both streams and the files
written compared byte for byte.

    python -P validation/probes/fs-concat/real.py --port <larmorx-freesurfer> [--json out.json]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shlex
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

POLICY = oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer")


def files(d: Path) -> dict[str, bytes]:
    return {str(p.relative_to(d)): p.read_bytes() for p in sorted(d.rglob("*")) if p.is_file()}


def official(exe: Path, args: list[str], env: dict, d: Path, runner: str) -> oracle_cache.Completed:
    return oracle_cache.run([exe.name, *args], env=env, cwd=d, outputs=[d], policy=POLICY,
                            timeout=1200, runner=runner)  # fmt: skip


def compare(port: Path, exe: Path, env: dict, work: Path, run: str, name: str,
            args: list[str], recorded: str | None = None) -> dict:  # fmt: skip
    """One call: the official program, the port with ``reproduce`` and ``fix``, each in a folder
    of its own; the row (identical or not, the notices of ``fix``; the official output against the
    run's recorded file when ``recorded`` gives its SHA-256)."""
    outs = {}
    for side in ("official", "reproduce", "fix"):
        d = work / run / name / side
        if d.exists():
            shutil.rmtree(d)
        d.mkdir(parents=True)
        if side == "official":
            done = official(exe, args, env, d, "fs-concat-real")
            rc, so, se = done.returncode, done.stdout, done.stderr
            ids: list[str] = []
        else:
            p = subprocess.run([str(port), "--upstream-defects", side, "freesurfer",
                                "mri_concat", *args], env=env, cwd=d,
                               capture_output=True, text=True, timeout=1200,
                               check=False)  # fmt: skip
            lines = p.stderr.splitlines()
            se = "".join(x + "\n" for x in lines if not x.startswith("larmorx:"))
            ids = sorted({m[1] for x in lines
                          for m in [re.match(r"larmorx: upstream defect (\S+)", x)] if m})  # fmt: skip
            rc, so = p.returncode, p.stdout
        outs[side] = (rc, so, se, files(d), ids)
    o = outs["official"][:4]
    row = {"run": run, "call": name, "status": outs["official"][0],
           "reproduce": outs["reproduce"][:4] == o, "fix": outs["fix"][:4] == o,
           "fix_notices": outs["fix"][4]}  # fmt: skip
    if recorded is not None:
        written = list(o[3].values())
        row["official_is_recorded"] = (
            len(written) == 1 and hashlib.sha256(written[0]).hexdigest() == recorded
        )
    print(json.dumps(row), flush=True)
    shutil.rmtree(work / run / name)
    return row


def recorded_output(run_dir: Path, rel: str) -> str | None:
    """The SHA-256 the run's record gives the step output ``rel`` (relative to the subject)."""
    for p in sorted((run_dir / "record" / "steps").glob("*.json")):
        d = json.loads(p.read_text(encoding="utf-8"))
        if rel in d.get("outputs", {}):
            return d["outputs"][rel]
    return None


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=lambda s: Path(s).resolve(), required=True)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--runs", nargs="*", help="the runs (default: every -v8 and -classic run)")
    a = ap.parse_args(argv)
    ws = workspace()
    fsh = ws / FS_HOME
    exe = fsh / "bin" / "mri_concat"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "FS_DISABLE_LANG": "1", "USER": "probe",
           **freesurfer_env()}  # fmt: skip
    runs_dir = ws / "oracles" / "freesurfer-8.2.0" / "runs"
    runs = (
        a.runs
        if a.runs is not None
        else sorted(p.name for p in runs_dir.iterdir() if p.name.endswith(("-v8", "-classic")))
    )
    work = ws / "oracles" / "inputs" / "fs-concat-real"
    avg = fsh / "average"
    rows = []
    for run in runs:
        subj = next(p for p in (runs_dir / run / "subjects").iterdir() if p.name != "fsaverage")
        mri = subj / "mri"
        inter = work / run / "inter"
        if inter.exists():
            shutil.rmtree(inter)
        inter.mkdir(parents=True)
        env["SUBJECTS_DIR"] = str(subj.parent)

        def made(args: list[str], name: str, prog: str, inter: Path = inter,
                 run: str = run) -> None:  # fmt: skip
            done = official(fsh / "bin" / prog, args, env, inter, f"fs-concat-real-{name}")
            if done.returncode != 0:
                raise SystemExit(f"{run}: the official {prog} {args}: {done.stderr}")

        log = (subj / "scripts" / "recon-all.log").read_text(encoding="utf-8", errors="replace")
        for h in ("lh", "rh"):
            # pctsurfcon's two samples, made again by its own commands (their temporary folder
            # replaced by ours)
            for kind in ("wm", "gm"):
                cmd = next(
                    shlex.split(x)
                    for x in log.splitlines()
                    if x.startswith("mri_vol2surf ")
                    and "/tmp.pctsurfcon." in x
                    and f"/{h}.{kind}.mgh" in x
                )
                o = cmd.index("--o")
                cmd[o + 1] = f"{h}.{kind}.mgh"
                made(cmd[1:], f"{h}-{kind}", "mri_vol2surf")
            rows.append(compare(a.port, exe, env, work, run, f"pctsurfcon-{h}",
                                [str(inter / f"{h}.wm.mgh"), str(inter / f"{h}.gm.mgh"),
                                 "--paired-diff-norm", "--mul", "100", "--o",
                                 f"{h}.w-g.pct.mgh"],
                                recorded_output(runs_dir / run, f"surf/{h}.w-g.pct.mgh")))  # fmt: skip
        if run.endswith("-v8"):
            lta = mri / "transforms" / "synthmorph.1.0mm.1.0mm" / "reg.targ_to_invol.lta"
            nu = str(mri / "nu.mgz")
            for h in ("lh", "rh"):
                prior = f"mca-dura.prior.warp.mni152.1.0mm.{h}.nii.gz"
                made(["--reg", str(lta), "--mov", str(avg / prior), "--o", f"prior.{h}.nii.gz"],
                     f"prior-{h}", "mri_vol2vol")  # fmt: skip
                made(["-T", ".001", "-crop-to-fov-mm", "80", "80", "80", nu,
                      str(inter / f"prior.{h}.nii.gz"), f"{h}.crop.nii.gz"], f"crop-{h}",
                     "mri_mask")  # fmt: skip
                made(["--regheader", "--interp", "nearest", "--mov", str(mri / "aseg.mgz"),
                      "--targ", str(inter / f"{h}.crop.nii.gz"), "--o", f"seg.{h}.mgz"],
                     f"seg-{h}", "mri_vol2vol")  # fmt: skip
                made(["--regheader", "--interp", "nearest", "--mov", str(inter / f"seg.{h}.mgz"),
                      "--targ", nu, "--o", f"seg.{h}.uncropped.mgz"], f"uncrop-{h}",
                     "mri_vol2vol")  # fmt: skip
            rows.append(compare(a.port, exe, env, work, run, "mcadura-sum",
                                [str(inter / "seg.lh.uncropped.mgz"),
                                 str(inter / "seg.rh.uncropped.mgz"), "--sum", "--ctab",
                                 str(fsh / "models" / "mca-dura.ctab"), "--o",
                                 "mca-dura.mgz"]))  # fmt: skip
        shutil.rmtree(inter)
    summary = {
        "calls": len(rows),
        "runs": runs,
        "reproduce_identical": sum(r["reproduce"] for r in rows),
        "fix_identical": sum(r["fix"] for r in rows),
        "fix_differ_without_notice": sum(not r["fix"] and not r["fix_notices"] for r in rows),
        "official_is_recorded": sum(r.get("official_is_recorded", False) for r in rows),
        "with_a_record": sum("official_is_recorded" in r for r in rows),
        "rows": rows,
    }
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    ok = summary["reproduce_identical"] == len(rows)
    return 0 if ok and summary["official_is_recorded"] == summary["with_a_record"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
