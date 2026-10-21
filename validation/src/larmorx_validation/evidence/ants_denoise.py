# SPDX-License-Identifier: Apache-2.0
"""DenoiseImage's evidence: fMRIPrep's denoising of a real T1w (sMRIPrep's ``-n Rician -s 1``),
by fMRIPrep, by ANTs 2.6.5 at 1 to 12 threads (twice at 8: its result is not reproducible), with
correctly rounded maths, by fMRIPrep's own ANTs at one thread, and by larmorx (1 and 12 threads,
default and ``reproduce`` modes); ANTS-43 corrected by default on the same run; the record's
cases; the differences against ANTs' own thread band; speed.

The worked example is the node fMRIPrep 25.2.6 ran first in its anatomical workflow
(``anat_template_wf/denoise``) on OpenNeuro ds000005 sub-01's raw T1w, its arguments from the
oracle run's ``command.txt``."""

from __future__ import annotations

import re
import shlex
import subprocess
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "ants-denoise"
CASE = "fmriprep-ds000005-denoise"
RUN = "ds000005-sub-01-noreconall"
NODE = "work/fmriprep_25_2_wf/sub_01_wf/anat_fit_wf/anat_template_wf/denoise/mapflow/_denoise0"
T1W = "openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz"
OUT = "sub-01_T1w_noise_corrected.nii.gz"
MASK = "out/sub-01/anat/sub-01_desc-brain_mask.nii.gz"
#: ANTs at these thread counts (the record's band: 2, 4 and 12), and a second run at 8
THREADS = (1, 2, 3, 4, 8, 12)
LX_THREADS = (1, 12)


def _argv(command: str, inp: str, out: str) -> list[str]:
    argv = shlex.split(command)[1:]
    return [
        inp if a == "/data/sub-01/anat/sub-01_T1w.nii.gz" else out if a == OUT else a for a in argv
    ]


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    ora = common.oracles()
    run = common.fmriprep_run(RUN)
    b, lib, sif = ants_bin(), crmath_preload(), ora / common.FMRIPREP_IMAGE
    if b is None or lib is None or not sif.is_file():
        raise FileNotFoundError("the ANTs oracle, the CORE-MATH preload or fMRIPrep's image")
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    t1 = e.input("T1w (raw, as fMRIPrep read it)", Path(td.get(T1W)))
    e.input("fMRIPrep's denoised T1w", run / NODE / OUT)
    d = e.dir
    runs = [(f"ants-{t}", t, False) for t in THREADS]
    runs += [("ants-8b", 8, False), ("ants-1-cr", 1, True)]
    for label, threads, cr in runs:
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
        if cr:
            env["LD_PRELOAD"] = str(lib)
        e.run(
            label,
            [str(b / "DenoiseImage"), *_argv(command, str(t1), str(d / f"{label}.nii.gz"))],
            env=env,
            unset=common.CLEAN_ENV,
        )
    home = d / ".home"
    home.mkdir(exist_ok=True)
    cmd = ["apptainer", "exec", "--cleanenv", "--home", f"{home}:/home/fmriprep"]
    for k, v in {"NSLOTS": "1", "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}.items():
        cmd += ["--env", f"{k}={v}"]
    cmd += ["-B", f"{t1.parent}:/in:ro", "-B", f"{d}:/out", str(sif), "DenoiseImage"]
    e.run(
        "fmriprep-image-1",
        [*cmd, *_argv(command, f"/in/{t1.name}", "/out/fmriprep-image-1.nii.gz")],
        unset=common.CLEAN_ENV,
    )
    for threads in LX_THREADS:
        for mode in ("fix", "reproduce"):
            label = f"larmorx-{threads}" + ("-reproduce" if mode == "reproduce" else "")
            e.run(
                label,
                fcache.larmorx_cli(
                    "ants", "DenoiseImage", *_argv(command, str(t1), str(d / f"{label}.nii.gz"))
                ),
                env={
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                    "LARMORX_UPSTREAM_DEFECTS": mode,
                },
                unset=common.CLEAN_ENV,
            )
    image_version = subprocess.run(
        ["apptainer", "exec", "--cleanenv", str(sif), "DenoiseImage", "--version"],
        capture_output=True,
        text=True,
        encoding="utf-8",
    ).stdout.strip()
    e.finish(
        versions={
            "ants": ants_version(),
            "ants_in_fmriprep_image": " ".join(image_version.split()),
        },
        command=command,
        threads={"ants": list(THREADS), "larmorx": list(LX_THREADS), "fmriprep_run": 8},
        settings="fMRIPrep 25.2.6's denoising of ds000005 sub-01's raw T1w "
        "(anat_template_wf/denoise), its arguments from command.txt; ANTs at 1, 2, 3, 4, 8 "
        "(twice) and 12 threads, with CORE-MATH at 1, fMRIPrep's own ANTs at 1; larmorx in both "
        "modes at 1 and 12 threads",
    )


#: repeated runs of ANTs at 8 and 12 threads (a race decides each result)
REPEATS = [
    ("ants-8-r3", 8),
    ("ants-8-r4", 8),
    ("ants-8-r5", 8),
    ("ants-8-r6", 8),
    ("ants-12-r2", 12),
    ("ants-12-r3", 12),
]


def _compute_repeats(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.ants_programs import ants_bin, ants_version

    run = common.fmriprep_run(RUN)
    b = ants_bin()
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    t1 = e.input("T1w (raw, as fMRIPrep read it)", Path(td.get(T1W)))
    for label, threads in REPEATS:
        e.run(
            label,
            [str(b / "DenoiseImage"), *_argv(command, str(t1), str(e.dir / f"{label}.nii.gz"))],
            env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)},
            unset=common.CLEAN_ENV,
        )
    e.finish(
        versions={"ants": ants_version()},
        command=command,
        settings="fMRIPrep's DenoiseImage command run again by ANTs 2.6.5: four more times "
        "at 8 threads and twice more at 12, one after another",
    )


def repeats_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE + "-repeats",
        "fMRIPrep's DenoiseImage command repeated by ANTs 2.6.5 at 8 and 12 threads",
        _compute_repeats,
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's DenoiseImage of ds000005 sub-01's T1w: fMRIPrep, ANTs 2.6.5 at 1-12 "
        "threads, CORE-MATH, fMRIPrep's ANTs at 1 thread, and larmorx",
        _compute,
    )


# ----------------------------------------------------------------------------------- export

#: panel id, cache label (None: fMRIPrep's own), role, label, sub-label
RUNS = [
    ("larmorx", "larmorx-1-reproduce", "larmorx", "larmorx (port), 1 thread", "reproduce mode"),
    ("larmorx-12", "larmorx-12-reproduce", "larmorx", "larmorx (port), 12 threads", "reproduce"),
    (
        "larmorx-fix",
        "larmorx-1",
        "larmorx",
        "larmorx (port), 1 thread",
        "default mode: ANTS-43 corrected",
    ),
    ("larmorx-12-fix", "larmorx-12", "larmorx", "larmorx (port), 12 threads", "default mode"),
    (
        "ants-1-cr",
        "ants-1-cr",
        "reference",
        "ANTs 2.6.5, 1 thread, correctly rounded maths",
        "the same binary, CORE-MATH preloaded",
    ),
    (
        "ants-1",
        "ants-1",
        "original",
        "ANTs 2.6.5, 1 thread, as built",
        "the repository's oracle build (GCC 14.3), glibc's maths",
    ),
    (
        "image-1",
        "fmriprep-image-1",
        "original",
        "ANTs 2.6.2 (fMRIPrep's image), 1 thread",
        "conda-forge's build, glibc's maths",
    ),
    ("ants-8", "ants-8", "original", "ANTs 2.6.5, 8 threads", "as built (fMRIPrep ran 8)"),
    ("ants-8b", "ants-8b", "original", "ANTs 2.6.5, 8 threads", "a second run, same command"),
    ("ants-12", "ants-12", "original", "ANTs 2.6.5, 12 threads", "as built"),
    (
        "fmriprep",
        None,
        "fmriprep",
        "fMRIPrep 25.2.6, as it ran",
        "ANTs 2.6.2 in its image, 8 threads (NSLOTS=8)",
    ),
]
OUTPUTS = [{"id": "denoised", "label": "Denoised T1w", "unit": ""}]


def _diffs() -> list[dict]:
    d = common.d
    out = [
        d(
            "lx-ants1",
            "larmorx",
            "ants-1",
            "main",
            "larmorx − ANTs 2.6.5, 1 thread, as built",
            common.ROLE_LX,
        ),
        d(
            "lx-cr",
            "larmorx",
            "ants-1-cr",
            "main",
            "larmorx − ANTs 2.6.5, 1 thread, correctly rounded maths",
            common.ROLE_LX,
        ),
        d(
            "lx-fmriprep",
            "larmorx",
            "fmriprep",
            "fmriprep",
            "larmorx − fMRIPrep's own output",
            common.ROLE_FMRIPREP,
        ),
        d(
            "fix-lx",
            "larmorx-fix",
            "larmorx",
            "main",
            "larmorx default mode − reproduce mode (ANTS-43)",
            "larmorx's correction",
        ),
        d(
            "lx12-lx1",
            "larmorx-12",
            "larmorx",
            "check",
            "larmorx, 12 threads − 1 thread",
            common.ROLE_SELF,
        ),
        d(
            "fix12-fix1",
            "larmorx-12-fix",
            "larmorx-fix",
            "check",
            "larmorx default mode, 12 threads − 1 thread",
            common.ROLE_SELF,
        ),
        d(
            "ants8b-ants8",
            "ants-8b",
            "ants-8",
            "variability",
            "ANTs 2.6.5, 8 threads: another run − run 1",
            common.ROLE_ORIG,
        ),
    ]
    out += [
        d(
            f"ants{t}-ants1",
            f"ants-{t}",
            "ants-1",
            "variability",
            f"ANTs 2.6.5, {t} threads − 1 thread",
            common.ROLE_ORIG,
        )
        for t in THREADS[1:]
    ]
    out += [
        d(
            "fmriprep-ants8",
            "fmriprep",
            "ants-8",
            "variability",
            "fMRIPrep's own output − ANTs 2.6.5, 8 threads",
            common.ROLE_ORIG,
        ),
        d(
            "image1-ants1",
            "image-1",
            "ants-1",
            "variability",
            "ANTs 2.6.2 − ANTs 2.6.5, 1 thread each, as built",
            common.ROLE_ORIG,
        ),
    ]
    return out


def _verdict(x: dict, same: dict[str, bool], reg: dict) -> None:
    i, links = x["id"], []
    if x["kind"] == "variability":
        x["verdict"] = None
        if i == "image1-ants1":
            x["reason"] = (
                "ANTs' version and build play no part at one thread."
                if same[i]
                else "ANTs 2.6.2 and 2.6.5 differ at one thread."
            )
        elif same[i]:
            x["reason"] = "The same bytes."
        elif i == "ants8b-ants8" or i.startswith("rep-"):
            x["reason"] = (
                "The same command again gives another result: ITK's threads share the image in "
                "slabs and its non-local means adds their contributions in the order they finish."
            )
            links.append({"label": "ANTS-42", "href": reg["ANTS-42"]["link"]})
        else:
            x["reason"] = "ANTs' result moves with its thread count (ANTS-42)."
            links.append({"label": "ANTS-42", "href": reg["ANTS-42"]["link"]})
    elif i == "fix-lx":
        x["verdict"] = "d13" if not same[i] else "bit-identical"
        x["reason"] = (
            "ANTS-43, corrected by default with a warning: where ITK's Rician correction factor "
            "overflows to NaN its bias becomes 0; larmorx computes the factor in double with "
            "scaled Bessel functions. Reproduce mode gives ANTs' result."
        )
        links.append({"label": "ANTS-43", "href": reg["ANTS-43"]["link"]})
    elif same[i]:
        x["verdict"] = "bit-identical"
        x["reason"] = {
            "lx-cr": "With correctly rounded maths, ANTs computes exactly what larmorx computes.",
            "lx12-lx1": "larmorx gives ANTs' single-thread result on any number of threads.",
            "fix12-fix1": "The same bytes on any number of threads.",
        }.get(i, "The same bytes.")
    elif i == "lx-ants1" and same["lx-cr"]:
        x["verdict"] = "identical-cr"
        x["reason"] = (
            "Only where glibc's expf is not correctly rounded for a value the run meets: with "
            "CORE-MATH preloaded, the same ANTs binary gives larmorx's output bit for bit."
        )
        links.append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
    elif i == "lx-fmriprep" and same["lx-cr"]:
        x["verdict"] = "within-variability"
        x["reason"] = (
            "fMRIPrep's output is one of ANTs' results at 8 threads, which move with the "
            "threads' order (the reference rows: ANTs at 8 threads against one thread, and "
            "fMRIPrep's run against the repository's at 8); larmorx's is ANTs' one-thread "
            "result with correctly rounded maths."
        )
        links.append({"label": "ANTS-42", "href": reg["ANTS-42"]["link"]})
    else:
        x["verdict"] = "open"
        x["reason"] = "Not accounted for by the other runs of this example."
    x["links"] = links


def _example(ctx: core.Context, reg: dict) -> dict:
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = entry(ctx)
    info = e.info()
    run = common.fmriprep_run(RUN)
    img = nib.load(str(td.get(T1W)))
    brain = common.load(str(run / MASK)) > 0
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(
        exm.Panel(
            "t1w",
            "input",
            "ds000005 sub-01 T1w, raw",
            "int16, as fMRIPrep read it from the dataset",
            {"all": np.asanyarray(img.dataobj.get_unscaled())},
        )
    )
    rep_e = repeats_entry(ctx)
    runs8 = {"ants-8b": e.dir / "ants-8b.nii.gz"}
    runs8.update({k: rep_e.dir / f"{k}.nii.gz" for k, t in REPEATS if t == 8})
    runs12 = {k: rep_e.dir / f"{k}.nii.gz" for k, t in REPEATS if t == 12}
    first8 = common.load(str(e.dir / "ants-8.nii.gz")).tobytes()
    first12 = common.load(str(e.dir / "ants-12.nii.gz")).tobytes()
    bytes8 = {k: common.load(str(v)).tobytes() for k, v in runs8.items()}
    bytes12 = {k: common.load(str(v)).tobytes() for k, v in runs12.items()}
    distinct8 = len({first8, *bytes8.values()})
    distinct12 = len({first12, *bytes12.values()})
    other = next((k for k, b in bytes8.items() if b != first8), "ants-8b")
    nth = {"ants-8b": 2, **{k: i + 3 for i, k in enumerate(k for k, t in REPEATS if t == 8)}}
    full: dict[str, dict] = {}
    for pid, label, role, text, sub in RUNS:
        if pid == "ants-8b":
            label = None
            sub = f"run {nth[other]} of {1 + len(runs8)}, the same command" + (
                "" if bytes8[other] != first8 else ": the same bytes"
            )
        path = (
            run / NODE / OUT
            if pid == "fmriprep"
            else runs8[other]
            if pid == "ants-8b"
            else e.dir / f"{label}.nii.gz"
        )
        arr = {"denoised": common.load(str(path))}
        full[pid] = arr
        ex.add(exm.Panel(pid, role, text, sub, arr))
    for t in THREADS:
        full.setdefault(f"ants-{t}", {"denoised": common.load(str(e.dir / f"ants-{t}.nii.gz"))})
    diffs = _diffs()
    for k, path in [*runs8.items(), *runs12.items()]:
        if k == other:
            continue
        t = 8 if k in runs8 else 12
        full[k] = {"denoised": common.load(str(path))}
        n = nth[k] if k in runs8 else 2 + list(runs12).index(k)
        diffs.append(
            common.d(
                f"rep-{k}",
                k,
                f"ants-{t}",
                "variability",
                f"ANTs 2.6.5, {t} threads: run {n} − run 1",
                common.ROLE_ORIG,
            )
        )
    common.compare_all(ex, diffs, OUTPUTS, full=full)
    same = common.identical(diffs)
    for x in diffs:
        _verdict(x, same, reg)
    for i in ("lx-cr", "lx12-lx1", "fix12-fix1", "image1-ants1"):
        if not same[i]:
            raise AssertionError(f"{TOOL}: the chain assumes {i} is bit-identical")
    by = {x["id"]: x for x in diffs}
    labels = {p.id: (p.label, p.sub, p.role) for p in ex.panels}

    def node(pid, label=None, sub=None):
        lab, s, role = labels[pid]
        return {"label": label or lab, "sub": sub if sub is not None else s, "role": role}

    def results(n: int) -> str:
        return f"{n} distinct result" + ("" if n == 1 else "s")

    same8 = "the same bytes every time" if distinct8 == 1 else results(distinct8)
    chain = {
        "title": "What separates fMRIPrep's output from larmorx's",
        "steps": [
            {"nodes": [node("fmriprep")]},
            {"nodes": [node("ants-8", sub=f"run {1 + len(runs8)} times: {same8}")]},
            {"nodes": [node("ants-1"), node("image-1")]},
            {
                "nodes": [
                    node("ants-1-cr"),
                    node("larmorx", "larmorx (port), reproduce mode", "1 and 12 threads"),
                ]
            },
        ],
        "links": [
            {
                "cause": "fMRIPrep's run at 8 threads",
                "max_abs": by["fmriprep-ants8"]["metrics"]["denoised"]["max_abs"],
            },
            {
                "cause": "1 thread instead of 8",
                "max_abs": by["ants8-ants1"]["metrics"]["denoised"]["max_abs"],
            },
            {
                "cause": "correctly rounded maths instead of glibc's",
                "max_abs": by["lx-ants1"]["metrics"]["denoised"]["max_abs"],
            },
        ],
        "note": f"Each box holds runs that give the same bytes. Here ANTs 2.6.5 gave "
        f"{results(distinct8)} in {1 + len(runs8)} runs at 8 threads (at 12, "
        f"{results(distinct12)} in {1 + len(runs12)}); fMRIPrep's run at 8 threads, with its "
        "own ANTs 2.6.2 build, gave another, although at one thread the two builds give the "
        "same bytes: at several threads the result depends on how the threads share the "
        "image (ANTS-42). larmorx gives ANTs' one-thread result with correctly rounded maths "
        "on any number of threads; its default mode then corrects ANTS-43 (the metrics "
        "below).",
    }
    moved = bytes8[other] != first8
    at, k = common.largest_in_slice(
        ex, "ants-8", "ants-8b" if moved else "ants-1", "denoised", "coronal"
    )
    default = {
        "output": "denoised",
        "plane": "coronal",
        "voxel": at,
        "panels": ["t1w", "ants-1", "ants-8", "larmorx"],
        "diffs": (["ants8b-ants8"] if moved else ["fmriprep-ants8"])
        + ["ants8-ants1", "lx-ants1", "fix-lx"],
        "compare": {"a": "ants-8", "b": "ants-8b" if moved else "ants-1"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        OUTPUTS,
        default=default,
        window_panel="larmorx",
        scale="log",
        caption=(
            "the crosshair where two of ANTs' runs at 8 threads differ most in it"
            if moved
            else "the crosshair where ANTs at 8 threads differs most from one thread in it"
        )
        + "; ITK's threads split the image into slabs along z, seen here as bands; the "
        "differences on one symmetric log scale over six decades below the largest.",
    )
    vers = info.get("versions", {})
    m = re.search(r"Version:\s*(\S+)", vers.get("ants_in_fmriprep_image", ""))
    image_ants = f"ANTs {m.group(1)} (conda-forge)" if m else "ANTs (version not recorded)"
    n_vox = int(np.prod(img.shape))
    fix = by["fix-lx"]["metrics"]["denoised"]
    return {
        "id": "fmriprep-denoise",
        "title": "fMRIPrep's denoising of a real T1w, by fMRIPrep, ANTs and larmorx",
        "summary": (
            "The first step of fMRIPrep 25.2.6's anatomical workflow on OpenNeuro ds000005 "
            "sub-01: the raw T1w and the arguments it gave DenoiseImage (sMRIPrep's Rician "
            "model), given to the repository's ANTs 2.6.5 at 1 to 12 threads (twice at 8), with "
            "correctly rounded maths, to fMRIPrep's own ANTs at one thread, and to larmorx in "
            "both modes."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01's T1w "
                f"({common.DS000005['licence']}); shown inside fMRIPrep's brain mask, dilated "
                "by 2 voxels",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `anat_fit_wf/anat_template_wf/denoise`, run with "
                "`NSLOTS=8`",
            },
            {"label": "Command", "text": "`" + " ".join(info["command"].split()) + "`"},
            {
                "label": "Programs",
                "text": f"{vers.get('ants', '?')}; {image_ants} in fMRIPrep's image; CORE-MATH "
                "preloaded where stated; larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`",
            },
            {
                "label": "Repeats",
                "text": f"the same command by ANTs 2.6.5 at 8 threads {1 + len(runs8)} times: "
                f"{results(distinct8)}; at 12 threads {1 + len(runs12)} times: "
                f"{results(distinct12)} (2026-10-21; earlier runs at 8 threads differed, "
                "[findings](repo:docs/findings/ants-denoise.md))",
            },
            {
                "label": "ANTS-43",
                "text": f"larmorx's default mode changes {fix['differ']:,} of {fix['voxels']:,} "
                f"values, by up to {fix['max_abs']:.3g} (the metrics below)",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": chain,
        "metrics_note": (
            f"Δ is A − B. Every number is computed in float64 over all {n_vox:,} voxels of the "
            f"{' × '.join(map(str, img.shape))} outputs, not only the brain shown; the range is "
            "B's maximum minus its minimum. The rows *the original against itself* are ANTs' "
            "own variability on this input: its thread counts, two runs at 8 threads, its "
            "version."
        ),
        "provenance": common.provenance(
            info, f"{TOOL}/{CASE} and {TOOL}/{CASE}-repeats", [common.DS000005]
        ),
    }


# ------------------------------------------------------------------------------------ cases


def _cases(ctx: core.Context, rec) -> list[dict]:
    from larmorx_validation.parity.ants_denoise import all_cases

    defs = {c.id: c for c in all_cases()}
    t = rec.table("Case", "Output", "Difference from ANTs as built")
    rows: dict[str, list[dict]] = {}
    for r in t.rows:
        rows.setdefault(records.strip_code(r["Case"]), []).append(r)
    out = []
    for r in rec.results:
        c = defs.get(r["case"])
        v = common.outcome_verdict(r)
        if v is None and r["case"] in rows:
            v = (
                "identical-cr",
                "; ".join(
                    f"{x['Output']}: as built, {x['Difference from ANTs as built']} "
                    f"(÷ ANTs' thread band: {x['max |diff| / band']} max |Δ|, "
                    f"{x['(1 - NCC) / band']} 1 − NCC)"
                    for x in rows[r["case"]]
                )
                + "; bit-identical with correctly rounded maths",
            )
        elif v is None:
            v = ("bit-identical", "")
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": c.description if c else "",
                "command": _command(c) if c else "",
                "results": {
                    "port": {
                        "outcome": r["status"],
                        "verdict": v[0],
                        "checks": f"{r['checks_passed']}/{r['checks_total']}",
                        "reason": common.clip(v[1], 400),
                    }
                },
                **({"example": "fmriprep-denoise"} if r["case"] == "fmriprep/smriprep-t1w" else {}),
            }
        )
    return out


def _command(c) -> str:
    def sub(a: str) -> str:
        a = re.sub(r"\{in:([^}]+)\}", lambda m: m.group(1), a)
        a = re.sub(r"\{gen:([^}]+)\}", lambda m: f"<generated: {m.group(1)}>", a)
        a = re.sub(r"\{missing:([^}]+)\}", lambda m: m.group(1), a)
        return (
            a.replace("{out}", "out.nii.gz").replace("{out2}", "noise.nii.gz").replace("{tmp}/", "")
        )

    return common.command_text(c.program if hasattr(c, "program") else "DenoiseImage", c.args, sub)


def _band(rec) -> dict:
    """The record's table of the cases that differ from ANTs as built, each divided by ANTs'
    own thread band; a case whose band is 0 (ANTs' threads change nothing: the ratio is
    infinite) is named in the note instead of drawn."""
    import math

    t = rec.table("Case", "Output", "max |diff| / band")
    rows, banded = [], []
    for r in t.rows:
        a, b = records.number(r["max |diff| / band"]), records.number(r["(1 - NCC) / band"])
        row = {
            "case": records.strip_code(r["Case"]),
            "output": r["Output"],
            "ratio_max": a if math.isfinite(a) else None,
            "ratio_ncc": b if math.isfinite(b) else None,
        }
        if row["ratio_max"] is None and row["ratio_ncc"] is None:
            banded.append(f"`{row['case']}` ({row['output']})")
            continue
        rows.append(row)
    vals = [v for r in rows for v in (r["ratio_max"], r["ratio_ncc"]) if v and v > 0]
    axis, ticks = common.ticks_log(vals)
    ticks = [[v, label + "×"] for v, label in ticks]
    n_cases = len({r["case"] for r in rows})
    return {
        "title": f"The {n_cases} cases that differ from ANTs as built: larmorx's difference "
        "divided by ANTs' own difference between thread counts (log scale)",
        "axis": axis,
        "ticks": ticks,
        "rows": rows,
        "note": "From the record's table: ANTs at 2, 4 and 12 threads against 1 thread give ANTs' "
        "thread band for each case and output; a ratio below 1 lies inside it, and a 1 − NCC of "
        "0 is not drawn. Every one of these differences vanishes with correctly rounded maths."
        + (
            " Not drawn: " + ", ".join(banded) + ", where ANTs' threads change nothing (a band "
            "of 0) and larmorx differs from ANTs as built by glibc's maths alone."
            if banded
            else ""
        ),
    }


def export(ctx: core.Context) -> dict:
    reg = core.register(ctx.repo)
    rec = records.load(ctx.repo, TOOL)
    cases = _cases(ctx, rec)
    bench = records.load_bench(ctx.repo, TOOL)
    from larmorx_validation.figures.ants_denoise import SPEED_LABELS

    speed = common.speed(
        bench,
        [
            {"id": "ANTs 2.6.5|1", "label": "ANTs, 1 thread", "mark": "original"},
            {"id": "ANTs 2.6.5|12", "label": "ANTs, 12 threads", "mark": "original-open"},
            {"id": "larmorx|1", "label": "larmorx, 1 thread", "mark": "larmorx"},
            {"id": "larmorx|12", "label": "larmorx, 12 threads", "mark": "larmorx-open"},
        ],
        SPEED_LABELS,
        title="Seconds for the whole command (read, denoise, write), median of repeated runs, "
        "log scale; the factor is larmorx's speed-up at one thread",
        base=("ANTs 2.6.5|1", "larmorx|1"),
        note="From the benchmark report `docs/benchmarks/ants-denoise.json` ({cpu}, {cpus} "
        "logical CPUs).",
    )
    e = entry(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "DenoiseImage",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": bench.source() if bench else [],
            "cache": [fcache.provenance(e), fcache.provenance(repeats_entry(ctx))],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cases, "port", "larmorx (port)", "port", f"docs/validation/{TOOL}.md", "validated"
            )
        ],
        "cases": cases,
        "cases_total": len(cases),
        "band": _band(rec),
        "speed": speed,
        "examples": [_example(ctx, reg)],
    }


if __name__ == "__main__":  # compute the cache entry only
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    print("done", entry(c).dir, repeats_entry(c).dir)
