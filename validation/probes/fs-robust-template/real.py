# SPDX-License-Identifier: Apache-2.0
"""mri_robust_template on fMRIPrep's recorded inputs: FreeSurfer 8.2.0's official program against
the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica; the official program is run as
a black box).

The fMRIPrep oracle runs whose subjects have several T1w images ran sMRIPrep's
``StructuralReference`` (``mri_robust_template``) on their N4-corrected T1w images
(``<workspace>/oracles/fmriprep-25.2.6/<run>/work/.../anat_template_wf/n4_correct/``):
ds004173's three (``acq-headmotion1``, ``acq-headmotion2``, ``acq-standard``) and ds005454's two
(``acq-denoised``, ``acq-mp2rage``). Each recipe runs on those inputs through the official
``mri_robust_template`` (``$FREESURFER_HOME/bin``, by the oracle-output cache, one thread) and
through the port (``larmorx-freesurfer freesurfer mri_robust_template``) with
``--upstream-defects`` ``reproduce`` and ``fix``, and compares the exit status, standard output
(the line of the time taken masked) and error, and every file written (byte for byte):

- ``fmriprep``: the recorded call (``--satit --fixtp --mov ... --inittp 1 --iscale --noit
  --template <t>.nii.gz --subsample 200 --lta ...``);
- ``reconall``: recon-all's motion correction (``--average 1 --satit --inittp 1 --fixtp --noit
  --iscale --iscaleout ... --subsample 200 --lta ...``), its template as ``rawavg.mgz``;
- ``longitudinal``: sMRIPrep's ``--longitudinal`` call (no ``--fixtp``, no ``--noit``: the mean
  space; with three inputs the global iterations), with ``--mapmov``.

The LTAs' creation line is fixed by ``FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING`` and
``USER`` on both sides; ``FS_DISABLE_LANG=1`` on the official side; no diagnostic level.

    python -P validation/probes/fs-robust-template/real.py [--runs a,b] [--recipes a,b] [--json out.json]
"""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

RUNS = {
    "ds004173-sub-603309": ("sub_603309", 3),
    "ds005454-sub-16": ("sub_16", 2),
}
RECIPES = ("fmriprep", "reconall", "longitudinal")
MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
CLI = REPO / "crates-freesurfer" / "target" / "release" / "larmorx-freesurfer"
TIME = re.compile(rb"registration took \d+ minutes and \d+ seconds\.")
FIXED = {
    "LC_ALL": "C",
    "LANG": "C",
    "OMP_NUM_THREADS": "1",
    "USER": "larmorx",
    "TZ": "UTC0",
    "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING": "Thu Jan  1 00:00:00 1970",
}


def inputs_of(ws: Path, run: str) -> list[Path]:
    sub, k = RUNS[run]
    base = (
        ws / "oracles" / "fmriprep-25.2.6" / f"{run}-noreconall" / "work" / "fmriprep_25_2_wf"
        / f"{sub}_wf" / "anat_fit_wf" / "anat_template_wf" / "n4_correct" / "mapflow"
    )  # fmt: skip
    out = []
    for i in range(k):
        found = sorted((base / f"_n4_correct{i}").glob("*_corrected.nii.gz"))
        if len(found) != 1:
            raise SystemExit(f"{run}: no single N4 output in _n4_correct{i}")
        out.append(found[0])
    return out


def args_of(recipe: str, ins: list[Path]) -> list[str]:
    k = len(ins)
    mov = [str(p) for p in ins]
    ltas = [f"tp{i + 1}.lta" for i in range(k)]
    if recipe == "fmriprep":
        return ["--satit", "--fixtp", "--mov", *mov, "--inittp", "1", "--iscale", "--noit",
                "--template", "template.nii.gz", "--subsample", "200", "--lta", *ltas]  # fmt: skip
    if recipe == "reconall":
        iscales = [f"tp{i + 1}-iscale.txt" for i in range(k)]
        return ["--mov", *mov, "--average", "1", "--template", "rawavg.mgz", "--satit",
                "--inittp", "1", "--fixtp", "--noit", "--iscale", "--iscaleout", *iscales,
                "--subsample", "200", "--lta", *ltas]  # fmt: skip
    if recipe == "longitudinal":
        warps = [f"tp{i + 1}-mapped.mgz" for i in range(k)]
        return ["--satit", "--mov", *mov, "--inittp", "1", "--iscale", "--template",
                "template.nii.gz", "--subsample", "200", "--lta", *ltas, "--mapmov", *warps]  # fmt: skip
    raise SystemExit(f"unknown recipe {recipe}")


def files(d: Path) -> dict[str, str]:
    return {
        p.name: hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(d.iterdir())
        if p.is_file()
    }


def official(exe: Path, args: list[str], d: Path) -> dict:
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "FS_DISABLE_LANG": "1", **FIXED,
           **freesurfer_env()}  # fmt: skip
    if any(k.startswith("DIAG") for k in env) or any("diag" in a.lower() for a in args):
        raise SystemExit("a diagnostic setting would print the licence file")
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    t0 = time.monotonic()
    done = oracle_cache.run(
        [exe.name, *args],
        env=env,
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(
            threads=1, family="freesurfer", formats="freesurfer", volatile=(TIME.pattern.decode(),)
        ),
        timeout=None,
        runner="fs-robust-template-real",
        peak=True,
    )
    return {
        "status": done.returncode,
        "stdout": TIME.sub(b"<TIME>", done.stdout.encode("utf-8", "surrogateescape")).decode(),
        "stderr": done.stderr,
        "files": files(d),
        "seconds": round(time.monotonic() - t0, 1),
        "served": done.cached,
        "peak_mib": getattr(done, "peak_mib", None),
    }


def port(args: list[str], d: Path, mode: str) -> dict:
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    env = {"PATH": "/usr/bin:/bin", **FIXED}
    t0 = time.monotonic()
    done = subprocess.run(
        ["/usr/bin/time", "-f", "PEAK_KIB %M", str(CLI), "--upstream-defects", mode, "freesurfer",
         "mri_robust_template", *args],
        env=env, cwd=d, capture_output=True, timeout=14400,
    )  # fmt: skip
    err = done.stderr.decode("utf-8", "surrogateescape")
    peak = re.search(r"PEAK_KIB (\d+)\n?$", err)
    err = err[: peak.start()] if peak else err
    notices = "".join(x for x in err.splitlines(True) if x.startswith("larmorx: "))
    err = "".join(x for x in err.splitlines(True) if not x.startswith("larmorx: "))
    return {
        "status": done.returncode,
        "stdout": TIME.sub(b"<TIME>", done.stdout).decode("utf-8", "surrogateescape"),
        "stderr": err,
        "notices": notices,
        "files": files(d),
        "seconds": round(time.monotonic() - t0, 1),
        "peak_mib": round(int(peak.group(1)) / 1024, 1) if peak else None,
    }


def first_line_difference(a: str, b: str) -> dict | None:
    la, lb = a.splitlines(), b.splitlines()
    for i, (x, y) in enumerate(zip(la, lb, strict=False)):
        if x != y:
            return {"line": i, "official": x[:300], "port": y[:300]}
    if len(la) != len(lb):
        return {"line": min(len(la), len(lb)), "official_lines": len(la), "port_lines": len(lb)}
    return None


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", default=",".join(RUNS))
    ap.add_argument("--recipes", default=",".join(RECIPES))
    ap.add_argument("--parallel", type=int, default=3)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_robust_template"
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-cli", "-j", str(a.jobs),
         "--manifest-path", str(MANIFEST)],
        check=True, env=env,
    )  # fmt: skip
    root = ws / "oracles" / "inputs" / "fs-robust-template" / "real"
    jobs = [(run, recipe) for run in a.runs.split(",") for recipe in a.recipes.split(",")]

    def one(job: tuple[str, str]) -> dict:
        run, recipe = job
        args = args_of(recipe, inputs_of(ws, run))
        base = root / run / recipe
        o = official(exe, args, base / "official")
        rep = port(args, base / "reproduce", "reproduce")
        fix = port(args, base / "fix", "fix")
        same = {k: o[k] == rep[k] for k in ("status", "stdout", "stderr", "files")}
        fix_same = {k: o[k] == fix[k] for k in ("status", "stdout", "files")}
        return {
            "run": run,
            "recipe": recipe,
            "inputs": len(RUNS[run][1] * [0]),
            "reproduce_identical": same,
            "fix_identical": fix_same,
            "reproduce_notices": rep["notices"],
            "fix_notices": fix["notices"],
            "seconds": {"official": o["seconds"], "reproduce": rep["seconds"], "fix": fix["seconds"]},
            "peak_mib": {
                "official": o["peak_mib"],
                "reproduce": rep["peak_mib"],
                "fix": fix["peak_mib"],
            },
            "served": o["served"],
            "files": o["files"],
            "first_difference": None
            if all(same.values())
            else {
                "stdout": first_line_difference(o["stdout"], rep["stdout"]),
                "official_err": o["stderr"][-600:],
                "reproduce_err": rep["stderr"][-600:],
                "files": [o["files"], rep["files"]],
                "status": [o["status"], rep["status"]],
            },
        }

    with concurrent.futures.ThreadPoolExecutor(a.parallel) as pool:
        rows = list(pool.map(one, jobs))
    summary = {
        "calls": len(rows),
        "reproduce_identical": sum(all(r["reproduce_identical"].values()) for r in rows),
        "fix_identical": sum(all(r["fix_identical"].values()) for r in rows),
        "rows": rows,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 0 if summary["reproduce_identical"] == len(rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
