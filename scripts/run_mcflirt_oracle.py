# SPDX-License-Identifier: Apache-2.0
"""Run FSL's mcflirt as a black-box oracle for ``lx.mri.hmc`` (specs/mcflirt.md).

Runs mcflirt (FSL 6.0.7.x, package fsl-mcflirt) on real BOLD runs from larmorx-testdata and on
the synthetic head-motion series (``synthetic/hmc/``), with fMRIPrep's command line, mcflirt's
defaults and a sweep of its options, plus edge cases. Every run gets its own directory with the
exact command (``cmd.json``), stdout/stderr, exit code, wall time and SHA-256 of every input
and output (``run.json``). ``manifest.json`` lists all runs. FSL itself is not touched: its
binaries are called by absolute path with a clean environment (``~/fsl/bin`` is never put on
PATH, because FSL ships its own compilers), ``FSLOUTPUTTYPE=NIFTI_GZ`` unless a case sets it.
mcflirt is single-threaded; ``--jobs`` runs that many mcflirt processes at once.

Real runs without an fMRIPrep boldref get a reference made here: the voxel-wise median of the
first 10 volumes, saved as float32 with the run's header (``refs/``). That reference only has
to be fixed and recorded; it is not meant to reproduce fMRIPrep's.

Usage (from anywhere; the workspace is found from this file's location)::

    python scripts/run_mcflirt_oracle.py [--jobs 3] [--only SUBSTRING ...] [--list] [--force]

Needs nibabel and numpy (the workspace .venv) and an editable larmorx-testdata install. The
outputs (outside git) go to ``<workspace>/oracles/fsl-6.0.7/mcflirt/``.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import platform
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path

FSL = Path.home() / "fsl"
MCFLIRT = FSL / "bin" / "mcflirt"
FSL_PACKAGES = (
    "fsl-mcflirt",
    "fsl-flirt",
    "fsl-newimage",
    "fsl-miscmaths",
    "fsl-armawrap",
    "fsl-newnifti",
    "fsl-utils",
    "fsl-znzlib",
)


def find_workspace() -> Path:
    for parent in Path(__file__).resolve().parents:
        if (parent / "larmorx-testdata").is_dir() and (parent / "oracles").is_dir():
            return parent
    raise SystemExit("workspace not found (a parent with larmorx-testdata/ and oracles/)")


WS = find_workspace()
OUT = WS / "oracles" / "fsl-6.0.7" / "mcflirt"
SYN = "synthetic/hmc/"


@dataclass
class Run:
    name: str
    steps: list[list[str]]  # argument lists; "{in}", "{ref}", "{out}", "{dir}" are expanded
    inputs: dict[str, str]  # placeholder -> catalog path, "ref:<name>" or "file:<name>"
    notes: str = ""
    env: dict[str, str | None] = field(default_factory=dict)  # None unsets a variable
    expect_fail: bool = False
    files: dict[str, str] = field(default_factory=dict)  # extra files written into the run dir


# ------------------------------------------------------------------------------------------------
# Inputs

REAL = {
    # name: (bold, reference or None for the median-of-10 reference)
    "ds000005": (
        "openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz",
        "openneuro-derivatives/ds000005-fmriprep/sub-01/func/"
        "sub-01_task-mixedgamblestask_run-1_boldref.nii.gz",
    ),
    "ds000117": (
        "openneuro/ds000117/sub-01/ses-mri/func/"
        "sub-01_ses-mri_task-facerecognition_run-01_bold.nii.gz",
        None,
    ),
    "ds000210-rest-echo2": (
        "openneuro/ds000210/sub-02/func/sub-02_task-rest_run-01_echo-2_bold.nii.gz",
        None,
    ),
    "ds006010": ("openneuro/ds006010/sub-206/func/sub-206_task-category_run-01_bold.nii.gz", None),
    "ds005040": ("openneuro/ds005040/sub-002/func/sub-002_task-clip_run-1_bold.nii.gz", None),
    "ds000122": (
        "openneuro/ds000122/sub-17/func/sub-17_task-visualattentiontask_run-01_bold.nii.gz",
        None,
    ),
    "ds003345": (
        "openneuro/ds003345/sub-22973/func/sub-22973_task-PenaltyKik_run-02_bold.nii.gz",
        None,
    ),
    "ds006736": ("openneuro/ds006736/sub-004/func/sub-004_task-freeRecall_bold.nii.gz", None),
    "ds000258-bigendian": (
        "openneuro/ds000258/sub-21262/func/sub-21262_task-rest_echo-4_bold.nii.gz",
        None,
    ),
}

FMRIPREP = ["-in", "{in}", "-out", "{out}", "-reffile", "{ref}", "-mats"]  # nipype's order
REPORTS = ["-plots", "-rmsrel", "-rmsabs"]
DEFAULT = ["-in", "{in}", "-out", "{out}", "-mats", *REPORTS]


def real_runs() -> list[Run]:
    runs = []
    for name, (bold, ref) in REAL.items():
        refkey = ref if ref else f"ref:{name}"
        io = {"in": bold, "ref": refkey}
        runs.append(
            Run(
                f"real/{name}/fmriprep",
                [FMRIPREP],
                io,
                "fMRIPrep's command (nipype MCFLIRT(save_mats=True), ref_file)",
            )
        )
        runs.append(
            Run(
                f"real/{name}/fmriprep-reports",
                [FMRIPREP + REPORTS],
                io,
                "fMRIPrep's command plus -plots -rmsrel -rmsabs",
            )
        )
        runs.append(
            Run(
                f"real/{name}/default",
                [DEFAULT],
                {"in": bold},
                "mcflirt's defaults (middle volume as reference)",
            )
        )
    # determinism: the fMRIPrep command repeated on ds000005
    bold, ref = REAL["ds000005"]
    for k in (2, 3):
        runs.append(
            Run(
                f"real/ds000005/fmriprep-repeat{k}",
                [FMRIPREP],
                {"in": bold, "ref": ref},
                "repeat of real/ds000005/fmriprep (run-to-run determinism)",
            )
        )
    # option sweep on ds000005 (fMRIPrep's command plus reports plus the option)
    sweep = {
        "stages1": ["-stages", "1"],
        "stages2": ["-stages", "2"],
        "stages4": ["-stages", "4"],
        "sinc_final": ["-sinc_final"],
        "spline_final": ["-spline_final"],
        "nn_final": ["-nn_final"],
        "stages4-sinc_final": ["-stages", "4", "-sinc_final"],
        "meanvol": ["-meanvol"],
        "stats": ["-stats"],
        "cost-corratio": ["-cost", "corratio"],
        "cost-mutualinfo": ["-cost", "mutualinfo"],
        "cost-normmi": ["-cost", "normmi"],
        "cost-woods": ["-cost", "woods"],
        "cost-leastsquares": ["-cost", "leastsquares"],
        "cost-normcorr": ["-cost", "normcorr"],
        "dof7": ["-dof", "7"],
        "dof9": ["-dof", "9"],
        "dof12": ["-dof", "12"],
        "smooth0": ["-smooth", "0"],
        "smooth2": ["-smooth", "2"],
        "rotation2": ["-rotation", "2"],
        "fudge": ["-fudge"],
        "scaling3": ["-scaling", "3"],
        "bins64": ["-bins", "64"],
        "2d": ["-2d"],
        "gdt": ["-gdt"],
    }
    for key, opt in sweep.items():
        runs.append(
            Run(
                f"real/ds000005/opt-{key}",
                [FMRIPREP + REPORTS + opt],
                {"in": bold, "ref": ref},
                f"option sweep: {' '.join(opt)}"
                + (
                    " (writes grefvol_<out>, impossible with an absolute -out)"
                    if key == "gdt"
                    else ""
                ),
                expect_fail=key == "gdt",
            )
        )
    runs.append(
        Run(
            "real/ds000005/opt-refvol0",
            [[*DEFAULT, "-refvol", "0"]],
            {"in": bold},
            "default with -refvol 0",
        )
    )
    runs.append(
        Run(
            "real/ds000005/opt-meanvol-noref",
            [[*DEFAULT, "-meanvol"]],
            {"in": bold},
            "-meanvol without -reffile",
        )
    )
    runs.append(
        Run(
            "real/ds003763-truncated/default",
            [DEFAULT],
            {"in": "openneuro/ds003763/sub-16111/func/sub-16111_task-heart_bold.nii.gz"},
            "a .nii.gz truncated as published (65,536 bytes)",
            expect_fail=True,
        )
    )
    return runs


def syn_runs() -> list[Run]:
    ref = SYN + "phantom/ref.nii.gz"
    runs = []
    series = [
        "motion/small",
        "motion/large",
        "motion/identical-copies",
        "orientation/ras",
        "orientation/ras-unflipped",
        "orientation/oblique",
        "dtypes/float32",
        "dtypes/float64",
        "dtypes/uint16",
        "dtypes/uint8",
        "dtypes/int16-scaled",
        "content/nan",
        "content/background",
        "geometry/single-volume",
        "geometry/single-volume-3d",
        "geometry/two-volumes",
    ]
    for s in series:
        io = {"in": SYN + s + ".nii.gz", "ref": ref}
        runs.append(Run(f"syn/{s}/reffile", [FMRIPREP + REPORTS], io, "-reffile phantom/ref"))
        runs.append(Run(f"syn/{s}/default", [DEFAULT], {"in": io["in"]}, "defaults"))
    small = SYN + "motion/small.nii.gz"
    runs += [
        Run(
            "syn/motion/small/reffile-ras",
            [FMRIPREP + REPORTS],
            {"in": small, "ref": SYN + "phantom/ref-ras.nii.gz"},
            "reference stored RAS (x-flipped data): same physical image as phantom/ref",
        ),
        Run(
            "syn/motion/small/reffile-grid2",
            [FMRIPREP + REPORTS],
            {"in": small, "ref": SYN + "phantom/ref-grid2.nii.gz"},
            "reference on a different grid",
            expect_fail=True,
        ),
        Run(
            "syn/motion/small/reffile-4d",
            [FMRIPREP + REPORTS],
            {"in": small, "ref": small},
            "a 4D file as -reffile (its first volume is used)",
        ),
        Run(
            "syn/motion/small/init",
            [FMRIPREP + REPORTS + ["-init", "{dir}/init.mat"]],
            {"in": small, "ref": ref},
            "-init with a 5 mm x translation",
            files={"init.mat": "1 0 0 5\n0 1 0 0\n0 0 1 0\n0 0 0 1\n"},
        ),
        Run(
            "syn/motion/small/refvol-out-of-range",
            [[*DEFAULT, "-refvol", "12"]],
            {"in": small},
            "-refvol 12 with 12 volumes",
            expect_fail=True,
        ),
        Run("syn/motion/small/refvol-0", [[*DEFAULT, "-refvol", "0"]], {"in": small}, ""),
        Run("syn/motion/small/meanvol", [[*DEFAULT, "-meanvol"]], {"in": small}, ""),
        Run(
            "syn/motion/small/stages4",
            [FMRIPREP + REPORTS + ["-stages", "4"]],
            {"in": small, "ref": ref},
            "",
        ),
        Run(
            "syn/motion/small/sinc_final",
            [FMRIPREP + REPORTS + ["-sinc_final"]],
            {"in": small, "ref": ref},
            "",
        ),
        Run(
            "syn/motion/small/spline_final",
            [FMRIPREP + REPORTS + ["-spline_final"]],
            {"in": small, "ref": ref},
            "",
        ),
        Run(
            "syn/motion/small/nn_final",
            [FMRIPREP + REPORTS + ["-nn_final"]],
            {"in": small, "ref": ref},
            "",
        ),
        Run(
            "syn/motion/small/2d",
            [FMRIPREP + REPORTS + ["-2d", "-report"]],
            {"in": small, "ref": ref},
            "-2d forces in-plane registration",
        ),
        Run(
            "syn/motion/small/smooth0",
            [FMRIPREP + REPORTS + ["-smooth", "0"]],
            {"in": small, "ref": ref},
            "",
        ),
        Run(
            "syn/motion/small/stats",
            [FMRIPREP + REPORTS + ["-stats"]],
            {"in": small, "ref": ref},
            "",
        ),
        Run(
            "syn/motion/small/report",
            [FMRIPREP + REPORTS + ["-report"]],
            {"in": small, "ref": ref},
            "progress messages",
        ),
        Run(
            "syn/motion/small/default-report",
            [[*DEFAULT, "-report"]],
            {"in": small},
            "progress messages, middle-volume reference",
        ),
        Run(
            "syn/motion/small/no-out",
            [["-in", "{dir}/small.nii.gz", "-mats", "-plots"]],
            {"in": small},
            "no -out: the output name is <input>_mcf",
            files={"copy:small.nii.gz": "in"},
        ),
        Run(
            "syn/motion/small/out-twice",
            [FMRIPREP + REPORTS, FMRIPREP + REPORTS],
            {"in": small, "ref": ref},
            "the same -out twice: the second .mat directory gets a '+'",
        ),
        Run(
            "syn/motion/small/out-no-ext",
            [
                [
                    "-in",
                    "{in}",
                    "-out",
                    "{dir}/plain",
                    "-reffile",
                    "{ref}",
                    "-mats",
                    "-plots",
                    "-rmsrel",
                    "-rmsabs",
                ]
            ],
            {"in": small, "ref": ref},
            "-out without an extension",
        ),
        Run(
            "syn/motion/small/fsloutputtype-nifti",
            [FMRIPREP + REPORTS],
            {"in": small, "ref": ref},
            "FSLOUTPUTTYPE=NIFTI with -out ending in .nii.gz",
            env={"FSLOUTPUTTYPE": "NIFTI"},
        ),
        Run(
            "syn/motion/small/unknown-option",
            [[*FMRIPREP, "-edge"]],
            {"in": small, "ref": ref},
            "-edge (offered by nipype) is not an mcflirt option",
            expect_fail=True,
        ),
        Run(
            "syn/motion/small/unknown-cost",
            [FMRIPREP + REPORTS + ["-cost", "bogus"]],
            {"in": small, "ref": ref},
            "an unknown cost name",
        ),
        Run(
            "syn/motion/small/no-input",
            [["-out", "{out}", "-mats"]],
            {},
            "no -in",
            expect_fail=True,
        ),
        Run(
            "syn/content/ramp/reffile",
            [FMRIPREP + REPORTS],
            {"in": SYN + "content/ramp.nii.gz", "ref": SYN + "content/ramp-ref.nii.gz"},
            "ramp moved along x: the default cost finds the shifts",
        ),
        Run(
            "syn/content/ramp/reffile-smooth0",
            [FMRIPREP + REPORTS + ["-smooth", "0"]],
            {"in": SYN + "content/ramp.nii.gz", "ref": SYN + "content/ramp-ref.nii.gz"},
            "ramp with -smooth 0 (the unweighted cost)",
        ),
        Run(
            "syn/geometry/thin-slab/default",
            [[*DEFAULT, "-report"]],
            {"in": SYN + "geometry/thin-slab.nii.gz"},
            "16 mm slab: in-plane registration",
        ),
        Run("syn/geometry/tiny/default", [DEFAULT], {"in": SYN + "geometry/tiny.nii.gz"}, ""),
        Run(
            "syn/geometry/thin-slab/rotation2",
            [[*DEFAULT, "-rotation", "2"]],
            {"in": SYN + "geometry/thin-slab.nii.gz"},
            "in-plane mode prints the tolerances",
        ),
        Run(
            "syn/geometry/thin-slab/fov10",
            [[*DEFAULT, "-fov", "10"]],
            {"in": SYN + "geometry/thin-slab.nii.gz"},
            "-fov 10: the 8 mm reference grid still has fewer than 3 slices",
        ),
        Run(
            "syn/motion/small/dof7",
            [FMRIPREP + REPORTS + ["-dof", "7"]],
            {"in": small, "ref": ref},
            "",
        ),
        Run(
            "syn/motion/small/dof12",
            [FMRIPREP + REPORTS + ["-dof", "12"]],
            {"in": small, "ref": ref},
            "",
        ),
        Run(
            "syn/motion/small/stages1",
            [FMRIPREP + REPORTS + ["-stages", "1"]],
            {"in": small, "ref": ref},
            "",
        ),
        Run(
            "syn/motion/small/fudge",
            [FMRIPREP + REPORTS + ["-fudge"]],
            {"in": small, "ref": ref},
            "-fudge: no initialisation from the previous volume",
        ),
        Run(
            "syn/motion/identical-copies/reffile-fudge",
            [FMRIPREP + REPORTS + ["-fudge"]],
            {"in": SYN + "motion/identical-copies.nii.gz", "ref": ref},
            "-fudge on identical copies: every volume starts from the identity",
        ),
        Run(
            "syn/motion/small/nifti2",
            [FMRIPREP + REPORTS],
            {"in": small, "ref": ref},
            "FSLOUTPUTTYPE=NIFTI2_GZ",
            env={"FSLOUTPUTTYPE": "NIFTI2_GZ"},
        ),
        Run(
            "syn/cli/rmsrel-only",
            [["-in", "{in}", "-out", "{out}", "-rmsrel"]],
            {"in": small},
            "-rmsrel without -mats: which files appear in the .mat directory",
        ),
        Run(
            "syn/cli/plots-only",
            [["-in", "{in}", "-out", "{out}", "-plots"]],
            {"in": small},
            "-plots without -mats: is a .mat directory created",
        ),
        Run("syn/cli/help", [["-help"]], {}, "-help alone", expect_fail=True),
        Run("syn/cli/no-arguments", [[]], {}, "no arguments", expect_fail=True),
        Run(
            "syn/cli/help-with-input", [["-in", "{in}", "-help"]], {"in": small}, "-help after -in"
        ),
        Run(
            "syn/cli/unknown-option-middle",
            [["-in", "{in}", "-edge", "-out", "{out}"]],
            {"in": small},
            "an unknown option followed by others",
            expect_fail=True,
        ),
        Run(
            "syn/cli/bare-input",
            [["{in}", "-out", "{out}", "-mats"]],
            {"in": small},
            "input given without -in",
        ),
        Run("syn/cli/refvol-minus1", [[*DEFAULT, "-refvol", "-1"]], {"in": small}, "-refvol -1"),
        Run(
            "syn/cli/refvol-minus2",
            [[*DEFAULT, "-refvol", "-2"]],
            {"in": small},
            "-refvol -2",
            expect_fail=True,
        ),
        Run(
            "syn/cli/dof5",
            [FMRIPREP + REPORTS + ["-dof", "5"]],
            {"in": small, "ref": ref},
            "-dof 5 (below 6)",
        ),
        Run(
            "syn/cli/stages0",
            [FMRIPREP + REPORTS + ["-stages", "0"]],
            {"in": small, "ref": ref},
            "-stages 0 with -reffile: the reference is never read",
            expect_fail=True,
        ),
        Run(
            "syn/motion/small/no-fsloutputtype",
            [FMRIPREP],
            {"in": small, "ref": ref},
            "FSLOUTPUTTYPE unset",
            env={"FSLOUTPUTTYPE": None},
            expect_fail=True,
        ),
    ]
    return runs


ALL = real_runs() + syn_runs()


# ------------------------------------------------------------------------------------------------
# Helpers


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def catalog_path(rel: str) -> Path:
    import larmorx_testdata as td

    entry = next(e for e in td.catalog() if e.path == rel)
    return Path(td.get(entry))


def median_reference(name: str, bold_rel: str) -> Path:
    import nibabel as nib
    import numpy as np

    out = OUT / "refs" / f"{name}_median10.nii.gz"
    if out.exists():
        return out
    out.parent.mkdir(parents=True, exist_ok=True)
    img = nib.load(catalog_path(bold_rel))
    data = np.asarray(img.dataobj[..., :10], dtype=np.float64)
    ref = np.median(data, axis=-1).astype(np.float32)
    hdr = img.header.copy()
    hdr.set_data_dtype(np.float32)
    nib.save(nib.Nifti1Image(ref, img.affine, hdr), out)
    return out


def resolve(key: str) -> Path:
    if key.startswith("ref:"):
        name = key[4:]
        return median_reference(name, REAL[name][0])
    return catalog_path(key)


def fsl_versions() -> dict:
    info = {"fslversion": (FSL / "etc" / "fslversion").read_text().strip()}
    for meta in sorted((FSL / "conda-meta").glob("fsl-*.json")):
        d = json.loads(meta.read_text())
        if d["name"] in FSL_PACKAGES:
            info[d["name"]] = f"{d['version']} ({d['build']}) sha256 {d.get('sha256')}"
    return info


def run_one(run: Run, force: bool) -> dict:
    rundir = OUT / "runs" / run.name
    if (rundir / "run.json").exists() and not force:
        return json.loads((rundir / "run.json").read_text())
    if rundir.exists():
        shutil.rmtree(rundir)
    rundir.mkdir(parents=True)
    inputs = {k: resolve(v) for k, v in run.inputs.items()}
    for fname, content in run.files.items():
        if fname.startswith("copy:"):
            shutil.copy(inputs[content], rundir / fname[5:])
        else:
            (rundir / fname).write_text(content)
    subst = {"{dir}": str(rundir), "{out}": str(rundir / "out_mcf.nii.gz")}
    subst.update({"{" + k + "}": str(p) for k, p in inputs.items()})
    env = {
        "HOME": str(Path.home()),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(FSL),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        **run.env,
    }
    env = {k: v for k, v in env.items() if v is not None}
    record = {
        "name": run.name,
        "notes": run.notes,
        "expect_fail": run.expect_fail,
        "env": env,
        "host": platform.node(),
        "machine": platform.machine(),
        "threads": "mcflirt is single-threaded (one process per run)",
        "inputs": {k: {"path": str(p), "sha256": sha256(p)} for k, p in inputs.items()},
        "steps": [],
    }
    for i, step in enumerate(run.steps):
        argv = [str(MCFLIRT)] + [_expand(a, subst) for a in step]
        t0 = time.time()
        proc = subprocess.run(argv, cwd=rundir, env=env, capture_output=True, text=True)
        wall = time.time() - t0
        (rundir / f"stdout{i}.txt").write_text(proc.stdout)
        (rundir / f"stderr{i}.txt").write_text(proc.stderr)
        record["steps"].append(
            {
                "argv": argv,
                "exit": proc.returncode,
                "wall_s": round(wall, 3),
                "started": dt.datetime.fromtimestamp(t0).isoformat(),
            }
        )
    outputs = {}
    for p in sorted(rundir.rglob("*")):
        if p.is_file() and p.name not in ("run.json",):
            outputs[p.relative_to(rundir).as_posix()] = sha256(p)
    record["outputs"] = outputs
    record["ok"] = all((s["exit"] != 0) == run.expect_fail for s in record["steps"][-1:])
    (rundir / "run.json").write_text(json.dumps(record, indent=1) + "\n")
    return record


def _expand(arg: str, subst: dict[str, str]) -> str:
    for k, v in subst.items():
        arg = arg.replace(k, v)
    return arg


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--only", nargs="*", default=[])
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--force", action="store_true")
    args = ap.parse_args()
    runs = [r for r in ALL if not args.only or any(s in r.name for s in args.only)]
    if args.list:
        for r in runs:
            print(r.name)
        return 0
    OUT.mkdir(parents=True, exist_ok=True)
    # references first (serially), so parallel runs do not race on them
    for r in runs:
        for v in r.inputs.values():
            resolve(v)
    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        records = list(pool.map(lambda r: run_one(r, args.force), runs))
    manifest_path = OUT / "manifest.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {"runs": {}}
    manifest["fsl"] = fsl_versions()
    manifest["mcflirt"] = str(MCFLIRT)
    manifest["generated_by"] = "larmorx scripts/run_mcflirt_oracle.py"
    manifest["updated"] = dt.datetime.now().isoformat(timespec="seconds")
    for rec in records:
        manifest["runs"][rec["name"]] = {
            "ok": rec["ok"],
            "exit": [s["exit"] for s in rec["steps"]],
            "wall_s": [s["wall_s"] for s in rec["steps"]],
        }
    manifest_path.write_text(json.dumps(manifest, indent=1, sort_keys=True) + "\n")
    bad = [r["name"] for r in records if not r["ok"]]
    print(f"{len(records)} runs, {len(bad)} unexpected exit codes: {bad}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
