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
import re
import shlex
import subprocess
from functools import cache
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

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

# ----------------------------------------------------------------------------------- export

#: The worked example's runs: panel id, cache label (None: fMRIPrep's own), role, label, sub.
#: larmorx comes first: its bias field is stored as it is, the others as the exclusive or of
#: their bits with it, and each corrected image as the exclusive or with input ÷ bias field.
RUNS = [
    ("larmorx", "larmorx-1", "larmorx", "larmorx (port), 1 thread", "default mode"),
    ("larmorx-12", "larmorx-12", "larmorx", "larmorx (port), 12 threads", "default mode"),
    ("larmorx-reproduce", "larmorx-1-reproduce", "larmorx", "larmorx (port), 1 thread",
     "reproduce mode"),
    ("larmorx-12-reproduce", "larmorx-12-reproduce", "larmorx", "larmorx (port), 12 threads",
     "reproduce mode"),
    ("ants-1-cr", "ants-1-cr", "reference", "ANTs 2.6.5, 1 thread, correctly rounded maths",
     "the same binary, CORE-MATH preloaded"),
    ("image-1-cr", "fmriprep-image-1-cr", "reference",
     "ANTs 2.6.2 (fMRIPrep's image), 1 thread, correctly rounded maths", "CORE-MATH preloaded"),
    ("ants-1", "ants-1", "original", "ANTs 2.6.5, 1 thread, as built",
     "the repository's oracle build (GCC 14.3), glibc's maths"),
    ("image-1", "fmriprep-image-1", "original", "ANTs 2.6.2 (fMRIPrep's image), 1 thread",
     "conda-forge's build, glibc's maths"),
    ("ants-8", "ants-8", "original", "ANTs 2.6.5, 8 threads",
     "the repository's oracle build, glibc's maths"),
    ("fmriprep", None, "fmriprep", "fMRIPrep 25.2.6's own output",
     "ANTs 2.6.2 in its image, 8 threads (NSLOTS=8), glibc's maths"),
]
OUTPUTS = [
    {"id": "corrected", "label": "Corrected image", "unit": ""},
    {"id": "bias", "label": "Bias field", "unit": ""},
]
DATA = {
    "dataset": "OpenNeuro ds000005 (Tom et al., 2007), snapshot 00001",
    "subject": "sub-01",
    "licence": "PDDL-1.0",
}


def _diffs() -> list[dict]:
    """The comparisons (A − B), in the order of the metrics table."""

    def d(i, a, b, kind, label, role):
        return {"id": i, "a": a, "b": b, "kind": kind, "label": label, "role_label": role}

    lx, ants, itself = "larmorx against the original", "the original against itself", None
    out = [
        d("lx-ants1", "larmorx", "ants-1", "main", "larmorx − ANTs 2.6.5, 1 thread, as built", lx),
        d("lx-cr", "larmorx", "ants-1-cr", "main",
          "larmorx − ANTs 2.6.5, 1 thread, correctly rounded maths", lx),
        d("lx-fmriprep", "larmorx", "fmriprep", "fmriprep", "larmorx − fMRIPrep's own output",
          "larmorx against fMRIPrep"),
        d("lx12-lx1", "larmorx-12", "larmorx", "check", "larmorx, 12 threads − 1 thread",
          "larmorx against itself"),
        d("lx-repro", "larmorx", "larmorx-reproduce", "check",
          "larmorx, default − reproduce mode", "larmorx against itself"),
    ]
    out += [
        d(f"ants{t}-ants1", f"ants-{t}", "ants-1", "variability",
          f"ANTs 2.6.5, {t} threads − 1 thread", ants)
        for t in BAND
    ]
    out += [
        d("fmriprep-ants8", "fmriprep", "ants-8", "variability",
          "fMRIPrep's own output − ANTs 2.6.5, 8 threads", ants),
        d("image1-ants1", "image-1", "ants-1", "variability",
          "ANTs 2.6.2 − ANTs 2.6.5, 1 thread each, as built", ants),
        d("image1cr-ants1cr", "image-1-cr", "ants-1-cr", "variability",
          "ANTs 2.6.2 − ANTs 2.6.5, 1 thread each, correctly rounded", ants),
    ]
    del itself
    return out


def _verdict(d: dict, same: dict[str, bool], reg: dict) -> None:
    """A comparison's verdict and reason, decided from the metrics, never assumed."""
    i, links = d["id"], []
    if d["kind"] == "variability":
        d["verdict"] = None
        if same[i]:
            d["reason"] = (
                "The same bytes: ANTs' version (2.6.2 from conda-forge, 2.6.5 built here) and "
                "build do not change this result at the same thread count and maths."
            )
        else:
            d["reason"] = (
                "ANTs' own result moves with its thread count: ITK's B-spline fit sums "
                "per-thread lattices, and float addition is not associative."
            )
            links.append({"label": "ANTS-37", "href": reg["ANTS-37"]["link"]})
    elif same[i]:
        d["verdict"] = "bit-identical"
        d["reason"] = {
            "lx-cr": "With correctly rounded maths, ANTs computes exactly what larmorx computes.",
            "lx12-lx1": "larmorx gives ANTs' single-thread result on any number of threads.",
            "lx-repro": "No correction of the register applies to this command: both modes give "
            "the same bytes (ANTS-36's notice, that -r without a mask rescales nothing, is "
            "printed in both).",
        }.get(i, "The same bytes.")
        if i == "lx-repro":
            links.append({"label": "ANTS-36", "href": reg["ANTS-36"]["link"]})
    elif i == "lx-ants1" and same["lx-cr"]:
        d["verdict"] = "identical-cr"
        d["reason"] = (
            "Only where glibc's expf, logf, exp or sincos is not correctly rounded: with CORE-MATH "
            "preloaded, the same ANTs binary gives larmorx's output bit for bit (next row)."
        )
        links.append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
    elif i == "lx-fmriprep" and same["lx-cr"] and same["fmriprep-ants8"]:
        d["verdict"] = "within-variability"
        d["reason"] = (
            "fMRIPrep's output is ANTs at 8 threads (bit-identical to ANTs 2.6.5 at 8 threads), "
            "larmorx's is ANTs at 1 thread with correctly rounded maths: this is how two runs of "
            "ANTs itself differ (its thread count, glibc's maths)."
        )
        links.append({"label": "ANTS-37", "href": reg["ANTS-37"]["link"]})
    else:
        d["verdict"] = "open"
        d["reason"] = "Not accounted for by the other runs of this example."
    d["links"] = links


def _short_command(command: str) -> str:
    """fMRIPrep's command line, its working directory's long paths shortened."""
    out = command.replace("/work/fmriprep_25_2_wf/sub_01_wf/anat_fit_wf/", "…/")
    return re.sub(r"\s+", " ", out).strip()


def _example(ctx: core.Context, e: fcache.Entry) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    run = _oracles() / "fmriprep-25.2.6" / RUN
    reg = core.register(ctx.repo)
    t1_img = nib.load(str(run / INPUT))
    brain = _load(str(run / MASK)) > 0
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(t1_img, show)
    ex = exm.Example(ctx, grid, show)

    def quotient(pid: str):
        ref = np.zeros(grid.shape, np.float32)
        np.divide(ex.shown["t1w:all"], ex.shown[f"{pid}:bias"], out=ref, where=ex.m)
        return ref, {"div": ["t1w:all", f"{pid}:bias"]}

    ex.predict["corrected"] = quotient
    ex.add(exm.Panel("t1w", "input", "Input: ds000005 sub-01 T1w",
                     "as fMRIPrep passed it, after its DenoiseImage",
                     {"all": _load(str(run / INPUT))}))
    ex.add(exm.Panel("weight", "input", "Weight image (-w)",
                     "fMRIPrep's white-matter posterior × prior", {"all": _load(str(run / WEIGHT))}))
    for pid, label, role, text, sub in RUNS:
        c, b = (
            (run / NODE / OUT_C, run / NODE / OUT_B)
            if label is None
            else (e.dir / f"{label}.nii.gz", e.dir / f"{label}-bias.nii.gz")
        )
        ex.add(exm.Panel(pid, role, text, sub, {"bias": _load(str(b)), "corrected": _load(str(c))}))
    band = {
        f"ants-{t}": {o: _load(str(e.dir / f"ants-{t}{s}.nii.gz")) for o, s in
                      (("corrected", ""), ("bias", "-bias"))}
        for t in BAND
    }
    shown_ids = {p.id for p in ex.panels}
    diffs = _diffs()
    for d in diffs:
        d["metrics"], disp = {}, {}
        for o in OUTPUTS:
            if d["a"] in shown_ids and d["b"] in shown_ids:
                r = ex.compare(d["a"], d["b"], o["id"])
                d["metrics"][o["id"]], disp[o["id"]] = r["metrics"], r["display"]
            else:
                full = {**{p.id: p.full for p in ex.panels}, **band}
                d["metrics"][o["id"]] = core.compare(full[d["a"]][o["id"]], full[d["b"]][o["id"]])
        if disp:
            d["display"] = disp
    same = {d["id"]: all(m["identical"] for m in d["metrics"].values()) for d in diffs}
    for d in diffs:
        _verdict(d, same, reg)
    by = {d["id"]: d for d in diffs}

    # the chain from fMRIPrep's output to larmorx's: every identity asserted from the metrics
    for i in ("fmriprep-ants8", "image1-ants1", "image1cr-ants1cr", "lx-cr", "lx12-lx1", "lx-repro"):
        if not same[i]:
            raise AssertionError(f"{TOOL}: the chain assumes {i} is bit-identical")
    labels = {p.id: (p.label, p.sub, p.role) for p in ex.panels}

    def node(pid, label=None, sub=None):
        lab, s, role = labels[pid]
        return {"label": label or lab, "sub": sub if sub is not None else s, "role": role}

    chain = {
        "title": "What separates fMRIPrep's output from larmorx's (the corrected image)",
        "steps": [
            {"nodes": [node("fmriprep"), node("ants-8")]},
            {"nodes": [node("ants-1"), node("image-1")]},
            {"nodes": [node("ants-1-cr"), node("image-1-cr"),
                       node("larmorx", "larmorx (port)", "1 and 12 threads, default and reproduce")]},
        ],
        "links": [
            {"cause": "1 thread instead of 8",
             "max_abs": by["ants8-ants1"]["metrics"]["corrected"]["max_abs"]},
            {"cause": "correctly rounded maths instead of glibc's",
             "max_abs": by["lx-ants1"]["metrics"]["corrected"]["max_abs"]},
        ],
        "note": "Each box holds runs that give the same bytes, both outputs. fMRIPrep's ANTs "
        "2.6.2 and the repository's ANTs 2.6.5 agree bit for bit at equal thread counts and "
        "maths, so the version plays no part; ANTs' thread count and glibc's maths are the only "
        "two causes, and larmorx computes ANTs' result at one thread with correctly rounded "
        "maths, on any number of threads.",
    }

    # the default view: an axial slice through the largest shown difference from ANTs as built
    at = by["lx-ants1"]["display"]["corrected"]["at"]
    plane, k = "axial", at[2]
    outputs = []
    for o in OUTPUTS:
        vals = ex.values(f"larmorx:{o['id']}")[ex.m]
        outputs.append({**o, "windows": exm.windows(vals)})
    panels = ex.describe_panels()
    for p in panels:
        if p["role"] == "input":
            w = exm.windows(ex.values(f"{p['id']}:all")[ex.m])[0]
            p["window"] = [w["lo"], w["hi"]] if p["id"] == "t1w" else [0.0, w["hi"]]
    default_panels = ["t1w", "fmriprep", "ants-1", "larmorx"]
    default_diffs = ["lx-ants1", "ants8-ants1", "lx-cr", "lx-fmriprep"]
    win = {p["id"]: p.get("window") for p in panels}
    ow = outputs[0]["windows"][0]
    post_p = {}
    for pid in default_panels:
        lo, hi = win[pid] if win.get(pid) else (ow["lo"], ow["hi"])
        a = ex.values(ex.key(pid, "corrected"))
        post_p[pid] = exm.poster_gray(ctx, exm.plane_slice(a, plane, k),
                                      exm.plane_slice(ex.m, plane, k), lo, hi)
    shared = [i for i in default_diffs if not by[i]["metrics"]["corrected"]["identical"]]
    rng = max(by[i]["display"]["corrected"]["max_abs"] for i in shared)
    post_d, ranges = {}, {}
    for i in shared:
        x = by[i]
        dv = ex.values(ex.key(x["a"], "corrected")) - ex.values(ex.key(x["b"], "corrected"))
        post_d[i] = exm.poster_diff(ctx, exm.plane_slice(dv, plane, k),
                                    exm.plane_slice(ex.m, plane, k), rng)
        ranges[i] = rng
    shape = grid.describe()["shape"]
    n_vox = int(np.prod(t1_img.shape))
    rec = records.load(ctx.repo, TOOL)
    th = {r["Quantity"]: r["Requirement"] for r in rec.table("Quantity", "Requirement").rows}
    vers = info.get("versions", {})
    sha = {k.split(" (")[0]: v for k, v in info.get("inputs", {}).items()}
    return {
        "id": "final-n4",
        "title": "fMRIPrep's final N4 of a real T1w, by fMRIPrep, ANTs and larmorx",
        "summary": (
            "sMRIPrep's last N4 of its brain extraction, weighted by the white-matter "
            "posterior, as fMRIPrep 25.2.6 ran it on OpenNeuro ds000005 sub-01: the same "
            "arguments and the same input files, read from the oracle run's working directory, "
            "given to fMRIPrep's own ANTs, to the repository's ANTs 2.6.5 and to larmorx."
        ),
        "facts": [
            {"label": "Data", "text": f"{DATA['dataset']}, {DATA['subject']}'s T1w "
             f"({DATA['licence']}); shown inside fMRIPrep's brain mask, dilated by 2 voxels"},
            {"label": "Node", "text": "fMRIPrep 25.2.6, `anat_fit_wf/brain_extraction_wf/"
             "atropos_wf/inu_n4_final` (sMRIPrep's final N4), run with `NSLOTS=8`"},
            {"label": "Command", "text": "`" + _short_command(info["command"]) + "`"},
            {"label": "Programs", "text": f"{vers.get('ants', '?')}; "
             f"{vers.get('ants_in_fmriprep_image', '?')} in fMRIPrep's image; CORE-MATH's "
             "correctly rounded functions preloaded where stated; larmorx at commit "
             f"`{info['larmorx_commit'][:12]}`"},
            {"label": "Grid", "text": f"{' × '.join(map(str, t1_img.shape))} voxels of "
             f"{' × '.join(f'{z:.3g}' for z in t1_img.header.get_zooms())} mm; shown: "
             f"{' × '.join(map(str, shape))} around the brain"},
        ],
        "grid": grid.describe(),
        "mask": core.blob(ctx, ex.mask),
        "outputs": outputs,
        "panels": panels,
        "diffs": diffs,
        "default": {
            "output": "corrected", "plane": plane, "voxel": at[:3],
            "panels": default_panels, "diffs": default_diffs,
            "compare": {"a": "ants-1", "b": "larmorx"},
        },
        "poster": {
            "plane": plane, "slice": k, "aspect": exm.aspect(grid, plane),
            "panels": post_p, "diffs": post_d, "ranges": ranges,
            "caption": f"Axial slice {k + 1} of {shape[2]} of the shown grid, through the largest "
            "difference between larmorx and ANTs as built; the corrected image, in the brain's "
            "0.5–99.5 % window.",
        },
        "chain": chain,
        "metrics_note": (
            f"Δ is A − B. Every number is computed in float64 over all {n_vox:,} voxels of the "
            f"{' × '.join(map(str, t1_img.shape))} outputs, not only the brain shown; the range "
            "is B's maximum minus its minimum; NCC is Pearson's correlation. The record's "
            f"thresholds: values {th.get('Values', '?')}. The rows *the original against "
            "itself* are ANTs' own variability on this input, the yardstick for the rest."
        ),
        "provenance": [
            f"Inputs: fMRIPrep 25.2.6's oracle run `{RUN}` (image sha256 "
            f"`{vers.get('fmriprep_image', '?')[:16]}…`): " + "; ".join(
                f"{k} ({v.split('sha256 ')[-1].rstrip(')')[:16]}…)" for k, v in sha.items()
            ),
            f"Runs: figures cache entry `{TOOL}/{CASE}`, computed {info['computed']} with "
            f"larmorx `{info['larmorx_commit'][:12]}`; every command, its environment, threads, "
            "exit status and time are in its `RUN_INFO.json`",
            "Data: " + f"{DATA['dataset']}, {DATA['subject']}, {DATA['licence']}",
        ],
    }


def _cases(ctx: core.Context, rec, reg: dict) -> tuple[list[dict], dict]:
    """Every case of the record, with its verdict: bit-identical to ANTs as built unless the
    record's table lists it as differing (then identical with correctly rounded maths, which
    the record checks for every case), rejected by both, ..."""
    from larmorx_validation.parity.ants_n4 import all_cases

    defs = {c.id: c for c in all_cases()}
    t = rec.table("Case", "Output", "Difference from ANTs as built")
    rows: dict[str, list[dict]] = {}
    for r in t.rows:
        rows.setdefault(records.strip_code(r["Case"]), []).append(r)
    out = []
    for r in rec.results:
        c = defs.get(r["case"])
        status = r["status"]
        reason = ""
        if status == "both-error":
            verdict, reason = "both-reject", _clip(r["reason"])
        elif status == "expected-divergence":
            verdict, reason = "divergence", _clip(r["reason"])
        elif status == "fail":
            verdict, reason = "open", _clip(r["reason"])
        elif r["case"] in rows:
            verdict = "identical-cr"
            reason = "; ".join(
                f"{x['Output']}: as built, {x['Difference from ANTs as built']} "
                f"(÷ ANTs' thread band: {x['max |diff| / band']} max |Δ|, "
                f"{x['(1 - NCC) / band']} 1 − NCC)"
                for x in rows[r["case"]]
            ) + "; bit-identical with correctly rounded maths"
        else:
            verdict = "bit-identical"
            if c is not None and not c.outputs:
                reason = "exit status compared (both 0); the usage text is not compared"
        out.append({
            "id": r["case"], "category": r["category"],
            "description": c.description if c else "",
            "command": _command(c) if c else "",
            "results": {"port": {"outcome": status, "verdict": verdict,
                                 "checks": f"{r['checks_passed']}/{r['checks_total']}",
                                 "reason": reason}},
            **({"example": "final-n4"} if r["case"] == "fmriprep/brain-extraction-final" else {}),
        })
    return out, {}


def _clip(s: str, n: int = 240) -> str:
    s = " ".join(s.split())
    return s if len(s) <= n else s[: n - 1] + "…"


def _command(c) -> str:
    """A case's command line as a reader needs it: placeholders named."""

    def arg(a: str) -> str:
        a = re.sub(r"\{in:([^}]+)\}", lambda m: m.group(1), a)
        a = re.sub(r"\{gen:([^}]+)\}", lambda m: f"<generated: {m.group(1)}>", a)
        a = re.sub(r"\{missing:([^}]+)\}", lambda m: m.group(1), a)
        a = a.replace("{out}", "out.nii.gz").replace("{out2}", "bias.nii.gz").replace("{tmp}/", "")
        return a

    return " ".join([c.program, *(shlex.quote(arg(a)) if " " in arg(a) else arg(a) for a in c.args)])


def _band(rec) -> dict:
    t = rec.table("Case", "Output", "max |diff| / band")
    rows = [
        {
            "case": records.strip_code(r["Case"]),
            "output": r["Output"],
            "ratio_max": records.number(r["max |diff| / band"]),
            "ratio_ncc": records.number(r["(1 - NCC) / band"]),
        }
        for r in t.rows
    ]
    vals = [v for r in rows for v in (r["ratio_max"], r["ratio_ncc"]) if v and v > 0]
    lo = 10 ** np.floor(np.log10(min(vals)))
    hi = 10 ** np.ceil(np.log10(max(vals)))
    ticks = []
    v = lo
    while v <= hi * 1.0001:
        ticks.append([float(v), f"{v:g}×"])
        v *= 10
    n_cases = len({r["case"] for r in rows})
    return {
        "title": f"The {n_cases} cases that differ from ANTs as built: larmorx's difference "
        "divided by ANTs' own difference between thread counts (log scale)",
        "axis": [float(lo), float(hi)],
        "ticks": ticks,
        "rows": rows,
        "note": "From the record's table: ANTs at 2, 3, 4, 8 and 12 threads against 1 thread give "
        "ANTs' thread band for each case and output; a ratio below 1 lies inside it. Every one "
        "of these differences vanishes with correctly rounded maths.",
    }


def _speed(bench) -> dict | None:
    if bench is None:
        return None
    series = [
        {"id": "ANTs 2.6.5|1", "label": "ANTs, 1 thread", "mark": "original"},
        {"id": "ANTs 2.6.5|12", "label": "ANTs, 12 threads", "mark": "original-open"},
        {"id": "larmorx|1", "label": "larmorx, 1 thread", "mark": "larmorx"},
        {"id": "larmorx|12", "label": "larmorx, 12 threads", "mark": "larmorx-open"},
    ]
    from larmorx_validation.figures.ants_n4 import SPEED_LABELS

    rows: dict[str, dict] = {}
    for r in bench.results:
        sid = f"{r['tool']}|{r['operation']}"
        if sid not in {s["id"] for s in series}:
            continue
        row = rows.setdefault(r["input"], {"label": SPEED_LABELS.get(r["input"], r["input"]),
                                           "input": r["input"], "seconds": {}})
        row["seconds"][sid] = r["median"]
    out = list(rows.values())
    for row in out:
        a, b = row["seconds"].get("ANTs 2.6.5|1"), row["seconds"].get("larmorx|1")
        row["speedup"] = a / b if a and b else None
    vals = [v for row in out for v in row["seconds"].values()]
    lo = 10 ** np.floor(np.log10(min(vals)))
    hi = 10 ** np.ceil(np.log10(max(vals)))
    ticks, v = [], lo
    while v <= hi * 1.0001:
        ticks.append([float(v), f"{v:g} s"])
        v *= 10
    env = bench.data.get("environment", {})
    return {
        "title": "Seconds for the whole command (read, N4, write), median of repeated runs, "
        "log scale; the factor is larmorx's speed-up at one thread",
        "axis": [float(lo), float(hi)],
        "ticks": ticks,
        "series": series,
        "rows": out,
        "note": f"From the benchmark report `docs/benchmarks/ants-n4.json` ({env.get('cpu', '?')}, "
        f"{env.get('logical_cpus', '?')} logical CPUs), measured on 2026-10-11 against the "
        "ANTs build of that day; a re-run against the current oracle build is planned.",
    }


def export(ctx: core.Context) -> dict:
    e = entry(ctx)
    rec = records.load(ctx.repo, TOOL)
    reg = core.register(ctx.repo)
    cases, _ = _cases(ctx, rec, reg)
    agreement = core.agreement(cases_for(cases, "port"))
    totals: dict[str, int] = {}
    for c in cases:
        v = c["results"]["port"]["verdict"]
        totals[v] = totals.get(v, 0) + 1
    bench = records.load_bench(ctx.repo, TOOL)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "N4BiasFieldCorrection",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": bench.source() if bench else [],
            "cache": [fcache.provenance(e)],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            {
                "id": "port", "label": "larmorx (port)", "short": "port",
                "record": "docs/validation/ants-n4.md", "status": "validated",
                "summary": {"cases": len(cases), "verdicts": totals},
                "agreement": agreement,
            }
        ],
        "cases": cases,
        "cases_total": len(cases),
        "band": _band(rec),
        "speed": _speed(bench),
        "examples": [_example(ctx, e)],
    }


def cases_for(cases: list[dict], impl: str) -> list[dict]:
    """The cases as one implementation's flat list (category, verdict)."""
    return [
        {"category": c["category"], "verdict": c["results"][impl]["verdict"]}
        for c in cases
        if impl in c["results"]
    ]


if __name__ == "__main__":  # compute the cache entry only: python -m ...evidence.ants_n4
    from larmorx_validation.figures.core import REPO, default_cache

    os.environ.setdefault("PYTHONUNBUFFERED", "1")
    e = entry(core.Context(repo=REPO, site=Path("."), cache=default_cache()))
    print("done", e.dir)
