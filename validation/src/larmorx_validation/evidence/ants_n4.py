# SPDX-License-Identifier: Apache-2.0
"""N4BiasFieldCorrection's evidence: fMRIPrep's final N4 of a real T1w, by fMRIPrep, by ANTs
2.6.5 (as built, with correctly rounded maths, at more threads) and by larmorx; the record's
cases; the differences against ANTs' own thread band; speed.

The worked example is the node fMRIPrep 25.2.6 ran in the brain extraction of ds000005 sub-01
(``inu_n4_final``, sMRIPrep's final N4, weighted by the white-matter posterior), with its own
inputs and arguments read from the oracle run's working directory: ``command.txt`` gives the
arguments, and the node's inputs (the denoised T1w, the weight image) are the files fMRIPrep
wrote before it.
"""

from __future__ import annotations

import os
import shlex
import subprocess
from functools import cache
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import common, records

TOOL = "ants-n4"
CASE = "fmriprep-ds000005-final-n4"
RUN = "ds000005-sub-01-noreconall"
WF = "work/fmriprep_25_2_wf/sub_01_wf/anat_fit_wf"
NODE = f"{WF}/brain_extraction_wf/atropos_wf/inu_n4_final/mapflow/_inu_n4_final0"
INPUT = f"{WF}/anat_template_wf/denoise/mapflow/_denoise0/sub-01_T1w_noise_corrected.nii.gz"
WEIGHT = f"{WF}/brain_extraction_wf/atropos_wf/apply_wm_prior/prodmap.nii.gz"
MASK = "out/sub-01/anat/sub-01_desc-brain_mask.nii.gz"
OUT_C, OUT_B = "sub-01_T1w_noise_corrected_corrected.nii.gz", "sub-01_T1w_noise_corrected_bias.nii.gz"
IMAGE = "images/fmriprep-25.2.6.sif"
#: ANTs' thread band, as the record measures it (parity/ants_n4.py's BAND_THREADS)
BAND = (2, 3, 4, 8, 12)
LX_THREADS = (1, 12)


def _oracles() -> Path:
    from larmorx_validation.figures.core import workspace

    ws = workspace()
    if ws is None:
        raise FileNotFoundError("no workspace with oracles/")
    return ws / "oracles"


def _argv(command: str, inp: str, weight: str, out_c: str, out_b: str) -> list[str]:
    """fMRIPrep's arguments (``command.txt``), its paths replaced by ``inp`` ... ``out_b``."""
    argv = shlex.split(command)[1:]
    out = []
    for a in argv:
        a = a.replace("/" + INPUT, inp).replace("/" + WEIGHT, weight)
        out.append(a.replace(OUT_C, out_c).replace(OUT_B, out_b))
    return out


def _compute(e: fcache.Entry) -> None:
    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    ora = _oracles()
    run = ora / "fmriprep-25.2.6" / RUN
    b, lib, sif = ants_bin(), crmath_preload(), ora / IMAGE
    if b is None or lib is None or not sif.is_file() or not run.is_dir():
        raise FileNotFoundError("the ANTs oracle, the CORE-MATH preload or the fMRIPrep run")
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    inp = e.input("input (fMRIPrep's denoised T1w)", run / INPUT)
    weight = e.input("weight image (fMRIPrep's WM posterior × prior)", run / WEIGHT)
    e.input("fMRIPrep's corrected image", run / NODE / OUT_C)
    e.input("fMRIPrep's bias field", run / NODE / OUT_B)
    d = e.dir
    clean = ("LD_PRELOAD", "NSLOTS", "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "OMP_NUM_THREADS")

    def outs(label: str) -> tuple[str, str]:
        return str(d / f"{label}.nii.gz"), str(d / f"{label}-bias.nii.gz")

    # ANTs 2.6.5, the repository's oracle build: 1 thread, CORE-MATH, the thread band
    for label, threads, cr in [
        ("ants-1", 1, False),
        ("ants-1-cr", 1, True),
        *[(f"ants-{t}", t, False) for t in BAND],
    ]:
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
        if cr:
            env["LD_PRELOAD"] = str(lib)
        e.run(
            label,
            [str(b / "N4BiasFieldCorrection"), *_argv(command, str(inp), str(weight), *outs(label))],
            env=env,
            unset=clean,
        )
    # ANTs 2.6.2 in fMRIPrep's own image (Apptainer): 1 thread, as built and with CORE-MATH
    home = d / ".home"
    home.mkdir(exist_ok=True)
    for label, cr in (("fmriprep-image-1", False), ("fmriprep-image-1-cr", True)):
        cmd = ["apptainer", "exec", "--cleanenv", "--home", f"{home}:/home/fmriprep"]
        env = {"NSLOTS": "1", "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}
        if cr:
            cmd += ["-B", f"{lib.parent}:/crmath:ro"]
            env["LD_PRELOAD"] = f"/crmath/{lib.name}"
        for k, v in env.items():
            cmd += ["--env", f"{k}={v}"]
        cmd += ["-B", f"{inp.parent}:/in1:ro", "-B", f"{weight.parent}:/in2:ro", "-B", f"{d}:/out"]
        args = _argv(
            command,
            f"/in1/{inp.name}",
            f"/in2/{weight.name}",
            f"/out/{label}.nii.gz",
            f"/out/{label}-bias.nii.gz",
        )
        e.run(label, [*cmd, str(sif), "N4BiasFieldCorrection", *args], unset=clean)
    # larmorx: 1 and 12 threads, its default (corrections on) and reproduce mode
    for threads in LX_THREADS:
        for mode in ("fix", "reproduce"):
            label = f"larmorx-{threads}" + ("-reproduce" if mode == "reproduce" else "")
            e.run(
                label,
                fcache.larmorx_cli(
                    "ants",
                    "N4BiasFieldCorrection",
                    *_argv(command, str(inp), str(weight), *outs(label)),
                ),
                env={
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                    "LARMORX_UPSTREAM_DEFECTS": mode,
                },
                unset=clean,
            )
    image_version = subprocess.run(
        ["apptainer", "exec", "--cleanenv", str(sif), "N4BiasFieldCorrection", "--version"],
        capture_output=True,
        text=True,
        encoding="utf-8",
    ).stdout.strip()
    e.finish(
        versions={
            "ants": ants_version(),
            "ants_in_fmriprep_image": " ".join(image_version.split()),
            "fmriprep_image": (ora / "images" / "fmriprep-25.2.6.sif.sha256")
            .read_text(encoding="utf-8")
            .split()[0],
        },
        command=command,
        threads={"band": list(BAND), "larmorx": list(LX_THREADS), "fmriprep_run": 8},
        settings="fMRIPrep 25.2.6's final N4 of its brain extraction (inu_n4_final), ds000005 "
        "sub-01, its arguments from command.txt and its inputs from the run's working "
        "directory; fMRIPrep ran it with NSLOTS=8",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's final N4 (inu_n4_final) of ds000005 sub-01: fMRIPrep, ANTs 2.6.5 and larmorx",
        _compute,
    )


@cache
def _load(path: str) -> np.ndarray:
    import nibabel as nib

    return np.asanyarray(nib.load(path).dataobj)


if __name__ == "__main__":  # compute the cache entry only: python -m ...evidence.ants_n4
    from larmorx_validation.figures.core import REPO, default_cache

    os.environ.setdefault("PYTHONUNBUFFERED", "1")
    e = entry(core.Context(repo=REPO, site=Path("."), cache=default_cache()))
    print("done", e.dir)
