# SPDX-License-Identifier: Apache-2.0
"""mri_coreg on recorded heads, fMRIPrep's and bbregister's calls: FreeSurfer 8.2.0's official
program against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica; the official program is run as
a black box).

Each head pairs a FreeSurfer reference run's subject (``<workspace>/oracles/freesurfer-8.2.0/runs/
<head>-v8/subjects/<subject>``: its ``brainmask.mgz`` the reference, its ``aparc+aseg.mgz`` the
reference's mask) with the BOLD reference fMRIPrep registered to the anatomy
(``<workspace>/oracles/fmriprep-25.2.6/<head>-noreconall/out/<subject>/func/*desc-coreg_boldref.nii.gz``,
the first run's). Two recipes, run through the official ``mri_coreg`` (``$FREESURFER_HOME/bin``,
by the oracle-output cache, one thread) and through the port (``larmorx-freesurfer freesurfer
mri_coreg``) with ``--upstream-defects`` ``reproduce`` and ``fix``; the exit status, standard
output (the times masked) and error, and every file written (byte for byte) compared:

- ``fmriprep``: fMRIPrep's ``MRICoreg`` (``--s <subject> --sd <subjects> --mov <boldref> --lta
  registration.lta --dof 6 --sep 4 --ftol 1.000000e-04 --linmintol 1.000000e-02``);
- ``bbregister``: ``bbregister --init-coreg``'s (``--s <subject> --mov <boldref> --regdat
  reg.init.dat --reg mri_coreg.lta --nthreads 1 --dof 6 --sep 4 --ftol .0001 --linmintol .01``,
  ``SUBJECTS_DIR`` from the environment).

The LTAs' creation line is fixed by ``FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING`` and
``USER`` on both sides; ``FS_DISABLE_LANG=1`` on the official side; no diagnostic level.

    python -P validation/probes/fs-coreg/real.py [--heads a,b] [--recipes a,b] [--json out.json]
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

HEADS = {
    "ds000005-sub-01": "sub-01",
    "ds000228-sub-pixar008": "sub-pixar008",
    "ds005454-sub-16": "sub-16",
    "ds002422-sub-02": "sub-02",
}
RECIPES = ("fmriprep", "bbregister")
MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
CLI = REPO / "crates-freesurfer" / "target" / "release" / "larmorx-freesurfer"
TIMES = (
    (re.compile(rb"OptTimeSec +[0-9.]+ sec"), b"OptTimeSec <T> sec"),
    (re.compile(rb"OptTimeMin +[0-9.]+ min"), b"OptTimeMin <T> min"),
    (re.compile(rb"mri_coreg RunTimeSec +[0-9.]+ sec"), b"mri_coreg RunTimeSec <T> sec"),
)
FIXED = {
    "LC_ALL": "C",
    "LANG": "C",
    "OMP_NUM_THREADS": "1",
    "USER": "larmorx",
    "TZ": "UTC0",
    "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING": "Thu Jan  1 00:00:00 1970",
}


def mask(b: bytes, d: Path) -> str:
    for t, rep in TIMES:
        b = t.sub(rep, b)
    return b.replace(str(d).encode(), b"<DIR>").decode("utf-8", "surrogateescape")


def inputs_of(ws: Path, head: str) -> tuple[Path, Path]:
    sub = HEADS[head]
    sd = ws / "oracles" / "freesurfer-8.2.0" / "runs" / f"{head}-v8" / "subjects"
    func = ws / "oracles" / "fmriprep-25.2.6" / f"{head}-noreconall" / "out" / sub / "func"
    found = sorted(func.glob("*desc-coreg_boldref.nii.gz"))
    if not found or not (sd / sub / "mri" / "brainmask.mgz").is_file():
        raise SystemExit(f"{head}: no BOLD reference or subject")
    return sd, found[0]


def args_of(recipe: str, sub: str, sd: Path, mov: Path) -> tuple[list[str], dict[str, str]]:
    if recipe == "fmriprep":
        return (["--s", sub, "--sd", str(sd), "--mov", str(mov), "--lta", "registration.lta",
                 "--dof", "6", "--sep", "4", "--ftol", "1.000000e-04", "--linmintol",
                 "1.000000e-02"], {})  # fmt: skip
    if recipe == "bbregister":
        return (["--s", sub, "--mov", str(mov), "--regdat", "reg.init.dat", "--reg",
                 "mri_coreg.lta", "--nthreads", "1", "--dof", "6", "--sep", "4", "--ftol",
                 ".0001", "--linmintol", ".01"], {"SUBJECTS_DIR": str(sd)})  # fmt: skip
    raise SystemExit(f"unknown recipe {recipe}")


def files(d: Path) -> dict[str, str]:
    return {
        p.name: hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(d.iterdir())
        if p.is_file()
    }


def official(exe: Path, args: list[str], extra: dict[str, str], d: Path) -> dict:
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "FS_DISABLE_LANG": "1", **FIXED,
           **freesurfer_env(), **extra}  # fmt: skip
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
            threads=1,
            family="freesurfer",
            formats="freesurfer",
            volatile=tuple(t.pattern.decode() for t, _ in TIMES),
        ),
        timeout=None,
        runner="fs-coreg-real",
        peak=True,
    )
    return {
        "status": done.returncode,
        "stdout": mask(done.stdout.encode("utf-8", "surrogateescape"), d),
        "stderr": done.stderr,
        "files": files(d),
        "seconds": round(time.monotonic() - t0, 1),
        "served": done.cached,
        "peak_mib": getattr(done, "peak_mib", None),
    }


def port(args: list[str], extra: dict[str, str], d: Path, mode: str, home: str) -> dict:
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    env = {"PATH": "/usr/bin:/bin", **FIXED, "FREESURFER_HOME": home, **extra}
    t0 = time.monotonic()
    done = subprocess.run(
        ["/usr/bin/time", "-f", "PEAK_KIB %M", str(CLI), "--upstream-defects", mode, "freesurfer",
         "mri_coreg", *args],
        env=env, cwd=d, capture_output=True, timeout=14400,
    )  # fmt: skip
    err = done.stderr.decode("utf-8", "surrogateescape")
    peak = re.search(r"PEAK_KIB (\d+)\n?$", err)
    err = err[: peak.start()] if peak else err
    notices = "".join(x for x in err.splitlines(True) if x.startswith("larmorx: "))
    err = "".join(x for x in err.splitlines(True) if not x.startswith("larmorx: "))
    return {
        "status": done.returncode,
        "stdout": mask(done.stdout, d),
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
    ap.add_argument("--heads", default=",".join(HEADS))
    ap.add_argument("--recipes", default=",".join(RECIPES))
    ap.add_argument("--parallel", type=int, default=3)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_coreg"
    home = str(ws / FS_HOME)
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-cli", "-j", str(a.jobs),
         "--manifest-path", str(MANIFEST)],
        check=True, env=env,
    )  # fmt: skip
    root = ws / "oracles" / "inputs" / "fs-coreg" / "real"
    jobs = [(h, r) for h in a.heads.split(",") for r in a.recipes.split(",")]

    def one(job: tuple[str, str]) -> dict:
        head, recipe = job
        sd, mov = inputs_of(ws, head)
        args, extra = args_of(recipe, HEADS[head], sd, mov)
        base = root / head / recipe
        o = official(exe, args, extra, base / "official")
        rep = port(args, extra, base / "reproduce", "reproduce", home)
        fix = port(args, extra, base / "fix", "fix", home)
        for x, d in ((o, base / "official"), (rep, base / "reproduce"), (fix, base / "fix")):
            x["stdout"] = x["stdout"].replace(str(d), "<DIR>")
        same = {k: o[k] == rep[k] for k in ("status", "stdout", "stderr", "files")}
        fix_same = {k: o[k] == fix[k] for k in ("status", "stdout", "files")}
        return {
            "head": head,
            "recipe": recipe,
            "reproduce_identical": same,
            "fix_identical": fix_same,
            "reproduce_notices": rep["notices"],
            "fix_notices": fix["notices"],
            "seconds": {
                "official": o["seconds"],
                "reproduce": rep["seconds"],
                "fix": fix["seconds"],
            },
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
            "fix_first_difference": None
            if all(fix_same.values())
            else first_line_difference(o["stdout"], fix["stdout"]),
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
